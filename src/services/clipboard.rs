mod image;
mod io;
mod model;
mod uri;
mod watcher;

#[cfg(test)]
mod tests;

pub use model::{ClipboardEntry, ClipboardFingerprint, ClipboardThumbnail};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

const DEFAULT_MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const THUMBNAIL_SIZE_PX: u32 = 400;
const MAX_FULL_FRAME_THUMBNAIL_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_MAX_IMAGE_DIMENSION_PX: u32 = 8192;

static MAX_IMAGE_BYTES: AtomicUsize = AtomicUsize::new(DEFAULT_MAX_IMAGE_BYTES);
static MAX_IMAGE_DIMENSION_PX: AtomicU32 = AtomicU32::new(DEFAULT_MAX_IMAGE_DIMENSION_PX);

pub fn configure_limits(max_image_bytes: usize, max_image_dimension_px: u32) {
    MAX_IMAGE_BYTES.store(max_image_bytes, Ordering::Relaxed);
    MAX_IMAGE_DIMENSION_PX.store(max_image_dimension_px, Ordering::Relaxed);
}

pub(super) fn max_image_bytes() -> usize {
    MAX_IMAGE_BYTES.load(Ordering::Relaxed)
}

pub(super) fn max_image_dimension_px() -> u32 {
    MAX_IMAGE_DIMENSION_PX.load(Ordering::Relaxed)
}

/// One-shot compatibility API for callers that do not need change notifications.
#[allow(dead_code)]
pub fn read_clipboard_entry() -> Option<ClipboardEntry> {
    io::read_clipboard_entry()
}

#[allow(dead_code)]
pub fn read_clipboard_text() -> Option<String> {
    io::read_clipboard_text()
}

#[allow(dead_code)]
pub fn read_clipboard_image() -> Option<ClipboardEntry> {
    io::read_clipboard_image()
}

pub fn write_clipboard_text(text: &str) -> bool {
    io::write_clipboard_text(text)
}

#[allow(dead_code)]
pub fn write_clipboard_image(mime: &str, bytes: &[u8]) -> bool {
    io::write_clipboard_image(mime, bytes)
}

pub fn write_owned_clipboard_image(mime: String, bytes: bytes::Bytes) -> bool {
    io::write_owned_clipboard_image(mime, bytes)
}

pub fn watch_clipboard(sender: tokio::sync::mpsc::Sender<ClipboardEntry>) {
    watcher::run(sender)
}

pub fn make_thumbnail(mime: &str, bytes: &bytes::Bytes) -> Option<ClipboardThumbnail> {
    image::make_thumbnail(mime, bytes)
}

pub(super) fn debug_log(message: impl std::fmt::Display) {
    if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
        eprintln!("[clippy-land] {message}");
    }
}
