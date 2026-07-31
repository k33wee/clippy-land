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
        ipc::signal_file_watcher(),
    ];

    if app.popup.is_some() {
        use cosmic::iced::core::keyboard;
        use cosmic::iced::event::{listen_raw, listen_with};
        use cosmic::iced::{Event, event};

        let unfocus_sub = listen_with(|event, _status, window_id| {
            if let Event::Window(cosmic::iced::window::Event::Unfocused) = event {
                Some(Message::WindowUnfocused(window_id))
            } else {
                None
            }
        });
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
        'q' | 'Q' => Some(Message::CloseTextOverlay),
        'h' | 'H' => Some(Message::MoveFocusLeft),
        'l' | 'L' => Some(Message::MoveFocusRight),
        '\n' | '\r' => Some(Message::ActivateSelection),
        _ => None,
    }
}
