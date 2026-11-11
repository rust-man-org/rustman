//! Environments: one row each, the active one marked like a selection, and
//! the open one's variables as a two-column table.

use iced::widget::{Space, column, container, hover, row, stack, text, text_input};
use iced::{Alignment, Background, Element, Length, Padding};

use crate::app::AppState;
use crate::domain::environment::AppEnvironment;
use crate::message::{Message, SidebarMsg};
use crate::state::sidebar::{RenameTarget, rename_input_id, shadowed};
use crate::ui::components::button::{self, Size, Variant, button_with_icon};
use crate::ui::components::icon_button::{IconSize, icon_button, icon_button_toggled};
use crate::ui::components::menu::{self, Align, Entry, Item};
use crate::ui::components::{field, hairline, scroll, tooltip};
use crate::ui::icons;
use crate::ui::theme::{
    LINE_MD, LINE_SM, line,
    CONTROL_SM, HAIRLINE, ICON_LG, ICON_SM, MONO, ROW_HEIGHT, SPACE_LG, SPACE_MD, SPACE_XL,
    SPACE_XS, TAB_INDICATOR, TAB_STRIP_HEIGHT, TEXT_CAPTION, TEXT_LG, TEXT_MD, TEXT_SM, Tokens,
    UI_FONT, UI_FONT_MEDIUM,
};

/// The chevron box in front of each name, as in the collections tree.
const DISCLOSURE: f32 = ICON_LG;
/// Where names and variables start: under the parent's name.
const INSET: f32 = SPACE_MD + DISCLOSURE + SPACE_XS;
/// Room kept free at the right for the row's "…" or remove button.
const ACTIONS: f32 = CONTROL_SM + SPACE_MD;
/// Share of a variable row given to the key.
const KEY_PORTION: u16 = 2;
const VALUE_PORTION: u16 = 3;
const MENU_WIDTH: f32 = 200.0;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let body: Element<'_, Message> = if state.environments.is_empty() {
        empty_state(t)
    } else {
        let mut list = column![].padding([SPACE_XS, 0.0]);
        for env in &state.environments {
            list = list.push(environment_row(state, t, env));
            if state.sidebar.env_editing.as_deref() == Some(env.id.as_str()) {
                list = list.push(variables(t, &env.id, &state.sidebar.env_edit_rows));
            }
        }
        scroll::vertical(t, list).height(Length::Fill).into()
    };
    super::undo::layer(state, column![header(t), hairline(t), body].height(Length::Fill).into())
}

fn header<'a>(t: &'static Tokens) -> Element<'a, Message> {
    let new = button_with_icon(t, icons::plus(), "New", Variant::Secondary, Size::Regular)
        .on_press(Message::Sidebar(SidebarMsg::EnvironmentCreated));
    container(
        row![
            text("Environments").size(TEXT_LG).font(UI_FONT_MEDIUM).color(t.text),
            Space::new().width(Length::Fill),
            new,
        ]
        .align_y(Alignment::Center),
    )
    .height(TAB_STRIP_HEIGHT - HAIRLINE)
    .padding(Padding {
        left: SPACE_LG,
        right: SPACE_MD,
        ..Padding::ZERO
    })
    .align_y(Alignment::Center)
    .into()
}

fn environment_row<'a>(state: &'a AppState, t: &'static Tokens, env: &'a AppEnvironment) -> Element<'a, Message> {
    let open = state.sidebar.env_editing.as_deref() == Some(env.id.as_str());
    if state.sidebar.renaming.as_ref().is_some_and(|r| r.target.id() == env.id) {
        return rename_row(t, &env.name);
    }
    let chevron = if open { icons::chevron_down() } else { icons::chevron_right() };
    let mut content = row![
        container(chevron.size(ICON_SM).color(t.text_muted)).center_x(DISCLOSURE).center_y(DISCLOSURE),
        Space::new().width(SPACE_XS),
        text(env.name.as_str())
            .size(TEXT_MD)
            .font(if env.is_active { UI_FONT_MEDIUM } else { UI_FONT })
            .color(t.text)
            .width(Length::Fill)
            .wrapping(iced_core::text::Wrapping::None),
    ]
    .align_y(Alignment::Center);
    if env.is_active {
        content = content.push(text("Active").size(TEXT_CAPTION).font(UI_FONT_MEDIUM).color(t.accent_text));
    }
    let active = env.is_active;
    let fill = active.then_some(t.accent_soft);
    let inset = if active { SPACE_MD - TAB_INDICATOR } else { SPACE_MD };
    let padded = container(content).height(Length::Fill).align_y(Alignment::Center).padding(Padding {
        left: inset,
        right: ACTIONS,
        ..Padding::ZERO
    });
    let inner: Element<'a, Message> = if active {
        row![accent_bar(t), padded].into()
    } else {
        padded.into()
    };
    let base = iced::widget::button(inner)
        .on_press(Message::Sidebar(SidebarMsg::EnvironmentToggleEdit(env.id.clone())))
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .padding(0)
        .style(move |_, status| {
            use iced::widget::button::Status;
            let hovered = matches!(status, Status::Hovered | Status::Pressed);
            iced::widget::button::Style {
                background: fill.or(hovered.then_some(t.hover)).map(Background::Color),
                text_color: t.text,
                ..Default::default()
            }
        });
    let menu_open = state.sidebar.context_menu.as_deref() == Some(env.id.as_str());
    let more = icon_button_toggled(t, icons::more(), IconSize::Small, menu_open)
        .on_press(Message::Sidebar(SidebarMsg::ContextMenuOpened(env.id.clone())));
    let actions = container(more)
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .align_x(Alignment::End)
        .align_y(Alignment::Center)
        .padding(Padding {
            right: SPACE_XS,
            ..Padding::ZERO
        });
    if !menu_open {
        return hover(base, actions);
    }
    let panel = menu::panel(t, environment_menu(t, env), MENU_WIDTH);
    menu::popover(stack![base, actions], Some(panel), Align::End, Message::Sidebar(SidebarMsg::ContextMenuClosed))
}

