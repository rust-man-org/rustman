//! Draws an HTML response body with ordinary Iced widgets.
//!
//! The counterpart to `domain::html`: that module turns a response body into a
//! document model, this one turns the model into widgets. Together they replace
//! the native webview that used to render HTML responses (WebKitGTK / WebView2
//! / WKWebView overlaid on the response panel), so previewing HTML needs no
//! native dependency and behaves the same everywhere — including native
//! Wayland, where a child webview cannot attach at all.

use iced::font::{Style as FontStyle, Weight};
use iced::widget::text::{Rich, Span};
use iced::widget::{column, container, row, scrollable, table, text, Space};
use iced::{Background, Border, Color, Element, Font, Length, Padding};

use crate::domain::html::{Block, Document, Inline, List, Table};
use crate::message::{AppMsg, Message};
use crate::ui::theme::{Palette, MONO, TEXT_LG, TEXT_MD, TEXT_SM, UI_FONT};

/// Draws `doc` to fill the response body panel.
pub fn view(doc: &Document) -> Element<'_, Message> {
    if doc.blocks.is_empty() {
        return container(
            text("This response has nothing to render.").size(TEXT_SM).color(Palette::text_subtle()),
        )
        .padding([14, 18])
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    scrollable(
        container(column(doc.blocks.iter().map(block).collect::<Vec<_>>()).spacing(10))
            .padding([14, 18])
            .width(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}

fn block(block: &Block) -> Element<'_, Message> {
    match block {
        Block::Heading { level, spans } => {
            let size = match level {
                1 => 21.0,
                2 => 18.0,
                3 => 16.0,
                4 => TEXT_LG,
                _ => TEXT_MD,
            };
            let color = if *level <= 3 { Palette::text() } else { Palette::text_muted() };
            rich(spans, size, bold_font(), color)
        }
        Block::Paragraph(spans) => rich(spans, TEXT_MD, UI_FONT, Palette::text()),
        Block::Code(code) => container(
            text(code.as_str()).font(MONO).size(TEXT_SM).color(Palette::text()),
        )
        .padding(Padding::from([8.0, 10.0]))
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface_high())),
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into(),
        Block::List(list) => list_view(list),
        Block::Quote(blocks) => quote_view(blocks),
        Block::Table(data) => table_view(data),
        Block::Rule => container(Space::new())
            .height(Length::Fixed(1.0))
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Palette::border_subtle())),
                ..Default::default()
            })
            .into(),
    }
}

fn list_view(list: &List) -> Element<'_, Message> {
    let items = list.items.iter().enumerate().map(|(index, blocks)| {
        let marker = if list.ordered {
            format!("{}.", list.start + index as u64)
        } else {
            "•".to_owned()
        };
        row![
            container(text(marker).size(TEXT_MD).color(Palette::text_subtle()))
                .width(Length::Fixed(22.0)),
            column(blocks.iter().map(block).collect::<Vec<_>>()).spacing(6).width(Length::Fill),
        ]
        .spacing(4)
        .width(Length::Fill)
        .into()
    });
    column(items.collect::<Vec<_>>()).spacing(5).width(Length::Fill).into()
}

fn quote_view(blocks: &[Block]) -> Element<'_, Message> {
    row![
        container(Space::new())
            .width(Length::Fixed(3.0))
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Palette::accent_soft())),
                ..Default::default()
            }),
        column(blocks.iter().map(block).collect::<Vec<_>>()).spacing(6).width(Length::Fill),
    ]
    .spacing(10)
    .width(Length::Fill)
    .into()
}

fn table_view(data: &Table) -> Element<'_, Message> {
    // Without a header row (`<th>`, or anything inside `<thead>`) the table is
    // just rows of cells; there is nothing to pin to the top.
    if data.headers.is_empty() {
        let rows: Vec<Element<Message>> = data
            .rows
            .iter()
            .map(|cells| {
                row(cells
                    .iter()
                    .map(|cell| {
                        container(rich(cell, TEXT_SM, UI_FONT, Palette::text()))
                            .padding([4, 8])
                            .into()
                    })
                    .collect::<Vec<Element<Message>>>())
                .width(Length::Fill)
                .into()
            })
            .collect();
        return column(rows).spacing(1).width(Length::Fill).into();
    }

    let columns = (0..data.headers.len()).map(|index| {
        table::column(
            container(
                rich(
                    &data.headers[index],
                    TEXT_SM,
                    bold_font(),
                    Palette::text_muted(),
                ),
            )
            .padding([4, 8]),
            move |cells: &Vec<Vec<Inline>>| {
                let cell = cells.get(index).map_or(&[][..], Vec::as_slice);
                container(rich(cell, TEXT_SM, UI_FONT, Palette::text())).padding([4, 8])
            },
        )
        .width(Length::Fill)
    });

    table::table(columns, data.rows.iter())
        .width(Length::Fill)
        .padding_x(0.0)
        .padding_y(2.0)
        .into()
}

/// Builds a paragraph of rich text, with links wired to the OS opener.
fn rich<'a>(runs: &'a [Inline], size: f32, font: Font, color: Color) -> Element<'a, Message> {
    let spans: Vec<Span<'a, String, Font>> = runs
        .iter()
        .filter(|run| !run.text.is_empty())
        .map(|run| {
            let mut font = font;
            if run.style.bold {
                font.weight = Weight::Bold;
            }
            if run.style.italic {
                font.style = FontStyle::Italic;
            }
            if run.style.code {
                font.family = MONO.family;
            }
            let mut span = Span::new(run.text.as_str())
                .font(font)
                .size(size)
                .color(color)
                .underline(run.style.underline)
                .strikethrough(run.style.strike);
            if run.style.code {
                span = span.background(Palette::surface_high()).padding([1, 3]);
            }
            match &run.link {
                Some(url) => span
                    .link(url.clone())
                    .color(Palette::accent())
                    .underline(true),
                None => span,
            }
        })
        .collect();

    Rich::with_spans(spans)
        .size(size)
        .width(Length::Fill)
        .on_link_click(|url: String| Message::App(AppMsg::OpenUrl(url)))
        .into()
}

fn bold_font() -> Font {
    Font { weight: Weight::Bold, ..UI_FONT }
}
