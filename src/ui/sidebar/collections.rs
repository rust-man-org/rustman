use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space},
    Background, Border, Color, Element, Length,
};

use crate::{
    app::AppState,
    message::{ImportMsg, Message, SidebarMsg},
    ui::{icons, theme::{Palette, TEXT_LG}, widgets::kv_table},
};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let action_bar = container(
        row![
            text("Collections").size(TEXT_LG).color(Palette::text()).font(crate::ui::theme::UI_FONT_MEDIUM),
            Space::new().width(Length::Fill),
            icon_btn_only(icons::import(), "Import Postman / OpenAPI / HTTPie / JSON", Message::Import(ImportMsg::OpenPostmanDialog)),
            icon_btn_only(icons::plus(), "New Collection", Message::Sidebar(SidebarMsg::NewCollection)),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(2)
        .padding([10, 10]),
    )
    .width(Length::Fill);

    let mut col = column![action_bar, iced::widget::rule::horizontal(1.0).style(crate::ui::styles::divider)].spacing(0);

    for collection in &state.collections {
        let is_expanded = state.sidebar.expanded.contains(&collection.id);
        let arrow = if is_expanded { icons::chevron_down() } else { icons::chevron_right() };

        let col_id = collection.id.clone();
        let col_id_del = collection.id.clone();
        let col_id_add = collection.id.clone();
        let col_id_ren = collection.id.clone();
        let renaming = state.sidebar.col_renaming.as_deref() == Some(collection.id.as_str());

        let name_part: Element<Message> = if renaming {
            let rid = collection.id.clone();
            let rid_done = collection.id.clone();
            text_input("Collection name", &collection.name)
                .on_input(move |s| {
                    Message::Sidebar(SidebarMsg::RenameCollection { id: rid.clone(), name: s })
                })
                .on_submit(Message::Sidebar(SidebarMsg::ToggleRenameCollection(rid_done)))
                .size(12)
                .padding([4, 8])
                .width(Length::Fill)
                .into()
        } else {
            button(
                row![
                    arrow.size(10).color(Palette::text_muted()),
                    text(&collection.name).size(12).color(Palette::text()),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::Sidebar(SidebarMsg::CollectionToggled(col_id)))
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
            .padding([5, 8])
            .into()
        };

        let header = row![
            name_part,
            hover_icon_btn(icons::plus().size(10), Message::Sidebar(SidebarMsg::NewRequestIn(col_id_add))),
            hover_icon_btn(
                icons::edit().size(11),
                Message::Sidebar(SidebarMsg::ToggleRenameCollection(col_id_ren)),
            ),
            button(icons::export().size(11))
                .on_press(Message::Import(ImportMsg::OpenExportDialog(col_id_del.clone())))
                .style(|_t, s| iced::widget::button::Style {
                    text_color: if matches!(s, iced::widget::button::Status::Hovered) {
                        Palette::accent()
                    } else {
                        Color::TRANSPARENT
                    },
                    ..Default::default()
                })
                .padding([3, 4]),
            button(icons::close().size(12))
                .on_press(Message::Sidebar(SidebarMsg::DeleteCollection(col_id_del)))
                .style(|_t, s| iced::widget::button::Style {
                    background: if matches!(s, iced::widget::button::Status::Hovered) {
                Some(Background::Color(Palette::error_soft()))
                    } else {
                        None
                    },
                    text_color: if matches!(s, iced::widget::button::Status::Hovered) {
                        Palette::ERROR
                    } else {
                        Color::TRANSPARENT
                    },
                    border: Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .padding([3, 6]),
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        col = col.push(header);

        if is_expanded {
            let requests = state
                .requests
                .get(&collection.id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            if requests.is_empty() {
                col = col.push(
                    container(
                        text("No requests.").size(11).color(Palette::text_subtle()),
                    )
                    .padding(iced::Padding { top: 2.0, right: 8.0, bottom: 2.0, left: 24.0 }),
                );
            }

            for req in requests {
                let is_selected =
                    state.sidebar.selected_request.as_deref() == Some(req.id.as_str());
                let method_color = method_color(req.method.as_str());
                let req_id = req.id.clone();
                let col_id = req.collection_id.clone();

                let req_renaming = state.sidebar.req_renaming.as_deref() == Some(req.id.as_str());
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
                        .padding(iced::Padding { top: 4.0, right: 4.0, bottom: 4.0, left: 24.0 })
                        .width(Length::Fill)
                        .into()
                } else {
                    button(
                        row![
                            container(
                                text(req.method.as_str())
                                    .size(9)
                                    .color(method_color)
                                    .font(crate::ui::theme::MONO),
                            )
                            .width(32),
                            text(&req.name).size(12).color(Palette::text()),
                        ]
                        .spacing(4)
                        .align_y(iced::Alignment::Center),
                    )
                    .on_press(Message::Sidebar(SidebarMsg::RequestOpened(req.clone())))
                    .style(move |t, s| req_item_style(t, s, is_selected))
                    .width(Length::Fill)
                    .padding(iced::Padding { top: 5.0, right: 4.0, bottom: 5.0, left: 10.0 })
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
                        hover_icon_btn(
                            icons::edit().size(10),
                            Message::Sidebar(SidebarMsg::ToggleRenameRequest(req.id.clone())),
                        ),
                        button(icons::close().size(11))
                            .on_press(Message::Sidebar(SidebarMsg::DeleteRequest {
                                id: req_id,
                                collection_id: col_id,
                            }))
                            .style(|_t, s| iced::widget::button::Style {
                                background: if matches!(s, iced::widget::button::Status::Hovered) {
                                    Some(Background::Color(Palette::error_soft()))
                                } else {
                                    None
                                },
                                text_color: if matches!(s, iced::widget::button::Status::Hovered) {
                                    Palette::ERROR
                                } else {
                                    Color::TRANSPARENT
                                },
                                border: Border { radius: 3.0.into(), ..Default::default() },
                                ..Default::default()
                            })
                            .padding([2, 5]),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::Fill),
                )
                .style(move |_| iced::widget::container::Style {
                    background: if is_selected { Some(Background::Color(Palette::accent_soft())) } else { None },
                    border: Border {
                        color: if is_selected { Color { a: 0.35, ..Palette::accent() } } else { Color::TRANSPARENT },
                        width: if is_selected { 1.0 } else { 0.0 },
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                })
                .padding(iced::Padding { top: 1.0, right: 6.0, bottom: 1.0, left: 16.0 })
                .width(Length::Fill);

                col = col.push(item_row);
            }
        }
    }

    if state.collections.is_empty() {
        return col
            .push(kv_table::empty_state("No collections yet.\nClick + New to create one."))
            .height(Length::Fill)
            .into();
    }

    scrollable(col)
        .height(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            crate::ui::theme::grabbable_scrollbar(),
        ))
        .style(crate::ui::theme::thin_scrollbar)
        .into()
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

fn hover_icon_btn(icon: iced::widget::Text<'static>, msg: Message) -> iced::Element<'static, Message> {
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

fn icon_btn_only(icon: iced::widget::Text<'static>, _tooltip: &str, msg: Message) -> iced::Element<'static, Message> {
    button(icon.size(12).color(Palette::text_muted()))
        .on_press(msg)
        .style(|_t, s| iced::widget::button::Style {
            background: if matches!(s, iced::widget::button::Status::Hovered) {
                Some(Background::Color(Palette::hover()))
            } else {
                None
            },
            border: Border { radius: 4.0.into(), ..Default::default() },
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
