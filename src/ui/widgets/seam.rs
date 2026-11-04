//! Drawing for splitter handles: the 8px drag area is filled with the panel
//! surface and shows a single 1px divider line (mockup: panels meet at a
//! hairline, no gap). The line turns accent while hovered or dragged.

use iced::advanced::renderer;
use iced::{Background, Rectangle};

use crate::ui::theme::Palette;

/// Where the 1px line sits across the handle's thickness.
#[derive(Clone, Copy)]
pub enum LineAt {
    /// First pixel (handle sits right of / below the panel it belongs to).
    Start,
    Center,
}

/// `vertical`: the handle separates left/right panels (a vertical line).
pub fn draw<R: renderer::Renderer>(
    renderer: &mut R,
    bounds: Rectangle,
    vertical: bool,
    at: LineAt,
    active: bool,
) {
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            ..Default::default()
        },
        Background::Color(Palette::surface()),
    );
    let line = if vertical {
        let x = match at {
            LineAt::Start => bounds.x,
            LineAt::Center => (bounds.x + bounds.width / 2.0).floor(),
        };
        Rectangle {
            x,
            width: 1.0,
            ..bounds
        }
    } else {
        let y = match at {
            LineAt::Start => bounds.y,
            LineAt::Center => (bounds.y + bounds.height / 2.0).floor(),
        };
        Rectangle {
            y,
            height: 1.0,
            ..bounds
        }
    };
    let color = if active {
        Palette::accent()
    } else {
        Palette::border_subtle()
    };
    renderer.fill_quad(
        renderer::Quad {
            bounds: line,
            ..Default::default()
        },
        Background::Color(color),
    );
}
