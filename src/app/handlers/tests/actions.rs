use super::*;
use crate::app::model::PopupSurface;

fn app_with_open_popup() -> (AppModel, cosmic::iced::window::Id) {
    let id = cosmic::iced::window::Id::unique();
    let mut app = AppModel {
        popup: Some(id),
        popup_surface: Some(PopupSurface::LayerSurface),
        ..Default::default()
    };
    app.history.push_back(HistoryItem {
        entry: image_entry(1),
        pinned: false,
    });
    app.recompute_filtered_indices();
    (app, id)
}

#[test]
fn clicking_entry_closes_popup_when_enabled() {
    let (mut app, _) = app_with_open_popup();
    app.settings.close_popup_on_action = true;

    dispatch(&mut app, Message::CopyFromHistory(0));

    assert!(app.popup.is_none());
    assert!(app.popup_surface.is_none());
}

#[test]
fn clicking_entry_keeps_popup_open_when_disabled() {
    let (mut app, id) = app_with_open_popup();
    app.settings.close_popup_on_action = false;

    dispatch(&mut app, Message::CopyFromHistory(0));

    assert_eq!(app.popup, Some(id));
    assert_eq!(app.popup_surface, Some(PopupSurface::LayerSurface));
}

#[test]
fn enter_on_entry_closes_popup_when_enabled() {
    let (mut app, _) = app_with_open_popup();
    app.settings.close_popup_on_action = true;
    app.keyboard_focus = Some((0, FocusPart::Entry));

    dispatch(&mut app, Message::ActivateSelection);

    assert!(app.popup.is_none());
    assert!(app.popup_surface.is_none());
}

#[test]
fn enter_on_hovered_entry_closes_popup() {
    let (mut app, _) = app_with_open_popup();
    app.settings.close_popup_on_action = true;
    app.hovered_index = Some(0);
    app.keyboard_focus = None;

    dispatch(&mut app, Message::ActivateSelection);

    assert!(app.popup.is_none());
}

#[test]
fn enter_on_entry_control_does_not_close_popup() {
    let (mut app, id) = app_with_open_popup();
    app.settings.close_popup_on_action = true;
    app.keyboard_focus = Some((0, FocusPart::Pin));

    dispatch(&mut app, Message::ActivateSelection);

    assert_eq!(app.popup, Some(id));
    assert!(app.history[0].pinned);
}
