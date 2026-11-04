use iced::{
    Alignment, Background, Border, Color, Element, Length, Shadow, Vector,
    widget::{Space, button, container, pick_list, row, text, text_editor},
};

use crate::{
    domain::request::HttpMethod,
    message::{Message, RequestMsg},
    state::tabs::RequestTabState,
    ui::{
        icons,
        theme::{Palette, TEXT_MD, TEXT_SM},
    },
};

pub fn view<'a>(
    tab: &'a RequestTabState,
    env: Option<&'a crate::domain::environment::AppEnvironment>,
) -> Element<'a, Message> {
    // A ws:// or wss:// URL is not an HTTP request: there is no method, no
    // cURL export and no variable-expansion preview for it. Decided up front so
    // the whole bar can be built consistently — the HTTP method picker used to
    // be baked into the URL pill unconditionally, so a WebSocket URL showed the
    // "WS" badge *and* a pointless "GET" dropdown right next to it, and picking
    // a method there silently mutated state that is never sent.
    let ws_mode = tab.is_websocket();

    let methods: Vec<&str> = HttpMethod::all().iter().map(|m| m.as_str()).collect();

    let color = method_color(&tab.method);

    // The method picker lives *inside* the URL pill — no border of its own,
    // just a divider between it and the input.
    let method_picker = pick_list(methods, Some(tab.method.as_str()), |m| {
        Message::Request(RequestMsg::MethodChanged(m.to_owned()))
    })
    .width(115)
    .text_size(TEXT_SM)
    .padding([10, 10])
    .font(crate::ui::theme::MONO)
    .style(move |_theme, _status| iced::widget::pick_list::Style {
        text_color: color,
        placeholder_color: Palette::text_subtle(),
        handle_color: Palette::text_subtle(),
        background: Background::Color(Palette::surface_high()),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
    });

    // The bar is a multi-line editor constrained to one line (Enter never
    // lands, pasted line breaks are stripped — see `RequestMsg::UrlEdited`).
    // Unlike the old single-line input it never shapes megabytes on paste:
    // huge pastes route to import before they can enter the widget, which is
    // what keeps Ctrl+V instant no matter the size.
    let url_input = url_field(tab);

    let inner_divider =
        container(Space::new().width(1).height(Length::Fill)).style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Palette::border_subtle())),
            ..Default::default()
        });

    // Clipboard/file import that bypasses text widgets entirely: pasting
    // megabytes into this bar freezes iced's input shaping for minutes
    // (see `URL_GUARD_LEN`), so huge commands go through here instead.
    let import_btn: Element<'static, Message> = button(icons::import().size(11).color(Palette::text_subtle()))
        .on_press(Message::Request(RequestMsg::OpenImportDialog))
        .style(iced::widget::button::text)
        .padding([5, 6])
        .into();

    let action_btn: Element<Message> = if ws_mode {
        ws_action_button(tab)
    } else if tab.is_loading {
        // Same fixed height as Send, so the bar doesn't resize when a request
        // starts or finishes.
        crate::ui::widgets::centered_button::centered_button(
            text("Abort").size(TEXT_SM),
            crate::ui::widgets::centered_button::BUTTON_HEIGHT,
            18.0,
        )
        .on_press(Message::Request(RequestMsg::Abort))
        .style(abort_button)
        .into()
    } else {
        // Centred in a fixed height: iced's button puts content at the top
        // padding edge with no vertical centering, so a padding-only button
        // sits visibly off-centre (issue #29).
        crate::ui::widgets::centered_button::centered_button(
            text("Send").size(TEXT_MD).color(Color::WHITE),
            crate::ui::widgets::centered_button::BUTTON_HEIGHT,
            20.0,
        )
        .on_press(Message::Request(RequestMsg::Send))
        .style(send_button)
        .into()
    };

    // Mockup `.url-row`: one flat 40px box holding method | URL | Send (Send
    // inset by 4px). In WebSocket mode the method picker is dropped (the
    // "WS" badge already says what this is). cURL export moved to the
    // request header's Share button.
    let mut pill_row = row![].align_y(Alignment::Center).height(Length::Fill);
    if ws_mode {
        pill_row = pill_row.push(container(ws_badge()).padding([0, 4]));
    } else {
        pill_row = pill_row.push(method_picker).push(inner_divider);
    }
    pill_row = pill_row
        .push(url_input)
        .push(container(action_btn).padding(4));
    let url_pill = container(pill_row)
        .style(url_pill_style)
        .height(40)
        .width(Length::Fill);

    let mut bar = row![url_pill].spacing(6).align_y(Alignment::Center);
    if !ws_mode {
        bar = bar.push(import_btn);
    }

    // Same resolution the send path uses, so the preview can't claim a different
    // URL than the one that goes out — the mismatch between this line and the
    // wire is how the double-scheme bug hid.
    let mut expanded = (!ws_mode && env.is_some() && tab.url.contains("{{"))
        .then(|| crate::services::http::resolve_url(&tab.url, env));
    if let Some(e) = expanded.as_mut() {
        if e.len() > 256 {
            let head = crate::services::format::head_capped(e.as_str(), 200);
            *e = format!("{head}…");
        }
    }

    let mut outer = iced::widget::Column::new().push(container(bar).width(Length::Fill));

    if let Some(exp) = expanded {
        outer = outer.push(
            container(
                row![
                    Space::new().width(115 + 1),
                    text(exp)
                        .size(10)
                        .color(Palette::accent())
                        .font(crate::ui::theme::MONO),
                ]
                .padding([0, 10]),
            )
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Palette::chrome())),
                border: Border {
                    color: Palette::border_subtle(),
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .width(Length::Fill),
        );
    }

    outer.into()
}

