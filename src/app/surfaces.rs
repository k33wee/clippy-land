use crate::app::model::PopupSurface;
use crate::app::{AppModel, Message};
use crate::services::toplevel::{Snapshot, WindowRect};
use cosmic::iced::window::Id;
use cosmic::prelude::*;
use cosmic::surface::action::{self, LiveSettings};

/// Preferred height of the toggle popup layer surface.
pub(in crate::app) const TOGGLE_POPUP_HEIGHT: u32 = 400;
const NEAR_WINDOW_MIN_WIDTH: i32 = 360;
const NEAR_WINDOW_MAX_WIDTH: i32 = 560;

/// Placement captured before the toggle popup can change keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum TogglePopupPlacement {
    /// Output-local popup bounds and the margin from its right output edge.
    NearFocusedWindow { rect: WindowRect, right_margin: i32 },
    /// Previous top-strip placement, used without focus/output geometry.
    ActiveMonitorStrip,
}

impl TogglePopupPlacement {
    pub(in crate::app) fn content_width(self) -> u32 {
        match self {
            Self::NearFocusedWindow { rect, .. } => rect.width as u32,
            Self::ActiveMonitorStrip => NEAR_WINDOW_MIN_WIDTH as u32,
        }
    }
}

/// Center on the focused window, constrained to its output's logical bounds.
/// Missing toplevel or xdg-output support retains the active-monitor fallback.
pub(in crate::app) fn plan_toggle_popup(snapshot: Option<Snapshot>) -> TogglePopupPlacement {
    let Some(Snapshot {
        focused_window: Some(window),
        focused_output: Some(output),
    }) = snapshot
    else {
        return TogglePopupPlacement::ActiveMonitorStrip;
    };
    if window.width <= 0 || window.height <= 0 || output.width <= 0 || output.height <= 0 {
        return TogglePopupPlacement::ActiveMonitorStrip;
    }

    let width = window
        .width
        .clamp(NEAR_WINDOW_MIN_WIDTH, NEAR_WINDOW_MAX_WIDTH)
        .min(output.width);
    let height = (TOGGLE_POPUP_HEIGHT as i32).min(output.height);
    let left = (i64::from(window.x) + i64::from(window.width) / 2 - i64::from(width) / 2)
        .clamp(0, i64::from(output.width - width)) as i32;
    let top = (i64::from(window.y) + i64::from(window.height) / 2 - i64::from(height) / 2)
        .clamp(0, i64::from(output.height - height)) as i32;

    TogglePopupPlacement::NearFocusedWindow {
        rect: WindowRect {
            x: left,
            y: top,
            width,
            height,
        },
        right_margin: output.width - left - width,
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

    let placement = plan_toggle_popup(app.toplevel_tracker.snapshot());
    app.toggle_popup_placement = Some(placement);

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
    app.toggle_popup_placement = None;
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
    use crate::services::toplevel::OutputSize;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> WindowRect {
        WindowRect {
            x,
            y,
            width,
            height,
        }
    }

    fn snapshot(window: WindowRect, width: i32, height: i32) -> Option<Snapshot> {
        Some(Snapshot {
            focused_window: Some(window),
            focused_output: Some(OutputSize { width, height }),
        })
    }

    fn bounds(placement: TogglePopupPlacement) -> WindowRect {
        let TogglePopupPlacement::NearFocusedWindow { rect, .. } = placement else {
            panic!("expected focused placement");
        };
        rect
    }

    #[test]
    fn plan_without_focus_or_output_uses_active_monitor_strip() {
        assert_eq!(
            plan_toggle_popup(None),
            TogglePopupPlacement::ActiveMonitorStrip
        );
        assert_eq!(
            plan_toggle_popup(Some(Snapshot::default())),
            TogglePopupPlacement::ActiveMonitorStrip
        );
        assert_eq!(
            plan_toggle_popup(Some(Snapshot {
                focused_window: Some(rect(10, 10, 800, 600)),
                focused_output: None,
            })),
            TogglePopupPlacement::ActiveMonitorStrip
        );
    }

    #[test]
    fn plan_with_degenerate_geometry_uses_active_monitor_strip() {
        for invalid in [rect(10, 10, 0, 600), rect(10, 10, 800, -5)] {
            assert_eq!(
                plan_toggle_popup(snapshot(invalid, 1920, 1080)),
                TogglePopupPlacement::ActiveMonitorStrip
            );
        }
        for (width, height) in [(0, 1080), (1920, -1)] {
            assert_eq!(
                plan_toggle_popup(snapshot(rect(0, 0, 800, 600), width, height)),
                TogglePopupPlacement::ActiveMonitorStrip
            );
        }
    }

    #[test]
    fn plan_centers_popup_on_focused_window() {
        assert_eq!(
            bounds(plan_toggle_popup(snapshot(
                rect(400, 200, 800, 600),
                1920,
                1080
            ))),
            rect(520, 300, 560, 400)
        );
    }

    #[test]
    fn plan_clamps_left_and_top_to_output_edges() {
        assert_eq!(
            bounds(plan_toggle_popup(snapshot(
                rect(0, 0, 200, 300),
                1920,
                1080
            ))),
            rect(0, 0, 360, 400)
        );
    }

    #[test]
    fn plan_clamps_right_and_bottom_to_output_edges() {
        assert_eq!(
            bounds(plan_toggle_popup(snapshot(
                rect(1800, 1000, 120, 80),
                1920,
                1080
            ))),
            rect(1560, 680, 360, 400)
        );
    }

    #[test]
    fn plan_fits_outputs_smaller_than_preferred_popup() {
        assert_eq!(
            bounds(plan_toggle_popup(snapshot(
                rect(40, 40, 200, 200),
                240,
                300
            ))),
            rect(0, 0, 240, 300)
        );
    }

    #[test]
    fn plan_widens_narrow_windows_to_minimum() {
        assert_eq!(
            bounds(plan_toggle_popup(snapshot(
                rect(40, 300, 200, 500),
                1920,
                1080
            ))),
            rect(0, 350, 360, 400)
        );
    }

    #[test]
    fn plan_caps_wide_windows_to_maximum_width() {
        assert_eq!(
            bounds(plan_toggle_popup(snapshot(
                rect(0, 0, 4000, 2000),
                4000,
                2000
            ))),
            rect(1720, 800, 560, 400)
        );
    }

    #[test]
    fn plan_uses_window_width_between_bounds() {
        assert_eq!(
            bounds(plan_toggle_popup(snapshot(
                rect(60, 80, 450, 500),
                1920,
                1080
            ))),
            rect(60, 130, 450, 400)
        );
    }

    #[test]
    fn extreme_window_geometry_does_not_overflow() {
        assert_eq!(
            bounds(plan_toggle_popup(snapshot(
                rect(i32::MAX, i32::MAX, i32::MAX, i32::MAX),
                1920,
                1080
            ))),
            rect(1360, 680, 560, 400)
        );
    }

    #[test]
    fn bottom_right_focus_sets_margins_without_requesting_an_exact_width() {
        let placement = plan_toggle_popup(snapshot(rect(1662, 558, 1082, 510), 2752, 1152));
        assert_eq!(bounds(placement), rect(1923, 613, 560, 400));
        assert_eq!(placement.content_width(), 560);
        let settings = crate::app::history_layer_surface_settings(Id::unique(), placement);
        use cosmic::iced::platform_specific::shell::wayland::commands::layer_surface::Anchor;
        assert_eq!(settings.anchor, Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
        // A compositor-chosen width forces the panel proxy's first configure.
        assert_eq!(settings.size, Some((None, Some(400))));
        assert_eq!(settings.margin.top, 613);
        assert_eq!(settings.margin.left, 1923);
        assert_eq!(settings.margin.right, 269);
        assert_eq!(settings.exclusive_zone, -1);
        assert!(matches!(
            settings.output,
            cosmic::iced::runtime::platform_specific::wayland::layer_surface::IcedOutput::Active
        ));
    }

    #[test]
    fn fallback_keeps_top_strip_on_active_output() {
        let placement = plan_toggle_popup(None);
        assert_eq!(placement.content_width(), 360);
        let settings = crate::app::history_layer_surface_settings(Id::unique(), placement);
        assert_eq!(settings.size, Some((None, Some(400))));
        assert_eq!(settings.margin.top, 0);
        assert_eq!(settings.margin.left, 0);
        assert_eq!(settings.margin.right, 0);
        assert_eq!(settings.exclusive_zone, 0);
    }
}
