mod filtering;
mod overlay;
mod row_state;
mod summary;

pub(super) use super::popup::{filtered_indices, selected_text_overlay};
pub(super) use super::row::{RowContent, RowRenderState};
pub(super) use super::summary::{summarize_one_line, summarize_one_line_with_limit};
pub(super) use crate::app::AppModel;
pub(super) use crate::app::model::HistoryItem;
pub(super) use crate::services::clipboard::ClipboardEntry;
pub(super) use cosmic::iced::widget::image::Handle as ImageHandle;

pub(super) fn push_text(app: &mut AppModel, text: &str) {
    app.history.push_back(HistoryItem {
        entry: ClipboardEntry::Text(text.to_string()),
        pinned: false,
    });
}

pub(super) fn push_image(app: &mut AppModel, mime: &str) {
    app.history.push_back(HistoryItem {
        entry: ClipboardEntry::Image {
            mime: mime.to_string(),
            bytes: vec![].into(),
            hash: 0,
            thumbnail_png: None,
        },
        pinned: false,
    });
}
