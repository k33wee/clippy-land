//! Synthesized paste keystrokes via the `zwp_virtual_keyboard_v1` protocol.
//!
//! While the clipboard popup is open it holds keyboard focus.  When an entry is
//! activated the popup is destroyed first, the compositor restores focus to the
//! previously active window, and only then is a Ctrl+V keystroke injected
//! through a short-lived virtual keyboard so the freshly written clipboard
//! contents land in that window.
//!
//! Not every environment allows this: the compositor must advertise the
//! `zwp_virtual_keyboard_manager_v1` global.  In particular cosmic-comp only
//! exposes it to non-sandboxed clients, so the Flatpak build gracefully
//! degrades to copy-only (the error is logged and ignored by callers).

use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use std::time::{Duration, Instant};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

/// Linux evdev keycode for `KEY_V`.
const KEY_V: u32 = 47;
/// `wl_keyboard` key state: released.
const KEY_STATE_RELEASED: u32 = 0;
/// `wl_keyboard` key state: pressed.
const KEY_STATE_PRESSED: u32 = 1;
/// `wl_keyboard` keymap format enum value for `xkb_keymap_format_text_v1`.
const KEYMAP_FORMAT_XKB_V1: u32 = 1;

/// How long a synthesized key press is reported as held (timestamps only).
const KEY_HOLD_MS: u32 = 40;

/// Delay between an action being triggered and the Ctrl+V injection, giving
/// the compositor time to destroy the popup and restore keyboard focus to the
/// previously active window.
const DEFAULT_PASTE_DELAY_MS: u64 = 250;

/// Upper bound for the `CLIPPY_LAND_PASTE_DELAY_MS` debug override.
const MAX_PASTE_DELAY_MS: u64 = 5_000;

static PROCESS_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Monotonic millisecond timestamp suitable for `wl_keyboard.key` events.
fn monotonic_ms() -> u32 {
    let base = PROCESS_START.get_or_init(Instant::now);
    base.elapsed().as_millis() as u32
}

/// How long to wait after closing the popup before injecting Ctrl+V.
///
/// `CLIPPY_LAND_PASTE_DELAY_MS` overrides the default for debugging.
pub fn paste_delay() -> Duration {
    std::env::var("CLIPPY_LAND_PASTE_DELAY_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|&ms| ms <= MAX_PASTE_DELAY_MS)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(DEFAULT_PASTE_DELAY_MS))
}

/// Injects Ctrl+V into the currently focused window. Blocking because it uses
/// Wayland round-trips; intended for `spawn_blocking`.
pub fn send_ctrl_v() -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|err| format!("wayland connect: {err}"))?;
    let (globals, mut queue) =
        registry_queue_init::<KeyboardState>(&conn).map_err(|err| format!("registry: {err}"))?;
    let qh = queue.handle();

    let manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ()).map_err(|_| {
        "zwp_virtual_keyboard_manager_v1 unavailable (unsupported compositor or sandboxed client)"
            .to_string()
    })?;

    let seat: WlSeat = globals
        .bind(&qh, 1..=7, ())
        .map_err(|err| format!("wl_seat bind: {err}"))?;

    let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());

    let (keymap_text, ctrl_mask) = compile_keymap_and_ctrl_mask()?;
    let mut keymap_bytes = keymap_text.into_bytes();
    // XKB text keymaps transported over Wayland are NUL-terminated and the
    // terminator is included in the advertised size.
    keymap_bytes.push(0);

    // The compositor mmaps the keymap from the passed fd, so a memfd avoids
    // touching the filesystem. Keep the file alive until the connection is
    // flushed below.
    let keymap_file = create_memfd_with(&keymap_bytes)?;
    keyboard.keymap(
        KEYMAP_FORMAT_XKB_V1,
        keymap_file.as_fd(),
        keymap_bytes.len() as u32,
    );

    let mut state = KeyboardState;
    queue
        .roundtrip(&mut state)
        .map_err(|err| format!("keymap roundtrip: {err}"))?;

    let press_time = monotonic_ms();
    keyboard.modifiers(ctrl_mask, 0, 0, 0);
    keyboard.key(press_time, KEY_V, KEY_STATE_PRESSED);
    queue
        .roundtrip(&mut state)
        .map_err(|err| format!("key press roundtrip: {err}"))?;

    std::thread::sleep(Duration::from_millis(2));

    keyboard.key(press_time + KEY_HOLD_MS, KEY_V, KEY_STATE_RELEASED);
    keyboard.modifiers(0, 0, 0, 0);
    queue
        .roundtrip(&mut state)
        .map_err(|err| format!("key release roundtrip: {err}"))?;

    keyboard.destroy();
    conn.flush()
        .map_err(|err| format!("wayland flush: {err}"))?;

    Ok(())
}

