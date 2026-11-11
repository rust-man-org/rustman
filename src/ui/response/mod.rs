//! The response pane.

use iced::Element;
use iced::widget::column;

use crate::message::{Message, ResponseTab};
use crate::state::tabs::RequestTabState;
use crate::ui::theme::Tokens;

pub mod body;
pub mod cookies;
pub mod header;
pub mod headers;
pub mod states;
pub mod tabs;
pub mod tests;

/// The whole pane: response tabs only once there is a response to show.
pub fn pane<'a>(tab: &'a RequestTabState, t: &'a Tokens) -> Element<'a, Message> {
    if tab.is_loading {
        return states::loading(tab, t);
    }
    if tab.response.is_none() {
        return states::empty(t);
    }
    if let Some(view) = &tab.response_view
        && let Some(failure) = &view.failure
    {
        return states::failed(tab, failure, &view.status, t);
    }
    let bar = tabs::bar(tab, t);
    let bar = match header::trailing(tab, t) {
        Some(header) => bar.trailing(header),
        None => bar,
    };
    let content = match tab.active_response_tab {
        ResponseTab::Body => body::view(tab),
        ResponseTab::Headers => headers::view(tab),
        ResponseTab::Cookies => cookies::view(tab),
        ResponseTab::Tests => tests::view(tab),
    };
    column![bar, content].into()
}
