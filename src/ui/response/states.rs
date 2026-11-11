//! The response pane before there is a body to show: waiting, nothing sent
//! yet, and a request that failed without a response.

use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::message::{Message, RequestMsg, ResponseMsg};
use crate::state::response_view::{Failure, FailureKind};
use crate::state::tabs::RequestTabState;
use crate::ui::components::button::{Size, Variant, button_with_icon, button_with_trailing_icon};
use crate::ui::components::Surface;
use crate::ui::components::kbd::{KeyCap, kbd};
use crate::ui::components::tab_bar::{Tab, TabBar};
use crate::ui::components::{card, hairline, progress, status_dot};
use crate::ui::{format, icons};
use crate::ui::theme::{
    CONTROL_LG, LINE_MD, LINE_SM, MONO, line, RADIUS_SM, SPACE_2XL, SPACE_LG, SPACE_MD, SPACE_XL, SPACE_XS, TEXT_LG,
    TEXT_MD, TEXT_SM, Tokens, UI_FONT_MEDIUM,
};

/// The failure card's width.
const CARD_WIDTH: f32 = 440.0;
/// Width of the label column in the failure details.
const DETAIL_LABEL_WIDTH: f32 = 96.0;
/// Extra space under centered content so it sits a little above center.
const OPTICAL_LIFT: f32 = 64.0;

/// The caption-only bar shown while there are no response tabs.
const CAPTION: &str = "Response";

pub fn loading<'a>(tab: &'a RequestTabState, t: &'a Tokens) -> Element<'a, Message> {
    let elapsed = tab.sent.as_ref().map(|s| s.started.elapsed()).unwrap_or_default();
    let bar = TabBar::new(t)
        .tab(Tab::caption(CAPTION))
        .trailing(text(format::seconds(elapsed.as_millis())).size(TEXT_SM).color(t.text_muted))
        .progress(Some(progress::phase(elapsed)));
    let waiting = tab.sent.as_ref().map_or("Waiting for a response", |s| s.waiting.as_str());
    column![bar, centered(prompt(t, waiting, "Press", "Esc", "to cancel"))].into()
}

pub fn empty(t: &Tokens) -> Element<'_, Message> {
    column![
        TabBar::new(t).tab(Tab::caption(CAPTION)),
        centered(prompt(t, "No response yet", "Press", SEND_KEYS, "to send it")),
    ]
    .into()
}

#[cfg(target_os = "macos")]
const SEND_KEYS: &str = "Cmd Enter";
#[cfg(not(target_os = "macos"))]
const SEND_KEYS: &str = "Ctrl Enter";

pub fn failed<'a>(tab: &'a RequestTabState, failure: &'a Failure, status: &'a str, t: &'a Tokens) -> Element<'a, Message> {
    let bar = TabBar::new(t).tab(Tab::caption(CAPTION)).trailing(
        row![
            status_dot::status_text(status, t.status.error),
            text(failure.elapsed.as_str()).size(TEXT_SM).color(t.text_muted),
        ]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center),
    );
    column![bar, centered(failure_card(failure, tab.failure_details_open, t))].into()
}

fn prompt<'a>(t: &'a Tokens, title: &'a str, before: &'a str, keys: &'a str, after: &'a str) -> Element<'a, Message> {
    column![
        text(title).size(TEXT_MD).line_height(line(LINE_MD)).font(UI_FONT_MEDIUM).color(t.text),
        row![
            text(before).size(TEXT_SM).color(t.text_muted),
            kbd(t, KeyCap::Text(keys), Surface::Canvas),
            text(after).size(TEXT_SM).color(t.text_muted),
        ]
        .spacing(SPACE_XS + 2.0)
        .align_y(Alignment::Center),
    ]
    .spacing(SPACE_XS)
    .align_x(Alignment::Center)
    .into()
}

fn centered<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(Padding {
            top: SPACE_2XL,
            right: SPACE_2XL,
            bottom: OPTICAL_LIFT,
            left: SPACE_2XL,
        })
        .center(Length::Fill)
        .into()
}

fn failure_card<'a>(failure: &'a Failure, details_open: bool, t: &'a Tokens) -> Element<'a, Message> {
    let mut content = column![
        failure_icon(failure.kind, t),
        Space::new().height(SPACE_XL),
        text(failure.title.as_str()).size(TEXT_LG).line_height(line(LINE_MD)).font(UI_FONT_MEDIUM).color(t.text),
        Space::new().height(SPACE_XS),
        text(failure.hint.as_str()).size(TEXT_MD).line_height(line(LINE_MD)).color(t.text_muted),
        Space::new().height(SPACE_XL),
        failure_actions(details_open, t),
    ];
    if details_open {
        content = content.push(Space::new().height(SPACE_XL)).push(failure_details(failure, t));
    }
    card::card(t, content).width(CARD_WIDTH).into()
}

fn failure_icon(kind: FailureKind, t: &Tokens) -> Element<'_, Message> {
    let glyph = match kind {
        FailureKind::Refused | FailureKind::Connect => icons::unplug(),
        FailureKind::Dns => icons::wifi_off(),
        FailureKind::Tls => icons::shield_alert(),
        FailureKind::Timeout => icons::hourglass(),
        FailureKind::Script | FailureKind::Other => icons::alert_triangle(),
    };
    container(glyph.color(t.status.error))
        .center(Length::Fixed(CONTROL_LG))
        .style(move |_| container::Style {
            background: Some(Background::Color(t.error_soft)),
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn failure_actions(details_open: bool, t: &Tokens) -> Element<'_, Message> {
    let chevron = if details_open { icons::chevron_up() } else { icons::chevron_down() };
    row![
        button_with_icon(t, icons::refresh(), "Retry", Variant::Secondary, Size::Large)
            .on_press(Message::Request(RequestMsg::Send)),
        button_with_trailing_icon(t, "Details", chevron, Variant::Ghost, Size::Large)
            .on_press(Message::Response(ResponseMsg::FailureDetailsToggled)),
    ]
    .spacing(SPACE_MD)
    .into()
}

fn failure_details<'a>(failure: &'a Failure, t: &'a Tokens) -> Element<'a, Message> {
    let line = |label: &'a str, value: &'a str| {
        row![
            text(label).size(TEXT_SM).line_height(line(LINE_SM)).color(t.text_muted).width(DETAIL_LABEL_WIDTH),
            text(value).size(TEXT_SM).line_height(line(LINE_SM)).font(MONO).color(t.text).width(Length::Fill),
        ]
        .spacing(SPACE_LG)
    };
    let mut rows = column![line("Error", failure.detail.as_str())].spacing(SPACE_XS);
    if let Some(request) = &failure.request {
        rows = rows.push(line("Request", request.as_str()));
    }
    rows = rows.push(line("Failed after", failure.elapsed.as_str()));
    column![hairline(t), rows].spacing(SPACE_LG).into()
}
