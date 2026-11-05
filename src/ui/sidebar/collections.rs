//! Collections panel (mockup `.sidebar`, Postman-like so it feels familiar):
//! filter box + `+` / import / `⋯` on top, a COLLECTIONS section, then the
//! collection → folder → request tree with counts and status icons.
//! Hovering a row shows `+` / `⋯`; the same actions are in the `+` and
//! right-click menus.

use iced::{
    Element, Length, Padding,
    widget::{Space, button, column, container, mouse_area, row, scrollable, text, text_input},
};

use super::tree::TreeIndex;
use crate::{
    app::AppState,
    domain::collection::SavedRequest,
    domain::group::Group,
    message::{ImportMsg, Message, RequestMsg, SidebarMsg},
    ui::{
        components::{
            atoms::{compact_icon_button, row_action, search_input, text as t},
            molecules::{
                menu, menu_divider, menu_item, tree_row,
                tree_row::{COLLECTION_ROW_H, ROW_H, RowFill},
            },
        },
        icons,
        theme::{MONO, Palette, TEXT_XS, UI_FONT_MEDIUM},
        tree_label::{tree_label_styled, tree_label_weighted},
        widgets::popup::popup,
    },
};

/// Horizontal padding of the tree body (mockup `.tree{padding:4px 6px 12px}`).
const TREE_PAD_X: f32 = 6.0;
/// Left padding of the section label and collection rows (aligned).
/// Mockup `.wf-heading`: 10px either side of the section label.
const SECTION_INDENT: f32 = 10.0;
const BASE_INDENT: f32 = 8.0;
/// Each nesting level: folder rows sit at 22px under a collection.
const STEP: f32 = 14.0;
/// Requests start this far right of their parent folder's indent
/// (mockup: folder 22px, its requests 44px).
const REQ_EXTRA: f32 = 22.0;
const METHOD_W: f32 = 34.0;
const COUNT_W: f32 = 28.0;
/// Folder row chrome before the name: chevron, gap, folder icon, gap.
const FOLDER_LEAD: f32 = 9.0 + 6.0 + 12.0 + 6.0;

fn folder_indent(depth: usize) -> f32 {
    BASE_INDENT + STEP * depth as f32
}

fn request_indent(depth: usize) -> f32 {
    folder_indent(depth.saturating_sub(1)) + REQ_EXTRA
}

/// `context_menu` slot id of the `+` dropdown (row ids are UUIDs, so it
/// cannot collide). Sharing the slot means opening one menu closes the other.
pub const ADD_MENU: &str = "sidebar:add";
/// Same slot, for the `⋯` (more actions) dropdown.
pub const MORE_MENU: &str = "sidebar:more";
/// Top of the dropdowns: just under the top bar's first row.
const ADD_MENU_TOP: f32 = 46.0;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let panel = panel(state);
    let dropdown = match state.sidebar.context_menu.as_deref() {
        Some(ADD_MENU) => add_menu(state),
        Some(MORE_MENU) => more_menu(),
        _ => return panel,
    };
    // Floating dropdown over the tree (Postman-style `+` / `⋯` menus).
    iced::widget::stack![
        panel,
        container(dropdown)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .padding(Padding {
                top: ADD_MENU_TOP,
                right: 8.0,
                bottom: 0.0,
                left: 8.0,
            }),
    ]
    .into()
}

