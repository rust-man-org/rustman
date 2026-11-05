use iced::{
    Background, Border, Element, Length,
    widget::{Space, button, column, container, pick_list, row, scrollable, text},
};

use crate::{
    domain::request::{BodyType, FormField, FormFieldType},
    message::{Message, RequestMsg},
    state::tabs::RequestTabState,
    ui::{icons, theme::Palette},
};

/// `open_menu`: the app's open context-menu key (form rows' Text/File menu).
pub fn view<'a>(tab: &'a RequestTabState, open_menu: Option<&str>) -> Element<'a, Message> {
    let type_tabs_row = body_toolbar(tab);

    let body_panel: Element<Message> = match tab.body_type {
        BodyType::None => container(
            column![
                Space::new().height(20),
                text("No body").size(13).color(Palette::text_muted()),
                text("This request does not have a body")
                    .size(11)
                    .color(Palette::text_subtle()),
            ]
            .spacing(4)
            .align_x(iced::Alignment::Center),
        )
        .center_x(Length::Fill)
        .padding([24, 8])
        .into(),

        BodyType::Json | BodyType::Text => tab
            .body_editor
            .view()
            .map(|m| Message::Request(RequestMsg::BodyEdited(m))),

        BodyType::FormData => form_view(tab, open_menu, true),
        BodyType::UrlEncoded => form_view(tab, open_menu, false),

        BodyType::GraphQL => {
            // Postman's layout: query on top, variables underneath. Both panes
            // are code editors, so both keep their own undo history and the
            // shared gutter/theme look.
            let query: Element<Message> = tab
                .body_editor
                .view()
                .map(|m| Message::Request(RequestMsg::BodyEdited(m)));
            let variables: Element<Message> = tab
                .graphql_variables_editor
                .view()
                .map(|m| Message::Request(RequestMsg::GraphQLVariablesEdited(m)));

            column![
                pane_label("Query"),
                container(query)
                    .height(Length::FillPortion(3))
                    .width(Length::Fill),
                pane_label("Variables"),
                container(variables)
                    .height(Length::FillPortion(2))
                    .width(Length::Fill),
            ]
            .spacing(0)
            .height(Length::Fill)
            .into()
        }
    };

    column![type_tabs_row, body_panel]
        .spacing(0)
        .height(Length::Fill)
        .into()
}

/// Expand + copy buttons for a form row, shown only when the value is in
/// preview mode (too big/multiline for an inline input). Small values stay
/// directly editable/selectable, so they need neither.
fn preview_actions<'a>(
    tab: &'a RequestTabState,
    field: &'a FormField,
    i: usize,
) -> Element<'a, Message> {
    let is_preview = field.field_type == FormFieldType::Text
        && !crate::ui::request::cell::is_inline(&field.value);
    if !is_preview {
        return Space::new().width(0).into();
    }
    let expanded = tab.form_viewers.contains_key(&field.id);
    row![
        button(
            (if expanded {
                icons::chevron_down()
            } else {
                icons::chevron_right()
            })
            .size(10)
            .color(Palette::text_muted())
        )
        .on_press(Message::Request(RequestMsg::FormValueExpandToggled(i)))
        .style(iced::widget::button::text)
        .padding([3, 4]),
        button(icons::copy().size(10).color(Palette::text_muted()))
            .on_press(Message::Request(RequestMsg::FormValueCopy(i)))
            .style(iced::widget::button::text)
            .padding([3, 4]),
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Height of an expanded form value, in pixels. The page editor scrolls
/// internally, so the row never grows with the value.
const FORM_VIEWER_HEIGHT: f32 = 240.0;

/// Paged full-value editor under an expanded preview row. Pages reuse the
/// response viewer's windowing (`page_starts`), and the page itself renders in
/// the shared code-editor UI — selectable, searchable, scrollable and
/// editable — so working with everything costs one bounded page regardless of
/// value size. Edits write back into the field value at once.
fn form_value_viewer<'a>(
    tab: &'a RequestTabState,
    field: &'a FormField,
) -> Option<Element<'a, Message>> {
    let viewer = tab.form_viewers.get(&field.id)?;
    let count = viewer.page_count();
    let page = viewer.page().min(count - 1);
    let (start, end) = viewer.page_span(&field.value);
    let index = field_index(tab, field);
    let editor: Element<'a, Message> = viewer.editor().view().map(move |editor_message| {
        Message::Request(RequestMsg::FormValueEdited(index, editor_message))
    });

    let prev = button(icons::arrow_left().size(11).color(Palette::text_muted()))
        .padding([4, 6])
        .style(iced::widget::button::text);
    let prev = if page > 0 {
        prev.on_press(Message::Request(RequestMsg::FormValuePagePrev(index)))
    } else {
        prev
    };
    let next = button(icons::arrow_right().size(11).color(Palette::text_muted()))
        .padding([4, 6])
        .style(iced::widget::button::text);
    let next = if page + 1 < count {
        next.on_press(Message::Request(RequestMsg::FormValuePageNext(index)))
    } else {
        next
    };
    Some(
        container(
            column![
                row![
                    prev,
                    text(format!("Page {}/{}", page + 1, count))
                        .size(10)
                        .color(Palette::text_muted()),
                    next,
                    Space::new().width(Length::Fill),
                    text(format!(
                        "{} of {}",
                        super::cell::human_size(end - start),
                        super::cell::human_size(field.value.len())
                    ))
                    .size(10)
                    .color(Palette::text_subtle()),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
                container(editor)
                    .height(Length::Fixed(FORM_VIEWER_HEIGHT))
                    .width(Length::Fill),
            ]
            .spacing(4),
        )
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Palette::surface_high())),
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .padding([6, 8])
        .width(Length::Fill)
        .into(),
    )
}

