mod clipboard;
mod history_limits;
mod hover;
mod ipc;
mod navigation;
mod popup;
mod scrolling;
mod search;
mod settings;

pub(super) use crate::app::model::{FocusPart, HistoryItem};
pub(super) use crate::app::view;
pub(super) use crate::app::{AppModel, Message};
pub(super) use crate::services::clipboard::ClipboardEntry;
pub(super) use crate::settings::AppSettings;

use super::update;

pub(super) fn dispatch(app: &mut AppModel, message: Message) {
    let _ = update(app, message);
}

pub(super) fn prewarm_for_first_popup(app: &mut AppModel) {
    super::prewarm_for_first_popup(app);
}

pub(super) fn text_entry(text: &str) -> ClipboardEntry {
    ClipboardEntry::Text(text.to_string())
}

pub(super) fn text_item(text: &str, pinned: bool) -> HistoryItem {
    HistoryItem {
        entry: text_entry(text),
        pinned,
    }
}

pub(super) fn image_entry(hash: u64) -> ClipboardEntry {
    ClipboardEntry::Image {
        mime: "image/png".to_string(),
        bytes: vec![1, 2, 3, 4].into(),
        hash,
        thumbnail_png: Some(vec![137, 80, 78, 71].into()),
    }
}

pub(super) fn item_text(item: &HistoryItem) -> &str {
    match &item.entry {
        ClipboardEntry::Text(text) => text,
        ClipboardEntry::Image { .. } => {
            panic!("expected text entry in handler tests")
        }
    }
}

pub(super) fn test_settings(max_history: usize, max_pinned: usize) -> AppSettings {
    AppSettings {
        max_history,
        max_pinned,
        ..AppSettings::default()
    }
    .normalized()
}

pub(super) use super::subscription::{message_for_latin_key, message_for_named_key};
pub(super) use super::{history, scroll};