fn panel(state: &AppState) -> Element<'_, Message> {
    // Mockup `.side-section` ⋯: import and export sit next to the label.
    let section_open = state.sidebar.context_menu.as_deref() == Some(SECTION_MENU);
    let section_more = crate::ui::widgets::mini::anchored_menu(
        crate::ui::widgets::mini::icon_button(
            icons::more(),
            "Import / export collections",
            Some(Message::Sidebar(SidebarMsg::ContextMenuOpened(
                SECTION_MENU.to_owned(),
            ))),
            section_open,
        ),
        section_open,
        more_menu_items(),
        230.0,
    );
    let header = container(
        row![
            section_header(
                "Collections",
                state.sidebar.collections_collapsed,
                SidebarMsg::CollectionsSectionToggled,
            ),
            section_more,
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center),
    )
    .padding(Padding {
        top: 10.0,
        right: TREE_PAD_X,
        bottom: 2.0,
        left: TREE_PAD_X,
    });
    let mut tree = column![].spacing(0);
    if !state.sidebar.collections_collapsed {
        if state.drafts.only_drafts {
            tree = tree.push(super::drafts::list(state));
        } else if state.collections.is_empty() {
            tree = tree.push(empty_state("No collections yet. Click + to create one.", 0));
        } else {
            let all: Vec<&SavedRequest> = state.requests.values().flatten().collect();
            let index = TreeIndex::build(&state.collections, &state.groups, &all, "");
            let mut any = false;
            for collection in state.collections.iter().filter(|c| index.shows(&c.id)) {
                any = true;
                tree = tree.push(collection_view(state, &index, collection));
            }
            if !any {
                tree = tree.push(empty_state("No requests or folders match.", 0));
            }
        }
    }
    let tree = container(tree).padding(Padding {
        top: 0.0,
        right: TREE_PAD_X,
        bottom: 12.0,
        left: TREE_PAD_X,
    });
    let environments = container(super::environment_list::view(state)).padding(Padding {
        left: TREE_PAD_X,
        right: TREE_PAD_X,
        bottom: 12.0,
        ..Padding::ZERO
    });
    let body = if state.sidebar.collections_collapsed {
        None
    } else {
        Some(
            scrollable(tree)
                .height(Length::Fill)
                .direction(iced::widget::scrollable::Direction::Vertical(
                    crate::ui::theme::grabbable_scrollbar(),
                ))
                .style(crate::ui::theme::thin_scrollbar)
                .into(),
        )
    };
    let sections = crate::ui::components::molecules::sidebar_sections(
        header.into(),
        body,
        environments.into(),
    );

    // Releasing the mouse anywhere over the tree ends any running drag.
    mouse_area(
        column![top_bar(state), sections]
            .spacing(0)
            .height(Length::Fill),
    )
    .on_release(Message::Sidebar(SidebarMsg::ItemDragEnd))
    .into()
}

/// Mockup `.side-section`: the `COLLECTIONS` label above the tree.
pub(super) fn section_header(
    label: &str,
    collapsed: bool,
    message: SidebarMsg,
) -> Element<'static, Message> {
    // A fixed-height button top-aligns its content; center the label in the
    // 24px hover band explicitly (mockup `.wf-heading`: align-items center).
    button(
        container(
            row![
                (if collapsed {
                    icons::chevron_right()
                } else {
                    icons::chevron_down()
                })
                .size(10)
                .color(Palette::text_muted()),
                text(label.to_uppercase())
                    .size(10)
                    .font(UI_FONT_MEDIUM)
                    .color(Palette::text_muted()),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .on_press(Message::Sidebar(message))
    .height(24)
    .width(Length::Fill)
    .style(|_, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then(|| Palette::hover().into()),
        text_color: Palette::text(),
        ..Default::default()
    })
    .padding(Padding {
        top: 0.0,
        right: 8.0,
        bottom: 0.0,
        left: SECTION_INDENT,
    })
    .into()
}

/// `⋯` dropdown: the less frequent collection-wide actions.
fn more_menu() -> Element<'static, Message> {
    menu(more_menu_items(), Length::Fixed(210.0))
}

/// `context_menu` slot id of the COLLECTIONS section's `⋯`.
pub const SECTION_MENU: &str = "sidebar:collections-section";

fn more_menu_items() -> Vec<Element<'static, Message>> {
    vec![
        menu_item(
            Some(icons::import_box()),
            "Import collections…",
            Some(Message::Import(ImportMsg::OpenPostmanDialog)),
            false,
        ),
        menu_item(
            Some(icons::export()),
            "Export all collections",
            Some(Message::Import(ImportMsg::ExportAllCollections)),
            false,
        ),
    ]
}

