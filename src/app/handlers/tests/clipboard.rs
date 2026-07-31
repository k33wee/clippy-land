use super::*;

#[test]
fn ignores_empty_and_short_numericish_entries() {
    assert!(history::should_ignore_clipboard_entry(""));
    assert!(history::should_ignore_clipboard_entry("  \n\t  "));
    assert!(history::should_ignore_clipboard_entry("12-34"));
    assert!(history::should_ignore_clipboard_entry("1,2,3"));
}

#[test]
fn keeps_nontrivial_entries() {
    assert!(!history::should_ignore_clipboard_entry("123456789"));
    assert!(!history::should_ignore_clipboard_entry("abc123"));
    assert!(!history::should_ignore_clipboard_entry("42 is the answer"));
}

#[test]
fn clipboard_changed_dedupes_and_preserves_pin_state() {
    let repeated = text_entry("repeat");
    let mut app = AppModel::default();
    app.history.push_back(text_item("front", false));
    app.history.push_back(HistoryItem {
        entry: repeated.clone(),
        pinned: true,
    });
    app.history.push_back(text_item("tail", false));

    dispatch(&mut app, Message::ClipboardChanged(repeated.clone()));

    let matches = app.history.iter().filter(|it| it.entry == repeated).count();
    assert_eq!(matches, 1);

    let idx = app
        .history
        .iter()
        .position(|it| it.entry == repeated)
        .expect("entry should still exist");
    assert!(app.history[idx].pinned);
}

#[test]
fn clipboard_changed_caches_and_prunes_thumbnail_handles() {
    let mut app = AppModel::default();

    dispatch(&mut app, Message::ClipboardChanged(image_entry(42)));
    assert_eq!(app.thumbnail_handles.len(), 1);

    dispatch(&mut app, Message::RemoveHistory(0));
    assert!(app.history.is_empty());
    assert!(app.thumbnail_handles.is_empty());
}

#[test]
fn thumbnail_result_updates_only_the_matching_image() {
    let mut app = AppModel::default();
    app.history.push_back(HistoryItem {
        entry: ClipboardEntry::Image {
            mime: "image/png".into(),
            bytes: vec![1, 2, 3, 4].into(),
            hash: 42,
            thumbnail_png: None,
        },
        pinned: false,
    });
    app.pending_thumbnails.insert((42, 4));

    dispatch(
        &mut app,
        Message::ThumbnailReady {
            hash: 42,
            bytes_len: 4,
            thumbnail: Some(crate::services::clipboard::ClipboardThumbnail {
                width: 1,
                height: 1,
                rgba: vec![255, 0, 0, 255].into(),
                png: vec![137, 80, 78, 71].into(),
            }),
        },
    );

    assert!(!app.pending_thumbnails.contains(&(42, 4)));
    assert!(app.thumbnail_handles.contains_key(&(42, 4)));
    match &app.history[0].entry {
        ClipboardEntry::Image { thumbnail_png, .. } => {
            assert_eq!(thumbnail_png.as_deref(), Some(&[137, 80, 78, 71][..]));
        }
        ClipboardEntry::Text(_) => panic!("expected image entry"),
    }
}

#[test]
fn stale_thumbnail_result_is_discarded() {
    let mut app = AppModel::default();
    app.pending_thumbnails.insert((42, 4));

    dispatch(
        &mut app,
        Message::ThumbnailReady {
            hash: 42,
            bytes_len: 4,
            thumbnail: Some(crate::services::clipboard::ClipboardThumbnail {
                width: 1,
                height: 1,
                rgba: vec![255, 0, 0, 255].into(),
                png: vec![137, 80, 78, 71].into(),
            }),
        },
    );

    assert!(app.pending_thumbnails.is_empty());
    assert!(app.thumbnail_handles.is_empty());
}

#[test]
fn failed_thumbnail_is_cached_and_not_requeued() {
    let mut app = AppModel {
        popup: Some(cosmic::iced::window::Id::unique()),
        ..Default::default()
    };
    app.history.push_back(HistoryItem {
        entry: ClipboardEntry::Image {
            mime: "image/png".into(),
            bytes: vec![1, 2, 3, 4].into(),
            hash: 42,
            thumbnail_png: None,
        },
        pinned: false,
    });
    app.pending_thumbnails.insert((42, 4));

    dispatch(
        &mut app,
        Message::ThumbnailReady {
            hash: 42,
            bytes_len: 4,
            thumbnail: None,
        },
    );

    assert!(app.pending_thumbnails.is_empty());
    assert!(app.failed_thumbnails.contains(&(42, 4)));
}

#[test]
fn delayed_thumbnail_decode_is_cancelled_after_popup_closes() {
    let mut app = AppModel::default();
    app.pending_thumbnails.insert((42, 4));

    dispatch(
        &mut app,
        Message::ThumbnailDecodeReady {
            hash: 42,
            bytes_len: 4,
            generation: 0,
        },
    );

    assert!(app.pending_thumbnails.is_empty());
}

#[test]
fn stale_debounced_thumbnail_decode_does_not_cancel_newer_work() {
    let mut app = AppModel {
        thumbnail_schedule_generation: 2,
        ..Default::default()
    };
    app.pending_thumbnails.insert((42, 4));

    dispatch(
        &mut app,
        Message::ThumbnailDecodeReady {
            hash: 42,
            bytes_len: 4,
            generation: 1,
        },
    );

    assert!(app.pending_thumbnails.contains(&(42, 4)));
}

#[test]
fn clipboard_changed_recomputes_filtered_indices_cache() {
    let mut app = AppModel {
        search_query: "ap".into(),
        ..Default::default()
    };
    app.recompute_filtered_indices();
    assert!(app.filtered_indices.is_empty());

    dispatch(
        &mut app,
        Message::ClipboardChanged(ClipboardEntry::Text("apple".into())),
    );

    assert_eq!(app.filtered_indices, vec![0]);
}

#[test]
fn clear_history_clears_thumbnail_handles() {
    let mut app = AppModel::default();

    dispatch(&mut app, Message::ClipboardChanged(image_entry(7)));
    assert_eq!(app.thumbnail_handles.len(), 1);

    dispatch(&mut app, Message::ClearHistory);
    assert!(app.history.is_empty());
    assert!(app.thumbnail_handles.is_empty());
}

#[test]
fn clear_history_retains_pinned_image_thumbnail_handles() {
    let mut app = AppModel::default();

    dispatch(&mut app, Message::ClipboardChanged(image_entry(7)));
    dispatch(&mut app, Message::TogglePin(0));
    assert_eq!(app.history.len(), 1);
    assert!(app.history[0].pinned);
    assert_eq!(app.thumbnail_handles.len(), 1);

    dispatch(&mut app, Message::ClearHistory);

    assert_eq!(app.history.len(), 1);
    assert!(app.history[0].pinned);
    assert_eq!(app.thumbnail_handles.len(), 1);
}

#[test]
fn open_text_overlay_sets_overlay_index_for_text_entry() {
    let mut app = AppModel::default();
    app.history
        .push_back(text_item("first line\nsecond line", false));

    dispatch(&mut app, Message::OpenTextOverlay(0));

    assert_eq!(app.text_overlay_index, Some(0));
}

#[test]
fn open_text_overlay_ignores_image_entries() {
    let mut app = AppModel::default();
    app.history.push_back(HistoryItem {
        entry: image_entry(9),
        pinned: false,
    });

    dispatch(&mut app, Message::OpenTextOverlay(0));

    assert!(app.text_overlay_index.is_none());
}
