//! Clipboard ownership on the panel-granted connection. wl-clipboard-rs opens
//! its own display connections, which lose the grant inside a Flatpak sandbox.
//! Each source lives until cancelled; all its protocol objects are then released.

use bytes::Bytes;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::OwnedFd;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client as ext;
use wayland_protocols_wlr::data_control::v1::client as wlr;

const TEXT_MIMES: [&str; 3] = ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"];
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn copy_text(text: &str) -> bool {
    copy(
        TEXT_MIMES.iter().map(|mime| (*mime).into()).collect(),
        Bytes::copy_from_slice(text.as_bytes()),
    )
}

pub(super) fn copy_image(mime: String, bytes: Bytes) -> bool {
    copy(vec![mime], bytes)
}

fn copy(mimes: Vec<String>, bytes: Bytes) -> bool {
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let spawn = std::thread::Builder::new()
        .name("clippy-land-clipboard-source".into())
        .spawn(move || match Publisher::connect(mimes, bytes) {
            Ok(mut publisher) => {
                let result = publisher
                    .queue
                    .roundtrip(&mut publisher.state)
                    .map_err(|err| format!("selection roundtrip: {err}"))
                    .and_then(|_| {
                        if publisher.state.cancelled {
                            Err("selection superseded".into())
                        } else {
                            Ok(())
                        }
                    });
                let ready = result.is_ok();
                let _ = ready_tx.send(result);
                if ready {
                    while !publisher.state.cancelled {
                        if publisher
                            .queue
                            .blocking_dispatch(&mut publisher.state)
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                let _ = ready_tx.send(Err(err));
            }
        });
    if spawn.is_err() {
        super::debug_log("clipboard publisher thread could not start");
        return false;
    }
    match ready_rx.recv() {
        Ok(Ok(())) => true,
        result => {
            super::debug_log(format!("clipboard publisher setup failed: {result:?}"));
            false
        }
    }
}

enum Objects {
    Ext(
        ext::ext_data_control_manager_v1::ExtDataControlManagerV1,
        ext::ext_data_control_device_v1::ExtDataControlDeviceV1,
        ext::ext_data_control_source_v1::ExtDataControlSourceV1,
    ),
    Wlr(
        wlr::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
        wlr::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
        wlr::zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
    ),
}

enum Offer {
    Ext(ext::ext_data_control_offer_v1::ExtDataControlOfferV1),
    Wlr(wlr::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1),
}
impl Offer {
    fn id(&self) -> wayland_backend::client::ObjectId {
        match self {
            Self::Ext(offer) => offer.id(),
            Self::Wlr(offer) => offer.id(),
        }
    }

    fn destroy(self) {
        match self {
            Self::Ext(offer) => offer.destroy(),
            Self::Wlr(offer) => offer.destroy(),
        }
    }
}

struct State {
    bytes: Bytes,
    mimes: Vec<String>,
    offers: Vec<Offer>,
    cancelled: bool,
}

struct Publisher {
    conn: Connection,
    queue: EventQueue<State>,
    seat: WlSeat,
    objects: Objects,
    state: State,
}

impl Publisher {
    fn connect(mimes: Vec<String>, bytes: Bytes) -> Result<Self, String> {
        let conn = crate::services::wayland::connection()?;
        let (globals, queue) =
            registry_queue_init::<State>(&conn).map_err(|err| format!("registry: {err}"))?;
        let qh = queue.handle();
        // Privileged COSMIC connections support wl_seat.release (version 5).
        let seat: WlSeat = match globals.bind(&qh, 5..=7, ()) {
            Ok(seat) => seat,
            Err(err) => {
                globals.destroy();
                return Err(format!("wl_seat bind: {err}"));
            }
        };
        let objects = if let Ok(manager) = globals
            .bind::<ext::ext_data_control_manager_v1::ExtDataControlManagerV1, _, _>(
            &qh,
            1..=1,
            (),
        ) {
            let source = manager.create_data_source(&qh, ());
            let device = manager.get_data_device(&seat, &qh, ());
            for mime in &mimes {
                source.offer(mime.clone());
            }
            device.set_selection(Some(&source));
            Objects::Ext(manager, device, source)
        } else if let Ok(manager) = globals
            .bind::<wlr::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, _, _>(
            &qh,
            1..=1,
            (),
        ) {
            let source = manager.create_data_source(&qh, ());
            let device = manager.get_data_device(&seat, &qh, ());
            for mime in &mimes {
                source.offer(mime.clone());
            }
            device.set_selection(Some(&source));
            Objects::Wlr(manager, device, source)
        } else {
            seat.release();
            globals.destroy();
            let _ = conn.flush();
            return Err("clipboard data-control protocol unavailable".into());
        };
        // wl_fixes lets COSMIC destroy the otherwise destructor-less registry.
        globals.destroy();
        Ok(Self {
            conn,
            queue,
            seat,
            objects,
            state: State {
                bytes,
                mimes,
                offers: Vec::new(),
                cancelled: false,
            },
        })
    }
}

impl Drop for Publisher {
    fn drop(&mut self) {
        for offer in self.state.offers.drain(..) {
            offer.destroy();
        }
        match &self.objects {
            Objects::Ext(manager, device, source) => {
                source.destroy();
                device.destroy();
                manager.destroy();
            }
            Objects::Wlr(manager, device, source) => {
                source.destroy();
                device.destroy();
                manager.destroy();
            }
        }
        self.seat.release();
        let _ = self.conn.flush();
    }
}

impl State {
    fn release_offer(&mut self, id: wayland_backend::client::ObjectId) {
        if let Some(index) = self.offers.iter().position(|offer| offer.id() == id) {
            self.offers.swap_remove(index).destroy();
        }
    }

    fn send(&self, mime: &str, fd: OwnedFd) {
        if !self.mimes.iter().any(|offered| offered == mime) {
            return;
        }
        let bytes = self.bytes.clone();
        // Never block Wayland dispatch on a recipient that stops reading.
        let _ = std::thread::Builder::new()
            .name("clippy-land-clipboard-send".into())
            .spawn(move || {
                if let Err(err) = write_data(File::from(fd), &bytes, TRANSFER_TIMEOUT) {
                    super::debug_log(format!("clipboard transfer failed: {err}"));
                }
            });
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
wayland_client::delegate_noop!(State: ignore WlSeat);
wayland_client::delegate_noop!(State: ignore ext::ext_data_control_manager_v1::ExtDataControlManagerV1);
wayland_client::delegate_noop!(State: ignore wlr::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1);
wayland_client::delegate_noop!(State: ignore ext::ext_data_control_offer_v1::ExtDataControlOfferV1);
wayland_client::delegate_noop!(State: ignore wlr::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1);

macro_rules! source_dispatch {
    ($iface:ty) => {
        impl Dispatch<$iface, ()> for State {
            fn event(
                state: &mut Self,
                _: &$iface,
                event: <$iface as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                type Event = <$iface as Proxy>::Event;
                match event {
                    Event::Send { mime_type, fd } => state.send(&mime_type, fd),
                    Event::Cancelled => state.cancelled = true,
                    _ => {}
                }
            }
        }
    };
}
source_dispatch!(ext::ext_data_control_source_v1::ExtDataControlSourceV1);
source_dispatch!(wlr::zwlr_data_control_source_v1::ZwlrDataControlSourceV1);

macro_rules! device_dispatch {
    ($iface:ty, $offer:ty, $variant:ident, $opcode:path) => {
        impl Dispatch<$iface, ()> for State {
            fn event(state: &mut Self, _: &$iface, event: <$iface as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                type Event = <$iface as Proxy>::Event;
                match event {
                    Event::DataOffer { id } => state.offers.push(Offer::$variant(id)),
                    Event::Selection { id: Some(id) } | Event::PrimarySelection { id: Some(id) } => state.release_offer(id.id()),
                    Event::Finished => state.cancelled = true,
                    _ => {}
                }
            }
            event_created_child!(State, $iface, [$opcode => ($offer, ())]);
        }
    }
}
device_dispatch!(
    ext::ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext::ext_data_control_offer_v1::ExtDataControlOfferV1,
    Ext,
    ext::ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE
);
device_dispatch!(
    wlr::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    wlr::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    Wlr,
    wlr::zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE
);

fn write_data(mut file: File, mut bytes: &[u8], timeout: Duration) -> io::Result<()> {
    fcntl_setfl(&file, fcntl_getfl(&file)? | OFlags::NONBLOCK)?;
    let deadline = Instant::now() + timeout;
    while !bytes.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match file.write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                let mut fds = [PollFd::new(&file, PollFlags::OUT)];
                let timespec = Timespec {
                    tv_sec: remaining.as_secs() as i64,
                    tv_nsec: remaining.subsec_nanos() as i64,
                };
                match poll(&mut fds, Some(&timespec)) {
                    Ok(0) => return Err(io::ErrorKind::TimedOut.into()),
                    Err(rustix::io::Errno::INTR) => continue,
                    Err(err) => return Err(err.into()),
                    Ok(_) => {}
                }
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::fd::OwnedFd;

    #[test]
    fn pipe_transfer_preserves_large_binary_payload() {
        let (mut reader, writer) = os_pipe::pipe().unwrap();
        let payload: Vec<u8> = (0..1024 * 1024).map(|n| (n % 251) as u8).collect();
        let expected = payload.clone();
        let transfer = std::thread::spawn(move || {
            write_data(
                File::from(OwnedFd::from(writer)),
                &payload,
                Duration::from_secs(2),
            )
        });
        let mut actual = Vec::new();
        reader.read_to_end(&mut actual).unwrap();
        transfer.join().unwrap().unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn stalled_recipient_is_bounded_by_timeout() {
        let (_reader, writer) = os_pipe::pipe().unwrap();
        let err = write_data(
            File::from(OwnedFd::from(writer)),
            &vec![0; 1024 * 1024],
            Duration::from_millis(10),
        )
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn disconnected_recipient_returns_broken_pipe() {
        let (reader, writer) = os_pipe::pipe().unwrap();
        drop(reader);
        let err = write_data(
            File::from(OwnedFd::from(writer)),
            b"test",
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }
}