fn ws_action_button(tab: &RequestTabState) -> Element<'static, Message> {
    if tab.ws.connected {
        button(text("Disconnect").size(12).color(Color::WHITE))
            .on_press(Message::Request(RequestMsg::WsDisconnect))
            .style(abort_button)
            .padding([5, 14])
            .into()
    } else if tab.ws.connecting {
        button(text("Connecting…").size(12).color(Color::WHITE))
            .style(send_button)
            .padding([5, 14])
            .into()
    } else {
        button(text("Connect").size(12).color(Color::WHITE))
            .on_press(Message::Request(RequestMsg::WsConnect))
            .style(send_button)
            .padding([5, 14])
            .into()
    }
}

fn ws_badge() -> Element<'static, Message> {
    container(text("WS").size(12).color(Color::WHITE))
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Palette::accent())),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .padding([6, 10])
        .into()
}

fn method_color(method: &HttpMethod) -> Color {
    match method {
        HttpMethod::Get => Palette::GET,
        HttpMethod::Post => Palette::POST,
        HttpMethod::Put => Palette::PUT,
        HttpMethod::Patch => Palette::PATCH,
        HttpMethod::Delete => Palette::DELETE,
        HttpMethod::Head | HttpMethod::Options => Palette::HEAD,
    }
}

/// The URL field itself: a multi-line editor constrained to one line,
/// invisible inside the pill (the pill owns the border).
fn url_field<'a>(tab: &'a RequestTabState) -> Element<'a, Message> {
    container(
        text_editor(&tab.url_editor)
            .placeholder("https://api.example.com/path  —  or paste a cURL command")
            .on_action(|action| Message::Request(RequestMsg::UrlEdited(action)))
            .size(TEXT_MD)
            .height(Length::Shrink)
            .padding([4, 10])
            .wrapping(iced_core::text::Wrapping::None)
            .style(url_editor_style),
    )
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}


/// The URL editor itself is invisible inside the pill — the pill owns the border.
fn url_editor_style(
    _theme: &iced::Theme,
    _status: iced::widget::text_editor::Status,
) -> iced::widget::text_editor::Style {
    let accent = Palette::accent();
    iced::widget::text_editor::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        placeholder: Palette::text_subtle(),
        value: Palette::text(),
        selection: Color {
            r: accent.r,
            g: accent.g,
            b: accent.b,
            a: 0.3,
        },
    }
}

/// Flat box, mockup `.url-row`: 1px subtle border, 4px radius, no shadow.
fn url_pill_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::background())),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

fn send_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let base_bg = Palette::accent();
    let hover_bg = Color {
        r: (base_bg.r + 0.06).min(1.0),
        g: (base_bg.g + 0.06).min(1.0),
        b: (base_bg.b + 0.04).min(1.0),
        a: 1.0,
    };
    iced::widget::button::Style {
        background: Some(Background::Color(match status {
            iced::widget::button::Status::Hovered => hover_bg,
            _ => base_bg,
        })),
        text_color: Color::WHITE,
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn abort_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let base_bg = Color {
        r: 0.75,
        g: 0.20,
        b: 0.20,
        a: 1.0,
    };
    let hover_bg = Color {
        r: 0.85,
        g: 0.25,
        b: 0.25,
        a: 1.0,
    };
    iced::widget::button::Style {
        background: Some(Background::Color(match status {
            iced::widget::button::Status::Hovered => hover_bg,
            _ => base_bg,
        })),
        text_color: Color::WHITE,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        shadow: Shadow {
            color: Color {
                r: base_bg.r,
                g: base_bg.g,
                b: base_bg.b,
                a: 0.35,
            },
            offset: Vector::new(0.0, 2.0),
            blur_radius: 10.0,
        },
        ..Default::default()
    }
}
