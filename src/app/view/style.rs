use crate::app::Message;
use cosmic::iced::{Background, Border, Color};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::button::Style as ButtonStyle;
use cosmic::widget::container::Style as ContainerStyle;
use std::rc::Rc;

/// Match the panel popup's theme-selected background and border.
/// The translucent fill requires the shortcut's separate layer-surface blur
/// request to be forwarded by a fixed cosmic-panel proxy.
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

#[derive(Clone, Copy)]
pub(super) enum ControlTone {
    Neutral,
    Accent,
    Destructive,
}

pub(super) fn control_svg_style(tone: ControlTone) -> cosmic::theme::Svg {
    match tone {
        ControlTone::Neutral => container_on_svg_style(),
        ControlTone::Accent => accent_svg_style(),
        ControlTone::Destructive => {
            cosmic::theme::Svg::Custom(Rc::new(|theme| cosmic::iced::widget::svg::Style {
                color: Some(theme.cosmic().destructive.base.into()),
            }))
        }
    }
}

fn control_button_style(theme: &cosmic::Theme, tone: ControlTone) -> ButtonStyle {
    match tone {
        ControlTone::Neutral => transparent_icon_button_style(theme),
        ControlTone::Accent => accent_icon_button_style(theme),
        ControlTone::Destructive => {
            let color = theme.cosmic().destructive.base.into();
            ButtonStyle {
                icon_color: Some(color),
                text_color: Some(color),
                ..transparent_icon_button_style(theme)
            }
        }
    }
}

pub(super) fn control_button_class(tone: ControlTone) -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(move |_, theme| control_button_style(theme, tone)),
        disabled: Box::new(transparent_icon_button_style),
        hovered: Box::new(move |_, theme| control_button_style(theme, tone)),
        pressed: Box::new(move |_, theme| control_button_style(theme, tone)),
    }
}

fn clear_button_style(theme: &cosmic::Theme, tone: ControlTone, opacity: f32) -> ButtonStyle {
    let on: Color = theme.current_container().component.on.into();
    ButtonStyle {
        background: Some(Background::Color(Color { a: opacity, ..on })),
        ..control_button_style(theme, tone)
    }
}

/// A small neutral chip on its own strip, rather than a red footer pill.
pub(super) fn quiet_clear_button_class() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|_, theme| clear_button_style(theme, ControlTone::Neutral, 0.10)),
        disabled: Box::new(|theme| clear_button_style(theme, ControlTone::Neutral, 0.06)),
        hovered: Box::new(|_, theme| clear_button_style(theme, ControlTone::Destructive, 0.16)),
        pressed: Box::new(|_, theme| clear_button_style(theme, ControlTone::Destructive, 0.22)),
    }
}

pub(super) fn image_badge_style(_: &cosmic::Theme) -> ContainerStyle {
    ContainerStyle {
        text_color: Some(Color::WHITE),
        background: Some(Background::Color(Color::from_rgba(0.04, 0.05, 0.07, 0.80))),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_clear_is_neutral_at_rest_and_destructive_only_on_interaction() {
        for theme in [cosmic::Theme::light(), cosmic::Theme::dark()] {
            let cosmic::theme::Button::Custom {
                active,
                hovered,
                pressed,
                ..
            } = quiet_clear_button_class()
            else {
                panic!("custom toolbar action style");
            };
            assert_eq!(
                active(false, &theme).background,
                Some(Background::Color(Color {
                    a: 0.10,
                    ..Color::from(theme.current_container().component.on)
                }))
            );
            assert_eq!(
                active(false, &theme).text_color,
                Some(theme.current_container().component.on.into())
            );
            assert_eq!(
                hovered(false, &theme).text_color,
                Some(theme.cosmic().destructive.base.into())
            );
            assert_eq!(
                pressed(false, &theme).text_color,
                Some(theme.cosmic().destructive.base.into())
            );
        }
    }

    #[test]
    fn shortcut_popup_background_follows_theme_transparency() {
        for mut theme in [
            cosmic::Theme::light(),
            cosmic::Theme::dark(),
            cosmic::Theme::light_hc(),
            cosmic::Theme::dark_hc(),
        ] {
            for transparent in [false, true] {
                theme.transparent = transparent;
                let style = layer_popup_style(&theme);
                let Some(Background::Color(color)) = style.background else {
                    panic!("shortcut popup must paint a color");
                };
                assert_eq!(
                    color,
                    Color::from(theme.cosmic().background(transparent).base)
                );
                if !transparent {
                    assert_eq!(color.a, 1.0);
                }
            }
        }
    }

    #[test]
    fn action_colors_follow_light_dark_and_high_contrast_palettes() {
        for theme in [
            cosmic::Theme::light(),
            cosmic::Theme::dark(),
            cosmic::Theme::light_hc(),
            cosmic::Theme::dark_hc(),
        ] {
            for (tone, expected) in [
                (
                    ControlTone::Neutral,
                    Color::from(theme.current_container().component.on),
                ),
                (ControlTone::Accent, Color::from(theme.cosmic().accent.base)),
                (
                    ControlTone::Destructive,
                    Color::from(theme.cosmic().destructive.base),
                ),
            ] {
                let button = control_button_style(&theme, tone);
                assert_eq!(button.icon_color, Some(expected));
                assert_eq!(button.text_color, Some(expected));
                let cosmic::theme::Svg::Custom(svg_style) = control_svg_style(tone) else {
                    panic!("control SVG must have an explicit semantic color");
                };
                assert_eq!(svg_style(&theme).color, Some(expected));
            }
        }
    }
}
