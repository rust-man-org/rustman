//! Button atom: one place for every variant, state and size.

use iced::{
    Background, Border, Color, Element,
    widget::{button, text},
};

use super::tooltip::tooltip;
use crate::ui::theme::{Palette, TEXT_SM};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// Filled accent; one per area.
    Primary,
    /// Bordered, on surface.
    #[default]
    Secondary,
    /// Text only until hovered.
    Ghost,
}

/// Builder:
/// `Button::new("Discard").on_press(msg).into()`,
/// `Button::new("Save").primary().on_press_maybe(None).tooltip("Why")`.
/// Without a message the button renders disabled.
pub struct Button<M> {
    label: String,
    variant: Variant,
    msg: Option<M>,
    tip: Option<String>,
}

impl<M> Button<M> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: Variant::default(),
            msg: None,
            tip: None,
        }
    }

    pub fn primary(mut self) -> Self {
        self.variant = Variant::Primary;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = Variant::Ghost;
        self
    }

    pub fn on_press(mut self, msg: M) -> Self {
        self.msg = Some(msg);
        self
    }

    pub fn on_press_maybe(mut self, msg: Option<M>) -> Self {
        self.msg = msg;
        self
    }

    /// Hover text; use it to explain a disabled button.
    pub fn tooltip(mut self, tip: impl Into<String>) -> Self {
        self.tip = Some(tip.into());
        self
    }
}

impl<'a, M: Clone + 'a> From<Button<M>> for Element<'a, M> {
    fn from(b: Button<M>) -> Self {
        let enabled = b.msg.is_some();
        let variant = b.variant;
        let fg = foreground(variant, enabled);
        let widget = button(text(b.label).size(TEXT_SM).color(fg))
            .on_press_maybe(b.msg)
            .padding([4, 10])
            .style(move |_t, status| {
                let hovered = enabled && matches!(status, button::Status::Hovered);
                button::Style {
                    background: background(variant, enabled, hovered).map(Background::Color),
                    text_color: fg,
                    border: Border {
                        color: match variant {
                            Variant::Secondary => Palette::border(),
                            _ => Color::TRANSPARENT,
                        },
                        width: 1.0,
                        radius: 5.0.into(),
                    },
                    ..Default::default()
                }
            });
        match b.tip {
            Some(tip) => tooltip(widget, tip),
            None => widget.into(),
        }
    }
}

fn foreground(variant: Variant, enabled: bool) -> Color {
    match (variant, enabled) {
        (Variant::Primary, true) => Color::WHITE,
        (Variant::Primary, false) => Color {
            a: 0.75,
            ..Color::WHITE
        },
        (Variant::Secondary, true) => Palette::text(),
        (Variant::Ghost, true) => Palette::text_muted(),
        (_, false) => Palette::text_subtle(),
    }
}

fn background(variant: Variant, enabled: bool, hovered: bool) -> Option<Color> {
    match variant {
        Variant::Primary if !enabled => Some(Color {
            a: 0.45,
            ..Palette::accent()
        }),
        Variant::Primary if hovered => Some(Palette::accent_hover()),
        Variant::Primary => Some(Palette::accent()),
        _ if hovered => Some(Palette::hover()),
        Variant::Secondary => Some(Palette::surface()),
        Variant::Ghost => None,
    }
}
