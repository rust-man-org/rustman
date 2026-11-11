//! The Settings dialog: a left nav of pages and the selected page.

mod about;
mod appearance;
mod general;
mod git;
pub mod network;
mod shortcuts;

use iced::widget::{Space, Text, column, container, row, text, toggler};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{Message, SettingsMsg};
use crate::ui::components::icon_button::{IconSize, icon_button};
use crate::ui::components::menu::panel_style;
use crate::ui::components::{scroll, tooltip};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_LG, HAIRLINE, ICON_LG, ICON_MD, LINE_LG, LINE_MD, LINE_SM, RADIUS_SM, SPACE_2XL, SPACE_LG,
    SPACE_MD, SPACE_XL, TEXT_LG, TEXT_MD, TEXT_SM, TEXT_XL, Tokens, UI_FONT_MEDIUM, line,
};

const WIDTH: f32 = 880.0;
const HEIGHT: f32 = 600.0;
/// The nav and its divider together, as in the mockup.
const NAV_WIDTH: f32 = 200.0;
const TOGGLE_SIZE: f32 = 18.0;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    General,
    Appearance,
    Network,
    Git,
    Shortcuts,
    About,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::General,
        Page::Appearance,
        Page::Network,
        Page::Git,
        Page::Shortcuts,
        Page::About,
    ];

    fn title(self) -> &'static str {
        match self {
            Page::General => "General",
            Page::Appearance => "Appearance",
            Page::Network => "Network & TLS",
            Page::Git => "Git",
            Page::Shortcuts => "Shortcuts",
            Page::About => "About",
        }
    }

    fn icon(self) -> Text<'static> {
        match self {
            Page::General => icons::settings(),
            Page::Appearance => icons::palette(),
            Page::Network => icons::network(),
            Page::Git => icons::git_branch(),
            Page::Shortcuts => icons::keyboard(),
            Page::About => icons::info(),
        }
    }
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let page = state.settings_page.unwrap_or(Page::General);
    let card = container(row![nav(t, page), body(state, t, page)])
        .padding(HAIRLINE)
        .width(Length::Fill)
        .max_width(WIDTH)
        .height(Length::Fill)
        .max_height(HEIGHT)
        .style(move |_| panel_style(t));
    crate::ui::widgets::overlay::modal_layer(card, Message::Settings(SettingsMsg::Closed), move |card| {
        container(card)
            .center(Length::Fill)
            .padding(SPACE_2XL)
            .style(move |_| container::Style {
                background: Some(Background::Color(t.scrim)),
                ..Default::default()
            })
            .into()
    })
}

fn nav<'a>(t: &'static Tokens, current: Page) -> Element<'a, Message> {
    let mut items = column![
        container(text("Settings").size(TEXT_XL).line_height(line(LINE_LG)).font(UI_FONT_MEDIUM).color(t.text)).padding(Padding {
            left: SPACE_MD,
            bottom: SPACE_LG,
            ..Padding::ZERO
        })
    ]
    .spacing(2.0);
    for page in Page::ALL {
        items = items.push(nav_item(t, page, page == current));
    }
    container(items)
        .width(NAV_WIDTH - HAIRLINE)
        .height(Length::Fill)
        .padding([SPACE_XL, SPACE_MD])
        .into()
}

