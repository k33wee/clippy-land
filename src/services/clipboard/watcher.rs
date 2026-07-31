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
use std::collections::HashMap;
use std::io::Read;
use std::os::fd::AsFd;
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
            fn event(state: &mut Self, _proxy: &$iface, event: <$iface as Proxy>::Event, seat: &WlSeat, _conn: &Connection, _qh: &wayland_client::QueueHandle<Self>) {
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
                                if Some(old.clone()) != state.current {
                                    old.destroy();
                                    state.offers.remove(&old);
                                } else {
                                    data.selected = Some(old);
                                }
                            }
                            data.selected = state.pending.clone().flatten();
                        }
                    }
                    Event::Finished => {
                        if let Some(data) = state.seats.get_mut(seat) {
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

fn choose_mime(mimes: &[String]) -> Option<String> {
    const PREFERRED: [&str; 6] = [
        "image/png",
        "image/jpeg",
        "image/webp",
        "text/uri-list",
        "text/plain;charset=utf-8",
        "UTF8_STRING",
    ];
    PREFERRED
        .iter()
        .find_map(|preferred| {
            mimes
                .iter()
                .find(|mime| mime.as_str() == *preferred)
                .cloned()
        })
        .or_else(|| {
            mimes
                .iter()
                .find(|mime| mime.starts_with("text/plain"))
                .cloned()
        })
}

fn read_offer(offer: &Offer, mime: String, queue: &EventQueue<State>) -> Option<ClipboardEntry> {
    let (mut reader, writer) = pipe().ok()?;
    offer.receive(mime.clone(), writer.as_fd());
    drop(writer);
    queue.flush().ok()?;

    let mut bytes = Vec::new();
    if mime.starts_with("image/") {
        let max = max_image_bytes();
        if reader
            .take((max + 1) as u64)
            .read_to_end(&mut bytes)
            .is_err()
        {
            return None;
        }
        if bytes.len() > max {
            log_image_too_large(bytes.len());
            return None;
        }
        return clipboard_entry_from_image_bytes(mime, bytes);
    }

    if reader.read_to_end(&mut bytes).is_err() {
        return None;
    }
    if mime == "text/uri-list" {
        let uris = String::from_utf8(bytes).ok()?;
        let path = parse_first_local_path_from_uri_list(&uris)?;
        return clipboard_entry_from_image_path(&path);
    }
    let text = String::from_utf8(bytes)
        .ok()?
        .trim_end_matches(['\n', '\r'])
        .to_string();
    (!text.is_empty()).then_some(ClipboardEntry::Text(text))
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
        if let Some(Some(offer)) = state.pending.take()
            && let Some(mime) = state
                .offers
                .get(&offer)
                .and_then(|mimes| choose_mime(mimes))
            && let Some(entry) = read_offer(&offer, mime, &queue)
        {
            // A new selection can arrive while the owner is producing a large
            // image.  Dispatch already-buffered events before publishing the old
            // bytes, and discard them if the offer is no longer current.
            if queue.dispatch_pending(&mut state).is_err() {
                break;
            }
            if state.current.as_ref() != Some(&offer) {
                continue;
            }
            if sender.blocking_send(entry).is_err() {
                break;
            }
        }
        if queue.blocking_dispatch(&mut state).is_err() {
            break;
        }
    }

    for data in state.seats.values() {
        if let Some(device) = &data.device {
            device.destroy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::choose_mime;

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
}
