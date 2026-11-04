use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Text, button, container, mouse_area, row, scrollable, text},
};

use crate::{
    app::AppState,
    message::{Message, RequestMsg},
    ui::{
        icons,
        theme::{MONO, Palette, TEXT_SM, TEXT_XS},
    },
};

use super::style::{method_color, tab_bar_container};

fn tab_icon_btn(icon: Text<'static>, msg: Message, side: f32) -> Element<'static, Message> {
    button(
        container(icon)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(msg)
    .style(|_t, status| {
        let hovered = matches!(status, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: if hovered {
                Some(Background::Color(Palette::hover()))
            } else {
                None
            },
            text_color: Palette::text_muted(),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .width(side)
    .height(side)
    .padding(0)
    .into()
}

fn method_chip(method: &crate::domain::request::HttpMethod) -> Element<'static, Message> {
    let color = method_color(method);
    container(text(method.as_str()).size(9).color(color).font(MONO))
        .padding([2, 5])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(iced::Color { a: 0.15, ..color })),
            border: Border {
                color: iced::Color { a: 0.35, ..color },
                width: 1.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Marks a tab as a WebSocket connection, in place of an HTTP method chip.
fn ws_chip() -> Element<'static, Message> {
    let color = Palette::accent();
    container(text("WS").size(9).color(color).font(MONO))
        .padding([2, 5])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(iced::Color { a: 0.15, ..color })),
            border: Border {
                color: iced::Color { a: 0.35, ..color },
                width: 1.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// The rounded "card" each tab lives in. Active tabs are raised with an accent
/// top edge; inactive tabs are flat until hovered.
fn tab_card(
    active: bool,
    dragging: bool,
) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
    move |_t| {
        let accent = Palette::accent();
        let (bg, border_color, border_width) = if dragging {
            (
                Some(Palette::accent_soft()),
                Color { a: 0.7, ..accent },
                1.0,
            )
        } else if active {
            (Some(Palette::surface()), Palette::border_subtle(), 1.0)
        } else {
            (None, Color::TRANSPARENT, 0.0)
        };
        iced::widget::container::Style {
            background: bg.map(Background::Color),
            border: Border {
                color: border_color,
                width: border_width,
                radius: 8.0.into(),
            },
            shadow: if active {
                iced::Shadow {
                    color: Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.35,
                    },
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 8.0,
                }
            } else {
                iced::Shadow::default()
            },
            ..Default::default()
        }
    }
}

pub(super) fn multi_tab_bar(state: &AppState) -> Element<'_, Message> {
    let tabs = state.tabs.tabs.iter().enumerate().map(|(i, tab)| {
        let active = i == state.tabs.active;
        let dragging = state.dragging_tab == Some(i);
        let title = if tab.url.is_empty() {
            tab.title.as_str()
        } else {
            &tab.url
        };
        // Truncate by characters, never by byte index: `&title[..20]` panics
        // when byte 20 falls inside a multi-byte UTF-8 sequence (a URL with
        // percent-decoded non-ASCII, or a renamed tab), and `panic = "abort"`
        // makes that a hard crash while merely drawing the tab strip.
        let short_title: String = {
            let mut chars = title.chars();
            let head: String = chars.by_ref().take(20).collect();
            if chars.next().is_some() {
                format!("{head}…")
            } else {
                head
            }
        };

        // Unsaved edits, else the persisted local draft (01): red while it
        // has field conflicts, accent otherwise.
        let draft = tab
            .saved_as
            .as_ref()
            .and_then(|(_, id)| state.drafts.get(id));
        let modified_dot = if tab.modified && tab.saved_as.is_some() {
            Some(Palette::accent())
        } else if tab.modified {
            Some(Palette::WARNING)
        } else if let Some(d) = draft {
            Some(if d.conflicts.is_empty() {
                Palette::accent()
            } else {
                Palette::ERROR
            })
        } else {
            None
        };

        let close_color = if active {
            Palette::text_muted()
        } else {
            Palette::text_subtle()
        };
        let close_btn = tab_icon_btn(
            icons::close().size(TEXT_XS).color(close_color),
            Message::Request(RequestMsg::CloseTab(i)),
            20.0,
        );

        let title_color = if active {
            Palette::text()
        } else {
            Palette::text_muted()
        };

        // Accent strip along the top of the active tab.
        let accent_strip = container(iced::widget::Space::new().height(2).width(Length::Fill))
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(if active {
                    Palette::accent()
                } else {
                    Color::TRANSPARENT
                })),
                border: Border {
                    radius: iced::border::Radius::default().top_left(8.0).top_right(8.0),
                    ..Default::default()
                },
                ..Default::default()
            })
            .width(Length::Fill);

        // A ws:// or wss:// tab has no HTTP method — showing "GET" there is
        // simply wrong (the method is never sent, and the URL bar hides its
        // picker in this mode for the same reason).
        let leading_chip = if tab.is_websocket() {
            ws_chip()
        } else {
            method_chip(&tab.method)
        };
        let mut label_row = row![leading_chip].spacing(7).align_y(Alignment::Center);
        if let Some(c) = modified_dot {
            label_row = label_row.push(icons::dot(c));
        }
        label_row = label_row.push(text(short_title).size(TEXT_SM).color(title_color));

        let tab_content = container(
            iced::widget::column![
                accent_strip,
                row![
                    container(label_row).padding(iced::Padding {
                        top: 6.0,
                        right: 10.0,
                        bottom: 7.0,
                        left: 12.0
                    }),
                    container(close_btn).padding(iced::Padding {
                        top: 0.0,
                        right: 6.0,
                        bottom: 1.0,
                        left: 0.0
                    }),
                ]
                .spacing(2)
                .align_y(Alignment::Center),
            ]
            .spacing(0),
        )
        .style(tab_card(active, dragging));

        let tab_btn = mouse_area(tab_content)
            .on_press(Message::Request(RequestMsg::TabDragStart(i)))
            .on_release(Message::Request(RequestMsg::TabDragEnd))
            .on_enter(Message::Request(RequestMsg::TabDragOver(i)));

        Element::from(tab_btn)
    });

    let mut tab_items: Vec<Element<Message>> = tabs.collect();
    // Ghost "+" button for a new tab.
    tab_items.push(
        container(
            button(
                container(icons::plus().size(11).color(Palette::text_muted()))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .on_press(Message::Request(RequestMsg::NewTab))
            .style(|_t, status| {
                let hovered = matches!(status, iced::widget::button::Status::Hovered);
                iced::widget::button::Style {
                    background: if hovered {
                        Some(Background::Color(Palette::hover()))
                    } else {
                        None
                    },
                    text_color: if hovered {
                        Palette::text()
                    } else {
                        Palette::text_muted()
                    },
                    border: Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .width(24)
            .height(24)
            .padding(0),
        )
        .padding(iced::Padding {
            top: 0.0,
            right: 4.0,
            bottom: 0.0,
            left: 4.0,
        })
        .into(),
    );

    // Global search moved to the app header (layout/header.rs).
    let bar = container(
        row![
            scrollable(row(tab_items).spacing(2).width(Length::Shrink))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new()
                        .width(3)
                        .scroller_width(3)
                        .margin(0),
                ))
                .style(crate::ui::theme::tab_scrollbar)
                .width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 4.0,
            right: 2.0,
            bottom: 0.0,
            left: 6.0,
        })
        .height(40),
    )
    .style(tab_bar_container)
    .width(Length::Fill);

    mouse_area(bar)
        .on_release(Message::Request(RequestMsg::TabDragEnd))
        .into()
}