/// Mockup `.side-top`: filter box, `+` (new …), import, `⋯` (more); then
/// the draft filter chips while there are drafts.
fn top_bar(state: &AppState) -> Element<'_, Message> {
    let first = row![
        crate::ui::widgets::popup::popup_left(
            search_input("Search", &state.sidebar.filter, |s| Message::Sidebar(
                SidebarMsg::FilterChanged(s)
            ))
            .id("sidebar-search-input")
            .on_submit(Message::Sidebar(SidebarMsg::SearchConfirm))
            .into(),
            super::search::view(state),
            Message::Sidebar(SidebarMsg::SearchClose),
            state.sidebar.search_open,
        ),
        compact_icon_button(
            icons::plus(),
            "New…",
            Message::Sidebar(SidebarMsg::ContextMenuOpened(ADD_MENU.to_owned())),
            26.0,
        ),
        compact_icon_button(
            icons::import_box(),
            "Import Postman / OpenAPI / HTTPie / JSON",
            Message::Import(ImportMsg::OpenPostmanDialog),
            26.0,
        ),
        compact_icon_button(
            icons::more(),
            "More actions",
            Message::Sidebar(SidebarMsg::ContextMenuOpened(MORE_MENU.to_owned())),
            26.0,
        ),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    let body = column![first]
        .push(super::drafts::filter_bar(state))
        .spacing(8);

    container(body)
        .padding(Padding {
            top: 10.0,
            right: 10.0,
            bottom: 4.0,
            left: 10.0,
        })
        .into()
}

fn collection_view<'a>(
    state: &'a AppState,
    index: &TreeIndex,
    collection: &'a crate::domain::collection::Collection,
) -> Element<'a, Message> {
    let expanded = index.filtering() || state.sidebar.expanded.contains(&collection.id);
    let renaming = state.sidebar.col_renaming.as_deref() == Some(collection.id.as_str());
    let dragging = state.sidebar.dragging_item.as_deref() == Some(collection.id.as_str());
    let menu_open = state.sidebar.context_menu.as_deref() == Some(collection.id.as_str());

    let header: Element<Message> = if renaming {
        let rid = collection.id.clone();
        let rid_done = collection.id.clone();
        rename_input(
            text_input("Collection name", &collection.name)
                .id(crate::state::sidebar::collection_rename_input_id(&rid))
                .on_input(move |s| {
                    Message::Sidebar(SidebarMsg::RenameCollection {
                        id: rid.clone(),
                        name: s,
                    })
                })
                .on_submit(Message::Sidebar(SidebarMsg::ToggleRenameCollection(
                    rid_done,
                ))),
            BASE_INDENT,
        )
    } else {
        let (actions, actions_w) = row_actions(
            state,
            &collection.id,
            Some(Message::Sidebar(SidebarMsg::NewRequestIn {
                parent_id: collection.id.clone(),
                parent_is_collection: true,
            })),
        );
        let (trailing, trailing_w) =
            count_or_actions(actions, actions_w, index.count(&collection.id), true);
        // Label room: sidebar minus tree padding, row padding, chevron, trailing.
        let avail = state.sidebar_width - 2.0 * TREE_PAD_X - BASE_INDENT - 8.0 - 16.0 - trailing_w;
        let content = row![
            chevron(expanded, 10.0),
            tree_label_weighted(
                collection.name.clone(),
                12.0,
                avail,
                UI_FONT_MEDIUM,
                Palette::text()
            ),
            trailing,
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        // Press both opens the folder and begins tracking for reorder —
        // same pattern as the tab strip (see TabDragStart/TabDragEnd).
        // A plain click with no movement just toggles, because release
        // clears the drag state. Double-click renames (see ItemDragStart).
        draggable(
            tree_row(
                content,
                BASE_INDENT,
                COLLECTION_ROW_H,
                drag_fill(
                    dragging
                        || state.sidebar.hovered_row.as_deref() == Some(collection.id.as_str()),
                ),
                Message::Sidebar(SidebarMsg::ItemDragStart(collection.id.clone())),
            ),
            &collection.id,
        )
    };

    let header = if menu_open {
        popup(
            header,
            collection_menu(&collection.id),
            Message::Sidebar(SidebarMsg::ContextMenuClosed),
        )
    } else {
        header
    };
    let mut col = column![header].spacing(0);
    if expanded {
        let (groups, requests) = children_of(state, index, &collection.id, true);
        if groups.is_empty() && requests.is_empty() && !index.filtering() {
            col = col.push(empty_state(
                "Empty. Right-click the collection to add a request or folder.",
                1,
            ));
        }
        col = render_children(state, index, col, groups, requests, 1);
    }
    col.into()
}

