use super::super::style::{container_on_svg_style, transparent_icon_button_style};
use crate::app::{Message, icons};
use cosmic::iced::Length;
use cosmic::prelude::*;
use cosmic::widget;

pub(super) fn text_overlay_layer(text: String) -> Element<'static, Message> {
    let close_button_icon = widget::icon(icons::named_symbolic_icon("window-close-symbolic"))
        .class(container_on_svg_style())
        .size(16);

    let close_button = widget::button::custom(close_button_icon)
        .class(cosmic::theme::Button::Custom {
            active: Box::new(|_, theme| transparent_icon_button_style(theme)),
            disabled: Box::new(transparent_icon_button_style),
            hovered: Box::new(|_, theme| transparent_icon_button_style(theme)),
            pressed: Box::new(|_, theme| transparent_icon_button_style(theme)),
        })
        .on_press(Message::CloseTextOverlay)
        .width(Length::Shrink);

    let header = widget::row::Row::new()
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::text::heading("Text preview"))
        .push(widget::space().width(Length::Fill))
        .push(widget::tooltip(
            close_button,
            widget::text("Close preview"),
            widget::tooltip::Position::Top,
        ));

    widget::container(
        widget::column::Column::new()
            .spacing(8)
            .height(Length::Fill)
            .push(header)
            .push(widget::divider::horizontal::default())
            .push(
                widget::scrollable(
                    widget::container(widget::text::body(text).width(Length::Fill))
                        .width(Length::Fill),
                )
                .id(crate::app::text_overlay_scroll_id())
                .height(Length::Fill)
                .width(Length::Fill),
            ),
    )
    .class(cosmic::theme::Container::Card)
    .padding([10, 12])
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}
