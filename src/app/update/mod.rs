use iced::Task;

use crate::message::{LayoutMsg, Message};

use super::AppState;

mod app_msg;
mod auth;
mod drafts;
mod environment_cell;
mod examples;
mod git;
mod import;
mod meta;
mod palette;
mod request;
mod response;
mod review;
mod save_dialog;
mod secrets;
mod settings;
mod sidebar;
#[cfg(test)]
mod sidebar_environments_tests;
#[cfg(test)]
mod sidebar_feedback_tests;
mod sidebar_search;
#[cfg(feature = "self-update")]
mod updater;
mod ws;

pub(crate) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let dirty = marks_session_dirty(&message);
    let prune_examples = matches!(
        &message,
        Message::Sidebar(
            crate::message::SidebarMsg::DeleteRequest { .. }
                | crate::message::SidebarMsg::CollectionDeleteConfirmed
                | crate::message::SidebarMsg::DeleteGroup(_)
        )
    );
    // Requests reach the active tab through many paths (row release, drafts,
    // search, palette, restore…), so refresh after everything except the
    // high-frequency noise that cannot change the request.
    let refresh_snippet = state.snippet_open
        && !matches!(
            &message,
            Message::Response(_)
                | Message::Layout(
                    LayoutMsg::SnippetWidthChanged(_)
                        | LayoutMsg::SidebarWidthChanged(_)
                        | LayoutMsg::PanelSplitChanged(_)
                        | LayoutMsg::SnippetCopied
                )
                | Message::Sidebar(
                    crate::message::SidebarMsg::ItemDragOver(_)
                        | crate::message::SidebarMsg::RowHoverEnd(_)
                        | crate::message::SidebarMsg::FilterChanged(_)
                        | crate::message::SidebarMsg::MenuFilterChanged(_)
                        | crate::message::SidebarMsg::RecentlyClosedToggled
                        | crate::message::SidebarMsg::ContextMenuOpened(_)
                        | crate::message::SidebarMsg::ContextMenuClosed
                        | crate::message::SidebarMsg::EnvironmentVariableHover(..)
                )
                | Message::Request(crate::message::RequestMsg::TabDragOver(_))
                | Message::Palette(crate::message::PaletteMsg::Frame(_))
        );
    let task = dispatch(state, message);
    if refresh_snippet {
        crate::app::snippet::refresh(state);
    }
    if prune_examples {
        let ids: std::collections::HashSet<_> = state
            .requests
            .values()
            .flatten()
            .map(|r| r.id.as_str())
            .collect();
        state
            .examples
            .items
            .retain(|id, _| ids.contains(id.as_str()));
    }

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
    if matches!(
        message,
        Message::Sidebar(
            crate::message::SidebarMsg::EnvironmentCellOpened { .. }
                | crate::message::SidebarMsg::EnvironmentCellEdited { .. }
                | crate::message::SidebarMsg::EnvironmentCellPageChanged(_)
                | crate::message::SidebarMsg::EnvironmentCellClosed
        )
    ) {
        return false;
    }
    if matches!(
        message,
        Message::Layout(LayoutMsg::ConsoleToggled | LayoutMsg::SnippetCopied)
            | Message::Sidebar(crate::message::SidebarMsg::EnvironmentPageClosed)
            | Message::App(crate::message::AppMsg::OpenTerminal)
            | Message::Response(crate::message::ResponseMsg::ConsoleEdited(_))
    ) {
        return false;
    }
    if matches!(message, Message::Example(_)) {
        return false;
    }
    // Company-edition messages decide for themselves (contract C11).
    match message {
        Message::Draft(m) => return m.marks_session_dirty(),
        Message::Meta(m) => return m.marks_session_dirty(),
        Message::Auth(m) => return m.marks_session_dirty(),
        Message::Secrets(m) => return m.marks_session_dirty(),
        Message::Review(m) => return m.marks_session_dirty(),
        _ => {}
    }
    if let Message::Request(crate::message::RequestMsg::FormValueEdited(_, editor_msg)) = message {
        return crate::state::tabs::editor_message_edits_content(editor_msg);
    }
    !matches!(
        message,
        Message::App(
            crate::message::AppMsg::AutoSaveSession
                | crate::message::AppMsg::ToastDismissed(_)
                | crate::message::AppMsg::Noop
                | crate::message::AppMsg::SpinnerTick
        )
        | Message::Palette(crate::message::PaletteMsg::Frame(_))
        // Expanded editors are transient UI: paging or expanding never
        // touches the persisted snapshot. A rejected oversized paste changes
        // nothing either — the old URL is kept. The import dialog chrome is
        // likewise UI-only until a parse lands.
        // The tree search box is view state; nothing persisted changes.
        | Message::Sidebar(
            crate::message::SidebarMsg::FilterChanged(_)
                | crate::message::SidebarMsg::MenuFilterChanged(_)
                | crate::message::SidebarMsg::RecentlyClosedToggled
                | crate::message::SidebarMsg::CollectionsSectionToggled
                | crate::message::SidebarMsg::EnvironmentsSectionToggled
                | crate::message::SidebarMsg::CollectionDeleteTextChanged(_)
                | crate::message::SidebarMsg::CollectionDeleteCancelled
                | crate::message::SidebarMsg::DeleteCollection(_)
                | crate::message::SidebarMsg::SearchMove(_)
                | crate::message::SidebarMsg::SearchClose
                | crate::message::SidebarMsg::RowHoverEnd(_)
                | crate::message::SidebarMsg::EnvironmentVariableHover(_, _, _)
                // Hover; a drag's new order is persisted on ItemDragEnd.
                | crate::message::SidebarMsg::ItemDragOver(_)
        )
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
    if (state.sidebar.collection_delete_id.is_some() || state.sidebar.request_delete.is_some())
        && matches!(&message, Message::Request(_) | Message::Palette(_))
    {
        return Task::none();
    }
    if blocks_example_exit(state, &message) {
        return Task::none();
    }
    if state.sidebar.environment_page.is_some() {
        if let Message::Request(msg) = &message {
            use crate::message::RequestMsg;
            match msg {
                RequestMsg::CloseCurrentTab => {
                    state.close_environment_workspace();
                    return Task::none();
                }
                RequestMsg::SwitchTab(_)
                | RequestMsg::NewTab
                | RequestMsg::NewWsTab
                | RequestMsg::CloseTab(_) => {
                    state.close_environment_workspace();
                }
                _ => return Task::none(),
            }
        }
        if matches!(
            &message,
            Message::Draft(crate::message::draft::DraftMsg::Open(_))
        ) {
            state.close_environment_workspace();
        }
    }
    if state.examples.editor.is_some() {
        if let Message::Request(msg) = &message {
            use crate::message::RequestMsg;
            match msg {
                RequestMsg::SaveRequest => {
                    return examples::handle(state, crate::message::example::ExampleMsg::Save);
                }
                RequestMsg::Send => {
                    state.status_message =
                        Some("Examples are edited without sending HTTP requests.".into());
                    return Task::none();
                }
                RequestMsg::SwitchTab(_)
                | RequestMsg::NewTab
                | RequestMsg::NewWsTab
                | RequestMsg::CloseTab(_)
                | RequestMsg::CloseCurrentTab => {
                    if examples::protect(state) {
                        return Task::none();
                    }
                    state.examples.editor = None;
                }
                _ => return Task::none(),
            }
        }
        if matches!(
            &message,
            Message::Sidebar(
                crate::message::SidebarMsg::RequestOpened(_)
                    | crate::message::SidebarMsg::EnvironmentOpened(_)
                    | crate::message::SidebarMsg::HistoryEntryOpened(_)
                    | crate::message::SidebarMsg::DeleteRequest { .. }
                    | crate::message::SidebarMsg::DeleteCollection(_)
                    | crate::message::SidebarMsg::DeleteGroup(_)
            )
        ) {
            if examples::protect(state) {
                return Task::none();
            }
            state.examples.editor = None;
        }
        if matches!(
            &message,
            Message::Draft(crate::message::draft::DraftMsg::Open(_))
        ) {
            if examples::protect(state) {
                return Task::none();
            }
            state.examples.editor = None;
        }
    }
    match message {
        Message::Example(msg) => examples::handle(state, msg),
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
        Message::Draft(msg) => drafts::handle(state, msg),
        Message::Meta(msg) => meta::handle(state, msg),
        Message::Auth(msg) => auth::handle(state, msg),
        Message::Secrets(msg) => secrets::handle(state, msg),
        Message::Review(msg) => review::handle(state, msg),
    }
}

