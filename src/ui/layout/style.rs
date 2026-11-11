use iced::{Background, Border};

use crate::ui::theme::{Palette, RADIUS_SM};

/// The whole window is one flat canvas: the rail, sidebar, tab strip, URL
/// row, both panes and the status bar all use this, separated by hairlines
/// rather than by darker bands.
pub fn canvas_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::tokens().canvas)),
        ..Default::default()
    }
}

pub fn icon_btn_style(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
    active: bool,
) -> iced::widget::button::Style {
    let t = Palette::tokens();
    iced::widget::button::Style {
        background: if active {
            Some(Background::Color(t.accent_soft))
        } else if matches!(status, iced::widget::button::Status::Hovered) {
            Some(Background::Color(t.hover))
        } else {
            None
        },
        text_color: t.text,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
