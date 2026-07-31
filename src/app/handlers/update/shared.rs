use crate::app::AppModel;
use crate::app::view::summary::text_overlay_available;
use crate::services::clipboard::ClipboardEntry;
use cosmic::iced::widget::image::Handle as ImageHandle;
use cosmic::prelude::*;

const THUMBNAIL_DECODE_DELAY: tokio::time::Duration = tokio::time::Duration::from_millis(350);

pub(super) fn cache_thumbnail_handle(app: &mut AppModel, entry: &ClipboardEntry) {
    let ClipboardEntry::Image {
        bytes,
        hash,
        thumbnail_png: Some(thumbnail_png),
        ..
    } = entry
    else {
        return;
    };

    app.thumbnail_handles
        .entry((*hash, bytes.len()))
        .or_insert_with(|| ImageHandle::from_bytes(thumbnail_png.clone()));
}

pub(super) fn prune_thumbnail_handles(app: &mut AppModel) {
    app.thumbnail_handles.retain(|key, _| {
        app.history.iter().any(|item| match &item.entry {
            ClipboardEntry::Image { bytes, hash, .. } => key == &(*hash, bytes.len()),
            ClipboardEntry::Text(_) => false,
        })
    });
    app.failed_thumbnails.retain(|key| {
        app.history.iter().any(|item| match &item.entry {
            ClipboardEntry::Image { bytes, hash, .. } => key == &(*hash, bytes.len()),
            ClipboardEntry::Text(_) => false,
        })
    });
}

pub(super) fn warm_thumbnail_handles(app: &mut AppModel) {
    for item in app.history.iter() {
        let ClipboardEntry::Image {
            bytes,
            hash,
            thumbnail_png: Some(thumbnail_png),
            ..
        } = &item.entry
        else {
            continue;
        };

        app.thumbnail_handles
            .entry((*hash, bytes.len()))
            .or_insert_with(|| ImageHandle::from_bytes(thumbnail_png.clone()));
    }
}

pub(super) fn schedule_missing_thumbnails(
    app: &mut AppModel,
) -> Task<cosmic::Action<crate::app::Message>> {
    schedule_missing_thumbnails_after(app, tokio::time::Duration::ZERO)
}

pub(super) fn schedule_missing_thumbnails_after_clipboard_change(
    app: &mut AppModel,
) -> Task<cosmic::Action<crate::app::Message>> {
    app.thumbnail_schedule_generation = app.thumbnail_schedule_generation.wrapping_add(1);
    app.pending_thumbnails.clear();
    schedule_missing_thumbnails_after(app, THUMBNAIL_DECODE_DELAY)
}

fn schedule_missing_thumbnails_after(
    app: &mut AppModel,
    delay: tokio::time::Duration,
) -> Task<cosmic::Action<crate::app::Message>> {
    if app.popup.is_none() || !app.popup_controls_ready {
        return Task::none();
    }
    if !app.pending_thumbnails.is_empty() {
        return Task::none();
    }

    for item in &app.history {
        let ClipboardEntry::Image {
            bytes,
            hash,
            thumbnail_png,
            ..
        } = &item.entry
        else {
            continue;
        };

        let key = (*hash, bytes.len());
        if thumbnail_png.is_some()
            || app.thumbnail_handles.contains_key(&key)
            || app.failed_thumbnails.contains(&key)
        {
            continue;
        }
        app.pending_thumbnails.insert(key);
        let generation = app.thumbnail_schedule_generation;
        return Task::perform(
            async move {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            },
            move |_| {
                cosmic::Action::App(crate::app::Message::ThumbnailDecodeReady {
                    hash: key.0,
                    bytes_len: key.1,
                    generation,
                })
            },
        );
    }

    Task::none()
}

pub(super) fn decode_thumbnail(
    mime: String,
    bytes: bytes::Bytes,
    key: (u64, usize),
) -> Task<cosmic::Action<crate::app::Message>> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                crate::services::clipboard::make_thumbnail(&mime, &bytes)
            })
            .await
            .unwrap_or(None)
        },
        move |thumbnail| {
            cosmic::Action::App(crate::app::Message::ThumbnailReady {
                hash: key.0,
                bytes_len: key.1,
                thumbnail,
            })
        },
    )
}

pub(super) fn row_has_preview(app: &AppModel, idx: usize) -> bool {
    app.history
        .get(idx)
        .and_then(|item| match &item.entry {
            ClipboardEntry::Text(text) => Some(text_overlay_available(text)),
            ClipboardEntry::Image { .. } => None,
        })
        .unwrap_or(false)
}

pub(super) fn parse_usize_field(input: &str) -> Result<usize, &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("value is required");
    }
    trimmed
        .parse::<usize>()
        .map_err(|_| "must be a valid positive integer")
}

pub(super) fn parse_u32_field(input: &str) -> Result<u32, &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("value is required");
    }
    trimmed
        .parse::<u32>()
        .map_err(|_| "must be a valid positive integer")
}
