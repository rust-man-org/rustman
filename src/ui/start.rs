//! The first-run Start card, shown in place of the URL row and panes while
//! the workspace is empty.

use std::sync::LazyLock;

use iced::widget::{Space, column, container, row, svg, text};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::message::{AppMsg, ImportMsg, Message, RequestMsg};
use crate::ui::components::kbd::{KeyCap, kbd};
use crate::ui::components::{Surface, card, tooltip::shortcut};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_LG, HAIRLINE, ICON_MD, MONO, RADIUS_SM, SPACE_2XL, SPACE_LG, SPACE_MD, SPACE_XL,
    LINE_LG, LINE_MD, SPACE_XS, TEXT_MD, TEXT_SM, TEXT_XL, Tokens, UI_FONT_MEDIUM, line,
};

const LOGO: &[u8] = include_bytes!("../../public/rustman-logo.svg");
pub const SAMPLE_URL: &str = "https://httpbin.org/json";
const WIDTH: f32 = 480.0;
const ROW: f32 = 52.0;
/// The card sits a little above the middle of the pane area.
const LIFT: f32 = 64.0;

/// Shortcut labels on the card: focus the URL, import cURL, the palette.
static KEYS: LazyLock<[String; 3]> = LazyLock::new(|| [shortcut("L"), shortcut("I"), shortcut("K")]);

pub fn view<'a>(t: &'a Tokens) -> Element<'a, Message> {
    let head = row![
        svg(svg::Handle::from_memory(LOGO)).width(CONTROL_LG).height(CONTROL_LG),
        column![
            text("Get started").size(TEXT_XL).line_height(line(LINE_LG)).font(UI_FONT_MEDIUM).color(t.text),
            text("Everything stays on this machine. No account, no telemetry.")
                .size(TEXT_MD)
                .line_height(line(LINE_MD))
                .color(t.text_muted),
        ],
    ]
    .spacing(SPACE_LG)
    .align_y(Alignment::Center);
    let rows = column![
        action(
            t,
            icons::file_plus(),
            "New request",
            detail(t, "Type a URL in the bar above"),
            Some(&KEYS[0]),
            Message::App(AppMsg::FocusUrlBar),
        ),
        action(
            t,
            icons::curl(),
            "Import cURL",
            detail(t, "Paste a cURL or HTTPie command"),
            Some(&KEYS[1]),
            Message::Request(RequestMsg::ImportFromClipboard),
        ),
        action(
            t,
            icons::file_input(),
            "Import collection",
            detail(t, "Postman, OpenAPI or a Rustman export"),
            None,
            Message::Import(ImportMsg::ImportCollectionFile),
        ),
        action(
            t,
            icons::play(),
            "Try a sample request",
            text(format!("GET {SAMPLE_URL}")).size(TEXT_SM).font(MONO).color(t.text_muted).into(),
            None,
            Message::Import(ImportMsg::SampleRequested),
        ),
    ];
    let foot = column![
        container(Space::new().width(Length::Fill).height(HAIRLINE)).style(move |_| container::Style {
            background: Some(Background::Color(t.border)),
            ..Default::default()
        }),
        row![
            kbd(t, KeyCap::Text(&KEYS[2]), Surface::Canvas),
            text("searches requests and runs any command").size(TEXT_SM).color(t.text_muted),
        ]
        .spacing(SPACE_XS + 2.0)
        .align_y(Alignment::Center),
    ]
    .spacing(SPACE_LG);
    let content = column![
        container(head).padding(Padding {
            left: SPACE_MD,
            right: SPACE_MD,
            bottom: SPACE_XL,
            ..Padding::ZERO
        }),
        rows,
        container(foot).padding(Padding {
            left: SPACE_MD,
            right: SPACE_MD,
            top: SPACE_LG,
            ..Padding::ZERO
        }),
    ];
    let card = card::card(t, content).width(WIDTH).padding(Padding {
        top: SPACE_2XL,
        right: SPACE_XL,
        bottom: SPACE_XL,
        left: SPACE_XL,
    });
    container(card)
        .center(Length::Fill)
        .padding(Padding {
            bottom: SPACE_2XL + LIFT,
            ..Padding::from(SPACE_2XL)
        })
        .into()
}

fn detail<'a>(t: &'a Tokens, s: &'static str) -> Element<'a, Message> {
    text(s).size(TEXT_SM).color(t.text_muted).into()
}

fn action<'a>(
    t: &'a Tokens,
    icon: iced::widget::Text<'a>,
    title: &'static str,
    detail: Element<'a, Message>,
    keys: Option<&'static str>,
    on_press: Message,
) -> Element<'a, Message> {
    let tile = container(icon.size(ICON_MD).color(t.accent_text))
        .center_x(CONTROL_LG)
        .center_y(CONTROL_LG)
        .style(move |_| container::Style {
            background: Some(Background::Color(t.canvas)),
            border: Border {
                color: t.border,
                width: HAIRLINE,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    let mut content = row![
        tile,
        column![text(title).size(TEXT_MD).line_height(line(LINE_MD)).font(UI_FONT_MEDIUM).color(t.text), detail].width(Length::Fill),
    ]
    .spacing(SPACE_LG)
    .align_y(Alignment::Center);
    if let Some(keys) = keys {
        content = content.push(kbd(t, KeyCap::Text(keys), Surface::Canvas));
    }
    iced::widget::button(container(content).center_y(Length::Fill))
        .on_press(on_press)
        .width(Length::Fill)
        .height(ROW)
        .padding([0.0, SPACE_MD])
        .style(move |_, status| {
            use iced::widget::button::Status;
            iced::widget::button::Style {
                background: matches!(status, Status::Hovered | Status::Pressed)
                    .then_some(Background::Color(t.surface_hover)),
                text_color: t.text,
                border: Border {
                    radius: RADIUS_SM.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}
