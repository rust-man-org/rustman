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

use crate::{
    consts::INLINE_LIMIT,
    message::Message,
    ui::theme::Palette,
};

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
            .style(crate::ui::styles::cell_input)
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
            container(text(head).size(size).color(Palette::text()).width(Length::Fill))
                .width(Length::Fill),
            text(rustman_core::text::human_size(value.len()))
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

/// A value too large for an inline input: its first line, cut, and its size.
pub fn preview<'a, M: 'a>(value: &str, t: &'a crate::ui::theme::Tokens) -> Element<'a, M> {
    let mut head: String = value.chars().take(PREVIEW_CHARS).collect();
    if let Some(nl) = head.find(['\n', '\r']) {
        head.truncate(nl);
    }
    head.push('…');
    row![
        text(head)
            .size(crate::ui::theme::TEXT_SM)
            .font(crate::ui::theme::MONO)
            .color(t.text)
            .width(Length::Fill),
        text(rustman_core::text::human_size(value.len()))
            .size(crate::ui::theme::TEXT_CAPTION)
            .color(t.text_subtle),
    ]
    .spacing(crate::ui::theme::SPACE_MD)
    .padding([0.0, crate::ui::theme::SPACE_MD])
    .align_y(iced::Alignment::Center)
    .into()
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
}
