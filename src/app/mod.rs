mod handlers;
mod icons;
mod messages;
mod model;
mod view;

pub use messages::Message;
pub use model::AppModel;

use cosmic::iced::{window::Id, Subscription};
use cosmic::prelude::*;

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;

    type Flags = ();

    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format
    const APP_ID: &'static str = "com.keewee.CosmicAppletClippyLand";

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

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}
