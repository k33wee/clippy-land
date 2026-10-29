use crate::services::clipboard;
use crate::settings::AppSettings;
use cosmic::iced::widget::image::Handle as ImageHandle;
use cosmic::iced::window::Id;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

use super::timing::PopupOpenTrace;

#[derive(Debug, Clone)]
pub(in crate::app) struct HistoryItem {
    pub(in crate::app) entry: clipboard::ClipboardEntry,
    pub(in crate::app) pinned: bool,
}

/// The application model stores app-specific state used to describe its interface
#[derive(Default)]
pub struct AppModel {
    pub(in crate::app) core: cosmic::Core,
    pub(in crate::app) settings: AppSettings,
    pub(in crate::app) popup: Option<Id>,
    pub(in crate::app) popup_surface: Option<PopupSurface>,
    /// Copy/paste work held until the backend confirms this popup was destroyed.
    pub(in crate::app) pending_paste:
        Option<(Id, cosmic::iced::Task<cosmic::Action<crate::app::Message>>)>,
    /// Placement captured when toggled, so subsequent focus changes cannot move it.
    pub(in crate::app) toggle_popup_placement: Option<crate::app::surfaces::TogglePopupPlacement>,
    /// False during initial popup mapping so non-essential footer controls can be revealed
    /// only after the popup has actually opened.
    pub(in crate::app) popup_controls_ready: bool,
    /// Current search query for filtering clipboard history.
    pub(in crate::app) search_query: String,
    /// Whether settings panel is visible inside popup.
    pub(in crate::app) settings_open: bool,
    pub(in crate::app) about_open: bool,
    /// Draft settings form values (text inputs).
    pub(in crate::app) settings_draft: SettingsDraft,
    /// Last settings save/validation error shown in UI.
    pub(in crate::app) settings_error: Option<String>,
    /// Latest clipboard entries, newest-first (with pinned items kept at the top).
    pub(in crate::app) history: VecDeque<HistoryItem>,
    /// Cached filtered indices for the current query.
    pub(in crate::app) filtered_indices: Vec<usize>,
    /// Query value used when `filtered_indices` was last computed.
    pub(in crate::app) filtered_query_cache: String,
    /// History length used when `filtered_indices` was last computed.
    pub(in crate::app) filtered_history_len_cache: usize,
    /// Cached decoded image handles for thumbnails, keyed by (content hash, byte length).
    pub(in crate::app) thumbnail_handles: HashMap<(u64, usize), ImageHandle>,
    /// Original header dimensions, including failed lookups, cached once per image.
    pub(in crate::app) image_dimensions: HashMap<(u64, usize), Option<(u32, u32)>>,
    /// Image identities waiting for or currently undergoing popup-preview decoding.
    pub(in crate::app) pending_thumbnails: HashSet<(u64, usize)>,
    /// Monotonic generation used to debounce thumbnail work after clipboard updates.
    pub(in crate::app) thumbnail_schedule_generation: u64,
    /// Images rejected by the decoder, so reopening the popup does not retry them forever.
    pub(in crate::app) failed_thumbnails: HashSet<(u64, usize)>,
    /// Index of the history entry the mouse is currently hovering over.
    pub(in crate::app) hovered_index: Option<usize>,
    /// The specific part of a row the mouse is hovering over (index, part)
    pub(in crate::app) hovered_focus: Option<(usize, FocusPart)>,
    pub(in crate::app) at_scroll_bottom: bool,
    /// Last observed history scroll viewport, used to keep keyboard selection in view.
    pub(in crate::app) history_viewport: Option<cosmic::iced::widget::scrollable::Viewport>,
    /// Keyboard focus within the history: (index, part) where part is Entry/Pin/Remove
    pub(in crate::app) keyboard_focus: Option<(usize, FocusPart)>,
    /// Explicit text preview overlay target, if open.
    pub(in crate::app) text_overlay_index: Option<usize>,
    /// Only the active image gets a larger decoded preview.
    pub(in crate::app) image_preview: Option<ImagePreview>,
    pub(in crate::app) image_preview_generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// One worker at a time; newer requests replace pending state instead of queueing buffers.
    pub(in crate::app) image_preview_in_flight: Option<u64>,
    /// Pending timing trace for popup open diagnostics.
    pub(in crate::app) popup_open_trace: RefCell<Option<PopupOpenTrace>>,
    /// Background focused-toplevel tracker used to place the toggle popup.
    pub(in crate::app) toplevel_tracker: crate::services::toplevel::Tracker,
}

pub(in crate::app) struct ImagePreview {
    pub key: (u64, usize),
    pub generation: u64,
    pub handle: Option<ImageHandle>,
    pub failed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SettingsDraft {
    pub max_history: String,
    pub max_pinned: String,
    pub max_image_mib: String,
    pub max_image_dimension_px: String,
}

impl SettingsDraft {
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            max_history: settings.max_history.to_string(),
            max_pinned: settings.max_pinned.to_string(),
            max_image_mib: (settings.max_image_bytes as f64 / 1_048_576.0).to_string(),
            max_image_dimension_px: settings.max_image_dimension_px.to_string(),
        }
    }
}

/// MiB is a UI unit only; existing byte-valued configuration stays unchanged.
pub(in crate::app) fn parse_image_mib(input: &str) -> Result<usize, ImageMiBError> {
    let mib = input
        .trim()
        .parse::<f64>()
        .map_err(|_| ImageMiBError::NotANumber)?;
    if !mib.is_finite() || !(0.25..=64.0).contains(&mib) {
        return Err(ImageMiBError::OutOfRange);
    }
    Ok((mib * 1_048_576.0).round() as usize)
}

/// Parse failures for the MiB field; the UI maps these to Fluent messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ImageMiBError {
    NotANumber,
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusPart {
    Entry,
    Preview,
    Pin,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum PopupSurface {
    AnchoredPopup,
    LayerSurface,
}

impl AppModel {
    pub(in crate::app) fn close_preview(&mut self) {
        self.image_preview_generation
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.text_overlay_index = None;
        self.image_preview = None;
    }

    pub(in crate::app) fn preview_is_open(&self) -> bool {
        self.text_overlay_index.is_some() || self.image_preview.is_some()
    }

    fn filtered_indices_cache_is_valid(&self) -> bool {
        self.filtered_query_cache == self.search_query
            && self.filtered_history_len_cache == self.history.len()
            && self
                .filtered_indices
                .iter()
                .all(|&idx| idx < self.history.len())
    }

    pub(in crate::app) fn current_filtered_len(&self) -> usize {
        if self.filtered_indices_cache_is_valid() {
            self.filtered_indices.len()
        } else {
            Self::compute_filtered_indices_for(&self.history, &self.search_query).len()
        }
    }

    pub(in crate::app) fn compute_filtered_indices_for(
        history: &VecDeque<HistoryItem>,
        search_query: &str,
    ) -> Vec<usize> {
        let query = search_query.to_lowercase();
        if query.is_empty() {
            return (0..history.len()).collect();
        }

        history
            .iter()
            .enumerate()
            .filter(|(_, item)| match &item.entry {
                clipboard::ClipboardEntry::Text(text) => text.to_lowercase().contains(&query),
                clipboard::ClipboardEntry::Image { mime, .. } => {
                    mime.to_lowercase().contains(&query)
                }
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    pub(in crate::app) fn recompute_filtered_indices(&mut self) {
        self.filtered_indices =
            Self::compute_filtered_indices_for(&self.history, &self.search_query);
        self.filtered_query_cache = self.search_query.clone();
        self.filtered_history_len_cache = self.history.len();
    }
}
