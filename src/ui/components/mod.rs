//! Token-driven building blocks shared by every view. Views compose these
//! instead of defining their own styles, so a theme is one `Tokens` value passed by reference.

pub mod button;
pub mod card;
pub mod field;
pub mod hover_box;
pub mod icon_button;
pub mod kbd;
pub mod kv_table;
pub mod menu;
pub mod progress;
pub mod scroll;
pub mod status_dot;
pub mod tab_bar;
pub mod tooltip;

pub use icon_button::icon_button;
pub use tooltip::tooltip;

/// A 1 px horizontal hairline in the theme's border color.
/// Which surface a component sits on. Raised surfaces (menus, dialogs, the
/// palette) use lifted borders, muted text and row tints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Surface {
    #[default]
    Canvas,
    Raised,
}

impl Surface {
    pub fn border(self, t: &crate::ui::theme::Tokens) -> iced::Color {
        match self {
            Surface::Canvas => t.border,
            Surface::Raised => t.on_raised.border,
        }
    }

    pub fn text_muted(self, t: &crate::ui::theme::Tokens) -> iced::Color {
        match self {
            Surface::Canvas => t.text_muted,
            Surface::Raised => t.on_raised.text_muted,
        }
    }

    pub fn hover(self, t: &crate::ui::theme::Tokens) -> iced::Color {
        match self {
            Surface::Canvas => t.hover,
            Surface::Raised => t.on_raised.hover,
        }
    }
}

/// Drawn as a filled box rather than a rule: rule snapping lands 1 px low.
pub fn hairline<'a, M: 'a>(t: &'a crate::ui::theme::Tokens) -> iced::Element<'a, M> {
    line(t.border, iced::Length::Fill, iced::Length::Fixed(crate::ui::theme::HAIRLINE))
}

fn line<'a, M: 'a>(color: iced::Color, width: iced::Length, height: iced::Length) -> iced::Element<'a, M> {
    iced::widget::container(iced::widget::Space::new().width(width).height(height))
        .style(move |_| iced::widget::container::Style {
            background: Some(color.into()),
            ..Default::default()
        })
        .into()
}

/// A 1 px vertical hairline filling the available height.
pub fn vhairline<'a, M: 'a>(t: &'a crate::ui::theme::Tokens) -> iced::Element<'a, M> {
    line(t.border, iced::Length::Fixed(crate::ui::theme::HAIRLINE), iced::Length::Fill)
}

/// A 1 px vertical separator, `height` tall.
pub fn vsep<'a, M: 'a>(t: &'a crate::ui::theme::Tokens, height: f32) -> iced::Element<'a, M> {
    iced::widget::container(iced::widget::Space::new().width(crate::ui::theme::HAIRLINE).height(height))
        .style(move |_| iced::widget::container::Style {
            background: Some(t.border.into()),
            ..Default::default()
        })
        .into()
}