fn environment_menu(t: &'static Tokens, env: &AppEnvironment) -> Vec<Entry<'static, Message>> {
    let side = Message::Sidebar;
    let mut entries = Vec::new();
    if !env.is_active {
        entries.push(Item::new("Use this environment", side(SidebarMsg::EnvironmentSelected(env.id.clone()))).icon(icons::check()).into());
    }
    entries.push(
        Item::new("Rename", side(SidebarMsg::RenameStarted(RenameTarget::Environment { id: env.id.clone() })))
            .icon(icons::edit())
            .into(),
    );
    entries.push(Entry::Separator);
    entries.push(
        Item::new("Delete", side(SidebarMsg::EnvironmentDeleted(env.id.clone())))
            .icon(icons::trash())
            .styled(t.on_raised.status.error, UI_FONT)
            .into(),
    );
    entries
}

fn accent_bar<'a>(t: &'static Tokens) -> Element<'a, Message> {
    container(Space::new().width(TAB_INDICATOR).height(Length::Fill))
        .style(move |_| container::Style {
            background: Some(Background::Color(t.accent)),
            ..Default::default()
        })
        .into()
}

fn rename_row<'a>(t: &'static Tokens, name: &'a str) -> Element<'a, Message> {
    let input = text_input("Name", name)
        .id(rename_input_id())
        .on_input(|name| Message::Sidebar(SidebarMsg::RenameEdited(name)))
        .on_submit(Message::Sidebar(SidebarMsg::RenameFinished))
        .size(TEXT_MD)
        .padding([2.0, SPACE_XS + 2.0])
        .style(move |_, status| field::field(t, status));
    container(input)
        .padding(Padding {
            left: INSET - SPACE_XS - 2.0,
            right: SPACE_MD,
            ..Padding::ZERO
        })
        .height(ROW_HEIGHT)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}

/// The open environment's variables: key and value cells that stay
/// borderless until focused, a remove button on hover, and an add row.
fn variables<'a>(t: &'static Tokens, env_id: &'a str, rows: &'a [(String, String)]) -> Element<'a, Message> {
    let columns = row![
        text("Variable").size(TEXT_CAPTION).font(UI_FONT_MEDIUM).color(t.text_muted).width(Length::FillPortion(KEY_PORTION)),
        text("Value").size(TEXT_CAPTION).font(UI_FONT_MEDIUM).color(t.text_muted).width(Length::FillPortion(VALUE_PORTION)),
    ]
    .spacing(SPACE_MD);
    let mut table = column![container(columns).height(CONTROL_SM).align_y(Alignment::End).padding(Padding {
        left: INSET,
        right: ACTIONS,
        bottom: SPACE_XS,
        ..Padding::ZERO
    })];
    for (index, (key, value)) in rows.iter().enumerate() {
        let row = variable_row(t, env_id, index, key, value, shadowed(rows, index));
        table = table.push(row);
    }
    let add = iced::widget::button(
        row![icons::plus().size(ICON_SM), text("Add variable").size(TEXT_SM)]
            .spacing(SPACE_XS + 2.0)
            .align_y(Alignment::Center),
    )
    .on_press(Message::Sidebar(SidebarMsg::EnvironmentVarAdded(env_id.to_owned())))
    .padding([0.0, SPACE_XS])
    .height(CONTROL_SM)
    .style(move |_, status| button::style(t, Variant::Ghost, status));
    table
        .push(
            container(add)
                .height(ROW_HEIGHT)
                .align_y(Alignment::Center)
                .padding(Padding {
                    left: INSET - SPACE_XS,
                    ..Padding::ZERO
                }),
        )
        .push(Space::new().height(SPACE_XS))
        .into()
}

