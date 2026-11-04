//! StatusChip molecule: a clickable "● Label · detail" chip, e.g. the
//! sync state in the header. Text always accompanies the dot.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{button, container, row, text},
};

use crate::ui::{
    components::{Tone, atoms::Mark},
    theme::{Palette, TEXT_SM, UI_FONT_MEDIUM},
};

pub fn status_chip<'a, M: Clone + 'a>(
    tone: Tone,
    label: impl Into<String>,
    detail: Option<String>,
    msg: M,
) -> Element<'a, M> {
    // Mockup `.sync-chip .dot`: 8px dot inside a 3px tinted ring.
    let ring = iced::Color {
        a: 0.22,
        ..tone.color()
    };
    let dot = container(Mark::Dot.view(tone, 8.0))
        .width(14)
        .height(14)
        .center_x(14)
        .center_y(14)
        .style(move |_| container::Style {
            background: Some(Background::Color(ring)),
            border: Border {
                radius: 7.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    let mut content = row![
        dot,
        text(label.into())
            .size(TEXT_SM)
            .color(Palette::text_muted())
            .font(UI_FONT_MEDIUM),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if let Some(detail) = detail {
        content = content.push(text("·").size(TEXT_SM).color(Palette::text_muted()));
        content = content.push(
            text(detail)
                .size(TEXT_SM)
                .color(Palette::text())
                .font(UI_FONT_MEDIUM),
        );
    }
    // A fixed-height button top-aligns its content; center it explicitly.
    button(container(content).center_y(Length::Fill))
        .on_press(msg)
        .height(32)
        .padding([0, 12])
        .style(|_t, s| button::Style {
            background: matches!(s, button::Status::Hovered)
                .then(|| Background::Color(Palette::hover())),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
