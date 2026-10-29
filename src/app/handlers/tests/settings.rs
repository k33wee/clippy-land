use super::*;
use crate::app::model::{SettingsDraft, parse_image_mib};

static CONFIG_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn settings_app() -> AppModel {
    let mut app = AppModel::default();
    app.settings_open = true;
    app.settings_draft = SettingsDraft::from_settings(&app.settings);
    app
}

#[test]
fn toggle_settings_panel_opens_and_prefills_readable_units() {
    let mut app = AppModel::default();
    app.settings.max_history = 444;
    app.settings.max_pinned = 33;
    app.settings.max_image_bytes = 3 * 1024 * 1024;
    dispatch(&mut app, Message::ToggleSettingsPanel);
    assert!(app.settings_open);
    assert_eq!(app.settings_draft.max_history, "444");
    assert_eq!(app.settings_draft.max_pinned, "33");
    assert_eq!(app.settings_draft.max_image_mib, "3");
    assert!(!app.about_open);
}

#[test]
fn mib_conversion_preserves_existing_byte_limits() {
    for bytes in [
        262_144, 262_145, 1_000_000, 1_048_575, 8_388_608, 8_388_609, 67_108_863, 67_108_864,
    ] {
        let settings = AppSettings {
            max_image_bytes: bytes,
            ..AppSettings::default()
        };
        let draft = SettingsDraft::from_settings(&settings);
        assert_eq!(parse_image_mib(&draft.max_image_mib), Ok(bytes));
    }
    assert_eq!(parse_image_mib(" 0.5 "), Ok(524_288));
    for value in ["", "NaN", "inf", "-1", "0.24", "64.1", "1e100", "words"] {
        assert!(parse_image_mib(value).is_err(), "{value} must fail");
    }
}

#[test]
fn apply_settings_rejects_invalid_input_and_keeps_editor_open() {
    for (field, value, expected) in [
        ("history", "not-a-number", "Max history"),
        ("history", "1", "Max history must be between"),
        ("pins", "501", "Max pinned must be between"),
        ("image", "0.1", "Image size"),
        ("image", "NaN", "Image size"),
        ("edge", "10", "Max image dimension must be between"),
    ] {
        let mut app = settings_app();
        match field {
            "history" => app.settings_draft.max_history = value.into(),
            "pins" => app.settings_draft.max_pinned = value.into(),
            "image" => app.settings_draft.max_image_mib = value.into(),
            "edge" => app.settings_draft.max_image_dimension_px = value.into(),
            _ => unreachable!(),
        }
        dispatch(&mut app, Message::ApplySettings);
        assert!(app.settings_open);
        assert!(app.settings_error.as_deref().unwrap().contains(expected));
    }
}

#[test]
fn apply_settings_rejects_pinned_greater_than_history() {
    let mut app = settings_app();
    app.settings_draft.max_history = "100".into();
    app.settings_draft.max_pinned = "101".into();
    dispatch(&mut app, Message::ApplySettings);
    assert_eq!(
        app.settings_error.as_deref(),
        Some("Max pinned cannot be greater than max history")
    );
}

fn with_temporary_config(test: impl FnOnce(&std::path::Path)) {
    let _guard = CONFIG_ENV_LOCK.lock().unwrap();
    let path = std::env::temp_dir().join(format!(
        "clippy-land-ui-settings-{}-{:?}.toml",
        std::process::id(),
        std::thread::current().id()
    ));
    struct RestoreConfig {
        previous: Option<std::ffi::OsString>,
        path: std::path::PathBuf,
    }
    impl Drop for RestoreConfig {
        fn drop(&mut self) {
            unsafe {
                if let Some(value) = &self.previous {
                    std::env::set_var("CLIPPY_LAND_CONFIG", value);
                } else {
                    std::env::remove_var("CLIPPY_LAND_CONFIG");
                }
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }
    let _restore = RestoreConfig {
        previous: std::env::var_os("CLIPPY_LAND_CONFIG"),
        path: path.clone(),
    };
    unsafe {
        std::env::set_var("CLIPPY_LAND_CONFIG", &path);
    }
    test(&path);
}

#[test]
fn apply_settings_converts_mib_without_changing_the_config_schema() {
    with_temporary_config(|path| {
        let mut app = settings_app();
        app.settings_draft.max_history = "350".into();
        app.settings_draft.max_pinned = "30".into();
        app.settings_draft.max_image_mib = "2.5".into();
        dispatch(&mut app, Message::ApplySettings);
        assert!(!app.settings_open);
        assert!(app.settings_error.is_none());
        assert_eq!(app.settings.max_history, 350);
        assert_eq!(app.settings.max_pinned, 30);
        assert_eq!(app.settings.max_image_bytes, 2_621_440);
        let persisted = std::fs::read_to_string(path).unwrap();
        assert!(persisted.contains("max_image_bytes = 2621440"));
        assert!(!persisted.contains("max_image_mib"));
    });
}

#[test]
fn behavior_toggles_still_update_and_persist_immediately() {
    with_temporary_config(|path| {
        let mut app = AppModel::default();
        dispatch(&mut app, Message::SettingsClosePopupOnActionToggled(false));
        assert!(!app.settings.paste_on_action);
        assert!(!app.settings.close_popup_on_action);
        dispatch(&mut app, Message::SettingsPasteOnActionToggled(true));
        assert!(app.settings.paste_on_action);
        assert!(app.settings.close_popup_on_action);
        dispatch(&mut app, Message::SettingsPasteOnActionToggled(false));
        assert!(!app.settings.paste_on_action);
        assert!(app.settings.close_popup_on_action);
        let persisted = std::fs::read_to_string(path).unwrap();
        assert!(persisted.contains("paste_on_action = false"));
    });
}
