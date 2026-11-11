//! The response pane's tab bar.

use crate::message::{Message, ResponseMsg, ResponseTab};
use crate::state::tabs::RequestTabState;
use crate::ui::components::tab_bar::{Tab, TabBar};
use crate::ui::theme::Tokens;

const TABS: [(ResponseTab, &str); 4] = [
    (ResponseTab::Body, "Body"),
    (ResponseTab::Headers, "Headers"),
    (ResponseTab::Cookies, "Cookies"),
    (ResponseTab::Tests, "Tests"),
];

/// The bar with the pane's tabs; `trailing` goes at its end.
pub fn bar<'a>(tab: &'a RequestTabState, t: &'a Tokens) -> TabBar<'a, Message> {
    TABS.iter().fold(TabBar::new(t), |bar, (kind, label)| {
        bar.tab(
            Tab::new(label, Message::Response(ResponseMsg::TabSelected(kind.clone())))
                .active(&tab.active_response_tab == kind)
                .count(count(tab, kind))
                .dot(tests_dot(tab, kind, t)),
        )
    })
}

fn count(tab: &RequestTabState, kind: &ResponseTab) -> usize {
    let headers = || tab.response.as_ref().filter(|r| r.status != 0).map(|r| &r.headers);
    match kind {
        ResponseTab::Headers => headers().map_or(0, std::collections::HashMap::len),
        ResponseTab::Cookies => headers().map_or(0, |h| {
            h.keys().filter(|k| k.eq_ignore_ascii_case("set-cookie")).count()
        }),
        ResponseTab::Tests => tab.test_results.len(),
        ResponseTab::Body => 0,
    }
}

/// Green when every test passed, red when any failed.
fn tests_dot(tab: &RequestTabState, kind: &ResponseTab, t: &Tokens) -> Option<iced::Color> {
    if *kind != ResponseTab::Tests || tab.test_results.is_empty() {
        return None;
    }
    Some(if tab.test_results.iter().any(|r| !r.passed) {
        t.status.error
    } else {
        t.status.success
    })
}
