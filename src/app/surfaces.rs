use crate::app::model::PopupSurface;
use crate::app::{AppModel, Message};
use cosmic::iced::window::Id;
use cosmic::prelude::*;
use cosmic::surface::action::{self, LiveSettings};

pub(in crate::app) fn open_layer_surface_popup(
    app: &mut AppModel,
) -> Task<cosmic::Action<Message>> {
    let new_id = Id::unique();
    app.popup.replace(new_id);
    app.popup_surface = Some(PopupSurface::LayerSurface);
    app.popup_controls_ready = false;
    app.note_popup_stage_marker("issuing libcosmic app_layer_shell request");

    surface_task(action::app_layer_shell::<AppModel>(
        default_live_settings,
        move |_| crate::app::history_layer_surface_settings(new_id),
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
