//! Dialogs that put something somewhere: Save request and New folder, on
//! one location picker of collections, folders and an inline new
//! collection.

use iced::widget::{Space, column, container, row, stack, text, text_input};
use iced::{Alignment, Background, Element, Length, Padding};

use crate::app::AppState;
use crate::domain::group::Group;
use crate::message::{FolderDialogMsg, Message, SaveDialogMsg, SidebarMsg};
use crate::state::sidebar::folder_name_input_id;
use crate::ui::components::Surface;
use crate::ui::components::{field, scroll};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_LG, CONTROL_XL, HAIRLINE, ICON_LG, ICON_MD, LINE_MD, LINE_SM, MONO, ROW_HEIGHT, SPACE_LG,
    SPACE_MD, SPACE_XL, SPACE_XS, TAB_INDICATOR, TEXT_CAPTION, TEXT_MD, TEXT_SM, Tokens, line,
};

use super::{Dialog, MEDIUM, cancel, confirm, label, panel, title};

/// One level of nesting in the location list.
const INDENT: f32 = ICON_LG + SPACE_XS;

/// What a location picker shows checked and what its rows send.
struct Picker<'a> {
    /// The checked collection or folder; ids are unique across both.
    checked: Option<&'a str>,
    /// The inline "New collection" name while that row is open.
    new_collection: Option<&'a str>,
    pick: fn(String) -> Message,
    toggle_new: Message,
    name_new: fn(String) -> Message,
    submit: Message,
}

pub(in crate::ui::layout) fn save_dialog(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let Some(dialog) = &state.save_dialog else {
        return Space::new().into();
    };
    let tab = state.tabs.tabs.iter().find(|t| t.id == dialog.tab_id).unwrap_or_else(|| state.tabs.active_tab());
    let method = tab.method.as_str();
    let subtitle = row![
        text(method).size(TEXT_CAPTION).font(MONO).color(t.on_raised.methods.for_method(method)),
        text(tab.url.as_str())
            .size(TEXT_SM)
            .line_height(line(LINE_MD))
            .font(MONO)
            .color(t.on_raised.text_muted)
            .wrapping(iced_core::text::Wrapping::None),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center);
    let send = Message::SaveDialog;
    let name = name_field(t, "Request name", &dialog.name)
        .id("save-dialog-name")
        .on_input(move |s| send(SaveDialogMsg::NameChanged(s)))
        .on_submit(send(SaveDialogMsg::Confirm));
    let picker = Picker {
        checked: dialog.new_collection.is_none().then_some(dialog.parent.as_deref()).flatten(),
        new_collection: dialog.new_collection.as_deref(),
        pick: |id| Message::SaveDialog(SaveDialogMsg::ParentSelected(id)),
        toggle_new: send(SaveDialogMsg::ToggleNewCollection),
        name_new: |s| Message::SaveDialog(SaveDialogMsg::NewCollectionNameChanged(s)),
        submit: send(SaveDialogMsg::Confirm),
    };
    let placed = dialog.new_collection.is_some() || dialog.parent.is_some();
    let body = with_hint(
        t,
        column![label(t, "Name"), name, Space::new().height(SPACE_XL), label(t, "Location"), locations(state, t, picker)],
        (!placed).then_some("Choose where to save the request"),
    );
    Dialog {
        width: MEDIUM,
        title: title(t, "Save request"),
        subtitle: Some(container(subtitle).padding(Padding { top: SPACE_XS, ..Padding::ZERO }).into()),
        body: body.into(),
        footer: vec![
            cancel(t, "Cancel", send(SaveDialogMsg::Close)),
            confirm(t, "Save", placed.then(|| send(SaveDialogMsg::Confirm))),
        ],
        dismiss: send(SaveDialogMsg::Close),
    }
    .view(t)
}

/// `body`, then `hint` under it when a location is still missing.
fn with_hint<'a>(t: &'a Tokens, body: iced::widget::Column<'a, Message>, hint: Option<&'static str>) -> iced::widget::Column<'a, Message> {
    match hint {
        Some(hint) => body.push(
            container(text(hint).size(TEXT_SM).line_height(line(LINE_SM)).color(t.on_raised.text_muted))
                .padding(Padding { top: SPACE_XS + 2.0, ..Padding::ZERO }),
        ),
        None => body,
    }
}

pub(in crate::ui::layout) fn folder_dialog(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let Some(dialog) = &state.sidebar.folder_dialog else {
        return Space::new().into();
    };
    let send = |msg| Message::Sidebar(SidebarMsg::FolderDialog(msg));
    let name = name_field(t, "New Folder", &dialog.name)
        .id(folder_name_input_id())
        .on_input(move |s| send(FolderDialogMsg::NameChanged(s)))
        .on_submit(send(FolderDialogMsg::Confirmed));
    let picker = Picker {
        checked: dialog.new_collection.is_none().then_some(dialog.parent.as_deref()).flatten(),
        new_collection: dialog.new_collection.as_deref(),
        pick: |id| Message::Sidebar(SidebarMsg::FolderDialog(FolderDialogMsg::ParentSelected(id))),
        toggle_new: send(FolderDialogMsg::NewCollectionToggled),
        name_new: |s| Message::Sidebar(SidebarMsg::FolderDialog(FolderDialogMsg::NewCollectionNameChanged(s))),
        submit: send(FolderDialogMsg::Confirmed),
    };
    let placed = dialog.new_collection.is_some() || dialog.parent.is_some();
    let body = with_hint(
        t,
        column![label(t, "Name"), name, Space::new().height(SPACE_XL), label(t, "Location"), locations(state, t, picker)],
        (!placed).then_some("Choose where to put the folder"),
    );
    Dialog {
        width: MEDIUM,
        title: title(t, "New folder"),
        subtitle: None,
        body: body.into(),
        footer: vec![
            cancel(t, "Cancel", send(FolderDialogMsg::Closed)),
            confirm(t, "Create", placed.then(|| send(FolderDialogMsg::Confirmed))),
        ],
        dismiss: send(FolderDialogMsg::Closed),
    }
    .view(t)
}

