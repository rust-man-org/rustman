//! The one scrollable look: no permanent bar and no reserved gutter. A thin
//! scroller floats over the content's edge while the pointer is over the
//! area, so the content keeps its full width.
//!
//! iced gives a style no clock, so the scroller appears and hides with the
//! hover instead of fading.

use iced::widget::scrollable::{self, AutoScroll, Direction, Rail, Scrollable, Scrollbar, Scroller, Status};
use iced::{Background, Border, Color, Element, Shadow, Theme};

use crate::ui::theme::Tokens;

/// Width of the scroller, and of the strip it captures the pointer in.
pub const SCROLLER_WIDTH: f32 = 4.0;
/// Gap between the scroller and the scrollable's edge.
const INSET: f32 = 2.0;
/// Opacity of the scroller while the pointer is over the area.
const AREA_HOVER_ALPHA: f32 = 0.6;

pub fn vertical<'a, M: 'a>(t: &'a Tokens, content: impl Into<Element<'a, M>>) -> Scrollable<'a, M> {
    styled(t, content, Direction::Vertical(bar()))
}

/// Scrolls with the wheel and trackpad but never draws a bar, not even on
/// hover, and reserves no gutter.
pub fn hidden<'a, M: 'a>(t: &'a Tokens, content: impl Into<Element<'a, M>>) -> Scrollable<'a, M> {
    let none = Scrollbar::new().width(0.0).scroller_width(0.0).margin(0.0);
    styled(t, content, Direction::Vertical(none))
}

pub fn both<'a, M: 'a>(t: &'a Tokens, content: impl Into<Element<'a, M>>) -> Scrollable<'a, M> {
    styled(t, content, Direction::Both { vertical: bar(), horizontal: bar() })
}

fn styled<'a, M: 'a>(t: &'a Tokens, content: impl Into<Element<'a, M>>, direction: Direction) -> Scrollable<'a, M> {
    Scrollable::with_direction(content, direction).style(move |_: &Theme, status| style(t, status))
}

fn bar() -> Scrollbar {
    Scrollbar::new().width(SCROLLER_WIDTH).scroller_width(SCROLLER_WIDTH).margin(INSET)
}

/// Invisible at rest, muted over the area, full strength on the scroller
/// itself and while dragging it.
pub fn style(t: &Tokens, status: Status) -> scrollable::Style {
    let (vertical, horizontal) = match status {
        Status::Active { .. } => (0.0, 0.0),
        Status::Hovered {
            is_vertical_scrollbar_hovered,
            is_horizontal_scrollbar_hovered,
            ..
        } => (alpha(is_vertical_scrollbar_hovered), alpha(is_horizontal_scrollbar_hovered)),
        Status::Dragged {
            is_vertical_scrollbar_dragged,
            is_horizontal_scrollbar_dragged,
            ..
        } => (alpha(is_vertical_scrollbar_dragged), alpha(is_horizontal_scrollbar_dragged)),
    };
    scrollable::Style {
        container: iced::widget::container::Style::default(),
        vertical_rail: rail(t.border_strong.scale_alpha(vertical)),
        horizontal_rail: rail(t.border_strong.scale_alpha(horizontal)),
        gap: None,
        auto_scroll: AutoScroll {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            shadow: Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}

fn alpha(engaged: bool) -> f32 {
    if engaged { 1.0 } else { AREA_HOVER_ALPHA }
}

fn rail(scroller: Color) -> Rail {
    Rail {
        background: None,
        border: Border::default(),
        scroller: Scroller {
            background: Background::Color(scroller),
            border: Border {
                radius: (SCROLLER_WIDTH / 2.0).into(),
                ..Border::default()
            },
        },
    }
}
