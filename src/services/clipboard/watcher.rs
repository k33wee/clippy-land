//! Event driven Wayland clipboard watcher.
//!
//! `wl_clipboard_rs::paste::get_contents` is intentionally a short lived helper: every call
//! creates a new data-control connection and asks the current offer to send its data.  That is
//! the wrong primitive for a clipboard history, which only needs to read an offer once when the
//! selection changes.  This module keeps one data-control connection alive and receives the
//! selection event from it.

use super::image::{
    clipboard_entry_from_image_bytes, clipboard_entry_from_image_path, log_image_too_large,
};
use super::uri::parse_first_local_path_from_uri_list;
use super::{ClipboardEntry, debug_log, max_image_bytes};
use os_pipe::pipe;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use std::collections::HashMap;
use std::io::Read;
use std::os::fd::AsFd;
use std::time::{Duration, Instant};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, event_created_child};
use wayland_protocols::ext::data_control::v1::client as ext;
use wayland_protocols_wlr::data_control::v1::client as zwlr;

#[derive(Clone)]
enum Manager {
    Wlr(zwlr::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1),
    Ext(ext::ext_data_control_manager_v1::ExtDataControlManagerV1),
}

#[derive(Clone)]
enum Device {
    Wlr(zwlr::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1),
    Ext(ext::ext_data_control_device_v1::ExtDataControlDeviceV1),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Offer {
    Wlr(zwlr::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1),
    Ext(ext::ext_data_control_offer_v1::ExtDataControlOfferV1),
}

impl From<zwlr::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1> for Offer {
    fn from(value: zwlr::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1) -> Self {
        Self::Wlr(value)
    }
}

impl From<ext::ext_data_control_offer_v1::ExtDataControlOfferV1> for Offer {
    fn from(value: ext::ext_data_control_offer_v1::ExtDataControlOfferV1) -> Self {
        Self::Ext(value)
    }
}

impl Offer {
    fn destroy(&self) {
        match self {
            Self::Wlr(offer) => offer.destroy(),
            Self::Ext(offer) => offer.destroy(),
        }
    }

    fn receive(&self, mime: String, fd: std::os::fd::BorrowedFd<'_>) {
        match self {
            Self::Wlr(offer) => offer.receive(mime, fd),
            Self::Ext(offer) => offer.receive(mime, fd),
        }
    }
}

impl Manager {
    fn get_data_device<D>(
        &self,
        seat: &WlSeat,
        qh: &wayland_client::QueueHandle<D>,
        data: WlSeat,
    ) -> Device
    where
        D: Dispatch<zwlr::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1, WlSeat>
            + Dispatch<ext::ext_data_control_device_v1::ExtDataControlDeviceV1, WlSeat>
            + 'static,
    {
        match self {
            Self::Wlr(manager) => Device::Wlr(manager.get_data_device(seat, qh, data)),
            Self::Ext(manager) => Device::Ext(manager.get_data_device(seat, qh, data)),
        }
    }
}

impl Device {
    fn destroy(&self) {
        match self {
            Self::Wlr(device) => device.destroy(),
            Self::Ext(device) => device.destroy(),
        }
    }
}

#[derive(Default)]
struct SeatState {
    device: Option<Device>,
    selected: Option<Offer>,
}

struct State {
    manager: Manager,
    seats: HashMap<WlSeat, SeatState>,
    offers: HashMap<Offer, Vec<String>>,
    pending: Option<Option<Offer>>,
    current: Option<Offer>,
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: <WlRegistry as Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
        _event: <zwlr::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext::ext_data_control_manager_v1::ExtDataControlManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ext::ext_data_control_manager_v1::ExtDataControlManagerV1,
        _event: <ext::ext_data_control_manager_v1::ExtDataControlManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Name { .. } = event {
            // The name is not needed: this watcher observes the first seat, as the existing
            // wl-clipboard-rs API does for `Seat::Unspecified`.
            let _ = state.seats.get(seat);
        }
    }
}

