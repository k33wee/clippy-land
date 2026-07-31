use crate::app::model::HistoryItem;
use crate::services::clipboard::ClipboardEntry;
use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u8 = 1;
#[cfg(not(test))]
const STATE_DIR_NAME: &str = "clippy-land";
const MANIFEST_FILE_NAME: &str = "pinned-history.toml";

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PinnedHistoryFile {
    schema_version: u8,
    entries: Vec<PersistedEntry>,
}

impl Default for PinnedHistoryFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PersistedEntry {
    kind: PersistedEntryKind,
    text: Option<String>,
    mime: Option<String>,
    hash: Option<u64>,
    bytes_len: Option<usize>,
    bytes_file: Option<String>,
    thumbnail_file: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum PersistedEntryKind {
    Text,
    Image,
}

pub(in crate::app) fn load(settings: &AppSettings) -> VecDeque<HistoryItem> {
    let Some(path) = pinned_history_path() else {
        return VecDeque::new();
    };

    load_from_path(&path, settings.max_pinned.min(AppSettings::MAX_PINNED))
}

pub(in crate::app) fn save(history: &VecDeque<HistoryItem>) {
    let Some(path) = pinned_history_path() else {
        return;
    };

    if let Err(err) = save_to_path(history, &path) {
        eprintln!("failed to save pinned clipboard history: {err}");
    }
}

fn load_from_path(path: &Path, max_pinned: usize) -> VecDeque<HistoryItem> {
    let Ok(raw) = fs::read_to_string(path) else {
        return VecDeque::new();
    };

    let Ok(file) = toml::from_str::<PinnedHistoryFile>(&raw) else {
        return VecDeque::new();
    };

    if file.schema_version != SCHEMA_VERSION {
        return VecDeque::new();
    }

    let blob_dir = blob_dir_for(path);
    file.entries
        .into_iter()
        .filter_map(|entry| entry.into_history_item(&blob_dir))
        .take(max_pinned)
        .collect()
}

fn save_to_path(history: &VecDeque<HistoryItem>, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let blob_dir = blob_dir_for(path);
    let mut blob_files_to_keep = HashSet::new();
    let entries = history
        .iter()
        .filter(|item| item.pinned)
        .take(AppSettings::MAX_PINNED)
        .enumerate()
        .map(|(idx, item)| entry_to_persisted(item, idx, &blob_dir, &mut blob_files_to_keep))
        .collect::<io::Result<Vec<_>>>()?;

    let file = PinnedHistoryFile {
        schema_version: SCHEMA_VERSION,
        entries,
    };
    let serialized = toml::to_string_pretty(&file).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to serialize pinned history: {err}"),
        )
    })?;

    let tmp_path = temp_path_for(path);
    fs::write(&tmp_path, serialized)?;
    fs::rename(&tmp_path, path)?;
    cleanup_blob_dir(&blob_dir, &blob_files_to_keep)?;

    Ok(())
}

fn entry_to_persisted(
    item: &HistoryItem,
    idx: usize,
    blob_dir: &Path,
    blob_files_to_keep: &mut HashSet<String>,
) -> io::Result<PersistedEntry> {
    match &item.entry {
        ClipboardEntry::Text(text) => Ok(PersistedEntry {
            kind: PersistedEntryKind::Text,
            text: Some(text.clone()),
            mime: None,
            hash: None,
            bytes_len: None,
            bytes_file: None,
            thumbnail_file: None,
        }),
        ClipboardEntry::Image {
            mime,
            bytes,
            hash,
            thumbnail_png,
        } => {
            fs::create_dir_all(blob_dir)?;

            let bytes_file = image_bytes_file_name(idx, *hash, bytes.len());
            fs::write(blob_dir.join(&bytes_file), bytes)?;
            blob_files_to_keep.insert(bytes_file.clone());

            let thumbnail_file = if let Some(thumbnail_png) = thumbnail_png {
                let thumbnail_file = image_thumbnail_file_name(idx, *hash);
                fs::write(blob_dir.join(&thumbnail_file), thumbnail_png)?;
                blob_files_to_keep.insert(thumbnail_file.clone());
                Some(thumbnail_file)
            } else {
                None
            };

            Ok(PersistedEntry {
                kind: PersistedEntryKind::Image,
                text: None,
                mime: Some(mime.clone()),
                hash: Some(*hash),
                bytes_len: Some(bytes.len()),
                bytes_file: Some(bytes_file),
                thumbnail_file,
            })
        }
    }
}

