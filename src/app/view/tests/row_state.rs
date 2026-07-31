use super::*;

#[test]
fn row_render_state_text_snapshot_keeps_only_needed_summaries() {
    let mut app = AppModel::default();
    let long_text = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz";
    push_text(&mut app, long_text);

    let state = RowRenderState::from_app(&app, 0, &app.history[0]);

    match state.content {
        RowContent::Text {
            collapsed_summary,
            expanded_summary,
            overlay_available,
        } => {
            assert_eq!(collapsed_summary, summarize_one_line(long_text));
            assert_eq!(
                expanded_summary,
                summarize_one_line_with_limit(long_text, 300)
            );
            assert!(overlay_available);
        }
        RowContent::Image { .. } => panic!("expected text row snapshot"),
    }
}

#[test]
fn row_render_state_image_snapshot_keeps_lightweight_metadata_and_handle() {
    let mut app = AppModel::default();
    let thumbnail = vec![1, 2, 3, 4];
    app.history.push_back(HistoryItem {
        entry: ClipboardEntry::Image {
            mime: "image/png".into(),
            bytes: vec![7; 4096].into(),
            hash: 42,
            thumbnail_png: Some(thumbnail.clone().into()),
        },
        pinned: true,
    });
    app.thumbnail_handles
        .insert((42, 4096), ImageHandle::from_bytes(thumbnail));

    let state = RowRenderState::from_app(&app, 0, &app.history[0]);

    match state.content {
        RowContent::Image {
            mime,
            bytes_len,
            content_hash,
            thumbnail_handle,
        } => {
            assert_eq!(mime, "image/png");
            assert_eq!(bytes_len, 4096);
            assert_eq!(content_hash, 42);
            assert!(thumbnail_handle.is_some());
            assert!(state.pinned);
        }
        RowContent::Text { .. } => panic!("expected image row snapshot"),
    }
}

#[test]
fn row_render_state_image_snapshot_without_cached_handle_keeps_none() {
    let mut app = AppModel::default();
    app.history.push_back(HistoryItem {
        entry: ClipboardEntry::Image {
            mime: "image/png".into(),
            bytes: vec![7; 4096].into(),
            hash: 777,
            thumbnail_png: Some(vec![1, 2, 3, 4].into()),
        },
        pinned: false,
    });

    let state = RowRenderState::from_app(&app, 0, &app.history[0]);

    match state.content {
        RowContent::Image {
            thumbnail_handle, ..
        } => {
            assert!(thumbnail_handle.is_none());
        }
        RowContent::Text { .. } => panic!("expected image row snapshot"),
    }
}
