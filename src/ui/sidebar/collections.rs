use iced::{
    Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, mouse_area, row, scrollable, text, text_input},
};

use crate::{
    app::AppState,
    domain::collection::SavedRequest,
    domain::group::Group,
    message::{ImportMsg, Message, SidebarMsg},
    ui::{
        icons,
        theme::{Palette, TEXT_LG},
        tree_label::tree_label,
        widgets::kv_table,
    },
};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let action_bar = container(
        row![
            text("Collections")
                .size(TEXT_LG)
                .color(Palette::text())
                .font(crate::ui::theme::UI_FONT_MEDIUM),
            Space::new().width(Length::Fill),
            icon_btn_only(
                icons::import(),
                "Import Postman / OpenAPI / HTTPie / JSON",
                Message::Import(ImportMsg::OpenPostmanDialog)
            ),
            icon_btn_only(
                icons::export(),
                "Export all collections (Rustman JSON)",
                Message::Import(ImportMsg::ExportAllCollections)
            ),
            icon_btn_only(
                icons::plus(),
                "New Collection",
                Message::Sidebar(SidebarMsg::NewCollection)
            ),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(2)
        .padding([10, 10]),
    )
    .width(Length::Fill);

    let mut col = column![
        action_bar,
        iced::widget::rule::horizontal(1.0).style(crate::ui::styles::divider)
    ]
    .spacing(0);

    // Local drafts (01): All / Drafts filter; the Drafts view is a flat list.
    col = col.push(super::drafts::filter_bar(state));
    if state.drafts.only_drafts {
        return scrollable(col.push(super::drafts::list(state)))
            .height(Length::Fill)
            .style(crate::ui::theme::thin_scrollbar)
            .into();
    }

    for collection in &state.collections {
        let is_expanded = state.sidebar.expanded.contains(&collection.id);
        let arrow = if is_expanded {
            icons::chevron_down()
        } else {
            icons::chevron_right()
        };

        let renaming = state.sidebar.col_renaming.as_deref() == Some(collection.id.as_str());
        let dragging = state.sidebar.dragging_item.as_deref() == Some(collection.id.as_str());
        let menu_open = state.sidebar.context_menu.as_deref() == Some(collection.id.as_str());

        let name_part: Element<Message> = if renaming {
            let rid = collection.id.clone();
            let rid_done = collection.id.clone();
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
                )))
                .size(12)
                .padding([4, 8])
                .width(Length::Fill)
                .into()
        } else {
            // Press both opens the folder and begins tracking for reorder —
            // same pattern as the tab strip (see TabDragStart/TabDragEnd).
            // A plain click with no movement just toggles, because release
            // clears the drag state.
            let cid = collection.id.clone();
            mouse_area(
                button(
                    row![
                        arrow.size(10).color(Palette::text_muted()),
                        // Label space: content width minus chevron, button
                        // padding and the two action buttons (~72px chrome).
                        tree_label(collection.name.clone(), 12.0, state.sidebar_width - 72.0),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::Sidebar(SidebarMsg::ItemDragStart(cid)))
                .style(|_t, s| iced::widget::button::Style {
                    background: if matches!(s, iced::widget::button::Status::Hovered) {
                        Some(Background::Color(Palette::hover()))
                    } else {
                        None
                    },
                    text_color: Palette::text(),
                    ..Default::default()
                })
                .width(Length::Fill)
                .padding([5, 8]),
            )
            // Right-click opens the collection menu (rename/export/delete);
            // double-click renames in place (see ItemDragStart).
            .on_enter(Message::Sidebar(SidebarMsg::ItemDragOver(
                collection.id.clone(),
            )))
            .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(
                collection.id.clone(),
            )))
            .into()
        };

        let header = row![
            name_part,
            hover_icon_btn(
                icons::folder_plus().size(10),
                Message::Sidebar(SidebarMsg::NewGroupIn {
                    parent_id: collection.id.clone(),
                    parent_is_collection: true,
                })
            ),
            hover_icon_btn(
                icons::plus().size(10),
                Message::Sidebar(SidebarMsg::NewRequestIn {
                    parent_id: collection.id.clone(),
                    parent_is_collection: true,
                })
            ),
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        col = col.push(container(header).style(move |_| dragging_style(dragging)));

        if menu_open {
            col = col.push(collection_menu(&collection.id));
        }

        if is_expanded {
            let (groups, requests) = children_of(state, &collection.id, true);
            if groups.is_empty() && requests.is_empty() {
                col = col.push(empty_state(
                    "No requests. Click + to add a folder or request.",
                    1,
                ));
            }
            col = render_children(state, col, groups, requests, 1);
        }
    }

    if state.collections.is_empty() {
        return col
            .push(kv_table::empty_state(
                "No collections yet.\nClick + New to create one.",
            ))
            .height(Length::Fill)
            .into();
    }

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