fn name_field<'a>(t: &'a Tokens, placeholder: &'a str, value: &'a str) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .size(TEXT_MD)
        .line_height(line(LINE_MD))
        .padding([(CONTROL_XL - LINE_MD) / 2.0, SPACE_LG])
        .style(move |_, s| field::field_on(t, s, Surface::Raised))
}

/// Collections and their folders as a short tree, then "New collection".
fn locations<'a>(state: &'a AppState, t: &'a Tokens, picker: Picker<'a>) -> Element<'a, Message> {
    let mut list = column![];
    for collection in &state.collections {
        let checked = picker.checked == Some(collection.id.as_str());
        list = list.push(location(t, &collection.name, 0, checked, (picker.pick)(collection.id.clone())));
        list = folders(state, t, &picker, list, &collection.id, true, 1);
    }
    let new: Element<'a, Message> = if let Some(typed) = picker.new_collection {
        let input = text_input("Collection name", typed)
            .on_input(picker.name_new)
            .on_submit(picker.submit)
            .size(TEXT_MD)
            .padding([SPACE_XS, SPACE_MD])
            .style(move |_, s| field::field_on(t, s, Surface::Raised));
        container(row![icon(icons::folder_plus(), t), input].spacing(SPACE_MD).align_y(Alignment::Center))
            .height(CONTROL_LG)
            .align_y(Alignment::Center)
            .padding([0.0, SPACE_LG])
            .into()
    } else {
        location_button(
            t,
            row![
                icon(icons::folder_plus(), t),
                text("New collection").size(TEXT_MD).color(t.on_raised.text_muted),
            ]
            .spacing(SPACE_MD)
            .align_y(Alignment::Center),
            0,
            false,
            CONTROL_LG,
            picker.toggle_new,
        )
    };
    let mut content = column![];
    if !state.collections.is_empty() {
        let divider = container(Space::new().width(Length::Fill).height(HAIRLINE)).style(move |_| container::Style {
            background: Some(Background::Color(t.on_raised.border)),
            ..Default::default()
        });
        content = content.push(scroll::vertical(t, list).height(Length::Shrink)).push(Space::new().height(SPACE_XS)).push(divider);
    }
    panel(t, content.push(new)).padding([SPACE_XS + HAIRLINE, HAIRLINE]).into()
}

/// The folders under `parent`, sorted as in the sidebar, each followed by
/// its own folders.
fn folders<'a>(
    state: &'a AppState,
    t: &'a Tokens,
    picker: &Picker<'a>,
    mut list: iced::widget::Column<'a, Message>,
    parent: &str,
    parent_is_collection: bool,
    depth: usize,
) -> iced::widget::Column<'a, Message> {
    let mut children: Vec<&'a Group> = state
        .groups
        .iter()
        .filter(|g| g.parent_id == parent && g.parent_is_collection == parent_is_collection)
        .collect();
    children.sort_by_key(|g| g.sort_order);
    for group in children {
        let checked = picker.checked == Some(group.id.as_str());
        list = list.push(location(t, &group.name, depth, checked, (picker.pick)(group.id.clone())));
        list = folders(state, t, picker, list, &group.id, false, depth + 1);
    }
    list
}

/// A row's leading icon, centered in the 16 px box the rows align on.
fn icon<'a>(glyph: iced::widget::Text<'a>, t: &'a Tokens) -> Element<'a, Message> {
    container(glyph.size(ICON_MD).color(t.on_raised.text_muted)).center_x(ICON_LG).into()
}

fn location<'a>(t: &'a Tokens, name: &'a str, depth: usize, checked: bool, on_press: Message) -> Element<'a, Message> {
    let mut content = row![
        icon(icons::folder(), t),
        text(name).size(TEXT_MD).color(t.text).width(Length::Fill),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center);
    if checked {
        content = content.push(container(icons::check().size(ICON_MD).color(t.accent_text)).center_x(ICON_LG));
    }
    location_button(t, content, depth, checked, ROW_HEIGHT, on_press)
}

fn location_button<'a>(
    t: &'a Tokens,
    content: impl Into<Element<'a, Message>>,
    depth: usize,
    checked: bool,
    height: f32,
    on_press: Message,
) -> Element<'a, Message> {
    let indent = SPACE_LG + depth as f32 * INDENT;
    let padded = container(content).height(Length::Fill).align_y(Alignment::Center).padding(Padding {
        left: indent,
        right: SPACE_LG,
        ..Padding::ZERO
    });
    let inner: Element<'a, Message> = if checked {
        let bar = container(Space::new().width(TAB_INDICATOR).height(Length::Fill)).style(move |_| container::Style {
            background: Some(Background::Color(t.accent)),
            ..Default::default()
        });
        stack![padded, bar].into()
    } else {
        padded.into()
    };
    iced::widget::button(inner)
        .on_press(on_press)
        .width(Length::Fill)
        .height(height)
        .padding(0)
        .style(move |_, status| {
            use iced::widget::button::Status;
            let hovered = matches!(status, Status::Hovered | Status::Pressed);
            iced::widget::button::Style {
                background: if checked {
                    Some(Background::Color(t.on_raised.selected))
                } else {
                    hovered.then_some(Background::Color(t.on_raised.hover))
                },
                text_color: t.text,
                ..Default::default()
            }
        })
        .into()
}
