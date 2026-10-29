use super::super::super::style::{
    ControlTone, control_button_class, control_svg_style, highlight_history_target,
};
use super::super::{RowContent, RowRenderState};
use crate::app::model::FocusPart;
use crate::app::{Message, icons};
use crate::fl;
use cosmic::iced::Length;
use cosmic::prelude::*;
use cosmic::widget;

pub(super) fn row_actions(state: &RowRenderState) -> Element<'static, Message> {
    let pin_active = state.row_keyboard_focus == Some(FocusPart::Pin)
        || state.hovered_focus == Some(FocusPart::Pin);
    let preview_active = state.row_keyboard_focus == Some(FocusPart::Preview)
        || state.hovered_focus == Some(FocusPart::Preview)
        || state.preview_open;
    let remove_active = state.row_keyboard_focus == Some(FocusPart::Remove)
        || state.hovered_focus == Some(FocusPart::Remove);

    let mut actions = widget::column::Column::new()
        .spacing(2)
        .align_x(cosmic::iced::Alignment::Center);

    actions = actions
        .push(pin_button(state, pin_active))
        .push(remove_button(state, remove_active));
    if matches!(
        &state.content,
        RowContent::Text {
            overlay_available: true,
            ..
        } | RowContent::Image { .. }
    ) {
        actions = actions.push(preview_button(state, preview_active));
    } else {
        // Keep pin/delete targets aligned even when this item has no preview.
        actions = actions.push(widget::space().height(26));
    }

    widget::container(actions)
        .width(Length::Fixed(36.0))
        .height(Length::Fill)
        .align_y(cosmic::iced::Alignment::Center)
        .padding([8, 2])
        .into()
}

fn pin_button(state: &RowRenderState, active: bool) -> Element<'static, Message> {
    let tone = if state.pinned || active {
        ControlTone::Accent
    } else {
        ControlTone::Neutral
    };

    let pin_icon = widget::icon(if state.pinned {
        icons::pin_icon_pinned()
    } else {
        icons::pin_icon()
    })
    .class(control_svg_style(tone))
    .size(16);

    let pin_button = widget::button::custom(pin_icon)
        .class(control_button_class(tone))
        .on_press(Message::TogglePin(state.idx))
        .width(Length::Shrink);

    widget::mouse_area(highlight_history_target(
        widget::tooltip(
            pin_button,
            widget::text(if state.pinned {
                fl!("unpin")
            } else {
                fl!("pin")
            }),
            widget::tooltip::Position::Top,
        )
        .into(),
        active,
    ))
    .on_enter(Message::HoverEntry(Some((state.idx, FocusPart::Pin))))
    .on_exit(Message::HoverEntry(Some((state.idx, FocusPart::Entry))))
    .into()
}

fn remove_button(state: &RowRenderState, active: bool) -> Element<'static, Message> {
    let tone = if active {
        ControlTone::Destructive
    } else {
        ControlTone::Neutral
    };
    let remove_button = widget::button::custom(
        widget::icon(icons::remove_icon())
            .class(control_svg_style(tone))
            .size(16),
    )
    .class(control_button_class(tone))
    .on_press(Message::RemoveHistory(state.idx))
    .width(Length::Shrink);

    widget::mouse_area(highlight_history_target(
        widget::tooltip(
            remove_button,
            widget::text(fl!("remove")),
            widget::tooltip::Position::Top,
        )
        .into(),
        active,
    ))
    .on_enter(Message::HoverEntry(Some((state.idx, FocusPart::Remove))))
    .on_exit(Message::HoverEntry(Some((state.idx, FocusPart::Entry))))
    .into()
}

fn preview_button(state: &RowRenderState, active: bool) -> Element<'static, Message> {
    let tone = if active {
        ControlTone::Accent
    } else {
        ControlTone::Neutral
    };
    let preview_button = widget::button::custom(
        widget::icon(icons::preview_icon())
            .class(control_svg_style(tone))
            .size(16),
    )
    .class(control_button_class(tone))
    .on_press(Message::OpenPreview(state.idx))
    .width(Length::Shrink);

    widget::mouse_area(highlight_history_target(
        widget::tooltip(
            preview_button,
            widget::text(fl!("preview")),
            widget::tooltip::Position::Top,
        )
        .into(),
        active,
    ))
    .on_enter(Message::HoverEntry(Some((state.idx, FocusPart::Preview))))
    .on_exit(Message::HoverEntry(Some((state.idx, FocusPart::Entry))))
    .into()
}