macro_rules! impl_device_dispatch {
    ($iface:ty, $offer_iface:ty, $offer_opcode:path) => {
        impl Dispatch<$iface, WlSeat> for State {
            fn event(
                state: &mut Self,
                _proxy: &$iface,
                event: <$iface as Proxy>::Event,
                seat: &WlSeat,
                _conn: &Connection,
                _qh: &wayland_client::QueueHandle<Self>,
            ) {
                type Event = <$iface as Proxy>::Event;
                match event {
                    Event::DataOffer { id } => {
                        state.offers.insert(Offer::from(id), Vec::new());
                    }
                    Event::Selection { id } => {
                        let next = id.map(Offer::from);
                        if state.current == next {
                            return;
                        }
                        state.current = next.clone();
                        state.pending = Some(next);
                        if let Some(data) = state.seats.get_mut(seat) {
                            if let Some(old) = data.selected.take() {
                                if Some(&old) != state.current.as_ref() {
                                    old.destroy();
                                    state.offers.remove(&old);
                                } else {
                                    data.selected = Some(old);
                                }
                            }
                            data.selected = state.pending.clone().flatten();
                        }
                    }
                    Event::PrimarySelection { id: Some(id) } => {
                        let offer = Offer::from(id);
                        if Some(&offer) != state.current.as_ref() {
                            offer.destroy();
                            state.offers.remove(&offer);
                        }
                    }
                    Event::Finished => {
                        if let Some(data) = state.seats.get_mut(seat) {
                            if let Some(old) = data.selected.take() {
                                if Some(&old) != state.current.as_ref() {
                                    old.destroy();
                                    state.offers.remove(&old);
                                }
                            }
                            data.device = None;
                        }
                    }
                    _ => {}
                }
            }

            event_created_child!(State, $iface, [$offer_opcode => ($offer_iface, ())]);
        }
    };
}

impl_device_dispatch!(
    zwlr::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    zwlr::zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE
);
impl_device_dispatch!(
    ext::ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext::ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext::ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE
);

macro_rules! impl_offer_dispatch {
    ($iface:ty) => {
        impl Dispatch<$iface, ()> for State {
            fn event(
                state: &mut Self,
                offer: &$iface,
                event: <$iface as Proxy>::Event,
                _data: &(),
                _conn: &Connection,
                _qh: &wayland_client::QueueHandle<Self>,
            ) {
                type Event = <$iface as Proxy>::Event;
                if let Event::Offer { mime_type } = event {
                    if let Some(mimes) = state.offers.get_mut(&Offer::from(offer.clone())) {
                        mimes.push(mime_type);
                    }
                }
            }
        }
    };
}

impl_offer_dispatch!(zwlr::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1);
impl_offer_dispatch!(ext::ext_data_control_offer_v1::ExtDataControlOfferV1);

const TRANSFER_TIMEOUT: Duration = Duration::from_millis(1500);
const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_URI_LIST_BYTES: usize = 1024 * 1024;
const IMAGE_MIMES: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];

fn is_text_mime(mime: &str) -> bool {
    match mime {
        "TEXT" | "STRING" | "UTF8_STRING" => true,
        x if x.starts_with("text/") => true,
        x if x.contains("json")
            || x.ends_with("script")
            || x.ends_with("xml")
            || x.ends_with("yaml")
            || x.ends_with("csv")
            || x.ends_with("ini") =>
        {
            true
        }
        _ => false,
    }
}

