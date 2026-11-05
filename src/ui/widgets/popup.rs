//! Row-anchored popup: only the anchor participates in normal layout.
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector, keyboard};

pub struct Popup<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    menu: Element<'a, Message, Theme, Renderer>,
    close: Message,
    align_left: bool,
    open: bool,
    /// Messages for which the menu stays open (typing in a search box
    /// inside it). `None`: any message closes the menu first.
    keep_open: Option<fn(&Message) -> bool>,
}

pub fn popup<'a, Message: Clone + 'a, Theme: 'a, Renderer: renderer::Renderer + 'a>(
    content: Element<'a, Message, Theme, Renderer>,
    menu: Element<'a, Message, Theme, Renderer>,
    close: Message,
) -> Element<'a, Message, Theme, Renderer> {
    Element::new(Popup {
        content,
        menu,
        close,
        align_left: false,
        open: true,
        keep_open: None,
    })
}

/// Stable wrapper for editable search inputs: toggling the popup preserves focus.
pub fn popup_left<'a, Message: Clone + 'a, Theme: 'a, Renderer: renderer::Renderer + 'a>(
    content: Element<'a, Message, Theme, Renderer>,
    menu: Element<'a, Message, Theme, Renderer>,
    close: Message,
    open: bool,
) -> Element<'a, Message, Theme, Renderer> {
    Element::new(Popup {
        content,
        menu,
        close,
        align_left: true,
        open,
        keep_open: None,
    })
}

/// Left-aligned open menu that holds its own input: messages matching
/// `keep_open` (e.g. the filter text) leave it open; any other one runs
/// and then closes it.
pub fn popup_left_sticky<'a, Message: Clone + 'a, Theme: 'a, Renderer: renderer::Renderer + 'a>(
    content: Element<'a, Message, Theme, Renderer>,
    menu: Element<'a, Message, Theme, Renderer>,
    close: Message,
    keep_open: fn(&Message) -> bool,
) -> Element<'a, Message, Theme, Renderer> {
    Element::new(Popup {
        content,
        menu,
        close,
        align_left: true,
        open: true,
        keep_open: Some(keep_open),
    })
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Popup<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Message: Clone,
{
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content), Tree::new(&self.menu)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content, &self.menu]);
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
        _renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if !self.open {
            return self.content.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                _renderer,
                viewport,
                translation,
            );
        }
        let anchor = layout.bounds() + translation;
        if !anchor.intersects(viewport) {
            return None;
        }
        Some(overlay::Element::new(Box::new(MenuOverlay {
            menu: &mut self.menu,
            tree: &mut tree.children[1],
            anchor,
            close: self.close.clone(),
            align_left: self.align_left,
            keep_open: self.keep_open,
        })))
    }
}

struct MenuOverlay<'a, 'b, Message, Theme, Renderer> {
    menu: &'a mut Element<'b, Message, Theme, Renderer>,
    tree: &'a mut Tree,
    anchor: Rectangle,
    close: Message,
    align_left: bool,
    keep_open: Option<fn(&Message) -> bool>,
}

fn menu_position(anchor: Rectangle, size: Size, bounds: Size) -> iced::Point {
    let x = (anchor.x + anchor.width - size.width).clamp(0.0, (bounds.width - size.width).max(0.0));
    let below = anchor.y + anchor.height + 2.0;
    let y = if below + size.height <= bounds.height {
        below
    } else {
        anchor.y - size.height - 2.0
    };
    iced::Point::new(x, y.clamp(0.0, (bounds.height - size.height).max(0.0)))
}

