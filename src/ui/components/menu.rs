//! Dropdown menus. [`popover`] anchors a floating panel under any trigger;
//! [`panel`] builds the standard menu panel (32 px rows, icon, label,
//! shortcut). The app owns which menu is open, so a menu is just state
//! plus two messages: toggle on the trigger, dismiss from the popover.

use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::{Space, Text, button, column, container, row, text};
use iced::{
    Alignment, Background, Border, Color, Element, Event, Font, Length, Padding, Point,
    Rectangle, Shadow, Size, Vector, keyboard,
};

use crate::ui::theme::{CONTROL_LG, CONTROL_SM, RADIUS_LG, RADIUS_SM, SPACE_MD, SPACE_XS, TEXT_MD, TEXT_SM, Tokens};

/// Gap between the trigger and the panel.
const GAP: f32 = SPACE_XS;

/// Which edge of the trigger the panel lines up with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
}

pub enum Entry<'a, M> {
    Item(Box<Item<'a, M>>),
    Separator,
}

pub struct Item<'a, M> {
    label: String,
    on_select: M,
    icon: Option<Text<'a>>,
    shortcut: Option<String>,
    checked: bool,
    color: Option<Color>,
    font: Option<Font>,
    lead: Option<Element<'a, M>>,
    dot: Option<Color>,
}

impl<'a, M> Item<'a, M> {
    pub fn new(label: impl Into<String>, on_select: M) -> Self {
        Item {
            label: label.into(),
            on_select,
            icon: None,
            shortcut: None,
            checked: false,
            color: None,
            font: None,
            lead: None,
            dot: None,
        }
    }

    /// A custom element before the label (a colored method name).
    pub fn lead(mut self, lead: impl Into<Element<'a, M>>) -> Self {
        self.lead = Some(lead.into());
        self
    }

    /// A status dot at the end of the row (unsaved changes).
    pub fn dot(mut self, color: Option<Color>) -> Self {
        self.dot = color;
        self
    }

    pub fn icon(mut self, icon: Text<'a>) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    /// Marks the current choice with a check.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Label color and font (method names are colored mono).
    pub fn styled(mut self, color: Color, font: Font) -> Self {
        self.color = Some(color);
        self.font = Some(font);
        self
    }
}

impl<'a, M> From<Item<'a, M>> for Entry<'a, M> {
    fn from(item: Item<'a, M>) -> Self {
        Entry::Item(Box::new(item))
    }
}

/// The standard menu panel.
pub fn panel<'a, M: Clone + 'a>(t: &'a Tokens, entries: Vec<Entry<'a, M>>, width: f32) -> Element<'a, M> {
    let rows = entries.into_iter().map(|entry| match entry {
        Entry::Item(item) => item_view(t, *item),
        Entry::Separator => container(raised_hairline(t)).padding([SPACE_XS, 0.0]).into(),
    });
    // iced draws the border inside the padding; CSS draws it outside.
    container(column(rows).width(Length::Fill))
        .padding(SPACE_XS + crate::ui::theme::HAIRLINE)
        .width(width)
        .style(move |_| panel_style(t))
        .into()
}

fn item_view<'a, M: Clone + 'a>(t: &'a Tokens, item: Item<'a, M>) -> Element<'a, M> {
    let r = t.on_raised;
    let mut content = row![].spacing(SPACE_MD).align_y(Alignment::Center);
    if let Some(icon) = item.icon {
        let icon: Text<'a> = icon.size(TEXT_MD + 1.0).color(r.text_muted);
        content = content.push(icon);
    }
    if let Some(lead) = item.lead {
        content = content.push(lead);
    }
    let mut label = text(item.label).size(TEXT_SM + 1.0);
    if let Some(color) = item.color {
        label = label.color(color);
    }
    if let Some(font) = item.font {
        label = label.font(font);
    }
    content = content.push(label).push(Space::new().width(Length::Fill));
    if let Some(keys) = item.shortcut {
        content = content.push(text(keys).size(TEXT_SM).color(r.text_muted));
    }
    if let Some(color) = item.dot {
        content = content.push(crate::ui::components::status_dot::dot(color));
    }
    if item.checked {
        let check: Text<'a> = crate::ui::icons::check().size(TEXT_SM).color(t.accent_text);
        content = content.push(check);
    }
    button(container(content).center_y(Length::Fill))
        .on_press(item.on_select)
        .width(Length::Fill)
        .height(CONTROL_LG)
        .padding(Padding::from([0.0, SPACE_MD]))
        .style(move |_, status| item_style(t, status))
        .into()
}

