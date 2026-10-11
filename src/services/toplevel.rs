//! Focused-toplevel tracking via `cosmic-toplevel-info-unstable-v1`.
//!
//! The IPC toggle opens the clipboard popup as a layer surface.  When the
//! currently focused toplevel is known, the popup can be placed next to where
//! the user is working instead of a full-width strip at the top of the screen.
//!
//! This module runs a dedicated Wayland connection on a background thread that
//! tracks the activated toplevel and its per-output geometry.  The thread is
//! purely event driven: it only wakes when a window is created, closed, moved,
//! resized, or focused, so the idle cost is zero.
//!
//! Not every environment allows this: cosmic-comp only exposes the
//! `zcosmic_toplevel_info_v1` global to non-sandboxed clients, so the Flatpak
//! build gracefully keeps the previous top-strip placement (the snapshot stays
//! `None` and callers fall back).
//!
//! Protocol XMLs are vendored under `protocols/` and scanned at compile time
//! with `wayland-scanner`.  The toplevel info protocol is bound as version 2:
//! that is the first version providing the `geometry` event, while avoiding
//! the `ext_workspace_enter` events introduced in version 3 (which reference
//! `ext_workspace_handle_v1` objects this app never needs).  The workspace
//! protocols are still scanned because version 2 sends the deprecated
//! `workspace_enter`/`workspace_leave` events, whose argument types must
//! therefore exist at compile time.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use wayland_backend::client::ObjectId;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};

use protocols::foreign_toplevel_list::client::{
    ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
};
use protocols::toplevel_info::client::{
    zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1,
    zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1,
};

/// Generated client bindings for the vendored protocol XMLs.
///
/// Each XML is scanned in its own module following the layout the
/// `wayland-protocols` crate uses, so the cross-interface references of
/// `cosmic-toplevel-info` resolve as sibling modules.
#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
#[allow(unused_imports)]
#[allow(unused_unsafe)]
#[allow(unused_variables)]
#[allow(missing_docs)]
#[allow(clippy::all)]
pub mod protocols {
    pub mod foreign_toplevel_list {
        pub mod client {
            use wayland_client;
            use wayland_client::protocol::*;

            pub mod __interfaces {
                use wayland_client::protocol::__interfaces::*;
                wayland_scanner::generate_interfaces!(
                    "./protocols/ext-foreign-toplevel-list-v1.xml"
                );
            }
            use self::__interfaces::*;

            wayland_scanner::generate_client_code!("./protocols/ext-foreign-toplevel-list-v1.xml");
        }
    }

    pub mod workspace {
        pub mod client {
            use bitflags;
            use wayland_client;
            use wayland_client::protocol::*;

            pub mod __interfaces {
                use wayland_client::protocol::__interfaces::*;
                wayland_scanner::generate_interfaces!(
                    "./protocols/cosmic-workspace-unstable-v1.xml"
                );
            }
            use self::__interfaces::*;

            wayland_scanner::generate_client_code!("./protocols/cosmic-workspace-unstable-v1.xml");
        }
    }

    pub mod ext_workspace {
        pub mod client {
            use wayland_client;
            use wayland_client::protocol::*;

            pub mod __interfaces {
                use wayland_client::protocol::__interfaces::*;
                wayland_scanner::generate_interfaces!("./protocols/ext-workspace-v1.xml");
            }
            use self::__interfaces::*;

            wayland_scanner::generate_client_code!("./protocols/ext-workspace-v1.xml");
        }
    }

    pub mod toplevel_info {
        pub mod client {
            use crate::services::toplevel::protocols::ext_workspace::client::*;
            use crate::services::toplevel::protocols::foreign_toplevel_list::client::*;
            use crate::services::toplevel::protocols::workspace::client::*;
            use wayland_client;
            use wayland_client::protocol::*;

            pub mod __interfaces {
                use crate::services::toplevel::protocols::ext_workspace::client::__interfaces::*;
                use crate::services::toplevel::protocols::foreign_toplevel_list::client::__interfaces::*;
                use crate::services::toplevel::protocols::workspace::client::__interfaces::*;
                use wayland_client::protocol::__interfaces::*;
                wayland_scanner::generate_interfaces!(
                    "./protocols/cosmic-toplevel-info-unstable-v1.xml"
                );
            }
            use self::__interfaces::*;

            wayland_scanner::generate_client_code!(
                "./protocols/cosmic-toplevel-info-unstable-v1.xml"
            );
        }
    }
}

