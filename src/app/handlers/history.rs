use crate::app::model::HistoryItem;
use crate::services::clipboard::{self, ClipboardEntry};
use crate::settings::AppSettings;
use std::collections::VecDeque;

pub(super) fn pinned_count(history: &VecDeque<HistoryItem>) -> usize {
    history.iter().filter(|it| it.pinned).count()
}

pub(super) fn insert_after_pins(history: &mut VecDeque<HistoryItem>, item: HistoryItem) {
    let pos = history.iter().take_while(|it| it.pinned).count();
    history.insert(pos, item);
}

fn reorder_pins_first(history: &mut VecDeque<HistoryItem>) {
    let mut saw_unpinned = false;
    let already_ordered = history.iter().all(|item| {
        if item.pinned {
            !saw_unpinned
        } else {
            saw_unpinned = true;
            true
        }
    });
    if already_ordered {
        return;
    }

    // Move entries instead of cloning them.  An image entry owns the complete encoded image, so
    // cloning here would copy several megabytes on every clipboard update.
    let entries = std::mem::take(history);
    let (pinned, unpinned): (Vec<_>, Vec<_>) = entries.into_iter().partition(|item| item.pinned);

    history.extend(pinned);
    history.extend(unpinned);
}

pub(super) fn reconcile_limits(history: &mut VecDeque<HistoryItem>, settings: &AppSettings) {
    let max_history = settings.max_history.max(1);
    let max_pinned = settings.max_pinned.min(max_history);

    let mut pinned_seen = 0usize;
    for item in history.iter_mut() {
        if item.pinned {
            if pinned_seen < max_pinned {
                pinned_seen += 1;
            } else {
                item.pinned = false;
            }
        }
    }

    reorder_pins_first(history);

    while history.len() > max_history {
        if let Some(idx) = history.iter().rposition(|it| !it.pinned) {
            let _ = history.remove(idx);
        } else {
            let _ = history.pop_back();
        }
    }
}

pub(super) fn trim_history(history: &mut VecDeque<HistoryItem>, settings: &AppSettings) {
    reconcile_limits(history, settings);
}

pub(super) fn toggle_pin(
    history: &mut VecDeque<HistoryItem>,
    index: usize,
    settings: &AppSettings,
) -> bool {
    let Some(mut item) = history.remove(index) else {
        return false;
    };

    let max_pinned = settings.max_pinned.min(settings.max_history);

    if item.pinned {
        item.pinned = false;
        insert_after_pins(history, item);
    } else if pinned_count(history) >= max_pinned {
        history.insert(index, item);
        return false;
    } else {
        item.pinned = true;
        insert_after_pins(history, item);
    }

    reconcile_limits(history, settings);
    true
}

pub(super) fn copy_history_item(
    item: &HistoryItem,
) -> cosmic::iced::Task<cosmic::Action<crate::app::Message>> {
    copy_clipboard_entry(&item.entry)
}

pub(super) fn copy_clipboard_entry(
    entry: &ClipboardEntry,
) -> cosmic::iced::Task<cosmic::Action<crate::app::Message>> {
    use cosmic::prelude::*;

    match entry {
        ClipboardEntry::Text(text) => {
            _ = clipboard::write_clipboard_text(text);
            Task::none()
        }
        ClipboardEntry::Image { mime, bytes, .. } => {
            let mime = mime.clone();
            let bytes = bytes.clone();
            Task::perform(
                async move {
                    _ = tokio::task::spawn_blocking(move || {
                        clipboard::write_owned_clipboard_image(mime, bytes)
                    })
                    .await;
                },
                |_| cosmic::Action::None,
            )
        }
    }
}

pub(super) fn should_ignore_clipboard_entry(entry: &str) -> bool {
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
