//! Connections for compositor services, separate from the panel's UI proxy.
//!
//! COSMIC Panel grants `X-HostWaylandDisplay` applets one privileged socket.
//! Adopt it once, before any threads start, and share the Wayland backend (not
//! duplicated socket descriptors) between clipboard, tracking, and keyboard queues.
//! Without that grant, use the display socket and let protocol availability
//! determine whether each service can run. Never consume `WAYLAND_SOCKET`: it
//! belongs to libcosmic's panel-hosted UI.

use std::ffi::{OsStr, OsString};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::OnceLock;

use wayland_client::Connection;

static PRIVILEGED_CONNECTION: OnceLock<Option<Connection>> = OnceLock::new();

/// Called from `main` before libcosmic or any worker threads are started.
pub fn initialize() {
    let socket = std::env::var_os("X_PRIVILEGED_WAYLAND_SOCKET");
    // SAFETY: main calls this before starting any threads. Removing the owning
    // variable prevents another consumer or a child from adopting the same FD.
    unsafe { std::env::remove_var("X_PRIVILEGED_WAYLAND_SOCKET") };
    let ui_fd = std::env::var("WAYLAND_SOCKET")
        .ok()
        .and_then(|raw| raw.parse().ok());
    PRIVILEGED_CONNECTION.get_or_init(|| {
        socket.and_then(
            |raw| match adopt_socket(&raw, ui_fd).and_then(connection_from_socket) {
                Ok(conn) => {
                    if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
                        eprintln!(
                            "[clippy-land] using panel-granted privileged Wayland connection"
                        );
                    }
                    Some(conn)
                }
                Err(err) => {
                    eprintln!("clippy-land: privileged Wayland connection unavailable ({err})");
                    None
                }
            },
        )
    });
}

pub fn has_privileged_connection() -> bool {
    PRIVILEGED_CONNECTION.get().is_some_and(Option::is_some)
}

/// A clone of the granted connection, or a fresh ordinary display connection.
pub fn connection() -> Result<Connection, String> {
    select_connection(
        PRIVILEGED_CONNECTION.get().and_then(Option::as_ref),
        std::env::var_os("WAYLAND_DISPLAY"),
        std::env::var_os("XDG_RUNTIME_DIR"),
    )
}

fn connection_from_socket(socket: UnixStream) -> Result<Connection, String> {
    Connection::from_socket(socket).map_err(|err| format!("wayland connect: {err}"))
}

fn adopt_socket(raw: &OsStr, ui_fd: Option<RawFd>) -> Result<UnixStream, String> {
    let fd: RawFd = raw
        .to_str()
        .and_then(|raw| raw.parse().ok())
        .filter(|&fd| fd > 2)
        .ok_or_else(|| "invalid privileged socket descriptor".to_string())?;
    if Some(fd) == ui_fd {
        return Err("privileged descriptor aliases the panel UI socket".into());
    }

    // Validate before taking ownership: malformed/stale environment values must
    // not close stdio, unrelated files, or trigger an IO-safety abort. This app
    // runs on Linux; /proc also lets us validate without borrowing an invalid FD.
    let metadata = std::fs::metadata(format!("/proc/self/fd/{fd}"))
        .map_err(|err| format!("privileged socket descriptor: {err}"))?;
    if !metadata.file_type().is_socket() {
        return Err("privileged descriptor is not a socket".into());
    }
    // SAFETY: this is the panel's inherited, open socket, validated above.
    // Startup is single-threaded and the owning environment variable is removed.
    let socket = unsafe { UnixStream::from_raw_fd(fd) };
    socket
        .peer_addr()
        .map_err(|err| format!("privileged socket is not a connected Unix socket: {err}"))?;
    let flags = rustix::io::fcntl_getfd(&socket).map_err(|err| format!("socket flags: {err}"))?;
    rustix::io::fcntl_setfd(&socket, flags | rustix::io::FdFlags::CLOEXEC)
        .map_err(|err| format!("socket close-on-exec: {err}"))?;
    Ok(socket)
}

fn select_connection(
    privileged: Option<&Connection>,
    display: Option<OsString>,
    runtime_dir: Option<OsString>,
) -> Result<Connection, String> {
    if let Some(conn) = privileged {
        return Ok(conn.clone());
    }
    let path = display_path(display, runtime_dir)?;
    let socket = UnixStream::connect(&path)
        .map_err(|err| format!("wayland display {}: {err}", path.display()))?;
    connection_from_socket(socket)
}