/// Render a parent's children: sorted folders first, then sorted requests.
fn render_children<'a>(
    state: &'a AppState,
    index: &TreeIndex,
    mut col: iced::widget::Column<'a, Message>,
    groups: Vec<&'a Group>,
    requests: Vec<&'a SavedRequest>,
    depth: usize,
) -> iced::widget::Column<'a, Message> {
    for group in groups {
        col = col.push(group_view(state, index, group, depth));
    }
    for req in requests {
        col = col.push(request_view(state, req, depth));
    }
    col
}

fn children_of<'a>(
    state: &'a AppState,
    index: &TreeIndex,
    parent_id: &str,
    parent_is_collection: bool,
) -> (Vec<&'a Group>, Vec<&'a SavedRequest>) {
    let mut groups: Vec<&Group> = state
        .groups
        .iter()
        .filter(|g| g.parent_id == parent_id && g.parent_is_collection == parent_is_collection)
        .filter(|g| index.shows(&g.id))
        .collect();
    groups.sort_by_key(|g| g.sort_order);

    let mut requests: Vec<&SavedRequest> = if parent_is_collection {
        state
            .requests
            .get(parent_id)
            .map(|reqs| reqs.iter().filter(|r| r.group_id.is_none()).collect())
            .unwrap_or_default()
    } else {
        state
            .requests
            .values()
            .flatten()
            .filter(|r| r.group_id.as_deref() == Some(parent_id))
            .collect()
    };
    requests.retain(|r| index.shows(&r.id));
    requests.sort_by_key(|r| r.sort_order);
    (groups, requests)
}

fn group_view<'a>(
    state: &'a AppState,
    index: &TreeIndex,
    group: &'a Group,
    depth: usize,
) -> Element<'a, Message> {
    let expanded = index.filtering() || state.sidebar.expanded.contains(&group.id);
    let renaming = state.sidebar.group_renaming.as_deref() == Some(group.id.as_str());
    let dragging = state.sidebar.dragging_item.as_deref() == Some(group.id.as_str());
    let menu_open = state.sidebar.context_menu.as_deref() == Some(group.id.as_str());
    let indent = folder_indent(depth);

    let header: Element<Message> = if renaming {
        let gid = group.id.clone();
        let gid_done = group.id.clone();
        rename_input(
            text_input("Folder name", &group.name)
                .id(crate::state::sidebar::group_rename_input_id(&gid))
                .on_input(move |s| {
                    Message::Sidebar(SidebarMsg::RenameGroup {
                        id: gid.clone(),
                        name: s,
                    })
                })
                .on_submit(Message::Sidebar(SidebarMsg::ToggleRenameGroup(gid_done))),
            indent,
        )
    } else {
        let (actions, actions_w) = row_actions(
            state,
            &group.id,
            Some(Message::Sidebar(SidebarMsg::NewRequestIn {
                parent_id: group.id.clone(),
                parent_is_collection: false,
            })),
        );
        let (trailing, trailing_w) =
            count_or_actions(actions, actions_w, index.count(&group.id), false);
        let avail =
            state.sidebar_width - 2.0 * TREE_PAD_X - indent - 8.0 - FOLDER_LEAD - trailing_w;
        // Mockup `.folder-row`: chevron, folder icon, secondary-text name.
        let content = row![
            chevron(expanded, 9.0),
            icons::folder().size(12).color(Palette::text_subtle()),
            tree_label_styled(group.name.clone(), 12.0, avail, None, Palette::text_muted()),
            trailing,
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        draggable(
            tree_row(
                content,
                indent,
                ROW_H,
                drag_fill(
                    dragging || state.sidebar.hovered_row.as_deref() == Some(group.id.as_str()),
                ),
                Message::Sidebar(SidebarMsg::ItemDragStart(group.id.clone())),
            ),
            &group.id,
        )
    };

    let header = if menu_open {
        popup(
            header,
            group_menu(&group.id),
            Message::Sidebar(SidebarMsg::ContextMenuClosed),
        )
    } else {
        header
    };
    let mut col = column![header].spacing(0);
    if expanded {
        let (groups, requests) = children_of(state, index, &group.id, false);
        if groups.is_empty() && requests.is_empty() && !index.filtering() {
            col = col.push(empty_state("Empty folder.", depth + 1));
        }
        col = render_children(state, index, col, groups, requests, depth + 1);
    }
    col.into()
}