fn ordered_text_mimes(mimes: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    const PREFERRED_TEXT: [&str; 3] = [
        "text/plain;charset=utf-8",
        "UTF8_STRING",
        "text/plain",
    ];
    for preferred in PREFERRED_TEXT {
        for mime in mimes {
            if mime.as_str() == preferred && !result.contains(mime) {
                result.push(mime.clone());
            }
        }
    }
    for mime in mimes {
        if mime.starts_with("text/plain") && !result.contains(mime) {
            result.push(mime.clone());
        }
    }
    for preferred in ["TEXT", "STRING"] {
        for mime in mimes {
            if mime.as_str() == preferred && !result.contains(mime) {
                result.push(mime.clone());
            }
        }
    }
    for mime in mimes {
        if mime.as_str() != "text/uri-list" && is_text_mime(mime) && !result.contains(mime) {
            result.push(mime.clone());
        }
    }
    for mime in mimes {
        if mime.as_str() == "text/uri-list" && !result.contains(mime) {
            result.push(mime.clone());
        }
    }
    result
}

#[cfg(test)]
fn choose_text_mime(mimes: &[String]) -> Option<String> {
    ordered_text_mimes(mimes).into_iter().next()
}

#[cfg(test)]
fn choose_image_mime(mimes: &[String]) -> Option<String> {
    for preferred in IMAGE_MIMES {
        if let Some(mime) = mimes.iter().find(|m| m.as_str() == preferred) {
            return Some(mime.clone());
        }
    }
    None
}

#[cfg(test)]
fn choose_mime(mimes: &[String]) -> Option<String> {
    choose_image_mime(mimes)
        .or_else(|| {
            mimes
                .iter()
                .find(|m| m.as_str() == "text/uri-list")
                .cloned()
        })
        .or_else(|| choose_text_mime(mimes))
}

fn read_pipe_bounded(
    reader: &mut os_pipe::PipeReader,
    max_bytes: usize,
    timeout: Duration,
    conn: Option<&Connection>,
    mut queue: Option<&mut EventQueue<State>>,
    mut state: Option<&mut State>,
    offer: Option<&Offer>,
) -> Option<Vec<u8>> {
    let flags = fcntl_getfl(&*reader).ok()?;
    let _ = fcntl_setfl(&*reader, flags | OFlags::NONBLOCK);

    let start = Instant::now();
    let mut bytes = Vec::new();

    loop {
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            debug_log("clipboard watcher: transfer timed out");
            return None;
        }
        let remaining = timeout - elapsed;
        let timeout_ms = remaining
            .as_millis()
            .min(i32::MAX as u128)
            .max(1) as i32;

        if let (Some(st), Some(offer)) = (state.as_deref(), offer)
            && st.current.as_ref() != Some(offer)
        {
            debug_log("clipboard watcher: transfer offer superseded");
            return None;
        }

        let mut guard = conn.and_then(|c| c.prepare_read());
        if guard.is_none() && conn.is_some() {
            if let (Some(ref mut q), Some(ref mut st)) = (queue.as_mut(), state.as_mut()) {
                if q.dispatch_pending(st).is_err() {
                    return None;
                }
                if let Some(offer) = offer
                    && st.current.as_ref() != Some(offer)
                {
                    debug_log("clipboard watcher: transfer offer superseded during dispatch");
                    return None;
                }
            }
            guard = conn.and_then(|c| c.prepare_read());
        }

        let pfd_pipe = PollFd::new(
            &*reader,
            PollFlags::IN | PollFlags::ERR | PollFlags::HUP,
        );

        let (poll_res, wayland_ready) = if let Some(ref g) = guard {
            let wayland_fd = g.connection_fd();
            let pfd_wayland = PollFd::new(
                &wayland_fd,
                PollFlags::IN | PollFlags::ERR,
            );
            let mut fds = [pfd_pipe, pfd_wayland];
            let res = poll(&mut fds, timeout_ms);
            let wayland_in = fds[1]
                .revents()
                .intersects(PollFlags::IN | PollFlags::ERR);
            (res, wayland_in)
        } else {
            let mut fds = [pfd_pipe];
            let res = poll(&mut fds, timeout_ms);
            (res, false)
        };

        match poll_res {
            Ok(0) => {
                debug_log("clipboard watcher: transfer poll timed out");
                return None;
            }
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return None,
            Ok(_) => {}
        }

        if wayland_ready {
            if let Some(g) = guard.take() {
                match g.read() {
                    Ok(_) => {
                        if let (Some(ref mut q), Some(ref mut st)) =
                            (queue.as_mut(), state.as_mut())
                            && q.dispatch_pending(st).is_err()
                        {
                            return None;
                        }
                    }
                    Err(wayland_client::backend::WaylandError::Io(e))
                        if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => return None,
                }
            }
            if let (Some(st), Some(offer)) = (state.as_deref(), offer)
                && st.current.as_ref() != Some(offer)
            {
                debug_log("clipboard watcher: transfer superseded by new selection");
                return None;
            }
        } else {
            drop(guard);
        }

        let mut chunk = [0u8; 16384];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => return Some(bytes),
                Ok(n) => {
                    if bytes.len() + n > max_bytes {
                        debug_log("clipboard watcher: transfer exceeded max bytes");
                        return None;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            }
        }
    }
}

