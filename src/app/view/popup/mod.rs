mod about;
mod overlay;
mod settings;

use super::row::{RowContent, RowRenderState, history_row};
use super::style::{layer_popup_style, quiet_clear_button_class};
use super::summary::{image_metadata, text_overlay_available};
use crate::app::{AppModel, Message, icons};
use crate::fl;
use cosmic::iced::{Alignment, Length, window::Id};
use cosmic::prelude::*;
use cosmic::widget;
use std::time::Instant;

const HISTORY_VIEWPORT_HEIGHT: f32 = 420.0;

pub(super) fn view(app: &AppModel) -> Element<'_, Message> {
    app.core
        .applet
        .icon_button_from_handle(icons::panel_icon())
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
    let crate::services::clipboard::ClipboardEntry::Text(text) =
        &app.history.get(active_idx)?.entry
    else {
        return None;
    };
    text_overlay_available(text).then(|| text.trim().to_string())
}

pub(super) fn view_window(app: &AppModel, _id: Id) -> Element<'_, Message> {
    let build_started = Instant::now();
    let visible = filtered_indices(app);
    let is_layer = app.popup_surface == Some(crate::app::model::PopupSurface::LayerSurface);
    let flexible_height = if is_layer {
        Length::Fill
    } else {
        Length::Shrink
    };
    let preview_height = if is_layer {
        Length::Fill
    } else {
        Length::Fixed(HISTORY_VIEWPORT_HEIGHT)
    };
    let mut visible_image_count = 0;
    let mut content = widget::column::Column::new()
        .spacing(8)
        .padding(cosmic::iced::Padding {
            top: 12.0,
            right: 16.0,
            bottom: 24.0,
            left: 16.0,
        })
        .height(flexible_height);

    if app.settings_open || app.about_open {
        let page = if app.settings_open {
            settings::settings_panel(app)
        } else {
            about::about_panel()
        };
        let page = widget::container(
            widget::scrollable(widget::container(page).padding([4, 0]).width(Length::Fill))
                .width(Length::Fill)
                .height(flexible_height),
        )
        .height(flexible_height)
        .max_height(if is_layer {
            crate::app::surfaces::TOGGLE_POPUP_HEIGHT as f32
        } else {
            460.0
        });
        content = content.push(page);
    } else {
        // The header, search, and separate Clear strip retain their geometry
        // while controls initialize or a query/preview hides the Clear action.
        content = content.push(history_header(app.popup_controls_ready)).push(
            widget::search_input(fl!("search-placeholder"), &app.search_query)
                .style(cosmic::theme::TextInput::Default)
                .padding([4, 8])
                .on_input(Message::SearchChanged)
                .on_clear(Message::SearchChanged(String::new()))
                .width(Length::Fill),
        );
        let mut clear_strip = widget::row::Row::new()
            .height(28)
            .align_y(Alignment::Center)
            .push(widget::space().width(Length::Fill));
        if app.popup_controls_ready
            && !app.history.is_empty()
            && app.search_query.is_empty()
            && !app.preview_is_open()
        {
            clear_strip = clear_strip.push(widget::tooltip(
                widget::button::text(fl!("clear"))
                    .class(quiet_clear_button_class())
                    .padding([4, 12])
                    .height(28)
                    .on_press(Message::ClearHistory),
                widget::text(fl!("delete-all")),
                widget::tooltip::Position::Bottom,
            ));
        }
        content = content.push(clear_strip);
        let history_area: Element<'_, Message> = if let Some(text) =
            selected_text_overlay(app, &visible)
        {
            widget::container(overlay::text_overlay_layer(text))
                .height(preview_height)
                .width(Length::Fill)
                .clip(true)
                .into()
        } else if let Some(preview) = &app.image_preview {
            let details = visible
                .iter()
                .find_map(|&idx| match &app.history[idx].entry {
                    crate::services::clipboard::ClipboardEntry::Image {
                        mime, bytes, hash, ..
                    } if (*hash, bytes.len()) == preview.key => Some(image_metadata(
                        mime,
                        bytes.len(),
                        app.image_dimensions.get(&preview.key).copied().flatten(),
                    )),
                    _ => None,
                })
                .unwrap_or_default();
            widget::container(overlay::image_overlay_layer(preview, details))
                .height(preview_height)
                .width(Length::Fill)
                .clip(true)
                .into()
        } else {
            history_scrollable_area(app, &visible, &mut visible_image_count)
        };
        content = content.push(history_area);
    }

    if app.popup_controls_ready && (app.settings_open || app.about_open) {
        let mut controls = widget::row::Row::new()
            .spacing(8)
            .align_y(Alignment::Center)
            .width(Length::Fill);
        controls = controls
            .push(
                widget::button::text(fl!("back"))
                    .leading_icon(icons::named_symbolic_icon("go-previous-symbolic"))
                    .on_press(Message::BackToHistory),
            )
            .push(widget::space().width(Length::Fill));
        if app.settings_open {
            controls = controls
                .push(widget::button::suggested(fl!("apply")).on_press(Message::ApplySettings));
        }
        content = content
            .push(widget::divider::horizontal::light())
            .push(widget::container(controls).width(Length::Fill));
    }

    app.note_popup_view_built(visible.len(), visible_image_count, build_started.elapsed());
    if is_layer {
        // Keep compositor-chosen layer geometry; the applet helper autosizes instead.
        let width = app
            .toggle_popup_placement
            .unwrap_or(crate::app::surfaces::TogglePopupPlacement::ActiveMonitorStrip)
            .content_width();
        let popup = widget::container(content)
            .class(cosmic::theme::Container::custom(layer_popup_style))
            .width(Length::Fixed(width as f32))
            .height(Length::Fill);
        widget::container(popup)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Start)
            .align_y(Alignment::Start)
            .into()
    } else {
        app.core.applet.popup_container(content).into()
    }
}

