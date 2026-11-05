//! Example workspace. The code editors share the requester's palette.
use crate::{
    app::AppState,
    domain::request::{BodyType, HttpMethod},
    message::{
        LayoutMsg, Message,
        example::{ExampleMsg, HeaderSide},
    },
    state::examples::ExampleEditor,
    ui::{
        components::atoms::Button,
        theme::{Palette, UI_FONT_MEDIUM},
    },
};
use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{
        Space, button, column, container, pick_list, row, rule, scrollable, text, text_input,
    },
};

fn msg(value: ExampleMsg) -> Message {
    Message::Example(value)
}

pub fn view<'a>(state: &'a AppState, editor: &'a ExampleEditor) -> Element<'a, Message> {
    let parent = state
        .requests
        .values()
        .flatten()
        .find(|r| r.id == editor.data.request_id);
    let path = parent
        .map(|r| {
            let col = state
                .collections
                .iter()
                .find(|c| c.id == r.collection_id)
                .map(|c| c.name.as_str())
                .unwrap_or("Collection");
            format!("{col}  ›  {}  ›", r.name)
        })
        .unwrap_or_else(|| "Parent endpoint unavailable  ›".into());
    let header = container(
        row![
            text(path).size(11).color(Palette::text_muted()),
            text_input("Example name", &editor.data.name)
                .style(crate::ui::styles::cell_input)
                .font(UI_FONT_MEDIUM)
                .size(12)
                .on_input(|v| msg(ExampleMsg::Name(v)))
                .width(Length::Fill),
            text(if editor.dirty { "Unsaved" } else { "Example" })
                .size(10)
                .color(Palette::text_muted()),
            Button::new("Revert")
                .ghost()
                .on_press(msg(ExampleMsg::Revert)),
            Button::new("Save")
                .on_press(msg(ExampleMsg::Save))
                .tooltip("Save example (Ctrl+S)"),
            Button::new("Close")
                .ghost()
                .on_press(msg(ExampleMsg::Close)),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([7, 12]);
    let url = container(
        row![
            pick_list(HttpMethod::all(), Some(&editor.data.request.method), |v| {
                msg(ExampleMsg::Method(v.to_string()))
            })
            .style(crate::ui::styles::pick_list)
            .text_size(12)
            .padding([6, 10]),
            text_input("URL", &editor.data.request.url)
                .style(crate::ui::styles::field_input)
                .on_input(|v| msg(ExampleMsg::Url(v)))
                .size(12)
                .padding([6, 10])
                .width(Length::Fill),
        ]
        .spacing(0)
        .align_y(Alignment::Center),
    )
    .padding([0, 12]);
    let request_tabs = container(
        row![
            tab(
                "Params",
                editor.request_show_params,
                ExampleMsg::ShowHeaders(HeaderSide::Params, true)
            ),
            tab(
                "Headers",
                editor.request_show_headers,
                ExampleMsg::ShowHeaders(HeaderSide::Request, true)
            ),
            tab(
                "Body",
                !editor.request_show_params && !editor.request_show_headers,
                ExampleMsg::ShowHeaders(HeaderSide::Request, false)
            ),
        ]
        .spacing(4),
    )
    .padding([5, 12]);
    let request_content: Element<Message> = if editor.request_show_params {
        table(editor, HeaderSide::Params)
    } else if editor.request_show_headers {
        table(editor, HeaderSide::Request)
    } else {
        let selected = if editor.data.request.body_type == BodyType::Json {
            "JSON"
        } else {
            "Text"
        };
        let toolbar = container(
            row![
                text("raw").size(11).color(Palette::text_muted()),
                pick_list(["JSON", "Text"], Some(selected), |s| msg(
                    ExampleMsg::BodyFormat(s.to_owned())
                ))
                .style(crate::ui::styles::pick_list)
                .text_size(11)
                .padding([3, 8]),
                Space::new().width(Length::Fill),
                Button::new("Beautify")
                    .ghost()
                    .on_press(msg(ExampleMsg::FormatBody(HeaderSide::Request))),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([0, 12]);
        column![
            toolbar,
            editor
                .request_body
                .view()
                .map(|m| msg(ExampleMsg::RequestBody(m)))
        ]
        .spacing(4)
        .height(Length::Fill)
        .into()
    };
    let request =
        column![request_tabs, request_content].height(Length::FillPortion(state.panel_split));
    let response_header = container(
        row![
            tab(
                "Body",
                !editor.response_show_headers,
                ExampleMsg::ShowHeaders(HeaderSide::Response, false)
            ),
            tab(
                "Headers",
                editor.response_show_headers,
                ExampleMsg::ShowHeaders(HeaderSide::Response, true)
            ),
            Space::new().width(Length::Fill),
            text("Status code").size(11).color(Palette::text_muted()),
            text_input("200", &editor.status)
                .style(crate::ui::styles::field_input)
                .size(11)
                .width(52)
                .on_input(|v| msg(ExampleMsg::Status(v))),
            text_input("OK", &editor.data.response.status_text)
                .style(crate::ui::styles::field_input)
                .size(11)
                .width(140)
                .on_input(|v| msg(ExampleMsg::StatusText(v))),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([5, 12]);
    let response_content: Element<Message> = if editor.response_show_headers {
        table(editor, HeaderSide::Response)
    } else {
        column![
            container(
                row![
                    text("Raw").size(11).color(Palette::text_muted()),
                    Space::new().width(Length::Fill),
                    Button::new("Beautify")
                        .ghost()
                        .on_press(msg(ExampleMsg::FormatBody(HeaderSide::Response))),
                ]
                .align_y(Alignment::Center)
            )
            .padding([0, 16]),
            editor
                .response_body
                .view()
                .map(|m| msg(ExampleMsg::ResponseBody(m))),
        ]
        .spacing(4)
        .height(Length::Fill)
        .into()
    };
    let response = column![response_header, response_content]
        .height(Length::FillPortion((100 - state.panel_split).max(1)));
    let panels = column![
        request,
        crate::ui::widgets::panel_splitter::panel_splitter(
            state.panel_split,
            crate::ui::widgets::panel_splitter::Orientation::Horizontal,
            |split| Message::Layout(LayoutMsg::PanelSplitChanged(split))
        ),
        response,
    ]
    .height(Length::Fill)
    .spacing(0);
    let mut content = column![header, url].spacing(0);
    if editor.confirm_discard {
        content = content.push(
            container(
                row![
                    text("Save or discard your example before leaving.")
                        .size(11)
                        .color(Palette::text()),
                    Space::new().width(Length::Fill),
                    Button::new("Discard changes").on_press(msg(ExampleMsg::Discard)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([6, 12]),
        );
    }
    content = content
        .push(panels)
        .push(rule::horizontal(1).style(crate::ui::styles::divider))
        .push(
            container(
                row![
                    text(state.status_message.as_deref().unwrap_or(
                        "Local example · edits do not send HTTP or change the endpoint"
                    ))
                    .size(10)
                    .color(Palette::text_muted()),
                    Space::new().width(Length::Fill),
                    Button::new(if editor.confirm_delete {
                        "Confirm delete"
                    } else {
                        "Delete example"
                    })
                    .ghost()
                    .on_press(msg(ExampleMsg::Delete)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([3, 12]),
        );
    container(content.height(Length::Fill))
        .height(Length::Fill)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Palette::surface().into()),
            text_color: Some(Palette::text()),
            ..Default::default()
        })
        .into()
}

fn tab(label: &'static str, active: bool, message: ExampleMsg) -> Element<'static, Message> {
    button(text(label).size(12))
        .padding([5, 8])
        .on_press(msg(message))
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered);
            button::Style {
                text_color: if active {
                    Palette::text()
                } else {
                    Palette::text_muted()
                },
                background: if hovered {
                    Some(Background::Color(Palette::hover()))
                } else {
                    None
                },
                border: Border {
                    width: if active { 1.0 } else { 0.0 },
                    color: Palette::border(),
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

fn table(editor: &ExampleEditor, side: HeaderSide) -> Element<'_, Message> {
    let rows = match side {
        HeaderSide::Request => &editor.request_headers,
        HeaderSide::Response => &editor.response_headers,
        HeaderSide::Params => &editor.data.request.params,
    };
    let mut content = column![
        row![
            text("Key").size(11).width(Length::Fill),
            text("Value").size(11).width(Length::Fill),
            Space::new().width(70)
        ]
        .spacing(8)
    ]
    .spacing(3);
    for (i, h) in rows.iter().enumerate() {
        content = content.push(
            row![
                text_input("Key", &h.key)
                    .style(crate::ui::styles::cell_input)
                    .size(12)
                    .on_input(move |v| msg(ExampleMsg::HeaderKey(side, i, v))),
                text_input("Value", &h.value)
                    .style(crate::ui::styles::cell_input)
                    .size(12)
                    .on_input(move |v| msg(ExampleMsg::HeaderValue(side, i, v))),
                Button::new("Remove")
                    .ghost()
                    .on_press(msg(ExampleMsg::HeaderRemove(side, i))),
            ]
            .spacing(8),
        );
    }
    content = content.push(
        Button::new(if side == HeaderSide::Params {
            "+ Add param"
        } else {
            "+ Add header"
        })
        .ghost()
        .on_press(msg(ExampleMsg::HeaderAdd(side))),
    );
    container(scrollable(content).height(Length::Fill))
        .padding([8, 16])
        .height(Length::Fill)
        .into()
}
