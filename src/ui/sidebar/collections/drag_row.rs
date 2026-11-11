//! A tree row that tells a click from a drag.
//!
//! A left press publishes `on_press` at once; it only becomes a drag once the
//! pointer travels [`DRAG_THRESHOLD_PX`], so jitter stays a click. The release
//! is seen wherever it happens and always publishes `on_release`, so a
//! gesture can't outlive the button. During a drag the row reports the drop
//! slot under the pointer and paints the indicator itself, which keeps the
//! widget tree's shape (and every row's state) stable while dragging.

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, overlay, renderer};
use iced::{Background, Border, Color, Element, Event, Length, Point, Rectangle, Size, Vector};

use crate::app::tree::DropZones;
use crate::state::sidebar::DropPosition;

/// Travel, in logical px, that turns a press into a drag.
pub const DRAG_THRESHOLD_PX: f32 = 4.0;

/// Whether the pointer at `now` has left the click radius around `origin`.
pub fn past_threshold(origin: Point, now: Point) -> bool {
    origin.distance(now) >= DRAG_THRESHOLD_PX
}

/// The drop slot under the pointer at `frac` of the row height (0 = top
/// edge). Rows taking both siblings and children split into three bands.
/// `after_allowed` is false for an open folder: the row below it is its own
/// first child, so an "after" line would sit where the drop doesn't go.
pub fn zone_at(frac: f32, zones: DropZones, after_allowed: bool) -> Option<DropPosition> {
    match (zones.sibling, zones.inside) {
        (true, true) => Some(if frac < 0.25 {
            DropPosition::Before
        } else if frac > 0.75 && after_allowed {
            DropPosition::After
        } else {
            DropPosition::Inside
        }),
        (true, false) => Some(if frac < 0.5 {
            DropPosition::Before
        } else {
            DropPosition::After
        }),
        (false, true) => Some(DropPosition::Inside),
        (false, false) => None,
    }
}

pub fn drag_row<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> DragRow<'a, Message, Theme, Renderer> {
    DragRow {
        content: content.into(),
        on_press: None,
        on_drag_start: None,
        on_release: None,
        on_right_press: None,
        on_hover: None,
        on_leave: None,
        zones: DropZones::default(),
        after_allowed: true,
        indicator: None,
        dragging: false,
        hover_background: None,
        accent: Color::WHITE,
        accent_soft: Color::TRANSPARENT,
        radius: 0.0,
    }
}

pub struct DragRow<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_press: Option<Message>,
    on_drag_start: Option<Message>,
    on_release: Option<Message>,
    on_right_press: Option<Message>,
    on_hover: Option<Box<dyn Fn(DropPosition) -> Message + 'a>>,
    on_leave: Option<Message>,
    zones: DropZones,
    after_allowed: bool,
    indicator: Option<DropPosition>,
    dragging: bool,
    hover_background: Option<Color>,
    accent: Color,
    accent_soft: Color,
    radius: f32,
}

impl<'a, Message, Theme, Renderer> DragRow<'a, Message, Theme, Renderer> {
    /// Left press on the row, plus the matching release (wherever it lands).
    pub fn on_press(mut self, press: Message, release: Message) -> Self {
        self.on_press = Some(press);
        self.on_release = Some(release);
        self
    }

    /// The pressed pointer travelled past [`DRAG_THRESHOLD_PX`].
    pub fn on_drag_start(mut self, message: Message) -> Self {
        self.on_drag_start = Some(message);
        self
    }

    pub fn on_right_press(mut self, message: Message) -> Self {
        self.on_right_press = Some(message);
        self
    }

    /// Report drop slots. Set only during a drag, so idle pointer motion
    /// publishes nothing.
    pub fn drop_target(
        mut self,
        zones: DropZones,
        after_allowed: bool,
        on_hover: impl Fn(DropPosition) -> Message + 'a,
        on_leave: Message,
    ) -> Self {
        self.zones = zones;
        self.after_allowed = after_allowed;
        self.on_hover = Some(Box::new(on_hover));
        self.on_leave = Some(on_leave);
        self
    }

    /// Paint the drop indicator for `position` (the current drop target).
    pub fn indicator(mut self, position: Option<DropPosition>) -> Self {
        self.indicator = position;
        self
    }

    /// A drag is active somewhere in the tree: grabbing cursor, no hover fill.
    pub fn dragging(mut self, dragging: bool) -> Self {
        self.dragging = dragging;
        self
    }

    /// Fill painted under the content while hovered.
    pub fn hover_background(mut self, color: Option<Color>, radius: f32) -> Self {
        self.hover_background = color;
        self.radius = radius;
        self
    }

    /// The drop indicator's line and outline color, and the "into this
    /// folder" fill.
    pub fn indicator_colors(mut self, accent: Color, accent_soft: Color) -> Self {
        self.accent = accent;
        self.accent_soft = accent_soft;
        self
    }
}

