use super::shared::{
    cache_thumbnail_handle, decode_thumbnail, prune_thumbnail_handles, schedule_missing_thumbnails,
    schedule_missing_thumbnails_after_clipboard_change,
};
use crate::app::model::HistoryItem;
use crate::app::{AppModel, Message, pinned_history};
use crate::services::clipboard::ClipboardEntry;
use cosmic::iced::widget::image::Handle as ImageHandle;
use cosmic::prelude::*;

use super::super::history;

pub(super) fn handle(
    app: &mut AppModel,
    message: Message,
) -> Option<Task<cosmic::Action<Message>>> {
    match message {
        Message::ClipboardChanged(entry) => {
            if app
                .history
                .front()
                .is_some_and(|it: &HistoryItem| it.entry == entry)
            {
                return Some(Task::none());
            }

            if let ClipboardEntry::Text(text) = &entry
                && history::should_ignore_clipboard_entry(text)
            {
                return Some(Task::none());
            }

            cache_thumbnail_handle(app, &entry);

            let pinned = app
                .history
                .iter()
                .position(|it| it.entry == entry)
                .and_then(|idx| app.history.remove(idx))
                .is_some_and(|it| it.pinned);

            history::insert_after_pins(&mut app.history, HistoryItem { entry, pinned });
            history::trim_history(&mut app.history, &app.settings);
            prune_thumbnail_handles(app);
            app.text_overlay_index = None;
            app.recompute_filtered_indices();
            if pinned {
                pinned_history::save(&app.history);
            }

            if app.popup.is_some() {
                Some(schedule_missing_thumbnails_after_clipboard_change(app))
            } else {
                Some(Task::none())
            }
        }
        Message::ThumbnailReady {
            hash,
            bytes_len,
            thumbnail,
        } => {
            let key = (hash, bytes_len);
            app.pending_thumbnails.remove(&key);

            let Some(thumbnail) = thumbnail else {
                app.failed_thumbnails.insert(key);
                return Some(if app.popup.is_some() {
                    schedule_missing_thumbnails(app)
                } else {
                    Task::none()
                });
            };
            let Some(item) = app.history.iter_mut().find(|item| match &item.entry {
                ClipboardEntry::Image { bytes, hash, .. } => (*hash, bytes.len()) == key,
                ClipboardEntry::Text(_) => false,
            }) else {
                return Some(schedule_missing_thumbnails(app));
            };

            if let ClipboardEntry::Image { thumbnail_png, .. } = &mut item.entry {
                *thumbnail_png = Some(thumbnail.png);
            }
            app.thumbnail_handles.insert(
                key,
                ImageHandle::from_rgba(thumbnail.width, thumbnail.height, thumbnail.rgba),
            );
            Some(if app.popup.is_some() {
                schedule_missing_thumbnails(app)
            } else {
                Task::none()
            })
        }
        Message::ThumbnailDecodeReady {
            hash,
            bytes_len,
            generation,
        } => {
            let key = (hash, bytes_len);
            if generation != app.thumbnail_schedule_generation {
                return Some(Task::none());
            }
            if app.popup.is_none() || !app.popup_controls_ready {
                app.pending_thumbnails.remove(&key);
                return Some(Task::none());
            }

            let Some((mime, bytes)) = app.history.iter().find_map(|item| match &item.entry {
                ClipboardEntry::Image {
                    mime,
                    bytes,
                    hash,
                    thumbnail_png: None,
                } if (*hash, bytes.len()) == key => Some((mime.clone(), bytes.clone())),
                _ => None,
            }) else {
                app.pending_thumbnails.remove(&key);
                return Some(schedule_missing_thumbnails(app));
            };

            Some(decode_thumbnail(mime, bytes, key))
        }
        Message::TogglePin(index) => {
            if history::toggle_pin(&mut app.history, index, &app.settings) {
                pinned_history::save(&app.history);
            }
            app.text_overlay_index = None;
            app.recompute_filtered_indices();
            Some(Task::none())
        }
        Message::OpenTextOverlay(index) => {
            if app
                .history
                .get(index)
                .is_some_and(|item| matches!(item.entry, ClipboardEntry::Text(_)))
            {
                app.text_overlay_index = Some(index);
            }
            Some(Task::none())
        }
        Message::CloseTextOverlay => {
            app.text_overlay_index = None;
            Some(Task::none())
        }
        Message::CopyFromHistory(index) => {
            if let Some(item) = app.history.get(index) {
                return Some(history::copy_history_item(item));
            }
            Some(Task::none())
        }
        Message::RemoveHistory(index) => {
            let removed_pinned = app.history.remove(index).is_some_and(|item| item.pinned);
            prune_thumbnail_handles(app);
            app.text_overlay_index = None;
            app.recompute_filtered_indices();
            if removed_pinned {
                pinned_history::save(&app.history);
            }
            Some(Task::none())
        }
        Message::ClearHistory => {
            app.history.retain(|item| item.pinned);
            prune_thumbnail_handles(app);
            app.text_overlay_index = None;
            app.recompute_filtered_indices();
            pinned_history::save(&app.history);
            Some(Task::none())
        }
        Message::SearchChanged(query) => {
            app.search_query = query;
            app.recompute_filtered_indices();
            app.text_overlay_index = None;
            app.hovered_index = None;
            app.hovered_focus = None;
            app.keyboard_focus = None;
            Some(Task::none())
        }
        _ => None,
    }
}
