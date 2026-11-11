use std::sync::LazyLock;

use iced::widget::{column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::message::Message;
use crate::ui::components::tooltip::shortcut;
use crate::ui::components::Surface;
use crate::ui::components::kbd::{KeyCap, kbd};
use crate::ui::theme::{CONTROL_LG, TEXT_MD, Tokens};

use super::rule;

/// What each shortcut does, its keys, and whether they take the platform modifier.
const SHORTCUTS: [(&str, &str, bool); 13] = [
    ("Send request", "Enter", true),
    ("New tab", "T", true),
    ("Close tab", "W", true),
    ("Save request", "S", true),
    ("Search or run a command", "K", true),
    ("Focus the URL", "L", true),
    ("Find in the response", "F", true),
    ("Copy as cURL", "E", true),
    ("Import cURL from the clipboard", "I", true),
    ("Next tab", "Tab", true),
    ("Switch to tab 1–9", "Alt 1–9", false),
    ("Zoom in, out, reset", "= − 0", true),
    ("Close a dialog or menu", "Esc", false),
];

/// The key labels, with the platform modifier where it applies.
static KEYS: LazyLock<Vec<String>> = LazyLock::new(|| {
    SHORTCUTS
        .iter()
        .map(|&(_, keys, with_modifier)| if with_modifier { shortcut(keys) } else { keys.to_owned() })
        .collect()
});

pub fn view<'a>(t: &'static Tokens) -> Element<'a, Message> {
    let mut list = column![];
    for (i, ((action, ..), keys)) in SHORTCUTS.iter().zip(KEYS.iter()).enumerate() {
        if i > 0 {
            list = list.push(rule(t));
        }
        list = list.push(
            container(
                row![
                    text(*action).size(TEXT_MD).color(t.text).width(Length::Fill),
                    kbd(t, KeyCap::Text(keys), Surface::Raised),
                ]
                .align_y(Alignment::Center),
            )
            .height(CONTROL_LG)
            .align_y(Alignment::Center),
        );
    }
    list.into()
}
