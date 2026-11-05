//! Environment navigation with explicit activation and row-anchored actions.
use crate::{
    app::AppState,
    domain::environment::AppEnvironment,
    message::{Message, SidebarMsg},
    ui::{
        components::{
            atoms::{row_action, row_toggle, text as t},
            molecules::{
                menu, menu_divider, menu_item, tree_row,
                tree_row::{ROW_H, RowFill},
            },
        },
        icons,
        theme::{Palette, UI_FONT_MEDIUM},
        tree_label::tree_label_weighted,
        widgets::popup::popup,
    },
};
use iced::{
    Element, Length,
    widget::{Space, button, column, container, mouse_area, row, scrollable},
};
pub fn view(state: &AppState) -> Element<'_, Message> {
    // Same 2px band under the label as the Collections header, so both
    // section labels line up whether either section is open or collapsed.
    let mut list = column![
        container(super::collections::section_header(
            "Environments",
            state.sidebar.environments_collapsed,
            SidebarMsg::EnvironmentsSectionToggled
        ))
        .padding([2, 0])
    ]
    .spacing(0);
    let mut rows = column![].spacing(2);
    if !state.sidebar.environments_collapsed {
        if state.environments.is_empty() {
            rows = rows.push(container(t::meta("No environments yet.")).padding([6, 24]));
        }
        for env in &state.environments {
            rows = rows.push(environment_row(state, env, 24.0));
        }
        list = list.push(
            container(
                scrollable(container(rows).padding(iced::Padding {
                    top: 2.0,
                    ..iced::Padding::ZERO
                }))
                .height(Length::Shrink)
                .style(crate::ui::theme::thin_scrollbar),
            )
            .max_height(192),
        );
    }
    list.width(Length::Fill).into()
}
pub(super) fn environment_row<'a>(
    state: &'a AppState,
    env: &'a AppEnvironment,
    indent: f32,
) -> Element<'a, Message> {
    let open = state.sidebar.context_menu.as_deref() == Some(env.id.as_str());
    let hovered = state.sidebar.hovered_row.as_deref() == Some(env.id.as_str()) || open;
    let selected = state.sidebar.environment_page.as_deref() == Some(env.id.as_str());
    let name = tree_label_weighted(
        env.name.clone(),
        12.0,
        (state.sidebar_width - indent - 78.0).max(24.0),
        UI_FONT_MEDIUM,
        Palette::text(),
    );
    let name = button(name)
        .padding(0)
        .width(Length::Fill)
        .style(|_, _| button::Style::default())
        .on_press(Message::Sidebar(SidebarMsg::EnvironmentOpened(
            env.id.clone(),
        )));
    let active = if hovered {
        row_toggle(
            icons::check_circle(),
            if env.is_active {
                "Deactivate environment"
            } else {
                "Set as active environment"
            },
            Message::Sidebar(SidebarMsg::EnvironmentSelected(if env.is_active {
                String::new()
            } else {
                env.id.clone()
            })),
            env.is_active,
        )
    } else {
        Space::new().width(22).height(22).into()
    };
    let more = if hovered {
        row_action(
            icons::more(),
            "More environment actions",
            Message::Sidebar(SidebarMsg::ContextMenuOpened(env.id.clone())),
        )
    } else {
        Space::new().width(22).height(22).into()
    };
    let base = tree_row(
        row![name, active, more]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        indent,
        ROW_H,
        if selected || hovered {
            RowFill::NeutralSelected
        } else {
            RowFill::None
        },
        Message::Sidebar(SidebarMsg::ContextMenuClosed),
    );
    let base: Element<Message> = mouse_area(base)
        .on_enter(Message::Sidebar(SidebarMsg::ItemDragOver(env.id.clone())))
        .on_exit(Message::Sidebar(SidebarMsg::RowHoverEnd(env.id.clone())))
        .on_right_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(
            env.id.clone(),
        )))
        .into();
    if !open {
        return base;
    }
    let action =
        |label: &str, msg, danger| menu_item(None, label, Some(Message::Sidebar(msg)), danger);
    let items = vec![
        action(
            if env.is_active {
                "Deactivate"
            } else {
                "Set as active"
            },
            SidebarMsg::EnvironmentSelected(if env.is_active {
                String::new()
            } else {
                env.id.clone()
            }),
            false,
        ),
        action(
            "Export",
            SidebarMsg::EnvironmentExport(env.id.clone()),
            false,
        ),
        menu_divider(),
        action(
            "Rename",
            SidebarMsg::EnvironmentRenameStarted(env.id.clone()),
            false,
        ),
        action(
            "Duplicate",
            SidebarMsg::EnvironmentDuplicated(env.id.clone()),
            false,
        ),
        menu_divider(),
        action(
            "Delete",
            SidebarMsg::EnvironmentDeleted(env.id.clone()),
            true,
        ),
    ];
    popup(
        base,
        menu(items, Length::Fixed(190.0)),
        Message::Sidebar(SidebarMsg::ContextMenuClosed),
    )
}
