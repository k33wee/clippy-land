use crate::app::{AppModel, Message};
use crate::fl;
use cosmic::iced::Length;
use cosmic::prelude::*;
use cosmic::widget;

pub(super) fn settings_panel(app: &AppModel) -> Element<'_, Message> {
    let history = widget::settings::section()
        .title(fl!("settings-history"))
        .add(number_row(
            fl!("settings-history-limit"),
            fl!("settings-placeholder-history"),
            &app.settings_draft.max_history,
            Message::SettingsMaxHistoryChanged,
            fl!("settings-history-help"),
        ))
        .add(number_row(
            fl!("settings-pin-limit"),
            fl!("settings-placeholder-pins"),
            &app.settings_draft.max_pinned,
            Message::SettingsMaxPinnedChanged,
            fl!("settings-pin-help"),
        ));

    let images = widget::settings::section()
        .title(fl!("settings-images"))
        .add(number_row(
            fl!("settings-image-size"),
            fl!("settings-placeholder-image-mib"),
            &app.settings_draft.max_image_mib,
            Message::SettingsMaxImageMibChanged,
            fl!("settings-image-size-help"),
        ))
        .add(number_row(
            fl!("settings-image-edge"),
            fl!("settings-placeholder-image-edge"),
            &app.settings_draft.max_image_dimension_px,
            Message::SettingsMaxImageDimensionChanged,
            fl!("settings-image-edge-help"),
        ));

    let behavior = widget::settings::section()
        .title(fl!("settings-behavior"))
        .add(toggle_row(
            fl!("settings-auto-paste"),
            app.settings.paste_on_action,
            Message::SettingsPasteOnActionToggled,
            fl!("settings-auto-paste-help"),
        ))
        .add(toggle_row(
            fl!("settings-auto-close"),
            app.settings.close_popup_on_action,
            Message::SettingsClosePopupOnActionToggled,
            fl!("settings-auto-close-help"),
        ));

    let mut content = widget::column::Column::new()
        .spacing(12)
        .width(Length::Fill)
        .push(widget::text::title4(fl!("settings")))
        .push(history)
        .push(images)
        .push(behavior);
    if let Some(error) = &app.settings_error {
        content = content.push(widget::text::body(error).class(cosmic::theme::Text::Custom(
            |theme| cosmic::iced::widget::text::Style {
                color: Some(theme.cosmic().destructive.base.into()),
                ..Default::default()
            },
        )));
    }
    content.into()
}

fn number_row<'a>(
    label: String,
    placeholder: String,
    value: &'a str,
    on_input: fn(String) -> Message,
    help: String,
) -> Element<'a, Message> {
    let label = widget::tooltip(
        widget::text::body(label),
        widget::text::body(help),
        widget::tooltip::Position::Top,
    );
    widget::row::Row::new()
        .spacing(8)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::container(label).width(Length::Fill))
        .push(
            widget::text_input(placeholder, value)
                .on_input(on_input)
                .width(92),
        )
        .into()
}

fn toggle_row(
    label: String,
    value: bool,
    on_toggle: fn(bool) -> Message,
    help: String,
) -> Element<'static, Message> {
    widget::tooltip(
        widget::settings::item::builder(label).control(widget::toggler(value).on_toggle(on_toggle)),
        widget::text::body(help),
        widget::tooltip::Position::Top,
    )
    .into()
}
