use super::*;

#[test]
fn toggling_pinned_item_moves_it_after_pinned_section() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("a", true));
    app.history.push_back(text_item("b", true));
    app.history.push_back(text_item("c", false));

    dispatch(&mut app, Message::TogglePin(0));

    assert!(app.history[0].pinned);
    assert_eq!(item_text(&app.history[0]), "b");
    assert!(!app.history[1].pinned);
    assert_eq!(item_text(&app.history[1]), "a");
}

#[test]
fn toggle_pin_respects_max_pinned_limit() {
    let mut app = AppModel {
        settings: test_settings(30, 5),
        ..Default::default()
    };

    for i in 0..app.settings.max_pinned {
        app.history.push_back(text_item(&format!("pin-{i}"), true));
    }
    app.history.push_back(text_item("unpinned", false));

    let max_pinned = app.settings.max_pinned;
    dispatch(&mut app, Message::TogglePin(max_pinned));

    assert_eq!(history::pinned_count(&app.history), app.settings.max_pinned);
    assert_eq!(item_text(&app.history[app.settings.max_pinned]), "unpinned");
    assert!(!app.history[app.settings.max_pinned].pinned);
}

#[test]
fn clipboard_changed_trims_to_max_history() {
    let mut app = AppModel {
        settings: test_settings(30, 5),
        ..Default::default()
    };

    for i in 0..app.settings.max_history {
        app.history
            .push_back(text_item(&format!("item-{i}"), false));
    }

    dispatch(
        &mut app,
        Message::ClipboardChanged(text_entry("fresh-entry")),
    );

    assert_eq!(app.history.len(), app.settings.max_history);
    assert_eq!(
        item_text(app.history.front().expect("front entry exists")),
        "fresh-entry"
    );
    assert!(!app.history.iter().any(|it| item_text(it) == "item-29"));
}

#[test]
fn reconcile_limits_unpins_overflow_then_reorders() {
    let mut history_vec = std::collections::VecDeque::new();
    history_vec.push_back(text_item("a", true));
    history_vec.push_back(text_item("b", true));
    history_vec.push_back(text_item("c", true));
    history_vec.push_back(text_item("d", false));

    let settings = test_settings(10, 2);
    history::reconcile_limits(&mut history_vec, &settings);

    assert!(history_vec[0].pinned);
    assert!(history_vec[1].pinned);
    assert!(!history_vec[2].pinned);
    assert_eq!(item_text(&history_vec[0]), "a");
    assert_eq!(item_text(&history_vec[1]), "b");
    assert_eq!(item_text(&history_vec[2]), "c");
}

#[test]
fn reconcile_limits_trims_oldest_unpinned_first() {
    let mut history_vec = std::collections::VecDeque::new();
    history_vec.push_back(text_item("p0", true));
    for i in 0..30 {
        history_vec.push_back(text_item(&format!("u{i}"), false));
    }

    let settings = test_settings(30, 1);
    history::reconcile_limits(&mut history_vec, &settings);

    assert_eq!(history_vec.len(), 30);
    assert_eq!(item_text(&history_vec[0]), "p0");
    assert_eq!(item_text(&history_vec[1]), "u0");
    assert_eq!(
        item_text(history_vec.back().expect("last entry exists")),
        "u28"
    );
}

#[test]
fn clear_history_preserves_pinned_entries() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("pinned", true));
    app.history.push_back(text_item("regular", false));

    dispatch(&mut app, Message::ClearHistory);

    assert_eq!(app.history.len(), 1);
    assert!(app.history[0].pinned);
    assert_eq!(item_text(&app.history[0]), "pinned");
}

#[test]
fn clear_history_is_safe_for_empty_history() {
    let mut app = AppModel::default();

    dispatch(&mut app, Message::ClearHistory);

    assert!(app.history.is_empty());
}

#[test]
fn remove_history_removes_entry_at_index() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("first", false));
    app.history.push_back(text_item("second", false));
    app.history.push_back(text_item("third", false));

    dispatch(&mut app, Message::RemoveHistory(1));

    assert_eq!(app.history.len(), 2);
    assert_eq!(item_text(&app.history[0]), "first");
    assert_eq!(item_text(&app.history[1]), "third");
}

#[test]
fn remove_history_can_delete_pinned_entry_directly() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("pinned", true));
    app.history.push_back(text_item("regular", false));

    dispatch(&mut app, Message::RemoveHistory(0));

    assert_eq!(app.history.len(), 1);
    assert_eq!(item_text(&app.history[0]), "regular");
    assert!(!app.history[0].pinned);
}

#[test]
fn remove_history_last_item_leaves_empty() {
    let mut app = AppModel::default();
    app.history.push_back(text_item("only", false));

    dispatch(&mut app, Message::RemoveHistory(0));

    assert!(app.history.is_empty());
}
