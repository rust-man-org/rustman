use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space},
    Background, Border, Color, Element, Length,
};

use crate::{
    domain::request::{BodyType, FormField, FormFieldType},
    message::{Message, RequestMsg},
    state::tabs::RequestTabState,
    ui::{icons, theme::Palette},
};

pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    let type_tabs = container(
        row![
            body_type_btn("None", BodyType::None, &tab.body_type),
            body_type_btn("JSON", BodyType::Json, &tab.body_type),
            body_type_btn("Text", BodyType::Text, &tab.body_type),
            body_type_btn("Form", BodyType::FormData, &tab.body_type),
            body_type_btn("GraphQL", BodyType::GraphQL, &tab.body_type),
        ]
        .spacing(2)
        .padding(3),
    )
    .style(|_| iced::widget::container::Style {
        background: Some(Background::Color(Palette::background())),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 9.0.into(),
        },
        ..Default::default()
    });

    let type_tabs_row = container(type_tabs).padding(iced::Padding { top: 4.0, right: 12.0, bottom: 6.0, left: 12.0 });

    let body_panel: Element<Message> = match tab.body_type {
        BodyType::None => container(
            column![
                Space::new().height(20),
                text("No body").size(13).color(Palette::text_muted()),
                text("Select JSON, Text, Form, or GraphQL to add a body")
                    .size(11)
                    .color(Palette::text_subtle()),
            ]
            .spacing(4)
            .align_x(iced::Alignment::Center),
        )
        .center_x(Length::Fill)
        .padding([24, 8])
        .into(),

        BodyType::Json | BodyType::Text => {
            let is_json = matches!(tab.body_type, BodyType::Json);
            let indent_label = if tab.body_indent_tabs { "Tabs" } else { "Spaces" };
            let toolbar = container(
                row![
                    text(if is_json { "JSON" } else { "Text" })
                        .size(10)
                        .color(if is_json { Palette::accent() } else { Palette::text_muted() }),
                    Space::new().width(Length::Fill),
                    button(
                        text(format!("Indent: {indent_label}"))
                            .size(10)
                            .color(Palette::text_subtle()),
                    )
                    .on_press(Message::Request(RequestMsg::ToggleBodyIndentStyle))
                    .style(iced::widget::button::text)
                    .padding([2, 6]),
                    if is_json {
                        button(
                            row![
                                icons::braces().size(10).color(Palette::text_muted()),
                                text("Format").size(10).color(Palette::text_muted()),
                            ]
                            .spacing(4)
                            .align_y(iced::Alignment::Center),
                        )
                            .on_press(Message::Request(RequestMsg::FormatBody))
                            .style(|_t, s| iced::widget::button::Style {
                                background: if matches!(s, iced::widget::button::Status::Hovered) {
                                    Some(Background::Color(Palette::hover()))
                                } else {
                                    Some(Background::Color(Palette::surface_high()))
                                },
                                text_color: Palette::text_muted(),
                                border: Border { color: Palette::border_subtle(), width: 1.0, radius: 6.0.into() },
                                ..Default::default()
                            })
                            .padding([3, 8])
                    } else {
                        button(text("").size(10))
                            .style(iced::widget::button::text)
                            .padding([2, 0])
                    },
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .padding([3, 8]),
            )
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Palette::background())),
                border: Border {
                    color: Palette::border_subtle(),
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .width(Length::Fill);

            let editor: Element<Message> = tab.body_editor
                .view()
                .map(|m| Message::Request(RequestMsg::BodyEdited(m)));

            column![toolbar, editor].spacing(0).height(Length::Fill).into()
        }

        BodyType::FormData => form_data_view(&tab.form_fields),

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
                container(query).height(Length::FillPortion(3)).width(Length::Fill),
                pane_label("Variables"),
                container(variables).height(Length::FillPortion(2)).width(Length::Fill),
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

fn body_type_btn<'a>(
    label: &'static str,
    variant: BodyType,
    current: &BodyType,
) -> Element<'a, Message> {
    let active = current == &variant;
    button(text(label).size(11))
        .on_press(Message::Request(RequestMsg::BodyTypeChanged(
            variant.as_str().to_owned(),
        )))
        .style(move |t, s| type_btn_style(t, s, active))
        .padding([5, 12])
        .into()
}