fn request_view<'a>(
    state: &'a AppState,
    req: &'a SavedRequest,
    depth: usize,
) -> Element<'a, Message> {
    let selected = state.examples.editor.is_none()
        && state.sidebar.environment_page.is_none()
        && state.sidebar.selected_request.as_deref() == Some(req.id.as_str());
    let active_parent = state
        .examples
        .editor
        .as_ref()
        .is_some_and(|e| e.data.request_id == req.id);
    let examples = state
        .examples
        .items
        .get(&req.id)
        .filter(|items| !items.is_empty());
    let expanded = state.examples.expanded.contains(&req.id);
    let dragging = state.sidebar.dragging_item.as_deref() == Some(req.id.as_str());
    let renaming = state.sidebar.req_renaming.as_deref() == Some(req.id.as_str());
    let menu_open = state.sidebar.context_menu.as_deref() == Some(req.id.as_str());
    let indent = request_indent(depth);

    let row_el: Element<Message> = if renaming {
        let rid = req.id.clone();
        let rcol = req.collection_id.clone();
        let rid_done = req.id.clone();
        rename_input(
            text_input("Request name", &req.name)
                .id(crate::state::sidebar::rename_input_id(&rid))
                .on_input(move |s| {
                    Message::Sidebar(SidebarMsg::RenameRequest {
                        id: rid.clone(),
                        collection_id: rcol.clone(),
                        name: s,
                    })
                })
                .on_submit(Message::Sidebar(SidebarMsg::ToggleRenameRequest(rid_done))),
            indent,
        )
    } else {
        let mark = super::drafts::mark_for(state, &req.id);
        let mark_w = if mark.is_some() {
            super::drafts::MARK_WIDTH + 6.0
        } else {
            0.0
        };
        let (actions, actions_w) = row_actions(state, &req.id, None);
        let avail = state.sidebar_width
            - 2.0 * TREE_PAD_X
            - indent
            - 8.0
            - METHOD_W
            - 7.0
            - mark_w
            - actions_w;
        // Mockup `.req-row`: names are secondary text; the selected one
        // reads as primary, medium weight, on the accent tint.
        let name = if selected || active_parent {
            tree_label_weighted(
                req.name.clone(),
                12.0,
                avail,
                UI_FONT_MEDIUM,
                Palette::text(),
            )
        } else {
            tree_label_styled(req.name.clone(), 12.0, avail, None, Palette::text_muted())
        };
        let disclosure: Element<Message> = if examples.is_some() {
            compact_icon_button(
                if expanded {
                    icons::chevron_down()
                } else {
                    icons::chevron_right()
                },
                if expanded {
                    "Collapse examples"
                } else {
                    "Expand examples"
                },
                Message::Example(crate::message::example::ExampleMsg::Toggle(req.id.clone())),
                16.0,
            )
        } else {
            Space::new().width(16).into()
        };
        let content = row![
            disclosure,
            container(
                text(method_label(req.method.as_str()))
                    .size(TEXT_XS)
                    .font(MONO)
                    .color(super::drafts::method_color(req.method.as_str())),
            )
            .width(METHOD_W),
            name,
        ]
        .push(mark)
        .push(actions)
        .spacing(7)
        .align_y(iced::Alignment::Center);
        // Updated mockup (frames 32/33): the active row is a neutral square
        // band, the same as the selected example and environment rows.
        let fill = if selected {
            RowFill::NeutralSelected
        } else {
            drag_fill(dragging || state.sidebar.hovered_row.as_deref() == Some(req.id.as_str()))
        };
        draggable(
            tree_row(
                content,
                (indent - 23.0).max(0.0),
                ROW_H,
                fill,
                Message::Sidebar(SidebarMsg::ItemDragStart(req.id.clone())),
            ),
            &req.id,
        )
    };

    let row_el = if menu_open {
        popup(
            row_el,
            request_menu(req),
            Message::Sidebar(SidebarMsg::ContextMenuClosed),
        )
    } else {
        row_el
    };
    let Some(examples) = examples else {
        return row_el;
    };
    let mut children = column![row_el];
    if expanded {
        for example in examples {
            let selected = state
                .examples
                .editor
                .as_ref()
                .is_some_and(|e| e.data.id == example.id);
            let label = tree_label_weighted(
                example.name.clone(),
                12.0,
                (state.sidebar_width - 2.0 * TREE_PAD_X - indent - METHOD_W - 20.0).max(45.0),
                UI_FONT_MEDIUM,
                Palette::text(),
            );
            let content = row![
                icons::example().size(12).color(Palette::text_muted()),
                label,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center);
            children = children.push(tree_row(
                content,
                indent + METHOD_W - 8.0,
                24.0,
                if selected {
                    RowFill::NeutralSelected
                } else {
                    RowFill::None
                },
                Message::Example(crate::message::example::ExampleMsg::Open(
                    example.id.clone(),
                )),
            ));
        }
    }
    children.into()
}

