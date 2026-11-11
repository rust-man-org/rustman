//! The URL row: one 36 px field holding the method menu and the URL, then
//! Send (or Cancel), the "…" menu for import and cURL export, and the
//! environment chip.

use iced::widget::{Space, column, container, row, text, text_editor};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::domain::environment::AppEnvironment;
use crate::domain::request::HttpMethod;
use crate::message::{AppMsg, MenuId, Message, RequestMsg, SidebarMsg, SidebarPanel};
use crate::state::tabs::RequestTabState;
use crate::ui::components::button::{Size, Variant, button};
use crate::ui::components::icon_button::{IconSize, icon_button_toggled};
use crate::ui::components::menu::{self, Align, Entry, Item};
use crate::ui::components::tooltip::shortcut;
use crate::ui::components::hover_box::hover_box;
use crate::ui::components::{Surface, field, status_dot, tooltip};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_XL, MONO, RADIUS_SM, SPACE_LG, SPACE_MD, SPACE_XS, TEXT_CAPTION, TEXT_MD, TEXT_SM,
    Tokens,
};

/// Widget id of the URL editor, for Ctrl+L focus.
pub const URL_EDITOR_ID: &str = "url-bar";

/// Width of the method menu inside the field.
const METHOD_WIDTH: f32 = 92.0;
/// Width of the "…" and environment menus.
const MENU_WIDTH: f32 = 280.0;
/// Expanded URLs longer than this are cut for the preview line.
const PREVIEW_MAX_BYTES: usize = 256;

pub fn view<'a>(
    tab: &'a RequestTabState,
    env: Option<&'a AppEnvironment>,
    environments: &'a [AppEnvironment],
    open_menu: Option<MenuId>,
    t: &'a Tokens,
) -> Element<'a, Message> {
    let ws = tab.is_websocket();
    let lead: Element<'a, Message> = if ws {
        ws_badge(t)
    } else {
        method_menu(tab, open_menu == Some(MenuId::Method), t)
    };
    let mut bar = row![url_field(lead, tab, t)]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center)
        .padding([SPACE_MD, SPACE_LG]);
    bar = bar.push(if ws { ws_action(tab, t) } else { send_or_cancel(tab, t) });
    if !ws {
        bar = bar.push(actions_menu(open_menu == Some(MenuId::UrlActions), t));
    }
    // The environment chip sits a step further out than the other controls.
    bar = bar.push(
        container(env_menu(env, environments, open_menu == Some(MenuId::Environment), t))
            .padding(Padding { left: SPACE_MD, ..Padding::ZERO }),
    );
    let mut outer = column![bar];
    if let Some(preview) = expanded_url(tab, env, ws, t) {
        outer = outer.push(preview);
    }
    outer.into()
}

