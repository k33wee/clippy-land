use super::shared::{prune_thumbnail_handles, row_has_preview};
use crate::app::model::FocusPart;
use crate::app::view::filtered_indices;
use crate::app::{AppModel, Message, pinned_history};

use super::super::{history, scroll};
use cosmic::prelude::*;

pub(super) fn handle(
    app: &mut AppModel,
    message: Message,
) -> Option<Task<cosmic::Action<Message>>> {
    match message {
        Message::HoverEntry(opt) => {
            let next_index = opt.map(|(idx, _)| idx);
            if app.hovered_index == next_index && app.hovered_focus == opt {
                return Some(Task::none());
            }

            if let Some((idx, part)) = opt {
                app.hovered_index = Some(idx);
                app.hovered_focus = Some((idx, part));
                app.keyboard_focus = None;
            } else {
                app.hovered_index = None;
                app.hovered_focus = None;
            }
            Some(Task::none())
        }
        Message::HistoryScrolled(viewport) => {
            app.at_scroll_bottom = viewport.relative_offset().y >= 0.999;
            app.history_viewport = Some(viewport);
            Some(Task::none())
        }
        Message::KeyboardNavigateUp if app.text_overlay_index.is_some() => {
            Some(scroll::scroll_text_overlay_up())
        }
        Message::KeyboardNavigateDown if app.text_overlay_index.is_some() => {
            Some(scroll::scroll_text_overlay_down())
        }
        Message::KeyboardNavigateUp => {
            let visible = filtered_indices(app);
            if visible.is_empty() {
                return Some(Task::none());
            }
            let new_idx = match app
                .hovered_index
                .and_then(|h| visible.iter().position(|&i| i == h))
            {
                Some(pos) => visible[if pos == 0 { visible.len() - 1 } else { pos - 1 }],
                None => *visible.last().unwrap(),
            };
            app.hovered_index = Some(new_idx);
            app.hovered_focus = None;
            app.keyboard_focus = Some((new_idx, FocusPart::Entry));
            app.at_scroll_bottom = false;
            Some(scroll::scroll_selection_into_view(app, new_idx))
        }
        Message::KeyboardNavigateDown => {
            let visible = filtered_indices(app);
            if visible.is_empty() {
                return Some(Task::none());
            }
            let new_idx = match app
                .hovered_index
                .and_then(|h| visible.iter().position(|&i| i == h))
            {
                Some(pos) => visible[(pos + 1) % visible.len()],
                None => visible[0],
            };
            app.hovered_index = Some(new_idx);
            app.hovered_focus = None;
            app.keyboard_focus = Some((new_idx, FocusPart::Entry));
            app.at_scroll_bottom = false;
            Some(scroll::scroll_selection_into_view(app, new_idx))
        }
        Message::MoveFocusLeft => {
            if let Some((idx, part)) = app.keyboard_focus {
                if Some(idx) != app.hovered_index {
                    if let Some(h) = app.hovered_index {
                        app.keyboard_focus = Some((h, FocusPart::Entry));
                    }
                } else {
                    let has_preview = row_has_preview(app, idx);
                    let new_part = match part {
                        FocusPart::Entry => FocusPart::Remove,
                        FocusPart::Preview => FocusPart::Entry,
                        FocusPart::Pin => {
                            if has_preview {
                                FocusPart::Preview
                            } else {
                                FocusPart::Entry
                            }
                        }
                        FocusPart::Remove => FocusPart::Pin,
                    };
                    app.keyboard_focus = Some((idx, new_part));
                }
            } else if let Some(h) = app.hovered_index {
                app.keyboard_focus = Some((h, FocusPart::Entry));
            }
            Some(Task::none())
        }
        Message::MoveFocusRight => {
            if let Some((idx, part)) = app.keyboard_focus {
                if Some(idx) != app.hovered_index {
                    if let Some(h) = app.hovered_index {
                        app.keyboard_focus = Some((h, FocusPart::Entry));
                    }
                } else {
                    let has_preview = row_has_preview(app, idx);
                    let new_part = match part {
                        FocusPart::Entry => {
                            if has_preview {
                                FocusPart::Preview
                            } else {
                                FocusPart::Pin
                            }
                        }
                        FocusPart::Preview => FocusPart::Pin,
                        FocusPart::Pin => FocusPart::Remove,
                        FocusPart::Remove => FocusPart::Entry,
                    };
                    app.keyboard_focus = Some((idx, new_part));
                }
            } else if let Some(h) = app.hovered_index {
                app.keyboard_focus = Some((h, FocusPart::Entry));
            }
            Some(Task::none())
        }
        Message::ActivateSelection => {
            if let Some((idx, part)) = app.keyboard_focus {
                match part {
                    FocusPart::Entry => {
                        if let Some(item) = app.history.get(idx) {
                            return Some(history::copy_history_item(item));
                        }
                    }
                    FocusPart::Preview => {
                        if row_has_preview(app, idx) {
                            app.text_overlay_index = Some(idx);
                        }
                    }
                    FocusPart::Pin => {
                        if history::toggle_pin(&mut app.history, idx, &app.settings) {
                            pinned_history::save(&app.history);
                        }
                        app.recompute_filtered_indices();
                    }
                    FocusPart::Remove => {
                        let removed_pinned =
                            app.history.remove(idx).is_some_and(|item| item.pinned);
                        prune_thumbnail_handles(app);
                        app.recompute_filtered_indices();
                        if removed_pinned {
                            pinned_history::save(&app.history);
                        }
                    }
                }
            } else if let Some(idx) = app.hovered_index
                && let Some(item) = app.history.get(idx)
            {
                return Some(history::copy_history_item(item));
            }
            Some(Task::none())
        }
        Message::EscapePressed => {
            if app.text_overlay_index.is_some() {
                app.text_overlay_index = None;
                Some(Task::none())
            } else {
                None
            }
        }
        _ => None,
    }
}