/// Every tree row: entering marks it hovered (and reorders onto it during a
/// drag), leaving clears that, right-click toggles its menu.
fn draggable<'a>(row_el: Element<'a, Message>, id: &str) -> Element<'a, Message> {
    mouse_area(row_el)
        .on_enter(Message::Sidebar(SidebarMsg::ItemDragOver(id.to_owned())))
        .on_exit(Message::Sidebar(SidebarMsg::RowHoverEnd(id.to_owned())))
        .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(
            id.to_owned(),
        )))
        .into()
}

const ACTION_W: f32 = 22.0;

fn drag_fill(dragging: bool) -> RowFill {
    if dragging {
        RowFill::Hover
    } else {
        RowFill::None
    }
}

/// Trailing slot of collection / folder rows: the request count (mockup
/// `.count` mono / `.f-mini`), swapped for the hover actions while shown.
fn count_or_actions(
    actions: Option<Element<'static, Message>>,
    actions_w: f32,
    n: usize,
    mono: bool,
) -> (Element<'static, Message>, f32) {
    if let Some(a) = actions {
        return (a, actions_w);
    }
    let mut label = text(n.to_string())
        .size(TEXT_XS)
        .color(Palette::text_subtle());
    if mono {
        label = label.font(MONO);
    }
    (
        container(label)
            .width(COUNT_W)
            .align_x(iced::alignment::Horizontal::Right)
            .into(),
        COUNT_W + 6.0,
    )
}

