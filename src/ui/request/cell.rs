//! Table cells that refuse to hold unbounded text.
//!
//! iced's `text_input` shapes its whole value on every layout (plus a fresh
//! `String` copy per layout). A multi-MB value — e.g. a markdown doc pasted
//! via `curl -F` — freezes the app, and embedded newlines corrupt its
//! single-line cursor math (this crate builds with `panic = "abort"`, so any
//! panic there is instant death, not an error dialog).
//!
//! So cells show a live input only for short single-line text; anything
//! bigger renders as a capped preview row (leading line + size + capped
//! hover peek) while state keeps the full value for send/save. The `width`,
//! `size` and `padding` mirror a plain `text_input`, so small values render
//! byte-identically to before.

use iced::{
    Element, Length,
    widget::{container, row, text, text_input},
};

use crate::{consts::INLINE_LIMIT, message::Message, ui::theme::Palette};

/// Chars of the leading line shown in a preview.
const PREVIEW_CHARS: usize = 200;

/// True when `value` is safe to edit in a single-line input: short enough
/// that per-layout shaping stays trivial, and with no line breaks to corrupt
/// the input's single-line cursor math. Length first — O(1) — so huge values
/// skip the byte scan entirely.
pub fn is_inline(value: &str) -> bool {
    value.len() <= INLINE_LIMIT && !value.bytes().any(|b| b == b'\n' || b == b'\r')
}

pub fn text_cell<'a>(
    placeholder: &'static str,
    value: &'a str,
    size: f32,
    padding: impl Into<iced::Padding>,
    width: impl Into<Length>,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    if is_inline(value) {
        return text_input(placeholder, value)
            .on_input(on_input)
            .size(size)
            .padding(padding)
            .width(width)
            .font(crate::ui::theme::MONO)
            .style(crate::ui::styles::kv_input)
            .into();
    }
    // Preview path: bounded head line + size. The full value never enters a
    // widget — head extraction is O(200 chars), the tip slice O(1).
    let mut head: String = value.chars().take(PREVIEW_CHARS).collect();
    if let Some(nl) = head.find(['\n', '\r']) {
        head.truncate(nl);
    }
    head.push('…');
    container(
        row![
            container(
                text(head)
                    .size(size)
                    .color(Palette::text())
                    .width(Length::Fill)
            )
            .width(Length::Fill),
            text(human_size(value.len()))
                .size(10)
                .color(Palette::text_subtle()),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 2])
    .width(width)
    .into()
}

pub(crate) fn human_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    }
}

#[cfg(test)]
mod cell_tests {
    use super::*;

    #[test]
    fn short_single_line_values_stay_editable() {
        assert!(is_inline(""));
        assert!(is_inline("hello"));
        assert!(is_inline(&"x".repeat(INLINE_LIMIT)));
    }

    #[test]
    fn long_or_multiline_values_do_not() {
        assert!(!is_inline(&"x".repeat(INLINE_LIMIT + 1)));
        assert!(!is_inline("a\nb"));
        assert!(!is_inline("a\rb"));
        // A huge value must not pay for a full scan: length check first.
        assert!(!is_inline(&"y".repeat(10_000_000)));
    }

    #[test]
    fn preview_head_is_first_line_capped() {
        let mut head: String = "line one\nline two\nline three".chars().take(200).collect();
        if let Some(nl) = head.find(['\n', '\r']) {
            head.truncate(nl);
        }
        assert_eq!(head, "line one");
    }

    #[test]
    fn preview_never_splits_utf8() {
        // Emoji straddling the tip boundary floors to a char boundary.
        use crate::consts::HOVER_TIP_BYTES;
        use crate::services::format::head_capped;
        let value = "a".repeat(HOVER_TIP_BYTES - 1) + "€";
        let tip = head_capped(&value, HOVER_TIP_BYTES);
        assert_eq!(tip, "a".repeat(HOVER_TIP_BYTES - 1));
        // Head over emoji is whole chars only.
        let head: String = "🎉".repeat(300).chars().take(200).collect();
        assert_eq!(head.chars().count(), 200);
    }

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(5_242_880), "5.0 MB");
    }
}
