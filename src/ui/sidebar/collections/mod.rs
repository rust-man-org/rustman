//! The collections tree: header with the "New" menu, then one row per
//! visible collection, folder and request.

mod cache;
mod drag_row;
mod menu;
pub mod model;
mod row;

pub use cache::TreeCache;
pub use row::METHOD_COLUMN;

use iced::widget::{Space, column, container, row as hrow, text};
use iced::{Alignment, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{AppMsg, ImportMsg, MenuId, Message, SidebarMsg};
use crate::ui::components::button::{self, Size, Variant, button_with_icon};
use crate::ui::components::{hairline, menu::{self as menus, Align}, scroll};
use crate::ui::icons;
use crate::ui::theme::{
    LINE_MD, LINE_SM, line,
    HAIRLINE, ICON_SM, SPACE_LG, SPACE_MD, SPACE_XL, SPACE_XS, TAB_STRIP_HEIGHT, TEXT_LG, TEXT_MD,
    TEXT_SM, Tokens, UI_FONT_MEDIUM,
};

use row::{Context, Drag};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let body: Element<'_, Message> = if state.collections.is_empty() {
        empty_state(t)
    } else {
        tree(state, t)
    };
    super::undo::layer(state, column![header(state, t), hairline(t), body].height(Length::Fill).into())
}

fn header<'a>(state: &AppState, t: &'static Tokens) -> Element<'a, Message> {
    let open = state.chrome.open_menu == Some(MenuId::SidebarNew);
    let label = hrow![
        text("New").size(TEXT_SM).font(UI_FONT_MEDIUM),
        icons::chevron_down().size(ICON_SM).color(t.text_muted),
    ]
    .spacing(SPACE_XS)
    .align_y(Alignment::Center);
    let trigger = iced::widget::button(container(label).center_y(Length::Fill))
        .on_press(Message::App(AppMsg::MenuToggled(MenuId::SidebarNew)))
        .height(Size::Regular.height())
        .padding(Padding {
            left: SPACE_LG + HAIRLINE,
            right: SPACE_MD + HAIRLINE,
            ..Padding::ZERO
        })
        .style(move |_, status| {
            let status = if open { iced::widget::button::Status::Pressed } else { status };
            button::style(t, Variant::Secondary, status)
        });
    let panel = open.then(|| menu::new_menu(t));
    let new = menus::popover(trigger, panel, Align::Start, Message::App(AppMsg::MenuDismissed));
    container(
        hrow![
            text("Collections").size(TEXT_LG).font(UI_FONT_MEDIUM).color(t.text),
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

fn tree<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let mut cache = state.sidebar.tree_cache.borrow_mut();
    let lines = cache.sync(state);
    let cx = Context {
        t,
        selected: selected_request(state),
        renaming: state.sidebar.renaming.as_ref(),
        menu: state.sidebar.context_menu.as_deref(),
        drag: state.sidebar.drag.as_ref().and_then(|d| Drag::find(d, lines)),
    };
    let rows = lines.iter().enumerate().map(|(i, line)| row::view(&cx, i, line));
    scroll::vertical(t, column(rows).padding([SPACE_XS, 0.0])).height(Length::Fill).into()
}

/// The request open in the active tab: the tree highlights where you are.
fn selected_request(state: &AppState) -> Option<&str> {
    state.tabs.active_tab().saved_as.as_ref().map(|(_, id)| id.as_str())
}

fn empty_state(t: &'static Tokens) -> Element<'static, Message> {
    let create = button_with_icon(
        t,
        icons::folder_plus(),
        "Create a collection",
        Variant::Secondary,
        Size::Regular,
    )
    .on_press(Message::Sidebar(SidebarMsg::NewCollection));
    let import = button_with_icon(t, icons::import(), "Import", Variant::Ghost, Size::Regular)
        .on_press(Message::Import(ImportMsg::ImportCollectionFile));
    container(
        column![
            text("No collections yet").size(TEXT_MD).line_height(line(LINE_MD)).font(UI_FONT_MEDIUM).color(t.text),
            text("Collections group related requests. Save a request into one, or import from Postman or OpenAPI.")
                .size(TEXT_SM)
                .line_height(line(LINE_SM))
                .color(t.text_muted),
            Space::new().height(SPACE_XS),
            hrow![create, import].spacing(SPACE_MD),
        ]
        .spacing(SPACE_XS),
    )
    .padding([SPACE_XL, SPACE_LG])
    .into()
}
