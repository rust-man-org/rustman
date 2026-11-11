//! The custom title row: logo and name, the centered command search, the
//! layout toggles, and the window controls. Empty space drags the window;
//! a double click maximizes it.

use iced::widget::{Space, button, container, mouse_area, row, svg, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{Message, PaletteMsg, SettingsMsg, SidebarMsg, WindowMsg};
use crate::ui::components::icon_button::{IconSize, icon_button};
use crate::ui::components::Surface;
use crate::ui::components::kbd::{KeyCap, kbd};
use crate::ui::components::{hairline, tooltip};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_MD, ICON_LG, RADIUS_SM, SPACE_LG, SPACE_MD, SPACE_XS, TAB_STRIP_HEIGHT, TEXT_MD,
    TEXT_SM, Tokens, UI_FONT_MEDIUM,
};

/// The row's height without its bottom hairline.
const HEIGHT: f32 = TAB_STRIP_HEIGHT - 1.0;
/// Width of the centered search field.
const SEARCH_WIDTH: f32 = 280.0;
/// The logo cell lines up with the 48 px rail below it.
const LOGO_CELL: f32 = 48.0;
const LOGO: f32 = 20.0;
/// Room left for the macOS traffic lights.
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS: f32 = 72.0;

static LOGO_SVG: &[u8] = include_bytes!("../../../public/rustman-logo.svg");

pub fn view<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let bar = row![
        container(brand(t)).width(Length::Fill),
        search(t),
        container(controls(state, t)).width(Length::Fill).align_x(Alignment::End),
    ]
    .align_y(Alignment::Center)
    .height(HEIGHT);
    let draggable = mouse_area(bar)
        .on_press(Message::Window(WindowMsg::Drag))
        .on_double_click(Message::Window(WindowMsg::ToggleMaximize));
    iced::widget::column![draggable, hairline(t)].into()
}

fn brand(t: &Tokens) -> Element<'_, Message> {
    let logo = svg(svg::Handle::from_memory(LOGO_SVG)).width(LOGO).height(LOGO);
    #[cfg(target_os = "macos")]
    let lead: Element<'_, Message> = row![Space::new().width(TRAFFIC_LIGHTS), logo].into();
    #[cfg(not(target_os = "macos"))]
    let lead: Element<'_, Message> = container(logo).center_x(LOGO_CELL).into();
    row![
        lead,
        Space::new().width(SPACE_LG),
        text("Rustman").size(TEXT_MD).font(UI_FONT_MEDIUM).color(t.text),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn search(t: &Tokens) -> Element<'_, Message> {
    let field = button(
        row![
            container(icons::search().size(TEXT_MD + 1.0)).center(ICON_LG),
            text("Search or run command").size(TEXT_SM),
            Space::new().width(Length::Fill),
            kbd(t, KeyCap::Text(PALETTE_KEYS), Surface::Canvas),
        ]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center)
        .height(Length::Fill),
    )
    .on_press(Message::Palette(PaletteMsg::Open))
    .width(SEARCH_WIDTH)
    .height(CONTROL_MD)
    .padding(Padding {
        left: SPACE_MD,
        right: SPACE_XS - 1.0,
        ..Padding::ZERO
    })
    .style(move |_, status| search_style(t, status));
    tooltip(t, field, "Command palette", None)
}

#[cfg(target_os = "macos")]
const PALETTE_KEYS: &str = "Cmd K";
#[cfg(not(target_os = "macos"))]
const PALETTE_KEYS: &str = "Ctrl K";

pub fn search_style(t: &Tokens, status: iced::widget::button::Status) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    iced::widget::button::Style {
        background: Some(Background::Color(t.surface)),
        text_color: t.text_muted,
        border: Border {
            color: if matches!(status, Status::Hovered | Status::Pressed) {
                t.border_strong
            } else {
                t.border
            },
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

fn controls<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let layout_icon = if state.horizontal_layout { icons::columns() } else { icons::rows() };
    let toggles = row![
        tooltip(
            t,
            icon_button(t, icons::panel_left(), IconSize::Medium)
                .on_press(Message::Sidebar(SidebarMsg::ToggleCollapsed)),
            "Toggle sidebar",
            None,
        ),
        tooltip(
            t,
            icon_button(t, layout_icon, IconSize::Medium)
                .on_press(Message::Settings(SettingsMsg::LayoutDirectionToggled)),
            "Split direction",
            None,
        ),
    ]
    .spacing(SPACE_XS)
    .align_y(Alignment::Center);
    #[cfg(target_os = "macos")]
    return container(toggles).padding([0.0, SPACE_MD]).into();
    #[cfg(not(target_os = "macos"))]
    row![toggles, Space::new().width(SPACE_MD), window_controls(state, t)]
        .align_y(Alignment::Center)
        .into()
}

/// Minimize, maximize/restore and close, each a full-height square.
#[cfg(not(target_os = "macos"))]
fn window_controls<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let control = |glyph: iced::widget::Text<'a>, msg: WindowMsg, danger: bool| {
        button(container(glyph.size(ICON_LG)).center(Length::Fill))
            .on_press(Message::Window(msg))
            .width(TAB_STRIP_HEIGHT)
            .height(HEIGHT)
            .padding(0)
            .style(move |_, status| window_control_style(t, status, danger))
    };
    let maximize = if state.chrome.maximized { icons::restore() } else { icons::maximize() };
    row![
        control(icons::minimize(), WindowMsg::Minimize, false),
        control(maximize, WindowMsg::ToggleMaximize, false),
        control(icons::close(), WindowMsg::Close, true),
    ]
    .into()
}

fn window_control_style(t: &Tokens, status: iced::widget::button::Status, danger: bool) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    let hovered = matches!(status, Status::Hovered | Status::Pressed);
    let (background, text_color) = match (hovered, danger) {
        (true, true) => (Some(t.status.error), Color::WHITE),
        (true, false) => (Some(t.hover), t.text),
        (false, _) => (None, t.text_muted),
    };
    iced::widget::button::Style {
        background: background.map(Background::Color),
        text_color,
        ..Default::default()
    }
}
