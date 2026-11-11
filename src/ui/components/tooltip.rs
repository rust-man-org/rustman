//! Hover tooltips: a raised bubble with the label and, beside it, the
//! shortcut in muted text ("New request  Ctrl T").

use iced::widget::{container, row, text, tooltip::Position};
use iced::{Alignment, Background, Border, Element, Shadow, Vector};

use crate::ui::theme::{RADIUS_SM, SPACE_MD, SPACE_XS, TEXT_SM, TOOLTIP_DELAY, Tokens};

/// Wraps `content` in a tooltip shown below it.
pub fn tooltip<'a, M: 'a>(
    t: &'a Tokens,
    content: impl Into<Element<'a, M>>,
    label: impl Into<String>,
    shortcut: Option<String>,
) -> Element<'a, M> {
    tooltip_at(t, content, label, shortcut, Position::Bottom)
}

pub fn tooltip_at<'a, M: 'a>(
    t: &'a Tokens,
    content: impl Into<Element<'a, M>>,
    label: impl Into<String>,
    shortcut: Option<String>,
    position: Position,
) -> Element<'a, M> {
    let mut body = row![text(label.into()).size(TEXT_SM).color(t.text)]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center);
    if let Some(keys) = shortcut {
        body = body.push(text(keys).size(TEXT_SM).color(t.on_raised.text_muted));
    }
    iced::widget::tooltip(content, container(body).padding([0.0, SPACE_XS / 2.0]), position)
        .gap(SPACE_MD - 2.0)
        .padding(SPACE_MD - 2.0)
        .delay(TOOLTIP_DELAY)
        .snap_within_viewport(true)
        .style(move |_| bubble(t))
        .into()
}

/// The floating-surface style shared by tooltips and menus.
pub fn bubble(t: &Tokens) -> container::Style {
    container::Style {
        background: Some(Background::Color(t.raised)),
        text_color: Some(t.text),
        border: Border {
            color: t.on_raised.border,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        shadow: Shadow {
            color: t.shadow,
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

/// "Ctrl T" style shortcut text for `key` with the platform modifier.
pub fn shortcut(key: &str) -> String {
    format!("{} {key}", crate::ui::theme::MOD_KEY)
}
