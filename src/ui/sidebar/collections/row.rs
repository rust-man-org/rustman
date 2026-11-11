use std::ops::Range;
use std::sync::LazyLock;

use iced::widget::{Space, container, hover, row, stack, text, text_input};
use iced::{Alignment, Background, Element, Length, Padding};

use crate::app::tree::{DropZones, NodeKind, drop_zones};
use crate::domain::request::HttpMethod;
use crate::message::{Message, SidebarMsg};
use crate::state::sidebar::{Rename, TreeDrag, rename_input_id};
use crate::ui::components::icon_button::{IconSize, icon_button_toggled};
use crate::ui::components::menu::{self, Align};
use crate::ui::components::{button, field};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_SM, ICON_LG, ICON_SM, MONO, ROW_HEIGHT, SPACE_MD, SPACE_XS, TAB_INDICATOR,
    TEXT_CAPTION, TEXT_MD, Tokens,
};
use crate::ui::tree_label::{Face, Measured};

use super::drag_row::drag_row;
use super::menu::row_menu;
use super::cache::Line;
use super::model::{Row, RowKind};

/// The chevron box in front of collection and folder names.
const DISCLOSURE: f32 = ICON_LG;
/// One level of nesting: children start under their parent's name.
const INDENT: f32 = DISCLOSURE + SPACE_XS;
/// Room kept free at the right for the "…" button.
const ACTIONS: f32 = CONTROL_SM + SPACE_MD;

/// Width of the method column: the widest method label plus a gap, on the
/// spacing grid, so "OPTIONS" never touches the name after it.
pub static METHOD_COLUMN: LazyLock<f32> = LazyLock::new(|| {
    let widest = HttpMethod::all()
        .iter()
        .map(|m| Measured::new(m.as_str(), TEXT_CAPTION, Face::Mono).width())
        .fold(0.0, f32::max);
    ((widest + SPACE_MD) / SPACE_XS).ceil() * SPACE_XS
});

/// What the rows of one frame share.
pub struct Context<'a> {
    pub t: &'static Tokens,
    pub selected: Option<&'a str>,
    pub renaming: Option<&'a Rename>,
    pub menu: Option<&'a str>,
    pub drag: Option<Drag<'a>>,
}

/// An active drag: the dragged row and the rows nested under it.
pub struct Drag<'a> {
    pub state: &'a TreeDrag,
    pub kind: NodeKind,
    pub index: usize,
    pub subtree: Range<usize>,
}

impl<'a> Drag<'a> {
    pub fn find(drag: &'a TreeDrag, lines: &[Line]) -> Option<Self> {
        if !drag.active {
            return None;
        }
        let index = lines.iter().position(|l| l.row.id == drag.item && l.row.node_kind().is_some())?;
        let depth = lines[index].row.depth;
        let end = lines[index + 1..]
            .iter()
            .position(|l| l.row.depth <= depth)
            .map_or(lines.len(), |n| index + 1 + n);
        Some(Drag {
            state: drag,
            kind: lines[index].row.node_kind()?,
            index,
            subtree: index + 1..end,
        })
    }
}

/// The row element for `line`, the `index`th visible row.
pub fn view(cx: &Context<'_>, index: usize, line: &Line) -> Element<'static, Message> {
    let row = &line.row;
    if let RowKind::Empty { parent_is_collection } = row.kind {
        return empty_row(cx.t, row.id.clone(), row.depth, parent_is_collection);
    }
    if cx.renaming.is_some_and(|r| r.target.id() == row.id) {
        return rename_row(cx, row);
    }
    let menu_open = cx.menu == Some(row.id.as_str());
    let panel = menu_open.then(|| row_menu(cx.t, row));
    let id = row.id.clone();
    let selected = cx.selected == Some(row.id.as_str()) && matches!(row.kind, RowKind::Request { .. });
    let base = draggable(cx, index, line, selected);
    let actions = actions_button(cx.t, id, menu_open);
    if cx.drag.is_some() {
        return base;
    }
    if !(selected || menu_open) {
        return hover(base, actions);
    }
    let anchored = stack![base, actions];
    menu::popover(anchored, panel, Align::End, Message::Sidebar(SidebarMsg::ContextMenuClosed))
}

fn draggable(cx: &Context<'_>, index: usize, line: &Line, selected: bool) -> Element<'static, Message> {
    let t = cx.t;
    let row = &line.row;
    let id = row.id.clone();
    let row_kind = row.node_kind();
    let open = row.is_open();
    let dragged = cx.drag.as_ref().is_some_and(|d| d.index == index);
    let fill = if selected {
        Some(t.accent_soft)
    } else if dragged {
        Some(t.hover)
    } else {
        None
    };
    let indent = SPACE_MD + row.depth as f32 * INDENT;
    let padded = |indent: f32| {
        container(body(t, line)).height(Length::Fill).align_y(Alignment::Center).padding(Padding {
            left: indent,
            right: ACTIONS,
            ..Padding::ZERO
        })
    };
    let inner: Element<'static, Message> = if selected {
        let bar = container(Space::new().width(TAB_INDICATOR).height(Length::Fill)).style(move |_| container::Style {
            background: Some(Background::Color(t.accent)),
            ..Default::default()
        });
        row![bar, padded(indent - TAB_INDICATOR)].into()
    } else {
        padded(indent).into()
    };
    let content = container(inner)
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .style(move |_| container::Style {
            background: fill.map(Background::Color),
            ..Default::default()
        });

    let mut gesture = drag_row(content)
        .on_press(
            Message::Sidebar(SidebarMsg::ItemPressed(id.clone())),
            Message::Sidebar(SidebarMsg::ItemDragEnd),
        )
        .on_drag_start(Message::Sidebar(SidebarMsg::ItemDragStarted(id.clone())))
        .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(id.clone())))
        .hover_background((!selected).then_some(t.hover), 0.0)
        .indicator_colors(t.accent, t.accent_soft);
    if let (Some(drag), Some(row_kind)) = (&cx.drag, row_kind) {
        gesture = gesture
            .dragging(true)
            .indicator(drag.state.target.as_ref().filter(|d| d.id == id).map(|d| d.position));
        if let Some((zones, after_allowed)) = drop_slots(drag, index, row_kind, open) {
            let hover_id = id.clone();
            gesture = gesture.drop_target(
                zones,
                after_allowed,
                move |position| {
                    Message::Sidebar(SidebarMsg::ItemDragHover {
                        id: hover_id.clone(),
                        position,
                    })
                },
                Message::Sidebar(SidebarMsg::ItemDragLeave(id)),
            );
        }
    }
    gesture.into()
}

