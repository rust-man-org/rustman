//! Response status chip and meta (mockup `.resp-status`, `.resp-meta`).

use iced::{
    Alignment, Background, Border, Color, Element,
    widget::{container, rich_text, row, span},
};

use crate::{
    domain::response::HttpResponse,
    message::Message,
    ui::{
        components::atoms::tooltip::tooltip,
        theme::{MONO, Palette},
    },
};

/// `.resp-status`: 22px tinted chip with the mono status code.
pub fn chip(resp: &HttpResponse) -> Element<'static, Message> {
    let (fg, bg, code, tip) = if resp.status == 0 {
        (
            Palette::ERROR,
            Palette::error_soft(),
            "ERR".to_owned(),
            "Request failed".to_owned(),
        )
    } else {
        let (fg, bg) = status_colors(resp.status);
        (
            fg,
            bg,
            resp.status.to_string(),
            format!("{} {}", resp.status, resp.status_text),
        )
    };
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..MONO
    };
    let chip = container(iced::widget::text(code).size(11).font(semibold).color(fg))
        .height(22)
        .center_y(22)
        .padding([0, 8])
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    tooltip(chip, tip)
}

/// `.resp-meta`: "212 ms · 1.2 KB", numbers in tx-2, units in tx-3.
pub fn meta(resp: &HttpResponse) -> Element<'static, Message> {
    if resp.status == 0 {
        return row![].into();
    }
    let (dur, dur_unit) = format_duration(resp.duration_ms);
    let (size, size_unit) = format_size(resp.body_size);
    let strong = |s: String| -> iced::widget::text::Span<'static, Message> {
        span(s).color(Palette::text_muted())
    };
    let unit = |s: &'static str| -> iced::widget::text::Span<'static, Message> {
        span(s).color(Palette::text_subtle())
    };
    row![
        rich_text([
            strong(dur),
            unit(dur_unit),
            unit("  ·  "),
            strong(size),
            unit(size_unit),
        ])
        .size(11)
        .font(MONO)
    ]
    .align_y(Alignment::Center)
    .into()
}

fn format_duration(ms: u64) -> (String, &'static str) {
    if ms < 1000 {
        (ms.to_string(), " ms")
    } else {
        (format!("{:.2}", ms as f64 / 1000.0), " s")
    }
}

fn format_size(bytes: usize) -> (String, &'static str) {
    if bytes < 1024 {
        (bytes.to_string(), " B")
    } else if bytes < 1024 * 1024 {
        (format!("{:.1}", bytes as f64 / 1024.0), " KB")
    } else {
        (format!("{:.1}", bytes as f64 / (1024.0 * 1024.0)), " MB")
    }
}

fn status_colors(code: u16) -> (Color, Color) {
    match code {
        200..=299 => (Palette::SUCCESS, Palette::success_soft()),
        300..=399 => (Palette::WARNING, Palette::warning_soft()),
        400..=499 => (Palette::ERROR, Palette::error_soft()),
        500..=599 => (Palette::ERROR, Palette::error_soft()),
        _ => (Palette::text_muted(), Palette::surface_high()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_units_split_from_numbers() {
        assert_eq!(format_duration(212), ("212".to_owned(), " ms"));
        assert_eq!(format_duration(1500), ("1.50".to_owned(), " s"));
        assert_eq!(format_size(1229), ("1.2".to_owned(), " KB"));
        assert_eq!(format_size(12), ("12".to_owned(), " B"));
    }
}
