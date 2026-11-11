//! Self-expiring content for transient status messages.
//!
//! `status_message` is set from a dozen handlers and never cleared, so a
//! "Copied!" from an hour ago stayed in the status bar forever. Rather than
//! threading a timer through every one of those call sites, the status bar
//! wraps its message in [`toast_timer`]: the widget remembers when it first
//! drew a given message, schedules a redraw for the deadline, and publishes
//! `on_expire` once it passes. A new message (different `key`) restarts the
//! clock.

use std::hash::{Hash, Hasher};
use std::time::Duration;

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, renderer};
use iced::time::Instant;
use iced::{Element, Event, Length, Rectangle, Size, window};

pub fn toast_timer<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    key: &str,
    ttl: Duration,
    on_expire: Message,
) -> Element<'a, Message> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    Element::new(ToastTimer {
        content: content.into(),
        key: hasher.finish(),
        ttl,
        on_expire,
    })
}

struct ToastTimer<'a, Message> {
    content: Element<'a, Message>,
    key: u64,
    ttl: Duration,
    on_expire: Message,
}

#[derive(Default)]
struct State {
    key: Option<u64>,
    shown_at: Option<Instant>,
    fired: bool,
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for ToastTimer<'_, Message> {
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
        let state = tree.state.downcast_mut::<State>();
        if state.key != Some(self.key) {
            *state = State {
                key: Some(self.key),
                ..State::default()
            };
        }
        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && !state.fired
        {
            let shown_at = *state.shown_at.get_or_insert(*now);
            let deadline = shown_at + self.ttl;
            if *now >= deadline {
                state.fired = true;
                shell.publish(self.on_expire.clone());
            } else {
                shell.request_redraw_at(deadline);
            }
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
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
