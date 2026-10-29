use super::*;
use crate::app::model::{FocusPart, PopupSurface};
use crate::app::view::row::{HISTORY_ROW_HEIGHT, history_row};
use cosmic::iced::advanced::{layout, widget::Tree};
use cosmic::iced::{Font, Size, window::Id};
use cosmic::prelude::*;

fn layout_element(
    mut element: Element<'_, crate::app::Message>,
    width: f32,
    height: f32,
) -> layout::Node {
    let renderer = cosmic::Renderer::new(Font::default(), 14.0.into());
    let mut tree = Tree::new(element.as_widget());
    element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(Size::ZERO, Size::new(width, height)),
    )
}

// Anchored popups add an autosize wrapper around the same content column.
fn popup_content(mut node: &layout::Node) -> &layout::Node {
    while node.children().len() == 1 {
        node = &node.children()[0];
    }
    node
}

fn popup_app(mode: &str, surface: PopupSurface, width: u32, height: u32) -> AppModel {
    let mut app = AppModel::default();
    app.popup = Some(Id::unique());
    app.popup_surface = Some(surface);
    app.popup_controls_ready = mode != "opening";
    app.toggle_popup_placement = Some(
        crate::app::surfaces::TogglePopupPlacement::NearFocusedWindow {
            rect: crate::services::toplevel::WindowRect {
                x: 0,
                y: 0,
                width: width as i32,
                height: height as i32,
            },
            right_margin: 0,
        },
    );
    if mode != "empty" {
        for _ in 0..40 {
            push_text(&mut app, "A clipboard entry\nWith a full-text preview");
        }
    }
    app.settings_open = mode == "settings";
    app.about_open = mode == "about";
    app.settings_draft = crate::app::model::SettingsDraft::from_settings(&app.settings);
    app.text_overlay_index = (mode == "preview").then_some(0);
    if mode == "image-preview" {
        app.image_preview = Some(crate::app::model::ImagePreview {
            key: (0, 0),
            generation: 1,
            handle: Some(ImageHandle::from_rgba(2, 2, vec![127; 16])),
            failed: false,
        });
    }
    if mode == "search" {
        app.search_query = "no matching entry".into();
    }
    app.recompute_filtered_indices();
    app
}

#[test]
fn text_and_image_rows_keep_the_same_height_on_hover_and_keyboard_focus() {
    let mut app = AppModel::default();
    push_text(&mut app, "Short text");
    push_text(&mut app, &"Long Unicode text 日本語 🦀 ".repeat(20));
    push_text(&mut app, &"語".repeat(80));
    push_text(&mut app, "First line\nSecond line\nThird line");
    push_image(&mut app, "image/png");
    push_image(&mut app, "image/jpeg");
    app.thumbnail_handles
        .insert((0, 0), ImageHandle::from_rgba(2, 2, vec![127; 16]));
    for width in [240.0, 320.0, 520.0] {
        for idx in 0..app.history.len() {
            for (hovered, keyboard_focus) in [
                (None, None),
                (Some(idx), None),
                (None, Some((idx, FocusPart::Pin))),
            ] {
                app.hovered_index = hovered;
                app.keyboard_focus = keyboard_focus;
                let state = RowRenderState::from_app(&app, idx, &app.history[idx]);
                let node = layout_element(history_row(state), width, 400.0);
                assert_eq!(node.bounds().height, HISTORY_ROW_HEIGHT);
            }
        }
    }
    app.thumbnail_handles.clear();
    let node = layout_element(
        history_row(RowRenderState::from_app(&app, 4, &app.history[4])),
        320.0,
        400.0,
    );
    assert_eq!(node.bounds().height, HISTORY_ROW_HEIGHT);
}