fn item_style(t: &Tokens, status: iced::widget::button::Status) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    iced::widget::button::Style {
        background: matches!(status, Status::Hovered | Status::Pressed)
            .then_some(Background::Color(t.on_raised.hover)),
        text_color: t.text,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn raised_hairline<'a, M: 'a>(t: &'a Tokens) -> Element<'a, M> {
    container(Space::new().width(Length::Fill).height(crate::ui::theme::HAIRLINE))
        .style(move |_| container::Style {
            background: Some(Background::Color(t.on_raised.border)),
            ..Default::default()
        })
        .into()
}

pub fn panel_style(t: &Tokens) -> container::Style {
    container::Style {
        background: Some(Background::Color(t.raised)),
        text_color: Some(t.text),
        border: Border {
            color: t.on_raised.border,
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        shadow: Shadow {
            color: t.shadow,
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        ..Default::default()
    }
}

/// The compact trigger of an inline dropdown ("JSON ▾"): 24 px, label
/// then a muted chevron, a hover fill, and the fill held while open.
pub fn trigger<'a, M: 'a>(t: &'a Tokens, label: impl Into<Element<'a, M>>, open: bool) -> iced::widget::Button<'a, M> {
    let chevron: Text<'a> = crate::ui::icons::chevron_down().size(TEXT_SM).color(t.text_muted);
    button(
        row![label.into(), chevron]
            .spacing(SPACE_XS)
            .align_y(Alignment::Center)
            .height(Length::Fill),
    )
    .height(CONTROL_SM)
    .padding(Padding {
        left: SPACE_MD,
        right: SPACE_XS,
        ..Padding::ZERO
    })
    .style(move |_, status| crate::ui::components::icon_button::style(t, status, open))
}

/// Floats `panel` under `anchor` while it is `Some`. A press outside both
/// (or Escape) publishes `on_dismiss`; so does a click inside the panel,
/// after the item's own message.
pub fn popover<'a, M: Clone + 'a>(
    anchor: impl Into<Element<'a, M>>,
    panel: Option<Element<'a, M>>,
    align: Align,
    on_dismiss: M,
) -> Element<'a, M> {
    Element::new(Popover {
        anchor: anchor.into(),
        panel,
        align,
        on_dismiss,
    })
}

/// Where a `panel` sized `size` goes under an anchor at `anchor`, kept
/// inside `viewport`.
pub fn place(anchor: Rectangle, size: Size, align: Align, viewport: Size) -> Point {
    let x = match align {
        Align::Start => anchor.x,
        Align::End => anchor.x + anchor.width - size.width,
    };
    let below = anchor.y + anchor.height + GAP;
    let y = if below + size.height > viewport.height {
        (anchor.y - GAP - size.height).max(0.0)
    } else {
        below
    };
    Point::new(x.clamp(0.0, (viewport.width - size.width).max(0.0)), y)
}

struct Popover<'a, M> {
    anchor: Element<'a, M>,
    panel: Option<Element<'a, M>>,
    align: Align,
    on_dismiss: M,
}

impl<M: Clone> Popover<'_, M> {
    fn widgets(&self) -> Vec<&dyn Widget<M, iced::Theme, iced::Renderer>> {
        std::iter::once(&self.anchor)
            .chain(self.panel.as_ref())
            .map(Element::as_widget)
            .collect()
    }
}

