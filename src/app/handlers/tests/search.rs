use super::*;

#[test]
fn search_changed_updates_query_and_clears_hover_and_keyboard() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("hello", false));
    app.hovered_index = Some(0);
    app.hovered_focus = Some((0, FocusPart::Entry));
    app.keyboard_focus = Some((0, FocusPart::Pin));

    dispatch(&mut app, Message::SearchChanged("he".into()));

    assert_eq!(app.search_query, "he");
    assert!(app.hovered_index.is_none());
    assert!(app.hovered_focus.is_none());
    assert!(app.keyboard_focus.is_none());
}

#[test]
fn search_changed_empty_string_clears_query() {
    let mut app = AppModel {
        search_query: "old".into(),
        ..Default::default()
    };

    dispatch(&mut app, Message::SearchChanged(String::new()));

    assert!(app.search_query.is_empty());
}

#[test]
fn search_changed_recomputes_filtered_indices_cache() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("apple", false));
    app.history.push_back(text_item("banana", false));
    app.recompute_filtered_indices();
    assert_eq!(app.filtered_indices, vec![0, 1]);

    dispatch(&mut app, Message::SearchChanged("ap".into()));
    assert_eq!(app.filtered_indices, vec![0]);
}

#[test]
fn search_changed_closes_text_overlay() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("overlay text", false));
    app.text_overlay_index = Some(0);

    dispatch(&mut app, Message::SearchChanged("over".into()));

    assert!(app.text_overlay_index.is_none());
}
