#![allow(dead_code)]

use super::image::{
    clipboard_entry_from_image_bytes, clipboard_entry_from_image_path, log_image_too_large,
};
use super::uri::parse_first_local_path_from_uri_list;
use super::{ClipboardEntry, debug_log, max_image_bytes};
use std::io::Read;
use wl_clipboard_rs::{
    copy::{MimeType as CopyMimeType, Options as CopyOptions, Source},
    paste::{ClipboardType, MimeType as PasteMimeType, Seat, get_contents},
};

pub fn read_clipboard_entry() -> Option<ClipboardEntry> {
    super::read_clipboard_image()
        .or_else(read_clipboard_image_from_uri_list)
        .or_else(|| super::read_clipboard_text().map(ClipboardEntry::Text))
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
            debug_log(format!("clipboard read get_contents error: {err:?}"));
            return None;
        }
    };

    let mut bytes = Vec::new();
    if let Err(err) = pipe.read_to_end(&mut bytes) {
        debug_log(format!("clipboard read pipe error: {err:?}"));
        return None;
    }

    let text = match String::from_utf8(bytes) {
        Ok(ok) => ok,
        Err(err) => {
            debug_log(format!("clipboard read utf8 error: {err:?}"));
            return None;
        }
    };

    let text = text.trim_end_matches(['\n', '\r']).to_string();
    (!text.is_empty()).then_some(text)
}

pub fn read_clipboard_image() -> Option<ClipboardEntry> {
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
        let max_image_bytes = max_image_bytes();
        let mut limited = pipe.take((max_image_bytes + 1) as u64);
        if limited.read_to_end(&mut bytes).is_err() {
            continue;
        }
        if bytes.len() > max_image_bytes {
            log_image_too_large(bytes.len());
            continue;
        }

        if let Some(entry) = clipboard_entry_from_image_bytes(actual_mime, bytes) {
            return Some(entry);
        }
    }

    None
}

pub fn write_clipboard_text(text: &str) -> bool {
    let opts = CopyOptions::new();
    match opts.copy(
        Source::Bytes(text.as_bytes().to_vec().into()),
        CopyMimeType::Autodetect,
    ) {
        Ok(()) => true,
        Err(err) => {
            debug_log(format!("clipboard write error: {err:?}"));
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
            debug_log(format!("clipboard image write error: {err:?}"));
            false
        }
    }
}

pub fn write_owned_clipboard_image(mime: String, bytes: bytes::Bytes) -> bool {
    let opts = CopyOptions::new();
    let bytes: Vec<u8> = bytes.into();
    match opts.copy(
        Source::Bytes(bytes.into_boxed_slice()),
        CopyMimeType::Specific(mime),
    ) {
        Ok(()) => true,
        Err(err) => {
            debug_log(format!("clipboard image write error: {err:?}"));
            false
        }
    }
}

fn read_clipboard_image_from_uri_list() -> Option<ClipboardEntry> {
    let result = get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        PasteMimeType::Specific("text/uri-list"),
    );

    let (mut pipe, _) = result.ok()?;
    let mut bytes = Vec::new();
    if pipe.read_to_end(&mut bytes).is_err() {
        return None;
    }

    let uris = String::from_utf8(bytes).ok()?;
    let path = parse_first_local_path_from_uri_list(&uris)?;
    clipboard_entry_from_image_path(&path)
}