/// Postman's inline row actions, shown while the row is hovered or its
/// menu is open: optional `+` (add request), then `⋯` (the row's menu).
/// Returns the element and the width it takes from the label.
fn row_actions<'a>(
    state: &AppState,
    id: &str,
    add: Option<Message>,
) -> (Option<Element<'a, Message>>, f32) {
    let active = state.sidebar.hovered_row.as_deref() == Some(id)
        || state.sidebar.context_menu.as_deref() == Some(id);
    if !active || state.sidebar.dragging_item.is_some() {
        return (None, 0.0);
    }
    let mut actions = row![].spacing(2).align_y(iced::Alignment::Center);
    let mut n = 1.0;
    if let Some(msg) = add {
        actions = actions.push(row_action(icons::plus(), "Add request", msg));
        n += 1.0;
    }
    actions = actions.push(row_action(
        icons::more(),
        "More actions",
        Message::Sidebar(SidebarMsg::ContextMenuOpened(id.to_owned())),
    ));
    // Each button plus the spacing in front of the group.
    (Some(actions.into()), n * (ACTION_W + 2.0) + 6.0)
}

/// Postman shortens the long verbs so names line up.
fn method_label(method: &str) -> &str {
    match method {
        "DELETE" => "DEL",
        "OPTIONS" => "OPT",
        other => other,
    }
}

fn rename_input<'a>(
    input: iced::widget::TextInput<'a, Message>,
    indent: f32,
) -> Element<'a, Message> {
    container(input.size(12).padding([4, 8]).width(Length::Fill))
        .padding(Padding {
            top: 1.0,
            right: 8.0,
            bottom: 1.0,
            left: indent - 8.0,
        })
        .into()
}

fn chevron(expanded: bool, size: f32) -> Element<'static, Message> {
    let icon = if expanded {
        icons::chevron_down()
    } else {
        icons::chevron_right()
    };
    container(icon.size(size).color(Palette::text_subtle()))
        .width(10)
        .into()
}

fn empty_state<'a>(label: &'static str, depth: usize) -> Element<'a, Message> {
    container(t::meta(label))
        .padding(Padding {
            top: 4.0,
            right: 8.0,
            bottom: 4.0,
            left: request_indent(depth),
        })
        .into()
}

/// Right-click menu for a collection: add children, rename, export, delete.
fn collection_menu(col_id: &str) -> Element<'_, Message> {
    context_menu(vec![
        ctx_item(
            "New request",
            Message::Sidebar(SidebarMsg::NewRequestIn {
                parent_id: col_id.to_owned(),
                parent_is_collection: true,
            }),
            false,
        ),
        ctx_item(
            "New folder",
            Message::Sidebar(SidebarMsg::NewGroupIn {
                parent_id: col_id.to_owned(),
                parent_is_collection: true,
            }),
            false,
        ),
        menu_divider(),
        ctx_item(
            "Rename",
            Message::Sidebar(SidebarMsg::ToggleRenameCollection(col_id.to_owned())),
            false,
        ),
        ctx_item(
            "Export…",
            Message::Import(ImportMsg::OpenExportDialog(col_id.to_owned())),
            false,
        ),
        menu_divider(),
        ctx_item(
            "Delete",
            Message::Sidebar(SidebarMsg::DeleteCollection(col_id.to_owned())),
            true,
        ),
    ])
}

/// Right-click menu for a folder: add children, rename, delete.
fn group_menu(group_id: &str) -> Element<'_, Message> {
    context_menu(vec![
        ctx_item(
            "New request",
            Message::Sidebar(SidebarMsg::NewRequestIn {
                parent_id: group_id.to_owned(),
                parent_is_collection: false,
            }),
            false,
        ),
        ctx_item(
            "New folder",
            Message::Sidebar(SidebarMsg::NewGroupIn {
                parent_id: group_id.to_owned(),
                parent_is_collection: false,
            }),
            false,
        ),
        menu_divider(),
        ctx_item(
            "Rename",
            Message::Sidebar(SidebarMsg::ToggleRenameGroup(group_id.to_owned())),
            false,
        ),
        menu_divider(),
        ctx_item(
            "Delete",
            Message::Sidebar(SidebarMsg::DeleteGroup(group_id.to_owned())),
            true,
        ),
    ])
}

