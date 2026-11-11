//! The request/response splitter: an overlay across the whole pane area
//! that takes no layout space. The hairline itself is part of the layout;
//! this grabs presses within `GRAB` px of it, shows the resize cursor,
//! draws an accent line while hovered or dragged, and reports the request
//! pane's new size in px.

use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, renderer};
use iced::{Background, Color, Element, Event, Length, Point, Rectangle, Size};

use crate::ui::theme::{HAIRLINE, MIN_PANE, TAB_INDICATOR, Tokens};

/// How far from the hairline, either side, a press still grabs it.
const GRAB: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Request on the left, response on the right.
    SideBySide,
    /// Request above, response below.
    Stacked,
}

/// The request pane's extent along the split, including its hairline:
/// the saved size (or `default`), kept so both panes stay usable in `total`.
pub fn pane_extent(saved: Option<f32>, default: f32, total: f32) -> f32 {
    let max = (total - MIN_PANE).max(MIN_PANE);
    saved.unwrap_or(default).clamp(MIN_PANE, max)
}

pub fn panel_splitter<'a, M: Clone + 'a>(
    t: &'a Tokens,
    extent: f32,
    orientation: Orientation,
    on_resize: fn(f32) -> M,
) -> Element<'a, M> {
    Element::new(PanelSplitter {
        tokens: t,
        extent,
        orientation,
        on_resize,
    })
}

struct PanelSplitter<'a, M> {
    tokens: &'a Tokens,
    extent: f32,
    orientation: Orientation,
    on_resize: fn(f32) -> M,
}

#[derive(Default)]
struct State {
    dragging: bool,
    hovered: bool,
}

impl<M> PanelSplitter<'_, M> {
    /// Distance of `point` from the area's origin along the split.
    fn along(&self, area: Rectangle, point: Point) -> f32 {
        match self.orientation {
            Orientation::SideBySide => point.x - area.x,
            Orientation::Stacked => point.y - area.y,
        }
    }

    fn total(&self, area: Rectangle) -> f32 {
        match self.orientation {
            Orientation::SideBySide => area.width,
            Orientation::Stacked => area.height,
        }
    }

    fn near_line(&self, area: Rectangle, cursor: mouse::Cursor) -> bool {
        cursor.position_over(area).is_some_and(|p| {
            let line = self.extent - HAIRLINE;
            (self.along(area, p) - line).abs() <= GRAB
        })
    }

    /// The accent line drawn over the hairline while active.
    fn indicator(&self, area: Rectangle) -> Rectangle {
        let start = self.extent - HAIRLINE - (TAB_INDICATOR - HAIRLINE) / 2.0;
        match self.orientation {
            Orientation::SideBySide => Rectangle {
                x: (area.x + start).round(),
                width: TAB_INDICATOR,
                ..area
            },
            Orientation::Stacked => Rectangle {
                y: (area.y + start).round(),
                height: TAB_INDICATOR,
                ..area
            },
        }
    }
}

impl<M: Clone, Theme, Renderer: renderer::Renderer> Widget<M, Theme, Renderer> for PanelSplitter<'_, M> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        if !(state.hovered || state.dragging) {
            return;
        }
        let alpha = if state.dragging { 0.9 } else { 0.6 };
        renderer.fill_quad(
            renderer::Quad {
                bounds: self.indicator(layout.bounds()),
                ..Default::default()
            },
            Background::Color(Color {
                a: alpha,
                ..self.tokens.accent
            }),
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        if !(state.dragging || self.near_line(layout.bounds(), cursor)) {
            return mouse::Interaction::None;
        }
        match self.orientation {
            Orientation::SideBySide => mouse::Interaction::ResizingColumn,
            Orientation::Stacked => mouse::Interaction::ResizingRow,
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let area = layout.bounds();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if self.near_line(area, cursor) => {
                state.dragging = true;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.dragging => {
                let extent = pane_extent(Some(self.along(area, *position) + HAIRLINE), 0.0, self.total(area));
                if (extent - self.extent).abs() >= 1.0 {
                    self.extent = extent;
                    shell.publish((self.on_resize)(extent));
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                shell.request_redraw();
            }
            _ => {}
        }
        let hovered = self.near_line(area, cursor);
        if hovered != state.hovered {
            state.hovered = hovered;
            shell.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_extent_is_used_until_the_user_drags() {
        assert!((pane_extent(None, 520.0, 1136.0) - 520.0).abs() < f32::EPSILON);
        assert!((pane_extent(Some(600.0), 520.0, 1136.0) - 600.0).abs() < f32::EPSILON);
    }

    #[test]
    fn both_panes_keep_their_minimum() {
        assert!((pane_extent(Some(10.0), 520.0, 1136.0) - MIN_PANE).abs() < f32::EPSILON);
        assert!((pane_extent(Some(5000.0), 520.0, 1136.0) - (1136.0 - MIN_PANE)).abs() < f32::EPSILON);
        assert!(
            (pane_extent(Some(400.0), 520.0, 200.0) - MIN_PANE).abs() < f32::EPSILON,
            "a tiny window still shows the request"
        );
    }
}
