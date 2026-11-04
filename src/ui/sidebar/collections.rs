//! Collections panel, laid out like Postman's sidebar so it feels familiar:
//! filter box + `+` / import / `⋯` on top, a COLLECTIONS section, then the
//! collection → folder → request tree. Hovering a row shows `+` / `⋯`
//! (Postman); the same actions are in the `+` and right-click menus.

use iced::{
    Element, Length, Padding,
    widget::{column, container, mouse_area, row, scrollable, text, text_input},
};

use super::tree::TreeIndex;
use crate::{
    app::AppState,
    domain::collection::SavedRequest,
    domain::group::Group,
    message::{ImportMsg, Message, RequestMsg, SidebarMsg},
    ui::{
        components::{
            atoms::{icon_button, row_action, search_input, text as t},
            molecules::{
                menu, menu_divider, menu_item, tree_row,
                tree_row::ROW_H,
            },
        },
        icons,
        theme::{MONO, Palette, TEXT_XS},
        tree_label::tree_label,
        widgets::kv_table,
    },
};

/// Horizontal padding of the tree body.
const TREE_PAD_X: f32 = 6.0;
/// Left padding of the `COLLECTIONS` section label.
const SECTION_INDENT: f32 = 8.0;
/// Left padding of a collection row (nested under the section label like
/// Postman); each further nesting level adds `STEP`.
const BASE_INDENT: f32 = 22.0;
const STEP: f32 = 14.0;
/// Requests sit a little right of folders so the method lines up under
/// the folder name rather than its chevron.
const REQ_EXTRA: f32 = 6.0;
const METHOD_W: f32 = 30.0;

fn folder_indent(depth: usize) -> f32 {
    BASE_INDENT + STEP * depth as f32
}

fn request_indent(depth: usize) -> f32 {
    folder_indent(depth) + REQ_EXTRA
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
    let mut col = column![top_bar(state)].spacing(0);

    // Local drafts (01): the Drafts view is a flat list.
    if state.drafts.only_drafts {
        return column![
            col,
            scrollable(super::drafts::list(state))
                .height(Length::Fill)
                .style(crate::ui::theme::thin_scrollbar),
        ]
        .into();
    }

    if state.collections.is_empty() {
        return col
            .push(kv_table::empty_state(
                "No collections yet.\nClick + to create one.",
            ))
            .height(Length::Fill)
            .into();
    }

    let all: Vec<&SavedRequest> = state.requests.values().flatten().collect();
    let index = TreeIndex::build(&state.collections, &state.groups, &all, &state.sidebar.filter);

    let mut tree = column![section_header("Collections")].spacing(0);
    let mut any = false;
    for collection in state.collections.iter().filter(|c| index.shows(&c.id)) {
        any = true;
        tree = tree.push(collection_view(state, &index, collection));
    }
    if !any {
        tree = tree.push(empty_state("No requests or folders match.", 0));
    }
    col = col.push(container(tree).padding(Padding {
        top: 2.0,
        right: TREE_PAD_X,
        bottom: 12.0,
        left: TREE_PAD_X,
    }));

    // Releasing the mouse anywhere over the tree ends any running drag.
    mouse_area(
        scrollable(col)
            .height(Length::Fill)
            .direction(iced::widget::scrollable::Direction::Vertical(
                crate::ui::theme::grabbable_scrollbar(),
            ))
            .style(crate::ui::theme::thin_scrollbar),
    )
    .on_release(Message::Sidebar(SidebarMsg::ItemDragEnd))
    .into()
}

