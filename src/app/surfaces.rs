use crate::app::model::PopupSurface;
use crate::app::{AppModel, Message};
use crate::services::toplevel::WindowRect;
use cosmic::iced::window::Id;
use cosmic::prelude::*;
use cosmic::surface::action::{self, LiveSettings};

/// Fixed height of the toggle popup layer surface.
pub(in crate::app) const TOGGLE_POPUP_HEIGHT: u32 = 400;

/// Preferred minimum and maximum width of the toggle popup when it is placed
/// next to the focused window.
const NEAR_WINDOW_MIN_WIDTH: i32 = 360;
const NEAR_WINDOW_MAX_WIDTH: i32 = 560;

/// Where the IPC-toggle popup layer surface should appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum TogglePopupPlacement {
    /// A panel of the given width centered on the focused window, expressed
    /// as margins from the top-left corner of the active output.
    NearFocusedWindow { left: i32, top: i32, width: u32 },
    /// Full-width strip below the panel of the active output (the previous
    /// behavior, also used when no focused window is known).
    ActiveMonitorStrip,
}

/// Chooses the toggle popup placement from the latest focused-window rect.
///
/// With no focused window the popup keeps the full-width top strip, but the
/// layer surface still targets the compositor's active output, so the popup
/// follows the monitor the user is working on even when the desktop is empty.
pub(in crate::app) fn plan_toggle_popup(focused: Option<WindowRect>) -> TogglePopupPlacement {
    let Some(rect) = focused else {
        return TogglePopupPlacement::ActiveMonitorStrip;
    };
    if rect.width <= 0 || rect.height <= 0 {
        return TogglePopupPlacement::ActiveMonitorStrip;
    }

    let width = rect
        .width
        .clamp(NEAR_WINDOW_MIN_WIDTH, NEAR_WINDOW_MAX_WIDTH);
    let left = (rect.x + rect.width / 2 - width / 2).max(0);
    let top = (rect.y + rect.height / 2 - TOGGLE_POPUP_HEIGHT as i32 / 2).max(0);

    TogglePopupPlacement::NearFocusedWindow {
        left,
        top,
        width: width as u32,
    }
}

pub(in crate::app) fn open_layer_surface_popup(
    app: &mut AppModel,
) -> Task<cosmic::Action<Message>> {
    let new_id = Id::unique();
    app.popup.replace(new_id);
    app.popup_surface = Some(PopupSurface::LayerSurface);
    app.popup_controls_ready = false;
    app.note_popup_stage_marker("issuing libcosmic app_layer_shell request");

    let focused_window = app
        .toplevel_tracker
        .snapshot()
        .and_then(|snapshot| snapshot.focused_window);
    let placement = plan_toggle_popup(focused_window);

    surface_task(action::app_layer_shell::<AppModel>(
        default_live_settings,
        move |_| crate::app::history_layer_surface_settings(new_id, placement),
        None,
    ))
}

pub(in crate::app) fn open_anchored_popup(app: &mut AppModel) -> Task<cosmic::Action<Message>> {
    let new_id = Id::unique();
    app.popup.replace(new_id);
    app.popup_surface = Some(PopupSurface::AnchoredPopup);
    app.popup_controls_ready = false;
    app.note_popup_stage_marker("issuing libcosmic app_popup request");

    surface_task(action::app_popup::<AppModel>(
        default_live_settings,
        move |app| {
            let parent = app.core.main_window_id().unwrap_or(Id::RESERVED);
            app.core
                .applet
                .get_popup_settings(parent, new_id, None, None, None)
        },
        None,
    ))
}

pub(in crate::app) fn destroy_popup_surface(
    id: Id,
    surface: Option<PopupSurface>,
) -> Task<cosmic::Action<Message>> {
    let action = match surface {
        Some(PopupSurface::AnchoredPopup) => action::destroy_popup(id),
        Some(PopupSurface::LayerSurface) | None => action::destroy_layer_shell(id),
    };

    surface_task(action)
}

fn default_live_settings(_: &AppModel) -> LiveSettings {
    LiveSettings::default()
}

fn surface_task(action: cosmic::surface::Action<Message>) -> Task<cosmic::Action<Message>> {
    cosmic::surface::surface_task(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> WindowRect {
        WindowRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn plan_without_focus_uses_active_monitor_strip() {
        assert_eq!(
            plan_toggle_popup(None),
            TogglePopupPlacement::ActiveMonitorStrip
        );
    }

    #[test]
    fn plan_with_degenerate_rect_uses_active_monitor_strip() {
        assert_eq!(
            plan_toggle_popup(Some(rect(10, 10, 0, 600))),
            TogglePopupPlacement::ActiveMonitorStrip
        );
        assert_eq!(
            plan_toggle_popup(Some(rect(10, 10, 800, -5))),
            TogglePopupPlacement::ActiveMonitorStrip
        );
    }

    #[test]
    fn plan_centers_popup_on_focused_window() {
        assert_eq!(
            plan_toggle_popup(Some(rect(400, 200, 800, 600))),
            TogglePopupPlacement::NearFocusedWindow {
                left: 400 + 800 / 2 - 560 / 2,
                top: 200 + 600 / 2 - 200,
                width: 560,
            }
        );
    }

    #[test]
    fn plan_clamps_left_margin_to_output_edge() {
        assert_eq!(
            plan_toggle_popup(Some(rect(0, 100, 400, 400))),
            TogglePopupPlacement::NearFocusedWindow {
                left: 0,
                top: 100,
                width: 400,
            }
        );
    }

    #[test]
    fn plan_clamps_top_margin_to_output_edge() {
        // A 600px window is capped to the maximum popup width of 560, so the
        // popup is centered on the window and clamped to the top edge.
        assert_eq!(
            plan_toggle_popup(Some(rect(100, 0, 600, 300))),
            TogglePopupPlacement::NearFocusedWindow {
                left: 100 + 300 - 280,
                top: 0,
                width: 560,
            }
        );
    }

    #[test]
    fn plan_widens_narrow_windows_to_minimum() {
        // A 200px window is narrower than the preferred popup; the popup uses
        // the minimum width and is clamped to stay inside the output.
        assert_eq!(
            plan_toggle_popup(Some(rect(40, 300, 200, 500))),
            TogglePopupPlacement::NearFocusedWindow {
                left: 0,
                top: 300 + 250 - 200,
                width: 360,
            }
        );
    }

    #[test]
    fn plan_caps_wide_windows_to_maximum_width() {
        assert_eq!(
            plan_toggle_popup(Some(rect(0, 0, 4000, 2000))),
            TogglePopupPlacement::NearFocusedWindow {
                left: 2000 - 280,
                top: 1000 - 200,
                width: 560,
            }
        );
    }

    #[test]
    fn plan_uses_window_width_between_bounds() {
        assert_eq!(
            plan_toggle_popup(Some(rect(60, 80, 450, 500))),
            TogglePopupPlacement::NearFocusedWindow {
                left: 60 + 225 - 225,
                top: 80 + 250 - 200,
                width: 450,
            }
        );
    }
}
