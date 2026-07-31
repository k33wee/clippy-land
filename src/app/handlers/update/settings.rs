use super::shared::{parse_u32_field, parse_usize_field, prune_thumbnail_handles};
use crate::app::model::SettingsDraft;
use crate::app::{AppModel, Message, pinned_history};
use crate::services::clipboard;
use crate::settings::AppSettings;

use super::super::history;

pub(super) fn handle(app: &mut AppModel, message: Message) -> bool {
    match message {
        Message::ToggleSettingsPanel => {
            app.settings_open = !app.settings_open;
            app.settings_error = None;
            app.text_overlay_index = None;
            if app.settings_open {
                app.settings_draft = SettingsDraft::from_settings(&app.settings);
            }
            true
        }
        Message::SettingsMaxHistoryChanged(value) => {
            app.settings_draft.max_history = value;
            true
        }
        Message::SettingsMaxPinnedChanged(value) => {
            app.settings_draft.max_pinned = value;
            true
        }
        Message::SettingsMaxImageBytesChanged(value) => {
            app.settings_draft.max_image_bytes = value;
            true
        }
        Message::SettingsMaxImageDimensionChanged(value) => {
            app.settings_draft.max_image_dimension_px = value;
            true
        }
        Message::ApplySettings => {
            let max_history = match parse_usize_field(&app.settings_draft.max_history) {
                Ok(v) => v,
                Err(err) => {
                    app.settings_error = Some(format!("Max history: {err}"));
                    return true;
                }
            };
            let max_pinned = match parse_usize_field(&app.settings_draft.max_pinned) {
                Ok(v) => v,
                Err(err) => {
                    app.settings_error = Some(format!("Max pinned: {err}"));
                    return true;
                }
            };
            let max_image_bytes = match parse_usize_field(&app.settings_draft.max_image_bytes) {
                Ok(v) => v,
                Err(err) => {
                    app.settings_error = Some(format!("Max image bytes: {err}"));
                    return true;
                }
            };
            let max_image_dimension_px =
                match parse_u32_field(&app.settings_draft.max_image_dimension_px) {
                    Ok(v) => v,
                    Err(err) => {
                        app.settings_error = Some(format!("Max image dimension: {err}"));
                        return true;
                    }
                };

            if !(AppSettings::MIN_HISTORY..=AppSettings::MAX_HISTORY).contains(&max_history) {
                app.settings_error = Some(format!(
                    "Max history must be between {} and {}",
                    AppSettings::MIN_HISTORY,
                    AppSettings::MAX_HISTORY
                ));
                return true;
            }

            if !(AppSettings::MIN_PINNED..=AppSettings::MAX_PINNED).contains(&max_pinned) {
                app.settings_error = Some(format!(
                    "Max pinned must be between {} and {}",
                    AppSettings::MIN_PINNED,
                    AppSettings::MAX_PINNED
                ));
                return true;
            }

            if max_pinned > max_history {
                app.settings_error = Some("Max pinned cannot be greater than max history".into());
                return true;
            }

            if !(AppSettings::MIN_IMAGE_BYTES..=AppSettings::MAX_IMAGE_BYTES)
                .contains(&max_image_bytes)
            {
                app.settings_error = Some(format!(
                    "Max image bytes must be between {} and {}",
                    AppSettings::MIN_IMAGE_BYTES,
                    AppSettings::MAX_IMAGE_BYTES
                ));
                return true;
            }

            if !(AppSettings::MIN_IMAGE_DIMENSION_PX..=AppSettings::MAX_IMAGE_DIMENSION_PX)
                .contains(&max_image_dimension_px)
            {
                app.settings_error = Some(format!(
                    "Max image dimension must be between {} and {}",
                    AppSettings::MIN_IMAGE_DIMENSION_PX,
                    AppSettings::MAX_IMAGE_DIMENSION_PX
                ));
                return true;
            }

            let updated = AppSettings {
                schema_version: 1,
                max_history,
                max_pinned,
                max_image_bytes,
                max_image_dimension_px,
            }
            .normalized();

            if let Err(err) = updated.save() {
                app.settings_error = Some(format!("Failed to save settings: {err}"));
                return true;
            }

            app.settings = updated;
            app.settings_draft = SettingsDraft::from_settings(&app.settings);
            app.settings_error = None;
            app.settings_open = false;

            clipboard::configure_limits(
                app.settings.max_image_bytes,
                app.settings.max_image_dimension_px,
            );
            app.failed_thumbnails.clear();
            history::reconcile_limits(&mut app.history, &app.settings);
            pinned_history::save(&app.history);
            prune_thumbnail_handles(app);
            app.text_overlay_index = None;
            app.recompute_filtered_indices();
            true
        }
        _ => false,
    }
}
