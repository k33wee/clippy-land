use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Read};

use wl_clipboard_rs::{
    copy::{MimeType as CopyMimeType, Options as CopyOptions, Source},
    paste::{get_contents, ClipboardType, MimeType as PasteMimeType, Seat},
};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const THUMBNAIL_SIZE_PX: u32 = 40;

#[derive(Debug, Clone)]
pub enum ClipboardEntry {
    Text(String),
    Image {
        mime: String,
        bytes: Vec<u8>,
        hash: u64,
        thumbnail_png: Option<Vec<u8>>,
    },
}

impl ClipboardEntry {
    pub fn fingerprint(&self) -> ClipboardFingerprint {
        match self {
            ClipboardEntry::Text(text) => ClipboardFingerprint::Text(text.clone()),
            ClipboardEntry::Image {
                mime,
                bytes,
                hash,
                thumbnail_png: _,
            } => ClipboardFingerprint::Image {
                mime: mime.clone(),
                bytes_len: bytes.len(),
                hash: *hash,
            },
        }
    }
}

impl PartialEq for ClipboardEntry {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ClipboardEntry::Text(a), ClipboardEntry::Text(b)) => a == b,
            (
                ClipboardEntry::Image {
                    mime: am,
                    bytes: ab,
                    hash: ah,
                    ..
                },
                ClipboardEntry::Image {
                    mime: bm,
                    bytes: bb,
                    hash: bh,
                    ..
                },
            ) => ah == bh && am == bm && ab.len() == bb.len(),
            _ => false,
        }
    }
}

impl Eq for ClipboardEntry {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardFingerprint {
    Text(String),
    Image {
        mime: String,
        bytes_len: usize,
        hash: u64,
    },
}

pub fn read_clipboard_entry() -> Option<ClipboardEntry> {
    read_clipboard_image().or_else(|| read_clipboard_text().map(ClipboardEntry::Text))
}

pub fn read_clipboard_text() -> Option<String> {
    let result = get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        PasteMimeType::Text,
    );

    let (mut pipe, _) = match result {
        Ok(ok) => ok,
        Err(err) => {
            if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
                eprintln!("[clippy-land] clipboard read get_contents error: {err:?}");
            }
            return None;
        }
    };

    let mut bytes = Vec::new();
    if let Err(err) = pipe.read_to_end(&mut bytes) {
        if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
            eprintln!("[clippy-land] clipboard read pipe error: {err:?}");
        }
        return None;
    }

    let text = match String::from_utf8(bytes) {
        Ok(ok) => ok,
        Err(err) => {
            if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
                eprintln!("[clippy-land] clipboard read utf8 error: {err:?}");
            }
            return None;
        }
    };
    let text = text.trim_end_matches(['\n', '\r']).to_string();
    (!text.is_empty()).then_some(text)
}

pub fn read_clipboard_image() -> Option<ClipboardEntry> {
    // Try common image formats first.
    const IMAGE_MIMES: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];

    for mime in IMAGE_MIMES {
        let result = get_contents(
            ClipboardType::Regular,
            Seat::Unspecified,
            PasteMimeType::Specific(mime),
        );

        let (pipe, actual_mime) = match result {
            Ok(ok) => ok,
            Err(_) => continue,
        };

        let mut bytes = Vec::new();
        let mut limited = pipe.take((MAX_IMAGE_BYTES + 1) as u64);
        if limited.read_to_end(&mut bytes).is_err() {
            continue;
        }
        if bytes.len() > MAX_IMAGE_BYTES {
            if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
                eprintln!(
                    "[clippy-land] clipboard image ignored (too large): {} bytes (max {})",
                    bytes.len(),
                    MAX_IMAGE_BYTES
                );
            }
            continue;
        }
        if bytes.is_empty() {
            continue;
        }

        let mut hasher = DefaultHasher::new();
        actual_mime.hash(&mut hasher);
        bytes.hash(&mut hasher);
        let hash = hasher.finish();

        let thumbnail_png = make_thumbnail_png(&actual_mime, &bytes);

        return Some(ClipboardEntry::Image {
            mime: actual_mime,
            bytes,
            hash,
            thumbnail_png,
        });
    }

    None
}

fn make_thumbnail_png(mime: &str, bytes: &[u8]) -> Option<Vec<u8>> {
    let format = match mime {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        _ => {
            // Let the decoder guess if we don't recognize the exact mime.
            return image::load_from_memory(bytes)
                .ok()
                .and_then(|img| encode_thumbnail_png(img));
        }
    };

    let decoded = image::load_from_memory_with_format(bytes, format)
        .or_else(|_| image::load_from_memory(bytes))
        .ok()?;

    encode_thumbnail_png(decoded)
}

fn encode_thumbnail_png(decoded: image::DynamicImage) -> Option<Vec<u8>> {
    let thumb = decoded.thumbnail(THUMBNAIL_SIZE_PX, THUMBNAIL_SIZE_PX);
    let mut out = Vec::new();
    let mut cursor = Cursor::new(&mut out);
    thumb.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
    Some(out)
}

pub fn write_clipboard_text(text: &str) -> bool {
    let opts = CopyOptions::new();
    match opts.copy(
        Source::Bytes(text.as_bytes().to_vec().into()),
        CopyMimeType::Autodetect,
    ) {
        Ok(()) => true,
        Err(err) => {
            if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
                eprintln!("[clippy-land] clipboard write error: {err:?}");
            }
            false
        }
    }
}

pub fn write_clipboard_image(mime: &str, bytes: &[u8]) -> bool {
    let opts = CopyOptions::new();
    match opts.copy(
        Source::Bytes(bytes.to_vec().into_boxed_slice()),
        CopyMimeType::Specific(mime.to_string()),
    ) {
        Ok(()) => true,
        Err(err) => {
            if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
                eprintln!("[clippy-land] clipboard image write error: {err:?}");
            }
            false
        }
    }
}