fn history_header(ready: bool) -> Element<'static, Message> {
    if !ready {
        return widget::space().height(32).width(Length::Fill).into();
    }
    widget::row::Row::new()
        .height(32)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .push(widget::tooltip(
            widget::button::icon(icons::named_symbolic_icon("help-about-symbolic"))
                .icon_size(14)
                .width(32)
                .height(32)
                .padding(9)
                .on_press(Message::ToggleAbout),
            widget::text(fl!("about")),
            widget::tooltip::Position::Bottom,
        ))
        .push(widget::space().width(Length::Fill))
        .push(widget::tooltip(
            widget::button::icon(icons::named_symbolic_icon("preferences-system-symbolic"))
                .icon_size(14)
                .width(32)
                .height(32)
                .padding(9)
                .on_press(Message::ToggleSettingsPanel),
            widget::text(fl!("settings")),
            widget::tooltip::Position::Bottom,
        ))
        .into()
}

fn empty_history_state(searching: bool, is_layer: bool) -> Element<'static, Message> {
    let (icon, title, hint) = if searching {
        (
            "system-search-symbolic",
            fl!("no-results"),
            fl!("no-results-hint"),
        )
    } else {
        ("edit-paste-symbolic", fl!("empty"), fl!("empty-hint"))
    };
    let status = widget::column::Column::new()
        .spacing(12)
        .align_x(Alignment::Center)
        .push(widget::icon(icons::named_symbolic_icon(icon)).size(40))
        .push(widget::text::heading(title).align_x(Alignment::Center))
        .push(
            widget::text::body(hint)
                .width(Length::Fill)
                .align_x(Alignment::Center),
        );
    widget::container(status)
        .padding([24, 20])
        .width(Length::Fill)
        .center_x(Length::Fill)
        .center_y(if is_layer {
            Length::Fill
        } else {
            Length::Fixed(HISTORY_VIEWPORT_HEIGHT)
        })
        .clip(true)
        .into()
}

fn history_scrollable_area<'a>(
    app: &'a AppModel,
    visible: &[usize],
    image_count: &mut usize,
) -> Element<'a, Message> {
    let is_layer = app.popup_surface == Some(crate::app::model::PopupSurface::LayerSurface);
    let height = if is_layer {
        Length::Fill
    } else {
        Length::Fixed(HISTORY_VIEWPORT_HEIGHT)
    };
    if visible.is_empty() {
        return empty_history_state(!app.history.is_empty(), is_layer);
    }
    let mut rows = widget::column::Column::new().spacing(0);
    let pinned_count = app.history.iter().filter(|item| item.pinned).count();
    for (position, &idx) in visible.iter().enumerate() {
        if position > 0 {
            if app.search_query.is_empty()
                && idx == pinned_count
                && pinned_count > 0
                && pinned_count < app.history.len()
            {
                rows = rows.push(widget::divider::horizontal::default());
            } else {
                rows = rows.push(widget::divider::horizontal::light());
            }
        }
        let state = RowRenderState::from_app(app, idx, &app.history[idx]);
        if matches!(state.content, RowContent::Image { .. }) {
            *image_count += 1;
        }
        rows = rows.push(history_row(state));
    }
    widget::container(
        widget::scrollable(widget::container(rows).width(Length::Fill))
            .id(crate::app::history_scroll_id())
            .on_scroll(Message::HistoryScrolled)
            .width(Length::Fill)
            .height(height),
    )
    .class(cosmic::theme::Container::List)
    .height(height)
    .max_height(if is_layer {
        crate::app::surfaces::TOGGLE_POPUP_HEIGHT as f32
    } else {
        HISTORY_VIEWPORT_HEIGHT
    })
    .clip(true)
    .width(Length::Fill)
    .into()
}
