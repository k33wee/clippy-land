mod handlers;
mod icons;
mod messages;
mod model;
mod pinned_history;
mod surfaces;
mod view;

pub use messages::Message;
pub use model::AppModel;
#[derive(Debug, Clone, Copy, Default)]
pub struct AppFlags {
    pub open_popup_on_start: bool,
}

use crate::app::model::SettingsDraft;
use crate::services::clipboard;
use crate::settings::AppSettings;
use cosmic::iced::platform_specific::shell::wayland::commands::layer_surface::{
    self, KeyboardInteractivity,
};
use cosmic::iced::runtime::platform_specific::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::{Limits, Subscription, window::Id};
use cosmic::prelude::*;
use std::time::{Duration, Instant};

pub(super) fn history_scroll_id() -> cosmic::iced::core::widget::Id {
    cosmic::iced::core::widget::Id::new("history-scroll")
}

pub(super) fn text_overlay_scroll_id() -> cosmic::iced::core::widget::Id {
    cosmic::iced::core::widget::Id::new("text-overlay-scroll")
}

pub(in crate::app) fn history_layer_surface_settings(
    id: Id,
    placement: surfaces::TogglePopupPlacement,
) -> SctkLayerSurfaceSettings {
    let (margin, height, exclusive_zone) = match placement {
        // Opposing horizontal anchors plus margins produce the desired small
        // surface while still letting the compositor choose its width.
        surfaces::TogglePopupPlacement::NearFocusedWindow { rect, right_margin } => (
            IcedMargin {
                top: rect.y,
                left: rect.x,
                right: right_margin,
                bottom: 0,
            },
            rect.height as u32,
            // Geometry is relative to the output, not the reserved work area.
            -1,
        ),
        surfaces::TogglePopupPlacement::ActiveMonitorStrip => {
            (IcedMargin::default(), surfaces::TOGGLE_POPUP_HEIGHT, 0)
        }
    };
    SctkLayerSurfaceSettings {
        id,
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
        // COSMIC Panel's layer proxy omits the initial configure when an
        // exact fixed size is requested. An unspecified width ensures the
        // compositor chooses a size and the proxy forwards that configure.
        anchor: layer_surface::Anchor::TOP
            | layer_surface::Anchor::LEFT
            | layer_surface::Anchor::RIGHT,
        namespace: "clippy-land".into(),
        margin,
        exclusive_zone,
        size: Some((None, Some(height))),
        size_limits: Limits::NONE.min_width(1.0).min_height(1.0),
        ..Default::default()
    }
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;

    type Flags = AppFlags;

    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format
    const APP_ID: &'static str = "io.github.k33wee.clippy-land";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let init_started = Instant::now();

        let stage_started = Instant::now();
        let settings = AppSettings::load().normalized();
        init_timing_log(
            "settings loaded and normalized",
            init_started,
            stage_started.elapsed(),
        );

        let stage_started = Instant::now();
        clipboard::configure_limits(settings.max_image_bytes, settings.max_image_dimension_px);
        init_timing_log(
            "runtime image limits configured",
            init_started,
            stage_started.elapsed(),
        );

        let mut core = core;
        // Disable automatic theme radii for layer surfaces: COSMIC Panel can
        // reject the radius before a newly opened layer surface is fully
        // configured, disconnecting the applet before it can show the popup.
        core.set_auto_corner_radius(cosmic::core::Auto::Window | cosmic::core::Auto::Popup);

        let mut app = AppModel {
            core,
            settings_draft: SettingsDraft::from_settings(&settings),
            settings,
            toplevel_tracker: crate::services::toplevel::Tracker::started(),
            ..Default::default()
        };

        let stage_started = Instant::now();
        app.history = pinned_history::load(&app.settings);
        init_timing_log(
            "pinned history loaded",
            init_started,
            stage_started.elapsed(),
        );

        let stage_started = Instant::now();
        icons::prewarm_popup_icons();
        init_timing_log(
            "popup icons prewarmed",
            init_started,
            stage_started.elapsed(),
        );

        let stage_started = Instant::now();
        handlers::prewarm_for_first_popup(&mut app);
        init_timing_log(
            "popup thumbnail handles prewarmed",
            init_started,
            stage_started.elapsed(),
        );

        let stage_started = Instant::now();
        app.recompute_filtered_indices();
        init_timing_log(
            "filtered indices recomputed",
            init_started,
            stage_started.elapsed(),
        );

        if flags.open_popup_on_start {
            app.begin_popup_open_trace("startup");
            let task = surfaces::open_layer_surface_popup(&mut app);

            (app, task)
        } else {
            (app, Task::none())
        }
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// Describes the interface based on the current state of the application model
    fn view(&self) -> Element<'_, Self::Message> {
        view::view(self)
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        view::view_window(self, _id)
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        handlers::subscription(self)
    }

    /// Handles messages emitted by the application and its widgets
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        handlers::update(self, message)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

fn init_timing_log(label: &'static str, init_started: Instant, stage_elapsed: Duration) {
    if std::env::var_os("CLIPPY_LAND_DEBUG_TIMING").is_some() {
        eprintln!(
            "[clippy-land timing] init stage: {label} at {:.2}ms (stage={:.2}ms)",
            duration_ms(init_started.elapsed()),
            duration_ms(stage_elapsed)
        );
    }
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