/// Postman `COLLECTIONS` section label above the tree.
fn section_header(label: &str) -> Element<'static, Message> {
    container(
        row![
            icons::chevron_down().size(10).color(Palette::text_subtle()),
            text(label.to_uppercase())
                .size(TEXT_XS)
                .font(crate::ui::theme::UI_FONT_MEDIUM)
                .color(Palette::text_muted()),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .height(28)
    .center_y(28)
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
    menu(
        vec![
            menu_item(
                Some(icons::import()),
                "Import…",
                Some(Message::Import(ImportMsg::OpenPostmanDialog)),
                false,
            ),
            menu_item(
                Some(icons::export()),
                "Export all collections",
                Some(Message::Import(ImportMsg::ExportAllCollections)),
                false,
            ),
        ],
        Length::Fixed(210.0),
    )
}

/// Postman-style top bar: filter box, `+` (new …), import, `⋯` (more);
/// then the draft filter chips.
fn top_bar(state: &AppState) -> Element<'_, Message> {
    let first = row![
        search_input("Search", &state.sidebar.filter, |s| {
            Message::Sidebar(SidebarMsg::FilterChanged(s))
        }),
        icon_button(
            icons::plus(),
            "New…",
            Message::Sidebar(SidebarMsg::ContextMenuOpened(ADD_MENU.to_owned())),
        ),
        icon_button(
            icons::import(),
            "Import Postman / OpenAPI / HTTPie / JSON",
            Message::Import(ImportMsg::OpenPostmanDialog),
        ),
        icon_button(
            icons::more(),
            "More actions",
            Message::Sidebar(SidebarMsg::ContextMenuOpened(MORE_MENU.to_owned())),
        ),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center);

    let body = column![first]
        .push(super::drafts::filter_bar(state))
        .spacing(8);

    container(body)
        .padding(Padding {
            top: 10.0,
            right: 8.0,
            bottom: 6.0,
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
        // Label room: sidebar minus tree padding, row padding, chevron, actions.
        let avail = state.sidebar_width
            - 2.0 * TREE_PAD_X
            - BASE_INDENT
            - 8.0
            - 16.0
            - actions_w;
        let content = row![
            chevron(expanded, 10.0),
            tree_label(collection.name.clone(), 12.0, avail),
        ]
        .push(actions)
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
                ROW_H,
                dragging,
                Message::Sidebar(SidebarMsg::ItemDragStart(collection.id.clone())),
            ),
            &collection.id,
        )
    };

    let mut col = column![header].spacing(0);
    if menu_open {
        col = col.push(collection_menu(&collection.id));
    }
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
        let avail = state.sidebar_width
            - 2.0 * TREE_PAD_X
            - indent
            - 8.0
            - 15.0
            - actions_w;
        let content = row![
            chevron(expanded, 9.0),
            tree_label(group.name.clone(), 12.0, avail),
        ]
        .push(actions)
        .spacing(6)
        .align_y(iced::Alignment::Center);
        draggable(
            tree_row(
                content,
                indent,
                ROW_H,
                dragging,
                Message::Sidebar(SidebarMsg::ItemDragStart(group.id.clone())),
            ),
            &group.id,
        )
    };

    let mut col = column![header].spacing(0);
    if menu_open {
        col = col.push(group_menu(&group.id, indent + 8.0));
    }
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
    let selected = state.sidebar.selected_request.as_deref() == Some(req.id.as_str());
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
            - 6.0
            - mark_w
            - actions_w;
        let content = row![
            container(
                text(method_label(req.method.as_str()))
                    .size(9)
                    .font(MONO)
                    .color(super::drafts::method_color(req.method.as_str())),
            )
            .width(METHOD_W),
            tree_label(req.name.clone(), 12.0, avail),
        ]
        .push(mark)
        .push(actions)
        .spacing(6)
        .align_y(iced::Alignment::Center);
        draggable(
            tree_row(
                content,
                indent,
                ROW_H,
                selected || dragging,
                Message::Sidebar(SidebarMsg::ItemDragStart(req.id.clone())),
            ),
            &req.id,
        )
    };

    if menu_open {
        column![row_el, request_menu(req, indent + 8.0)]
            .spacing(0)
            .width(Length::Fill)
            .into()
    } else {
        row_el
    }
}

/// Every tree row: entering marks it hovered (and reorders onto it during a
/// drag), leaving clears that, right-click toggles its menu.
fn draggable<'a>(row_el: Element<'a, Message>, id: &str) -> Element<'a, Message> {
    mouse_area(row_el)
        .on_enter(Message::Sidebar(SidebarMsg::ItemDragOver(id.to_owned())))
        .on_exit(Message::Sidebar(SidebarMsg::RowHoverEnd(id.to_owned())))
        .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(id.to_owned())))
        .into()
}

const ACTION_W: f32 = 22.0;

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
    context_menu(
        vec![
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
            ctx_item(
                "Delete",
                Message::Sidebar(SidebarMsg::DeleteCollection(col_id.to_owned())),
                true,
            ),
        ],
        BASE_INDENT + 8.0,
    )
}

/// Right-click menu for a folder: add children, rename, delete.
fn group_menu(group_id: &str, indent: f32) -> Element<'_, Message> {
    context_menu(
        vec![
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
            ctx_item(
                "Rename",
                Message::Sidebar(SidebarMsg::ToggleRenameGroup(group_id.to_owned())),
                false,
            ),
            ctx_item(
                "Delete",
                Message::Sidebar(SidebarMsg::DeleteGroup(group_id.to_owned())),
                true,
            ),
        ],
        indent,
    )
}

/// Right-click menu for a request: duplicate, rename, delete.
fn request_menu(req: &SavedRequest, indent: f32) -> Element<'_, Message> {
    context_menu(
        vec![
            ctx_item(
                "Duplicate",
                Message::Sidebar(SidebarMsg::CloneRequest {
                    id: req.id.clone(),
                    collection_id: req.collection_id.clone(),
                }),
                false,
            ),
            ctx_item(
                "Rename",
                Message::Sidebar(SidebarMsg::ToggleRenameRequest(req.id.clone())),
                false,
            ),
            ctx_item(
                "Delete",
                Message::Sidebar(SidebarMsg::DeleteRequest {
                    id: req.id.clone(),
                    collection_id: req.collection_id.clone(),
                }),
                true,
            ),
        ],
        indent,
    )
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
    let selected = state.sidebar.selected_request.as_deref().and_then(|id| {
        state
            .requests
            .values()
            .flatten()
            .find(|r| r.id == id)
    });
    match selected {
        Some(r) => Some(match &r.group_id {
            Some(g) => (g.clone(), false),
            None => (r.collection_id.clone(), true),
        }),
        None => state.collections.first().map(|c| (c.id.clone(), true)),
    }
}

/// Right-click menu card, indented under its row.
fn context_menu(items: Vec<Element<'static, Message>>, indent: f32) -> Element<'static, Message> {
    container(menu(items, Length::Fill))
        .padding(Padding {
            top: 2.0,
            right: 8.0,
            bottom: 2.0,
            left: indent,
        })
        .width(Length::Fill)
        .into()
}

fn ctx_item(label: &'static str, msg: Message, danger: bool) -> Element<'static, Message> {
    menu_item(None, label, Some(msg), danger)
}
