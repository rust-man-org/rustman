use iced::{
    Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, scrollable, text, text_editor, text_input},
};

use crate::{
    app::AppState,
    message::{GitMsg, Message, RequestMsg, SaveDialogMsg},
    ui::{
        icons,
        theme::{MONO, Palette, TEXT_LG},
    },
};

pub(super) fn collection_delete_dialog(state: &AppState) -> Element<'_, Message> {
    use crate::message::SidebarMsg;
    let id = state.sidebar.collection_delete_id.as_deref().unwrap_or("");
    let name = state
        .collections
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name.as_str())
        .unwrap_or("Collection");
    let count = state.requests.get(id).map_or(0, Vec::len);
    let allowed = state.sidebar.collection_delete_text == "delete";
    let input = text_input("Type delete", &state.sidebar.collection_delete_text)
        .id("collection-delete-confirm")
        .style(crate::ui::styles::field_input)
        .size(12)
        .padding([6, 8])
        .on_input(|text| Message::Sidebar(SidebarMsg::CollectionDeleteTextChanged(text)))
        .on_submit(Message::Sidebar(SidebarMsg::CollectionDeleteConfirmed));
    let mut content = column![
        row![
            text(format!(
                "Delete \"{}\"?",
                crate::services::format::head_capped(name, 180)
            ))
            .size(14)
            .font(crate::ui::theme::UI_FONT_MEDIUM)
            .color(Palette::text()),
            Space::new().width(Length::Fill),
            crate::ui::components::atoms::compact_icon_button(
                icons::close(),
                "Cancel deletion",
                Message::Sidebar(SidebarMsg::CollectionDeleteCancelled),
                22.0
            )
        ]
        .align_y(iced::Alignment::Center),
        text(format!(
            "{count} requests · folders and saved examples will also be deleted"
        ))
        .size(11)
        .color(Palette::text_muted()),
    ]
    .spacing(10);
    // Layer 1 (Postman-style): say what goes, then a plain Delete. Layer 2:
    // type "delete". The handler enforces both, not just this view.
    if !state.sidebar.collection_delete_typing {
        content = content.push(
            text(
                "Deleting this removes every request, folder and saved example in it \
                 from this workspace. Local drafts of its requests are kept as \
                 orphans. This can't be undone.",
            )
            .size(12)
            .color(Palette::text()),
        );
        content = content.push(
            row![
                Space::new().width(Length::Fill),
                crate::ui::components::atoms::Button::new("Cancel")
                    .on_press(Message::Sidebar(SidebarMsg::CollectionDeleteCancelled)),
                button(text("Delete").size(12))
                    .on_press(Message::Sidebar(SidebarMsg::CollectionDeleteProceed))
                    .style(danger_btn_style)
                    .padding([5, 12]),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        );
        return modal_overlay(
            container(content)
                .padding([16, 18])
                .width(460)
                .style(modal_card_style),
        );
    }
    content = content
        .push(
            text("To confirm, type \"delete\" in the box below")
                .size(12)
                .color(Palette::text()),
        )
        .push(input);
    if let Some(error) = &state.status_message {
        content = content.push(text(error).size(11).color(Palette::ERROR));
    }
    content = content.push(
        row![
            Space::new().width(Length::Fill),
            crate::ui::components::atoms::Button::new("Cancel")
                .on_press(Message::Sidebar(SidebarMsg::CollectionDeleteCancelled)),
            button(text("Delete Collection").size(12))
                .on_press_maybe(
                    allowed.then_some(Message::Sidebar(SidebarMsg::CollectionDeleteConfirmed))
                )
                .style(move |theme, status| {
                    if matches!(status, button::Status::Disabled) {
                        button::Style {
                            background: Some(Palette::error_soft().into()),
                            text_color: Palette::text_subtle(),
                            border: Border {
                                radius: 6.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    } else {
                        danger_btn_style(theme, status)
                    }
                })
                .padding([5, 12]),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    );
    modal_overlay(
        container(content)
            .padding([16, 18])
            .width(460)
            .style(modal_card_style),
    )
}

/// "Delete "<request>"?" — a single confirm, like Postman's request delete.
pub(super) fn request_delete_dialog(state: &AppState) -> Element<'_, Message> {
    use crate::message::SidebarMsg;
    let name = state
        .sidebar
        .request_delete
        .as_ref()
        .and_then(|(id, col)| state.requests.get(col)?.iter().find(|r| &r.id == id))
        .map(|r| r.name.as_str())
        .unwrap_or("request");
    let name = crate::services::format::head_capped(name, 180);
    let content = column![
        row![
            text(format!("Delete \"{name}\"?"))
                .size(14)
                .font(crate::ui::theme::UI_FONT_MEDIUM)
                .color(Palette::text()),
            Space::new().width(Length::Fill),
            crate::ui::components::atoms::compact_icon_button(
                icons::close(),
                "Cancel deletion",
                Message::Sidebar(SidebarMsg::RequestDeleteCancelled),
                22.0
            )
        ]
        .align_y(iced::Alignment::Center),
        row![
            text("Are you sure you want to delete ")
                .size(12)
                .color(Palette::text()),
            text(name.to_owned())
                .size(12)
                .font(crate::ui::theme::UI_FONT_MEDIUM)
                .color(Palette::text()),
            text("?").size(12).color(Palette::text()),
        ],
        row![
            Space::new().width(Length::Fill),
            crate::ui::components::atoms::Button::new("Cancel")
                .on_press(Message::Sidebar(SidebarMsg::RequestDeleteCancelled)),
            button(text("Delete").size(12))
                .on_press(Message::Sidebar(SidebarMsg::RequestDeleteConfirmed))
                .style(danger_btn_style)
                .padding([5, 12]),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(12);
    modal_overlay(
        container(content)
            .padding([16, 18])
            .width(440)
            .style(modal_card_style),
    )
}

pub(super) fn save_dialog(state: &AppState) -> Element<'_, Message> {
    let name_input = text_input("Request name", &state.save_dialog_name)
        .on_input(|s| Message::SaveDialog(SaveDialogMsg::NameChanged(s)))
        .size(13)
        .padding([8, 10])
        .style(crate::ui::styles::field_input);

    let mut col_col = column![text("Save to").size(11).color(Palette::text_muted())].spacing(4);

    for col in &state.collections {
        let selected = !state.save_dialog_new_col
            && state.save_dialog_group_id.is_none()
            && state.save_dialog_collection_id.as_deref() == Some(&col.id);
        let col_id = col.id.clone();
        col_col = col_col.push(collection_radio_btn(
            col.name.clone(),
            selected,
            move || Message::SaveDialog(SaveDialogMsg::ParentSelected(col_id.clone())),
        ));
        col_col = save_dialog_groups(state, col_col, &col.id, true, 1);
    }

    let new_col_selected = state.save_dialog_new_col;
    col_col = col_col.push(collection_radio_btn(
        "+ New collection".to_owned(),
        new_col_selected,
        || Message::SaveDialog(SaveDialogMsg::ToggleNewCollection),
    ));

    let new_col_input: Option<Element<'_, Message>> = if state.save_dialog_new_col {
        Some(
            text_input("Collection name…", &state.save_dialog_new_col_name)
                .on_input(|s| Message::SaveDialog(SaveDialogMsg::NewCollectionNameChanged(s)))
                .size(12)
                .padding([6, 10])
                .style(crate::ui::styles::field_input)
                .into(),
        )
    } else {
        None
    };

    let btns = row![
        button(text("Cancel").size(12).color(Palette::text_muted()))
            .on_press(Message::SaveDialog(SaveDialogMsg::Close))
            .style(iced::widget::button::text)
            .padding([6, 14]),
        Space::new().width(Length::Fill),
        button(text("Save").size(12).color(Color::WHITE))
            .on_press(Message::SaveDialog(SaveDialogMsg::Confirm))
            .style(accent_btn_style)
            .padding([6, 18]),
    ]
    .align_y(iced::Alignment::Center);

    let mut body_col = column![
        text("Save Request")
            .size(TEXT_LG)
            .color(Palette::text())
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().height(8),
        text("Name").size(11).color(Palette::text_muted()),
        name_input,
        Space::new().height(8),
        col_col,
    ]
    .spacing(4)
    .width(440);

    if let Some(inp) = new_col_input {
        body_col = body_col.push(inp);
    }
    body_col = body_col.push(Space::new().height(12)).push(btns);

    modal_overlay(
        container(body_col)
            .style(modal_card_style)
            .padding([20, 24])
            .width(440),
    )
}

pub(super) fn export_dialog(state: &AppState) -> Element<'_, Message> {
    use crate::message::ImportMsg;

    let col_id = state.export_dialog_collection.clone().unwrap_or_default();
    let col_name = state
        .collections
        .iter()
        .find(|c| c.id == col_id)
        .map(|c| c.name.clone())
        .unwrap_or_default();

    let postman_id = col_id.clone();
    let native_id = col_id;

    let body_col = column![
        text(format!("Export “{col_name}”"))
            .size(TEXT_LG)
            .color(Palette::text())
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().height(4),
        text("Choose a format:")
            .size(11)
            .color(Palette::text_muted()),
        Space::new().height(10),
        button(
            column![
                text("Postman v2.1 collection").size(12).color(Color::WHITE),
                text("For importing into Postman and compatible tools")
                    .size(10)
                    .color(Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.7
                    }),
            ]
            .spacing(2),
        )
        .on_press(Message::Import(ImportMsg::ExportCollection(postman_id)))
        .style(accent_btn_style)
        .padding([10, 14])
        .width(Length::Fill),
        Space::new().height(6),
        button(
            column![
                text("Rustman JSON").size(12).color(Palette::text()),
                text("Lossless native format — re-importable into Rustman")
                    .size(10)
                    .color(Palette::text_muted()),
            ]
            .spacing(2),
        )
        .on_press(Message::Import(ImportMsg::ExportCollectionJson(native_id)))
        .style(|_t, s| iced::widget::button::Style {
            background: Some(Background::Color(
                if matches!(s, iced::widget::button::Status::Hovered) {
                    Palette::surface_high()
                } else {
                    Palette::surface_raised()
                },
            )),
            text_color: Palette::text(),
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 6.0.into()
            },
            ..Default::default()
        })
        .padding([10, 14])
        .width(Length::Fill),
        Space::new().height(12),
        button(text("Cancel").size(12).color(Palette::text_muted()))
            .on_press(Message::Import(ImportMsg::CloseExportDialog))
            .style(iced::widget::button::text)
            .padding([6, 14]),
    ]
    .spacing(0)
    .width(400);

    modal_overlay(
        container(body_col)
            .style(modal_card_style)
            .padding([20, 24])
            .width(400),
    )
}

