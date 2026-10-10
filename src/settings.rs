use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppSettings {
    pub schema_version: u8,
    /// Paste the selected clipboard entry into the previously focused app.
    pub paste_on_action: bool,
    /// Close the popup after copying or pasting an entry.
    pub close_popup_on_action: bool,
    pub max_history: usize,
    pub max_pinned: usize,
    pub max_image_bytes: usize,
    pub max_image_dimension_px: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            paste_on_action: true,
            close_popup_on_action: true,
            max_history: 200,
            max_pinned: 20,
            max_image_bytes: 8 * 1024 * 1024,
            max_image_dimension_px: 8192,
        }
    }
}

impl AppSettings {
    pub const MIN_HISTORY: usize = 30;
    pub const MAX_HISTORY: usize = 5000;
    pub const MIN_PINNED: usize = 0;
    pub const MAX_PINNED: usize = 500;
    pub const MIN_IMAGE_BYTES: usize = 256 * 1024;
    pub const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
    pub const MIN_IMAGE_DIMENSION_PX: u32 = 512;
    pub const MAX_IMAGE_DIMENSION_PX: u32 = 16384;

    pub fn normalized(mut self) -> Self {
        self.schema_version = 1;
        if self.paste_on_action {
            // Auto-paste relies on closing the popup first so focus can return
            // to the previously active application.
            self.close_popup_on_action = true;
        }
        self.max_history = self.max_history.clamp(Self::MIN_HISTORY, Self::MAX_HISTORY);
        self.max_pinned = self.max_pinned.clamp(Self::MIN_PINNED, Self::MAX_PINNED);
        self.max_pinned = self.max_pinned.min(self.max_history);
        self.max_image_bytes = self
            .max_image_bytes
            .clamp(Self::MIN_IMAGE_BYTES, Self::MAX_IMAGE_BYTES);
        self.max_image_dimension_px = self
            .max_image_dimension_px
            .clamp(Self::MIN_IMAGE_DIMENSION_PX, Self::MAX_IMAGE_DIMENSION_PX);
        self
    }

    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };

        Self::load_from_path(&path)
    }

    pub fn save(&self) -> io::Result<()> {
        let path = config_path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Cannot determine config path (missing HOME/XDG_CONFIG_HOME)",
            )
        })?;

        self.save_to_path(&path)
    }

    pub(crate) fn load_from_path(path: &Path) -> Self {
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::default();
        };

        toml::from_str::<AppSettings>(&raw)
            .map(Self::normalized)
            .unwrap_or_else(|_| Self::default())
    }

    pub(crate) fn save_to_path(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let normalized = self.clone().normalized();
        let serialized = toml::to_string_pretty(&normalized).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to serialize settings: {err}"),
            )
        })?;

        fs::write(path, serialized)
    }
}

fn config_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("CLIPPY_LAND_CONFIG") {
        let path = PathBuf::from(explicit);
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }

    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;

    Some(config_dir.join("clippy-land").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::AppSettings;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn defaults_are_valid_and_normalized() {
        let settings = AppSettings::default().normalized();

        assert_eq!(settings.schema_version, 1);
        assert!(settings.paste_on_action);
        assert!(settings.close_popup_on_action);
        assert!(settings.max_history >= AppSettings::MIN_HISTORY);
        assert!(settings.max_history <= AppSettings::MAX_HISTORY);
        assert!(settings.max_pinned <= settings.max_history);
        assert!(settings.max_image_bytes >= AppSettings::MIN_IMAGE_BYTES);
        assert!(settings.max_image_bytes <= AppSettings::MAX_IMAGE_BYTES);
        assert!(settings.max_image_dimension_px >= AppSettings::MIN_IMAGE_DIMENSION_PX);
        assert!(settings.max_image_dimension_px <= AppSettings::MAX_IMAGE_DIMENSION_PX);
    }

    #[test]
    fn normalization_clamps_and_relates_fields() {
        let raw = AppSettings {
            schema_version: 9,
            paste_on_action: false,
            close_popup_on_action: false,
            max_history: 1,
            max_pinned: 9999,
            max_image_bytes: 1,
            max_image_dimension_px: 99_999,
        };

        let normalized = raw.normalized();

        assert_eq!(normalized.schema_version, 1);
        assert!(!normalized.paste_on_action);
        assert!(!normalized.close_popup_on_action);
        assert_eq!(normalized.max_history, AppSettings::MIN_HISTORY);
        assert_eq!(normalized.max_pinned, AppSettings::MIN_HISTORY);
        assert_eq!(normalized.max_image_bytes, AppSettings::MIN_IMAGE_BYTES);
        assert_eq!(
            normalized.max_image_dimension_px,
            AppSettings::MAX_IMAGE_DIMENSION_PX
        );
    }

    #[test]
    fn paste_on_action_requires_popup_close() {
        let settings = AppSettings {
            paste_on_action: true,
            close_popup_on_action: false,
            ..AppSettings::default()
        }
        .normalized();

        assert!(settings.close_popup_on_action);
    }

    #[test]
    fn save_and_load_round_trip_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("clippy-land-settings-{unique}.toml"));

        let settings = AppSettings {
            schema_version: 1,
            paste_on_action: false,
            close_popup_on_action: true,
            max_history: 333,
            max_pinned: 22,
            max_image_bytes: 2 * 1024 * 1024,
            max_image_dimension_px: 4096,
        }
        .normalized();

        settings
            .save_to_path(&path)
            .expect("settings should save to path");

        let loaded = AppSettings::load_from_path(&path);
        assert_eq!(loaded, settings);

        let _ = std::fs::remove_file(path);
    }
}