/// Render a parent's children: sorted folders first, then sorted requests.
fn render_children<'a>(
    state: &'a AppState,
    mut col: iced::widget::Column<'a, Message>,
    groups: Vec<&'a Group>,
    requests: Vec<&'a SavedRequest>,
    depth: usize,
) -> iced::widget::Column<'a, Message> {
    for group in groups {
        col = col.push(group_view(state, group, depth));
    }
    for req in requests {
        col = col.push(request_view(state, req, depth));
    }
    col
}

fn children_of<'a>(
    state: &'a AppState,
    parent_id: &str,
    parent_is_collection: bool,
) -> (Vec<&'a Group>, Vec<&'a SavedRequest>) {
    let mut groups: Vec<&Group> = state
        .groups
        .iter()
        .filter(|g| g.parent_id == parent_id && g.parent_is_collection == parent_is_collection)
        .collect();
    groups.sort_by_key(|g| g.sort_order);

    let requests: Vec<&SavedRequest> = if parent_is_collection {
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
    let mut requests = requests;
    requests.sort_by_key(|r| r.sort_order);
    (groups, requests)
}

fn group_view<'a>(state: &'a AppState, group: &'a Group, depth: usize) -> Element<'a, Message> {
    let is_expanded = state.sidebar.expanded.contains(&group.id);
    let arrow = if is_expanded {
        icons::chevron_down()
    } else {
        icons::chevron_right()
    };

    let renaming = state.sidebar.group_renaming.as_deref() == Some(group.id.as_str());
    let dragging = state.sidebar.dragging_item.as_deref() == Some(group.id.as_str());
    let menu_open = state.sidebar.context_menu.as_deref() == Some(group.id.as_str());
    let pad_left = 8.0 + (depth as f32) * 12.0;

    let name_part: Element<Message> = if renaming {
        let gid = group.id.clone();
        let gid_done = group.id.clone();
        text_input("Folder name", &group.name)
            .id(crate::state::sidebar::group_rename_input_id(&gid))
            .on_input(move |s| {
                Message::Sidebar(SidebarMsg::RenameGroup {
                    id: gid.clone(),
                    name: s,
                })
            })
            .on_submit(Message::Sidebar(SidebarMsg::ToggleRenameGroup(gid_done)))
            .size(12)
            .padding([4, 8])
            .width(Length::Fill)
            .into()
    } else {
        let gid = group.id.clone();
        mouse_area(
            button(
                row![
                    arrow.size(10).color(Palette::text_muted()),
                    icons::folder().size(12).color(Palette::text_muted()),
                    tree_label(
                        group.name.clone(),
                        12.0,
                        state.sidebar_width - pad_left - 72.0
                    ),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::Sidebar(SidebarMsg::ItemDragStart(gid)))
            .style(|_t, s| iced::widget::button::Style {
                background: if matches!(s, iced::widget::button::Status::Hovered) {
                    Some(Background::Color(Palette::hover()))
                } else {
                    None
                },
                text_color: Palette::text(),
                ..Default::default()
            })
            .width(Length::Fill)
            .padding([5, 8]),
        )
        .on_enter(Message::Sidebar(SidebarMsg::ItemDragOver(group.id.clone())))
        .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(
            group.id.clone(),
        )))
        .into()
    };

    let header = row![
        name_part,
        hover_icon_btn(
            icons::folder_plus().size(10),
            Message::Sidebar(SidebarMsg::NewGroupIn {
                parent_id: group.id.clone(),
                parent_is_collection: false,
            })
        ),
        hover_icon_btn(
            icons::plus().size(10),
            Message::Sidebar(SidebarMsg::NewRequestIn {
                parent_id: group.id.clone(),
                parent_is_collection: false,
            })
        ),
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    let mut col = column![container(header).style(move |_| dragging_style(dragging))].spacing(0);

    if menu_open {
        col = col.push(group_menu(&group.id, pad_left + 16.0));
    }

    if is_expanded {
        let parent = group.id.clone();
        let (groups, requests) = children_of(state, &parent, false);
        if groups.is_empty() && requests.is_empty() {
            col = col.push(empty_state("No requests.", depth + 1));
        }
        col = render_children(state, col, groups, requests, depth + 1);
    }

    container(col)
        .padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: pad_left,
        })
        .width(Length::Fill)
        .into()
}

