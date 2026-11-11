//! A keycap for shortcut hints ("Ctrl Enter", "↵").

use iced::widget::{container, text};
use iced::{Background, Border, Element, Length, Padding};

use crate::ui::components::Surface;
use crate::ui::theme::{CONTROL_SM, RADIUS_XS, SPACE_XS, TEXT_CAPTION, Tokens};

/// Keycap height: a step below the compact control height.
const HEIGHT: f32 = CONTROL_SM - SPACE_XS;

/// What a keycap shows: key names, or one icon glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCap<'a> {
    Text(&'a str),
    Icon(char),
}

/// On the canvas a keycap is filled with the canvas color (so it reads on
/// fields too); on a raised surface it is outline only.
pub fn kbd<'a, M: 'a>(t: &'a Tokens, keys: KeyCap<'a>, surface: Surface) -> Element<'a, M> {
    let label = match keys {
        KeyCap::Text(s) => text(s),
        KeyCap::Icon(c) => text(c).font(crate::ui::icons::ICON_FONT),
    };
    container(label.size(TEXT_CAPTION).color(surface.text_muted(t)))
        .height(HEIGHT)
        .center_y(Length::Fixed(HEIGHT))
        .padding(Padding::from([0.0, SPACE_XS + 2.0]))
        .style(move |_| style(t, surface))
        .into()
}

fn style(t: &Tokens, surface: Surface) -> container::Style {
    container::Style {
        background: (surface == Surface::Canvas).then_some(Background::Color(t.canvas)),
        border: Border {
            color: surface.border(t),
            width: 1.0,
            radius: RADIUS_XS.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raised_keycaps_are_outline_only_with_lifted_colors() {
        let t = Tokens::for_theme(3);
        let raised = style(t, Surface::Raised);
        assert!(raised.background.is_none());
        assert_eq!(raised.border.color, t.on_raised.border);
        assert_eq!(Surface::Raised.text_muted(t), t.on_raised.text_muted);
        let canvas = style(t, Surface::Canvas);
        assert_eq!(canvas.background, Some(Background::Color(t.canvas)));
        assert_eq!(canvas.border.color, t.border);
    }
}
