use super::image::clipboard_entry_from_image_path;
use super::uri::parse_first_local_path_from_uri_list;
use super::*;
use ::image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn image_entry(
    mime: &str,
    bytes: Vec<u8>,
    hash: u64,
    thumbnail_png: Option<Vec<u8>>,
) -> ClipboardEntry {
    ClipboardEntry::Image {
        mime: mime.to_string(),
        bytes: bytes.into(),
        hash,
        thumbnail_png: thumbnail_png.map(Into::into),
    }
}

#[test]
fn text_fingerprint_matches_text_content() {
    let entry = ClipboardEntry::Text("hello".to_string());
    assert_eq!(
        entry.fingerprint(),
        ClipboardFingerprint::Text("hello".to_string())
    );
}

#[test]
fn image_fingerprint_tracks_mime_hash_and_length() {
    let entry = image_entry("image/png", vec![1, 2, 3, 4], 99, None);
    assert_eq!(
        entry.fingerprint(),
        ClipboardFingerprint::Image {
            mime: "image/png".to_string(),
            bytes_len: 4,
            hash: 99,
        }
    );
}

#[test]
fn image_equality_uses_hash_mime_and_length() {
    let a = image_entry("image/png", vec![1, 2, 3], 7, None);
    let b = image_entry("image/png", vec![9, 8, 7], 7, Some(vec![0, 1, 2]));
    let c = image_entry("image/png", vec![9, 8], 7, None);
    let d = image_entry("image/png", vec![9, 8, 7], 8, None);

    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, d);
}

#[test]
fn parses_first_local_file_uri() {
    let list = "# comment\n\nfile:///tmp/hello%20world.png\nfile:///tmp/second.jpg\n";
    let parsed = parse_first_local_path_from_uri_list(list).expect("first path should parse");
    assert_eq!(parsed, PathBuf::from("/tmp/hello world.png"));
}

#[test]
fn creates_image_entry_from_local_file_path() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("clippy-land-test-{unique}.png"));

    let img = RgbaImage::from_pixel(2, 2, Rgba([0, 255, 0, 255]));
    DynamicImage::ImageRgba8(img)
        .save_with_format(&path, ImageFormat::Png)
        .expect("test image should save");

    let entry = clipboard_entry_from_image_path(&path).expect("image entry should be created");
    match entry {
        ClipboardEntry::Image { mime, bytes, .. } => {
            assert_eq!(mime, "image/png");
            assert!(!bytes.is_empty());
        }
        ClipboardEntry::Text(_) => panic!("expected image"),
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn image_entry_defers_thumbnail_decoding() {
    let img = RgbaImage::from_pixel(8, 4, Rgba([0, 255, 0, 255]));
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("test image encoding should work");

    let entry = super::image::clipboard_entry_from_image_bytes("image/png".to_string(), png)
        .expect("image entry should be created");

    match entry {
        ClipboardEntry::Image { thumbnail_png, .. } => assert!(thumbnail_png.is_none()),
        ClipboardEntry::Text(_) => panic!("expected image entry"),
    }
}

#[test]
fn builds_rgba_and_png_thumbnail_on_demand() {
    let img = RgbaImage::from_pixel(8, 4, Rgba([0, 255, 0, 255]));
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("test image encoding should work");

    let thumbnail = super::make_thumbnail("image/png", &png.into())
        .expect("valid image should produce a thumbnail");

    assert_eq!((thumbnail.width, thumbnail.height), (8, 4));
    assert_eq!(thumbnail.rgba.len(), 8 * 4 * 4);
    assert!(thumbnail.png.starts_with(&[137, 80, 78, 71]));
}

fn encode_large_test_png() -> Vec<u8> {
    let img = RgbaImage::from_fn(1024, 768, |x, y| {
        let mixed = x
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(y.wrapping_mul(0x85EB_CA6B));
        Rgba([
            mixed as u8,
            mixed.rotate_left(11) as u8,
            mixed.rotate_left(21) as u8,
            255,
        ])
    });
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("test image encoding should work");
    png
}

#[test]
fn streams_large_png_directly_into_bounded_thumbnail_storage() {
    let png = encode_large_test_png();

    let thumbnail = super::make_thumbnail("image/png", &png.into())
        .expect("valid large PNG should produce a thumbnail");

    assert_eq!((thumbnail.width, thumbnail.height), (400, 300));
    assert_eq!(thumbnail.rgba.len(), 400 * 300 * 4);
}

#[test]
#[ignore = "manual benchmark for the 2-4 MiB clipboard-image workload"]
fn benchmark_large_png_thumbnail_decode() {
    let png = encode_large_test_png();
    assert!(
        (2 * 1024 * 1024..=4 * 1024 * 1024).contains(&png.len()),
        "benchmark PNG is {} bytes",
        png.len()
    );

    let started = std::time::Instant::now();
    let thumbnail = super::make_thumbnail("image/png", &png.into())
        .expect("benchmark PNG should produce a thumbnail");
    eprintln!(
        "decoded 2-4 MiB PNG into {}x{} thumbnail in {:?}",
        thumbnail.width,
        thumbnail.height,
        started.elapsed()
    );
}

#[test]
fn rejects_thumbnail_with_oversized_dimensions() {
    let huge = RgbaImage::from_pixel(9000, 1, Rgba([255, 0, 0, 255]));
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(huge)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("huge test image encoding should work");

    assert!(super::make_thumbnail("image/png", &png.into()).is_none());
}

#[test]
fn rejects_malformed_image_bytes_for_thumbnail() {
    assert!(super::make_thumbnail("image/png", &vec![1, 2, 3, 4, 5, 6, 7].into()).is_none());
}

fn wayland_tests_enabled() -> bool {
    std::env::var("CLIPPY_LAND_RUN_WAYLAND_TESTS")
        .map(|v| v == "1")
        .unwrap_or(false)
        && std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("XDG_RUNTIME_DIR").is_some()
}

fn read_text_with_retry(expected: &str, retries: usize) -> bool {
    for _ in 0..retries {
        if read_clipboard_text().as_deref() == Some(expected) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn read_image_with_retry(retries: usize) -> Option<ClipboardEntry> {
    for _ in 0..retries {
        if let Some(entry) = read_clipboard_image() {
            return Some(entry);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

#[test]
#[ignore = "requires CLIPPY_LAND_RUN_WAYLAND_TESTS=1 and a live Wayland session"]
fn wayland_clipboard_text_round_trip() {
    if !wayland_tests_enabled() {
        return;
    }

    let text = "clippy-land-wayland-text";
    assert!(write_clipboard_text(text));
    assert!(read_text_with_retry(text, 20));
}

#[test]
#[ignore = "requires CLIPPY_LAND_RUN_WAYLAND_TESTS=1 and a live Wayland session"]
fn wayland_clipboard_image_round_trip() {
    if !wayland_tests_enabled() {
        return;
    }

    let img = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("test image encoding should work");

    assert!(write_clipboard_image("image/png", &png));

    let read = read_image_with_retry(20).expect("clipboard image should be readable");
    match read {
        ClipboardEntry::Image { mime, bytes, .. } => {
            assert_eq!(mime, "image/png");
            assert_eq!(bytes, png);
        }
        ClipboardEntry::Text(_) => panic!("expected image entry"),
    }
}
