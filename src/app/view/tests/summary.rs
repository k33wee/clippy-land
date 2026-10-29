use super::*;

#[test]
fn list_excerpt_keeps_multiline_context_and_bounds_unicode_text() {
    use crate::app::view::summary::history_excerpt;
    assert_eq!(
        history_excerpt("\n First line\n\tSecond   line \nThird line  "),
        "First line Second line Third line"
    );
    assert_eq!(history_excerpt(" \n\t "), "");
    let excerpt = history_excerpt(&"語".repeat(1000));
    assert_eq!(excerpt.chars().count(), 300);
    assert!(excerpt.ends_with('…'));
    assert_eq!(history_excerpt(&"x".repeat(300)), "x".repeat(300));
}

#[test]
fn summarizes_first_nonempty_line() {
    let input = "\n   \n  hello world  \nsecond line";
    assert_eq!(summarize_one_line(input), "hello world");
}

#[test]
fn truncates_long_lines_with_ellipsis() {
    let input = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabc";
    assert_eq!(
        summarize_one_line(input),
        "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyza…"
    );
}

#[test]
fn summarize_with_custom_limit_allows_longer_expansion() {
    let input = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnop";
    assert_eq!(
        summarize_one_line_with_limit(input, 150),
        "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnop"
    );
}

#[test]
fn summarize_with_custom_limit_truncates_to_requested_length() {
    let input = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnop";
    assert_eq!(
        summarize_one_line_with_limit(input, 20),
        "abcdefghijklmnopqrs…"
    );
}

#[test]
fn returns_empty_for_blank_text() {
    assert_eq!(summarize_one_line("\n  \n\t"), "");
}

#[test]
fn image_metadata_uses_fluent_units_and_dimensions() {
    use crate::app::view::summary::image_metadata;
    // Fluent wraps placeables in Unicode bidi isolates; compare visible text.
    fn visible(text: &str) -> String {
        text.chars()
            .filter(|c| !matches!(*c, '\u{2066}'..='\u{2069}'))
            .collect()
    }
    assert_eq!(
        visible(&image_metadata("image/png", 1024, Some((1920, 1080)))),
        "1920 × 1080 · PNG · 1 KiB"
    );
    assert_eq!(
        visible(&image_metadata("image/png", 300, None)),
        "PNG · 300 B"
    );
    assert_eq!(
        visible(&image_metadata("image/png", 1_572_864, None)),
        "PNG · 1.5 MiB"
    );
}
