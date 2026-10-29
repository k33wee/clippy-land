mod clipboard;
mod navigation;
mod popup;
mod preview;
mod settings;
mod shared;

use crate::app::{AppModel, Message};
use cosmic::prelude::*;

pub(super) fn update(app: &mut AppModel, message: Message) -> Task<cosmic::Action<Message>> {
    if let Message::OpenAboutLink(url) = message {
        return Task::perform(
            async move {
                match tokio::process::Command::new("xdg-open")
                    .arg(url)
                    .status()
                    .await
                {
                    Ok(status) if status.success() => {}
                    result => eprintln!("Could not open About link: {result:?}"),
                }
            },
            |_| cosmic::Action::None,
        );
    }
    if matches!(
        &message,
        Message::OpenPreview(_) | Message::ClosePreview | Message::ImagePreviewReady { .. }
    ) {
        return preview::handle(app, message).unwrap_or_else(Task::none);
    }

    // Clipboard and thumbnail messages own large buffers and are always consumed here.
    if matches!(
        &message,
        Message::ClipboardChanged(_)
            | Message::ThumbnailDecodeReady { .. }
            | Message::ThumbnailReady { .. }
    ) {
        return clipboard::handle(app, message).unwrap_or_else(Task::none);
    }

    if let Some(task) = clipboard::handle(app, message.clone()) {
        return task;
    }

    if let Some(task) = navigation::handle(app, message.clone()) {
        return task;
    }

    if settings::handle(app, message.clone()) {
        return Task::none();
    }

    if let Some(task) = popup::handle(app, message.clone()) {
        return task;
    }

    if matches!(message, Message::EscapePressed) {
        return popup::handle(app, Message::TogglePopup).unwrap_or_else(Task::none);
    }

    Task::none()
}

pub(super) fn warm_thumbnail_handles(app: &mut AppModel) {
    popup::warm_for_first_popup(app);
}
