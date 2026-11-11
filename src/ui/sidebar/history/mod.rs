//! Sent requests, newest first, under a header per day.

mod model;

use iced::widget::text::Wrapping;
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Background, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{Message, RequestMsg, SidebarMsg};
use crate::ui::components::button::{Size, Variant, button, button_with_icon};
use crate::ui::components::{hairline, scroll, tooltip};
use crate::ui::icons;
use crate::ui::sidebar::collections::METHOD_COLUMN;
use crate::ui::theme::{
    LINE_MD, LINE_SM, line,
    CONTROL_SM, HAIRLINE, MONO, ROW_HEIGHT, SPACE_LG, SPACE_MD, SPACE_XL, SPACE_XS,
    TAB_STRIP_HEIGHT, TEXT_CAPTION, TEXT_LG, TEXT_MD, TEXT_SM, Tokens, UI_FONT_MEDIUM,
};

pub use model::{HistoryCache, split_url};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let body = if state.history.is_empty() {
        empty_state(t)
    } else {
        list(state, t)
    };
    column![header(state, t), hairline(t), body].height(Length::Fill).into()
}

fn header<'a>(state: &AppState, t: &'static Tokens) -> Element<'a, Message> {
    let clear = button(t, "Clear", Variant::Ghost, Size::Compact)
        .on_press_maybe((!state.history.is_empty()).then_some(Message::Sidebar(SidebarMsg::ClearHistory)));
    container(
        row![
            text("History").size(TEXT_LG).font(UI_FONT_MEDIUM).color(t.text),
            Space::new().width(Length::Fill),
            tooltip::tooltip(t, clear, "Delete all history entries", None),
        ]
        .align_y(Alignment::Center),
    )
    .height(TAB_STRIP_HEIGHT - HAIRLINE)
    .padding(Padding {
        left: SPACE_LG,
        right: SPACE_MD,
        ..Padding::ZERO
    })
    .align_y(Alignment::Center)
    .into()
}

fn list<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let mut cache = state.sidebar.history_cache.borrow_mut();
    let lines = cache.sync(state);
    let rows = lines.iter().map(|line| match line {
        model::Line::Day(label) => day_header(t, label),
        model::Line::Entry(entry) => entry_row(t, entry),
    });
    scroll::vertical(t, column(rows).padding([SPACE_XS, 0.0])).height(Length::Fill).into()
}

fn day_header(t: &'static Tokens, label: &str) -> Element<'static, Message> {
    container(text(label.to_owned()).size(TEXT_CAPTION).font(UI_FONT_MEDIUM).color(t.text_muted))
        .height(CONTROL_SM)
        .padding(Padding {
            left: SPACE_MD,
            bottom: SPACE_XS,
            ..Padding::ZERO
        })
        .align_y(Alignment::End)
        .into()
}

fn entry_row(t: &'static Tokens, entry: &model::EntryLine) -> Element<'static, Message> {
    let part = |s: &str, color| text(s.to_owned()).size(TEXT_SM).font(MONO).color(color).wrapping(Wrapping::None);
    let mut url = row![part(&entry.path, t.text)].spacing(SPACE_MD).align_y(Alignment::Center);
    if let Some(host) = &entry.host {
        url = url.push(part(host, t.text_muted));
    }
    let content = row![
        text(entry.method.clone())
            .size(TEXT_CAPTION)
            .font(MONO)
            .color(t.method_color(&entry.method))
            .width(*METHOD_COLUMN),
        container(url).width(Length::Fill).clip(true),
        Space::new().width(SPACE_MD),
        text(entry.status.clone())
            .size(TEXT_SM)
            .font(UI_FONT_MEDIUM)
            .color(t.status_color(entry.code)),
    ]
    .align_y(Alignment::Center)
    .padding(Padding {
        left: SPACE_MD,
        right: SPACE_MD,
        ..Padding::ZERO
    });
    let open = iced::widget::button(container(content).center_y(Length::Fill))
        .on_press(Message::Sidebar(SidebarMsg::HistoryEntryOpened(entry.id.clone())))
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .padding(0)
        .style(move |_, status| {
            use iced::widget::button::Status;
            iced::widget::button::Style {
                background: matches!(status, Status::Hovered | Status::Pressed)
                    .then_some(Background::Color(t.hover)),
                text_color: t.text,
                ..Default::default()
            }
        });
    tooltip::tooltip(t, open, entry.summary.clone(), None)
}

fn empty_state(t: &'static Tokens) -> Element<'static, Message> {
    let new = button_with_icon(t, icons::file_plus(), "New request", Variant::Secondary, Size::Regular)
        .on_press(Message::Request(RequestMsg::NewTab));
    container(
        column![
            text("No history yet").size(TEXT_MD).line_height(line(LINE_MD)).font(UI_FONT_MEDIUM).color(t.text),
            text("Every request you send is listed here, newest first.")
                .size(TEXT_SM)
                .line_height(line(LINE_SM))
                .color(t.text_muted),
            Space::new().height(SPACE_XS),
            new,
        ]
        .spacing(SPACE_XS),
    )
    .padding([SPACE_XL, SPACE_LG])
    .into()
}