pub(super) fn close_confirm_dialog(_state: &AppState) -> Element<'_, Message> {
    let btns = row![
        button(text("Cancel").size(12).color(Palette::text_muted()))
            .on_press(Message::Request(RequestMsg::CancelCloseTab))
            .style(iced::widget::button::text)
            .padding([6, 14]),
        Space::new().width(Length::Fill),
        button(text("Discard").size(12).color(Color::WHITE))
            .on_press(Message::Request(RequestMsg::ConfirmCloseTab))
            .style(danger_btn_style)
            .padding([6, 18]),
    ]
    .align_y(iced::Alignment::Center);

    let body_col = column![
        text("Discard unsaved changes?")
            .size(TEXT_LG)
            .color(Palette::text())
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().height(8),
        text("This tab has unsaved edits that will be lost if you close it.")
            .size(12)
            .color(Palette::text_muted()),
        Space::new().height(16),
        btns,
    ]
    .spacing(4)
    .width(380);

    modal_overlay(
        container(body_col)
            .style(modal_card_style)
            .padding([20, 24])
            .width(380),
    )
}

pub(super) fn restore_confirm_dialog(state: &AppState) -> Element<'_, Message> {
    let id = state.git_restore_confirm.clone().unwrap_or_default();
    let short: String = id.chars().take(7).collect();

    let btns = row![
        button(text("Cancel").size(12).color(Palette::text_muted()))
            .on_press(Message::Git(GitMsg::CancelRestore))
            .style(iced::widget::button::text)
            .padding([6, 14]),
        Space::new().width(Length::Fill),
        button(text("Restore").size(12).color(Color::WHITE))
            .on_press(Message::Git(GitMsg::RestoreCommit(id)))
            .style(danger_btn_style)
            .padding([6, 18]),
    ]
    .align_y(iced::Alignment::Center);

    let body_col = column![
        row![
            text("Restore commit ")
                .size(TEXT_LG)
                .color(Palette::text())
                .font(crate::ui::theme::UI_FONT_MEDIUM),
            text(short)
                .size(TEXT_LG)
                .color(Palette::accent())
                .font(MONO),
            text("?")
                .size(TEXT_LG)
                .color(Palette::text())
                .font(crate::ui::theme::UI_FONT_MEDIUM),
        ],
        Space::new().height(8),
        text(
            "Your collections will be reset to this commit. Uncommitted local \
              changes — including edits open in tabs — will be overwritten, and \
              collections added after this commit will be removed."
        )
        .size(12)
        .color(Palette::text_muted()),
        Space::new().height(16),
        btns,
    ]
    .spacing(4)
    .width(420);

    modal_overlay(
        container(body_col)
            .style(modal_card_style)
            .padding([20, 24])
            .width(420),
    )
}