impl PersistedEntry {
    fn into_history_item(self, blob_dir: &Path) -> Option<HistoryItem> {
        match self.kind {
            PersistedEntryKind::Text => Some(HistoryItem {
                entry: ClipboardEntry::Text(self.text?),
                pinned: true,
            }),
            PersistedEntryKind::Image => {
                let mime = self.mime?;
                let hash = self.hash?;
                let bytes_len = self.bytes_len?;
                let bytes_file = self.bytes_file?;
                let bytes_path = safe_blob_path(blob_dir, &bytes_file)?;
                let bytes = fs::read(bytes_path).ok()?;
                if bytes.len() != bytes_len {
                    return None;
                }

                let thumbnail_png = self
                    .thumbnail_file
                    .and_then(|file_name| safe_blob_path(blob_dir, &file_name))
                    .and_then(|path| fs::read(path).ok())
                    .map(Into::into);

                Some(HistoryItem {
                    entry: ClipboardEntry::Image {
                        mime,
                        bytes: bytes.into(),
                        hash,
                        thumbnail_png,
                    },
                    pinned: true,
                })
            }
        }
    }
}

fn pinned_history_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("CLIPPY_LAND_PINNED_HISTORY") {
        let path = PathBuf::from(explicit);
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }

    default_pinned_history_path()
}

#[cfg(test)]
fn default_pinned_history_path() -> Option<PathBuf> {
    None
}

#[cfg(not(test))]
fn default_pinned_history_path() -> Option<PathBuf> {
    let state_dir = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;

    Some(state_dir.join(STATE_DIR_NAME).join(MANIFEST_FILE_NAME))
}

fn blob_dir_for(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("pinned-history");

    parent.join(format!("{stem}.blobs"))
}

fn safe_blob_path(blob_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let path = Path::new(file_name);
    if path.file_name()?.to_str()? != file_name {
        return None;
    }

    Some(blob_dir.join(path))
}

fn temp_path_for(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(MANIFEST_FILE_NAME);

    parent.join(format!(".{file_name}.tmp"))
}

fn image_bytes_file_name(idx: usize, hash: u64, bytes_len: usize) -> String {
    format!("entry-{idx:04}-{hash:016x}-{bytes_len}.bin")
}

fn image_thumbnail_file_name(idx: usize, hash: u64) -> String {
    format!("entry-{idx:04}-{hash:016x}-thumbnail.png")
}

fn cleanup_blob_dir(blob_dir: &Path, blob_files_to_keep: &HashSet<String>) -> io::Result<()> {
    if !blob_dir.exists() {
        return Ok(());
    }

    if blob_files_to_keep.is_empty() {
        return remove_dir_all_if_exists(blob_dir);
    }

    for entry in fs::read_dir(blob_dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };

        if blob_files_to_keep.contains(file_name) {
            continue;
        }

        let path = entry.path();
        if path.is_dir() {
            remove_dir_all_if_exists(&path)?;
        } else {
            remove_file_if_exists(&path)?;
        }
    }

    Ok(())
}

