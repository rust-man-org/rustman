use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{button, column, container, row, text},
};

use crate::{
    message::{Message, RequestMsg, RequestTab},
    state::tabs::RequestTabState,
    ui::{
        icons,
        theme::{MONO, Palette, UI_FONT, UI_FONT_MEDIUM},
    },
};

/// Mockup `.req-tabs`: flat 26px tabs on one row, a hairline underneath.
pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    let tabs = [
        (RequestTab::Params, "Params"),
        (RequestTab::Auth, "Authorization"),
        (RequestTab::Headers, "Headers"),
        (RequestTab::Body, "Body"),
        (RequestTab::Scripts, "Scripts"),
    ];

    let btns: Vec<Element<Message>> = tabs
        .iter()
        .map(|(t, label)| {
            let active = &tab.active_request_tab == t;
            let count = match t {
                RequestTab::Params => tab
                    .params
                    .iter()
                    .filter(|p| p.enabled && !p.key.is_empty())
                    .count(),
                RequestTab::Headers => tab
                    .headers
                    .iter()
                    .filter(|h| h.enabled && !h.key.is_empty())
                    .count(),
                _ => 0,
            };
            let dot = (*t == RequestTab::Auth && has_auth(tab)).then(Palette::accent);
            flat_tab(
                label,
                (count > 0).then_some(count),
                dot,
                active,
                TabLook::Boxed,
                Message::Request(RequestMsg::TabSelected(t.clone())),
            )
        })
        .collect();

    column![
        container(row(btns).spacing(2).align_y(Alignment::Center))
            .padding([6, 12])
            .width(Length::Fill),
        iced::widget::rule::horizontal(1.0).style(crate::ui::styles::divider),
    ]
    .into()
}

fn has_auth(tab: &RequestTabState) -> bool {
    !tab.bearer_token.is_empty()
        || !tab.basic_user.is_empty()
        || !tab.basic_pass.is_empty()
        || !tab.api_key_name.is_empty()
        || !tab.api_key_value.is_empty()
        || !tab.cookie_string.is_empty()
        || !tab.jwt_secret.is_empty()
        || !tab.jwt_subject.is_empty()
}

/// How a [`flat_tab`] marks the active tab.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TabLook {
    /// Request tabs (`.rtab`): 26px, raised `bg-2` box when active or hovered.
    Boxed,
    /// Response tabs (`.rsp-tab`): text only, the active one brighter.
    Text,
}

/// One flat tab: label, optional mono count and status dot.
pub fn flat_tab<'a>(
    label: &str,
    count: Option<usize>,
    dot: Option<Color>,
    active: bool,
    look: TabLook,
    msg: Message,
) -> Element<'a, Message> {
    let mut content = row![].align_y(Alignment::Center).spacing(5);
    if let Some(color) = dot {
        content = content.push(icons::dot(color));
    }
    content = content.push(text(label.to_owned()).size(12).font(if active {
        UI_FONT_MEDIUM
    } else {
        UI_FONT
    }));
    if let Some(n) = count {
        content = content.push(
            text(n.to_string())
                .size(if look == TabLook::Boxed { 10 } else { 9 })
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..MONO
                })
                .color(Palette::text_subtle()),
        );
    }

    let pad = if look == TabLook::Boxed {
        [0, 10]
    } else {
        [0, 2]
    };
    button(container(content).height(26).center_y(26))
        .on_press(msg)
        .style(move |_, status| flat_tab_style(status, active, look))
        .padding(pad)
        .into()
}

fn flat_tab_style(
    status: iced::widget::button::Status,
    active: bool,
    look: TabLook,
) -> iced::widget::button::Style {
    let hovered = matches!(status, iced::widget::button::Status::Hovered);
    let lit = active || hovered;
    iced::widget::button::Style {
        background: (look == TabLook::Boxed && lit)
            .then(|| Background::Color(Palette::surface_high())),
        text_color: if lit {
            Palette::text()
        } else {
            Palette::text_muted()
        },
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