/// Row index of `field` for page messages (ids are stable, indices are not —
/// resolve at render time).
fn field_index(tab: &RequestTabState, field: &FormField) -> usize {
    tab.form_fields
        .iter()
        .position(|f| f.id == field.id)
        .unwrap_or(usize::MAX)
}

/// Body mode, Postman's radio row (none / form-data / x-www-form-urlencoded
/// / raw / GraphQL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    None,
    Form,
    UrlEncoded,
    Raw,
    GraphQl,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::None => "none",
            Mode::Form => "form-data",
            Mode::UrlEncoded => "x-www-form-urlencoded",
            Mode::Raw => "raw",
            Mode::GraphQl => "GraphQL",
        }
    }
}

const MODES: &[Mode] = &[
    Mode::None,
    Mode::Form,
    Mode::UrlEncoded,
    Mode::Raw,
    Mode::GraphQl,
];

fn mode_of(body: &BodyType) -> Mode {
    match body {
        BodyType::None => Mode::None,
        BodyType::FormData => Mode::Form,
        BodyType::UrlEncoded => Mode::UrlEncoded,
        BodyType::Json | BodyType::Text => Mode::Raw,
        BodyType::GraphQL => Mode::GraphQl,
    }
}

/// The body type a mode switch lands on; raw keeps JSON/Text if already raw.
fn body_for(mode: Mode, current: &BodyType) -> BodyType {
    match mode {
        Mode::None => BodyType::None,
        Mode::Form => BodyType::FormData,
        Mode::UrlEncoded => BodyType::UrlEncoded,
        Mode::GraphQl => BodyType::GraphQL,
        Mode::Raw if matches!(current, BodyType::Text) => BodyType::Text,
        Mode::Raw => BodyType::Json,
    }
}

/// Postman body bar: mode radios; raw adds the JSON/Text picker next to
/// it and Indent/Beautify on the right.
fn body_toolbar(tab: &RequestTabState) -> Element<'_, Message> {
    let current = tab.body_type.clone();
    let mode = mode_of(&current);
    let mut left = row![].spacing(16).align_y(iced::Alignment::Center);
    for &m in MODES {
        let target = body_for(m, &current).as_str().to_owned();
        left = left.push(
            iced::widget::radio(m.label(), m, Some(mode), move |_| {
                Message::Request(RequestMsg::BodyTypeChanged(target.clone()))
            })
            .size(13)
            .spacing(6)
            .text_size(12)
            .style(crate::ui::styles::radio),
        );
    }

    let raw = mode == Mode::Raw;
    let is_json = matches!(tab.body_type, BodyType::Json);
    if raw {
        left = left.push(
            pick_list(
                &["JSON", "Text"][..],
                Some(if is_json { "JSON" } else { "Text" }),
                |s| {
                    Message::Request(RequestMsg::BodyTypeChanged(
                        if s == "JSON" {
                            BodyType::Json
                        } else {
                            BodyType::Text
                        }
                        .as_str()
                        .to_owned(),
                    ))
                },
            )
            .text_size(12)
            .font(crate::ui::theme::UI_FONT_MEDIUM)
            .padding([4, 10])
            .style(crate::ui::styles::boxed_pick_list),
        );
    }

    let mut bar = row![left, Space::new().width(Length::Fill)]
        .spacing(8)
        .align_y(iced::Alignment::Center);
    if raw {
        let indent = if tab.body_indent_tabs {
            "Indent: Tabs"
        } else {
            "Indent: Spaces"
        };
        bar = bar.push(crate::ui::widgets::mini::text_button(
            None,
            indent,
            Some(Message::Request(RequestMsg::ToggleBodyIndentStyle)),
        ));
        if is_json {
            bar = bar.push(crate::ui::widgets::mini::text_button(
                Some(icons::braces()),
                "Beautify",
                Some(Message::Request(RequestMsg::FormatBody)),
            ));
        }
    }
    container(bar)
        .height(44)
        .center_y(44)
        .padding([0, 16])
        .width(Length::Fill)
        .into()
}

/// Section header above a split pane (GraphQL's Query / Variables).
fn pane_label(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(10).color(Palette::text_muted()))
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Palette::background())),
            ..Default::default()
        })
        .padding(iced::Padding {
            top: 3.0,
            right: 8.0,
            bottom: 3.0,
            left: 8.0,
        })
        .width(Length::Fill)
        .into()
}

