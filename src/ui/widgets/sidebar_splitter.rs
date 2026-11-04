use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, renderer};
use iced::{Element, Event, Length, Rectangle, Size};

#[derive(Clone)]
struct State {
    dragging: bool,
    hovered: bool,
    /// Sidebar width snapshotted at drag start.
    start_width: f32,
    /// Cursor x snapshotted at drag start.
    start_x: f32,
}

/// A draggable vertical splitter for the sidebar (issue #53: the sidebar
/// used to be a fixed 264px). Unlike `panel_splitter` — which works in split
/// ratios — this one tracks an absolute pixel width: the new width is the
/// width at drag start plus the cursor's horizontal travel since.
///
/// `width` is the full left-panel width including the 48px icon rail; the
/// message carries the sidebar *content* width the same way `AppState` stores
/// it, so the handler can clamp + persist without re-deriving the offset.
pub fn sidebar_splitter<'a, Message, Theme, Renderer>(
    width: f32,
    on_resize: impl Fn(f32) -> Message + 'a,
) -> SidebarSplitter<'a, Message, Theme, Renderer> {
    SidebarSplitter {
        width,
        on_resize: Box::new(on_resize),
        _phantom: std::marker::PhantomData,
    }
}

pub struct SidebarSplitter<'a, Message, Theme, Renderer> {
    width: f32,
    on_resize: Box<dyn Fn(f32) -> Message + 'a>,
    _phantom: std::marker::PhantomData<(Theme, Renderer)>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SidebarSplitter<'_, Message, Theme, Renderer>
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
            start_width: 0.0,
            start_x: 0.0,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![]
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(8.0), Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(Length::Fixed(8.0), Length::Fill, Size::ZERO))
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
        // Fill the drag area so no window background shows as a gap; the
        // visible divider is one hairline against the sidebar.
        let state = tree.state.downcast_ref::<State>();
        let active = state.dragging || cursor.is_over(layout.bounds());
        crate::ui::widgets::seam::draw(
            renderer,
            layout.bounds(),
            true,
            crate::ui::widgets::seam::LineAt::Start,
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
            mouse::Interaction::ResizingColumn
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
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(button))
                if *button == mouse::Button::Left && cursor.is_over(bounds) =>
            {
                state.dragging = true;
                state.start_width = self.width;
                if let Some(pos) = cursor.position() {
                    state.start_x = pos.x;
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.dragging => {
                let new_width = state.start_width + (position.x - state.start_x);
                if (new_width - self.width).abs() >= 1.0 {
                    shell.publish((self.on_resize)(new_width));
                    self.width = new_width;
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

impl<'a, Message, Theme, Renderer> From<SidebarSplitter<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'static + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(splitter: SidebarSplitter<'a, Message, Theme, Renderer>) -> Self {
        Element::new(splitter)
    }
}
