use crate::services::clipboard;
use cosmic::iced::window::Id;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub(super) struct HistoryItem {
    pub(super) entry: clipboard::ClipboardEntry,
    pub(super) pinned: bool,
}

/// The application model stores app-specific state used to describe its interface
#[derive(Default)]
pub struct AppModel {
    pub(super) core: cosmic::Core,
    pub(super) popup: Option<Id>,
    /// Latest clipboard entries, newest-first (with pinned items kept at the top).
    pub(super) history: VecDeque<HistoryItem>,
}
