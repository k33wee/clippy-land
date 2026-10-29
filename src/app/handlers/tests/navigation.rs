use super::*;
use crate::app::model::PopupSurface;
use cosmic::iced::core::keyboard::key::Named as NamedKey;

#[test]
fn keyboard_navigation_down_steps_through_filtered_indices() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("apple", false));
    app.history.push_back(text_item("banana", false));
    app.history.push_back(text_item("apricot", false));
    app.search_query = "ap".into();
    app.recompute_filtered_indices();

    dispatch(&mut app, Message::KeyboardNavigateDown);
    assert_eq!(app.hovered_index, Some(0));

    dispatch(&mut app, Message::KeyboardNavigateDown);
    assert_eq!(app.hovered_index, Some(2));

    dispatch(&mut app, Message::KeyboardNavigateDown);
    assert_eq!(app.hovered_index, Some(0));
}

#[test]
fn keyboard_navigation_up_wraps_to_last_filtered_index() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("apple", false));
    app.history.push_back(text_item("banana", false));
    app.history.push_back(text_item("apricot", false));
    app.search_query = "ap".into();
    app.recompute_filtered_indices();

    dispatch(&mut app, Message::KeyboardNavigateUp);
    assert_eq!(app.hovered_index, Some(2));

    dispatch(&mut app, Message::KeyboardNavigateUp);
    assert_eq!(app.hovered_index, Some(0));
}

#[test]
fn keyboard_navigation_does_nothing_when_filtered_list_is_empty() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("apple", false));
    app.search_query = "zzz".into();
    app.recompute_filtered_indices();

    dispatch(&mut app, Message::KeyboardNavigateDown);
    assert!(app.hovered_index.is_none());

    dispatch(&mut app, Message::KeyboardNavigateUp);
    assert!(app.hovered_index.is_none());
}

#[test]
fn close_text_overlay_message_clears_overlay_index() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("overlay text", false));
    app.text_overlay_index = Some(0);

    dispatch(&mut app, Message::ClosePreview);

    assert!(app.text_overlay_index.is_none());
}

#[test]
fn close_text_overlay_message_closes_overlay_without_closing_popup() {
    let mut app = AppModel::default();
    let popup_id = cosmic::iced::window::Id::unique();
    app.popup = Some(popup_id);
    app.history
        .push_back(text_item("first line\nsecond line", false));
    app.text_overlay_index = Some(0);

    dispatch(&mut app, Message::ClosePreview);

    assert_eq!(app.popup, Some(popup_id));
    assert!(app.text_overlay_index.is_none());
}

#[test]
fn close_text_overlay_message_is_noop_when_overlay_not_open() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("overlay text", false));

    dispatch(&mut app, Message::ClosePreview);

    assert!(app.text_overlay_index.is_none());
}

#[test]
fn vertical_navigation_keys_emit_keyboard_navigation_intents() {
    assert!(matches!(
        message_for_named_key(NamedKey::ArrowDown),
        Some(Message::KeyboardNavigateDown)
    ));
    assert!(matches!(
        message_for_named_key(NamedKey::ArrowUp),
        Some(Message::KeyboardNavigateUp)
    ));
    assert!(matches!(
        message_for_latin_key('j'),
        Some(Message::KeyboardNavigateDown)
    ));
    assert!(matches!(
        message_for_latin_key('k'),
        Some(Message::KeyboardNavigateUp)
    ));
}

#[test]
fn uppercase_vim_navigation_keys_emit_keyboard_navigation_intents() {
    assert!(matches!(
        message_for_latin_key('J'),
        Some(Message::KeyboardNavigateDown)
    ));
    assert!(matches!(
        message_for_latin_key('K'),
        Some(Message::KeyboardNavigateUp)
    ));
}

#[test]
fn overlay_closed_keyboard_navigation_moves_history_selection() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("first", false));
    app.history.push_back(text_item("second", false));
    app.recompute_filtered_indices();

    dispatch(&mut app, Message::KeyboardNavigateDown);
    assert_eq!(app.hovered_index, Some(0));
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Entry)));

    dispatch(&mut app, Message::KeyboardNavigateUp);
    assert_eq!(app.hovered_index, Some(1));
    assert_eq!(app.keyboard_focus, Some((1, FocusPart::Entry)));
}

