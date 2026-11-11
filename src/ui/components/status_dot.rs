//! Colored status text led by a 6 px dot ("● 200 OK").

use iced::widget::{Space, container, row, text};
use iced::{Alignment, Background, Border, Color, Element};

use crate::ui::theme::{DOT, SPACE_MD, TEXT_SM, UI_FONT_MEDIUM};

pub fn dot<'a, M: 'a>(color: Color) -> Element<'a, M> {
    container(Space::new().width(DOT).height(DOT))
        .style(move |_| container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                radius: (DOT / 2.0).into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

pub fn status_text<'a, M: 'a>(label: impl text::IntoFragment<'a>, color: Color) -> Element<'a, M> {
    row![
        dot(color),
        text(label).size(TEXT_SM).font(UI_FONT_MEDIUM).color(color),
    ]
    .spacing(SPACE_MD - 2.0)
    .align_y(Alignment::Center)
    .into()
}