fn nav_item<'a>(t: &'static Tokens, page: Page, on: bool) -> Element<'a, Message> {
    let r = &t.on_raised;
    let content = row![
        container(page.icon().size(ICON_MD).color(if on { t.accent_text } else { r.text_muted })).center_x(ICON_LG),
        text(page.title()).size(TEXT_MD).line_height(line(LINE_MD)).color(if on { t.text } else { r.text_muted }),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center);
    iced::widget::button(container(content).center_y(Length::Fill))
        .on_press(Message::Settings(SettingsMsg::PageSelected(page)))
        .width(Length::Fill)
        .height(CONTROL_LG)
        .padding([0.0, SPACE_MD])
        .style(move |_, status| {
            use iced::widget::button::Status;
            let hovered = matches!(status, Status::Hovered | Status::Pressed);
            iced::widget::button::Style {
                background: if on {
                    Some(Background::Color(r.selected))
                } else {
                    hovered.then_some(Background::Color(r.hover))
                },
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

fn body<'a>(state: &'a AppState, t: &'static Tokens, page: Page) -> Element<'a, Message> {
    let close = icon_button(t, icons::close(), IconSize::Medium).on_press(Message::Settings(SettingsMsg::Closed));
    let head = row![
        text(page.title()).size(TEXT_LG).line_height(line(LINE_LG)).font(UI_FONT_MEDIUM).color(t.text),
        Space::new().width(Length::Fill),
        tooltip::tooltip(t, close, "Close", Some("Esc".to_owned())),
    ]
    .align_y(Alignment::Center);
    let content = match page {
        Page::General => general::view(state, t),
        Page::Appearance => appearance::view(state, t),
        Page::Network => network::view(state, t),
        Page::Git => git::view(state, t),
        Page::Shortcuts => shortcuts::view(t),
        Page::About => about::view(state, t),
    };
    let divider = container(Space::new().width(HAIRLINE).height(Length::Fill)).style(move |_| container::Style {
        background: Some(Background::Color(t.on_raised.border)),
        ..Default::default()
    });
    row![
        divider,
        column![
            head,
            scroll::hidden(
                t,
                container(content).padding(Padding {
                    bottom: SPACE_XL,
                    ..Padding::ZERO
                })
            )
            .height(Length::Fill),
        ]
        .spacing(SPACE_XL)
        .padding(Padding {
            top: SPACE_XL,
            right: SPACE_2XL,
            bottom: 0.0,
            left: SPACE_2XL,
        }),
    ]
    .into()
}

/// A titled section: a 13 px title, a muted description and its content.
fn section<'a>(t: &'static Tokens, title: &'static str, description: &'static str, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![
        body_text(t, title).font(UI_FONT_MEDIUM),
        hint(t, description),
        Space::new().height(SPACE_LG),
        content.into(),
    ]
    .into()
}

/// 13/20 body text.
fn body_text<'a>(t: &'static Tokens, s: &'a str) -> iced::widget::Text<'a> {
    text(s).size(TEXT_MD).line_height(line(LINE_MD)).color(t.text)
}

/// 12/18 muted text under a label.
fn hint<'a>(t: &'static Tokens, s: &'a str) -> iced::widget::Text<'a> {
    text(s).size(TEXT_SM).line_height(line(LINE_SM)).color(t.on_raised.text_muted)
}

/// A setting with its label and hint on the left and its control on the right.
pub(crate) fn setting_row<'a>(t: &'static Tokens, label: &'static str, description: &'static str, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![
        column![body_text(t, label), hint(t, description)].width(Length::Fill),
        control.into(),
    ]
    .spacing(SPACE_XL)
    .align_y(Alignment::Center)
    .into()
}

/// An on/off switch.
pub(crate) fn switch<'a>(t: &'static Tokens, on: bool, message: Message) -> Element<'a, Message> {
    toggler(on)
        .on_toggle(move |_| message.clone())
        .size(TOGGLE_SIZE)
        .style(move |_, status| {
            use iced::widget::toggler::Status;
            let hovered = matches!(status, Status::Hovered { .. });
            toggler::Style {
                background: Background::Color(if on { t.accent_fill } else { t.border_strong }),
                background_border_width: 0.0,
                background_border_color: t.border,
                foreground: Background::Color(if on { t.on_accent } else if hovered { t.text } else { t.raised }),
                foreground_border_width: 0.0,
                foreground_border_color: t.border,
                text_color: None,
                border_radius: None,
                padding_ratio: 0.1,
            }
        })
        .into()
}

/// Rows of settings separated by hairlines, as on a settings page.
fn rows<'a>(t: &'static Tokens, items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut list = column![].spacing(SPACE_XL);
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            list = list.push(rule(t));
        }
        list = list.push(item);
    }
    list.into()
}

fn rule<'a>(t: &'static Tokens) -> Element<'a, Message> {
    container(Space::new().width(Length::Fill).height(HAIRLINE))
        .style(move |_| container::Style {
            background: Some(Background::Color(t.on_raised.border)),
            ..Default::default()
        })
        .into()
}
