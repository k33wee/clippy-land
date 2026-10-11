use crate::app::Message;
use cosmic::iced::{Background, Border, Color};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::button::Style as ButtonStyle;
use cosmic::widget::container::Style as ContainerStyle;
use std::rc::Rc;

/// Match the applet popup helper's appearance without its window-autosizing wrapper.
pub(super) fn layer_popup_style(theme: &cosmic::Theme) -> ContainerStyle {
    let cosmic = theme.cosmic();
    let background = cosmic.background(theme.transparent);
    ContainerStyle {
        text_color: Some(background.on.into()),
        icon_color: Some(background.on.into()),
        background: Some(Background::Color(background.base.into())),
        border: Border {
            radius: cosmic.corner_radii.radius_m.into(),
            width: 1.0,
            color: background.divider.into(),
        },
        snap: true,
        ..Default::default()
    }
}

pub(super) fn highlight_history_target<'a>(
    content: Element<'a, Message>,
    active: bool,
) -> Element<'a, Message> {
    if !active {
        return content;
    }

    widget::container(content)
        .class(cosmic::theme::Container::custom(|theme| {
            let c = theme.cosmic();
            // Use `on` (the contrasting color) at low alpha so the highlight is
            // visible in both light mode (dark-on-white) and dark mode (light-on-dark).
            // `component.hover` is pre-composited and nearly invisible in light mode.
            let on: Color = theme.current_container().component.on.into();
            ContainerStyle {
                background: Some(Background::Color(Color { a: 0.1, ..on })),
                border: Border {
                    radius: c.corner_radii.radius_s.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

/// Transparent style for icon-only buttons. Suppresses the built-in hover
/// background so `highlight_history_target` is the sole visual indicator,
/// giving identical feedback for both mouse and keyboard navigation.
pub(super) fn transparent_icon_button_style(theme: &cosmic::Theme) -> ButtonStyle {
    let c = theme.cosmic();
    let on_color: Color = theme.current_container().component.on.into();

    ButtonStyle {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border_radius: c.corner_radii.radius_s.into(),
        icon_color: Some(on_color),
        text_color: Some(on_color),
        ..Default::default()
    }
}

pub(super) fn container_on_svg_style() -> cosmic::theme::Svg {
    cosmic::theme::Svg::Custom(Rc::new(|theme| cosmic::iced::widget::svg::Style {
        color: Some(theme.current_container().component.on.into()),
    }))
}

pub(super) fn accent_svg_style() -> cosmic::theme::Svg {
    cosmic::theme::Svg::Custom(Rc::new(|theme| cosmic::iced::widget::svg::Style {
        color: Some(theme.cosmic().accent.base.into()),
    }))
}

/// Transparent icon button style that keeps the icon accent-colored.
/// Used to preserve pinned-state visual feedback for symbolic theme icons.
pub(super) fn accent_icon_button_style(theme: &cosmic::Theme) -> ButtonStyle {
    let c = theme.cosmic();
    let accent: Color = c.accent.base.into();

    ButtonStyle {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border_radius: c.corner_radii.radius_s.into(),
        icon_color: Some(accent),
        text_color: Some(accent),
        ..Default::default()
    }
}

pub(super) fn transparent_entry_button_style(theme: &cosmic::Theme) -> ButtonStyle {
    let c = theme.cosmic();
    let on_color: Color = theme.current_container().component.on.into();

    ButtonStyle {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border_radius: c.corner_radii.radius_s.into(),
        icon_color: Some(on_color),
        text_color: Some(on_color),
        ..Default::default()
    }
}

/// Pressed variant: stronger highlight using the `on` color at higher alpha.
/// Iced applies this automatically while the button is held down.
pub(super) fn pressed_entry_button_style(theme: &cosmic::Theme) -> ButtonStyle {
    let c = theme.cosmic();
    let on_color: Color = theme.current_container().component.on.into();

    ButtonStyle {
        background: Some(Background::Color(Color { a: 0.2, ..on_color })),
        border_radius: c.corner_radii.radius_s.into(),
        icon_color: Some(on_color),
        text_color: Some(on_color),
        ..Default::default()
    }
}