fn url_field<'a>(lead: Element<'a, Message>, tab: &'a RequestTabState, t: &'a Tokens) -> Element<'a, Message> {
    let editor = text_editor(&tab.url_editor)
        .id(URL_EDITOR_ID)
        .placeholder("Enter a URL, or paste a cURL command")
        .on_action(|action| Message::Request(RequestMsg::UrlEdited(action)))
        .font(MONO)
        .size(TEXT_MD)
        .height(Length::Shrink)
        .padding([0.0, SPACE_LG])
        .wrapping(iced_core::text::Wrapping::None)
        .style(move |_, _| editor_style(t));
    let field = container(
        row![lead, container(editor).center_y(Length::Fill).width(Length::Fill)]
            .align_y(Alignment::Center)
            .height(Length::Fill),
    )
    .height(CONTROL_XL)
    .width(Length::Fill);
    hover_box(field, move |hovered| container::Style {
        background: Some(Background::Color(t.surface)),
        border: Border {
            color: field::field_border(t, Surface::Canvas, hovered),
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
}

fn editor_style(t: &Tokens) -> text_editor::Style {
    text_editor::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: t.text_subtle,
        value: t.text,
        selection: field::selection(t),
    }
}

fn method_menu<'a>(tab: &'a RequestTabState, open: bool, t: &'a Tokens) -> Element<'a, Message> {
    let trigger = iced::widget::button(
        row![
            text(tab.method.as_str()).font(MONO).size(TEXT_SM).color(t.method_color(tab.method.as_str())),
            Space::new().width(Length::Fill),
            icons::chevron_down().size(TEXT_SM).color(t.text_muted),
        ]
        .align_y(Alignment::Center)
        .height(Length::Fill),
    )
    .on_press(Message::App(AppMsg::MenuToggled(MenuId::Method)))
    .width(METHOD_WIDTH)
    .height(Length::Fill)
    .padding(Padding {
        left: SPACE_LG,
        right: SPACE_MD,
        ..Padding::ZERO
    })
    .style(move |_, status| method_trigger_style(t, status, open));
    let panel = open.then(|| menu::panel(t, method_entries(tab, t), METHOD_WIDTH + SPACE_LG * 4.0));
    row![
        menu::popover(trigger, panel, Align::Start, Message::App(AppMsg::MenuDismissed)),
        crate::ui::components::vsep(t, CONTROL_XL - 2.0),
    ]
    .into()
}

fn method_trigger_style(t: &Tokens, status: iced::widget::button::Status, open: bool) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    let lit = open || matches!(status, Status::Hovered | Status::Pressed);
    iced::widget::button::Style {
        background: lit.then_some(Background::Color(t.surface_hover)),
        border: Border {
            radius: iced::border::Radius::default().left(RADIUS_SM),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn method_entries<'a>(tab: &RequestTabState, t: &'a Tokens) -> Vec<Entry<'a, Message>> {
    HttpMethod::all()
        .iter()
        .map(|m| {
            Item::new(m.as_str(), Message::Request(RequestMsg::MethodChanged(m.clone())))
                .styled(t.on_raised.methods.for_method(m.as_str()), MONO)
                .checked(&tab.method == m)
                .into()
        })
        .collect()
}

fn ws_badge(t: &Tokens) -> Element<'_, Message> {
    container(text("WS").font(MONO).size(TEXT_SM).color(t.accent_text))
        .center_y(Length::Fill)
        .width(METHOD_WIDTH)
        .padding(Padding {
            left: SPACE_LG,
            ..Padding::ZERO
        })
        .into()
}

fn send_or_cancel<'a>(tab: &'a RequestTabState, t: &'a Tokens) -> Element<'a, Message> {
    let hero = |label, variant| crate::ui::components::button::fixed(t, label, variant, Size::Hero, Size::Hero.min_width());
    if tab.is_loading {
        return tooltip(
            t,
            hero("Cancel", Variant::Secondary).on_press(Message::Request(RequestMsg::Abort)),
            "Cancel",
            Some("Esc".to_owned()),
        );
    }
    let ready = !tab.url.trim().is_empty();
    tooltip(
        t,
        hero("Send", Variant::Primary).on_press_maybe(ready.then_some(Message::Request(RequestMsg::Send))),
        "Send",
        Some(shortcut("Enter")),
    )
}

fn ws_action<'a>(tab: &'a RequestTabState, t: &'a Tokens) -> Element<'a, Message> {
    let (label, variant, msg) = if tab.ws.connected {
        ("Disconnect", Variant::Secondary, Some(RequestMsg::WsDisconnect))
    } else if tab.ws.connecting {
        ("Connecting…", Variant::Primary, None)
    } else {
        ("Connect", Variant::Primary, Some(RequestMsg::WsConnect))
    };
    button(t, label, variant, Size::Hero)
        .on_press_maybe(msg.map(Message::Request))
        .into()
}

fn actions_menu(open: bool, t: &Tokens) -> Element<'_, Message> {
    let trigger = icon_button_toggled(t, icons::ellipsis(), IconSize::Large, open)
        .on_press(Message::App(AppMsg::MenuToggled(MenuId::UrlActions)));
    let panel = open.then(|| {
        menu::panel(
            t,
            vec![
                Item::new("Import cURL / HTTPie…", Message::Request(RequestMsg::OpenImportDialog))
                    .icon(icons::import())
                    .shortcut(shortcut("I"))
                    .into(),
                Item::new("Copy as cURL", Message::Request(RequestMsg::ExportCurl))
                    .icon(icons::curl())
                    .shortcut(shortcut("E"))
                    .into(),
            ],
            MENU_WIDTH,
        )
    });
    tooltip(
        t,
        menu::popover(trigger, panel, Align::End, Message::App(AppMsg::MenuDismissed)),
        "Import cURL, copy as cURL",
        None,
    )
}

