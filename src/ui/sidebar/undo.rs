//! The toast that offers to restore the last deleted request or
//! environment, floating at the bottom of the sidebar panel.

use iced::widget::{Space, container, row, stack, text};
use iced::{Alignment, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{Message, SidebarMsg};
use crate::ui::components::Surface;
use crate::ui::components::button::{Size, Variant, button_on};
use crate::ui::components::menu::panel_style;
use crate::ui::theme::{SPACE_LG, SPACE_MD, SPACE_XS, TEXT_SM, Tokens};

/// `panel` with the toast over its bottom edge while an undo is offered.
/// The layer is always there so the panel's widget state survives it.
pub fn layer<'a>(state: &'a AppState, panel: Element<'a, Message>) -> Element<'a, Message> {
    let t = state.tokens();
    let toast: Element<'a, Message> = match &state.sidebar.undo {
        Some(undo) => container(card(t, undo.item.name()))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::End)
            .padding(SPACE_MD)
            .into(),
        None => Space::new().into(),
    };
    stack![panel, toast].into()
}

fn card<'a>(t: &'a Tokens, name: &'a str) -> Element<'a, Message> {
    let label = text(format!("Deleted '{name}'"))
        .size(TEXT_SM)
        .color(t.text)
        .width(Length::Fill)
        .wrapping(iced_core::text::Wrapping::None);
    let undo = button_on(t, Surface::Raised, "Undo", Variant::Ghost, Size::Compact)
        .on_press(Message::Sidebar(SidebarMsg::UndoDelete));
    container(row![container(label).width(Length::Fill).clip(true), undo].spacing(SPACE_MD).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding(Padding {
            top: SPACE_XS,
            right: SPACE_XS,
            bottom: SPACE_XS,
            left: SPACE_LG,
        })
        .style(move |_| panel_style(t))
        .into()
}
