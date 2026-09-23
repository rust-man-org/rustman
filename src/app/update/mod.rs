use iced::Task;

use crate::message::{LayoutMsg, Message};

use super::AppState;

mod app_msg;
mod git;
mod import;
mod palette;
mod request;
mod response;
mod save_dialog;
mod settings;
mod sidebar;
mod updater;
mod ws;

pub(crate) fn update(state: &mut AppState, message: Message) -> Task<Message> {

    let marks_dirty = !matches!(
        message,
        Message::App(
            crate::message::AppMsg::AutoSaveSession
                | crate::message::AppMsg::Noop
                | crate::message::AppMsg::SpinnerTick
        )
    );

    let task = dispatch(state, message);

    if marks_dirty {
        state.session_dirty = true;
    }
    task
}

fn dispatch(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::Sidebar(msg) => sidebar::handle(state, msg),
        Message::Request(msg) => request::handle(state, msg),
        Message::Response(msg) => response::handle(state, msg),
        Message::WebSocket(msg) => ws::handle(state, msg),
        Message::Palette(msg) => palette::handle(state, msg),
        Message::SaveDialog(msg) => save_dialog::handle(state, msg),
        Message::Git(msg) => git::handle(state, msg),
        Message::Import(msg) => import::handle(state, msg),
        Message::App(msg) => app_msg::handle(state, msg),
        Message::Settings(msg) => settings::handle(state, msg),
        Message::Layout(msg) => handle_layout(state, msg),
        Message::Update(msg) => updater::handle(state, msg),
    }
}

fn handle_layout(state: &mut AppState, msg: LayoutMsg) -> Task<Message> {
    match msg {
        LayoutMsg::ZoomIn => state.ui_scale = (state.ui_scale + 0.1).min(2.0),
        LayoutMsg::ZoomOut => state.ui_scale = (state.ui_scale - 0.1).max(0.5),
        LayoutMsg::ZoomReset => state.ui_scale = 1.0,
        LayoutMsg::PanelSplitChanged(split) => {
            if split >= 1 && split <= 99 {
                state.panel_split = split;
            }
        }
    }
    Task::none()
}