fn env_menu<'a>(
    env: Option<&'a AppEnvironment>,
    environments: &'a [AppEnvironment],
    open: bool,
    t: &'a Tokens,
) -> Element<'a, Message> {
    let (dot, name) = match env {
        Some(e) => (t.status.success, e.name.as_str()),
        None => (t.text_subtle, "No environment"),
    };
    let trigger = iced::widget::button(
        row![
            status_dot::dot(dot),
            text(name).size(TEXT_SM),
            icons::chevron_down().size(TEXT_SM).color(t.text_muted),
        ]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center)
        .height(Length::Fill),
    )
    .on_press(Message::App(AppMsg::MenuToggled(MenuId::Environment)))
    .height(CONTROL_XL)
    .padding(Padding {
        left: SPACE_LG,
        right: SPACE_MD,
        ..Padding::ZERO
    })
    .style(move |_, status| env_chip_style(t, status, open));
    let panel = open.then(|| menu::panel(t, env_entries(env, environments), MENU_WIDTH));
    tooltip(
        t,
        menu::popover(trigger, panel, Align::End, Message::App(AppMsg::MenuDismissed)),
        "Environment",
        None,
    )
}

fn env_chip_style(t: &Tokens, status: iced::widget::button::Status, open: bool) -> iced::widget::button::Style {
    use iced::widget::button::Status;
    let lit = open || matches!(status, Status::Hovered | Status::Pressed);
    iced::widget::button::Style {
        background: lit.then_some(Background::Color(t.hover)),
        text_color: t.text,
        border: Border {
            color: t.border,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

fn env_entries<'a>(active: Option<&AppEnvironment>, environments: &'a [AppEnvironment]) -> Vec<Entry<'a, Message>> {
    let select = |id: &str| Message::Sidebar(SidebarMsg::EnvironmentSelected(id.to_owned()));
    let mut entries: Vec<Entry<'a, Message>> = environments
        .iter()
        .map(|e| {
            Item::new(e.name.as_str(), select(&e.id))
                .checked(active.is_some_and(|a| a.id == e.id))
                .into()
        })
        .collect();
    entries.push(Item::new("No environment", select("")).checked(active.is_none()).into());
    entries.push(Entry::Separator);
    entries.push(
        Item::new("Manage environments…", Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Environments)))
            .icon(icons::environments())
            .into(),
    );
    entries
}

/// The URL with `{{variables}}` expanded, under the field, when the active
/// environment changes what is sent.
fn expanded_url<'a>(
    tab: &RequestTabState,
    env: Option<&AppEnvironment>,
    ws: bool,
    t: &'a Tokens,
) -> Option<Element<'a, Message>> {
    if ws || env.is_none() || !tab.url.contains("{{") {
        return None;
    }
    let mut url = rustman_core::template::resolve_url(&tab.url, env);
    if url.len() > PREVIEW_MAX_BYTES {
        url = format!("{}…", rustman_core::text::head_capped(&url, PREVIEW_MAX_BYTES));
    }
    Some(
        container(text(url).size(TEXT_CAPTION).font(MONO).color(t.text_muted))
            .padding(Padding {
                left: SPACE_LG + METHOD_WIDTH + SPACE_LG,
                bottom: SPACE_XS,
                ..Padding::ZERO
            })
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(id: &str, name: &str) -> AppEnvironment {
        AppEnvironment {
            id: id.to_owned(),
            name: name.to_owned(),
            variables: std::collections::HashMap::new(),
            is_active: false,
        }
    }

    #[test]
    fn the_environment_menu_lists_each_environment_then_none_and_manage() {
        let envs = [env("a", "Local"), env("b", "Staging")];
        assert_eq!(env_entries(Some(&envs[0]), &envs).len(), 5);
    }
}