#[test]
fn reference_layout_has_top_navigation_full_width_search_and_a_separate_clear_strip() {
    for (surface, width, height) in [
        (PopupSurface::LayerSurface, 360, 580),
        (PopupSurface::LayerSurface, 560, 580),
        (PopupSurface::AnchoredPopup, 360, 1000),
    ] {
        let app = popup_app("history", surface, width, height);
        let node = layout_element(
            super::super::view_window(&app, app.popup.unwrap()),
            width as f32,
            height as f32,
        );
        let content = popup_content(&node);
        assert_eq!(
            content.children().len(),
            4,
            "header, search, Clear strip, list; no main footer"
        );
        let header = &content.children()[0];
        assert_eq!(header.children().len(), 3, "About, spacer, Settings");
        for index in [0, 2] {
            let target = &header.children()[index];
            assert_eq!(target.bounds().size(), Size::new(32.0, 32.0));
            let mut icon = target;
            while icon.children().len() == 1 {
                icon = &icon.children()[0];
            }
            assert_eq!(icon.bounds().size(), Size::new(14.0, 14.0));
        }
        let search = content.children()[1].bounds();
        let strip = &content.children()[2];
        assert_eq!(strip.children().len(), 2, "spacer and Clear chip");
        let clear = strip.children()[1].bounds();
        assert!(clear.width <= 90.0);
        assert!(
            search.width >= width as f32 - 33.0,
            "Clear must not squeeze search"
        );
        assert!(strip.bounds().y >= search.y + search.height + 7.0);
        assert!((clear.x + clear.width - strip.bounds().width).abs() < 1.0);
        assert!((strip.bounds().height - 28.0).abs() < 1.0);
        let list = content.children()[3].bounds();
        assert!(list.y >= strip.bounds().y + strip.bounds().height + 7.0);
        assert!(
            list.height >= 400.0,
            "the reference-sized popup must show more than three rows"
        );
        assert!(list.y + list.height <= content.bounds().height - 23.0);
    }
}

#[test]
fn anchored_history_keeps_reference_height_for_empty_and_short_lists() {
    for count in [0, 1, 40] {
        let mut app = popup_app("history", PopupSurface::AnchoredPopup, 360, 1000);
        app.history.truncate(count);
        app.recompute_filtered_indices();
        let node = layout_element(
            super::super::view_window(&app, app.popup.unwrap()),
            360.0,
            1000.0,
        );
        assert!(
            (node.bounds().height - 580.0).abs() < 1.0,
            "{count} entries"
        );
    }
}

#[test]
fn clear_visibility_never_moves_the_search_or_list() {
    let mut geometry = None;
    for mode in [
        "history",
        "search",
        "preview",
        "image-preview",
        "empty",
        "opening",
    ] {
        let app = popup_app(mode, PopupSurface::LayerSurface, 360, 580);
        let node = layout_element(
            super::super::view_window(&app, app.popup.unwrap()),
            360.0,
            580.0,
        );
        let content = popup_content(&node);
        assert_eq!(content.children().len(), 4);
        let strip = &content.children()[2];
        assert_eq!(
            strip.children().len(),
            if mode == "history" { 2 } else { 1 },
            "{mode}"
        );
        let actual = (
            content.children()[1].bounds().y,
            strip.bounds().y,
            content.children()[3].bounds().y,
        );
        if let Some(expected) = geometry {
            assert_eq!(actual, expected, "{mode}");
        } else {
            geometry = Some(actual);
        }
    }
}

#[test]
fn pages_keep_back_and_apply_visible_and_history_fits_short_surfaces() {
    for (surface, width, height) in [
        (PopupSurface::LayerSurface, 360, 580),
        (PopupSurface::LayerSurface, 560, 400),
        (PopupSurface::LayerSurface, 360, 260),
        (PopupSurface::LayerSurface, 240, 300),
        // The applet helper intentionally autosizes anchored popups using its
        // own 360×1000 limits; constrained-size cases apply to layer surfaces.
        (PopupSurface::AnchoredPopup, 360, 1000),
    ] {
        for mode in [
            "history",
            "settings",
            "about",
            "preview",
            "image-preview",
            "search",
            "empty",
        ] {
            let app = popup_app(mode, surface, width, height);
            let node = layout_element(
                super::super::view_window(&app, app.popup.unwrap()),
                width as f32,
                height as f32,
            );
            let content = popup_content(&node);
            let last = content.children().last().unwrap().bounds();
            if matches!(mode, "settings" | "about") {
                assert!(
                    last.height >= 28.0,
                    "{surface:?}, {mode}: footer controls stay usable"
                );
                let footer = content.children().last().unwrap();
                assert_eq!(
                    footer.children()[0].children().len(),
                    if mode == "settings" { 3 } else { 2 }
                );
            }
            assert!(
                last.y + last.height <= content.bounds().height,
                "{surface:?}, {mode}: content stays inside popup"
            );
            assert!(
                node.bounds().height <= height as f32,
                "{surface:?}, {mode}, {width}×{height}: actual popup height {}",
                node.bounds().height
            );
        }
    }
}