fn request_view<'a>(
    state: &'a AppState,
    req: &'a SavedRequest,
    depth: usize,
) -> Element<'a, Message> {
    let is_selected = state.sidebar.selected_request.as_deref() == Some(req.id.as_str());
    let dragging = state.sidebar.dragging_item.as_deref() == Some(req.id.as_str());
    let method_color = method_color(req.method.as_str());
    let req_renaming = state.sidebar.req_renaming.as_deref() == Some(req.id.as_str());
    let menu_open = state.sidebar.context_menu.as_deref() == Some(req.id.as_str());
    let pad_left = 8.0 + (depth as f32) * 12.0 + 8.0;
    let draft_badge = super::drafts::badge_for(state, &req.id);
    let badge_w = if draft_badge.is_some() {
        super::drafts::BADGE_WIDTH
    } else {
        0.0
    };

    let name_part: Element<Message> = if req_renaming {
        let rid = req.id.clone();
        let rcol = req.collection_id.clone();
        let rid_done = req.id.clone();
        text_input("Request name", &req.name)
            .id(crate::state::sidebar::rename_input_id(&rid))
            .on_input(move |s| {
                Message::Sidebar(SidebarMsg::RenameRequest {
                    id: rid.clone(),
                    collection_id: rcol.clone(),
                    name: s,
                })
            })
            .on_submit(Message::Sidebar(SidebarMsg::ToggleRenameRequest(rid_done)))
            .size(12)
            .padding(iced::Padding {
                top: 4.0,
                right: 4.0,
                bottom: 4.0,
                left: 8.0,
            })
            .width(Length::Fill)
            .into()
    } else {
        let rid = req.id.clone();
        mouse_area(
            button(
                row![
                    container(
                        text(req.method.as_str())
                            .size(9)
                            .color(method_color)
                            .font(crate::ui::theme::MONO),
                    )
                    .width(32),
                    // Label space: content width minus indent, method badge,
                    // clone button and padding (~68px chrome).
                    tree_label(
                        req.name.clone(),
                        12.0,
                        state.sidebar_width - pad_left - 68.0 - badge_w
                    ),
                ]
                .push(draft_badge)
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::Sidebar(SidebarMsg::ItemDragStart(rid)))
            .style(move |t, s| req_item_style(t, s, is_selected))
            .width(Length::Fill)
            .padding(iced::Padding {
                top: 5.0,
                right: 4.0,
                bottom: 5.0,
                left: 8.0,
            }),
        )
        .on_enter(Message::Sidebar(SidebarMsg::ItemDragOver(req.id.clone())))
        .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(
            req.id.clone(),
        )))
        .into()
    };

    let item_row = container(
        row![
            name_part,
            hover_icon_btn(
                icons::copy().size(10),
                Message::Sidebar(SidebarMsg::CloneRequest {
                    id: req.id.clone(),
                    collection_id: req.collection_id.clone(),
                }),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .style(move |_| iced::widget::container::Style {
        background: if is_selected {
            Some(Background::Color(Palette::accent_soft()))
        } else if dragging {
            Some(Background::Color(Palette::hover()))
        } else {
            None
        },
        border: Border {
            color: if is_selected {
                Color {
                    a: 0.35,
                    ..Palette::accent()
                }
            } else {
                Color::TRANSPARENT
            },
            width: if is_selected { 1.0 } else { 0.0 },
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .padding(iced::Padding {
        top: 1.0,
        right: 6.0,
        bottom: 1.0,
        left: pad_left,
    })
    .width(Length::Fill);

    let item_row: Element<Message> = item_row.into();
    if menu_open {
        column![item_row, request_menu(req, pad_left + 16.0),]
            .spacing(0)
            .width(Length::Fill)
            .into()
    } else {
        item_row
    }
}

fn empty_state<'a>(text_label: &'static str, depth: usize) -> Element<'a, Message> {
    container(text(text_label).size(11).color(Palette::text_subtle()))
        .padding(iced::Padding {
            top: 2.0,
            right: 8.0,
            bottom: 2.0,
            left: 8.0 + (depth as f32) * 12.0 + 24.0,
        })
        .into()
}

/// Right-click menu for a collection header. Export and delete live here
/// now (instead of hover buttons); rename is here too since double-click is
/// undiscoverable on its own.
fn collection_menu(col_id: &str) -> Element<'_, Message> {
    let ren_id = col_id.to_owned();
    let exp_id = col_id.to_owned();
    let del_id = col_id.to_owned();
    context_menu(
        vec![
            menu_item(
                "Rename",
                Message::Sidebar(SidebarMsg::ToggleRenameCollection(ren_id)),
                false,
            ),
            menu_item(
                "Export…",
                Message::Import(ImportMsg::OpenExportDialog(exp_id)),
                false,
            ),
            menu_item(
                "Delete",
                Message::Sidebar(SidebarMsg::DeleteCollection(del_id)),
                true,
            ),
        ],
        24.0,
    )
}

/// Right-click menu for a folder row: rename + delete.
fn group_menu(group_id: &str, indent: f32) -> Element<'_, Message> {
    let ren_id = group_id.to_owned();
    let del_id = group_id.to_owned();
    context_menu(
        vec![
            menu_item(
                "Rename",
                Message::Sidebar(SidebarMsg::ToggleRenameGroup(ren_id)),
                false,
            ),
            menu_item(
                "Delete",
                Message::Sidebar(SidebarMsg::DeleteGroup(del_id)),
                true,
            ),
        ],
        indent,
    )
}

/// Right-click menu for a request row: rename + delete (clone stays a hover
/// button next to the name).
fn request_menu(req: &SavedRequest, indent: f32) -> Element<'_, Message> {
    let ren_id = req.id.clone();
    let del_id = req.id.clone();
    let del_col = req.collection_id.clone();
    context_menu(
        vec![
            menu_item(
                "Rename",
                Message::Sidebar(SidebarMsg::ToggleRenameRequest(ren_id)),
                false,
            ),
            menu_item(
                "Delete",
                Message::Sidebar(SidebarMsg::DeleteRequest {
                    id: del_id,
                    collection_id: del_col,
                }),
                true,
            ),
        ],
        indent,
    )
}

