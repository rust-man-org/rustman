//! Response status chip and meta (mockup `.resp-status`, `.resp-meta`), with
//! Postman-style hover cards for the time and size breakdown.

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Space, column, container, rich_text, row, span, text},
};

use crate::{
    domain::response::HttpResponse,
    message::Message,
    ui::{
        components::atoms::tooltip::tooltip,
        theme::{MONO, Palette, UI_FONT_MEDIUM},
    },
};

/// `.resp-status`: 22px tinted chip, "200 OK" like Postman.
pub fn chip(resp: &HttpResponse) -> Element<'static, Message> {
    let failed = resp.status == 0;
    let (fg, bg) = if failed {
        (Palette::ERROR, Palette::error_soft())
    } else {
        status_colors(resp.status)
    };
    let label = if failed {
        "ERR".to_owned()
    } else {
        format!("{} {}", resp.status, resp.status_text)
            .trim()
            .to_owned()
    };
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..MONO
    };
    let chip = container(text(label).size(11).font(semibold).color(fg))
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
    if failed {
        tooltip(chip, "Request failed".to_owned())
    } else {
        chip.into()
    }
}

/// `.resp-meta`: "212 ms · 1.2 KB", numbers in tx-2, units in tx-3. Hovering
/// the time or the size opens its breakdown card.
pub fn meta(resp: &HttpResponse) -> Element<'static, Message> {
    if resp.status == 0 {
        return row![].into();
    }
    let (dur, dur_unit) = format_duration(resp.duration_ms);
    let (size, size_unit) = format_size(resp.body_size + resp.sizes.response_headers);
    let strong = |s: String| -> iced::widget::text::Span<'static, Message> {
        span(s).color(Palette::text_muted())
    };
    let unit = |s: &'static str| -> iced::widget::text::Span<'static, Message> {
        span(s).color(Palette::text_subtle())
    };
    let time = rich_text([strong(dur), unit(dur_unit)]).size(11).font(MONO);
    let size = rich_text([strong(size), unit(size_unit)])
        .size(11)
        .font(MONO);
    row![
        hover_card(time.into(), time_card(resp)),
        text("·").size(11).color(Palette::text_subtle()),
        hover_card(size.into(), size_card(resp)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

fn hover_card(
    anchor: Element<'static, Message>,
    card: Option<Element<'static, Message>>,
) -> Element<'static, Message> {
    let Some(card) = card else {
        return anchor;
    };
    iced::widget::tooltip(
        container(anchor).padding([2, 2]),
        container(card)
            .width(300)
            .padding(12)
            .style(|_| container::Style {
                background: Some(Background::Color(Palette::surface_high())),
                border: Border {
                    color: Palette::border(),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 12.0,
                },
                ..Default::default()
            }),
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

/// Waterfall of where the time went. `None` for responses without phase
/// timings (e.g. loaded from older history).
fn time_card(resp: &HttpResponse) -> Option<Element<'static, Message>> {
    let t = resp.timing;
    let phases = [
        ("Prepare", t.prepare_us, Palette::text_subtle()),
        ("Waiting (TTFB)", t.wait_us, Palette::accent()),
        ("Download", t.download_us, Palette::SUCCESS),
        ("Process", t.process_us, Palette::text_subtle()),
    ];
    let total: u64 = phases.iter().map(|p| p.1).sum();
    if total == 0 {
        return None;
    }
    let mut rows = column![card_title("Response Time", fmt_us(total))].spacing(6);
    let mut offset = 0u64;
    for (label, us, color) in phases {
        // Waterfall: lead-in, the phase, then the rest, in 1/1000ths.
        let portion = |v: u64| ((v * 1000) / total).min(1000) as u16;
        let (lead, len) = (portion(offset), portion(us).max(4));
        let tail = 1000u16.saturating_sub(lead + len);
        offset += us;
        let bar = row![
            Space::new().width(Length::FillPortion(lead.max(1))),
            container(Space::new())
                .width(Length::FillPortion(len))
                .height(8)
                .style(move |_| container::Style {
                    background: Some(Background::Color(color)),
                    border: Border {
                        radius: 2.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            Space::new().width(Length::FillPortion(tail.max(1))),
        ]
        .width(Length::Fill)
        .align_y(Alignment::Center);
        rows = rows.push(
            row![
                text(label).size(11).color(Palette::text_muted()).width(104),
                bar,
                text(fmt_us(us))
                    .size(11)
                    .font(MONO)
                    .color(Palette::text_muted())
                    .width(72)
                    .align_x(iced::alignment::Horizontal::Right),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    rows = rows.push(
        text("Waiting includes DNS lookup, connect and TLS handshake.")
            .size(10)
            .color(Palette::text_subtle()),
    );
    Some(rows.into())
}

/// Response and request sizes, headers and body. `None` when the header
/// sizes were not recorded (older history).
fn size_card(resp: &HttpResponse) -> Option<Element<'static, Message>> {
    let s = resp.sizes;
    if s.response_headers == 0 {
        return None;
    }
    let line = |label: &'static str, bytes: usize| -> Element<'static, Message> {
        row![
            text(label).size(11).color(Palette::text_muted()),
            Space::new().width(Length::Fill),
            text(fmt_bytes(bytes))
                .size(11)
                .font(MONO)
                .color(Palette::text_muted()),
        ]
        .padding(iced::padding::left(14))
        .into()
    };
    let rule = container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::border_subtle())),
            ..Default::default()
        });
    Some(
        column![
            card_title(
                "Response Size",
                fmt_bytes(s.response_headers + resp.body_size)
            ),
            line("Headers", s.response_headers),
            line("Body", resp.body_size),
            rule,
            card_title(
                "Request Size",
                fmt_bytes(s.request_headers + s.request_body)
            ),
            line("Headers", s.request_headers),
            line("Body", s.request_body),
        ]
        .spacing(6)
        .into(),
    )
}

fn card_title(label: &'static str, value: String) -> Element<'static, Message> {
    row![
        text(label)
            .size(12)
            .font(UI_FONT_MEDIUM)
            .color(Palette::text()),
        Space::new().width(Length::Fill),
        text(value).size(12).font(MONO).color(Palette::text()),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn fmt_us(us: u64) -> String {
    format!("{:.2} ms", us as f64 / 1000.0)
}

fn fmt_bytes(bytes: usize) -> String {
    let (n, unit) = format_size(bytes);
    format!("{n}{unit}")
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
        assert_eq!(fmt_us(778_440), "778.44 ms");
    }

    #[test]
    fn cards_only_with_recorded_breakdown() {
        let mut resp = HttpResponse {
            status: 200,
            ..Default::default()
        };
        assert!(time_card(&resp).is_none());
        assert!(size_card(&resp).is_none());
        resp.timing.wait_us = 1000;
        resp.sizes.response_headers = 120;
        assert!(time_card(&resp).is_some());
        assert!(size_card(&resp).is_some());
    }
}