#[test]
fn url_text_and_image_badges_still_emit_copy_on_click() {
    use cosmic::iced::advanced::{Layout, Shell, clipboard};
    use cosmic::iced::{Event, Point, Rectangle, mouse};
    for image in [false, true] {
        let mut app = AppModel::default();
        if image {
            push_image(&mut app, "image/png");
            app.image_dimensions.insert((0, 0), Some((728, 389)));
            app.thumbnail_handles
                .insert((0, 0), ImageHandle::from_rgba(2, 2, vec![127; 16]));
            app.hovered_index = Some(0);
        } else {
            push_text(
                &mut app,
                "https://example.org/this-is-a-long-url-that-wraps-across-several-lines",
            );
        }
        let mut element = history_row(RowRenderState::from_app(&app, 0, &app.history[0]));
        let renderer = cosmic::Renderer::new(Font::default(), 14.0.into());
        let mut tree = Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(328.0, 112.0)),
        );
        let cursor = mouse::Cursor::Available(if image {
            Point::new(24.0, 16.0)
        } else {
            Point::new(80.0, 56.0)
        });
        let mut messages = Vec::new();
        for event in [
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ] {
            element.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(event),
                Layout::new(&node),
                cursor,
                &renderer,
                &mut clipboard::Null,
                &mut Shell::new(&mut messages),
                &Rectangle::with_size(Size::new(328.0, 112.0)),
            );
        }
        assert_eq!(
            messages
                .iter()
                .filter(|message| matches!(message, crate::app::Message::CopyFromHistory(0)))
                .count(),
            1,
            "{image}: visual wrappers must not swallow the copy action"
        );
        assert!(!messages.iter().any(|message| matches!(
            message,
            crate::app::Message::OpenPreview(_)
                | crate::app::Message::TogglePin(_)
                | crate::app::Message::RemoveHistory(_)
        )));
    }
}

#[test]
fn row_actions_do_not_copy_or_activate_when_dragged_off_target() {
    use crate::app::Message;
    use cosmic::iced::advanced::{Layout, Shell, clipboard};
    use cosmic::iced::{Event, Point, Rectangle, mouse};
    for slot in 0..3 {
        for release_inside in [true, false] {
            let mut app = AppModel::default();
            push_text(&mut app, &"Long text ".repeat(30));
            let mut element = history_row(RowRenderState::from_app(&app, 0, &app.history[0]));
            let renderer = cosmic::Renderer::new(Font::default(), 14.0.into());
            let mut tree = Tree::new(element.as_widget());
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, Size::new(328.0, 112.0)),
            );
            let row = popup_content(&node);
            let container = &row.children()[1];
            let column = &container.children()[0];
            let target = column.children()[slot].bounds();
            let point = Point::new(
                container.bounds().x + column.bounds().x + target.center_x(),
                container.bounds().y + column.bounds().y + target.center_y(),
            );
            let release = if release_inside {
                point
            } else {
                Point::new(60.0, 56.0)
            };
            let mut messages = Vec::new();
            for (event, position) in [
                (mouse::Event::ButtonPressed(mouse::Button::Left), point),
                (mouse::Event::ButtonReleased(mouse::Button::Left), release),
            ] {
                element.as_widget_mut().update(
                    &mut tree,
                    &Event::Mouse(event),
                    Layout::new(&node),
                    mouse::Cursor::Available(position),
                    &renderer,
                    &mut clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &Rectangle::with_size(Size::new(328.0, 112.0)),
                );
            }
            assert!(
                !messages
                    .iter()
                    .any(|message| matches!(message, Message::CopyFromHistory(_)))
            );
            let matching = messages
                .iter()
                .filter(|message| match slot {
                    0 => matches!(message, Message::TogglePin(0)),
                    1 => matches!(message, Message::RemoveHistory(0)),
                    _ => matches!(message, Message::OpenPreview(0)),
                })
                .count();
            assert_eq!(matching, usize::from(release_inside));
            assert_eq!(
                messages
                    .iter()
                    .filter(|message| matches!(
                        message,
                        Message::TogglePin(_) | Message::RemoveHistory(_) | Message::OpenPreview(_)
                    ))
                    .count(),
                matching
            );
        }
    }
}

#[test]
fn pin_and_delete_targets_align_across_text_image_and_previewless_rows() {
    let mut app = AppModel::default();
    push_text(&mut app, "Short text");
    push_text(&mut app, &"Long text ".repeat(40));
    push_image(&mut app, "image/png");
    let mut expected = None;
    for idx in 0..app.history.len() {
        let node = layout_element(
            history_row(RowRenderState::from_app(&app, idx, &app.history[idx])),
            328.0,
            112.0,
        );
        let row = popup_content(&node);
        let actions = &row.children()[1].children()[0];
        assert_eq!(
            actions.children().len(),
            3,
            "pin, delete, preview or its reserved slot"
        );
        let centers = (
            actions.children()[0].bounds().center_y(),
            actions.children()[1].bounds().center_y(),
        );
        if let Some(expected) = expected {
            assert_eq!(centers, expected);
        } else {
            expected = Some(centers);
        }
    }
}