/// Right-click menu for a request: duplicate, rename, delete.
fn request_menu(req: &SavedRequest) -> Element<'_, Message> {
    context_menu(vec![
        ctx_item(
            "Add example",
            Message::Example(crate::message::example::ExampleMsg::Create(req.id.clone())),
            false,
        ),
        menu_divider(),
        ctx_item(
            "Duplicate",
            Message::Sidebar(SidebarMsg::CloneRequest {
                id: req.id.clone(),
                collection_id: req.collection_id.clone(),
            }),
            false,
        ),
        menu_divider(),
        ctx_item(
            "Rename",
            Message::Sidebar(SidebarMsg::ToggleRenameRequest(req.id.clone())),
            false,
        ),
        menu_divider(),
        ctx_item(
            "Delete",
            Message::Sidebar(SidebarMsg::RequestDeleteAsked {
                id: req.id.clone(),
                collection_id: req.collection_id.clone(),
            }),
            true,
        ),
    ])
}

/// The `+` dropdown (Postman-style): what to create. Folder and request go
/// into the folder or collection holding the selected request, else the
/// first collection; without any collection they are disabled.
fn add_menu(state: &AppState) -> Element<'_, Message> {
    let target = add_target(state);
    let folder = target.clone().map(|(parent_id, parent_is_collection)| {
        Message::Sidebar(SidebarMsg::NewGroupIn {
            parent_id,
            parent_is_collection,
        })
    });
    let saved = target.map(|(parent_id, parent_is_collection)| {
        Message::Sidebar(SidebarMsg::NewRequestIn {
            parent_id,
            parent_is_collection,
        })
    });
    menu(
        vec![
            menu_item(
                Some(icons::globe()),
                "HTTP",
                Some(Message::Request(RequestMsg::NewTab)),
                false,
            ),
            menu_item(
                Some(icons::zap()),
                "WebSocket",
                Some(Message::Request(RequestMsg::NewWsTab)),
                false,
            ),
            menu_divider(),
            menu_item(Some(icons::plus()), "Request in collection", saved, false),
            menu_item(Some(icons::folder_plus()), "Folder", folder, false),
            menu_item(
                Some(icons::collections()),
                "Collection",
                Some(Message::Sidebar(SidebarMsg::NewCollection)),
                false,
            ),
            menu_item(
                Some(icons::environments()),
                "Environment",
                Some(Message::Sidebar(SidebarMsg::EnvironmentCreated)),
                false,
            ),
        ],
        Length::Fixed(210.0),
    )
}

/// Where the `+` menu adds a folder or request: (parent id, is collection).
fn add_target(state: &AppState) -> Option<(String, bool)> {
    let selected = state
        .sidebar
        .selected_request
        .as_deref()
        .and_then(|id| state.requests.values().flatten().find(|r| r.id == id));
    match selected {
        Some(r) => Some(match &r.group_id {
            Some(g) => (g.clone(), false),
            None => (r.collection_id.clone(), true),
        }),
        None => state.collections.first().map(|c| (c.id.clone(), true)),
    }
}

/// Fixed-width popup card; positioning belongs to the anchored overlay.
fn context_menu(items: Vec<Element<'static, Message>>) -> Element<'static, Message> {
    menu(items, Length::Fixed(196.0))
}

/// Mockup `.ctx-item`: every entry carries its icon.
fn ctx_item(label: &'static str, msg: Message, danger: bool) -> Element<'static, Message> {
    let icon = match label {
        "New request" | "Add example" => icons::plus(),
        "New folder" => icons::folder_plus(),
        "Rename" => icons::edit(),
        "Export…" => icons::export(),
        "Duplicate" => icons::copy(),
        _ => icons::trash(),
    };
    menu_item(Some(icon), label, Some(msg), danger)
}
