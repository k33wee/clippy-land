//! IPC mechanism for external toggle functionality via file-based signaling.
//!
//! When the `--toggle` command is invoked, it writes a timestamp to a signal file
//! in XDG_RUNTIME_DIR. A fixed-interval polling loop in the running applet detects the file,
//! deletes it, and sends a ToggleViaIpc message to open/close the popup.

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::app::Message;
use cosmic::iced::Subscription;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::stream::channel;

const SIGNAL_POLL_INTERVAL_MS: u64 = 50;

/// Get the signal file path for IPC toggle functionality.
/// Returns None if XDG_RUNTIME_DIR is not set.
pub fn get_signal_file_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CLIPPY_LAND_SIGNAL_FILE") {
        let path = PathBuf::from(path);
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }

    std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(|runtime_dir| PathBuf::from(runtime_dir).join("clippy-land-toggle"))
}

/// Send a toggle signal by writing a timestamp to the signal file.
pub fn send_toggle_signal() -> std::io::Result<()> {
    let signal_file = get_signal_file_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "XDG_RUNTIME_DIR not set - cannot send toggle signal",
        )
    })?;

    let timestamp_ms = unix_timestamp_ms()?;
    let timestamp = timestamp_ms.to_string();

    fs::write(&signal_file, timestamp)?;
    ipc_timing_log(format!(
        "ipc toggle signal written at unix_ms={} path={}",
        timestamp_ms,
        signal_file.display()
    ));
    Ok(())
}

struct SignalFileWatcher;

impl Hash for SignalFileWatcher {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "clippy-land-signal-file-watcher".hash(state);
    }
}

/// Poll for the signal file at a fixed interval.
///
/// Uses simple polling instead of filesystem notifications to avoid CPU busy-loops
/// caused by inotify on a busy /run/user/ directory.
pub fn signal_file_watcher() -> Subscription<Message> {
    Subscription::run_with(SignalFileWatcher, |_| {
        channel(1, |mut output: mpsc::Sender<Message>| async move {
            let signal_file = match get_signal_file_path() {
                Some(path) => path,
                None => {
                    futures_util::future::pending::<()>().await;
                    unreachable!();
                }
            };

            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(SIGNAL_POLL_INTERVAL_MS))
                    .await;
                if signal_file.exists() {
                    log_signal_detection(&signal_file);
                    let _ = std::fs::remove_file(&signal_file);
                    match output.send(Message::ToggleViaIpc).await {
                        Ok(()) => ipc_timing_log("ipc toggle message delivered to applet"),
                        Err(_) => {
                            ipc_timing_log("ipc toggle message receiver closed before delivery");
                            break;
                        }
                    }
                }
            }
        })
    })
}

fn log_signal_detection(signal_file: &PathBuf) {
    let Some(detected_ms) = unix_timestamp_ms().ok() else {
        ipc_timing_log(format!(
            "ipc toggle signal detected but current time could not be read: path={}",
            signal_file.display()
        ));
        return;
    };

    match fs::read_to_string(signal_file) {
        Ok(raw) => {
            let trimmed = raw.trim();
            match parse_signal_timestamp_ms(trimmed) {
                Some(written_ms) => ipc_timing_log(format!(
                    "ipc toggle signal detected after {}ms path={}",
                    detected_ms.saturating_sub(written_ms),
                    signal_file.display()
                )),
                None => ipc_timing_log(format!(
                    "ipc toggle signal detected with unparsable payload {:?} path={}",
                    trimmed,
                    signal_file.display()
                )),
            }
        }
        Err(err) => ipc_timing_log(format!(
            "ipc toggle signal detected but payload could not be read ({err}) path={}",
            signal_file.display()
        )),
    }
}

fn parse_signal_timestamp_ms(raw: &str) -> Option<u128> {
    raw.parse().ok()
}

fn unix_timestamp_ms() -> std::io::Result<u128> {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(std::io::Error::other)
}

fn ipc_timing_log(message: impl std::fmt::Display) {
    if std::env::var_os("CLIPPY_LAND_DEBUG_TIMING").is_some() {
        eprintln!("[clippy-land timing] {message}");
    }
}

#[cfg(test)]
pub(crate) fn parse_signal_timestamp_ms_for_test(raw: &str) -> Option<u128> {
    parse_signal_timestamp_ms(raw)
}
