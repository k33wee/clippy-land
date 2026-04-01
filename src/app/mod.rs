mod handlers;
mod icons;
mod messages;
mod model;
mod view;

pub use messages::Message;
pub use model::AppModel;

use cosmic::iced::{Subscription, window::Id};
use cosmic::prelude::*;

pub(super) fn history_scroll_id() -> cosmic::iced_core::widget::Id {
    cosmic::iced_core::widget::Id::new("history-scroll")
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format
    const APP_ID: &'static str = "io.github.k33wee.clippy-land";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Debug: print startup info to stderr to help diagnose panel launch
        let pid = std::process::id();
        let args: Vec<String> = std::env::args().collect();
        let envs: Vec<(String, String)> = std::env::vars().collect();
        let filtered: Vec<(String, String)> = envs
            .iter()
            .filter(|(k, _)| k.starts_with("COSMIC") || k.contains("APPL") || k.contains("DBUS") || k.contains("XDG"))
            .cloned()
            .collect();
        eprintln!(
            "clippy-land init: pid={} args={:?} env-filter={:?}",
            pid,
            args,
            filtered
        );

        // Also write a small runtime file to help detect if the applet was launched by the panel
        let _ = (|| {
            let dir = std::env::var("XDG_RUNTIME_DIR").or_else(|_| std::env::var("TMPDIR")).unwrap_or_else(|_| String::from("/tmp"));
            let path = format!("{}/clippy-land-startup-{}.log", dir, pid);
            if let Ok(mut f) = std::fs::File::create(&path) {
                use std::io::Write;
                let _ = writeln!(f, "pid={}", pid);
                let _ = writeln!(f, "args={:?}", args);
                let _ = writeln!(f, "env-filter={:?}", filtered);
            }
        })();

        (
            AppModel {
                core,
                ..Default::default()
            },
            Task::none(),
        )
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// Describes the interface based on the current state of the application model
    fn view(&self) -> Element<'_, Self::Message> {
        view::view(self)
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        view::view_window(self, _id)
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        handlers::subscription(self)
    }

    /// Handles messages emitted by the application and its widgets
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        handlers::update(self, message)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
