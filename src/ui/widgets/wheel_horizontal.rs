//! Lets a vertical mouse wheel scroll a horizontal strip: iced's
//! horizontal-only scrollable ignores the wheel's vertical delta.

use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// A wheel step as horizontal movement: a mostly vertical step turns
/// sideways, a trackpad's horizontal step passes through.
pub fn horizontal_delta(delta: mouse::ScrollDelta) -> mouse::ScrollDelta {
    match delta {
        mouse::ScrollDelta::Lines { x, y } if y.abs() > x.abs() => mouse::ScrollDelta::Lines { x: y, y: 0.0 },
        mouse::ScrollDelta::Pixels { x, y } if y.abs() > x.abs() => mouse::ScrollDelta::Pixels { x: y, y: 0.0 },
        other => other,
    }
}

pub fn wheel_horizontal<'a, M: 'a>(content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    Element::new(WheelHorizontal { content: content.into() })
}

struct WheelHorizontal<'a, M> {
    content: Element<'a, M>,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for WheelHorizontal<'_, M> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operation: &mut dyn Operation) {
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
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        let mapped;
        let event = match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(layout.bounds()) => {
                mapped = Event::Mouse(mouse::Event::WheelScrolled { delta: horizontal_delta(*delta) });
                &mapped
            }
            other => other,
        };
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
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mouse::ScrollDelta;

    #[test]
    fn a_vertical_wheel_scrolls_sideways_and_trackpads_pass_through() {
        assert_eq!(horizontal_delta(ScrollDelta::Lines { x: 0.0, y: -1.0 }), ScrollDelta::Lines { x: -1.0, y: 0.0 });
        assert_eq!(horizontal_delta(ScrollDelta::Pixels { x: 0.0, y: 12.0 }), ScrollDelta::Pixels { x: 12.0, y: 0.0 });
        let sideways = ScrollDelta::Pixels { x: 30.0, y: 2.0 };
        assert_eq!(horizontal_delta(sideways), sideways);
    }
}
