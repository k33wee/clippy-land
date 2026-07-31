use super::{
    ClipboardEntry, ClipboardThumbnail, MAX_FULL_FRAME_THUMBNAIL_BYTES, THUMBNAIL_SIZE_PX,
    debug_log, max_image_bytes, max_image_dimension_px,
};
use bytes::Bytes;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::Path;

pub(super) fn clipboard_entry_from_image_bytes(
    mime: String,
    bytes: Vec<u8>,
) -> Option<ClipboardEntry> {
    let max_image_bytes = max_image_bytes();
    if bytes.is_empty() || bytes.len() > max_image_bytes {
        return None;
    }

    let mut hasher = DefaultHasher::new();
    mime.hash(&mut hasher);
    bytes.hash(&mut hasher);
    let hash = hasher.finish();

    Some(ClipboardEntry::Image {
        mime,
        bytes: bytes.into(),
        hash,
        thumbnail_png: None,
    })
}

pub(super) fn clipboard_entry_from_image_path(path: &Path) -> Option<ClipboardEntry> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => return None,
    };

    let bytes = std::fs::read(path).ok()?;
    clipboard_entry_from_image_bytes(mime.to_string(), bytes)
}

pub fn make_thumbnail(mime: &str, bytes: &Bytes) -> Option<ClipboardThumbnail> {
    if mime == "image/png" {
        return make_png_thumbnail(bytes);
    }

    let max_dimension = max_image_dimension_px();
    let format = match mime {
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        _ => image::guess_format(bytes).ok()?,
    };

    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(max_dimension);
    limits.max_image_height = Some(max_dimension);
    limits.max_alloc = Some(MAX_FULL_FRAME_THUMBNAIL_BYTES);
    reader.limits(limits);
    let decoded = match reader.decode() {
        Ok(decoded) => decoded,
        Err(err) => {
            debug_log(format!("clipboard thumbnail decode skipped: {err}"));
            return None;
        }
    };

    encode_thumbnail(decoded)
}

fn make_png_thumbnail(bytes: &Bytes) -> Option<ClipboardThumbnail> {
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: max_image_bytes(),
        },
    );
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(png::Transformations::normalize_to_color8());

    let mut reader = decoder.read_info().map_err(log_png_decode_error).ok()?;
    let info = reader.info();
    let (source_width, source_height) = (info.width, info.height);
    if source_width == 0
        || source_height == 0
        || source_width > max_image_dimension_px()
        || source_height > max_image_dimension_px()
    {
        debug_log(format!(
            "clipboard thumbnail decode skipped: invalid dimensions {source_width}x{source_height}"
        ));
        return None;
    }

    if info.interlaced {
        if reader.output_buffer_size()? as u64 > MAX_FULL_FRAME_THUMBNAIL_BYTES {
            debug_log("clipboard thumbnail decode skipped: interlaced PNG frame is too large");
            return None;
        }
        return decode_png_frame(reader, source_width, source_height);
    }

    let (width, height) = thumbnail_dimensions(source_width, source_height);
    let channels = png_channels(reader.output_color_type())?;
    let output_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    let sample_x: Vec<usize> = (0..width)
        .map(|x| sample_source_coordinate(x, width, source_width) as usize)
        .collect();
    let mut rgba = Vec::with_capacity(output_len);
    let mut source_y = 0u32;
    let mut output_y = 0u32;

    while let Some(row) = reader.next_row().map_err(log_png_decode_error).ok()? {
        if output_y < height
            && source_y == sample_source_coordinate(output_y, height, source_height)
        {
            for source_x in &sample_x {
                let offset = source_x.checked_mul(channels)?;
                let end = offset.checked_add(channels)?;
                rgba.extend_from_slice(&png_pixel_to_rgba(row.data().get(offset..end)?, channels));
            }
            output_y += 1;
        }
        source_y += 1;
    }

    if source_y != source_height || output_y != height || rgba.len() != output_len {
        debug_log("clipboard thumbnail decode skipped: incomplete PNG image data");
        return None;
    }

    encode_rgba_thumbnail(width, height, rgba)
}

