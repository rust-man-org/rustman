//! Keeps app shortcuts out of a field being typed in.
//!
//! iced hands every key press to every widget and to the app's key listener,
//! wherever the focus is, and a text input lets Ctrl+W or Alt+1 through. So
//! a Ctrl+W typed into a rename would close the tab behind it. Wrapped in a
//! typing scope, a modified key press that reaches a focused field inside is
//! marked captured, and the listeners skip captured events.

use iced::advanced::widget::operation::{Focusable, Operation};
use iced::advanced::widget::{Id, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector, keyboard};

pub struct TypingScope<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

pub fn typing_scope<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> TypingScope<'a, Message, Theme, Renderer> {
    TypingScope { content: content.into() }
}

/// Whether a press with these modifiers could be an app shortcut.
pub fn is_shortcut_chord(modifiers: keyboard::Modifiers) -> bool {
    modifiers.command() || modifiers.control() || modifiers.alt()
}

/// Finds whether any focusable widget it visits has the focus.
struct FocusProbe(bool);

impl Operation for FocusProbe {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn focusable(&mut self, _id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        self.0 |= state.is_focused();
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TypingScope<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation);
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
        self.content
            .as_widget_mut()
            .update(&mut tree.children[0], event, layout, cursor, renderer, clipboard, shell, viewport);
        if let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) = event
            && is_shortcut_chord(*modifiers)
            && !shell.is_event_captured()
        {
            let mut probe = FocusProbe(false);
            self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, &mut probe);
            if probe.0 {
                shell.capture_event();
            }
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
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
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
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer> From<TypingScope<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer + 'a,
{
    fn from(scope: TypingScope<'a, Message, Theme, Renderer>) -> Self {
        Element::new(scope)
    }
}
