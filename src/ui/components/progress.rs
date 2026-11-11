//! A 2 px indeterminate progress bar: an accent segment sweeping left to
//! right, eased so it lingers in the middle.

use std::time::Duration;

use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Layout, layout, mouse, renderer};
use iced::{Background, Color, Element, Length, Rectangle, Size};

use crate::ui::theme::{TAB_INDICATOR, Tokens};

/// One sweep.
const CYCLE: Duration = Duration::from_millis(1300);
/// Segment length as a fraction of the track.
const SEGMENT: f32 = 0.28;

/// How far into the current sweep `elapsed` is, in `0.0..1.0`.
pub fn phase(elapsed: Duration) -> f32 {
    elapsed.as_secs_f32().rem_euclid(CYCLE.as_secs_f32()) / CYCLE.as_secs_f32()
}

/// The visible part of the segment at `phase`, as `(start, end)` fractions
/// of the track, clipped to `0.0..=1.0`.
pub fn segment(phase: f32) -> (f32, f32) {
    let p = phase.clamp(0.0, 1.0);
    let eased = p * p * (3.0 - 2.0 * p);
    let left = -SEGMENT + eased * (1.0 + SEGMENT);
    (left.max(0.0), (left + SEGMENT).min(1.0))
}

pub struct Indeterminate {
    color: Color,
    phase: f32,
}

pub fn indeterminate(t: &Tokens, phase: f32) -> Indeterminate {
    Indeterminate {
        color: t.accent,
        phase,
    }
}

impl<M, Theme, Renderer: renderer::Renderer> Widget<M, Theme, Renderer> for Indeterminate {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(TAB_INDICATOR))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fixed(TAB_INDICATOR))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let b = layout.bounds();
        let (start, end) = segment(self.phase);
        if end <= start {
            return;
        }
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: b.x + b.width * start,
                    width: b.width * (end - start),
                    ..b
                },
                ..Default::default()
            },
            Background::Color(self.color),
        );
    }
}

impl<'a, M: 'a, Theme: 'a, Renderer: renderer::Renderer + 'a> From<Indeterminate>
    for Element<'a, M, Theme, Renderer>
{
    fn from(bar: Indeterminate) -> Self {
        Element::new(bar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_segment_enters_from_the_left_and_leaves_on_the_right() {
        assert_eq!(segment(0.0), (0.0, 0.0));
        assert_eq!(segment(1.0), (1.0, 1.0));
        let (start, end) = segment(0.5);
        assert!(start > 0.0 && end < 1.0);
        assert!((end - start - SEGMENT).abs() < 1e-6, "full length mid-sweep");
    }

    #[test]
    fn the_segment_only_moves_forward() {
        let mut last = segment(0.0).0;
        for i in 1..=100u8 {
            let (start, end) = segment(f32::from(i) / 100.0);
            assert!(start >= last && end >= start);
            last = start;
        }
    }

    #[test]
    fn phase_wraps_every_cycle() {
        assert_eq!(phase(Duration::ZERO), 0.0);
        assert_eq!(phase(CYCLE), 0.0);
        assert!((phase(CYCLE / 2) - 0.5).abs() < 1e-3);
    }
}