#[derive(Debug, Default)]
struct State {
    /// Where the left button went down on this row, while it is still held.
    press_origin: Option<Point>,
    /// `on_drag_start` already sent for the current press.
    drag_reported: bool,
    /// Pointer over the row (for the hover background redraw).
    hovered: bool,
    /// Last drop slot reported via `on_hover`, to send only changes.
    last_zone: Option<DropPosition>,
}

impl<Message, Theme, Renderer> iced::advanced::Widget<Message, Theme, Renderer>
    for DragRow<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        // The release ends the gesture even if something else captured it.
        if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
        | Event::Touch(iced::touch::Event::FingerLifted { .. }) = event
        {
            if state.press_origin.take().is_some()
                && let Some(message) = &self.on_release
            {
                shell.publish(message.clone());
            }
            state.drag_reported = false;
            state.last_zone = None;
            return;
        }

        if let Event::Mouse(mouse::Event::CursorMoved { .. }) = event {
            let hovered = cursor.is_over(bounds);
            if hovered != state.hovered {
                state.hovered = hovered;
                if self.hover_background.is_some() {
                    shell.request_redraw();
                }
            }

            // Inside a scrollable the cursor levitates once it leaves the
            // viewport; landing it keeps origin and position in one space.
            if let (Some(origin), Some(now)) = (state.press_origin, cursor.land().position())
                && !state.drag_reported
                && past_threshold(origin, now)
            {
                state.drag_reported = true;
                if let Some(message) = &self.on_drag_start {
                    shell.publish(message.clone());
                }
            }

            if let Some(on_hover) = &self.on_hover {
                let zone = cursor.position_in(bounds).and_then(|p| {
                    let frac = if bounds.height > 0.0 {
                        p.y / bounds.height
                    } else {
                        0.5
                    };
                    zone_at(frac, self.zones, self.after_allowed)
                });
                if zone != state.last_zone {
                    match zone {
                        Some(zone) => shell.publish(on_hover(zone)),
                        None => {
                            if let Some(message) = &self.on_leave {
                                shell.publish(message.clone());
                            }
                        }
                    }
                    state.last_zone = zone;
                }
            } else {
                state.last_zone = None;
            }
            return;
        }

        if shell.is_event_captured() || !cursor.is_over(bounds) {
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                if let Some(message) = &self.on_press {
                    state.press_origin = cursor.position();
                    state.drag_reported = false;
                    shell.publish(message.clone());
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if let Some(message) = &self.on_right_press {
                    shell.publish(message.clone());
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let content = self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        );
        if content != mouse::Interaction::None || !cursor.is_over(layout.bounds()) {
            return content;
        }
        if self.dragging {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Pointer
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let radius = self.radius.into();

        if self.indicator == Some(DropPosition::Inside) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        color: self.accent,
                        width: 1.5,
                        radius,
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(self.accent_soft),
            );
        } else if let Some(color) = self.hover_background
            && !self.dragging
            && cursor.is_over(bounds)
        {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius,
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(color),
            );
        }

        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );

        let line_y = match self.indicator {
            Some(DropPosition::Before) => Some(bounds.y - 1.0),
            Some(DropPosition::After) => Some(bounds.y + bounds.height - 1.0),
            _ => None,
        };
        if let Some(y) = line_y {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x,
                        y,
                        width: bounds.width,
                        height: 2.0,
                    },
                    border: Border {
                        radius: 1.0.into(),
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(self.accent),
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<DragRow<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(row: DragRow<'a, Message, Theme, Renderer>) -> Self {
        Element::new(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_stays_a_click_and_travel_starts_a_drag() {
        let origin = Point::new(100.0, 50.0);
        assert!(!past_threshold(origin, origin));
        assert!(!past_threshold(origin, Point::new(102.0, 51.5)));
        assert!(past_threshold(origin, Point::new(100.0, 54.0)));
        assert!(past_threshold(origin, Point::new(96.0, 50.0)));
    }

    #[test]
    fn rows_taking_siblings_and_children_split_in_three_bands() {
        let both = DropZones {
            sibling: true,
            inside: true,
        };
        assert_eq!(zone_at(0.1, both, true), Some(DropPosition::Before));
        assert_eq!(zone_at(0.5, both, true), Some(DropPosition::Inside));
        assert_eq!(zone_at(0.9, both, true), Some(DropPosition::After));
        assert_eq!(zone_at(0.9, both, false), Some(DropPosition::Inside));
    }

    #[test]
    fn sibling_only_and_inside_only_rows() {
        let sibling = DropZones {
            sibling: true,
            inside: false,
        };
        assert_eq!(zone_at(0.3, sibling, true), Some(DropPosition::Before));
        assert_eq!(zone_at(0.7, sibling, true), Some(DropPosition::After));
        let inside = DropZones {
            sibling: false,
            inside: true,
        };
        assert_eq!(zone_at(0.0, inside, true), Some(DropPosition::Inside));
        assert_eq!(zone_at(0.5, DropZones::default(), true), None);
    }
}