pub(super) fn curl_modal(state: &AppState) -> Element<'_, Message> {
    let title = row![
        text("cURL Command")
            .size(TEXT_LG)
            .color(Palette::text())
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().width(Length::Fill),
        button(icons::close().size(16).color(Palette::text_muted()))
            .on_press(Message::Request(RequestMsg::CloseCurlModal))
            .style(iced::widget::button::text)
            .padding([2, 6]),
    ]
    .align_y(iced::Alignment::Center);

    let cmd_box = {
        // The modal is a plain text view with no paging: a multi-megabyte
        // export (easy with a huge form value) would freeze shaping while
        // open. Show a capped head with a note — Copy still takes everything.
        let cmd = state.curl_modal_command.as_str();
        let head = crate::services::format::head_capped(cmd, crate::consts::EXPORT_PREVIEW_BYTES);
        let truncated = head.len() < cmd.len();
        let shown = if truncated {
            format!("{head}…")
        } else {
            head.to_owned()
        };
        let mut col: iced::widget::Column<'_, Message> = column![];
        if truncated {
            col = col.push(
                text(format!(
                    "Showing {} of {} — Copy takes the full command.",
                    crate::ui::request::cell::human_size(head.len()),
                    crate::ui::request::cell::human_size(cmd.len()),
                ))
                .size(10)
                .color(Palette::WARNING),
            );
            col = col.push(Space::new().height(6));
        }
        col = col.push(container(
            scrollable(
                container(text(shown).font(MONO).size(11).color(Color {
                    r: 0.7,
                    g: 0.9,
                    b: 0.7,
                    a: 1.0,
                }))
                .padding([10, 12])
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .height(200)
            .style(crate::ui::theme::thin_scrollbar),
        ));
        container(col)
    }
    .style(|_| iced::widget::container::Style {
        background: Some(Background::Color(Palette::background())),
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .width(Length::Fill);

    let inner = container(
        column![
            title,
            Space::new().height(10),
            text("Run this command in your terminal:")
                .size(11)
                .color(Palette::text_muted()),
            Space::new().height(6),
            cmd_box,
            Space::new().height(12),
            row![
                button(text("Close").size(12).color(Palette::text_muted()))
                    .on_press(Message::Request(RequestMsg::CloseCurlModal))
                    .style(iced::widget::button::text)
                    .padding([6, 14]),
                Space::new().width(Length::Fill),
                button(
                    row![
                        icons::copy().size(12).color(Color::WHITE),
                        text("Copy").size(12).color(Color::WHITE),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::Request(RequestMsg::CopyCurlToClipboard))
                .style(accent_btn_style)
                .padding([7, 16]),
            ]
            .align_y(iced::Alignment::Center),
        ]
        .spacing(0)
        .width(560),
    )
    .style(modal_card_style)
    .padding([20, 24])
    .width(560);

    modal_overlay(inner)
}

pub(super) fn import_dialog(state: &AppState) -> Element<'_, Message> {
    use crate::message::ImportMsg;
    let busy = state.importing;
    let action = |label: &'static str, msg| {
        button(text(label).size(12).color(Palette::accent()))
            .on_press_maybe((!busy).then_some(Message::Import(msg)))
            .style(iced::widget::button::text)
            .padding([5, 8])
    };
    let title = row![
        text("Use Resources")
            .size(TEXT_LG)
            .font(crate::ui::theme::UI_FONT_MEDIUM)
            .color(Palette::text()),
        Space::new().width(Length::Fill),
        crate::ui::components::atoms::compact_icon_button(
            icons::close(),
            "Close import",
            Message::Request(RequestMsg::CloseImportDialog),
            24.0
        ),
    ]
    .align_y(iced::Alignment::Center);
    let editor = text_editor(&state.import_editor)
        .id("resource-import")
        .placeholder("Paste a cURL command, OpenAPI spec, or collection JSON")
        .on_action(move |action| Message::Import(ImportMsg::Edited(action)))
        .font(MONO)
        .size(12)
        .padding(10)
        .height(140)
        .style(|theme, status| {
            let mut style = text_editor::default(theme, status);
            style.background = Palette::background().into();
            style.border = Border {
                color: if matches!(status, text_editor::Status::Focused { .. }) {
                    Palette::accent()
                } else {
                    Palette::border()
                },
                width: 1.0,
                radius: 5.0.into(),
            };
            style.value = Palette::text();
            style.placeholder = Palette::text_subtle();
            style
        });
    let drop_area = container(
        column![
            icons::import().size(22).color(Palette::text_muted()),
            text(if busy {
                "Importing resources..."
            } else {
                "Drop files or folders here to import"
            })
            .size(13)
            .font(crate::ui::theme::UI_FONT_MEDIUM)
            .color(Palette::text()),
            row![
                text("Or select").size(12).color(Palette::text_muted()),
                action("files", ImportMsg::ChooseFiles),
                text("or").size(12).color(Palette::text_muted()),
                action("folders", ImportMsg::ChooseFolder),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(2),
        ]
        .spacing(10)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(220)
    .center_x(Length::Fill)
    .center_y(220)
    .style(|_| container::Style {
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: 5.0.into(),
        },
        ..Default::default()
    });
    let mut body = column![
        title,
        row![
            text("Resources").size(12).color(Palette::text_muted()),
            container(text("Import").size(12).color(Palette::text()))
                .padding([4, 8])
                .style(|_| container::Style {
                    background: Some(Palette::surface_high().into()),
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
        editor,
        drop_area,
        text("Postman / OpenAPI / Rustman JSON / HTTPie / cURL")
            .size(11)
            .color(Palette::text_muted()),
    ]
    .spacing(12);
    if let Some(error) = &state.import_error {
        body = body.push(text(error).size(12).color(Palette::ERROR));
    }
    body = body.push(
        row![
            action("Paste from clipboard", ImportMsg::Clipboard),
            Space::new().width(Length::Fill),
            button(text("Cancel").size(12).color(Palette::text_muted()))
                .on_press(Message::Request(RequestMsg::CloseImportDialog))
                .style(iced::widget::button::text)
                .padding([7, 14]),
            button(text(if busy { "Importing..." } else { "Import" }).size(12))
                .on_press_maybe(
                    (!busy && !state.import_editor.text().trim().is_empty())
                        .then_some(Message::Import(ImportMsg::Submit))
                )
                .style(|theme, status| {
                    let mut style = accent_btn_style(theme, status);
                    style.shadow = iced::Shadow::default();
                    if matches!(status, button::Status::Disabled) {
                        style.background = Some(Palette::surface_high().into());
                        style.text_color = Palette::text_muted();
                    }
                    style
                })
                .padding([7, 18]),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    );
    let card = container(body)
        .padding(18)
        .width(Length::Fill)
        .max_width(848)
        .style(|theme| container::Style {
            shadow: iced::Shadow::default(),
            ..modal_card_style(theme)
        });
    iced::widget::opaque(
        container(
            scrollable(container(card).center_x(Length::Fill).padding(24))
                .height(Length::Fill)
                .style(crate::ui::theme::thin_scrollbar),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.60).into()),
            ..Default::default()
        }),
    )
    .into()
}

fn editor_box<'a>(label: &'static str, editor: Element<'a, Message>) -> Element<'a, Message> {
    column![
        text(label)
            .size(11)
            .color(Palette::text_muted())
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().height(4),
        container(editor)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Palette::background())),
                border: Border {
                    color: Palette::border(),
                    width: 1.0,
                    radius: 6.0.into()
                },
                ..Default::default()
            })
            .height(Length::FillPortion(1))
            .width(Length::Fill),
    ]
    .spacing(0)
    .height(Length::FillPortion(1))
    .into()
}

