//! Response head (mockup `.resp-head`): Body / Cookies / Headers / Test
//! Results tabs and a history shortcut on the left; status chip, timing and
//! size, Save (as example) and a ⋯ menu on the right.

use iced::{
    Alignment, Element, Length,
    widget::{Space, column, container, row},
};

use crate::{
    app::AppState,
    message::{Message, ResponseMsg, ResponseTab, SidebarMsg, SidebarPanel},
    ui::{
        components::molecules::menu_item,
        icons,
        request::tabs::{TabLook, flat_tab},
        theme::Palette,
        widgets::mini,
    },
};

pub const MORE_MENU: &str = "response:more";

pub fn view(state: &AppState) -> Element<'_, Message> {
    let tab = state.tabs.active_tab();
    let tabs = [
        (ResponseTab::Body, "Body"),
        (ResponseTab::Cookies, "Cookies"),
        (ResponseTab::Headers, "Headers"),
        (ResponseTab::Tests, "Test Results"),
    ];

    let mut left = row![].spacing(14).align_y(Alignment::Center);
    for (t, label) in tabs {
        let active = tab.active_response_tab == t;
        let (count, dot) = if t == ResponseTab::Tests && !tab.test_results.is_empty() {
            let failed = tab.test_results.iter().any(|r| !r.passed);
            (
                Some(tab.test_results.len()),
                Some(if failed {
                    Palette::ERROR
                } else {
                    Palette::SUCCESS
                }),
            )
        } else {
            (None, None)
        };
        left = left.push(flat_tab(
            label,
            count,
            dot,
            active,
            TabLook::Text,
            Message::Response(ResponseMsg::TabSelected(t)),
        ));
    }
    left = left.push(mini::icon_button(
        icons::history(),
        "History",
        Some(Message::Sidebar(SidebarMsg::PanelSelected(
            SidebarPanel::History,
        ))),
        false,
    ));

    let mut right = row![].spacing(8).align_y(Alignment::Center);
    if let Some(resp) = tab.response.as_ref().filter(|_| !tab.is_loading) {
        let text_ok = resp.error.is_none() && !resp.is_binary;
        let can_example = tab.saved_as.is_some() && text_ok && !resp.body_stored;
        right = right
            .push(super::status_bar::chip(resp))
            .push(super::status_bar::meta(resp))
            .push(crate::ui::components::atoms::tooltip::tooltip(
                mini::text_button(
                    Some(icons::example()),
                    "Save",
                    can_example.then_some(Message::Example(
                        crate::message::example::ExampleMsg::SaveResponse,
                    )),
                ),
                "Save this response as an example",
            ));

        let open = state.sidebar.context_menu.as_deref() == Some(MORE_MENU);
        let has_body = resp.error.is_none() && resp.status != 0;
        let items = vec![
            menu_item(
                Some(icons::copy()),
                "Copy body",
                (has_body && !resp.is_binary).then_some(Message::Response(ResponseMsg::CopyBody)),
                false,
            ),
            menu_item(
                Some(icons::file_down()),
                "Save response to file",
                (has_body && !resp.body_stored).then_some(Message::Response(ResponseMsg::SaveBody)),
                false,
            ),
            menu_item(
                Some(icons::external_link()),
                "Open in browser",
                (has_body && resp.is_html())
                    .then_some(Message::Response(ResponseMsg::OpenBodyInBrowser)),
                false,
            ),
        ];
        right = right.push(mini::anchored_menu(
            mini::icon_button(
                icons::more(),
                "More",
                Some(Message::Sidebar(SidebarMsg::ContextMenuOpened(
                    MORE_MENU.to_owned(),
                ))),
                open,
            ),
            open,
            items,
            220.0,
        ));
    }

    column![
        container(
            row![left, Space::new().width(Length::Fill), right]
                .spacing(8)
                .align_y(Alignment::Center),
        )
        .height(38)
        .center_y(38)
        .padding([0, 12])
        .width(Length::Fill),
        iced::widget::rule::horizontal(1.0).style(crate::ui::styles::divider),
    ]
    .into()
}
