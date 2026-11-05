//! Mockup `.exp-modal`: Export / Import collections with a checklist.

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{
        Space, button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
    },
};

use crate::{
    app::AppState,
    message::{ImportMsg, Message},
    services::import::resources::{ScanKind, ScanStatus},
    state::collections_modal::{CollectionsModal, ExportFormat, ModalTab},
    ui::{
        icons,
        theme::{MONO, Palette, UI_FONT, UI_FONT_MEDIUM},
    },
};

const WIDTH: f32 = 540.0;
const LIST_MAX: f32 = 274.0;

pub fn view<'a>(state: &'a AppState, modal: &'a CollectionsModal) -> Element<'a, Message> {
    let (title, subtitle) = match modal.tab {
        ModalTab::Export => (
            "Export collections",
            "Choose the collections to export as Postman Collection v2.1.",
        ),
        ModalTab::Import => (
            "Import collections",
            "Choose a Postman export folder, then review the collections before importing.",
        ),
    };
    let head = row![
        column![
            text(title).size(15).font(semibold()).color(Palette::text()),
            text(subtitle).size(11).color(Palette::text_subtle()),
        ]
        .spacing(3)
        .width(Length::Fill),
        icon_btn(icons::close(), Message::Import(ImportMsg::ModalClosed)),
    ]
    .padding(iced::Padding {
        top: 16.0,
        right: 18.0,
        bottom: 10.0,
        left: 18.0,
    });
    let tabs = row![
        tab_btn("Export", modal.tab == ModalTab::Export, ModalTab::Export),
        tab_btn("Import", modal.tab == ModalTab::Import, ModalTab::Import),
    ]
    .spacing(2)
    .padding(iced::Padding {
        top: 0.0,
        right: 18.0,
        bottom: 8.0,
        left: 18.0,
    });

    let body = match modal.tab {
        ModalTab::Export => export_view(state, modal),
        ModalTab::Import => import_view(modal),
    };

    let card = container(column![head, tabs, body])
        .width(WIDTH)
        .clip(true)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface())),
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: iced::Vector::new(0.0, 18.0),
                blur_radius: 48.0,
            },
            ..Default::default()
        });

    iced::widget::opaque(
        iced::widget::mouse_area(
            container(iced::widget::opaque(card))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.45).into()),
                    ..Default::default()
                }),
        )
        .on_press(Message::Import(ImportMsg::ModalClosed)),
    )
    .into()
}

fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..UI_FONT
    }
}

fn pad_x<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(iced::Padding {
            top: 0.0,
            right: 18.0,
            bottom: 8.0,
            left: 18.0,
        })
        .width(Length::Fill)
        .into()
}

fn search(placeholder: &'static str, value: &str) -> Element<'static, Message> {
    pad_x(
        text_input(placeholder, value)
            .id(crate::state::collections_modal::SEARCH_ID)
            .on_input(|q| Message::Import(ImportMsg::ModalFilterChanged(q)))
            .icon(text_input::Icon {
                font: icons::ICON_FONT,
                code_point: '\u{e151}',
                size: Some(12.0.into()),
                spacing: 6.0,
                side: text_input::Side::Left,
            })
            .size(12)
            .padding([6, 8])
            .style(crate::ui::styles::boxed_input),
    )
}

/// `.exp-list` / `.imp-list`: hairlines above and below, scrolls past
/// `LIST_MAX`.
fn list<'a>(rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let hair = || {
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(|_| container::Style {
                background: Some(Palette::border_subtle().into()),
                ..Default::default()
            })
    };
    column![
        hair(),
        container(
            scrollable(column(rows).spacing(0).padding([4, 10]))
                .style(crate::ui::theme::thin_scrollbar)
        )
        .max_height(LIST_MAX),
        hair(),
    ]
    .into()
}

fn check<'a>(on: bool, msg: Option<Message>) -> Element<'a, Message> {
    let mut c = checkbox(on)
        .size(16)
        .spacing(0)
        .style(crate::ui::styles::checkbox);
    if let Some(msg) = msg {
        c = c.on_toggle(move |_| msg.clone());
    }
    c.into()
}

