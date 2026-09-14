//! Event-driven D-Bus IPC for the external `--toggle` command.
//!
//! The running applet owns a well-known name on the user's session bus. A short-lived
//! `--toggle` process calls its `Toggle` method, so the applet sleeps until a request arrives
//! instead of polling a signal file while idle.

use std::hash::{Hash, Hasher};
use std::time::SystemTime;

use crate::app::Message;
use cosmic::iced::Subscription;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::stream::channel;

pub(crate) const BUS_NAME: &str = "io.github.k33wee.ClippyLand";
pub(crate) const OBJECT_PATH: &str = "/io/github/k33wee/ClippyLand";
pub(crate) const INTERFACE_NAME: &str = "io.github.k33wee.ClippyLand";
const BUS_NAME_ENV: &str = "CLIPPY_LAND_BUS_NAME";

/// Ask the running applet to toggle its popup over the user's session bus.
pub fn send_toggle() -> std::io::Result<()> {
    let started_ms = unix_timestamp_ms().ok();
    let bus_name = bus_name();
    let connection = zbus::blocking::Connection::session().map_err(dbus_io_error)?;
    let proxy = zbus::blocking::Proxy::new(&connection, bus_name, OBJECT_PATH, INTERFACE_NAME)
        .map_err(dbus_io_error)?;
    proxy.call_method("Toggle", &()).map_err(dbus_io_error)?;

    if let (Some(started_ms), Ok(completed_ms)) = (started_ms, unix_timestamp_ms()) {
        ipc_timing_log(format!(
            "ipc D-Bus toggle completed in {}ms",
            completed_ms.saturating_sub(started_ms)
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct ToggleService {
    output: mpsc::Sender<Message>,
}

#[zbus::interface(name = "io.github.k33wee.ClippyLand")]
impl ToggleService {
    async fn toggle(&self) {
        let mut output = self.output.clone();
        match output.send(Message::ToggleViaIpc).await {
            Ok(()) => ipc_timing_log("ipc D-Bus toggle delivered to applet"),
            Err(_) => ipc_timing_log("ipc D-Bus toggle receiver closed before delivery"),
        }
    }
}

struct DbusToggleWatcher;

impl Hash for DbusToggleWatcher {
    fn hash<H: Hasher>(&self, state: &mut H) {
        "clippy-land-dbus-toggle-watcher".hash(state);
    }
}

/// Listen for toggle requests without periodic wakeups while the applet is idle.
pub fn toggle_watcher() -> Subscription<Message> {
    Subscription::run_with(DbusToggleWatcher, |_| {
        channel(1, |output: mpsc::Sender<Message>| async move {
            let connection = zbus::connection::Builder::session()
                .and_then(|builder| builder.name(bus_name()))
                .and_then(|builder| builder.serve_at(OBJECT_PATH, ToggleService { output }));

            match connection {
                Ok(builder) => match builder.build().await {
                    Ok(_connection) => {
                        ipc_timing_log("ipc D-Bus toggle service ready");
                        futures_util::future::pending::<()>().await;
                    }
                    Err(err) => {
                        eprintln!("Failed to start clippy-land D-Bus service: {err}");
                        // Keep this subscription alive. Ending it would make iced repeatedly
                        // recreate a failing subscription and waste CPU.
                        futures_util::future::pending::<()>().await;
                    }
                },
                Err(err) => {
                    eprintln!("Failed to configure clippy-land D-Bus service: {err}");
                    futures_util::future::pending::<()>().await;
                }
            }
        })
    })
}

fn bus_name() -> String {
    std::env::var(BUS_NAME_ENV)
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| BUS_NAME.to_owned())
}

fn dbus_io_error(error: zbus::Error) -> std::io::Error {
    std::io::Error::other(error)
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
