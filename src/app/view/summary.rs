use crate::fl;

pub(super) fn image_metadata(
    mime: &str,
    bytes_len: usize,
    dimensions: Option<(u32, u32)>,
) -> String {
    let format = mime.strip_prefix("image/").unwrap_or(mime).to_uppercase();
    let size = if bytes_len >= 1_048_576 {
        fl!(
            "size-mib",
            size = format!("{:.1}", bytes_len as f64 / 1_048_576.0)
        )
    } else if bytes_len >= 1024 {
        fl!("size-kib", size = bytes_len.div_ceil(1024).to_string())
    } else {
        fl!("size-bytes", size = bytes_len.to_string())
    };
    match dimensions {
        Some((width, height)) => fl!(
            "image-metadata",
            width = width,
            height = height,
            format = format,
            size = size
        ),
        None => fl!("image-metadata-compact", format = format, size = size),
    }
}

pub(crate) const DEFAULT_MAX_CHARS: usize = 80;
pub(crate) const EXPANDED_MAX_CHARS: usize = 300;

/// A bounded multi-line excerpt for the fixed-height list, without changing the
/// original clipboard text or the existing preview-availability rules.
pub(super) fn history_excerpt(text: &str) -> String {
    let mut excerpt = String::new();
    let mut previous_space = true;
    let mut count = 0;
    for character in text.chars() {
        let character = if character.is_whitespace() {
            ' '
        } else {
            character
        };
        if character == ' ' && previous_space {
            continue;
        }
        if count == EXPANDED_MAX_CHARS {
            excerpt.pop();
            excerpt.push('…');
            break;
        }
        excerpt.push(character);
        previous_space = character == ' ';
        count += 1;
    }
    excerpt.trim_end().to_string()
}

pub(super) fn summarize_one_line(text: &str) -> String {
    summarize_one_line_with_limit(text, DEFAULT_MAX_CHARS)
}

pub(crate) fn text_overlay_available(text: &str) -> bool {
    let collapsed = summarize_one_line(text);
    let expanded = summarize_one_line_with_limit(text, EXPANDED_MAX_CHARS);
    let has_more_nonempty_lines = text
        .lines()
        .skip_while(|line| line.trim().is_empty())
        .skip(1)
        .any(|line| !line.trim().is_empty());

    collapsed != expanded || has_more_nonempty_lines
}

pub(super) fn summarize_one_line_with_limit(text: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(1);
    let mut line = text
        .lines()
        .map(|line| line.trim_start())
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .trim_end()
        .to_string();
    if line.chars().count() > max_chars {
        line = line.chars().take(max_chars - 1).collect::<String>();
        line.push('…');
    }
    line
}
