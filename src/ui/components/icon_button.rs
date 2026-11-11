//! Square, borderless icon buttons. Always pair with a tooltip.

use iced::widget::{Button, Text, container};
use iced::{Background, Border, Length};

use crate::ui::theme::{CONTROL_MD, CONTROL_SM, CONTROL_XL, ICON_LG, ICON_MD, RADIUS_SM, Tokens};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSize {
    /// 24 px, inside bars.
    Small,
    /// 28 px, the tab strip and dialog chrome.
    Medium,
    /// 36 px, next to the URL field.
    Large,
}

impl IconSize {
    pub const fn side(self) -> f32 {
        match self {
            IconSize::Small => CONTROL_SM,
            IconSize::Medium => CONTROL_MD,
            IconSize::Large => CONTROL_XL,
        }
    }

    const fn glyph(self) -> f32 {
        match self {
            IconSize::Large => ICON_LG,
            _ => ICON_MD,
        }
    }
}

/// A ghost icon button; the glyph takes the button's text color, so it
/// brightens on hover.
pub fn icon_button<'a, M: 'a>(t: &'a Tokens, icon: Text<'a>, size: IconSize) -> Button<'a, M> {
    icon_button_toggled(t, icon, size, false)
}

/// An icon button that can show an "on" state (an open menu, a toggle).
pub fn icon_button_toggled<'a, M: 'a>(
    t: &'a Tokens,
    icon: Text<'a>,
    size: IconSize,
    on: bool,
) -> Button<'a, M> {
    iced::widget::button(
        container(icon.size(size.glyph()))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(size.side())
    .height(size.side())
    .padding(0)
    .style(move |_, status| style(t, status, on))
}

pub fn style(t: &Tokens, status: iced::widget::button::Status, on: bool) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    let hovered = on || matches!(status, Status::Hovered | Status::Pressed);
    iced::widget::button::Style {
        background: hovered.then_some(Background::Color(t.hover)),
        text_color: match status {
            Status::Disabled => t.text_subtle,
            _ if hovered => t.text,
            _ => t.text_muted,
        },
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
