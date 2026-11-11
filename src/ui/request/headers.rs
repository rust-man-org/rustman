//! The Headers tab: a key-value table, with value suggestions on a
//! Content-Type row.

use iced::Element;

use crate::message::{AppMsg, MenuId, Message, RequestMsg};
use crate::state::tabs::RequestTabState;
use crate::ui::components::icon_button::{IconSize, icon_button_toggled};
use crate::ui::components::kv_table::{Actions, Row, kv_table};
use crate::ui::components::menu::{self, Align, Entry, Item};
use crate::ui::components::scroll;
use crate::ui::icons;
use crate::ui::theme::{TEXT_SM, Tokens};
use crate::ui::widgets::kv_table as bulk;

const CONTENT_TYPE_SUGGESTIONS: &[&str] = &[
    "application/json",
    "application/xml",
    "text/plain",
    "text/html",
    "text/css",
    "text/javascript",
    "application/javascript",
    "application/x-www-form-urlencoded",
    "multipart/form-data",
    "application/octet-stream",
    "application/pdf",
    "application/zip",
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/svg+xml",
    "audio/mpeg",
    "video/mp4",
    "application/graphql-response+json",
    "application/ld+json",
    "application/merge-patch+json",
    "application/problem+json",
    "application/vnd.api+json",
    "application/yaml",
    "text/csv",
    "text/xml",
    "application/grpc",
    "application/x-protobuf",
    "application/x-ndjson",
];

const HELP: &str = "No custom headers. Content-Type comes from the body type, and auth headers from the Auth tab.";
/// Width of the suggestions menu.
const MENU_WIDTH: f32 = 320.0;

const ACTIONS: Actions<Message> = Actions {
    toggle: |i| Message::Request(RequestMsg::HeaderToggled(i)),
    key: |i, s| Message::Request(RequestMsg::HeaderKeyChanged(i, s)),
    value: |i, s| Message::Request(RequestMsg::HeaderValueChanged(i, s)),
    remove: |i| Message::Request(RequestMsg::HeaderRemoved(i)),
    append: |field, s| Message::Request(RequestMsg::HeaderAppended(field, s)),
};

pub fn view<'a>(tab: &'a RequestTabState, open_menu: Option<MenuId>, t: &'a Tokens) -> Element<'a, Message> {
    if let Some(content) = &tab.headers_bulk {
        return bulk::bulk_panel(
            content,
            |a| Message::Request(RequestMsg::HeadersBulkEdited(a)),
            Message::Request(RequestMsg::HeadersBulkToggle),
        );
    }
    let rows = tab
        .headers
        .iter()
        .enumerate()
        .map(|(i, h)| Row {
            id: h.id.as_str(),
            enabled: h.enabled,
            key: h.key.as_str(),
            value: h.value.as_str(),
            suffix: h
                .key
                .eq_ignore_ascii_case("content-type")
                .then(|| suggestions(i, &h.id, open_menu == Some(MenuId::HeaderSuggestions(i)), t)),
        })
        .collect();
    scroll::vertical(t, kv_table(t, rows, ACTIONS, "Add header", Some(HELP))).into()
}

fn suggestions<'a>(row: usize, id: &str, open: bool, t: &'a Tokens) -> Element<'a, Message> {
    let trigger = icon_button_toggled(t, icons::chevron_down().size(TEXT_SM), IconSize::Small, open)
        .on_press(Message::App(AppMsg::MenuToggled(MenuId::HeaderSuggestions(row))));
    let panel = open.then(|| {
        let entries: Vec<Entry<'_, Message>> = CONTENT_TYPE_SUGGESTIONS
            .iter()
            .map(|s| Item::new(*s, Message::Request(RequestMsg::HeaderValueChanged(id.to_owned(), (*s).to_owned()))).into())
            .collect();
        menu::panel(t, entries, MENU_WIDTH)
    });
    menu::popover(trigger, panel, Align::End, Message::App(AppMsg::MenuDismissed))
}
