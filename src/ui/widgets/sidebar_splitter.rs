//! The sidebar's edge: a 1 px hairline that is also the resize handle.
//! Presses within `GRAB` px of it start a drag; the new width is the width
//! at drag start plus the cursor's travel since.

use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, layout, mouse, renderer};
use iced::{Background, Color, Element, Event, Length, Rectangle, Size};

use crate::ui::theme::{HAIRLINE, TAB_INDICATOR, Tokens};

/// How far from the hairline, either side, a press still grabs it.
const GRAB: f32 = 3.0;

/// `width` is the whole left panel (rail included); `on_resize` receives
/// the new value of the same measure.
pub fn sidebar_splitter<'a, M: Clone + 'a>(t: &'a Tokens, width: f32, on_resize: fn(f32) -> M) -> Element<'a, M> {
    Element::new(SidebarSplitter {
        tokens: t,
        width,
        on_resize,
    })
}

struct SidebarSplitter<'a, M> {
    tokens: &'a Tokens,
    width: f32,
    on_resize: fn(f32) -> M,
}

#[derive(Default)]
struct State {
    drag_from: Option<(f32, f32)>,
    hovered: bool,
}

fn grab_zone(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x - GRAB,
        width: bounds.width + 2.0 * GRAB,
        ..bounds
    }
}

impl<M: Clone, Theme, Renderer: renderer::Renderer> Widget<M, Theme, Renderer> for SidebarSplitter<'_, M> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(HAIRLINE), Length::Fill)
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fixed(HAIRLINE), Length::Fill)
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
        let b = layout.bounds();
        let (bounds, color) = if state.hovered || state.drag_from.is_some() {
            let x = (b.x - (TAB_INDICATOR - HAIRLINE) / 2.0).round();
            (Rectangle { x, width: TAB_INDICATOR, ..b }, Color { a: 0.6, ..self.tokens.accent })
        } else {
            (b, self.tokens.border)
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..Default::default()
            },
            Background::Color(color),
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
        if state.drag_from.is_some() || cursor.is_over(grab_zone(layout.bounds())) {
            mouse::Interaction::ResizingColumn
        } else {
            mouse::Interaction::None
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
        let zone = grab_zone(layout.bounds());
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cursor.is_over(zone) => {
                if let Some(at) = cursor.position() {
                    state.drag_from = Some((self.width, at.x));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some((start_width, start_x)) = state.drag_from {
                    let width = start_width + (position.x - start_x);
                    if (width - self.width).abs() >= 1.0 {
                        self.width = width;
                        shell.publish((self.on_resize)(width));
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.drag_from.is_some() => {
                state.drag_from = None;
                shell.request_redraw();
            }
            _ => {}
        }
        let hovered = cursor.is_over(zone);
        if hovered != state.hovered {
            state.hovered = hovered;
            shell.request_redraw();
        }
    }
}
