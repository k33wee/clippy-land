use super::*;

#[test]
fn toggle_settings_panel_opens_and_prefills_draft() {
    let mut app = AppModel {
        settings: AppSettings {
            max_history: 444,
            max_pinned: 33,
            max_image_bytes: 3 * 1024 * 1024,
            max_image_dimension_px: 2048,
            ..AppSettings::default()
        }
        .normalized(),
        ..Default::default()
    };

    dispatch(&mut app, Message::ToggleSettingsPanel);

    assert!(app.settings_open);
    assert_eq!(app.settings_draft.max_history, "444");
    assert_eq!(app.settings_draft.max_pinned, "33");
    assert_eq!(
        app.settings_draft.max_image_bytes,
        (3 * 1024 * 1024).to_string()
    );
    assert_eq!(app.settings_draft.max_image_dimension_px, "2048");
}

#[test]
fn apply_settings_rejects_invalid_input_with_error() {
    let mut app = AppModel {
        settings_open: true,
        ..Default::default()
    };
    app.settings_draft.max_history = "not-a-number".into();
    app.settings_draft.max_pinned = "10".into();
    app.settings_draft.max_image_bytes = "1048576".into();
    app.settings_draft.max_image_dimension_px = "2048".into();

    dispatch(&mut app, Message::ApplySettings);

    assert!(app.settings_open);
    assert!(app.settings_error.is_some());
}

#[test]
fn apply_settings_rejects_out_of_range_values() {
    let mut app = AppModel {
        settings_open: true,
        ..Default::default()
    };
    app.settings_draft.max_history = "1".into();
    app.settings_draft.max_pinned = "0".into();
    app.settings_draft.max_image_bytes = "1048576".into();
    app.settings_draft.max_image_dimension_px = "1024".into();

    dispatch(&mut app, Message::ApplySettings);

    assert!(app.settings_open);
    let err = app.settings_error.expect("range error should be present");
    assert!(err.contains("Max history must be between"));
}

#[test]
fn apply_settings_rejects_pinned_greater_than_history() {
    let mut app = AppModel {
        settings_open: true,
        ..Default::default()
    };
    app.settings_draft.max_history = "100".into();
    app.settings_draft.max_pinned = "101".into();
    app.settings_draft.max_image_bytes = "1048576".into();
    app.settings_draft.max_image_dimension_px = "1024".into();

    dispatch(&mut app, Message::ApplySettings);

    assert!(app.settings_open);
    assert_eq!(
        app.settings_error.as_deref(),
        Some("Max pinned cannot be greater than max history")
    );
}

#[test]
fn apply_settings_rejects_image_bytes_below_minimum() {
    let mut app = AppModel {
        settings_open: true,
        ..Default::default()
    };
    app.settings_draft.max_history = "200".into();
    app.settings_draft.max_pinned = "20".into();
    app.settings_draft.max_image_bytes = "1".into();
    app.settings_draft.max_image_dimension_px = "1024".into();

    dispatch(&mut app, Message::ApplySettings);

    assert!(app.settings_open);
    let err = app.settings_error.expect("range error should be present");
    assert!(err.contains("Max image bytes must be between"));
}

#[test]
fn apply_settings_updates_runtime_settings_and_closes_panel() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should be after unix epoch")
        .as_nanos();
    let cfg_path = std::env::temp_dir().join(format!("clippy-land-test-settings-{unique}.toml"));
    unsafe { std::env::set_var("CLIPPY_LAND_CONFIG", &cfg_path) };

    let mut app = AppModel {
        settings_open: true,
        ..Default::default()
    };
    app.settings_draft.max_history = "350".into();
    app.settings_draft.max_pinned = "30".into();
    app.settings_draft.max_image_bytes = "2097152".into();
    app.settings_draft.max_image_dimension_px = "4096".into();

    dispatch(&mut app, Message::ApplySettings);

    assert!(!app.settings_open);
    assert!(app.settings_error.is_none());
    assert_eq!(app.settings.max_history, 350);
    assert_eq!(app.settings.max_pinned, 30);
    assert_eq!(app.settings.max_image_bytes, 2 * 1024 * 1024);
    assert_eq!(app.settings.max_image_dimension_px, 4096);

    let persisted = std::fs::read_to_string(&cfg_path).expect("settings should be written");
    assert!(persisted.contains("max_history = 350"));
    assert!(persisted.contains("max_pinned = 30"));

    let _ = std::fs::remove_file(cfg_path);
    unsafe { std::env::remove_var("CLIPPY_LAND_CONFIG") };
}