impl<M: Clone> Widget<M, iced::Theme, iced::Renderer> for Popover<'_, M> {
    fn children(&self) -> Vec<Tree> {
        self.widgets().into_iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.widgets());
    }

    fn size(&self) -> Size<Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.anchor.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.anchor.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.anchor
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
        self.anchor.as_widget_mut().update(
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
        self.anchor
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
        self.anchor
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
        let (anchor_tree, rest) = tree.children.split_first_mut()?;
        let inner = self
            .anchor
            .as_widget_mut()
            .overlay(anchor_tree, layout, renderer, viewport, translation);
        let panel = match (self.panel.as_mut(), rest.first_mut()) {
            (Some(panel), Some(panel_tree)) => Some(overlay::Element::new(Box::new(PanelOverlay {
                panel,
                tree: panel_tree,
                anchor: layout.bounds() + translation,
                align: self.align,
                on_dismiss: self.on_dismiss.clone(),
            }))),
            _ => None,
        };
        match (inner, panel) {
            (None, None) => None,
            (Some(one), None) | (None, Some(one)) => Some(one),
            (Some(a), Some(b)) => Some(overlay::Group::with_children(vec![a, b]).overlay()),
        }
    }
}

struct PanelOverlay<'a, 'b, M> {
    panel: &'b mut Element<'a, M>,
    tree: &'b mut Tree,
    anchor: Rectangle,
    align: Align,
    on_dismiss: M,
}

impl<M: Clone> overlay::Overlay<M, iced::Theme, iced::Renderer> for PanelOverlay<'_, '_, M> {
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::ZERO, bounds);
        let node = self.panel.as_widget_mut().layout(self.tree, renderer, &limits);
        let at = place(self.anchor, node.size(), self.align, bounds);
        node.move_to(at)
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();
        self.panel
            .as_widget()
            .draw(self.tree, renderer, theme, style, layout, cursor, &bounds);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
    ) {
        let bounds = layout.bounds();
        self.panel.as_widget_mut().update(
            self.tree, event, layout, cursor, renderer, clipboard, shell, &bounds,
        );
        let over_panel = cursor.is_over(bounds);
        match event {
            // The trigger toggles the menu itself; dismissing on it too would
            // close and immediately reopen the menu.
            Event::Mouse(mouse::Event::ButtonPressed(_))
                if !over_panel && !cursor.is_over(self.anchor) =>
            {
                shell.publish(self.on_dismiss.clone());
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if over_panel => {
                shell.publish(self.on_dismiss.clone());
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
                shell.publish(self.on_dismiss.clone());
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        self.panel
            .as_widget()
            .mouse_interaction(self.tree, layout, cursor, &bounds, renderer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Size = Size::new(1440.0, 900.0);

    #[test]
    fn panels_open_below_their_trigger_on_the_chosen_edge() {
        let anchor = Rectangle::new(Point::new(300.0, 50.0), Size::new(80.0, 24.0));
        let size = Size::new(200.0, 120.0);
        assert_eq!(place(anchor, size, Align::Start, VIEW), Point::new(300.0, 78.0));
        assert_eq!(place(anchor, size, Align::End, VIEW), Point::new(180.0, 78.0));
        let at_left_edge = Rectangle::new(Point::new(10.0, 50.0), Size::new(80.0, 24.0));
        assert_eq!(place(at_left_edge, size, Align::End, VIEW).x, 0.0, "clamped inside");
    }

    #[test]
    fn panels_flip_above_and_stay_inside_the_window() {
        let near_bottom = Rectangle::new(Point::new(1400.0, 860.0), Size::new(30.0, 24.0));
        let at = place(near_bottom, Size::new(200.0, 120.0), Align::Start, VIEW);
        assert_eq!(at.y, 860.0 - GAP - 120.0, "flips above");
        assert_eq!(at.x, 1240.0, "clamped to the right edge");
    }
}