fn display_path(
    display: Option<OsString>,
    runtime_dir: Option<OsString>,
) -> Result<PathBuf, String> {
    let display = PathBuf::from(display.ok_or("WAYLAND_DISPLAY is not set")?);
    if display.is_absolute() {
        return Ok(display);
    }
    if display.as_os_str().is_empty() {
        return Err("WAYLAND_DISPLAY is empty".into());
    }
    let runtime_dir = PathBuf::from(runtime_dir.ok_or("XDG_RUNTIME_DIR is not set")?);
    if !runtime_dir.is_absolute() {
        return Err("XDG_RUNTIME_DIR is not absolute".into());
    }
    Ok(runtime_dir.join(display))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::{AsRawFd, IntoRawFd};

    #[test]
    fn rejects_malformed_and_stdio_descriptors() {
        for raw in ["", "abc", "-1", "0", "1", "2", "2147483648", "2147483647"] {
            assert!(
                adopt_socket(OsStr::new(raw), None).is_err(),
                "accepted {raw}"
            );
        }
    }

    #[test]
    fn rejects_non_socket_without_taking_ownership() {
        let file = std::fs::File::open("/dev/null").unwrap();
        let raw = file.as_raw_fd().to_string();
        assert!(adopt_socket(OsStr::new(&raw), None).is_err());
        assert!(file.metadata().is_ok(), "unrelated descriptor was closed");
    }

    #[test]
    fn adopts_inherited_socket_and_sets_close_on_exec() {
        let (socket, _peer) = UnixStream::pair().unwrap();
        rustix::io::fcntl_setfd(&socket, rustix::io::FdFlags::empty()).unwrap();
        let fd = socket.into_raw_fd();
        let adopted = adopt_socket(OsStr::new(&fd.to_string()), None).unwrap();
        assert_eq!(adopted.as_raw_fd(), fd);
        assert!(
            rustix::io::fcntl_getfd(&adopted)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }

    #[test]
    fn privileged_consumers_share_one_backend_even_across_threads() {
        let (socket, _peer) = UnixStream::pair().unwrap();
        let conn = connection_from_socket(socket).unwrap();
        let tracker = select_connection(Some(&conn), None, None).unwrap();
        let keyboard = select_connection(Some(&conn), Some("missing".into()), None).unwrap();
        assert_eq!(conn, tracker);
        assert_eq!(conn, keyboard);
        std::thread::spawn(move || assert_eq!(tracker, keyboard))
            .join()
            .unwrap();
    }

    #[test]
    fn rejects_alias_of_ui_socket_without_closing_it() {
        let (socket, _peer) = UnixStream::pair().unwrap();
        let fd = socket.as_raw_fd();
        assert!(adopt_socket(OsStr::new(&fd.to_string()), Some(fd)).is_err());
        assert!(socket.peer_addr().is_ok(), "UI socket was consumed");
    }

    #[test]
    fn shared_backend_dispatches_two_event_queues_concurrently() {
        use std::io::{Read, Write};
        use std::sync::{Arc, Barrier, mpsc};
        use std::time::Duration;

        let (socket, mut server) = UnixStream::pair().unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let (stop_tx, stop_rx) = mpsc::channel();
        // Minimal Wayland peer: answer two wl_display.sync requests with
        // wl_callback.done and wl_display.delete_id, keeping the socket alive
        // until both queues have consumed their own callbacks.
        let peer = std::thread::spawn(move || {
            for _ in 0..2 {
                let mut request = [0u8; 12];
                server.read_exact(&mut request).unwrap();
                assert_eq!(u32::from_ne_bytes(request[..4].try_into().unwrap()), 1);
                assert_eq!(
                    u32::from_ne_bytes(request[4..8].try_into().unwrap()),
                    12 << 16
                );
                let callback = u32::from_ne_bytes(request[8..].try_into().unwrap());
                for message in [[callback, 12 << 16, 0], [1, (12 << 16) | 1, callback]] {
                    for word in message {
                        server.write_all(&word.to_ne_bytes()).unwrap();
                    }
                }
            }
            stop_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        let conn = connection_from_socket(socket).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let conn = conn.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let mut queue = conn.new_event_queue::<()>();
                    barrier.wait();
                    queue.roundtrip(&mut ()).unwrap();
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        stop_tx.send(()).unwrap();
        peer.join().unwrap();
    }

    #[test]
    fn desktop_entry_requests_privileged_panel_access() {
        assert!(
            include_str!("../../resources/io.github.k33wee.clippy-land.desktop")
                .lines()
                .any(|line| line == "X-HostWaylandDisplay=true")
        );
    }

    #[test]
    fn flatpak_toggle_permission_is_not_downgraded_to_talk() {
        let manifest = include_str!("../../io.github.k33wee.clippy-land.json");
        assert!(manifest.contains("--own-name=io.github.k33wee.ClippyLand"));
        // A later talk policy for the same name overrides own in Flatpak.
        assert!(!manifest.contains("--talk-name=io.github.k33wee.ClippyLand"));
    }

    #[test]
    fn fallback_resolves_display_without_a_ui_socket() {
        assert_eq!(
            display_path(Some("wayland-1".into()), Some("/run/user/1000".into())).unwrap(),
            PathBuf::from("/run/user/1000/wayland-1")
        );
        assert_eq!(
            display_path(Some("/tmp/compositor.sock".into()), None).unwrap(),
            PathBuf::from("/tmp/compositor.sock")
        );
        assert!(display_path(None, Some("/run/user/1000".into())).is_err());
        assert!(display_path(Some("".into()), Some("/run/user/1000".into())).is_err());
        assert!(display_path(Some("wayland-1".into()), None).is_err());
        assert!(display_path(Some("wayland-1".into()), Some("relative".into())).is_err());
    }
}
