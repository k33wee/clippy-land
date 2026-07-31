use super::*;

#[test]
fn hover_entry_sets_hover_state_and_clears_keyboard_focus() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("item", false));
    app.keyboard_focus = Some((0, FocusPart::Entry));

    dispatch(&mut app, Message::HoverEntry(Some((0, FocusPart::Pin))));

    assert_eq!(app.hovered_index, Some(0));
    assert_eq!(app.hovered_focus, Some((0, FocusPart::Pin)));
    assert!(app.keyboard_focus.is_none());
}

#[test]
fn hover_entry_none_clears_hover_state() {
    let mut app = AppModel {
        hovered_index: Some(2),
        hovered_focus: Some((2, FocusPart::Remove)),
        ..Default::default()
    };

    dispatch(&mut app, Message::HoverEntry(None));

    assert!(app.hovered_index.is_none());
    assert!(app.hovered_focus.is_none());
}

#[test]
fn redundant_hover_entry_does_not_clear_keyboard_focus() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("item", false));
    app.hovered_index = Some(0);
    app.hovered_focus = Some((0, FocusPart::Entry));
    app.keyboard_focus = Some((0, FocusPart::Remove));

    dispatch(&mut app, Message::HoverEntry(Some((0, FocusPart::Entry))));

    assert_eq!(app.hovered_index, Some(0));
    assert_eq!(app.hovered_focus, Some((0, FocusPart::Entry)));
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Remove)));
}

#[test]
fn hover_entry_action_exit_can_fall_back_to_entry_without_clearing() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("item", false));

    dispatch(&mut app, Message::HoverEntry(Some((0, FocusPart::Pin))));
    assert_eq!(app.hovered_focus, Some((0, FocusPart::Pin)));

    dispatch(&mut app, Message::HoverEntry(Some((0, FocusPart::Entry))));
    assert_eq!(app.hovered_index, Some(0));
    assert_eq!(app.hovered_focus, Some((0, FocusPart::Entry)));
}
