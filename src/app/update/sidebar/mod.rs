mod delete;
mod environments;
mod folder_dialog;
mod history;
mod tree;

use iced::Task;
use rustman_core::store::HistoryStore;

use crate::app::AppState;
use crate::message::{GitMsg, Message, SidebarMsg, SidebarPanel};

pub(super) use tree::{NEW_COLLECTION, new_collection};
pub(in crate::app) use tree::finish_rename;

pub(super) fn handle(state: &mut AppState, msg: SidebarMsg) -> Task<Message> {
    match msg {
        SidebarMsg::ToggleCollapsed => state.sidebar.collapsed = !state.sidebar.collapsed,
        SidebarMsg::PanelSelected(panel) => return select_panel(state, panel),
        SidebarMsg::HistoryEntryOpened(id) => return history::open(state, id),
        SidebarMsg::HistoryRequestLoaded(result) => history::loaded(state, result),
        SidebarMsg::FolderDialog(msg) => return folder_dialog::handle(state, msg),
        SidebarMsg::ClearHistory => {
            state.history.clear();
            if let Some(store) = &state.store {
                store.write_with(HistoryStore::clear_history, |e| Message::App(crate::message::AppMsg::StoreFailed(e)));
            }
        }
        SidebarMsg::DeleteAsked(_)
        | SidebarMsg::DeleteConfirmed
        | SidebarMsg::DeleteCancelled
        | SidebarMsg::DeleteRequest { .. }
        | SidebarMsg::EnvironmentDeleted(_)
        | SidebarMsg::UndoDelete
        | SidebarMsg::UndoExpired(_) => return delete::handle(state, msg),
        SidebarMsg::EnvironmentSelected(_)
        | SidebarMsg::EnvironmentCreated
        | SidebarMsg::EnvironmentToggleEdit(_)
        | SidebarMsg::EnvironmentVarAdded(_)
        | SidebarMsg::EnvironmentVarKeyChanged(..)
        | SidebarMsg::EnvironmentVarValueChanged(..)
        | SidebarMsg::EnvironmentVarRemoved(..) => environments::handle(state, msg),
        tree_msg => return tree::handle(state, tree_msg),
    }
    Task::none()
}

/// Selecting the open panel again collapses the sidebar.
fn select_panel(state: &mut AppState, panel: SidebarPanel) -> Task<Message> {
    if state.sidebar.panel == panel && !state.sidebar.collapsed {
        state.sidebar.collapsed = true;
        return Task::none();
    }
    let refresh_git = panel == SidebarPanel::Git;
    state.sidebar.panel = panel;
    state.sidebar.collapsed = false;
    if refresh_git {
        Task::done(Message::Git(GitMsg::Refresh))
    } else {
        Task::none()
    }
}

#[cfg(test)]
mod open_bench;
