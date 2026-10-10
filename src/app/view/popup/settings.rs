use crate::app::{AppModel, Message};
use cosmic::iced::Length;
use cosmic::prelude::*;
use cosmic::widget;

pub(super) fn settings_panel(app: &AppModel) -> Element<'_, Message> {
    let mut col = widget::column::Column::new().spacing(8).width(Length::Fill);

    col = col
        .push(widget::text::heading("Settings"))
        .push(widget::text::caption("History limits"));

    let history_max = widget::column::Column::new()
        .spacing(4)
        .push(widget::text::body("Max history entries"))
        .push(
            widget::text_input("e.g. 200", &app.settings_draft.max_history)
                .on_input(Message::SettingsMaxHistoryChanged)
                .width(Length::Fill),
        )
        .push(widget::text::caption("Allowed range: 30–5000"));

    let pinned_max = widget::column::Column::new()
        .spacing(4)
        .push(widget::text::body("Max pinned entries"))
        .push(
            widget::text_input("e.g. 20", &app.settings_draft.max_pinned)
                .on_input(Message::SettingsMaxPinnedChanged)
                .width(Length::Fill),
        )
        .push(widget::text::caption(
            "Allowed range: 0–500 (and ≤ max history)",
        ));

    col = col.push(
        widget::row::Row::new()
            .spacing(8)
            .push(history_max)
            .push(pinned_max),
    );

    col = col.push(widget::divider::horizontal::light());
    col = col.push(widget::text::caption("Image limits"));

    let image_bytes = widget::column::Column::new()
        .spacing(4)
        .push(widget::text::body("Max image size (bytes)"))
        .push(
            widget::text_input("e.g. 8388608", &app.settings_draft.max_image_bytes)
                .on_input(Message::SettingsMaxImageBytesChanged)
                .width(Length::Fill),
        )
        .push(widget::text::caption("Allowed range: 262144–67108864"));

    let image_dimension = widget::column::Column::new()
        .spacing(4)
        .push(widget::text::body("Max image dimension (px)"))
        .push(
            widget::text_input("e.g. 8192", &app.settings_draft.max_image_dimension_px)
                .on_input(Message::SettingsMaxImageDimensionChanged)
                .width(Length::Fill),
        )
        .push(widget::text::caption("Allowed range: 512–16384"));

    col = col.push(
        widget::row::Row::new()
            .spacing(8)
            .push(image_bytes)
            .push(image_dimension),
    );

    col = col.push(widget::divider::horizontal::light());
    col = col.push(widget::text::caption("Behavior"));

    let paste_on_action = widget::column::Column::new()
        .spacing(4)
        .push(
            widget::toggler(app.settings.paste_on_action)
                .label("Paste into the active app after selection".to_string())
                .on_toggle(Message::SettingsPasteOnActionToggled),
        )
        .push(widget::text::caption(
            "After Enter or a click, closes the popup and sends Ctrl+V once focus returns to the previous app. Falls back to copy-only when virtual keyboard access is unavailable.",
        ));

    let close_popup_on_action = widget::column::Column::new()
        .spacing(4)
        .push(
            widget::toggler(app.settings.close_popup_on_action)
                .label("Close popup after selection".to_string())
                .on_toggle(Message::SettingsClosePopupOnActionToggled),
        )
        .push(widget::text::caption(
            "Closes the clipboard popup after an entry is copied or pasted. Disabling this also disables auto-paste.",
        ));

    col = col.push(paste_on_action).push(close_popup_on_action);

    if let Some(err) = &app.settings_error {
        col = col.push(widget::text::body(err));
    }

    let apply_row = widget::row::Row::new()
        .width(Length::Fill)
        .push(widget::button::suggested("Apply").on_press(Message::ApplySettings));

    col = col.push(apply_row);

    widget::container(col)
        .class(cosmic::theme::Container::Card)
        .padding([8, 12])
        .width(Length::Fill)
        .into()
}
