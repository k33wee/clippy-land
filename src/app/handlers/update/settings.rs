use super::shared::{parse_u32_field, parse_usize_field, prune_thumbnail_handles};
use crate::app::model::ImageMiBError;
use crate::app::model::SettingsDraft;
use crate::app::model::parse_image_mib;
use crate::app::{AppModel, Message, pinned_history};
use crate::fl;
use crate::services::clipboard;
use crate::settings::AppSettings;

use super::super::history;

fn image_mib_error_message(error: ImageMiBError) -> String {
    match error {
        ImageMiBError::NotANumber => fl!("error-mib-number"),
        ImageMiBError::OutOfRange => fl!("error-mib-range"),
    }
}

pub(super) fn handle(app: &mut AppModel, message: Message) -> bool {
    match message {
        Message::ToggleSettingsPanel => {
            app.settings_open = !app.settings_open;
            app.about_open = false;
            app.settings_error = None;
            app.close_preview();
            app.hovered_index = None;
            app.keyboard_focus = None;
            app.hovered_focus = None;
            if app.settings_open {
                app.settings_draft = SettingsDraft::from_settings(&app.settings);
            }
            true
        }
        Message::ToggleAbout => {
            app.about_open = !app.about_open;
            app.settings_open = false;
            app.settings_error = None;
            app.close_preview();
            app.hovered_index = None;
            app.keyboard_focus = None;
            app.hovered_focus = None;
            true
        }
        Message::BackToHistory => {
            app.settings_open = false;
            app.about_open = false;
            app.settings_error = None;
            app.hovered_index = None;
            app.keyboard_focus = None;
            app.hovered_focus = None;
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
        Message::SettingsMaxImageMibChanged(value) => {
            app.settings_draft.max_image_mib = value;
            true
        }
        Message::SettingsMaxImageDimensionChanged(value) => {
            app.settings_draft.max_image_dimension_px = value;
            true
        }
        Message::SettingsPasteOnActionToggled(enabled) => {
            let mut updated = app.settings.clone();
            updated.paste_on_action = enabled;
            if enabled {
                updated.close_popup_on_action = true;
            }
            match updated.save() {
                Ok(()) => {
                    app.settings = updated;
                    app.settings_error = None;
                }
                Err(err) => {
                    app.settings_error = Some(fl!("settings-error-save", error = err.to_string()));
                }
            }
            true
        }
        Message::SettingsClosePopupOnActionToggled(enabled) => {
            let mut updated = app.settings.clone();
            updated.close_popup_on_action = enabled;
            if !enabled {
                updated.paste_on_action = false;
            }
            match updated.save() {
                Ok(()) => {
                    app.settings = updated;
                    app.settings_error = None;
                }
                Err(err) => {
                    app.settings_error = Some(fl!("settings-error-save", error = err.to_string()));
                }
            }
            true
        }
        Message::ApplySettings => {
            let max_history = match parse_usize_field(&app.settings_draft.max_history) {
                Ok(v) => v,
                Err(err) => {
                    app.settings_error = Some(fl!("settings-error-history", error = err.message()));
                    return true;
                }
            };
            let max_pinned = match parse_usize_field(&app.settings_draft.max_pinned) {
                Ok(v) => v,
                Err(err) => {
                    app.settings_error = Some(fl!("settings-error-pinned", error = err.message()));
                    return true;
                }
            };
            let max_image_bytes = match parse_image_mib(&app.settings_draft.max_image_mib) {
                Ok(v) => v,
                Err(err) => {
                    app.settings_error = Some(fl!(
                        "settings-error-image-size",
                        error = image_mib_error_message(err)
                    ));
                    return true;
                }
            };
            let max_image_dimension_px =
                match parse_u32_field(&app.settings_draft.max_image_dimension_px) {
                    Ok(v) => v,
                    Err(err) => {
                        app.settings_error =
                            Some(fl!("settings-error-image-dimension", error = err.message()));
                        return true;
                    }
                };

            if !(AppSettings::MIN_HISTORY..=AppSettings::MAX_HISTORY).contains(&max_history) {
                app.settings_error = Some(fl!(
                    "settings-error-history-range",
                    min = (AppSettings::MIN_HISTORY as u32),
                    max = (AppSettings::MAX_HISTORY as u32)
                ));
                return true;
            }

            if !(AppSettings::MIN_PINNED..=AppSettings::MAX_PINNED).contains(&max_pinned) {
                app.settings_error = Some(fl!(
                    "settings-error-pinned-range",
                    min = (AppSettings::MIN_PINNED as u32),
                    max = (AppSettings::MAX_PINNED as u32)
                ));
                return true;
            }

            if max_pinned > max_history {
                app.settings_error = Some(fl!("settings-error-pinned-le-history"));
                return true;
            }

            if !(AppSettings::MIN_IMAGE_BYTES..=AppSettings::MAX_IMAGE_BYTES)
                .contains(&max_image_bytes)
            {
                app.settings_error = Some(fl!(
                    "settings-error-image-bytes-range",
                    min = (AppSettings::MIN_IMAGE_BYTES as u32),
                    max = (AppSettings::MAX_IMAGE_BYTES as u32)
                ));
                return true;
            }

            if !(AppSettings::MIN_IMAGE_DIMENSION_PX..=AppSettings::MAX_IMAGE_DIMENSION_PX)
                .contains(&max_image_dimension_px)
            {
                app.settings_error = Some(fl!(
                    "settings-error-image-dimension-range",
                    min = AppSettings::MIN_IMAGE_DIMENSION_PX,
                    max = AppSettings::MAX_IMAGE_DIMENSION_PX
                ));
                return true;
            }

            let updated = AppSettings {
                schema_version: 1,
                paste_on_action: app.settings.paste_on_action,
                close_popup_on_action: app.settings.close_popup_on_action,
                max_history,
                max_pinned,
                max_image_bytes,
                max_image_dimension_px,
            }
            .normalized();

            if let Err(err) = updated.save() {
                app.settings_error = Some(fl!("settings-error-save", error = err.to_string()));
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
            app.close_preview();
            app.recompute_filtered_indices();
            true
        }
        _ => false,
    }
}
