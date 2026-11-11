//! A bordered `surface` card (the error card, the start card).

use iced::widget::container;
use iced::{Background, Border, Element};

use crate::ui::theme::{RADIUS_LG, SPACE_2XL, Tokens};

pub fn card<'a, M: 'a>(t: &'a Tokens, content: impl Into<Element<'a, M>>) -> container::Container<'a, M> {
    container(content).padding(SPACE_2XL).style(move |_| style(t))
}

pub fn style(t: &Tokens) -> container::Style {
    container::Style {
        background: Some(Background::Color(t.surface)),
        text_color: Some(t.text),
        border: Border {
            color: t.border,
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        ..Default::default()
    }
}