fn remove_file_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn remove_dir_all_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_manifest_path(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("clippy-land-{test_name}-{unique}"))
            .join("pinned-history.toml")
    }

    fn text_item(text: &str, pinned: bool) -> HistoryItem {
        HistoryItem {
            entry: ClipboardEntry::Text(text.to_string()),
            pinned,
        }
    }

    fn image_item(hash: u64, pinned: bool) -> HistoryItem {
        HistoryItem {
            entry: ClipboardEntry::Image {
                mime: "image/png".to_string(),
                bytes: vec![1, 2, 3, 4, 5].into(),
                hash,
                thumbnail_png: Some(vec![137, 80, 78, 71].into()),
            },
            pinned,
        }
    }

    #[test]
    fn save_and_load_round_trip_keeps_only_pinned_entries() {
        let path = unique_manifest_path("round-trip");
        let mut history = VecDeque::new();
        history.push_back(text_item("saved text", true));
        history.push_back(text_item("regular text", false));
        history.push_back(image_item(0xfeed_beef, true));

        save_to_path(&history, &path).expect("pinned history should save");

        let loaded = load_from_path(&path, AppSettings::MAX_PINNED);
        assert_eq!(loaded.len(), 2);
        assert!(loaded.iter().all(|item| item.pinned));

        match &loaded[0].entry {
            ClipboardEntry::Text(text) => assert_eq!(text, "saved text"),
            ClipboardEntry::Image { .. } => panic!("expected text entry"),
        }

        match &loaded[1].entry {
            ClipboardEntry::Image {
                mime,
                bytes,
                hash,
                thumbnail_png,
            } => {
                assert_eq!(mime, "image/png");
                assert_eq!(bytes.as_ref(), &[1, 2, 3, 4, 5]);
                assert_eq!(*hash, 0xfeed_beef);
                assert_eq!(thumbnail_png.as_deref(), Some(&[137, 80, 78, 71][..]));
            }
            ClipboardEntry::Text(_) => panic!("expected image entry"),
        }

        let raw = fs::read_to_string(&path).expect("manifest should be readable");
        assert!(!raw.contains("regular text"));

        let _ = fs::remove_dir_all(path.parent().expect("manifest has parent"));
    }

    #[test]
    fn load_caps_entries_to_current_max_pinned() {
        let path = unique_manifest_path("cap");
        let mut history = VecDeque::new();
        history.push_back(text_item("a", true));
        history.push_back(text_item("b", true));

        save_to_path(&history, &path).expect("pinned history should save");

        let loaded = load_from_path(&path, 1);
        assert_eq!(loaded.len(), 1);
        match &loaded[0].entry {
            ClipboardEntry::Text(text) => assert_eq!(text, "a"),
            ClipboardEntry::Image { .. } => panic!("expected text entry"),
        }

        let _ = fs::remove_dir_all(path.parent().expect("manifest has parent"));
    }

    #[test]
    fn save_empty_pinned_history_removes_stale_blob_dir() {
        let path = unique_manifest_path("empty");
        let mut history = VecDeque::new();
        history.push_back(image_item(0xabc, true));
        save_to_path(&history, &path).expect("initial pinned history should save");
        assert!(blob_dir_for(&path).exists());

        history.clear();
        save_to_path(&history, &path).expect("empty pinned history should save");

        assert!(!blob_dir_for(&path).exists());
        let loaded = load_from_path(&path, AppSettings::MAX_PINNED);
        assert!(loaded.is_empty());

        let _ = fs::remove_dir_all(path.parent().expect("manifest has parent"));
    }

    #[test]
    fn load_skips_missing_image_blob_and_keeps_later_valid_pin() {
        let path = unique_manifest_path("missing-blob");
        let mut history = VecDeque::new();
        history.push_back(image_item(0xdef, true));
        history.push_back(text_item("still saved", true));

        save_to_path(&history, &path).expect("pinned history should save");
        fs::remove_dir_all(blob_dir_for(&path)).expect("blob dir should be removable");

        let loaded = load_from_path(&path, 1);
        assert_eq!(loaded.len(), 1);
        match &loaded[0].entry {
            ClipboardEntry::Text(text) => assert_eq!(text, "still saved"),
            ClipboardEntry::Image { .. } => panic!("missing image blob should be skipped"),
        }

        let _ = fs::remove_dir_all(path.parent().expect("manifest has parent"));
    }
}
