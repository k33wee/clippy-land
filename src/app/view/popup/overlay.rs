use crate::app::model::ImagePreview;
use crate::app::{Message, icons};
use crate::fl;
use cosmic::iced::{Alignment, ContentFit, Length};
use cosmic::prelude::*;
use cosmic::widget;

fn header(title: String) -> Element<'static, Message> {
    widget::row::Row::new()
        .align_y(Alignment::Center)
        .push(widget::text::heading(title))
        .push(widget::space().width(Length::Fill))
        .push(widget::tooltip(
            widget::button::icon(icons::named_symbolic_icon("window-close-symbolic"))
                .on_press(Message::ClosePreview),
            widget::text(fl!("close-preview")),
            widget::tooltip::Position::Top,
        ))
        .into()
}

pub(super) fn text_overlay_layer(text: String) -> Element<'static, Message> {
    widget::container(
        widget::column::Column::new()
            .spacing(8)
            .height(Length::Fill)
            .push(header(fl!("text-preview")))
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

pub(super) fn image_overlay_layer(
    preview: &ImagePreview,
    details: String,
) -> Element<'static, Message> {
    let image: Element<'static, Message> = if let Some(handle) = &preview.handle {
        widget::image(handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Contain)
            .into()
    } else {
        widget::text::body(if preview.failed {
            fl!("image-preview-failed")
        } else {
            fl!("image-preview-loading")
        })
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into()
    };
    widget::container(
        widget::column::Column::new()
            .spacing(8)
            .height(Length::Fill)
            .push(header(fl!("image-preview")))
            .push(widget::divider::horizontal::default())
            .push(
                widget::container(image)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .clip(true),
            )
            .push(
                widget::text::caption(details)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
            ),
    )
    .class(cosmic::theme::Container::Card)
    .padding([10, 12])
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}
