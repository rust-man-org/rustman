use iced::{Background, Border, Color, Shadow};

use crate::ui::theme::Palette;

pub fn method_color(method: &crate::domain::request::HttpMethod) -> Color {
    use crate::domain::request::HttpMethod as M;
    match method {
        M::Get => Palette::GET,
        M::Post => Palette::POST,
        M::Put => Palette::PUT,
        M::Patch => Palette::PATCH,
        M::Delete => Palette::DELETE,
        M::Head | M::Options => Palette::HEAD,
    }
}

// Panels carry no border of their own: iced borders draw on all four sides,
// so neighbouring panels doubled up into thick seams. Dividers come from the
// splitters (`widgets::seam`) and explicit rules instead (mockup: hairlines).

pub fn icon_rail_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::chrome())),
        ..Default::default()
    }
}

pub fn icon_btn_style(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
    active: bool,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
        background: if active {
            Some(Background::Color(Palette::accent_soft()))
        } else if matches!(status, iced::widget::button::Status::Hovered) {
            Some(Background::Color(Palette::hover()))
        } else {
            None
        },
        text_color: Palette::text(),
        border: Border {
            color: if active {
                Color {
                    a: 0.30,
                    ..Palette::accent()
                }
            } else {
                Color::TRANSPARENT
            },
            width: if active { 1.0 } else { 0.0 },
            radius: 10.0.into(),
        },
        ..Default::default()
    }
}

pub fn sidebar_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::surface())),
        ..Default::default()
    }
}

pub fn surface_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::surface())),
        shadow: Shadow::default(),
        ..Default::default()
    }
}