pub(super) fn global_scripts_modal(state: &AppState) -> Element<'_, Message> {
    use crate::message::SettingsMsg;

    let title = row![
        text("Global Scripts")
            .size(TEXT_LG)
            .color(Palette::text())
            .font(crate::ui::theme::UI_FONT_MEDIUM),
        Space::new().width(Length::Fill),
        button(icons::close().size(16).color(Palette::text_muted()))
            .on_press(Message::Settings(SettingsMsg::CloseGlobalScriptsModal))
            .style(iced::widget::button::text)
            .padding([2, 6]),
    ]
    .align_y(iced::Alignment::Center);

    let pre_request: Element<Message> = text_editor(&state.global_pre_request_editor)
        .on_action(|a| Message::Settings(SettingsMsg::GlobalPreRequestScriptEdited(a)))
        .font(MONO)
        .size(12)
        .into();
    let test: Element<Message> = text_editor(&state.global_test_editor)
        .on_action(|a| Message::Settings(SettingsMsg::GlobalTestScriptEdited(a)))
        .font(MONO)
        .size(12)
        .into();

    let inner = container(
        column![
            title,
            Space::new().height(6),
            text(
                "Runs before every request's own pre-request/test script — for setup you'd \
                  otherwise copy-paste into each request."
            )
            .size(11)
            .color(Palette::text_subtle()),
            Space::new().height(14),
            editor_box("Pre-request", pre_request),
            Space::new().height(12),
            editor_box("Test", test),
            Space::new().height(14),
            row![
                Space::new().width(Length::Fill),
                button(text("Done").size(12).color(Color::WHITE))
                    .on_press(Message::Settings(SettingsMsg::CloseGlobalScriptsModal))
                    .style(accent_btn_style)
                    .padding([7, 18]),
            ],
        ]
        .spacing(0)
        .width(720)
        .height(600),
    )
    .style(modal_card_style)
    .padding([20, 24])
    .width(720)
    .height(640);

    modal_overlay(inner)
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Folder rows under `parent` for the save dialog's parent picker, indented
/// by `depth` and sorted like the sidebar tree (issue #53).
fn save_dialog_groups<'a>(
    state: &'a AppState,
    mut col: iced::widget::Column<'a, Message>,
    parent_id: &str,
    parent_is_collection: bool,
    depth: usize,
) -> iced::widget::Column<'a, Message> {
    let mut groups: Vec<&'a crate::domain::group::Group> = state
        .groups
        .iter()
        .filter(|g| g.parent_id == parent_id && g.parent_is_collection == parent_is_collection)
        .collect();
    groups.sort_by_key(|g| g.sort_order);
    for group in groups {
        let selected =
            !state.save_dialog_new_col && state.save_dialog_group_id.as_deref() == Some(&group.id);
        let gid = group.id.clone();
        let label = format!("└ {}", group.name);
        let row: Element<'static, Message> =
            container(collection_radio_btn(label, selected, move || {
                Message::SaveDialog(SaveDialogMsg::ParentSelected(gid.clone()))
            }))
            .padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: (depth as f32) * 16.0,
            })
            .into();
        col = col.push(row);
        col = save_dialog_groups(state, col, &group.id, false, depth + 1);
    }
    col
}

