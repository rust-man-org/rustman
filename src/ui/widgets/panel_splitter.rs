use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, renderer};
use iced::{Element, Event, Length, Rectangle, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
struct State {
    dragging: bool,
    hovered: bool,
    /// Snapshotted origin (content_top for H, content_left for V) at drag start.
    drag_origin: f32,
    /// Snapshotted available space (excluding splitter) at drag start.
    drag_total: f32,
}

/// A draggable splitter bar. In `Horizontal` mode (top/bottom panels) it's
/// a horizontal bar resizing rows; in `Vertical` mode (left/right panels) it's
/// a vertical bar resizing columns. The split ratio is calculated directly from
/// the cursor position each frame, making drag feel smooth with no jitter.
pub fn panel_splitter<'a, Message, Theme, Renderer>(
    panel_split: u16,
    orientation: Orientation,
    on_resize: impl Fn(u16) -> Message + 'a,
) -> PanelSplitter<'a, Message, Theme, Renderer> {
    PanelSplitter {
        panel_split,
        orientation,
        on_resize: Box::new(on_resize),
        _phantom: std::marker::PhantomData,
    }
}

pub struct PanelSplitter<'a, Message, Theme, Renderer> {
    panel_split: u16,
    orientation: Orientation,
    on_resize: Box<dyn Fn(u16) -> Message + 'a>,
    _phantom: std::marker::PhantomData<(Theme, Renderer)>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for PanelSplitter<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Message: Clone + 'static,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State {
            dragging: false,
            hovered: false,
            drag_origin: 0.0,
            drag_total: 0.0,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![]
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        match self.orientation {
            Orientation::Horizontal => Size::new(Length::Fill, Length::Fixed(8.0)),
            Orientation::Vertical => Size::new(Length::Fixed(8.0), Length::Fill),
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let (w, h) = match self.orientation {
            Orientation::Horizontal => (Length::Fill, Length::Fixed(8.0)),
            Orientation::Vertical => (Length::Fixed(8.0), Length::Fill),
        };
        layout::Node::new(limits.resolve(w, h, Size::ZERO))
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        // Fill the drag area with the panel surface (an undrawn area showed
        // the window background as a thick gap) and draw one hairline.
        let state = tree.state.downcast_ref::<State>();
        let active = state.dragging || cursor.is_over(layout.bounds());
        crate::ui::widgets::seam::draw(
            renderer,
            layout.bounds(),
            self.orientation == Orientation::Vertical,
            crate::ui::widgets::seam::LineAt::Center,
            active,
        );
    }
    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            match self.orientation {
                Orientation::Horizontal => mouse::Interaction::ResizingRow,
                Orientation::Vertical => mouse::Interaction::ResizingColumn,
            }
        } else {
            mouse::Interaction::Idle
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
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(button))
                if *button == mouse::Button::Left && cursor.is_over(bounds) =>
            {
                state.dragging = true;
                // Snapshot the available area boundaries at drag start.
                match self.orientation {
                    Orientation::Horizontal => {
                        let p = self.panel_split as f32 / 100.0;
                        let fixed_bot = 29.0;
                        state.drag_origin =
                            (bounds.y - p * (viewport.height - fixed_bot - 8.0)) / (1.0 - p);
                        state.drag_total =
                            (viewport.height - state.drag_origin - fixed_bot - 8.0).max(1.0);
                    }
                    Orientation::Vertical => {
                        let p = self.panel_split as f32 / 100.0;
                        state.drag_origin = (bounds.x - p * (viewport.width - 8.0)) / (1.0 - p);
                        state.drag_total = (viewport.width - state.drag_origin - 8.0).max(1.0);
                    }
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.dragging => {
                let new_split = match self.orientation {
                    Orientation::Horizontal => {
                        let ratio =
                            ((position.y - state.drag_origin) / state.drag_total).clamp(0.0, 1.0);
                        (ratio * 100.0).round().clamp(1.0, 99.0) as u16
                    }
                    Orientation::Vertical => {
                        let ratio =
                            ((position.x - state.drag_origin) / state.drag_total).clamp(0.0, 1.0);
                        (ratio * 100.0).round().clamp(1.0, 99.0) as u16
                    }
                };
                if new_split != self.panel_split {
                    shell.publish((self.on_resize)(new_split));
                    self.panel_split = new_split;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button))
                if *button == mouse::Button::Left && state.dragging =>
            {
                state.dragging = false;
            }
            _ => {}
        }

        state.hovered = cursor.is_over(bounds);
    }
}

impl<'a, Message, Theme, Renderer> From<PanelSplitter<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'static + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(splitter: PanelSplitter<'a, Message, Theme, Renderer>) -> Self {
        Element::new(splitter)
    }
}
