use iced::widget::{Space, column, container, row, svg, text};
use iced::{Alignment, Background, Border, Element, Length};

use crate::app::AppState;
use crate::message::{AppMsg, Message, SettingsMsg};
use crate::ui::components::Surface;
use crate::ui::components::button::{Size, Variant, button_on};
use crate::ui::theme::{
    CONTROL_LG, HAIRLINE, RADIUS_SM, SPACE_LG, SPACE_MD, SPACE_XL, SPACE_XS, TEXT_LG, TEXT_SM,
    Tokens, UI_FONT_MEDIUM,
};

const LOGO: &[u8] = include_bytes!("../../../public/rustman-logo.svg");
const REPOSITORY: &str = "https://github.com/rust-man-org/rustman";
const ISSUES: &str = "https://github.com/rust-man-org/rustman/issues";

pub fn view<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let link = |label: &'static str, url: &'static str| {
        button_on(t, Surface::Raised, label, Variant::Secondary, Size::Regular)
            .on_press(Message::App(AppMsg::OpenUrl(url.to_owned())))
    };
    let mut page = column![
        row![
            svg(svg::Handle::from_memory(LOGO)).width(CONTROL_LG).height(CONTROL_LG),
            column![
                text(format!("Rustman {}", env!("CARGO_PKG_VERSION"))).size(TEXT_LG).font(UI_FONT_MEDIUM).color(t.text),
                text("A native API client. Everything stays on this machine: no account, no telemetry.")
                    .size(TEXT_SM)
                    .color(t.on_raised.text_muted),
            ]
            .spacing(SPACE_XS / 2.0),
        ]
        .spacing(SPACE_LG)
        .align_y(Alignment::Center),
        row![link("Source on GitHub", REPOSITORY), link("Report a bug", ISSUES)].spacing(SPACE_MD),
        iced::widget::button(text("Built in Rust with iced").size(TEXT_SM).color(t.text_subtle))
            .on_press(Message::Settings(SettingsMsg::CreatorTapped))
            .padding(0)
            .style(|_, _| iced::widget::button::Style::default()),
    ]
    .spacing(SPACE_XL);
    #[cfg(feature = "self-update")]
    {
        page = page.push(updates(state, t));
    }
    if state.creator_unlocked {
        page = page.push(creator(t));
    }
    page.into()
}

#[cfg(feature = "self-update")]
fn updates<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    use crate::app::UpdateState;
    use crate::message::UpdateMsg;
    let busy = matches!(state.update, UpdateState::Checking | UpdateState::Installing);
    let status = match &state.update {
        UpdateState::Idle => "Not checked yet",
        UpdateState::Checking => "Checking…",
        UpdateState::Available(_) => "An update is available",
        UpdateState::Installing => "Downloading the update…",
        UpdateState::Ready(_) => "Installed. Restart to use it.",
        UpdateState::UpToDate => "You're on the latest version",
        UpdateState::Failed(_) => "The last check failed",
    };
    let check = button_on(t, Surface::Raised, "Check for updates", Variant::Secondary, Size::Regular)
        .on_press_maybe((!busy).then_some(Message::Update(UpdateMsg::Check)));
    super::rows(
        t,
        vec![
            super::setting_row(t, "Updates", status, check),
            super::setting_row(
                t,
                "Check on launch",
                "Off means no network at all when Rustman starts.",
                super::switch(t, state.auto_check_updates, Message::Settings(SettingsMsg::AutoCheckUpdatesToggled)),
            ),
        ],
    )
}

/// The card three taps on the footer reveal.
fn creator<'a>(t: &'static Tokens) -> Element<'a, Message> {
    let link = |label: &'static str, url: &'static str| {
        iced::widget::button(text(label).size(TEXT_SM).color(t.accent_text))
            .on_press(Message::App(AppMsg::OpenUrl(url.to_owned())))
            .padding(0)
            .style(|_, _| iced::widget::button::Style::default())
    };
    container(
        column![
            text("Built by Animesh, open to work and freelance gigs.").size(TEXT_SM).color(t.text),
            row![
                link("github.com/animeshchaudhri", "https://github.com/animeshchaudhri"),
                link("ac04@duck.com", "mailto:ac04@duck.com"),
                link("animesh.us", "https://animesh.us"),
                Space::new().width(Length::Fill),
            ]
            .spacing(SPACE_LG),
        ]
        .spacing(SPACE_MD),
    )
    .padding(SPACE_LG)
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(t.surface)),
        border: Border {
            color: t.on_raised.border,
            width: HAIRLINE,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
    .into()
}
