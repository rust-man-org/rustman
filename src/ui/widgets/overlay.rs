//! Modal plumbing shared by every dialog and the command palette.
//!
//! [`modal_layer`] turns a centered card into a proper modal:
//!
//! * **Blocks the UI underneath.** A plain dimmed container in a `stack!`
//!   reports no mouse interaction, so iced's `Stack` hands the click straight
//!   to the layer below — buttons behind the scrim stayed clickable while a
//!   dialog was open.
//! * **Click outside to dismiss**, like every desktop and web dialog.
//! * **Esc to dismiss.** Only the palette and sidebar rename handled Esc;
//!   every other dialog needed a mouse trip to its Cancel button.

use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::{container, mouse_area, opaque};
use iced::{Element, Event, Length, Rectangle, Size, Vector, keyboard};

/// Wraps a fully laid-out modal (scrim + centered card) so presses on the
/// scrim and the Esc key both publish `on_dismiss`, and nothing below the
/// modal receives the press.
///
/// `card` must be the dialog card itself (not the scrim): presses inside it
/// are swallowed so clicking a label or padding never dismisses.
pub fn modal_layer<'a, Message: Clone + 'a>(
    card: impl Into<Element<'a, Message>>,
    on_dismiss: Message,
    layout_card: impl FnOnce(Element<'a, Message>) -> Element<'a, Message>,
) -> Element<'a, Message> {
    let card = opaque(card);
    let scrim = mouse_area(
        container(layout_card(card))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(on_dismiss.clone());
    opaque(EscapeCatcher {
        content: scrim.into(),
        on_escape: on_dismiss,
    })
}

/// Publishes `on_escape` when Escape is pressed anywhere, then lets the
/// event continue (so the global Esc handler can still cancel a rename).
struct EscapeCatcher<'a, Message> {
    content: Element<'a, Message>,
    on_escape: Message,
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for EscapeCatcher<'_, Message> {
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
        renderer: &iced::Renderer,
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
        renderer: &iced::Renderer,
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
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event
            && *key == keyboard::Key::Named(keyboard::key::Named::Escape)
        {
            shell.publish(self.on_escape.clone());
        }
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: Clone + 'a> From<EscapeCatcher<'a, Message>> for Element<'a, Message> {
    fn from(w: EscapeCatcher<'a, Message>) -> Self {
        Element::new(w)
    }
}
