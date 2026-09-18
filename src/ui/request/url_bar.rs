use iced::{
    widget::{button, container, pick_list, row, text, text_input, Space},
    Alignment, Background, Border, Color, Element, Length, Shadow, Vector,
};

use crate::{
    domain::request::HttpMethod,
    message::{Message, RequestMsg},
    state::tabs::RequestTabState,
    ui::{theme::{Palette, TEXT_MD, TEXT_SM}, icons},
};


pub fn view<'a>(tab: &'a RequestTabState, env: Option<&'a crate::domain::environment::AppEnvironment>) -> Element<'a, Message> {
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
    let method_picker = pick_list(
        methods,
        Some(tab.method.as_str()),
        |m| Message::Request(RequestMsg::MethodChanged(m.to_owned())),
    )
    .width(100)
    .text_size(TEXT_MD)
    .padding([6, 8])
    .style(move |_theme, _status| iced::widget::pick_list::Style {
        text_color: color,
        placeholder_color: Palette::text_subtle(),
        handle_color: Palette::text_muted(),
        background: Background::Color(Color::TRANSPARENT),
        border: Border { color: Color::TRANSPARENT, width: 0.0, radius: 0.0.into() },
    });

    let url_input = text_input(
        "https://api.example.com/path  —  or paste a cURL command",
        &tab.url,
    )
    .on_input(|v| {
        let trimmed = v.trim_start().to_lowercase();
        if trimmed.starts_with("curl ") || trimmed.starts_with("curl\t") {
            Message::Request(RequestMsg::ImportCurl(v.trim_start().to_owned()))
        } else if crate::services::import::httpie::is_httpie_command(&v) {
            Message::Request(RequestMsg::ImportHttpie(v.trim_start().to_owned()))
        } else {
            Message::Request(RequestMsg::UrlChanged(v))
        }
    })
    .size(TEXT_MD)
    .padding([10, 14])
    .width(Length::Fill)
    .style(url_input_style);

    let inner_divider = container(Space::new().width(1).height(24))
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Palette::border())),
            ..Default::default()
        });

    // One rounded pill holding method + URL — the app's hero input. In
    // WebSocket mode the method picker and its divider are dropped entirely
    // (the "WS" badge to the left of the pill already says what this is).
    let pill_row = if ws_mode {
        row![url_input]
    } else {
        row![method_picker, inner_divider, url_input]
    };
    let url_pill = container(pill_row.align_y(Alignment::Center))
        .style(url_pill_style)
        .width(Length::Fill);

    let curl_btn = button(icons::curl().size(11).color(Palette::text_subtle()))
        .on_press(Message::Request(RequestMsg::ExportCurl))
        .style(iced::widget::button::text)
        .padding([5, 6]);

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

    let mut bar = row![].spacing(6).align_y(Alignment::Center).padding([6, 10]);
    if ws_mode {
        bar = bar.push(ws_badge());
    }
    bar = bar.push(url_pill).push(action_btn);
    if !ws_mode {
        bar = bar.push(curl_btn);
    }

    // Same resolution the send path uses, so the preview can't claim a different
    // URL than the one that goes out — the mismatch between this line and the
    // wire is how the double-scheme bug hid.
    let expanded = (!ws_mode && env.is_some() && tab.url.contains("{{"))
        .then(|| crate::services::http::resolve_url(&tab.url, env));

    let mut outer = iced::widget::Column::new()
        .push(container(bar).style(url_bar_container).width(Length::Fill));

    if let Some(exp) = expanded {
        outer = outer.push(
            container(
                row![
                    Space::new().width(100 + 6),
                    text(exp).size(10).color(Palette::accent()).font(crate::ui::theme::MONO),
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
            border: Border { radius: 6.0.into(), ..Default::default() },
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

/// The URL input itself is invisible inside the pill — the pill owns the border.
fn url_input_style(
    _theme: &iced::Theme,
    _status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let accent = Palette::accent();
    iced::widget::text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border { color: Color::TRANSPARENT, width: 0.0, radius: 0.0.into() },
        icon: Palette::text_muted(),
        placeholder: Palette::text_subtle(),
        value: Palette::text(),
        selection: Color { r: accent.r, g: accent.g, b: accent.b, a: 0.3 },
    }
}

fn url_pill_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::surface_high())),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: Shadow { color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 }, offset: Vector::new(0.0, 2.0), blur_radius: 8.0 },
        ..Default::default()
    }
}

fn url_bar_container(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Palette::chrome())),
        border: Border {
            color: Palette::border_subtle(),
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: Shadow { color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.14 }, offset: Vector::new(0.0, 1.0), blur_radius: 6.0 },
        ..Default::default()
    }
}

fn send_button(_theme: &iced::Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
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
        border: Border { radius: 8.0.into(), ..Default::default() },
        shadow: Shadow { color: Color { r: base_bg.r, g: base_bg.g, b: base_bg.b, a: 0.35 }, offset: Vector::new(0.0, 2.0), blur_radius: 10.0 },
        ..Default::default()
    }
}

fn abort_button(_theme: &iced::Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
    let base_bg = Color { r: 0.75, g: 0.20, b: 0.20, a: 1.0 };
    let hover_bg = Color { r: 0.85, g: 0.25, b: 0.25, a: 1.0 };
    iced::widget::button::Style {
        background: Some(Background::Color(match status {
            iced::widget::button::Status::Hovered => hover_bg,
            _ => base_bg,
        })),
        text_color: Color::WHITE,
        border: Border { radius: 6.0.into(), ..Default::default() },
        shadow: Shadow { color: Color { r: base_bg.r, g: base_bg.g, b: base_bg.b, a: 0.35 }, offset: Vector::new(0.0, 2.0), blur_radius: 10.0 },
        ..Default::default()
    }
}