/// Section header above a split pane (GraphQL's Query / Variables).
fn pane_label(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(10).color(Palette::text_muted()))
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Palette::background())),
            ..Default::default()
        })
        .padding(iced::Padding { top: 3.0, right: 8.0, bottom: 3.0, left: 8.0 })
        .width(Length::Fill)
        .into()
}

fn form_data_view(fields: &[FormField]) -> Element<'_, Message> {
    let mut col = column![].spacing(0);
    for (i, field) in fields.iter().enumerate() {
        let is_file = field.field_type == FormFieldType::File;
        let accent = Palette::accent();
        let muted = Palette::text_muted();

        let type_btn = button(
            text(if is_file { "File" } else { "Text" }).size(10).color(if is_file { accent } else { muted }),
        )
        .on_press(Message::Request(RequestMsg::FormFieldTypeToggled(i)))
        .style(move |_, _| iced::widget::button::Style {
            background: if is_file { Some(Background::Color(Palette::accent_soft())) } else { None },
            text_color: if is_file { accent } else { muted },
            border: iced::Border { color: if is_file { accent } else { Color::TRANSPARENT }, width: 1.0, radius: 6.0.into() },
            ..Default::default()
        })
        .padding([2, 6]);

        let value_widget: Element<Message> = if is_file {
            let label = field.file_name.as_deref().unwrap_or("Choose file…");
            row![
                button(text(label).size(11).color(Palette::text()))
                    .on_press(Message::Request(RequestMsg::FormFieldPickFile(i)))
                    .style(|_, s| iced::widget::button::Style {
                        background: Some(Background::Color(if matches!(s, iced::widget::button::Status::Hovered) {
                            Palette::surface_raised()
                        } else {
                            Palette::surface_high()
                        })),
                        text_color: Palette::text(),
                        border: iced::Border { color: Palette::border_subtle(), width: 1.0, radius: 6.0.into() },
                        ..Default::default()
                    })
                    .padding([4, 10])
                    .width(Length::Fill),
            ]
            .width(Length::Fill)
            .into()
        } else {
            text_input("Value", &field.value)
                .on_input(move |s| Message::Request(RequestMsg::FormFieldValueChanged(i, s)))
                .size(12)
                .padding([4, 6])
                .width(Length::Fill)
                .style(crate::ui::styles::cell_input)
                .into()
        };

        let row_el = container(
            row![
                type_btn,
                text_input("Key", &field.key)
                    .on_input(move |s| Message::Request(RequestMsg::FormFieldKeyChanged(i, s)))
                    .size(12)
                    .padding([4, 6])
                    .width(120)
                    .style(crate::ui::styles::cell_input),
                value_widget,
                button(icons::close().size(10).color(Palette::text_muted()))
                    .on_press(Message::Request(RequestMsg::FormFieldRemoved(i)))
                    .style(iced::widget::button::text)
                    .padding([3, 6]),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .style(move |_| iced::widget::container::Style {
            background: if i % 2 == 0 {
                Some(Background::Color(Palette::row_odd()))
            } else {
                None
            },
            ..Default::default()
        })
        .padding(iced::Padding { top: 2.0, right: 8.0, bottom: 2.0, left: 8.0 })
        .width(Length::Fill);
        col = col.push(row_el);
    }
    col = col.push(
        container(
            button(
                row![
                    text("+").size(13).color(Palette::accent()),
                    text(" Add field").size(11).color(Palette::text_muted()),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::Request(RequestMsg::FormFieldAdded))
            .style(iced::widget::button::text)
            .padding([6, 14]),
        )
        .width(Length::Fill),
    );
    scrollable(col).height(Length::Fill).into()
}

fn type_btn_style(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
    active: bool,
) -> iced::widget::button::Style {
    let hovered = matches!(status, iced::widget::button::Status::Hovered);
    iced::widget::button::Style {
        background: if active {
            Some(Background::Color(Palette::surface_raised()))
        } else if hovered {
            Some(Background::Color(Palette::hover()))
        } else {
            None
        },
        text_color: if active { Palette::accent() } else { Palette::text_muted() },
        border: Border {
            color: if active { Palette::border() } else { Color::TRANSPARENT },
            width: if active { 1.0 } else { 0.0 },
            radius: 7.0.into(),
        },
        shadow: if active {
            iced::Shadow { color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 }, offset: iced::Vector::new(0.0, 1.0), blur_radius: 4.0 }
        } else {
            iced::Shadow::default()
        },
        ..Default::default()
    }
}