/// Shared right-click menu card: bordered popover, indented under its row.
fn context_menu(items: Vec<Element<'static, Message>>, indent: f32) -> Element<'static, Message> {
    container(
        container(column(items).spacing(1))
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Palette::surface_raised())),
                border: Border {
                    color: Palette::border(),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
            .padding(4)
            .width(Length::Fill),
    )
    .padding(iced::Padding {
        top: 2.0,
        right: 8.0,
        bottom: 2.0,
        left: indent,
    })
    .width(Length::Fill)
    .into()
}

fn menu_item(label: &'static str, msg: Message, danger: bool) -> Element<'static, Message> {
    button(
        row![text(label).size(12).color(if danger {
            Palette::ERROR
        } else {
            Palette::text()
        })]
        .width(Length::Fill),
    )
    .on_press(msg)
    .style(move |_t, s| iced::widget::button::Style {
        background: if matches!(s, iced::widget::button::Status::Hovered) {
            Some(Background::Color(if danger {
                Palette::error_soft()
            } else {
                Palette::hover()
            }))
        } else {
            None
        },
        border: Border {
            radius: 5.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .padding([6, 10])
    .width(Length::Fill)
    .into()
}

/// Container style applied while an item is mid-drag (issue #53 reorder).
fn dragging_style(dragging: bool) -> iced::widget::container::Style {
    let mut style = iced::widget::container::Style::default();
    if dragging {
        style.background = Some(Background::Color(Palette::hover()));
    }
    style
}

fn method_color(method: &str) -> iced::Color {
    match method {
        "GET" => Palette::GET,
        "POST" => Palette::POST,
        "PUT" => Palette::PUT,
        "PATCH" => Palette::PATCH,
        "DELETE" => Palette::DELETE,
        _ => Palette::HEAD,
    }
}

fn hover_icon_btn(
    icon: iced::widget::Text<'static>,
    msg: Message,
) -> iced::Element<'static, Message> {
    button(icon)
        .on_press(msg)
        .style(|_t, s| iced::widget::button::Style {
            text_color: if matches!(s, iced::widget::button::Status::Hovered) {
                Palette::accent()
            } else {
                Palette::text_subtle()
            },
            ..Default::default()
        })
        .padding([3, 4])
        .into()
}

fn icon_btn_only(
    icon: iced::widget::Text<'static>,
    _tooltip: &str,
    msg: Message,
) -> iced::Element<'static, Message> {
    button(icon.size(12).color(Palette::text_muted()))
        .on_press(msg)
        .style(|_t, s| iced::widget::button::Style {
            background: if matches!(s, iced::widget::button::Status::Hovered) {
                Some(Background::Color(Palette::hover()))
            } else {
                None
            },
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .padding([4, 5])
        .into()
}

fn req_item_style(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
    selected: bool,
) -> iced::widget::button::Style {
    // Selection background/border is owned by the wrapping row container now
    // (so it spans the edit/delete icons too, not just this name button).
    iced::widget::button::Style {
        background: if !selected && matches!(status, iced::widget::button::Status::Hovered) {
            Some(Background::Color(Palette::hover()))
        } else {
            None
        },
        text_color: Palette::text(),
        ..Default::default()
    }
}
