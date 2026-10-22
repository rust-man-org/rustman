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
#[cfg(feature = "self-update")]
mod updater;
mod ws;

pub(crate) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let dirty = marks_session_dirty(&message);
    let task = dispatch(state, message);

    if dirty {
        state.session_dirty = true;
    }
    task
}

/// Whether `message` changes persisted session state. Transient UI (autosave
/// ticks, spinner frames, read-only viewer navigation) must not schedule a
/// save: saving serializes every tab, so marking a selection scroll dirty
/// would rewrite a multi-megabyte form value for no reason — and
/// `persist_session` clears the flag, so false positives also keep the
/// autosave subscription alive instead of letting an idle app go quiet.
/// Expanded editors DO dirty on content edits (their write-back mutates the
/// snapshot); navigation/selection/search inside them does not.
pub(crate) fn marks_session_dirty(message: &Message) -> bool {
    if let Message::Request(crate::message::RequestMsg::FormValueEdited(_, editor_msg)) = message {
        return crate::state::tabs::editor_message_edits_content(editor_msg);
    }
    !matches!(
        message,
        Message::App(
            crate::message::AppMsg::AutoSaveSession
                | crate::message::AppMsg::Noop
                | crate::message::AppMsg::SpinnerTick
        )
        // Expanded editors are transient UI: paging or expanding never
        // touches the persisted snapshot. A rejected oversized paste changes
        // nothing either — the old URL is kept. The import dialog chrome is
        // likewise UI-only until a parse lands.
        | Message::Request(
            crate::message::RequestMsg::FormValueExpandToggled(_)
                | crate::message::RequestMsg::FormValuePagePrev(_)
                | crate::message::RequestMsg::FormValuePageNext(_)
                | crate::message::RequestMsg::OversizedUrlRejected(_)
                | crate::message::RequestMsg::OpenImportDialog
                | crate::message::RequestMsg::CloseImportDialog
                | crate::message::RequestMsg::ImportFromClipboard
                | crate::message::RequestMsg::ImportFromFile
        )
        // Saving or opening a response body only writes a file and a
        // transient status line; nothing in the persisted session snapshot
        // changes.
        | Message::Response(
            crate::message::ResponseMsg::SaveBody
                | crate::message::ResponseMsg::BodySaved(_)
                | crate::message::ResponseMsg::OpenBodyInBrowser
                | crate::message::ResponseMsg::BodyOpened(_)
        )
    )
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
        #[cfg(feature = "self-update")]
        Message::Update(msg) => updater::handle(state, msg),
    }
}

fn handle_layout(state: &mut AppState, msg: LayoutMsg) -> Task<Message> {
    match msg {
        LayoutMsg::ZoomIn => {
            state.ui_scale = (((state.ui_scale + 0.1) * 10.0).round() / 10.0).min(2.0);
        }
        LayoutMsg::ZoomOut => {
            state.ui_scale = (((state.ui_scale - 0.1) * 10.0).round() / 10.0).max(0.5);
        }
        LayoutMsg::ZoomReset => state.ui_scale = 1.0,
        LayoutMsg::SetZoom(scale) => state.ui_scale = scale.clamp(0.5, 2.0),
        LayoutMsg::PanelSplitChanged(split) => {
            if split >= 1 && split <= 99 {
                state.panel_split = split;
            }
        }
        LayoutMsg::SidebarWidthChanged(width) => {
            state.sidebar_width = width.clamp(160.0, 600.0);
        }
    }
    Task::none()
}

#[cfg(test)]
mod dirty_tests {
    use super::*;
    use crate::message::RequestMsg;

    #[test]
    fn viewer_navigation_never_dirties_but_edits_do() {
        use iced_code_editor::Message as EditorMessage;

        assert!(!marks_session_dirty(&Message::Request(
            RequestMsg::FormValueExpandToggled(0)
        )));
        assert!(!marks_session_dirty(&Message::Request(
            RequestMsg::FormValuePageNext(0)
        )));
        assert!(!marks_session_dirty(&Message::Request(
            RequestMsg::FormValueEdited(0, EditorMessage::SelectAll)
        )));
        assert!(marks_session_dirty(&Message::Request(
            RequestMsg::FormValueEdited(0, EditorMessage::CharacterInput('x'))
        )));
        assert!(marks_session_dirty(&Message::Request(
            RequestMsg::UrlChanged("https://x.test".to_owned())
        )));
    }
}
