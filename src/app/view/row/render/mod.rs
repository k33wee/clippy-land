mod actions;
mod content;

use super::super::style::{
    highlight_history_target, pressed_entry_button_style, transparent_entry_button_style,
};
use super::{HISTORY_ROW_HEIGHT, state::RowRenderState};
use crate::app::Message;
use crate::app::model::FocusPart;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget;

pub(in crate::app) fn history_row(state: RowRenderState) -> Element<'static, Message> {
    let row_alignment = Alignment::Center;
    let label = content::row_label(
        &state,
        state.row_is_hovered || state.row_keyboard_focus.is_some(),
    );

    let copy_button = widget::button::custom(label)
        .class(cosmic::theme::Button::Custom {
            active: Box::new(|_, theme| transparent_entry_button_style(theme)),
            disabled: Box::new(transparent_entry_button_style),
            hovered: Box::new(|_, theme| transparent_entry_button_style(theme)),
            pressed: Box::new(|_, theme| pressed_entry_button_style(theme)),
        })
        .on_press(Message::CopyFromHistory(state.idx))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([8, 16]);

    let entry_active = state.row_keyboard_focus == Some(FocusPart::Entry)
        || state.hovered_focus == Some(FocusPart::Entry);

    let copy_button_elem = widget::mouse_area(highlight_history_target(
        widget::container(copy_button).width(Length::Fill).into(),
        entry_active,
    ));

    let entry = widget::row::Row::new()
        .push(copy_button_elem)
        .push(actions::row_actions(&state))
        .align_y(row_alignment)
        .width(Length::Fill);

    let row_card = widget::container(entry)
        .class(cosmic::theme::Container::Transparent)
        .width(Length::Fill)
        .height(Length::Fixed(HISTORY_ROW_HEIGHT))
        .clip(true);

    widget::mouse_area(row_card)
        .on_enter(Message::HoverEntry(Some((state.idx, FocusPart::Entry))))
        .on_exit(Message::HoverEntry(None))
        .into()
}