fn decode_png_frame(
    mut reader: png::Reader<Cursor<&Bytes>>,
    width: u32,
    height: u32,
) -> Option<ClipboardThumbnail> {
    let buffer_len = reader.output_buffer_size()?;
    let mut decoded = vec![0; buffer_len];
    let output = reader
        .next_frame(&mut decoded)
        .map_err(log_png_decode_error)
        .ok()?;
    let decoded = match (output.color_type, output.bit_depth) {
        (png::ColorType::Grayscale, png::BitDepth::Eight) => {
            image::DynamicImage::ImageLuma8(image::ImageBuffer::from_raw(width, height, decoded)?)
        }
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => {
            image::DynamicImage::ImageLumaA8(image::ImageBuffer::from_raw(width, height, decoded)?)
        }
        (png::ColorType::Rgb, png::BitDepth::Eight) => {
            image::DynamicImage::ImageRgb8(image::ImageBuffer::from_raw(width, height, decoded)?)
        }
        (png::ColorType::Rgba, png::BitDepth::Eight) => {
            image::DynamicImage::ImageRgba8(image::ImageBuffer::from_raw(width, height, decoded)?)
        }
        _ => return None,
    };
    encode_thumbnail(decoded)
}

fn png_channels((color, depth): (png::ColorType, png::BitDepth)) -> Option<usize> {
    (depth == png::BitDepth::Eight).then(|| color.samples())
}

fn png_pixel_to_rgba(pixel: &[u8], channels: usize) -> [u8; 4] {
    match channels {
        1 => [pixel[0], pixel[0], pixel[0], 255],
        2 => [pixel[0], pixel[0], pixel[0], pixel[1]],
        3 => [pixel[0], pixel[1], pixel[2], 255],
        4 => [pixel[0], pixel[1], pixel[2], pixel[3]],
        _ => unreachable!("PNG decoder only produces 1-4 channels"),
    }
}

fn thumbnail_dimensions(width: u32, height: u32) -> (u32, u32) {
    if width <= THUMBNAIL_SIZE_PX && height <= THUMBNAIL_SIZE_PX {
        return (width, height);
    }

    let scale =
        (THUMBNAIL_SIZE_PX as f64 / width as f64).min(THUMBNAIL_SIZE_PX as f64 / height as f64);
    (
        (width as f64 * scale).round().max(1.0) as u32,
        (height as f64 * scale).round().max(1.0) as u32,
    )
}

fn sample_source_coordinate(value: u32, target_size: u32, source_size: u32) -> u32 {
    let numerator = (u64::from(value) * 2 + 1) * u64::from(source_size);
    let denominator = u64::from(target_size) * 2;
    (numerator / denominator).min(u64::from(source_size - 1)) as u32
}

fn log_png_decode_error(err: png::DecodingError) {
    debug_log(format!("clipboard thumbnail decode skipped: {err}"));
}

fn encode_thumbnail(decoded: image::DynamicImage) -> Option<ClipboardThumbnail> {
    let thumb = if decoded.width() <= THUMBNAIL_SIZE_PX && decoded.height() <= THUMBNAIL_SIZE_PX {
        decoded.into_rgba8()
    } else {
        decoded
            .thumbnail(THUMBNAIL_SIZE_PX, THUMBNAIL_SIZE_PX)
            .into_rgba8()
    };
    encode_rgba_thumbnail(thumb.width(), thumb.height(), thumb.into_raw())
}

fn encode_rgba_thumbnail(width: u32, height: u32, rgba: Vec<u8>) -> Option<ClipboardThumbnail> {
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fastest);
    encoder.set_filter(png::Filter::NoFilter);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&rgba).ok()?;
    drop(writer);

    Some(ClipboardThumbnail {
        width,
        height,
        rgba: rgba.into(),
        png: png.into(),
    })
}

pub(super) fn log_image_too_large(len: usize) {
    let max_image_bytes = max_image_bytes();
    debug_log(format!(
        "clipboard image ignored (too large): {} bytes (max {})",
        len, max_image_bytes
    ));
}
