//! The response header at the end of the response tab bar: colored status,
//! muted timing and size, then the body actions.

use std::sync::LazyLock;

use iced::widget::{Text, row, text};
use iced::{Alignment, Element};

use crate::domain::response::HttpResponse;
use crate::message::{Message, ResponseMsg};
use crate::state::response_view::ResponseView;
use crate::ui::tree_label::Face;
use crate::state::tabs::RequestTabState;
use crate::ui::components::icon_button::IconSize;
use crate::ui::components::tooltip::shortcut;
use crate::ui::components::{icon_button, status_dot, tooltip, vsep};
use crate::ui::icons;
use crate::ui::theme::{INLINE_SEPARATOR, SPACE_MD, TEXT_SM, Tokens};

/// The gap between the header's muted items: one space of its text, so
/// "2 ms · 1.4 KB" reads as the one line it was before it was split up for
/// the tooltips.
static GAP: LazyLock<f32> = LazyLock::new(|| Face::Regular.advance(' ', TEXT_SM));
const DOT: &str = "·";

/// `None` until there is a response to describe.
pub fn trailing<'a>(tab: &'a RequestTabState, t: &'a Tokens) -> Option<Element<'a, Message>> {
    let resp = tab.response.as_ref().filter(|r| r.error.is_none())?;
    let view = tab.response_view.as_ref()?;
    Some(
        row![
            status_dot::status_text(view.status.as_str(), t.status_color(resp.status)),
            meta(view, t),
            vsep(t, INLINE_SEPARATOR),
            actions(resp, t),
        ]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center)
        .into(),
    )
}

/// "2 ms · 1.4 KB · ↪ 2 redirects", muted. Only the size and the redirects
/// have more to say on hover.
fn meta<'a>(view: &'a ResponseView, t: &'a Tokens) -> Element<'a, Message> {
    fn muted<'a>(s: &'a str, t: &Tokens) -> Text<'a> {
        text(s).size(TEXT_SM).color(t.text_muted)
    }
    let with_tip = |item: Text<'a>, tip: Option<&'a str>| -> Element<'a, Message> {
        match tip {
            Some(tip) => tooltip(t, item, tip, None),
            None => item.into(),
        }
    };
    let mut line = row![muted(&view.elapsed, t)].spacing(*GAP).align_y(Alignment::Center);
    if let Some(size) = &view.size {
        line = line.push(muted(DOT, t)).push(with_tip(muted(size, t), view.size_tip.as_deref()));
    }
    if let Some(redirects) = &view.redirects {
        line = line.push(muted(DOT, t)).push(with_tip(muted(&redirects.label, t), Some(&redirects.hops)));
    }
    line.into()
}

fn actions<'a>(resp: &HttpResponse, t: &'a Tokens) -> Element<'a, Message> {
    let action = |icon, msg: Option<ResponseMsg>, label: &'static str, keys: Option<String>| {
        tooltip(
            t,
            icon_button(t, icon, IconSize::Small).on_press_maybe(msg.map(Message::Response)),
            label,
            keys,
        )
    };
    let savable = !resp.body_stored || resp.binary_data.is_some();
    row![
        action(icons::search(), Some(ResponseMsg::SearchOpen), "Search", Some(shortcut("F"))),
        action(icons::import(), savable.then_some(ResponseMsg::SaveBody), "Save to file", None),
        action(icons::copy(), (!resp.is_binary).then_some(ResponseMsg::CopyBody), "Copy body", None),
    ]
    .spacing(SPACE_MD)
    .into()
}