/// Logical rectangle of a toplevel window, relative to the output it is
/// displayed on (as reported by the compositor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Latest knowledge about the focused toplevel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Snapshot {
    /// Geometry of the activated toplevel relative to the output where it
    /// covers the largest area, or `None` when no toplevel is focused.
    pub focused_window: Option<WindowRect>,
}

/// `zcosmic_toplevel_handle_v1.state` array value marking the activated
/// toplevel.
const STATE_ACTIVATED: u32 = 2;

/// Key kinds usable with [`TrackerCore`].
trait WindowKey: Clone + Eq + Hash {}

impl WindowKey for ObjectId {}
impl WindowKey for u64 {}

#[derive(Debug, Clone)]
struct WindowRecord<K: WindowKey> {
    activated: bool,
    rects: HashMap<K, WindowRect>,
}

impl<K: WindowKey> Default for WindowRecord<K> {
    fn default() -> Self {
        Self {
            activated: false,
            rects: HashMap::new(),
        }
    }
}

/// Pure reduction state for toplevel events, independent of Wayland types so
/// it can be unit tested.
#[derive(Debug)]
struct TrackerCore<K: WindowKey> {
    windows: HashMap<K, WindowRecord<K>>,
}

impl<K: WindowKey> Default for TrackerCore<K> {
    fn default() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }
}

impl<K: WindowKey> TrackerCore<K> {
    fn touch(&mut self, key: K) {
        self.windows.entry(key).or_default();
    }

    fn set_activated(&mut self, key: K, activated: bool) {
        self.windows.entry(key).or_default().activated = activated;
    }

    fn set_geometry(&mut self, key: K, output: K, rect: WindowRect) {
        self.windows
            .entry(key)
            .or_default()
            .rects
            .insert(output, rect);
    }

    fn output_left(&mut self, key: K, output: K) {
        if let Some(record) = self.windows.get_mut(&key) {
            record.rects.remove(&output);
        }
    }

    fn remove(&mut self, key: &K) {
        self.windows.remove(key);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            focused_window: self
                .windows
                .values()
                .filter(|record| record.activated)
                .filter_map(largest_rect)
                .max_by_key(|rect| (rect.width as i64) * (rect.height as i64)),
        }
    }
}

fn largest_rect<K: WindowKey>(record: &WindowRecord<K>) -> Option<WindowRect> {
    record
        .rects
        .values()
        .copied()
        .filter(|rect| rect.width > 0 && rect.height > 0)
        .max_by_key(|rect| (rect.width as i64) * (rect.height as i64))
}

struct TrackerState {
    core: TrackerCore<ObjectId>,
    shared: Arc<Mutex<Option<Snapshot>>>,
    info: Option<ZcosmicToplevelInfoV1>,
    foreign_handles: HashMap<ObjectId, ZcosmicToplevelHandleV1>,
    bound_output_names: std::collections::HashSet<u32>,
    _outputs: Vec<WlOutput>,
}

