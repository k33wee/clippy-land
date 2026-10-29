use crate::app::model::ImagePreview;
use crate::app::view::summary::text_overlay_available;
use crate::app::{AppModel, Message};
use crate::services::clipboard::{self, ClipboardEntry};
use cosmic::iced::widget::image::Handle;
use cosmic::prelude::*;
use std::sync::atomic::Ordering;

pub(super) fn open(app: &mut AppModel, index: usize) -> Task<cosmic::Action<Message>> {
    if app.settings_open || app.about_open {
        return Task::none();
    }
    let Some(item) = app.history.get(index) else {
        return Task::none();
    };
    match &item.entry {
        ClipboardEntry::Text(text) => {
            if text_overlay_available(text) {
                app.close_preview();
                app.text_overlay_index = Some(index);
            }
            Task::none()
        }
        ClipboardEntry::Image { bytes, hash, .. } => {
            let key = (*hash, bytes.len());
            if app
                .image_preview
                .as_ref()
                .is_some_and(|preview| preview.key == key)
            {
                return Task::none();
            }
            app.text_overlay_index = None;
            let generation = app
                .image_preview_generation
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_add(1);
            app.image_preview = Some(ImagePreview {
                key,
                generation,
                handle: None,
                failed: false,
            });
            start_pending(app)
        }
    }
}

/// Only one task owns encoded image bytes. Switching while it runs replaces the
/// desired preview; its completion starts just the latest request, never a queue.
fn start_pending(app: &mut AppModel) -> Task<cosmic::Action<Message>> {
    if app.image_preview_in_flight.is_some() {
        return Task::none();
    }
    let Some(active) = &app.image_preview else {
        return Task::none();
    };
    if active.handle.is_some() || active.failed {
        return Task::none();
    }
    let (key, generation) = (active.key, active.generation);
    let Some((mime, bytes)) = app.history.iter().find_map(|item| match &item.entry {
        ClipboardEntry::Image {
            mime, bytes, hash, ..
        } if (*hash, bytes.len()) == key => Some((mime.clone(), bytes.clone())),
        _ => None,
    }) else {
        app.close_preview();
        return Task::none();
    };
    app.image_preview_in_flight = Some(generation);
    let request = app.image_preview_generation.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                clipboard::make_preview(&mime, &bytes, || {
                    request.load(Ordering::Relaxed) != generation
                })
            })
            .await
            .unwrap_or(None)
        },
        move |preview| {
            cosmic::Action::App(Message::ImagePreviewReady {
                key,
                generation,
                preview,
            })
        },
    )
}

pub(super) fn handle(
    app: &mut AppModel,
    message: Message,
) -> Option<Task<cosmic::Action<Message>>> {
    match message {
        Message::OpenPreview(index) => Some(open(app, index)),
        Message::ClosePreview => {
            app.close_preview();
            Some(Task::none())
        }
        Message::ImagePreviewReady {
            key,
            generation,
            preview,
        } => {
            if app.image_preview_in_flight != Some(generation) {
                return Some(Task::none());
            }
            app.image_preview_in_flight = None;
            if let Some(active) = &mut app.image_preview
                && active.key == key
                && active.generation == generation
            {
                active.failed = preview.is_none();
                active.handle =
                    preview.map(|image| Handle::from_rgba(image.width, image.height, image.rgba));
            }
            Some(start_pending(app))
        }
        _ => None,
    }
}