/// One `.exp-row` / `.imp-row`: the whole row toggles; bg-1 on hover.
fn row_button<'a>(
    content: impl Into<Element<'a, Message>>,
    msg: Option<Message>,
    dim: bool,
) -> Element<'a, Message> {
    let enabled = msg.is_some();
    button(content)
        .on_press_maybe(msg)
        .width(Length::Fill)
        .padding([7, 8])
        .style(move |_, status| button::Style {
            background: (enabled && matches!(status, button::Status::Hovered))
                .then(|| Background::Color(Palette::background())),
            text_color: if dim {
                Palette::text_subtle()
            } else {
                Palette::text()
            },
            border: Border {
                radius: 5.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn all_row<'a>(on: bool, msg: Message, count: String) -> Element<'a, Message> {
    row_button(
        row![
            check(on, Some(msg.clone())),
            text("Select all")
                .size(11)
                .font(UI_FONT_MEDIUM)
                .color(Palette::text_muted()),
            Space::new().width(Length::Fill),
            meta(count),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
        Some(msg),
        false,
    )
}

fn name_text<'a>(name: &str, dim: bool) -> iced::widget::Text<'a> {
    text(name.to_uppercase())
        .size(11)
        .font(UI_FONT_MEDIUM)
        .color(if dim {
            Palette::text_subtle()
        } else {
            Palette::text()
        })
        .wrapping(iced::widget::text::Wrapping::None)
}

fn meta<'a>(s: String) -> iced::widget::Text<'a> {
    text(s)
        .size(10)
        .font(MONO)
        .color(Palette::text_subtle())
        .wrapping(iced::widget::text::Wrapping::None)
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

// ── Export ────────────────────────────────────────────────────────────────

fn export_view<'a>(state: &'a AppState, modal: &'a CollectionsModal) -> Element<'a, Message> {
    let visible: Vec<_> = state
        .collections
        .iter()
        .filter(|c| modal.matches(&c.name, ""))
        .collect();
    let all_on = !visible.is_empty()
        && visible
            .iter()
            .all(|c| modal.export_selected.contains(&c.id));
    let mut rows = vec![all_row(
        all_on,
        Message::Import(ImportMsg::ExportToggledAll),
        plural(state.collections.len(), "collection"),
    )];
    let mut picked_requests = 0;
    for c in &state.collections {
        let (requests, folders) = collection_counts(state, &c.id);
        let on = modal.export_selected.contains(&c.id);
        if on {
            picked_requests += requests;
        }
        if !visible.iter().any(|v| v.id == c.id) {
            continue;
        }
        let msg = Message::Import(ImportMsg::ExportToggled(c.id.clone()));
        rows.push(row_button(
            row![
                check(on, Some(msg.clone())),
                container(name_text(&c.name, false))
                    .width(Length::Fill)
                    .clip(true),
                meta(format!(
                    "{} · {}",
                    plural(requests, "request"),
                    plural(folders, "folder")
                )),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            Some(msg),
            false,
        ));
    }

    let env = state.environments.iter().find(|e| e.is_active);
    let opt_row = |content: Element<'a, Message>| {
        container(content)
            .padding([0, 12])
            .height(38)
            .center_y(38)
            .width(Length::Fill)
    };
    let hair = || {
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(|_| container::Style {
                background: Some(Palette::border_subtle().into()),
                ..Default::default()
            })
    };
    let mut opts = column![opt_row(
        row![
            text("Format").size(12).color(Palette::text_muted()),
            Space::new().width(Length::Fill),
            pick_list(&ExportFormat::ALL[..], Some(modal.export_format), |f| {
                Message::Import(ImportMsg::ExportFormatChanged(f))
            })
            .text_size(11)
            .padding([5, 9])
            .style(crate::ui::styles::boxed_pick_list),
        ]
        .align_y(Alignment::Center)
        .into()
    )];
    if modal.export_format == ExportFormat::Postman {
        let label = match env {
            Some(e) => format!("Include environment ({})", e.name),
            None => "Include environment (no active environment)".to_owned(),
        };
        let msg = env.map(|_| Message::Import(ImportMsg::ExportIncludeEnvToggled));
        opts = opts.push(hair()).push(opt_row(
            row![
                check(modal.include_env && env.is_some(), msg),
                text(label).size(12).color(Palette::text_muted()),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into(),
        ));
    }
    opts = opts.push(hair()).push(
        container(
            text(
                "Environment values are written as they are; {{…}} placeholders stay. \
                 Each collection becomes its own file in the folder you choose.",
            )
            .size(11)
            .color(Palette::text_subtle()),
        )
        .padding(iced::Padding {
            top: 9.0,
            right: 12.0,
            bottom: 10.0,
            left: 12.0,
        })
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Palette::background().into()),
            ..Default::default()
        }),
    );
    let opts = container(opts)
        .clip(true)
        .width(Length::Fill)
        .style(|_| container::Style {
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    let n = modal
        .export_selected
        .iter()
        .filter(|id| state.collections.iter().any(|c| &c.id == *id))
        .count();
    let footer = footer(
        format!(
            "{} · {} selected",
            plural(n, "collection"),
            plural(picked_requests, "request")
        ),
        format!("Export {}", plural(n, "collection")),
        (n > 0).then_some(Message::Import(ImportMsg::ExportSubmit)),
    );
    column![
        search("Search collections", &modal.filter),
        list(rows),
        container(opts).padding(iced::Padding {
            top: 12.0,
            right: 18.0,
            bottom: 12.0,
            left: 18.0,
        }),
        footer,
    ]
    .into()
}

/// (requests, folders) under one collection.
fn collection_counts(state: &AppState, collection_id: &str) -> (usize, usize) {
    let requests = state.requests.get(collection_id).map_or(0, Vec::len);
    let folders = state
        .groups
        .iter()
        .filter(|g| {
            crate::app::tree::root_collection_of(&state.groups, &g.id).as_deref()
                == Some(collection_id)
        })
        .count();
    (requests, folders)
}

// ── Import ────────────────────────────────────────────────────────────────

fn import_view(modal: &CollectionsModal) -> Element<'_, Message> {
    let folder_label = modal
        .folder
        .clone()
        .unwrap_or_else(|| "No folder selected".to_owned());
    let picker = pad_x(
        container(
            row![
                icons::folder().size(12).color(Palette::text_subtle()),
                container(
                    text(folder_label)
                        .size(11)
                        .font(MONO)
                        .color(if modal.folder.is_some() {
                            Palette::text_muted()
                        } else {
                            Palette::text_subtle()
                        })
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(Length::Fill)
                .clip(true),
                small_btn(
                    "Files",
                    (!modal.scanning).then_some(Message::Import(ImportMsg::PickChooseFiles))
                ),
                small_btn(
                    if modal.folder.is_some() {
                        "Change folder"
                    } else {
                        "Choose folder"
                    },
                    (!modal.scanning).then_some(Message::Import(ImportMsg::PickChooseFolder)),
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([4, 4])
        .padding(iced::Padding {
            top: 4.0,
            right: 4.0,
            bottom: 4.0,
            left: 10.0,
        })
        .style(|_| container::Style {
            background: Some(Palette::background().into()),
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }),
    );

    let mut parts: Vec<Element<Message>> = vec![picker];
    if modal.entries.is_empty() {
        let note = if modal.scanning {
            "Reading files…"
        } else {
            "Choose the folder of a Postman data export (collection/, environment/ and \
             archive.json) or any folder of collection and environment files."
        };
        parts.push(pad_x(
            container(text(note).size(12).color(Palette::text_muted()))
                .padding([24, 12])
                .width(Length::Fill)
                .center_x(Length::Fill),
        ));
    } else {
        parts.push(search("Search collections or files", &modal.filter));
        let visible = modal.visible();
        let ready = modal.entries.iter().filter(|e| e.importable()).count();
        let all_on = visible
            .iter()
            .filter(|&&i| modal.entries[i].importable())
            .all(|&i| modal.selected[i]);
        let mut rows = vec![all_row(
            all_on,
            Message::Import(ImportMsg::PickToggledAll),
            format!("{} · {ready} ready", plural(modal.entries.len(), "file")),
        )];
        for i in visible {
            rows.push(import_row(modal, i));
        }
        parts.push(list(rows));
    }
    if let Some(error) = &modal.error {
        parts.push(
            container(text(error).size(11).color(Palette::ERROR))
                .padding([8, 18])
                .into(),
        );
    }

    let chosen = modal.chosen();
    let cols = chosen
        .iter()
        .filter(|e| e.kind == ScanKind::Collection)
        .count();
    let envs = chosen
        .iter()
        .filter(|e| e.kind == ScanKind::Environment)
        .count();
    let requests: usize = chosen.iter().map(|e| e.summary.requests).sum();
    let mut what = Vec::new();
    if cols > 0 {
        what.push(plural(cols, "collection"));
    }
    if envs > 0 {
        what.push(plural(envs, "environment"));
    }
    let label = if what.is_empty() {
        "Import".to_owned()
    } else {
        format!("Import {}", what.join(" + "))
    };
    parts.push(footer(
        format!(
            "{} ready · {}",
            plural(cols, "collection"),
            plural(requests, "request")
        ),
        label,
        (!chosen.is_empty() && !modal.scanning).then_some(Message::Import(ImportMsg::PickSubmit)),
    ));
    column(parts).into()
}

fn import_row(modal: &CollectionsModal, i: usize) -> Element<'_, Message> {
    let e = &modal.entries[i];
    let dim = !e.importable();
    let (chip, fg, bg) = match &e.status {
        ScanStatus::Ready => ("Ready", Palette::SUCCESS, Palette::success_soft()),
        ScanStatus::Review => ("Needs review", Palette::accent(), Palette::accent_soft()),
        ScanStatus::Unsupported(_) => ("Unsupported", Palette::ERROR, Palette::error_soft()),
        ScanStatus::Error(_) => ("Error", Palette::ERROR, Palette::error_soft()),
    };
    let why = match &e.status {
        ScanStatus::Unsupported(w) | ScanStatus::Error(w) => Some(w.clone()),
        ScanStatus::Review => Some(format!(
            "{} with Postman scripts (kept as comments)",
            plural(e.summary.scripts_to_review, "request")
        )),
        ScanStatus::Ready => None,
    };
    let mut main = column![name_text(&e.name, dim), meta(e.file.clone()),].spacing(2);
    if let Some(why) = why {
        main = main.push(
            text(why)
                .size(10)
                .font(MONO)
                .color(if dim {
                    Palette::ERROR
                } else {
                    Palette::text_subtle()
                })
                .wrapping(iced::widget::text::Wrapping::Word),
        );
    }
    let counts = match (e.kind, dim) {
        (_, true) if e.summary == Default::default() => "—".to_owned(),
        (ScanKind::Environment, _) => {
            format!("environment · {}", plural(e.summary.variables, "variable"))
        }
        (ScanKind::Collection, _) => format!(
            "{} · {}",
            plural(e.summary.folders, "folder"),
            plural(e.summary.requests, "request")
        ),
    };
    let msg = (!dim).then_some(Message::Import(ImportMsg::PickToggled(i)));
    row_button(
        row![
            check(modal.selected[i] && !dim, msg.clone()),
            container(main).width(Length::Fill).clip(true),
            meta(counts),
            container(text(chip).size(10).font(UI_FONT_MEDIUM).color(fg))
                .padding([2, 7])
                .style(move |_| container::Style {
                    background: Some(bg.into()),
                    border: Border {
                        radius: 24.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        msg,
        dim,
    )
}

// ── Shared bits ───────────────────────────────────────────────────────────

fn footer<'a>(count: String, action: String, msg: Option<Message>) -> Element<'a, Message> {
    let enabled = msg.is_some();
    column![
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(|_| container::Style {
                background: Some(Palette::border_subtle().into()),
                ..Default::default()
            }),
        row![
            text(count)
                .size(11)
                .color(Palette::text_subtle())
                .width(Length::Fill),
            button(text("Cancel").size(12).color(Palette::text_muted()))
                .on_press(Message::Import(ImportMsg::ModalClosed))
                .padding([7, 14])
                .style(|_, status| button::Style {
                    background: matches!(status, button::Status::Hovered)
                        .then(|| Background::Color(Palette::surface_high())),
                    text_color: Palette::text_muted(),
                    border: Border {
                        color: Palette::border(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }),
            button(text(action).size(12).font(UI_FONT_MEDIUM))
                .on_press_maybe(msg)
                .padding([7, 14])
                .style(move |_, status| button::Style {
                    background: Some(Background::Color(if !enabled {
                        Palette::surface_high()
                    } else if matches!(status, button::Status::Hovered) {
                        Color {
                            a: 0.88,
                            ..Palette::accent()
                        }
                    } else {
                        Palette::accent()
                    })),
                    text_color: if enabled {
                        Color::WHITE
                    } else {
                        Palette::text_subtle()
                    },
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 12.0,
            right: 18.0,
            bottom: 16.0,
            left: 18.0,
        }),
    ]
    .into()
}

fn tab_btn<'a>(label: &'static str, active: bool, tab: ModalTab) -> Element<'a, Message> {
    button(
        container(
            text(label)
                .size(11)
                .font(if active { UI_FONT_MEDIUM } else { UI_FONT }),
        )
        .height(24)
        .center_y(24),
    )
    .on_press(Message::Import(ImportMsg::ModalTabSelected(tab)))
    .padding([0, 8])
    .style(move |_, status| button::Style {
        background: (active || matches!(status, button::Status::Hovered))
            .then(|| Background::Color(Palette::surface_high())),
        text_color: if active {
            Palette::text()
        } else {
            Palette::text_subtle()
        },
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

fn small_btn<'a>(label: &'static str, msg: Option<Message>) -> Element<'a, Message> {
    let enabled = msg.is_some();
    button(container(text(label).size(11)).height(22).center_y(22))
        .on_press_maybe(msg)
        .padding([0, 8])
        .style(move |_, status| {
            let hot = enabled && matches!(status, button::Status::Hovered);
            button::Style {
                background: hot.then(|| Background::Color(Palette::surface_high())),
                text_color: if hot {
                    Palette::text()
                } else if enabled {
                    Palette::text_muted()
                } else {
                    Palette::text_subtle()
                },
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

fn icon_btn<'a>(icon: iced::widget::Text<'static>, msg: Message) -> Element<'a, Message> {
    crate::ui::widgets::mini::icon_button(icon, "Close", Some(msg), false)
}
