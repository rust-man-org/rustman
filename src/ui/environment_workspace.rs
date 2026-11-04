//! Main environment page. Composition only; table/field/button styles are atomic.
use crate::{
    app::AppState,
    domain::environment::AppEnvironment,
    message::{Message, SidebarMsg},
    ui::{
        components::{
            atoms::{Button, compact_icon_button, table_input},
            molecules::DataTable,
        },
        icons,
        theme::{Palette, UI_FONT_MEDIUM},
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{Space, checkbox, column, container, row, scrollable, text, text_input},
};

pub fn view<'a>(state: &'a AppState, env: &'a AppEnvironment) -> Element<'a, Message> {
    let active_id = env.id.clone();
    let name_id = env.id.clone();
    let header = row![
        checkbox(env.is_active)
            .on_toggle(
                move |active| Message::Sidebar(SidebarMsg::EnvironmentSelected(if active {
                    active_id.clone()
                } else {
                    String::new()
                }))
            )
            .style(crate::ui::styles::checkbox)
            .size(13),
        text_input("Environment name", &env.name)
            .id("environment-name")
            .style(crate::ui::styles::table_cell_input)
            .font(UI_FONT_MEDIUM)
            .size(13)
            .on_input(
                move |name| Message::Sidebar(SidebarMsg::EnvironmentNameChanged(
                    name_id.clone(),
                    name
                ))
            )
            .width(Length::Fill),
        text("Changes save automatically")
            .size(10)
            .color(Palette::text_muted()),
        Button::new("Close")
            .ghost()
            .on_press(Message::Sidebar(SidebarMsg::EnvironmentPageClosed)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let mut table = DataTable::ruled([
        ("Variable", Length::FillPortion(2)),
        ("Value", Length::FillPortion(3)),
        ("", Length::Fixed(32.0)),
    ]);
    for (i, (key, value)) in state.sidebar.env_edit_rows.iter().enumerate() {
        let key_id = env.id.clone();
        let value_id = env.id.clone();
        table = table.row_with_hover(
            vec![
                table_input(
                    "Variable",
                    key,
                    move |v| {
                        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(key_id.clone(), i, v))
                    },
                    Message::Sidebar(SidebarMsg::EnvironmentCellOpened {
                        environment_id: env.id.clone(),
                        row: i,
                        value: false,
                    }),
                ),
                table_input(
                    "Value",
                    value,
                    move |v| {
                        Message::Sidebar(SidebarMsg::EnvironmentVarValueChanged(
                            value_id.clone(),
                            i,
                            v,
                        ))
                    },
                    Message::Sidebar(SidebarMsg::EnvironmentCellOpened {
                        environment_id: env.id.clone(),
                        row: i,
                        value: true,
                    }),
                ),
                if state.sidebar.env_variable_hover.as_ref() == Some(&(env.id.clone(), i)) {
                    compact_icon_button(
                        icons::trash(),
                        "Remove variable",
                        Message::Sidebar(SidebarMsg::EnvironmentVarRemoved(env.id.clone(), i)),
                        22.0,
                    )
                } else {
                    Space::new().width(22).height(22).into()
                },
            ],
            Message::Sidebar(SidebarMsg::EnvironmentVariableHover(
                env.id.clone(),
                i,
                true,
            )),
            Message::Sidebar(SidebarMsg::EnvironmentVariableHover(
                env.id.clone(),
                i,
                false,
            )),
        );
    }
    table = table.row_with_hover(
        vec![
            table_input(
                "Add variable",
                "",
                {
                    let id = env.id.clone();
                    let index = state.sidebar.env_edit_rows.len();
                    move |key| {
                        Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(
                            id.clone(),
                            index,
                            key,
                        ))
                    }
                },
                Message::Sidebar(SidebarMsg::EnvironmentCellClosed),
            ),
            table_input(
                "",
                "",
                {
                    let id = env.id.clone();
                    let index = state.sidebar.env_edit_rows.len();
                    move |value| {
                        Message::Sidebar(SidebarMsg::EnvironmentVarValueChanged(
                            id.clone(),
                            index,
                            value,
                        ))
                    }
                },
                Message::Sidebar(SidebarMsg::EnvironmentCellClosed),
            ),
            Space::new().into(),
        ],
        Message::Sidebar(SidebarMsg::EnvironmentVariableHover(
            env.id.clone(),
            state.sidebar.env_edit_rows.len(),
            true,
        )),
        Message::Sidebar(SidebarMsg::EnvironmentVariableHover(
            env.id.clone(),
            state.sidebar.env_edit_rows.len(),
            false,
        )),
    );
    let mut content = column![
        header,
        scrollable(Element::from(table))
            .height(Length::Fill)
            .style(crate::ui::theme::thin_scrollbar)
    ]
    .spacing(12)
    .height(Length::Fill);
    if let Some(cell) = state
        .environment_cell
        .as_ref()
        .filter(|cell| cell.environment_id == env.id)
    {
        let token = cell.token.clone();
        content = content.push(
            column![
                row![
                    text(if cell.value {
                        "Edit variable value"
                    } else {
                        "Edit variable name"
                    })
                    .size(12)
                    .color(Palette::text()),
                    Space::new().width(Length::Fill),
                    Button::new("Previous").ghost().on_press_maybe(
                        (cell.editor.page() > 0).then_some(Message::Sidebar(
                            SidebarMsg::EnvironmentCellPageChanged(false)
                        ))
                    ),
                    text(format!(
                        "Page {}/{}",
                        cell.editor.page() + 1,
                        cell.editor.page_count()
                    ))
                    .size(11)
                    .color(Palette::text_muted()),
                    Button::new("Next").ghost().on_press_maybe(
                        (cell.editor.page() + 1 < cell.editor.page_count()).then_some(
                            Message::Sidebar(SidebarMsg::EnvironmentCellPageChanged(true))
                        )
                    ),
                    Button::new("Close editor")
                        .ghost()
                        .on_press(Message::Sidebar(SidebarMsg::EnvironmentCellClosed)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                container(
                    cell.editor
                        .editor()
                        .view()
                        .map(
                            move |message| Message::Sidebar(SidebarMsg::EnvironmentCellEdited {
                                token: token.clone(),
                                message
                            })
                        )
                )
                .height(180),
            ]
            .spacing(6),
        );
    }
    container(content)
        .padding([8, 16])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::ui::styles::content_surface)
        .into()
}
