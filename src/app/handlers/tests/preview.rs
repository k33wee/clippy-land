use super::*;
use crate::app::model::ImagePreview;

fn image_app() -> AppModel {
    let mut app = AppModel {
        popup: Some(cosmic::iced::window::Id::unique()),
        ..Default::default()
    };
    app.history.push_back(HistoryItem {
        entry: image_entry(17),
        pinned: false,
    });
    app.recompute_filtered_indices();
    app
}

#[test]
fn image_preview_is_available_to_keyboard_navigation() {
    let mut app = image_app();
    app.hovered_index = Some(0);
    app.keyboard_focus = Some((0, FocusPart::Entry));
    dispatch(&mut app, Message::MoveFocusRight);
    assert_eq!(app.keyboard_focus, Some((0, FocusPart::Preview)));
    dispatch(&mut app, Message::ActivateSelection);
    assert!(app.image_preview.is_some());
    assert!(app.text_overlay_index.is_none());
}

#[test]
fn image_preview_discards_stale_results_and_releases_closed_images() {
    let mut app = image_app();
    dispatch(&mut app, Message::OpenPreview(0));
    let first = app.image_preview.as_ref().unwrap();
    let (key, old_generation) = (first.key, first.generation);
    dispatch(&mut app, Message::ClosePreview);
    dispatch(&mut app, Message::OpenPreview(0));
    let generation = app.image_preview.as_ref().unwrap().generation;
    assert_ne!(generation, old_generation);
    let image = || {
        Some(crate::services::clipboard::ClipboardPreview {
            width: 1,
            height: 1,
            rgba: vec![255; 4].into(),
        })
    };
    dispatch(
        &mut app,
        Message::ImagePreviewReady {
            key,
            generation: old_generation,
            preview: image(),
        },
    );
    assert!(app.image_preview.as_ref().unwrap().handle.is_none());
    dispatch(
        &mut app,
        Message::ImagePreviewReady {
            key,
            generation,
            preview: image(),
        },
    );
    assert!(app.image_preview.as_ref().unwrap().handle.is_some());
    dispatch(&mut app, Message::EscapePressed);
    assert!(app.image_preview.is_none());
    assert!(app.popup.is_some());
    dispatch(
        &mut app,
        Message::ImagePreviewReady {
            key,
            generation,
            preview: image(),
        },
    );
    assert!(app.image_preview.is_none());
}

#[tokio::test]
async fn switching_previews_keeps_only_one_worker_and_starts_the_latest_request() {
    use cosmic::iced::futures::StreamExt;
    use cosmic::iced::runtime::{Action as RuntimeAction, task};
    let mut app = image_app();
    app.history.push_back(HistoryItem {
        entry: image_entry(18),
        pinned: false,
    });
    let first = super::update(&mut app, Message::OpenPreview(0));
    let old_generation = app.image_preview_in_flight.unwrap();
    let queued = super::update(&mut app, Message::OpenPreview(1));
    let current = app.image_preview.as_ref().unwrap().generation;
    assert_ne!(old_generation, current);
    assert_eq!(app.image_preview_in_flight, Some(old_generation));
    assert!(
        task::into_stream(queued).is_none(),
        "no queued image task or captured buffer"
    );
    let events = task::into_stream(first).unwrap().collect::<Vec<_>>().await;
    let RuntimeAction::Output(cosmic::Action::App(message)) = events.into_iter().next().unwrap()
    else {
        panic!("preview result");
    };
    let latest = super::update(&mut app, message);
    assert_eq!(app.image_preview_in_flight, Some(current));
    let events = task::into_stream(latest).unwrap().collect::<Vec<_>>().await;
    for event in events {
        if let RuntimeAction::Output(cosmic::Action::App(message)) = event {
            dispatch(&mut app, message);
        }
    }
    assert!(app.image_preview_in_flight.is_none());
    assert!(app.image_preview.as_ref().unwrap().failed);
    assert_eq!(app.image_preview.as_ref().unwrap().key.0, 18);
}

#[test]
fn failed_preview_is_an_explicit_state_not_a_blank_image() {
    let mut app = image_app();
    dispatch(&mut app, Message::OpenPreview(0));
    let active = app.image_preview.as_ref().unwrap();
    let (key, generation) = (active.key, active.generation);
    dispatch(
        &mut app,
        Message::ImagePreviewReady {
            key,
            generation,
            preview: None,
        },
    );
    assert!(app.image_preview.as_ref().unwrap().failed);
}

#[test]
fn secondary_pages_and_previews_do_not_activate_hidden_history_actions() {
    for page in ["settings", "about", "image", "text"] {
        let mut app = image_app();
        app.hovered_index = Some(0);
        app.keyboard_focus = Some((0, FocusPart::Remove));
        match page {
            "settings" => app.settings_open = true,
            "about" => app.about_open = true,
            "image" => {
                app.image_preview = Some(ImagePreview {
                    key: (17, 4),
                    generation: 1,
                    handle: None,
                    failed: false,
                })
            }
            "text" => app.text_overlay_index = Some(0),
            _ => unreachable!(),
        }
        dispatch(&mut app, Message::ActivateSelection);
        dispatch(&mut app, Message::MoveFocusRight);
        assert_eq!(app.history.len(), 1, "{page}");
        assert_eq!(app.keyboard_focus, Some((0, FocusPart::Remove)), "{page}");
    }
}

#[test]
fn about_and_settings_are_mutually_exclusive_and_escape_returns_to_history() {
    let mut app = image_app();
    dispatch(&mut app, Message::OpenPreview(0));
    dispatch(&mut app, Message::ToggleAbout);
    assert!(app.about_open);
    assert!(!app.preview_is_open());
    dispatch(&mut app, Message::ToggleSettingsPanel);
    assert!(!app.about_open);
    assert!(app.settings_open);
    dispatch(&mut app, Message::EscapePressed);
    assert!(!app.settings_open);
    assert!(app.popup.is_some());
    dispatch(&mut app, Message::ToggleAbout);
    dispatch(&mut app, Message::EscapePressed);
    assert!(!app.about_open);
    assert!(app.popup.is_some());
}

#[test]
fn removing_image_closes_preview_and_prunes_metadata() {
    let mut app = image_app();
    app.image_dimensions.insert((17, 4), Some((800, 600)));
    dispatch(&mut app, Message::OpenPreview(0));
    dispatch(&mut app, Message::RemoveHistory(0));
    assert!(app.image_preview.is_none());
    assert!(app.image_dimensions.is_empty());
}