fn read_offer(
    offer: &Offer,
    mimes: &[String],
    conn: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
) -> Option<ClipboardEntry> {
    for mime in IMAGE_MIMES {
        if !mimes.iter().any(|m| m.as_str() == mime) {
            continue;
        }
        if state.current.as_ref() != Some(offer) {
            return None;
        }
        let Ok((mut reader, writer)) = pipe() else {
            continue;
        };
        offer.receive(mime.to_string(), writer.as_fd());
        drop(writer);
        if conn.flush().is_err() {
            return None;
        }

        let max = max_image_bytes();
        if let Some(bytes) = read_pipe_bounded(
            &mut reader,
            max + 1,
            TRANSFER_TIMEOUT,
            Some(conn),
            Some(queue),
            Some(state),
            Some(offer),
        ) {
            if bytes.len() > max {
                log_image_too_large(bytes.len());
            } else if let Some(entry) = clipboard_entry_from_image_bytes(mime.to_string(), bytes) {
                return Some(entry);
            }
        }
    }

    if mimes.iter().any(|m| m.as_str() == "text/uri-list") {
        if state.current.as_ref() != Some(offer) {
            return None;
        }
        if let Ok((mut reader, writer)) = pipe() {
            offer.receive("text/uri-list".to_string(), writer.as_fd());
            drop(writer);
            if conn.flush().is_err() {
                return None;
            }

            if let Some(bytes) = read_pipe_bounded(
                &mut reader,
                MAX_URI_LIST_BYTES,
                TRANSFER_TIMEOUT,
                Some(conn),
                Some(queue),
                Some(state),
                Some(offer),
            )
                && let Ok(uris) = String::from_utf8(bytes)
                && let Some(path) = parse_first_local_path_from_uri_list(&uris)
                && let Some(entry) = clipboard_entry_from_image_path(&path)
            {
                return Some(entry);
            }
        }
    }

    for text_mime in ordered_text_mimes(mimes) {
        if state.current.as_ref() != Some(offer) {
            return None;
        }
        let Ok((mut reader, writer)) = pipe() else {
            continue;
        };
        offer.receive(text_mime.clone(), writer.as_fd());
        drop(writer);
        if conn.flush().is_err() {
            return None;
        }

        if let Some(bytes) = read_pipe_bounded(
            &mut reader,
            MAX_TEXT_BYTES,
            TRANSFER_TIMEOUT,
            Some(conn),
            Some(queue),
            Some(state),
            Some(offer),
        )
            && let Ok(text) = String::from_utf8(bytes)
        {
            let text = text.trim_end_matches(['\n', '\r']).to_string();
            if !text.is_empty() {
                return Some(ClipboardEntry::Text(text));
            }
        }
    }

    None
}

