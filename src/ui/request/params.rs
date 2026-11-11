//! The Params tab: a key-value table kept in sync with the URL's query.

use iced::Element;

use crate::message::{Message, RequestMsg};
use crate::state::tabs::RequestTabState;
use crate::ui::components::kv_table::{Actions, Row, kv_table};
use crate::ui::components::scroll;
use crate::ui::theme::Tokens;
use crate::ui::widgets::kv_table as bulk;

const HELP: &str = "Query parameters stay in sync with the URL. Type ?key=value there, or add a row here.";

const ACTIONS: Actions<Message> = Actions {
    toggle: |i| Message::Request(RequestMsg::ParamToggled(i)),
    key: |i, s| Message::Request(RequestMsg::ParamKeyChanged(i, s)),
    value: |i, s| Message::Request(RequestMsg::ParamValueChanged(i, s)),
    remove: |i| Message::Request(RequestMsg::ParamRemoved(i)),
    append: |field, s| Message::Request(RequestMsg::ParamAppended(field, s)),
};

pub fn view<'a>(tab: &'a RequestTabState, t: &'a Tokens) -> Element<'a, Message> {
    if let Some(content) = &tab.params_bulk {
        return bulk::bulk_panel(
            content,
            |a| Message::Request(RequestMsg::ParamsBulkEdited(a)),
            Message::Request(RequestMsg::ParamsBulkToggle),
        );
    }
    let rows = tab
        .params
        .iter()
        .map(|p| Row {
            id: p.id.as_str(),
            enabled: p.enabled,
            key: p.key.as_str(),
            value: p.value.as_str(),
            suffix: None,
        })
        .collect();
    scroll::vertical(t, kv_table(t, rows, ACTIONS, "Add parameter", Some(HELP))).into()
}
