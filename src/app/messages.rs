use crate::services::clipboard;
use cosmic::iced::widget::scrollable;
use cosmic::iced::window::Id;

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    /// Toggle popup triggered externally via the --toggle CLI flag.
    ToggleViaIpc,
    PopupClosed(Id),
    PopupOpened(Id),
    PopupRedraw(Id),
    /// Sent when a window loses focus, used to close the layer surface popup.
    WindowUnfocused(Id),
    ClipboardChanged(clipboard::ClipboardEntry),
    ThumbnailDecodeReady {
        hash: u64,
        bytes_len: usize,
        generation: u64,
    },
    ThumbnailReady {
        hash: u64,
        bytes_len: usize,
        thumbnail: Option<clipboard::ClipboardThumbnail>,
    },
    TogglePin(usize),
    OpenTextOverlay(usize),
    CloseTextOverlay,
    RemoveHistory(usize),
    ClearHistory,
    CopyFromHistory(usize),
    HoverEntry(Option<(usize, crate::app::model::FocusPart)>),
    HistoryScrolled(scrollable::Viewport),
    /// Search query changed — filters the visible history items.
    SearchChanged(String),
    ToggleSettingsPanel,
    SettingsMaxHistoryChanged(String),
    SettingsMaxPinnedChanged(String),
    SettingsMaxImageBytesChanged(String),
    SettingsMaxImageDimensionChanged(String),
    ApplySettings,
    /// Keyboard vertical navigation, resolved by current UI state.
    KeyboardNavigateUp,
    /// Keyboard vertical navigation, resolved by current UI state.
    KeyboardNavigateDown,
    /// Move sub-focus left (e.g., to actions)
    MoveFocusLeft,
    /// Move sub-focus right (e.g., to actions)
    MoveFocusRight,
    /// Activate the currently selected entry or focused control (Enter)
    ActivateSelection,
    /// Escape key behavior: close overlay first, then popup.
    EscapePressed,
}