fn collection_radio_btn(
    label: String,
    selected: bool,
    on_press: impl Fn() -> Message,
) -> Element<'static, Message> {
    button(
        row![
            container(Space::new().width(8).height(8))
                .style(move |_| iced::widget::container::Style {
                    background: Some(Background::Color(if selected {
                        Palette::accent()
                    } else {
                        Palette::border()
                    })),
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .width(8)
                .height(8),
            text(label).size(12).color(if selected {
                Palette::accent()
            } else {
                Palette::text_muted()
            }),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .on_press(on_press())
    .style(move |_t, s| iced::widget::button::Style {
        background: if selected || matches!(s, iced::widget::button::Status::Hovered) {
            Some(Background::Color(Palette::surface_high()))
        } else {
            None
        },
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        text_color: Palette::text(),
        ..Default::default()
    })
    .padding([5, 8])
    .width(Length::Fill)
    .into()
}

fn modal_overlay<'a>(inner: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(column![
        Space::new()
            .width(Length::Fill)
            .height(Length::FillPortion(2)),
        row![
            Space::new().width(Length::Fill),
            inner.into(),
            Space::new().width(Length::Fill),
        ],
        Space::new()
            .width(Length::Fill)
            .height(Length::FillPortion(3)),
    ])
    .style(|_| iced::widget::container::Style {
        background: Some(Background::Color(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.70,
        })),
        ..Default::default()
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn modal_card_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::surface())),
        border: Border {
            color: Palette::border(),
            width: 1.0,
            radius: 14.0.into(),
        },
        shadow: iced::Shadow {
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.7,
            },
            offset: iced::Vector::new(0.0, 12.0),
            blur_radius: 48.0,
        },
        ..Default::default()
    }
}

fn danger_btn_style(
    _t: &iced::Theme,
    s: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let base = Palette::ERROR;
    let bg = if matches!(s, iced::widget::button::Status::Hovered) {
        Color {
            r: (base.r + 0.06).min(1.0),
            g: (base.g + 0.06).min(1.0),
            b: (base.b + 0.06).min(1.0),
            a: 1.0,
        }
    } else {
        base
    };
    iced::widget::button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color { a: 0.30, ..base },
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 10.0,
        },
        ..Default::default()
    }
}

fn accent_btn_style(
    _t: &iced::Theme,
    s: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let accent = Palette::accent();
    let bg = if matches!(s, iced::widget::button::Status::Hovered) {
        Palette::accent_hover()
    } else {
        accent
    };
    iced::widget::button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color { a: 0.30, ..accent },
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 10.0,
        },
        ..Default::default()
    }
}