/// Context-menu key of row `i`'s Text/File menu.
pub fn form_type_menu_key(i: usize) -> String {
    format!("request:form-type:{i}")
}

/// Postman's form table (form-data and x-www-form-urlencoded): check, key,
/// value, description, delete; a placeholder row adds a field. form-data
/// keys carry a `Text ⌄` / `File ⌄` menu at their right edge.
fn form_view<'a>(
    tab: &'a RequestTabState,
    open_menu: Option<&str>,
    files: bool,
) -> Element<'a, Message> {
    let mut rows: Vec<Element<Message>> = vec![crate::ui::widgets::kv_table::head3(
        Space::new().width(0).into(),
    )];
    for (i, field) in tab.form_fields.iter().enumerate() {
        let is_file = files && field.field_type == FormFieldType::File;

        let key_input = crate::ui::request::cell::text_cell(
            "Key",
            &field.key,
            12.0,
            [5, 7],
            Length::Fill,
            move |s| Message::Request(RequestMsg::FormFieldKeyChanged(i, s)),
        );
        let key_cell: Element<Message> = if files {
            let menu_key = form_type_menu_key(i);
            let open = open_menu == Some(menu_key.as_str());
            let chip = button(
                row![
                    text(if is_file { "File" } else { "Text" })
                        .size(11)
                        .color(Palette::text_subtle()),
                    icons::chevron_down().size(10).color(Palette::text_subtle()),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::Sidebar(
                crate::message::SidebarMsg::ContextMenuOpened(menu_key),
            ))
            .padding([3, 6])
            .style(move |_, s| iced::widget::button::Style {
                background: (open || matches!(s, iced::widget::button::Status::Hovered))
                    .then(|| Background::Color(Palette::surface_high())),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
            let item = |label: &'static str, kind: FormFieldType| {
                crate::ui::components::molecules::menu::menu_item(
                    None,
                    label,
                    Some(Message::Request(RequestMsg::FormFieldTypeSet(i, kind))),
                    false,
                )
            };
            let chip = crate::ui::widgets::mini::anchored_menu(
                chip.into(),
                open,
                vec![
                    item("Text", FormFieldType::Text),
                    item("File", FormFieldType::File),
                ],
                72.0,
            );
            row![key_input, chip]
                .spacing(4)
                .align_y(iced::Alignment::Center)
                .into()
        } else {
            key_input
        };

        let value_widget: Element<Message> = if is_file {
            let picked = field.file_name.is_some();
            let label = field.file_name.as_deref().unwrap_or("Select files");
            button(
                row![
                    icons::file_up().size(12).color(Palette::text_muted()),
                    text(label).size(12).color(if picked {
                        Palette::text()
                    } else {
                        Palette::text_muted()
                    }),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::Request(RequestMsg::FormFieldPickFile(i)))
            .style(|_, s| iced::widget::button::Style {
                background: Some(Background::Color(
                    if matches!(s, iced::widget::button::Status::Hovered) {
                        Palette::surface_raised()
                    } else {
                        Palette::surface_high()
                    },
                )),
                border: Border {
                    color: Palette::border_subtle(),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .padding([4, 10])
            .into()
        } else {
            crate::ui::request::cell::text_cell(
                "Value",
                &field.value,
                12.0,
                [5, 7],
                Length::Fill,
                move |s| Message::Request(RequestMsg::FormFieldValueChanged(i, s)),
            )
        };

        let cells = crate::ui::widgets::kv_table::kv_row3(
            iced::widget::checkbox(field.enabled)
                .on_toggle(move |_| Message::Request(RequestMsg::FormFieldToggled(i)))
                .size(13)
                .spacing(0)
                .style(crate::ui::styles::checkbox)
                .into(),
            key_cell,
            row![value_widget, preview_actions(tab, field, i)]
                .spacing(4)
                .align_y(iced::Alignment::Center)
                .into(),
            crate::ui::request::cell::text_cell(
                "Description",
                &field.description,
                12.0,
                [5, 7],
                Length::Fill,
                move |s| Message::Request(RequestMsg::FormFieldDescriptionChanged(i, s)),
            ),
            crate::ui::widgets::kv_table::remove_button(Message::Request(
                RequestMsg::FormFieldRemoved(i),
            )),
        );
        match form_value_viewer(tab, field) {
            Some(expanded) => rows.push(
                column![cells, container(expanded).padding([0, 12])]
                    .spacing(4)
                    .into(),
            ),
            None => rows.push(cells),
        }
    }
    rows.push(crate::ui::widgets::kv_table::add_row3(Message::Request(
        RequestMsg::FormFieldAdded,
    )));
    scrollable(column![
        container(crate::ui::widgets::kv_table::frame(rows)).padding(iced::Padding {
            top: 0.0,
            right: 16.0,
            bottom: 14.0,
            left: 16.0,
        })
    ])
    .height(Length::Fill)
    .direction(iced::widget::scrollable::Direction::Vertical(
        crate::ui::theme::grabbable_scrollbar(),
    ))
    .style(crate::ui::theme::thin_scrollbar)
    .into()
}
