//! Environment navigation; both sidebar locations share the same row actions.
use crate::{
    app::AppState,
    message::{Message, SidebarMsg},
    ui::{
        components::atoms::{Button, text as t},
        theme,
    },
};
use iced::{
    Element, Length,
    widget::{Space, column, container, row, scrollable},
};
pub fn view(state: &AppState) -> Element<'_, Message> {
    let header = container(
        row![
            t::body("Environments"),
            Space::new().width(Length::Fill),
            Button::new("+ New")
                .ghost()
                .on_press(Message::Sidebar(SidebarMsg::EnvironmentCreated))
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([8, 10]);
    let mut items = column![].spacing(0);
    if state.environments.is_empty() {
        items = items.push(container(t::caption("No environments yet.")).padding(12));
    }
    for env in &state.environments {
        items = items.push(super::environment_list::environment_row(state, env, 8.0));
    }
    column![
        header,
        scrollable(items)
            .height(Length::Fill)
            .style(theme::thin_scrollbar)
    ]
    .height(Length::Fill)
    .into()
}