/// Sleeps the paste delay, then injects Ctrl+V.  Async counterpart used by the
/// copy handler so the popup has already been closed by the time this runs.
pub async fn delayed_ctrl_v() {
    tokio::time::sleep(paste_delay()).await;
    let result = tokio::task::spawn_blocking(send_ctrl_v)
        .await
        .unwrap_or_else(|err| Err(format!("virtual keyboard task failed: {err}")));
    if let Err(err) = result {
        log_paste_failure(&err);
    }
}

fn log_paste_failure(reason: &str) {
    if std::env::var_os("CLIPPY_LAND_DEBUG_CLIPBOARD").is_some() {
        eprintln!("[clippy-land] paste not synthesized: {reason}");
    }
}

/// Compiles the default xkb keymap (honouring `XKB_DEFAULT_*` environment
/// variables) and returns its serialized text form plus the modifier-state bit
/// for Control in that keymap.
fn compile_keymap_and_ctrl_mask() -> Result<(String, u32), String> {
    let context = xkbcommon::xkb::Context::new(xkbcommon::xkb::CONTEXT_NO_FLAGS);
    let keymap = xkbcommon::xkb::Keymap::new_from_names(
        &context,
        "",
        "",
        "",
        "",
        None,
        xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .ok_or_else(|| "failed to compile xkb keymap".to_string())?;

    let ctrl_index = keymap.mod_get_index(xkbcommon::xkb::MOD_NAME_CTRL);
    if ctrl_index >= u32::BITS {
        return Err("xkb keymap has no Control modifier".to_string());
    }
    let ctrl_mask = 1u32 << ctrl_index;

    let text = keymap.get_as_string(xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1);
    Ok((text, ctrl_mask))
}

fn create_memfd_with(contents: &[u8]) -> Result<File, String> {
    let fd = rustix::fs::memfd_create("clippy-land-keymap", rustix::fs::MemfdFlags::CLOEXEC)
        .map_err(|err| format!("memfd_create: {err}"))?;

    let mut file = File::from(fd);
    file.write_all(contents)
        .map_err(|err| format!("keymap write: {err}"))?;
    file.flush().map_err(|err| format!("keymap flush: {err}"))?;
    Ok(file)
}

#[derive(Default)]
struct KeyboardState;

impl Dispatch<WlRegistry, GlobalListContents> for KeyboardState {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: <WlRegistry as wayland_client::Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for KeyboardState {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        _event: <WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for KeyboardState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardManagerV1,
        _event: <ZwpVirtualKeyboardManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for KeyboardState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardV1,
        _event: <ZwpVirtualKeyboardV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn default_keymap_compiles_and_exposes_control_modifier() {
        let (text, ctrl_mask) = compile_keymap_and_ctrl_mask().expect("keymap should compile");

        assert!(text.contains("xkb_keymap"));
        // Standard keymaps declare modifiers as Shift, Lock, Control, Mod1..5,
        // so Control should sit on the third bit.
        assert_eq!(ctrl_mask, 1 << 2);
    }

    #[test]
    fn memfd_receives_keymap_contents() {
        let file = create_memfd_with(b"xkb_keymap { }").expect("memfd should be writable");

        use std::os::fd::AsRawFd;
        let mut reopened = File::open(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .expect("memfd should be re-openable via /proc");
        let mut contents = String::new();
        reopened
            .read_to_string(&mut contents)
            .expect("memfd contents should be readable");
        assert_eq!(contents, "xkb_keymap { }");
    }

    #[test]
    fn paste_delay_defaults_and_honors_override() {
        let previous = std::env::var_os("CLIPPY_LAND_PASTE_DELAY_MS");
        unsafe { std::env::remove_var("CLIPPY_LAND_PASTE_DELAY_MS") };
        assert_eq!(paste_delay(), Duration::from_millis(DEFAULT_PASTE_DELAY_MS));

        // The override is read per call, so a stale value is not cached.
        unsafe { std::env::set_var("CLIPPY_LAND_PASTE_DELAY_MS", "1000") };
        assert_eq!(paste_delay(), Duration::from_millis(1000));

        unsafe { std::env::set_var("CLIPPY_LAND_PASTE_DELAY_MS", "999999999") };
        assert_eq!(paste_delay(), Duration::from_millis(DEFAULT_PASTE_DELAY_MS));

        match previous {
            Some(value) => unsafe { std::env::set_var("CLIPPY_LAND_PASTE_DELAY_MS", value) },
            None => unsafe { std::env::remove_var("CLIPPY_LAND_PASTE_DELAY_MS") },
        }
    }
}