#[cfg(test)]
mod tests {
    use crate::state::tabs::RequestTabState;

    fn tab_with_url(url: &str) -> RequestTabState {
        let mut tab = RequestTabState::new();
        tab.url = url.to_owned();
        tab
    }

    /// A ws:// or wss:// tab must not be labelled with an HTTP method: the
    /// method is never sent, so showing "GET" is simply wrong.
    #[test]
    fn websocket_urls_are_detected_for_the_tab_chip() {
        for url in [
            "ws://localhost:8080",
            "wss://echo.example.com",
            "WSS://UPPER.example.com",
            "  wss://leading-space.example.com",
        ] {
            assert!(
                tab_with_url(url).is_websocket(),
                "{url} should be treated as a WebSocket tab"
            );
        }
    }

    #[test]
    fn http_urls_still_use_a_method_chip() {
        for url in [
            "https://api.example.com",
            "http://localhost:3000",
            "",
            // Not a scheme match: must not be mistaken for a socket.
            "https://example.com/wss://nested",
        ] {
            assert!(
                !tab_with_url(url).is_websocket(),
                "{url} should keep its HTTP method chip"
            );
        }
    }

    /// The tab title is truncated by characters, not bytes. Slicing a `str` at a
    /// byte index inside a multi-byte sequence panics, and `panic = "abort"`
    /// turns that into a hard crash just from drawing the tab strip.
    #[test]
    fn tab_title_truncation_never_splits_a_character() {
        fn short_title(title: &str) -> String {
            let mut chars = title.chars();
            let head: String = chars.by_ref().take(20).collect();
            if chars.next().is_some() {
                format!("{head}…")
            } else {
                head
            }
        }

        for title in [
            "🎉".repeat(30),
            "日本語のタブタイトルです".repeat(3),
            "https://example.com/café/ünïcödé/path/that/is/long".to_owned(),
            "short".to_owned(),
        ] {
            let out = short_title(&title);
            assert!(out.chars().count() <= 21, "got {out:?}");
        }

        assert_eq!(short_title("short"), "short");
        assert!(short_title(&"a".repeat(50)).ends_with('…'));
    }
}
