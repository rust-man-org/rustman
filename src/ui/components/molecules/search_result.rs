//! Search result row: stable height, method/icon, title, path, safe detail, kind.
use crate::ui::theme::{MONO, Palette};
use iced::{
    Background, Border, Color, Element, Length, Shadow, Vector,
    widget::{button, column, container, row, rule, text},
};

pub fn search_result<'a, M: Clone + 'a>(
    leading: Element<'a, M>,
    title: Element<'a, M>,
    path: String,
    detail: String,
    kind: &'static str,
    selected: bool,
    on_press: M,
) -> Element<'a, M> {
    let tag = container(text(kind).size(10).color(Palette::text_muted()))
        .padding([2, 7])
        .style(|_| container::Style {
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        });
    button(
        row![
            container(leading).width(56).padding([3, 0]),
            column![
                row![title, tag].spacing(8).align_y(iced::Alignment::Center),
                text(path)
                    .size(11)
                    .color(Palette::text_muted())
                    .wrapping(iced::widget::text::Wrapping::None),
                container(
                    text(detail)
                        .size(10)
                        .font(MONO)
                        .color(Palette::text_muted())
                        .wrapping(iced::widget::text::Wrapping::None)
                )
                .width(Length::Fill)
                .padding([3, 7])
                .style(|_| container::Style {
                    background: Some(Background::Color(Palette::background())),
                    border: Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ]
            .spacing(3)
            .width(Length::Fill),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Start),
    )
    .on_press(on_press)
    .width(Length::Fill)
    .height(80)
    .padding([9, 10])
    .style(move |_, status| button::Style {
        background: (selected || matches!(status, button::Status::Hovered))
            .then(|| Background::Color(Palette::hover())),
        text_color: Palette::text(),
        border: Border {
            radius: 5.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// A search popover has a reading surface, with subdued header/footer chrome.
/// Do not use the raised context-menu token for the entire results surface.
pub fn search_panel<'a, M: 'a>(
    header: Element<'a, M>,
    results: Element<'a, M>,
    footer: Element<'a, M>,
) -> Element<'a, M> {
    let chrome = |_: &iced::Theme| container::Style {
        background: Some(Background::Color(Palette::background())),
        ..Default::default()
    };
    container(column![
        container(header)
            .padding([12, 14])
            .width(Length::Fill)
            .style(chrome),
        rule::horizontal(1).style(crate::ui::styles::divider),
        container(results).padding([4, 6]).width(Length::Fill),
        rule::horizontal(1).style(crate::ui::styles::divider),
        container(footer)
            .padding([10, 14])
            .width(Length::Fill)
            .style(chrome),
    ])
    .width(568)
    .clip(true)
    .style(|_| container::Style {
        background: Some(Background::Color(Palette::surface())),
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow {
            color: Color {
                a: 0.14,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 6.0),
            blur_radius: 20.0,
        },
        ..Default::default()
    })
    .into()
}
