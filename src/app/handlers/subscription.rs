use crate::app::{AppModel, Message};
use crate::ipc;
use crate::services::clipboard;
use cosmic::iced::Subscription;
use cosmic::iced::futures::channel::mpsc;
use futures_util::SinkExt;

use cosmic::iced::core::keyboard::key::Named as NamedKey;

pub(super) fn subscription(app: &AppModel) -> Subscription<Message> {
    struct ClipboardSubscription;

    let mut subs: Vec<Subscription<Message>> = vec![
        Subscription::run_with(std::any::TypeId::of::<ClipboardSubscription>(), |_| {
            cosmic::iced::stream::channel(1, move |mut channel: mpsc::Sender<Message>| async move {
                let (clipboard_tx, mut clipboard_rx) = tokio::sync::mpsc::channel(1);
                std::thread::Builder::new()
                    .name("clipboard-watcher".to_string())
                    .spawn(move || clipboard::watch_clipboard(clipboard_tx))
                    .expect("clipboard watcher thread should start");
                let mut last_seen: Option<clipboard::ClipboardFingerprint> = None;

                while let Some(next) = clipboard_rx.recv().await {
                    let next_fp = next.fingerprint();
                    if last_seen.as_ref() == Some(&next_fp) {
                        continue;
                    }

                    last_seen = Some(next_fp);

                    if channel.send(Message::ClipboardChanged(next)).await.is_err() {
                        break;
                    }
                }
            })
        }),
        ipc::toggle_watcher(),
    ];

    if app.popup.is_some() {
        use cosmic::iced::core::keyboard;
        use cosmic::iced::event::{listen_raw, listen_with};
        use cosmic::iced::{Event, event};

        let unfocus_sub =
            listen_with(|event, _status, window_id| message_for_focus_event(&event, window_id));
        subs.push(unfocus_sub);

        if app.popup_open_trace_pending() {
            let popup_lifecycle_sub = listen_with(|event, _status, window_id| match event {
                Event::Window(cosmic::iced::window::Event::Opened { .. }) => {
                    Some(Message::PopupOpened(window_id))
                }
                Event::Window(cosmic::iced::window::Event::RedrawRequested(_)) => {
                    Some(Message::PopupRedraw(window_id))
                }
                _ => None,
            });
            subs.push(popup_lifecycle_sub);
        }

        let key_sub = listen_raw(move |event, status, _| {
            if event::Status::Ignored != status {
                return None;
            }

            match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(named),
                    ..
                }) => return message_for_named_key(named),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character(c),
                    physical_key,
                    ..
                }) => {
                    let key_obj = keyboard::Key::Character(c.clone());
                    if let Some(ch) = key_obj.to_latin(physical_key) {
                        return message_for_latin_key(ch);
                    }
                }
                _ => (),
            }

            None
        });
        subs.push(key_sub);
    }

    Subscription::batch(subs)
}

fn message_for_focus_event(
    event: &cosmic::iced::Event,
    window_id: cosmic::iced::window::Id,
) -> Option<Message> {
    use cosmic::iced::Event;
    use cosmic::iced::core::event::{PlatformSpecific, wayland};

    match event {
        Event::Window(cosmic::iced::window::Event::Unfocused) => {
            Some(Message::WindowUnfocused(window_id))
        }
        // Layer surfaces do not emit Window::Unfocused. Use their own ID,
        // rather than assuming the subscription's window ID is the surface.
        Event::PlatformSpecific(PlatformSpecific::Wayland(wayland::Event::Layer(
            wayland::LayerEvent::Unfocused,
            _,
            id,
        ))) => Some(Message::WindowUnfocused(*id)),
        _ => None,
    }
}

pub(super) fn message_for_named_key(named: NamedKey) -> Option<Message> {
    match named {
        NamedKey::ArrowUp => Some(Message::KeyboardNavigateUp),
        NamedKey::ArrowDown => Some(Message::KeyboardNavigateDown),
        NamedKey::ArrowLeft => Some(Message::MoveFocusLeft),
        NamedKey::ArrowRight => Some(Message::MoveFocusRight),
        NamedKey::Enter => Some(Message::ActivateSelection),
        NamedKey::Escape => Some(Message::EscapePressed),
        _ => None,
    }
}

pub(super) fn message_for_latin_key(ch: char) -> Option<Message> {
    match ch {
        'j' | 'J' => Some(Message::KeyboardNavigateDown),
        'k' | 'K' => Some(Message::KeyboardNavigateUp),
        'q' | 'Q' => Some(Message::ClosePreview),
        'h' | 'H' => Some(Message::MoveFocusLeft),
        'l' | 'L' => Some(Message::MoveFocusRight),
        '\n' | '\r' => Some(Message::ActivateSelection),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::core::event::{PlatformSpecific, wayland};
    use cosmic::iced::{Event, window::Id};
    use wayland_client::protocol::wl_surface::WlSurface;
    use wayland_client::{Connection, Proxy};

    #[test]
    fn layer_focus_loss_closes_toggle_popup_using_the_surface_id() {
        let (socket, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
        let conn = Connection::from_socket(socket).unwrap();
        let surface = WlSurface::inert(conn.backend().downgrade());
        let mut app = AppModel::default();
        let _ = crate::app::handlers::update(&mut app, Message::ToggleViaIpc);
        let id = app.popup.unwrap();
        app.search_query = "search".into();
        app.settings_open = true;
        let other_id = Id::unique();

        let focused = Event::PlatformSpecific(PlatformSpecific::Wayland(wayland::Event::Layer(
            wayland::LayerEvent::Focused,
            surface.clone(),
            id,
        )));
        assert!(message_for_focus_event(&focused, other_id).is_none());

        let unfocused = Event::PlatformSpecific(PlatformSpecific::Wayland(wayland::Event::Layer(
            wayland::LayerEvent::Unfocused,
            surface,
            id,
        )));
        let message = message_for_focus_event(&unfocused, other_id).unwrap();
        assert!(matches!(message, Message::WindowUnfocused(closed) if closed == id));
        let _ = crate::app::handlers::update(&mut app, message);
        assert!(app.popup.is_none());
        assert!(app.search_query.is_empty());
        assert!(!app.settings_open);
        assert!(app.pending_paste.is_none());
    }

    #[test]
    fn ordinary_window_unfocus_still_maps_to_its_window_id() {
        let id = Id::unique();
        assert!(matches!(
            message_for_focus_event(&Event::Window(cosmic::iced::window::Event::Unfocused), id),
            Some(Message::WindowUnfocused(closed)) if closed == id
        ));
        assert!(
            message_for_focus_event(&Event::Window(cosmic::iced::window::Event::Focused), id)
                .is_none()
        );
    }
}