/// The drop zones the row at `index` offers the active drag, and whether
/// "after" is among them.
fn drop_slots(drag: &Drag<'_>, index: usize, target: NodeKind, open: bool) -> Option<(DropZones, bool)> {
    let zones = drop_zones(drag.kind, target, index == drag.index, drag.subtree.contains(&index));
    (zones != DropZones::default()).then_some((zones, !open))
}

/// The label face: collections stand out in medium weight.
pub fn face(row: &Row) -> Face {
    if matches!(row.kind, RowKind::Collection { .. }) {
        Face::Medium
    } else {
        Face::Regular
    }
}

/// Room for the label of `row` in a tree `width` px wide.
pub fn label_budget(row: &Row, width: f32) -> f32 {
    width - SPACE_MD - row.depth as f32 * INDENT - lead_width(row) - ACTIONS
}

fn lead_width(row: &Row) -> f32 {
    match row.kind {
        RowKind::Request { .. } => *METHOD_COLUMN,
        _ => INDENT,
    }
}

fn body(t: &'static Tokens, line: &Line) -> Element<'static, Message> {
    let lead: Element<'static, Message> = match &line.row.kind {
        RowKind::Request { method, .. } => text(method.as_str())
            .size(TEXT_CAPTION)
            .font(MONO)
            .color(t.method_color(method.as_str()))
            .width(*METHOD_COLUMN)
            .into(),
        RowKind::Collection { expanded } | RowKind::Folder { expanded } => {
            let chevron = if *expanded { icons::chevron_down() } else { icons::chevron_right() };
            row![
                container(chevron.size(ICON_SM).color(t.text_muted))
                    .center_x(DISCLOSURE)
                    .center_y(DISCLOSURE),
                Space::new().width(INDENT - DISCLOSURE),
            ]
            .into()
        }
        RowKind::Empty { .. } => Space::new().into(),
    };
    let label = line.label.view(line.fitted.as_deref(), t.text, t);
    row![lead, label].align_y(Alignment::Center).into()
}

fn actions_button(t: &'static Tokens, id: String, open: bool) -> Element<'static, Message> {
    let more = icon_button_toggled(t, icons::more(), IconSize::Small, open)
        .on_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(id)));
    container(more)
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .align_x(Alignment::End)
        .align_y(Alignment::Center)
        .padding(Padding {
            right: SPACE_XS,
            ..Padding::ZERO
        })
        .into()
}

fn rename_row(cx: &Context<'_>, row: &Row) -> Element<'static, Message> {
    let t = cx.t;
    let indent = SPACE_MD + row.depth as f32 * INDENT;
    let input = text_input("Name", &row.name)
        .id(rename_input_id())
        .on_input(|name| Message::Sidebar(SidebarMsg::RenameEdited(name)))
        .on_submit(Message::Sidebar(SidebarMsg::RenameFinished))
        .size(TEXT_MD)
        .padding([2.0, SPACE_XS + 2.0])
        .style(move |_, status| field::field(t, status));
    container(input)
        .padding(Padding {
            left: indent,
            right: SPACE_MD,
            ..Padding::ZERO
        })
        .height(ROW_HEIGHT)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

/// The child row of an open collection or folder that has none: an action
/// rather than a dead "empty" label.
fn empty_row(
    t: &'static Tokens,
    parent_id: String,
    depth: usize,
    parent_is_collection: bool,
) -> Element<'static, Message> {
    let content = row![
        icons::plus().size(ICON_SM),
        text("Add a request").size(TEXT_MD),
    ]
    .spacing(SPACE_XS + 2.0)
    .align_y(Alignment::Center);
    let add = iced::widget::button(content)
        .on_press(Message::Sidebar(SidebarMsg::NewRequestIn {
            parent_id,
            parent_is_collection,
        }))
        .padding([0.0, SPACE_XS])
        .height(CONTROL_SM)
        .style(move |_, status| button::style(t, button::Variant::Ghost, status));
    container(add)
        .padding(Padding {
            left: SPACE_MD + depth as f32 * INDENT - SPACE_XS,
            ..Padding::ZERO
        })
        .height(ROW_HEIGHT)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_method_column_leaves_a_gap_after_the_longest_method() {
        for method in HttpMethod::all() {
            let label = Measured::new(method.as_str(), TEXT_CAPTION, Face::Mono).width();
            assert!(*METHOD_COLUMN - label >= SPACE_MD, "{} leaves {:.1}px", method.as_str(), *METHOD_COLUMN - label);
        }
        assert_eq!(*METHOD_COLUMN % SPACE_XS, 0.0, "on the spacing grid");
    }
}
