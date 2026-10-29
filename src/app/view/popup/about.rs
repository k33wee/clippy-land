use crate::app::{Message, icons};
use crate::fl;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;

pub(super) fn about_panel() -> Element<'static, Message> {
    let identity = widget::column::Column::new()
        .spacing(8)
        .align_x(Alignment::Center)
        .push(widget::icon(icons::panel_icon()).size(48))
        .push(widget::text::title3("Clippy Land"))
        .push(widget::text::caption(fl!(
            "about-caption",
            version = env!("CARGO_PKG_VERSION")
        )))
        .push(widget::text::body(fl!("about-description")).align_x(Alignment::Center));

    widget::column::Column::new()
        .spacing(12)
        .width(Length::Fill)
        .push(widget::text::title4(fl!("about")))
        .push(
            widget::container(identity)
                .width(Length::Fill)
                .center_x(Length::Fill),
        )
        .push(widget::divider::horizontal::light())
        .push(
            widget::button::link(fl!("about-source")).on_press(Message::OpenAboutLink(
                "https://github.com/k33wee/clippy-land",
            )),
        )
        .push(
            widget::button::link(fl!("about-issues")).on_press(Message::OpenAboutLink(
                "https://github.com/k33wee/clippy-land/issues",
            )),
        )
        .push(
            widget::button::link("GPL-3.0-or-later").on_press(Message::OpenAboutLink(
                "https://www.gnu.org/licenses/gpl-3.0.html",
            )),
        )
        .into()
}