#[test]
fn overlay_open_keyboard_navigation_scrolls_without_moving_history_focus() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("first", false));
    app.history.push_back(text_item("second", false));
    app.text_overlay_index = Some(0);
    app.hovered_index = Some(0);
    app.keyboard_focus = Some((0, FocusPart::Preview));

    dispatch(&mut app, Message::KeyboardNavigateDown);
    assert_eq!(app.hovered_index, Some(0));
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Preview)));

    dispatch(&mut app, Message::KeyboardNavigateUp);
    assert_eq!(app.hovered_index, Some(0));
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Preview)));
}

#[test]
fn escape_pressed_closes_overlay_without_closing_popup() {
    let mut app = AppModel::default();
    let popup_id = cosmic::iced::window::Id::unique();
    app.popup = Some(popup_id);
    app.history
        .push_back(text_item("first line\nsecond line", false));
    app.text_overlay_index = Some(0);

    dispatch(&mut app, Message::EscapePressed);

    assert_eq!(app.popup, Some(popup_id));
    assert!(app.text_overlay_index.is_none());
}

#[test]
fn escape_pressed_closes_popup_when_overlay_not_open() {
    let mut app = AppModel::default();
    let popup_id = cosmic::iced::window::Id::unique();
    app.popup = Some(popup_id);
    app.popup_surface = Some(PopupSurface::AnchoredPopup);

    dispatch(&mut app, Message::EscapePressed);

    assert!(app.popup.is_none());
    assert!(app.popup_surface.is_none());
}

#[test]
fn move_focus_right_cycles_entry_pin_remove() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("item", false));
    app.hovered_index = Some(0);
    app.keyboard_focus = Some((0, FocusPart::Entry));

    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Pin)));

    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Remove)));

    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Entry)));
}

#[test]
fn move_focus_right_cycles_entry_preview_pin_remove_when_preview_available() {
    let mut app = AppModel::default();
    app.history
        .push_back(text_item("first line\nsecond line", false));
    app.hovered_index = Some(0);
    app.keyboard_focus = Some((0, FocusPart::Entry));

    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Preview)));

    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Pin)));

    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Remove)));

    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Entry)));
}

#[test]
fn move_focus_left_cycles_entry_remove_pin() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("item", false));
    app.hovered_index = Some(0);
    app.keyboard_focus = Some((0, FocusPart::Entry));

    dispatch(&mut app, Message::MoveFocusLeft);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Remove)));

    dispatch(&mut app, Message::MoveFocusLeft);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Pin)));

    dispatch(&mut app, Message::MoveFocusLeft);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Entry)));
}

#[test]
fn move_focus_left_cycles_entry_remove_pin_preview_when_preview_available() {
    let mut app = AppModel::default();
    app.history
        .push_back(text_item("first line\nsecond line", false));
    app.hovered_index = Some(0);
    app.keyboard_focus = Some((0, FocusPart::Entry));

    dispatch(&mut app, Message::MoveFocusLeft);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Remove)));

    dispatch(&mut app, Message::MoveFocusLeft);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Pin)));

    dispatch(&mut app, Message::MoveFocusLeft);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Preview)));

    dispatch(&mut app, Message::MoveFocusLeft);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Entry)));
}

#[test]
fn activate_selection_opens_text_overlay_when_preview_is_focused() {
    let mut app = AppModel::default();
    app.history
        .push_back(text_item("first line\nsecond line", false));
    app.keyboard_focus = Some((0, FocusPart::Preview));

    dispatch(&mut app, Message::ActivateSelection);

    assert_eq!(app.text_overlay_index, Some(0));
}

#[test]
fn move_focus_without_hover_initialises_to_entry() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("item", false));
    app.hovered_index = Some(0);

    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Entry)));
}
