use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, row, text, text_input},
};

use crate::{
    message::{Message, RequestMsg},
    state::tabs::{RequestTabState, WsFrame, WsMessage},
    ui::{
        icons,
        theme::{MONO, Palette, TEXT_SM, TEXT_CAPTION},
    },
};

/// Messages the feed draws, newest last: a widget each, every frame, so
/// the rest of the kept history waits off screen.
const FEED_SHOWN: usize = 200;

pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    let ws = &tab.ws;

    let status_color = if ws.connected {
        Palette::success()
    } else {
        Palette::text_muted()
    };
    let status_bar = container(
        row![
            icons::dot(status_color),
            text(if ws.connected {
                "Connected"
            } else {
                "Disconnected"
            })
            .size(TEXT_CAPTION)
            .color(status_color),
            Space::new().width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([4, 12]);

    let mut feed = column![].spacing(4).padding([8, 10]);
    let hidden = ws.messages.len().saturating_sub(FEED_SHOWN);
    if hidden > 0 {
        feed = feed.push(text(format!("{hidden} earlier messages not shown")).size(TEXT_CAPTION).color(Palette::text_subtle()));
    }
    for msg in ws.messages.newest(FEED_SHOWN) {
        feed = feed.push(message_bubble(msg));
    }
    if ws.messages.is_empty() {
        feed = feed.push(
            container(
                text(if ws.connected {
                    "Connected — send a message to start the feed"
                } else {
                    "Not connected. Hit Connect to open the socket."
                })
                .size(TEXT_SM)
                .color(Palette::text_subtle()),
            )
            .padding([24, 0])
            .center_x(Length::Fill)
            .width(Length::Fill),
        );
    }

    let send_row = row![
        text_input("Message to send…", &ws.draft)
            .on_input(|s| Message::Request(RequestMsg::WsMessageChanged(s)))
            .on_submit(Message::Request(RequestMsg::WsSend))
            .size(12)
            .padding([8, 10])
            .width(Length::Fill)
            .style(crate::ui::styles::field_input),
        button(
            row![
                icons::send().size(12).color(Color::WHITE),
                text("Send").size(TEXT_SM).color(Color::WHITE),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .on_press(Message::Request(RequestMsg::WsSend))
        .style(send_style)
        .padding([8, 14]),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding([8, 10]);

    column![
        status_bar,
        crate::ui::components::scroll::vertical(crate::ui::theme::Palette::tokens(), feed).height(Length::Fill),
        container(send_row).style(|_| container::Style {
            background: Some(Background::Color(Palette::background())),
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 0.0.into()
            },
            ..Default::default()
        }),
    ]
    .spacing(0)
    .into()
}

/// Chat-style bubble: outgoing on the right in accent, incoming on the left.
fn message_bubble(msg: &WsMessage) -> Element<'_, Message> {
    let (bg, border_color, label, label_color) = if msg.frame == WsFrame::Sent {
        (
            Palette::accent_soft(),
            Color {
                a: 0.3,
                ..Palette::accent()
            },
            "→ sent",
            Palette::accent(),
        )
    } else {
        (
            Palette::surface_high(),
            Palette::border_subtle(),
            "← recv",
            Palette::success(),
        )
    };

    let bubble = container(
        column![
            text(label).size(crate::ui::theme::TEXT_CAPTION).color(label_color),
            text(msg.text.as_str())
                .size(12)
                .color(if msg.frame == WsFrame::Binary { Palette::text_muted() } else { Palette::text() })
                .font(MONO),
        ]
        .spacing(2),
    )
    .padding([6, 10])
    .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .max_width(560);

    let aligned: Element<Message> = if msg.frame == WsFrame::Sent {
        row![Space::new().width(Length::Fill), bubble].into()
    } else {
        row![bubble, Space::new().width(Length::Fill)].into()
    };
    container(aligned).width(Length::Fill).into()
}

fn send_style(_t: &iced::Theme, s: iced::widget::button::Status) -> iced::widget::button::Style {
    let accent = Palette::accent();
    iced::widget::button::Style {
        background: Some(Background::Color(match s {
            iced::widget::button::Status::Hovered => Palette::accent_hover(),
            _ => accent,
        })),
        text_color: Color::WHITE,
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color { a: 0.3, ..accent },
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}