impl<Message: Clone, Theme, Renderer: renderer::Renderer> overlay::Overlay<Message, Theme, Renderer>
    for MenuOverlay<'_, '_, Message, Theme, Renderer>
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let node = self.menu.as_widget_mut().layout(
            self.tree,
            renderer,
            &layout::Limits::new(Size::ZERO, bounds),
        );
        let anchor = if self.align_left {
            Rectangle {
                width: node.size().width,
                ..self.anchor
            }
        } else {
            self.anchor
        };
        let position = menu_position(anchor, node.size(), bounds);
        node.move_to(position)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.menu.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let escape = matches!(
            event,
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            })
        );
        let outside_press = matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_)))
            && !cursor.is_over(layout.bounds())
            && !cursor.is_over(self.anchor);
        if escape || outside_press {
            shell.publish(self.close.clone());
            shell.capture_event();
            return;
        }
        let mut actions = Vec::new();
        let mut menu_shell = Shell::new(&mut actions);
        self.menu.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            &mut menu_shell,
            &layout.bounds(),
        );
        // Close before running the selected action: rename/new-child views must
        // not remain underneath their old menu. Preserve redraw/capture state.
        if let Some(keep_open) = self.keep_open {
            // Sticky menu: run the action first, then close unless every
            // message asked to keep it open (typing in its search box).
            let closes = std::cell::Cell::new(false);
            shell.merge(menu_shell, |m| {
                if !keep_open(&m) {
                    closes.set(true);
                }
                m
            });
            if closes.get() {
                shell.publish(self.close.clone());
            }
        } else {
            if !menu_shell.is_empty() {
                shell.publish(self.close.clone());
            }
            shell.merge(menu_shell, |action| action);
        }
        // Menu padding must also block clicks from reaching the tree beneath it.
        if cursor.is_over(layout.bounds()) && matches!(event, Event::Mouse(_)) {
            shell.capture_event();
        }
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        self.menu
            .as_widget_mut()
            .operate(self.tree, layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.menu.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::{layout, widget::Tree};
    use iced::{Size, widget::Space};

    #[test]
    fn opening_menu_keeps_row_height_and_following_rows_in_place() {
        let anchor: Element<'_, (), iced::Theme, ()> = Space::new().width(300).height(28).into();
        let menu = Space::new().width(196).height(180).into();
        let mut element = popup(anchor, menu, ());
        let mut tree = Tree::new(&element);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &(),
            &layout::Limits::new(Size::ZERO, Size::new(400.0, 800.0)),
        );
        assert_eq!(node.size(), Size::new(300.0, 28.0));
        assert!(
            element
                .as_widget_mut()
                .overlay(
                    &mut tree,
                    iced::advanced::Layout::new(&node),
                    &(),
                    &iced::Rectangle::with_size(Size::new(400.0, 800.0)),
                    iced::Vector::ZERO
                )
                .is_some()
        );
    }
    #[test]
    fn menu_flips_above_bottom_rows_and_stays_inside_window() {
        let anchor = Rectangle {
            x: 250.0,
            y: 760.0,
            width: 100.0,
            height: 28.0,
        };
        let size = Size::new(196.0, 180.0);
        assert_eq!(
            menu_position(anchor, size, Size::new(320.0, 800.0)),
            iced::Point::new(124.0, 578.0)
        );
    }

    #[test]
    fn outside_click_closes_without_clicking_through() {
        use iced::advanced::Shell;
        use iced::advanced::overlay::Overlay;
        let mut menu: Element<'_, bool, iced::Theme, ()> =
            Space::new().width(196).height(180).into();
        let mut tree = Tree::new(&menu);
        let mut overlay = MenuOverlay {
            menu: &mut menu,
            tree: &mut tree,
            anchor: Rectangle::with_size(Size::new(300.0, 28.0)),
            close: true,
            align_left: false,
            keep_open: None,
        };
        let node = overlay.layout(&(), Size::new(400.0, 800.0));
        let event = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        let mut clipboard = iced::advanced::clipboard::Null;
        overlay.update(
            &event,
            Layout::new(&node),
            mouse::Cursor::Available(iced::Point::new(390.0, 500.0)),
            &(),
            &mut clipboard,
            &mut shell,
        );
        assert!(shell.is_event_captured());
        assert_eq!(messages, vec![true]);
    }
    #[test]
    fn left_popup_stays_anchored_and_allows_input_clicks() {
        use iced::advanced::overlay::Overlay;
        let mut menu: Element<'_, bool, iced::Theme, ()> =
            Space::new().width(568).height(300).into();
        let mut tree = Tree::new(&menu);
        let anchor = Rectangle {
            x: 60.0,
            y: 60.0,
            width: 200.0,
            height: 28.0,
        };
        let mut overlay = MenuOverlay {
            menu: &mut menu,
            tree: &mut tree,
            anchor,
            close: true,
            align_left: true,
            keep_open: None,
        };
        let node = overlay.layout(&(), Size::new(1280.0, 800.0));
        assert_eq!(node.bounds().x, 60.0);
        assert_eq!(node.bounds().y, 90.0);
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        let mut clipboard = iced::advanced::clipboard::Null;
        overlay.update(
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(iced::Point::new(90.0, 72.0)),
            &(),
            &mut clipboard,
            &mut shell,
        );
        assert!(!shell.is_event_captured());
        assert!(messages.is_empty());
    }
    #[test]
    fn choosing_menu_item_closes_popup_before_running_action() {
        use iced::advanced::overlay::Overlay;
        let mut menu: Element<'_, &str, iced::Theme, ()> =
            iced::widget::button(Space::new().width(196).height(26))
                .on_press("action")
                .into();
        let mut tree = Tree::new(&menu);
        let mut overlay = MenuOverlay {
            menu: &mut menu,
            tree: &mut tree,
            anchor: Rectangle::with_size(Size::new(300.0, 28.0)),
            close: "close",
            align_left: false,
            keep_open: None,
        };
        let node = overlay.layout(&(), Size::new(400.0, 800.0));
        let mut messages = Vec::new();
        let mut clipboard = iced::advanced::clipboard::Null;
        for event in [
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ] {
            overlay.update(
                &Event::Mouse(event),
                Layout::new(&node),
                mouse::Cursor::Available(node.bounds().center()),
                &(),
                &mut clipboard,
                &mut Shell::new(&mut messages),
            );
        }
        assert_eq!(messages, vec!["close", "action"]);
    }

    #[test]
    fn sticky_menu_stays_open_for_kept_messages_only() {
        use iced::advanced::overlay::Overlay;
        for (action, expected) in [("type", vec!["type"]), ("pick", vec!["pick", "close"])] {
            let mut menu: Element<'_, &str, iced::Theme, ()> =
                iced::widget::button(Space::new().width(196).height(26))
                    .on_press(action)
                    .into();
            let mut tree = Tree::new(&menu);
            let mut overlay = MenuOverlay {
                menu: &mut menu,
                tree: &mut tree,
                anchor: Rectangle::with_size(Size::new(300.0, 28.0)),
                close: "close",
                align_left: true,
                keep_open: Some(|m: &&str| *m == "type"),
            };
            let node = overlay.layout(&(), Size::new(400.0, 800.0));
            let mut messages = Vec::new();
            let mut clipboard = iced::advanced::clipboard::Null;
            for event in [
                mouse::Event::ButtonPressed(mouse::Button::Left),
                mouse::Event::ButtonReleased(mouse::Button::Left),
            ] {
                overlay.update(
                    &Event::Mouse(event),
                    Layout::new(&node),
                    mouse::Cursor::Available(node.bounds().center()),
                    &(),
                    &mut clipboard,
                    &mut Shell::new(&mut messages),
                );
            }
            assert_eq!(messages, expected);
        }
    }
}
