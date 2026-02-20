use super::{AppModel, Message};
use super::model::HistoryItem;
use crate::services::clipboard;
use cosmic::iced::Subscription;
use cosmic::iced_winit::commands::popup::{destroy_popup, get_popup};
use cosmic::prelude::*;
use futures_util::SinkExt;
use std::collections::VecDeque;
use std::time::Duration;

const MAX_HISTORY: usize = 30;
const MAX_PINNED: usize = 5;

pub fn subscription(_app: &AppModel) -> Subscription<Message> {
    struct ClipboardSubscription;

    Subscription::batch(vec![Subscription::run_with_id(
        std::any::TypeId::of::<ClipboardSubscription>(),
        cosmic::iced::stream::channel(1, move |mut channel| async move {
            let mut last_seen: Option<clipboard::ClipboardFingerprint> = None;

            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;

                let next = tokio::task::spawn_blocking(clipboard::read_clipboard_entry)
                    .await
                    .ok()
                    .flatten();

                let Some(next) = next else {
                    continue;
                };

                let next_fp = next.fingerprint();
                if last_seen.as_ref() == Some(&next_fp) {
                    continue;
                }

                last_seen = Some(next_fp);

                if channel.send(Message::ClipboardChanged(next)).await.is_err() {
                    break;
                }
            }
        }),
    )])
}

fn pinned_count(history: &VecDeque<HistoryItem>) -> usize {
    history.iter().filter(|it| it.pinned).count()
}

fn insert_after_pins(history: &mut VecDeque<HistoryItem>, item: HistoryItem) {
    let pos = history.iter().take_while(|it| it.pinned).count();
    history.insert(pos, item);
}

fn trim_history(history: &mut VecDeque<HistoryItem>) {
    while history.len() > MAX_HISTORY {
        if let Some(idx) = history.iter().rposition(|it| !it.pinned) {
            let _ = history.remove(idx);
        } else {
            break;
        }
    }
}

pub fn update(app: &mut AppModel, message: Message) -> Task<cosmic::Action<Message>> {
    match message {
        Message::ClipboardChanged(entry) => {
            if app
                .history
                .front()
                .is_some_and(|it: &HistoryItem| &it.entry == &entry)
            {
                return Task::none();
            }

            if let clipboard::ClipboardEntry::Text(text) = &entry {
                if should_ignore_clipboard_entry(text) {
                    return Task::none();
                }
            }

            // Remove any existing entries that match to keep the history unique, but keep pin state.
            let pinned = app
                .history
                .iter()
                .position(|it| &it.entry == &entry)
                .and_then(|idx| app.history.remove(idx))
                .is_some_and(|it| it.pinned);

            insert_after_pins(
                &mut app.history,
                HistoryItem {
                    entry,
                    pinned,
                },
            );
            trim_history(&mut app.history);
        }
        Message::TogglePin(index) => {
            let Some(mut item) = app.history.remove(index) else {
                return Task::none();
            };

            if item.pinned {
                item.pinned = false;
                insert_after_pins(&mut app.history, item);
            } else if pinned_count(&app.history) >= MAX_PINNED {
                // Pin limit reached; keep the item where it was.
                app.history.insert(index, item);
            } else {
                item.pinned = true;
                insert_after_pins(&mut app.history, item);
            }
        }
        Message::CopyFromHistory(index) => {
            if let Some(item) = app.history.get(index) {
                match &item.entry {
                    clipboard::ClipboardEntry::Text(text) => {
                        _ = clipboard::write_clipboard_text(text);
                    }
                    clipboard::ClipboardEntry::Image { mime, bytes, .. } => {
                        _ = clipboard::write_clipboard_image(mime, bytes);
                    }
                }
            }
        }
        Message::RemoveHistory(index) => {
            let _ = app.history.remove(index);
        }
        Message::TogglePopup => {
            return if let Some(p) = app.popup.take() {
                destroy_popup(p)
            } else {
                let new_id = cosmic::iced::window::Id::unique();
                app.popup.replace(new_id);
                let popup_settings = app.core.applet.get_popup_settings(
                    app.core.main_window_id().unwrap(),
                    new_id,
                    None,
                    None,
                    None,
                );
                get_popup(popup_settings)
            };
        }
        Message::PopupClosed(id) => {
            if app.popup.as_ref() == Some(&id) {
                app.popup = None;
            }
        }
    }
    Task::none()
}

fn should_ignore_clipboard_entry(entry: &str) -> bool {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return true;
    }

    if trimmed.chars().all(|c| {
        c.is_ascii_digit() || matches!(c, ',' | '.' | ':' | ';' | '/' | '\\' | '_' | '-' | ' ')
    }) && trimmed.chars().count() <= 8
    {
        return true;
    }

    false
}
