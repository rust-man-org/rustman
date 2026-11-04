use crate::{
    app::AppState,
    message::{LayoutMsg, Message, ResponseMsg},
    ui::{
        components::atoms::Button,
        theme::{MONO, Palette},
    },
};
use iced::{
    Element, Length,
    widget::{Space, column, container, row, text, text_editor},
};

pub(super) fn view(state: &AppState) -> Element<'_, Message> {
    let tab = state.tabs.active_tab();
    let body: Element<Message> = if tab.script_logs.is_empty() {
        text("No console output yet. Script output appears here after a request.")
            .size(11)
            .color(Palette::text_muted())
            .into()
    } else {
        text_editor(&tab.console_editor)
            .style(crate::ui::styles::scripts_editor)
            .font(MONO)
            .size(11)
            .on_action(|action| Message::Response(ResponseMsg::ConsoleEdited(action)))
            .height(Length::Fill)
            .into()
    };
    container(
        column![
            row![
                text("Console").size(12).color(Palette::text()),
                Space::new().width(Length::Fill),
                Button::new("Close")
                    .ghost()
                    .on_press(Message::Layout(LayoutMsg::ConsoleToggled)),
            ]
            .align_y(iced::Alignment::Center),
            body
        ]
        .spacing(8),
    )
    .padding([6, 12])
    .height(160)
    .width(Length::Fill)
    .style(crate::ui::styles::content_surface)
    .into()
}
