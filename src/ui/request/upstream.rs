//! The notice over a request a Git pull changed while it was open.

use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Background, Element, Length, Padding};

use crate::message::{GitMsg, Message};
use crate::ui::components::button::{Size, Variant, button};
use crate::ui::components::hairline;
use crate::ui::theme::{HAIRLINE, SPACE_LG, SPACE_MD, TABLE_ROW_HEIGHT, TEXT_SM, Tokens};

/// One table row high, the notice offers to take Git's version or keep the
/// edits in the tab.
pub fn view<'a>(tab_id: &'a str, t: &'a Tokens) -> Element<'a, Message> {
    let reload = button(t, "Reload", Variant::Secondary, Size::Compact)
        .on_press_with(move || Message::Git(GitMsg::ReloadTab(tab_id.to_owned())));
    let keep = button(t, "Keep mine", Variant::Ghost, Size::Compact)
        .on_press_with(move || Message::Git(GitMsg::KeepMine(tab_id.to_owned())));
    let bar = row![
        text("Git brought in a newer version of this request.").size(TEXT_SM).color(t.text),
        Space::new().width(Length::Fill),
        keep,
        reload,
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center)
    .height(TABLE_ROW_HEIGHT - HAIRLINE)
    .padding(Padding { left: SPACE_LG, right: SPACE_LG, ..Padding::ZERO });
    column![
        container(bar).style(move |_| container::Style {
            background: Some(Background::Color(t.accent_soft)),
            ..Default::default()
        }),
        hairline(t),
    ]
    .into()
}
