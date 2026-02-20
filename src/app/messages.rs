use crate::services::clipboard;
use cosmic::iced::window::Id;

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    ClipboardChanged(clipboard::ClipboardEntry),
    TogglePin(usize),
    RemoveHistory(usize),
    CopyFromHistory(usize),
}