impl TrackerState {
    fn publish(&self) {
        if let Ok(mut guard) = self.shared.lock() {
            *guard = Some(self.core.snapshot());
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for TrackerState {
    fn event(
        state: &mut Self,
        proxy: &WlRegistry,
        event: <WlRegistry as Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wayland_client::protocol::wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == "wl_output" => {
                // Initial outputs are bound from GlobalList below; bind only
                // names not already handled there (typically hot-plug events).
                if state.bound_output_names.insert(name) {
                    let output: WlOutput = proxy.bind(name, version.min(2), qh, ());
                    state._outputs.push(output);
                }
            }
            wayland_client::protocol::wl_registry::Event::GlobalRemove { name } => {
                state.bound_output_names.remove(&name);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for TrackerState {
    fn event(
        _state: &mut Self,
        _proxy: &WlOutput,
        _event: <WlOutput as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for TrackerState {
    event_created_child!(TrackerState, ExtForeignToplevelListV1, [
        0 => (ExtForeignToplevelHandleV1, ()), // opcode 0: toplevel(new_id)
    ]);

    fn event(
        state: &mut Self,
        _proxy: &ExtForeignToplevelListV1,
        event: <ExtForeignToplevelListV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use protocols::foreign_toplevel_list::client::ext_foreign_toplevel_list_v1::Event;

        let Event::Toplevel { toplevel } = event else {
            return;
        };
        let Some(info) = &state.info else {
            return;
        };
        let handle: ZcosmicToplevelHandleV1 = info.get_cosmic_toplevel(&toplevel, qh, ());
        state.core.touch(handle.id());
        state.foreign_handles.insert(toplevel.id(), handle);
    }
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for TrackerState {
    fn event(
        state: &mut Self,
        proxy: &ExtForeignToplevelHandleV1,
        event: <ExtForeignToplevelHandleV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use protocols::foreign_toplevel_list::client::ext_foreign_toplevel_handle_v1::Event;

        if let Event::Closed = event {
            if let Some(cosmic_handle) = state.foreign_handles.remove(&proxy.id()) {
                state.core.remove(&cosmic_handle.id());
                cosmic_handle.destroy();
            }
            proxy.destroy();
            state.publish();
        }
    }
}

impl Dispatch<ZcosmicToplevelHandleV1, ()> for TrackerState {
    fn event(
        state: &mut Self,
        proxy: &ZcosmicToplevelHandleV1,
        event: <ZcosmicToplevelHandleV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use protocols::toplevel_info::client::zcosmic_toplevel_handle_v1::Event;

        match event {
            Event::State { state: states } => {
                let activated = state_array_contains(&states, STATE_ACTIVATED);
                state.core.set_activated(proxy.id(), activated);
                state.publish();
            }
            Event::Geometry {
                output,
                x,
                y,
                width,
                height,
            } => {
                let rect = WindowRect {
                    x,
                    y,
                    width,
                    height,
                };
                state.core.set_geometry(proxy.id(), output.id(), rect);
                state.publish();
            }
            Event::OutputLeave { output } => {
                state.core.output_left(proxy.id(), output.id());
                state.publish();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZcosmicToplevelInfoV1, ()> for TrackerState {
    fn event(
        _state: &mut Self,
        _proxy: &ZcosmicToplevelInfoV1,
        _event: <ZcosmicToplevelInfoV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

/// The `state` event packs a list of little-endian `u32` state values into a
/// raw Wayland array; decodes whether `needle` is present.
fn state_array_contains(states: &[u8], needle: u32) -> bool {
    let (chunks, _) = states.as_chunks::<4>();
    chunks
        .iter()
        .any(|chunk| u32::from_le_bytes(*chunk) == needle)
}

/// Shared handle onto the background toplevel tracker.
#[derive(Clone, Default)]
pub struct Tracker {
    snapshot: Arc<Mutex<Option<Snapshot>>>,
}

impl Tracker {
    /// Returns the latest snapshot, or `None` while the tracker has not yet
    /// processed any events (startup, unsupported compositor, sandboxed
    /// client).  Never blocks meaningfully.
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.snapshot.lock().ok().and_then(|guard| *guard)
    }

    /// Creates a tracker and starts its background listener thread.
    ///
    /// Failures (no Wayland socket, missing global) leave the snapshot `None`
    /// forever, which callers treat as "use the fallback placement".
    pub fn started() -> Self {
        let tracker = Self::default();
        let shared = Arc::clone(&tracker.snapshot);
        let spawn = std::thread::Builder::new()
            .name("clippy-land-toplevel".into())
            .spawn(move || {
                if let Err(err) = run(shared) {
                    eprintln!("clippy-land: toplevel tracking disabled ({err})");
                }
            });
        if spawn.is_err() {
            eprintln!("clippy-land: toplevel tracking disabled (thread spawn failed)");
        }
        tracker
    }
}

fn run(shared: Arc<Mutex<Option<Snapshot>>>) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|err| format!("wayland connect: {err}"))?;
    let (globals, mut queue) =
        registry_queue_init::<TrackerState>(&conn).map_err(|err| format!("registry: {err}"))?;
    let qh = queue.handle();

    let _list: ExtForeignToplevelListV1 = globals
        .bind(&qh, 1..=1, ())
        .map_err(|_| "ext_foreign_toplevel_list_v1 unavailable".to_string())?;
    let info: ZcosmicToplevelInfoV1 = globals.bind(&qh, 2..=2, ()).map_err(|_| {
        "zcosmic_toplevel_info_v1 unavailable (compositor without cosmic toplevel info or sandboxed client)"
            .to_string()
    })?;

    // Bind all current outputs: geometry events reference their wl_output
    // objects. Remember names so queued initial registry events don't bind
    // duplicates, while later hot-plug events are handled by Dispatch.
    let mut bound_output_names = std::collections::HashSet::new();
    let outputs: Vec<WlOutput> = globals.contents().with_list(|entries| {
        entries
            .iter()
            .filter(|global| global.interface == "wl_output")
            .map(|global| {
                bound_output_names.insert(global.name);
                globals
                    .registry()
                    .bind(global.name, global.version.min(2), &qh, ())
            })
            .collect()
    });

    let mut state = TrackerState {
        core: TrackerCore {
            windows: HashMap::new(),
        },
        shared,
        info: Some(info),
        foreign_handles: HashMap::new(),
        bound_output_names,
        _outputs: outputs,
    };
    // Publish an initial (empty) snapshot once all globals are bound.
    state.publish();

    loop {
        queue
            .blocking_dispatch(&mut state)
            .map_err(|err| format!("dispatch: {err}"))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> WindowRect {
        WindowRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn empty_core_reports_no_focus() {
        let core: TrackerCore<u64> = TrackerCore::default();
        assert_eq!(core.snapshot(), Snapshot::default());
    }

    #[test]
    fn activated_toplevel_with_geometry_is_reported() {
        let mut core: TrackerCore<u64> = TrackerCore::default();
        core.set_activated(1, true);
        core.set_geometry(1, 10, rect(120, 40, 800, 600));

        assert_eq!(
            core.snapshot().focused_window,
            Some(rect(120, 40, 800, 600))
        );
    }

    #[test]
    fn inactive_toplevel_is_ignored() {
        let mut core: TrackerCore<u64> = TrackerCore::default();
        core.set_activated(1, true);
        core.set_geometry(1, 10, rect(0, 0, 800, 600));
        core.set_activated(1, false);

        assert_eq!(core.snapshot().focused_window, None);
    }

    #[test]
    fn geometry_without_activation_is_ignored() {
        let mut core: TrackerCore<u64> = TrackerCore::default();
        core.set_geometry(1, 10, rect(0, 0, 800, 600));

        assert_eq!(core.snapshot().focused_window, None);
    }

    #[test]
    fn largest_output_rect_wins_for_multi_output_windows() {
        let mut core: TrackerCore<u64> = TrackerCore::default();
        core.set_activated(1, true);
        core.set_geometry(1, 10, rect(0, 0, 500, 400));
        core.set_geometry(1, 20, rect(0, 0, 1920, 1080));

        assert_eq!(core.snapshot().focused_window, Some(rect(0, 0, 1920, 1080)));
    }

    #[test]
    fn activated_toplevel_without_geometry_is_ignored() {
        let mut core: TrackerCore<u64> = TrackerCore::default();
        core.set_activated(1, true);

        assert_eq!(core.snapshot().focused_window, None);
    }

    #[test]
    fn closing_focused_toplevel_clears_snapshot() {
        let mut core: TrackerCore<u64> = TrackerCore::default();
        core.set_activated(1, true);
        core.set_geometry(1, 10, rect(0, 0, 800, 600));
        core.remove(&1);

        assert_eq!(core.snapshot().focused_window, None);
    }

    #[test]
    fn output_leave_drops_only_that_output_rect() {
        let mut core: TrackerCore<u64> = TrackerCore::default();
        core.set_activated(1, true);
        core.set_geometry(1, 10, rect(0, 0, 1920, 1080));
        core.set_geometry(1, 20, rect(0, 0, 500, 400));
        core.output_left(1, 10);

        assert_eq!(core.snapshot().focused_window, Some(rect(0, 0, 500, 400)));
    }

    #[test]
    fn degenerate_rects_are_never_reported() {
        let mut core: TrackerCore<u64> = TrackerCore::default();
        core.set_activated(1, true);
        core.set_geometry(1, 10, rect(0, 0, 0, 600));
        core.set_geometry(1, 20, rect(0, 0, 800, 0));

        assert_eq!(core.snapshot().focused_window, None);
    }

    #[test]
    fn tracker_snapshot_is_none_before_any_publish() {
        let tracker = Tracker::default();
        assert_eq!(tracker.snapshot(), None);
    }

    /// Live round-trip against the running compositor.
    ///
    /// Run with:
    /// `CLIPPY_LAND_LIVE_TOPLEVEL=1 cargo test live_tracker -- --ignored --nocapture`
    #[test]
    #[ignore = "set CLIPPY_LAND_LIVE_TOPLEVEL=1 to run against the live compositor"]
    fn live_tracker_reports_focused_window() {
        if std::env::var_os("CLIPPY_LAND_LIVE_TOPLEVEL").is_none() {
            return;
        }
        let tracker = Tracker::started();
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let snapshot = tracker
            .snapshot()
            .expect("tracker should publish a snapshot within 1.5s");
        println!("focused_window = {:?}", snapshot.focused_window);
        assert!(
            snapshot.focused_window.is_some(),
            "expected a focused toplevel while a test terminal holds focus"
        );
    }
}
