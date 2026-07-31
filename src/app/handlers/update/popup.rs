use super::shared::{schedule_missing_thumbnails, warm_thumbnail_handles};
use crate::app::{AppModel, Message};
use cosmic::prelude::*;
use std::time::Instant;

pub(super) fn handle(
    app: &mut AppModel,
    message: Message,
) -> Option<Task<cosmic::Action<Message>>> {
    match message {
        Message::TogglePopup => Some(toggle_popup(app)),
        Message::ToggleViaIpc => Some(toggle_via_ipc(app)),
        Message::PopupOpened(id) => {
            if app.popup.as_ref() == Some(&id) {
                app.popup_controls_ready = true;
                app.note_popup_stage_marker("popup controls ready after popup open");
                app.note_popup_opened();
                return Some(schedule_missing_thumbnails(app));
            }
            Some(Task::none())
        }
        Message::PopupRedraw(id) => {
            if app.popup.as_ref() == Some(&id) {
                app.note_popup_stage_marker("first popup redraw observed");
                app.finish_popup_open_trace_on_redraw();
            }
            Some(Task::none())
        }
        Message::WindowUnfocused(id) => Some(window_unfocused(app, id)),
        Message::PopupClosed(id) => {
            if app.popup.as_ref() == Some(&id) {
                clear_popup_state(app, "popup closed before first redraw");
            }
            Some(Task::none())
        }
        _ => None,
    }
}

pub(super) fn warm_for_first_popup(app: &mut AppModel) {
    warm_thumbnail_handles(app);
}

fn toggle_popup(app: &mut AppModel) -> Task<cosmic::Action<Message>> {
    if app.popup.is_some() {
        close_popup(app, "popup toggled closed before first view")
    } else {
        app.begin_popup_open_trace("icon-click");
        open_anchored_popup(app)
    }
}

fn toggle_via_ipc(app: &mut AppModel) -> Task<cosmic::Action<Message>> {
    if app.popup.is_some() {
        close_popup(app, "ipc toggle closed popup before first view")
    } else {
        app.begin_popup_open_trace("ipc-toggle");
        let warm_started = Instant::now();
        warm_thumbnail_handles(app);
        app.note_popup_stage_duration("warm_thumbnail_handles complete", warm_started.elapsed());
        open_layer_surface_popup(app)
    }
}

fn open_layer_surface_popup(app: &mut AppModel) -> Task<cosmic::Action<Message>> {
    crate::app::surfaces::open_layer_surface_popup(app)
}

fn open_anchored_popup(app: &mut AppModel) -> Task<cosmic::Action<Message>> {
    crate::app::surfaces::open_anchored_popup(app)
}

fn close_popup(app: &mut AppModel, reason: &'static str) -> Task<cosmic::Action<Message>> {
    let Some(id) = app.popup.take() else {
        return Task::none();
    };

    let surface = app.popup_surface.take();

    clear_popup_state(app, reason);

    crate::app::surfaces::destroy_popup_surface(id, surface)
}

fn clear_popup_state(app: &mut AppModel, reason: &'static str) {
    app.popup = None;
    app.popup_surface = None;
    app.popup_controls_ready = false;
    app.search_query.clear();
    app.settings_open = false;
    app.settings_error = None;
    app.hovered_index = None;
    app.at_scroll_bottom = false;
    app.history_viewport = None;
    app.text_overlay_index = None;
    app.cancel_popup_open_trace(reason);
}

fn window_unfocused(
    app: &mut AppModel,
    id: cosmic::iced::window::Id,
) -> Task<cosmic::Action<Message>> {
    if app.popup.as_ref() == Some(&id)
        && app.popup_surface == Some(crate::app::model::PopupSurface::LayerSurface)
    {
        close_popup(app, "window lost focus before first redraw")
    } else {
        Task::none()
    }
}
