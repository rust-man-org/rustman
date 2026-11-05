//! Request tab strip, mockup `.tabsbar` / `.tab`: flat tabs separated by
//! hairlines, the active one raised to bg-0 with a 2px accent top edge.

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{Space, button, column, container, mouse_area, row, scrollable, text},
};

use crate::{
    app::AppState,
    message::{Message, RequestMsg},
    ui::{
        icons,
        theme::{MONO, Palette, TEXT_SM},
    },
};

use super::style::method_color;

/// `.tabsbar`: 36px including its 1px bottom rule.
const BAR_H: f32 = 36.0;

/// 1px hairline in `bd-1`, vertical or horizontal.
fn hairline(vertical: bool) -> Element<'static, Message> {
    let (w, h) = if vertical {
        (Length::Fixed(1.0), Length::Fill)
    } else {
        (Length::Fill, Length::Fixed(1.0))
    };
    container(Space::new())
        .width(w)
        .height(h)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Palette::border_subtle())),
            ..Default::default()
        })
        .into()
}

/// `.tab .x`: 16px, tx-3, bg-2 on hover.
fn close_button(msg: Message) -> Element<'static, Message> {
    button(container(icons::close().size(11).color(Palette::text_subtle())).center(Length::Fill))
        .on_press(msg)
        .width(16)
        .height(16)
        .padding(0)
        .style(|_t, status| iced::widget::button::Style {
            background: matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            )
            .then(|| Background::Color(Palette::surface_high())),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// `.tab .m`: the method as plain coloured mono text, no chip.
fn method_label(label: &str, color: Color) -> Element<'static, Message> {
    text(label.to_owned())
        .size(10)
        .font(MONO)
        .color(color)
        .into()
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

        // A ws:// or wss:// tab has no HTTP method — showing "GET" there is
        // simply wrong (the method is never sent, and the URL bar hides its
        // picker in this mode for the same reason).
        let lead = if tab.is_websocket() {
            method_label("WS", Palette::accent())
        } else {
            method_label(tab.method.as_str(), method_color(&tab.method))
        };
        let mut label_row = row![lead].spacing(7).align_y(Alignment::Center);
        label_row = label_row.push(text(short_title).size(TEXT_SM).color(if active {
            Palette::text()
        } else {
            Palette::text_muted()
        }));
        if let Some(c) = modified_dot {
            label_row = label_row.push(icons::dot(c));
        }
        label_row = label_row.push(close_button(Message::Request(RequestMsg::CloseTab(i))));

        // `.tab.active::after`: 2px accent along the top edge.
        let strip = container(Space::new())
            .width(Length::Fill)
            .height(2)
            .style(move |_| iced::widget::container::Style {
                background: active.then(|| Background::Color(Palette::accent())),
                ..Default::default()
            });

        let card = container(column![
            strip,
            container(label_row).padding([0, 10]).center_y(Length::Fill),
        ])
        .height(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: if dragging {
                Some(Background::Color(Palette::accent_soft()))
            } else if active {
                Some(Background::Color(Palette::surface()))
            } else {
                None
            },
            ..Default::default()
        });

        let tab_btn = mouse_area(card)
            .on_press(Message::Request(RequestMsg::TabDragStart(i)))
            .on_release(Message::Request(RequestMsg::TabDragEnd))
            .on_enter(Message::Request(RequestMsg::TabDragOver(i)))
            .interaction(iced::mouse::Interaction::Pointer);

        Element::from(row![tab_btn, hairline(true)].height(Length::Fill))
    });

    let mut tab_items: Vec<Element<Message>> = tabs.collect();
    // `.tab.new`: a bare "+", tx-3 brightening to tx-1 on hover.
    tab_items.push(
        button(container(icons::plus().size(12)).center_y(Length::Fill))
            .on_press(Message::Request(RequestMsg::NewTab))
            .height(Length::Fill)
            .padding([0, 12])
            .style(|_t, status| iced::widget::button::Style {
                text_color: if matches!(status, iced::widget::button::Status::Hovered) {
                    Palette::text()
                } else {
                    Palette::text_subtle()
                },
                ..Default::default()
            })
            .into(),
    );

    // Global search moved to the app header (layout/header.rs).
    let strip = container(
        scrollable(row(tab_items).height(Length::Fill).width(Length::Shrink))
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new()
                    .width(3)
                    .scroller_width(3)
                    .margin(0),
            ))
            .style(crate::ui::theme::tab_scrollbar)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding([0, 6])
    .height(Length::Fill)
    .width(Length::Fill);

    // Postman's right end: open-tabs list, then the environment selector.
    let strip = row![
        strip,
        super::tab_menus::tab_list(state),
        hairline(true),
        container(super::tab_menus::env_selector(state))
            .padding([0, 8])
            .center_y(Length::Fill),
    ]
    .height(Length::Fill)
    .align_y(Alignment::Center);

    let bar = container(column![strip, hairline(false)])
        .height(BAR_H)
        .width(Length::Fill)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Palette::background())),
            ..Default::default()
        });

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
