mod overlay;
mod settings;

use super::row::{RowContent, RowRenderState, history_row};
use super::style::{container_on_svg_style, transparent_icon_button_style};
use super::summary::text_overlay_available;
use crate::app::{AppModel, Message, icons};
use crate::fl;
use cosmic::iced::{Alignment, Length, window::Id};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::Instant;

pub(super) fn view(app: &AppModel) -> Element<'_, Message> {
    app.core
        .applet
        .icon_button("edit-copy-symbolic")
        .on_press_down(Message::TogglePopup)
        .into()
}

/// Returns the indices into `app.history` that match the current search query.
pub(crate) fn filtered_indices(app: &AppModel) -> Vec<usize> {
    let cache_looks_valid = app.filtered_query_cache == app.search_query
        && app.filtered_history_len_cache == app.history.len()
        && app
            .filtered_indices
            .iter()
            .all(|&idx| idx < app.history.len());

    if cache_looks_valid {
        app.filtered_indices.clone()
    } else {
        AppModel::compute_filtered_indices_for(&app.history, &app.search_query)
    }
}

pub(crate) fn selected_text_overlay(app: &AppModel, visible: &[usize]) -> Option<String> {
    let active_idx = app.text_overlay_index?;

    if !visible.contains(&active_idx) {
        return None;
    }

    let item = app.history.get(active_idx)?;
    let crate::services::clipboard::ClipboardEntry::Text(text) = &item.entry else {
        return None;
    };

    if !text_overlay_available(text) {
        return None;
    }

    Some(text.trim().to_string())
}

pub(super) fn view_window(app: &AppModel, _id: Id) -> Element<'_, Message> {
    let build_started = Instant::now();
    let visible = filtered_indices(app);
    let mut visible_image_count = 0usize;

    let history_area: Element<'_, Message> =
        if let Some(text_overlay) = selected_text_overlay(app, &visible) {
            visible_image_count = count_visible_images(app, &visible);
            widget::container(overlay::text_overlay_layer(text_overlay))
                .width(Length::Fill)
                .height(380.0)
                .into()
        } else {
            history_scrollable_area(app, &visible, &mut visible_image_count)
        };

    let search_bar = widget::container(
        widget::search_input(fl!("search-placeholder"), &app.search_query)
            .on_input(Message::SearchChanged)
            .on_clear(Message::SearchChanged(String::new()))
            .width(Length::Fill),
    )
    .padding([0, 12]);

    let mut content = widget::column::Column::new().spacing(8).padding([8, 8]);

    if app.settings_open {
        let settings_form = settings::settings_panel(app);
        content = content.push(widget::container(settings_form).padding([0, 12]));
    } else {
        if !app.history.is_empty() {
            content = content.push(search_bar);
        }

        content = content.push(history_area);
    };

    let mut left_controls = widget::row::Row::new()
        .spacing(8)
        .align_y(Alignment::Center);
    let mut right_controls = widget::row::Row::new()
        .spacing(8)
        .align_y(Alignment::Center);
    let mut has_left_controls = false;
    let mut has_right_controls = false;

    if app.popup_controls_ready {
        let settings_button_icon =
            widget::icon(icons::named_symbolic_icon("preferences-system-symbolic"))
                .class(container_on_svg_style())
                .size(16);

        let settings_button = widget::button::custom(settings_button_icon)
            .class(cosmic::theme::Button::Custom {
                active: Box::new(|_, theme| transparent_icon_button_style(theme)),
                disabled: Box::new(transparent_icon_button_style),
                hovered: Box::new(|_, theme| transparent_icon_button_style(theme)),
                pressed: Box::new(|_, theme| transparent_icon_button_style(theme)),
            })
            .on_press(Message::ToggleSettingsPanel)
            .width(Length::Shrink);
        left_controls = left_controls.push(widget::tooltip(
            settings_button,
            widget::text(if app.settings_open {
                "Close settings"
            } else {
                "Settings"
            }),
            widget::tooltip::Position::Top,
        ));
        has_left_controls = true;
    }

    if !app.history.is_empty() && app.search_query.is_empty() {
        let delete_all_button = widget::button::destructive(fl!("delete-all"))
            .leading_icon(icons::remove_icon())
            .on_press(Message::ClearHistory);
        right_controls = right_controls.push(delete_all_button);
        has_right_controls = true;
    }

    if has_left_controls || has_right_controls {
        let mut controls_sheet = widget::row::Row::new()
            .width(Length::Fill)
            .align_y(Alignment::Center);

        if has_left_controls {
            controls_sheet = controls_sheet.push(
                widget::container(left_controls)
                    .width(Length::Fill)
                    .align_x(Alignment::Start),
            );
        } else {
            controls_sheet = controls_sheet.push(widget::space().width(Length::Fill));
        }

        if has_right_controls {
            controls_sheet = controls_sheet.push(
                widget::container(right_controls)
                    .width(Length::Fill)
                    .align_x(Alignment::End),
            );
        }

        let controls_sheet = widget::container(controls_sheet)
            .padding([8, 8])
            .width(Length::Fill);
        content = content.push(controls_sheet);
    }

    app.note_popup_view_built(visible.len(), visible_image_count, build_started.elapsed());

    app.core.applet.popup_container(content).into()
}

fn count_visible_images(app: &AppModel, visible: &[usize]) -> usize {
    visible
        .iter()
        .filter(|&&idx| {
            matches!(
                app.history.get(idx).map(|item| &item.entry),
                Some(crate::services::clipboard::ClipboardEntry::Image { .. })
            )
        })
        .count()
}

fn history_scrollable_area<'a>(
    app: &'a AppModel,
    visible: &[usize],
    visible_image_count: &mut usize,
) -> Element<'a, Message> {
    let mut history_column = widget::column::Column::new().spacing(4);

    if app.history.is_empty() {
        history_column = history_column.push(
            widget::container(widget::text::body(fl!("empty")))
                .width(Length::Fill)
                .center_x(Length::Fill),
        );
    } else if visible.is_empty() {
        history_column = history_column.push(
            widget::container(widget::text::body(fl!("no-results")))
                .width(Length::Fill)
                .center_x(Length::Fill),
        );
    } else {
        let pinned_count = app.history.iter().filter(|it| it.pinned).count();

        for &idx in visible {
            if app.search_query.is_empty()
                && idx == pinned_count
                && pinned_count > 0
                && pinned_count < app.history.len()
            {
                history_column = history_column.push(widget::divider::horizontal::default());
            }

            let row_state = RowRenderState::from_app(app, idx, &app.history[idx]);
            if matches!(row_state.content, RowContent::Image { .. }) {
                *visible_image_count += 1;
            }
            history_column = history_column.push(history_row(row_state));
        }
    }

    widget::container(
        widget::scrollable(
            widget::container(history_column)
                .padding([0, 12, 0, 12])
                .width(Length::Fill),
        )
        .id(crate::app::history_scroll_id())
        .on_scroll(Message::HistoryScrolled)
        .width(Length::Fill),
    )
    .max_height(400.0)
    .clip(true)
    .width(Length::Fill)
    .into()
}
