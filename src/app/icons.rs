use cosmic::widget;
use std::sync::LazyLock;

// Keep the panel indicator monochrome even when an icon theme supplies a raster
// fallback. The applet helper recolors this symbolic SVG for the panel theme.
static PANEL_ICON: LazyLock<widget::icon::Handle> = LazyLock::new(|| {
    widget::icon::from_svg_bytes(&include_bytes!("../../resources/icon.svg")[..]).symbolic(true)
});
static PREVIEW_ICON: LazyLock<widget::icon::Handle> = LazyLock::new(|| {
    widget::icon::from_svg_bytes(&include_bytes!("../../resources/preview-symbolic.svg")[..])
        .symbolic(true)
});
static REMOVE_ICON: LazyLock<widget::icon::Handle> =
    LazyLock::new(|| symbolic_icon("edit-delete-symbolic"));
static PIN_ICON: LazyLock<widget::icon::Handle> = LazyLock::new(|| symbolic_icon("pin-symbolic"));
static SEARCH_ICON: LazyLock<widget::icon::Handle> =
    LazyLock::new(|| symbolic_icon("system-search-symbolic"));
static SETTINGS_ICON: LazyLock<widget::icon::Handle> =
    LazyLock::new(|| symbolic_icon("preferences-system-symbolic"));
static CLOSE_ICON: LazyLock<widget::icon::Handle> =
    LazyLock::new(|| symbolic_icon("window-close-symbolic"));

fn symbolic_icon(name: &str) -> widget::icon::Handle {
    widget::icon::from_name(name).handle()
}

pub fn named_symbolic_icon(name: &str) -> widget::icon::Handle {
    match name {
        "edit-delete-symbolic" => REMOVE_ICON.clone(),
        "pin-symbolic" => PIN_ICON.clone(),
        "system-search-symbolic" => SEARCH_ICON.clone(),
        "preferences-system-symbolic" => SETTINGS_ICON.clone(),
        "window-close-symbolic" => CLOSE_ICON.clone(),
        _ => symbolic_icon(name),
    }
}

pub fn panel_icon() -> widget::icon::Handle {
    PANEL_ICON.clone()
}

pub fn preview_icon() -> widget::icon::Handle {
    PREVIEW_ICON.clone()
}

pub fn remove_icon() -> widget::icon::Handle {
    REMOVE_ICON.clone()
}

pub fn pin_icon() -> widget::icon::Handle {
    PIN_ICON.clone()
}

/// Pinned variant uses the same symbolic icon; button styling controls visual emphasis.
pub fn pin_icon_pinned() -> widget::icon::Handle {
    PIN_ICON.clone()
}

pub fn prewarm_popup_icons() {
    let _ = PANEL_ICON.clone();
    let _ = PREVIEW_ICON.clone();
    let _ = SETTINGS_ICON.clone();
    let _ = CLOSE_ICON.clone();
    let _ = SEARCH_ICON.clone();
    let _ = PIN_ICON.clone();
    let _ = REMOVE_ICON.clone();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_indicators_are_always_recolorable_svgs() {
        for icon in [panel_icon(), preview_icon()] {
            assert!(icon.symbolic);
            assert!(matches!(icon.data, widget::icon::Data::Svg(_)));
        }
    }
}