fn blocks_example_exit(state: &mut AppState, message: &Message) -> bool {
    let exit = matches!(
        message,
        Message::App(crate::message::AppMsg::WindowCloseRequested(_))
    );
    #[cfg(feature = "self-update")]
    let exit = exit || matches!(message, Message::Update(crate::message::UpdateMsg::Restart));
    exit && examples::protect(state)
}

fn handle_layout(state: &mut AppState, msg: LayoutMsg) -> Task<Message> {
    match msg {
        LayoutMsg::ConsoleToggled => state.console_open = !state.console_open,
        LayoutMsg::ZoomIn => state.ui_scale = (state.ui_scale + 0.1).min(2.0),
        LayoutMsg::ZoomOut => state.ui_scale = (state.ui_scale - 0.1).max(0.5),
        LayoutMsg::ZoomReset => state.ui_scale = 1.0,
        LayoutMsg::PanelSplitChanged(split) => {
            if split >= 1 && split <= 99 {
                state.panel_split = split;
            }
        }
        LayoutMsg::SidebarWidthChanged(width) => {
            state.sidebar_width = width.clamp(160.0, 600.0);
        }
        LayoutMsg::SnippetToggled => {
            state.snippet_open = !state.snippet_open;
            crate::app::snippet::refresh(state);
        }
        LayoutMsg::SnippetWidthChanged(width) => {
            state.snippet_width = width.clamp(crate::app::snippet::MIN_W, crate::app::snippet::MAX_W);
        }
        LayoutMsg::SnippetLangChanged(lang) => {
            state.snippet_lang = lang;
            crate::app::snippet::refresh(state);
        }
        LayoutMsg::SnippetCopied => {
            return match crate::app::snippet::code(state.tabs.active_tab(), state.snippet_lang) {
                Ok(cmd) => {
                    state.status_message = Some("cURL command copied!".to_owned());
                    iced::clipboard::write::<Message>(cmd)
                }
                Err(err) => {
                    state.status_message = Some(err);
                    Task::none()
                }
            };
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