fn drain_wayland_events(
    conn: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
) -> Result<(), wayland_client::DispatchError> {
    loop {
        queue.dispatch_pending(state)?;

        let Some(guard) = conn.prepare_read() else {
            continue;
        };

        let fd = guard.connection_fd();
        let mut fds = [PollFd::new(
            &fd,
            PollFlags::IN | PollFlags::ERR,
        )];

        match poll(&mut fds, 0) {
            Ok(n) if n > 0 => match guard.read() {
                Ok(_) => continue,
                Err(wayland_client::backend::WaylandError::Io(e))
                    if e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    break;
                }
                Err(e) => return Err(e.into()),
            },
            _ => break,
        }
    }
    Ok(())
}

/// Runs until the subscription is dropped.  There is one Wayland connection for the lifetime of
/// this function; the current offer is read only after a `selection` event arrives.
pub fn run(sender: tokio::sync::mpsc::Sender<ClipboardEntry>) {
    let Ok(conn) = Connection::connect_to_env() else {
        debug_log("clipboard watcher: unable to connect to Wayland");
        return;
    };
    let Ok((globals, mut queue)) = registry_queue_init::<State>(&conn) else {
        debug_log("clipboard watcher: unable to initialise Wayland globals");
        return;
    };
    let qh = queue.handle();
    let ext_manager = globals
        .bind::<ext::ext_data_control_manager_v1::ExtDataControlManagerV1, _, _>(&qh, 1..=1, ())
        .ok()
        .map(Manager::Ext);
    let manager = ext_manager.or_else(|| {
        globals
            .bind::<zwlr::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, _, _>(
                &qh,
                1..=1,
                (),
            )
            .ok()
            .map(Manager::Wlr)
    });
    let Some(manager) = manager else {
        debug_log("clipboard watcher: data-control protocol unavailable");
        return;
    };

    let registry = globals.registry();
    #[allow(clippy::mutable_key_type)]
    let mut seats = HashMap::new();
    globals.contents().with_list(|list| {
        if let Some(global) = list
            .iter()
            .find(|g| g.interface == WlSeat::interface().name && g.version >= 2)
        {
            let seat = registry.bind(global.name, 2, &qh, ());
            seats.insert(seat, SeatState::default());
        }
    });
    if seats.is_empty() {
        debug_log("clipboard watcher: no Wayland seats");
        return;
    }

    #[allow(clippy::mutable_key_type)]
    let mut state = State {
        manager,
        seats,
        offers: HashMap::new(),
        pending: None,
        current: None,
    };
    let seat_list: Vec<WlSeat> = state.seats.keys().cloned().collect();
    for seat in seat_list {
        let device = state.manager.get_data_device(&seat, &qh, seat.clone());
        state.seats.get_mut(&seat).unwrap().device = Some(device);
    }
    if queue.roundtrip(&mut state).is_err() {
        return;
    }

    loop {
        if state.pending.is_none()
            && queue.blocking_dispatch(&mut state).is_err()
        {
            break;
        }
        if let Some(Some(offer)) = state.pending.take() {
            let mimes = state.offers.get(&offer).cloned().unwrap_or_default();
            if let Some(entry) = read_offer(&offer, &mimes, &conn, &mut queue, &mut state) {
                if drain_wayland_events(&conn, &mut queue, &mut state).is_err() {
                    break;
                }
                if state.current.as_ref() != Some(&offer) {
                    continue;
                }
                if sender.blocking_send(entry).is_err() {
                    break;
                }
            }
        }
    }

    for data in state.seats.values_mut() {
        if let Some(old) = data.selected.take() {
            old.destroy();
            state.offers.remove(&old);
        }
        if let Some(device) = data.device.take() {
            device.destroy();
        }
    }
    for (offer, _) in state.offers.drain() {
        offer.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_mime, choose_text_mime, is_text_mime, read_pipe_bounded};
    use os_pipe::pipe;
    use std::io::Write;
    use std::time::Duration;

    fn mimes(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn prefers_png_then_jpeg_before_text() {
        assert_eq!(
            choose_mime(&mimes(&["text/plain", "image/jpeg"])),
            Some("image/jpeg".into())
        );
        assert_eq!(
            choose_mime(&mimes(&["image/jpeg", "image/png"])),
            Some("image/png".into())
        );
    }

    #[test]
    fn falls_back_to_uri_list_and_plain_text() {
        assert_eq!(
            choose_mime(&mimes(&["text/plain", "text/uri-list"])),
            Some("text/uri-list".into())
        );
        assert_eq!(
            choose_mime(&mimes(&["text/plain;charset=utf-8"])),
            Some("text/plain;charset=utf-8".into())
        );
        assert_eq!(choose_mime(&mimes(&["application/octet-stream"])), None);
    }

    #[test]
    fn recognizes_extended_text_mimes() {
        assert!(is_text_mime("TEXT"));
        assert!(is_text_mime("STRING"));
        assert!(is_text_mime("UTF8_STRING"));
        assert!(is_text_mime("text/plain"));
        assert!(is_text_mime("text/html"));
        assert!(is_text_mime("application/json"));
        assert!(is_text_mime("application/xml"));
        assert!(is_text_mime("application/x-yaml"));
        assert!(is_text_mime("text/x-shellscript"));
        assert!(is_text_mime("text/csv"));
        assert!(is_text_mime("text/ini"));
        assert!(!is_text_mime("application/octet-stream"));
        assert!(!is_text_mime("image/png"));
    }

    #[test]
    fn chooses_extended_text_mimes_when_offered() {
        assert_eq!(
            choose_mime(&mimes(&["TEXT"])),
            Some("TEXT".into())
        );
        assert_eq!(
            choose_mime(&mimes(&["STRING"])),
            Some("STRING".into())
        );
        assert_eq!(
            choose_mime(&mimes(&["application/json"])),
            Some("application/json".into())
        );
        assert_eq!(
            choose_mime(&mimes(&["application/xml"])),
            Some("application/xml".into())
        );
        assert_eq!(
            choose_text_mime(&mimes(&["application/json", "text/plain"])),
            Some("text/plain".into())
        );
        assert_eq!(
            choose_text_mime(&mimes(&["text/plain", "UTF8_STRING"])),
            Some("UTF8_STRING".into())
        );
    }

    #[test]
    fn read_pipe_bounded_reads_data_within_limit() {
        let (mut reader, mut writer) = pipe().expect("pipe should create");
        writer.write_all(b"hello world").expect("write should succeed");
        drop(writer);

        let data = read_pipe_bounded(
            &mut reader,
            1024,
            Duration::from_millis(500),
            None,
            None,
            None,
            None,
        );
        assert_eq!(data, Some(b"hello world".to_vec()));
    }

    #[test]
    fn read_pipe_bounded_aborts_when_exceeding_max_bytes() {
        let (mut reader, mut writer) = pipe().expect("pipe should create");
        writer.write_all(b"0123456789extra").expect("write should succeed");
        drop(writer);

        let data = read_pipe_bounded(
            &mut reader,
            10,
            Duration::from_millis(500),
            None,
            None,
            None,
            None,
        );
        assert_eq!(data, None);
    }

    #[test]
    fn non_image_uri_and_invalid_image_fall_back_to_text_decoding() {
        use super::super::image::clipboard_entry_from_image_bytes;
        use super::super::uri::parse_first_local_path_from_uri_list;

        // Malformed image returns None
        assert!(clipboard_entry_from_image_bytes("image/png".into(), vec![0, 1, 2]).is_none());

        // Non-image URI returns None
        let parsed_path = parse_first_local_path_from_uri_list("file:///etc/hosts\n");
        assert!(parsed_path.is_some());
        assert!(super::super::image::clipboard_entry_from_image_path(&parsed_path.unwrap()).is_none());

        // Text parsing succeeds
        let text_bytes = b"fallback plain text content".to_vec();
        let text = String::from_utf8(text_bytes).ok().map(|t| t.trim_end_matches(['\n', '\r']).to_string());
        assert_eq!(text, Some("fallback plain text content".to_string()));
    }
}
