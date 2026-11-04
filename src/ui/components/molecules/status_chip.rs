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
    let mut content = row![
        Mark::Dot.view(tone, 8.0),
        text(label.into())
            .size(TEXT_SM)
            .color(Palette::text())
            .font(UI_FONT_MEDIUM),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if let Some(detail) = detail {
        content = content.push(text(format!("· {detail}")).size(TEXT_SM).color(Palette::text_muted()));
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
