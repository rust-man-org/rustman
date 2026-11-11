use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector, keyboard};

/// Maps a key press to an app-level shortcut message, if it is one.
pub type ShortcutMap<Message> = fn(&keyboard::Key, keyboard::Modifiers) -> Option<Message>;

pub struct KeyGuard<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    enabled: bool,
    active: bool,
    on_undo: Message,
    on_redo: Message,
    on_send: Message,
    shortcuts: Option<ShortcutMap<Message>>,
}

pub fn key_guard<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    active: bool,
    on_undo: Message,
    on_redo: Message,
    on_send: Message,
) -> KeyGuard<'a, Message, Theme, Renderer> {
    KeyGuard {
        content: content.into(),
        enabled: true,
        active,
        on_undo,
        on_redo,
        on_send,
        shortcuts: None,
    }
}

impl<Message, Theme, Renderer> KeyGuard<'_, Message, Theme, Renderer> {
    /// Off while a modal owns the keyboard: every key then passes through
    /// untouched, so nothing reaches the request behind the modal.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Extra app shortcuts, checked before any child sees the key and
    /// captured when matched — so e.g. Ctrl+L never also lands in an editor.
    /// Pass `None` to disable them (while a modal owns the keyboard).
    pub fn shortcuts(mut self, map: Option<ShortcutMap<Message>>) -> Self {
        self.shortcuts = map;
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for KeyGuard<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Message: Clone,
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
        // A field typed in elsewhere (see `typing_scope`) already took it.
        if self.enabled
            && !shell.is_event_captured()
            && let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event
        {
            use keyboard::key::Named;
            // Always intercept Cmd+Enter so body editors don't also insert a newline.
            if *key == keyboard::Key::Named(Named::Enter) && modifiers.command() {
                shell.publish(self.on_send.clone());
                shell.capture_event();
                return;
            }
            if let Some(map) = self.shortcuts
                && let Some(message) = map(key, *modifiers)
            {
                shell.publish(message);
                shell.capture_event();
                return;
            }
            if self.active {
                if let keyboard::Key::Character(c) = key.as_ref() {
                    if c.eq_ignore_ascii_case("z") && modifiers.command() {
                        shell.publish(if modifiers.shift() {
                            self.on_redo.clone()
                        } else {
                            self.on_undo.clone()
                        });
                        shell.capture_event();
                        return;
                    }
                }
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
        renderer: &Renderer,
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
        renderer: &mut Renderer,
        theme: &Theme,
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

impl<'a, Message, Theme, Renderer> From<KeyGuard<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer,
{
    fn from(guard: KeyGuard<'a, Message, Theme, Renderer>) -> Self {
        Element::new(guard)
    }
}