/// One variable. A key repeating an earlier row's is shown in the error
/// color, since only the first row's value is used.
fn variable_row<'a>(
    t: &'static Tokens,
    env_id: &'a str,
    index: usize,
    key: &'a str,
    value: &'a str,
    duplicate: bool,
) -> Element<'a, Message> {
    let cell = |placeholder: &'static str, value: &'a str| {
        text_input(placeholder, value)
            .size(TEXT_SM)
            .font(MONO)
            .padding([SPACE_XS, SPACE_XS + 2.0])
            .style(move |_, status| field::cell(t, status))
    };
    let key_cell = cell("name", key)
        .style(move |_, status| iced::widget::text_input::Style {
            value: if duplicate { t.status.error } else { t.text },
            ..field::cell(t, status)
        })
        .on_input(move |k| Message::Sidebar(SidebarMsg::EnvironmentVarKeyChanged(env_id.to_owned(), index, k)))
        .width(Length::FillPortion(KEY_PORTION));
    let value_cell = cell("value", value)
        .on_input(move |v| Message::Sidebar(SidebarMsg::EnvironmentVarValueChanged(env_id.to_owned(), index, v)))
        .width(Length::FillPortion(VALUE_PORTION));
    let base = container(row![key_cell, value_cell].spacing(SPACE_MD).align_y(Alignment::Center))
        .height(ROW_HEIGHT)
        .align_y(Alignment::Center)
        .padding(Padding {
            left: INSET - SPACE_XS - 2.0,
            right: ACTIONS,
            ..Padding::ZERO
        });
    let remove = icon_button(t, icons::close(), IconSize::Small)
        .on_press(Message::Sidebar(SidebarMsg::EnvironmentVarRemoved(env_id.to_owned(), index)));
    let actions = container(remove)
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .align_x(Alignment::End)
        .align_y(Alignment::Center)
        .padding(Padding {
            right: SPACE_XS,
            ..Padding::ZERO
        });
    // The hint rides on the hover layer, which spans the row: wrapping the
    // fields instead would rebuild them, and drop the focus of the one being
    // typed in, as the name turns into a duplicate.
    let actions: Element<'a, Message> = if duplicate {
        tooltip::tooltip(t, actions, "Duplicate name: only the first row's value is used", None)
    } else {
        actions.into()
    };
    hover(base, actions)
}

fn empty_state<'a>(t: &'static Tokens) -> Element<'a, Message> {
    let create = button_with_icon(t, icons::plus(), "Create an environment", Variant::Secondary, Size::Regular)
        .on_press(Message::Sidebar(SidebarMsg::EnvironmentCreated));
    container(
        column![
            text("No environments yet").size(TEXT_MD).line_height(line(LINE_MD)).font(UI_FONT_MEDIUM).color(t.text),
            text("Environments hold variables such as base_url. Use them as {{base_url}} in URLs, params, headers and bodies.")
                .size(TEXT_SM)
                .line_height(line(LINE_SM))
                .color(t.text_muted),
            Space::new().height(SPACE_XS),
            create,
        ]
        .spacing(SPACE_XS),
    )
    .padding([SPACE_XL, SPACE_LG])
    .into()
}

#[cfg(test)]
mod tests {
    use iced::advanced::widget::Tree;
    use iced::advanced::widget::tree::Tag;

    use super::*;

    type InputState = iced::widget::text_input::State<<iced::Renderer as iced::advanced::text::Renderer>::Paragraph>;

    fn first_input(tree: &mut Tree) -> Option<&mut InputState> {
        if tree.tag == Tag::of::<InputState>() {
            return Some(tree.state.downcast_mut::<InputState>());
        }
        tree.children.iter_mut().find_map(first_input)
    }

    /// iced rebuilds a widget's state, focus included, when its place in
    /// the tree changes kind. Typing a name that repeats an earlier one
    /// flags the row, and the field being typed in must keep the keyboard.
    #[test]
    fn the_name_field_keeps_focus_when_it_becomes_a_duplicate() {
        let t = crate::ui::theme::Tokens::for_theme(0);
        let mut tree = Tree::new(variable_row(t, "env", 1, "bas", "x", false));
        first_input(&mut tree).expect("the name field").focus();

        tree.diff(variable_row(t, "env", 1, "base", "x", true));
        assert!(first_input(&mut tree).is_some_and(|input| input.is_focused()));
    }
}
