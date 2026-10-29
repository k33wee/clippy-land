use super::super::super::style::image_badge_style;
use super::super::RowContent;
use super::super::state::RowRenderState;
use crate::app::view::summary::image_metadata;
use crate::app::{Message, icons};
use crate::fl;
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::widget::{Stack, image::Handle as ImageHandle};
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;

const IMAGE_PREVIEW_HEIGHT: f32 = 96.0;

pub(super) fn row_label(state: &RowRenderState, show_metadata: bool) -> Element<'static, Message> {
    match &state.content {
        RowContent::Text {
            expanded_summary, ..
        } => {
            let text: Element<'static, Message> = if (expanded_summary.starts_with("https://")
                || expanded_summary.starts_with("http://"))
                && !expanded_summary.chars().any(char::is_whitespace)
            {
                // Visual URL treatment only: activating the row still copies it.
                let span =
                    cosmic::iced::widget::text::Span::<'static, (), cosmic::iced::Font>::new(
                        expanded_summary.clone(),
                    )
                    .underline(true);
                cosmic::iced::widget::rich_text([span])
                    .size(14)
                    .font(cosmic::font::semibold())
                    .line_height(cosmic::iced::advanced::text::LineHeight::Absolute(
                        21.0.into(),
                    ))
                    .width(Length::Fill)
                    .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(4)))
                    .into()
            } else {
                widget::text::body(expanded_summary.clone())
                    .font(cosmic::font::semibold())
                    .width(Length::Fill)
                    .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(4)))
                    .into()
            };
            widget::container(text)
                .width(Length::Fill)
                .center_y(Length::Fill)
                .into()
        }
        RowContent::Image {
            mime,
            bytes_len,
            dimensions,
            thumbnail_handle,
            ..
        } => {
            let preview: Element<'static, Message> = if let Some(handle) = thumbnail_handle.clone()
            {
                widget::image::<ImageHandle>(handle)
                    .width(Length::Fill)
                    .height(Length::Fixed(IMAGE_PREVIEW_HEIGHT))
                    .content_fit(cosmic::iced::ContentFit::Contain)
                    .expand(false)
                    .into()
            } else {
                widget::icon(icons::named_symbolic_icon("image-x-generic-symbolic"))
                    .size(32)
                    .into()
            };
            let mut frame = Stack::new()
                .push(
                    widget::container(preview)
                        .center_x(Length::Fill)
                        .center_y(Length::Fixed(IMAGE_PREVIEW_HEIGHT))
                        .clip(true),
                )
                .width(Length::Fill)
                .height(Length::Fixed(IMAGE_PREVIEW_HEIGHT));
            if show_metadata {
                let label = dimensions
                    .map(|(width, height)| fl!("image-dimensions", width = width, height = height))
                    .unwrap_or_else(|| mime.strip_prefix("image/").unwrap_or(mime).to_uppercase());
                let badge = widget::tooltip(
                    widget::container(widget::text::caption(label))
                        .padding([3, 6])
                        .class(cosmic::theme::Container::custom(image_badge_style)),
                    widget::text::body(image_metadata(mime, *bytes_len, *dimensions)),
                    widget::tooltip::Position::Bottom,
                );
                frame = frame.push(
                    widget::container(badge)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(Alignment::Start)
                        .align_y(Alignment::Start),
                );
            }
            frame.into()
        }
    }
}
