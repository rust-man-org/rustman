//! The request pane's tab bar. On the Body tab its end holds the Format
//! action and the body-type menu, so the body starts right under the bar.

use iced::widget::{row, text};
use iced::{Alignment, Element};

use crate::domain::request::BodyType;
use crate::message::{AppMsg, MenuId, Message, RequestMsg, RequestTab};
use crate::state::tabs::RequestTabState;
use crate::ui::components::button::{Size, Variant, button};
use crate::ui::components::icon_button::IconSize;
use crate::ui::components::menu::{self, Align, Entry, Item};
use crate::ui::components::tab_bar::{Tab, TabBar};
use crate::ui::components::{icon_button, tooltip};
use crate::ui::icons;
use crate::ui::theme::{SPACE_XS, TEXT_SM, Tokens};

const TABS: [(RequestTab, &str); 5] = [
    (RequestTab::Params, "Params"),
    (RequestTab::Headers, "Headers"),
    (RequestTab::Body, "Body"),
    (RequestTab::Auth, "Auth"),
    (RequestTab::Scripts, "Scripts"),
];

const BODY_TYPES: [(BodyType, &str); 5] = [
    (BodyType::None, "None"),
    (BodyType::Json, "JSON"),
    (BodyType::Text, "Text"),
    (BodyType::FormData, "Form"),
    (BodyType::GraphQL, "GraphQL"),
];

/// Width of the body-type menu panel.
const BODY_MENU_WIDTH: f32 = 200.0;

pub fn view<'a>(tab: &'a RequestTabState, t: &'a Tokens, open_menu: Option<MenuId>) -> Element<'a, Message> {
    let bar = TABS.iter().fold(TabBar::new(t), |bar, (kind, label)| {
        bar.tab(
            Tab::new(label, Message::Request(RequestMsg::TabSelected(kind.clone())))
                .active(&tab.active_request_tab == kind)
                .count(count(tab, kind)),
        )
    });
    match tab.active_request_tab {
        RequestTab::Body => bar.trailing(body_controls(tab, t, open_menu == Some(MenuId::BodyType))),
        RequestTab::Params => bar.trailing(bulk_toggle(t, tab.params_bulk.is_some(), RequestMsg::ParamsBulkToggle)),
        RequestTab::Headers => bar.trailing(bulk_toggle(t, tab.headers_bulk.is_some(), RequestMsg::HeadersBulkToggle)),
        _ => bar,
    }
    .into()
}

/// The rows that will be sent, by the request's own rules: a param needs a
/// key or a value (`?=v` counts), a header needs a name.
fn count(tab: &RequestTabState, kind: &RequestTab) -> usize {
    match kind {
        RequestTab::Params => tab.params.iter().filter(|p| p.enabled && !p.is_blank()).count(),
        RequestTab::Headers => tab.headers.iter().filter(|h| h.enabled && !h.key.is_empty()).count(),
        _ => 0,
    }
}

fn bulk_toggle(t: &Tokens, bulk: bool, msg: RequestMsg) -> Element<'_, Message> {
    let label = if bulk { "Table view" } else { "Bulk edit" };
    button(t, label, Variant::Ghost, Size::Compact)
        .on_press(Message::Request(msg))
        .into()
}

fn body_controls<'a>(tab: &'a RequestTabState, t: &'a Tokens, menu_open: bool) -> Element<'a, Message> {
    let mut controls = row![].spacing(SPACE_XS).align_y(Alignment::Center);
    if tab.body_type == BodyType::Json {
        controls = controls.push(tooltip(
            t,
            icon_button(t, icons::braces(), IconSize::Small)
                .on_press(Message::Request(RequestMsg::FormatBody)),
            "Format JSON",
            None,
        ));
    }
    controls.push(body_type_menu(tab, t, menu_open)).into()
}

fn body_type_menu<'a>(tab: &'a RequestTabState, t: &'a Tokens, open: bool) -> Element<'a, Message> {
    let trigger = menu::trigger(t, text(body_type_label(&tab.body_type)).size(TEXT_SM), open)
        .on_press(Message::App(AppMsg::MenuToggled(MenuId::BodyType)));
    let panel = open.then(|| menu::panel(t, body_type_entries(tab), BODY_MENU_WIDTH));
    tooltip(
        t,
        menu::popover(trigger, panel, Align::End, Message::App(AppMsg::MenuDismissed)),
        "Body type",
        None,
    )
}

fn body_type_entries<'a>(tab: &RequestTabState) -> Vec<Entry<'a, Message>> {
    let mut entries: Vec<Entry<'a, Message>> = BODY_TYPES
        .iter()
        .map(|(kind, label)| {
            Item::new(*label, Message::Request(RequestMsg::BodyTypeChanged(kind.clone())))
                .checked(&tab.body_type == kind)
                .into()
        })
        .collect();
    if matches!(tab.body_type, BodyType::Json | BodyType::Text) {
        entries.push(Entry::Separator);
        entries.push(
            Item::new("Indent with tabs", Message::Request(RequestMsg::ToggleBodyIndentStyle))
                .checked(tab.body_indent_tabs)
                .into(),
        );
    }
    entries
}

pub fn body_type_label(kind: &BodyType) -> &'static str {
    BODY_TYPES
        .iter()
        .find(|(k, _)| k == kind)
        .map_or("None", |(_, label)| label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_rows_that_will_be_sent() {
        let mut tab = RequestTabState::new();
        let row = |key: &str, enabled| crate::domain::request::KeyValue {
            id: key.to_owned(),
            key: key.to_owned(),
            value: String::new(),
            enabled,
        };
        tab.params = vec![row("page", true), row("", true), row("off", false)];
        assert_eq!(count(&tab, &RequestTab::Params), 1);
        assert_eq!(count(&tab, &RequestTab::Body), 0);
    }

    #[test]
    fn every_body_type_has_a_label_and_the_indent_toggle_only_for_text_bodies() {
        let mut tab = RequestTabState::new();
        for (kind, label) in BODY_TYPES {
            assert_eq!(body_type_label(&kind), label);
        }
        tab.body_type = BodyType::FormData;
        assert_eq!(body_type_entries(&tab).len(), BODY_TYPES.len());
        tab.body_type = BodyType::Json;
        assert_eq!(body_type_entries(&tab).len(), BODY_TYPES.len() + 2);
    }
}
