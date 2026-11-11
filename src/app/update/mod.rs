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
mod window;

pub(crate) use git::branch_task as git_branch_task;
pub(in crate::app) use sidebar::finish_rename;
pub(crate) use git::default_identity_task as git_default_identity_task;
pub(crate) use window::decorations_toggle as window_decorations_toggle;
mod ws;

pub(crate) fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let message = shift_click_extends_selection(state.keyboard_modifiers, message);
    let dirty = marks_session_dirty(&message);
    let before = (state.tabs.active, state.tabs.tabs.len());
    let task = dispatch(state, message);
    crate::ui::layout::sync_strip_labels(&mut state.tabs);
    let task = if before == (state.tabs.active, state.tabs.tabs.len()) {
        task
    } else {
        Task::batch([task, crate::ui::layout::reveal_active_tab(state)])
    };

    if dirty {
        state.session_dirty = true;
        state.session_changed_at = std::time::Instant::now();
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
    // The code editors publish a message for every pointer move over them
    // (hover tracking) and for scrolls and focus changes. None of those
    // change content; counting them as edits kept the 3 s autosave — a full
    // re-serialisation of every tab on the UI thread — firing for as long as
    // the mouse moved over a body editor.
    if let Message::Request(
        crate::message::RequestMsg::BodyEdited(editor_msg)
        | crate::message::RequestMsg::GraphQLVariablesEdited(editor_msg)
        | crate::message::RequestMsg::PreRequestScriptEdited(editor_msg)
        | crate::message::RequestMsg::TestScriptEdited(editor_msg),
    ) = message
        && editor_message_is_pointer_only(editor_msg)
    {
        return false;
    }
    !matches!(
        message,
        Message::App(
            crate::message::AppMsg::AutoSaveSession
                | crate::message::AppMsg::WindowCloseRequested(_)
                | crate::message::AppMsg::Noop
                | crate::message::AppMsg::SpinnerTick
                | crate::message::AppMsg::MenuToggled(_)
                | crate::message::AppMsg::MenuDismissed
                | crate::message::AppMsg::TabStripScrolled { .. }
        )
        | Message::Window(
            crate::message::WindowMsg::Drag
                | crate::message::WindowMsg::DragResize(_)
                | crate::message::WindowMsg::ToggleMaximize
                | crate::message::WindowMsg::Minimize
                | crate::message::WindowMsg::Close
                | crate::message::WindowMsg::Resized(_)
                | crate::message::WindowMsg::MaximizedChanged(_)
        )
        | Message::Response(crate::message::ResponseMsg::FailureDetailsToggled)
        | Message::Git(crate::message::GitMsg::BranchLoaded { .. })
        | Message::ModifiersChanged(_)
        // Pressing, dragging over and right-clicking tree rows is transient
        // UI; only the outcome (a click that opens a tab, a drop that moves
        // a row — both via ItemDragEnd) touches anything persisted.
        | Message::Sidebar(
            crate::message::SidebarMsg::ItemPressed(_)
                | crate::message::SidebarMsg::ItemDragStarted(_)
                | crate::message::SidebarMsg::ItemDragHover { .. }
                | crate::message::SidebarMsg::ItemDragLeave(_)
                | crate::message::SidebarMsg::ItemDragCancel
                | crate::message::SidebarMsg::ItemDragStray { pressed: true }
                | crate::message::SidebarMsg::ContextMenuOpened(_)
                | crate::message::SidebarMsg::ContextMenuClosed
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
        // changes. The body find bar is likewise viewer-only state.
        | Message::Response(
            crate::message::ResponseMsg::SaveBody
                | crate::message::ResponseMsg::BodySaved(_)
                | crate::message::ResponseMsg::OpenBodyInBrowser
                | crate::message::ResponseMsg::BodyOpened(_)
                | crate::message::ResponseMsg::SearchOpen
                | crate::message::ResponseMsg::SearchClose
                | crate::message::ResponseMsg::SearchQueryChanged(_)
                | crate::message::ResponseMsg::SearchToggleCase
                | crate::message::ResponseMsg::SearchNext
                | crate::message::ResponseMsg::SearchPrev
        )
    )
}

/// Editor messages that only track the pointer, scroll or focus — never an
/// edit. Deliberately an allow-list of known-inert variants: anything not
/// listed (including variants added later) still counts as a possible edit.
fn editor_message_is_pointer_only(message: &iced_code_editor::Message) -> bool {
    use iced_code_editor::Message as EditorMessage;
    matches!(
        message,
        EditorMessage::MouseHover(_)
            | EditorMessage::MouseDrag(_)
            | EditorMessage::MouseRelease
            | EditorMessage::Scrolled(_)
            | EditorMessage::HorizontalScrolled(_)
            | EditorMessage::CanvasFocusGained
            | EditorMessage::CanvasFocusLost
            | EditorMessage::ContextMenuRequested(_)
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
        Message::Window(msg) => window::handle(state, msg),
        #[cfg(feature = "self-update")]
        Message::Update(msg) => updater::handle(state, msg),
        Message::ModifiersChanged(modifiers) => {
            state.keyboard_modifiers = modifiers;
            Task::none()
        }
    }
}

/// Shift+click in a plain `text_editor` extends the selection (issue #68).
///
/// iced's `text_editor` turns every single click into `Action::Click`, which
/// moves the caret and drops the selection — it never looks at Shift. A
/// `Drag` to the same point is exactly "extend from the anchor" (it keeps an
/// existing selection's anchor, or anchors at the caret), and holding the
/// button afterwards keeps extending, so Shift turns the click into that.
/// The code editors (request body, response viewer) handle Shift+click
/// themselves in the iced-code-editor fork.
fn shift_click_extends_selection(modifiers: iced::keyboard::Modifiers, message: Message) -> Message {
    use crate::message::{RequestMsg, ResponseMsg, SettingsMsg};
    use iced::widget::text_editor::Action;

    if !modifiers.shift() {
        return message;
    }
    let extend = |action: Action| match action {
        Action::Click(point) => Action::Drag(point),
        other => other,
    };
    match message {
        Message::Request(RequestMsg::UrlEdited(a)) => {
            Message::Request(RequestMsg::UrlEdited(extend(a)))
        }
        Message::Request(RequestMsg::HeadersBulkEdited(a)) => {
            Message::Request(RequestMsg::HeadersBulkEdited(extend(a)))
        }
        Message::Request(RequestMsg::ParamsBulkEdited(a)) => {
            Message::Request(RequestMsg::ParamsBulkEdited(extend(a)))
        }
        Message::Response(ResponseMsg::ConsoleEdited(a)) => {
            Message::Response(ResponseMsg::ConsoleEdited(extend(a)))
        }
        Message::Settings(SettingsMsg::GlobalPreRequestScriptEdited(a)) => {
            Message::Settings(SettingsMsg::GlobalPreRequestScriptEdited(extend(a)))
        }
        Message::Settings(SettingsMsg::GlobalTestScriptEdited(a)) => {
            Message::Settings(SettingsMsg::GlobalTestScriptEdited(extend(a)))
        }
        other => other,
    }
}

fn handle_layout(state: &mut AppState, msg: LayoutMsg) -> Task<Message> {
    match msg {
        LayoutMsg::ZoomIn => state.ui_scale = (state.ui_scale + 0.1).min(2.0),
        LayoutMsg::ZoomOut => state.ui_scale = (state.ui_scale - 0.1).max(0.5),
        LayoutMsg::ZoomReset => state.ui_scale = 1.0,
        LayoutMsg::SidebarWidthChanged(width) => {
            state.sidebar_width = width.clamp(160.0, 600.0);
        }
        LayoutMsg::RequestPaneResized(px) => {
            let prefs = &mut state.window_prefs;
            if state.horizontal_layout {
                prefs.request_width = Some(px);
            } else {
                prefs.request_height = Some(px);
            }
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

    #[test]
    fn shift_click_in_a_text_editor_extends_the_selection() {
        use iced::keyboard::Modifiers;
        use iced::widget::text_editor::{Action, Content};

        let mut content = Content::<iced::Renderer>::with_text("https://example.com/users");
        let click = |x: f32| Action::Click(iced::Point::new(x, 5.0));
        let as_action = |message: Message| match message {
            Message::Request(RequestMsg::UrlEdited(a)) => a,
            other => panic!("unexpected {other:?}"),
        };

        // A plain click stays a click; with Shift it becomes a drag.
        let plain = shift_click_extends_selection(
            Modifiers::empty(),
            Message::Request(RequestMsg::UrlEdited(click(10.0))),
        );
        assert!(matches!(as_action(plain), Action::Click(_)));

        content.perform(Action::Move(iced::widget::text_editor::Motion::Home));
        let shifted = shift_click_extends_selection(
            Modifiers::SHIFT,
            Message::Request(RequestMsg::UrlEdited(click(1.0e6))),
        );
        let action = as_action(shifted);
        assert!(matches!(action, Action::Drag(_)));
        content.perform(action);
        assert_eq!(
            content.selection().as_deref(),
            Some("https://example.com/users"),
            "Shift+click past the end selects from the caret to the end"
        );

        // Unrelated messages pass through untouched.
        assert!(matches!(
            shift_click_extends_selection(Modifiers::SHIFT, Message::Request(RequestMsg::NewTab)),
            Message::Request(RequestMsg::NewTab)
        ));
    }

    #[test]
    fn pointer_tracking_and_tree_gestures_never_dirty() {
        use crate::message::SidebarMsg;
        use iced_code_editor::Message as EditorMessage;

        let hover = EditorMessage::MouseHover(iced::Point::new(4.0, 4.0));
        assert!(!marks_session_dirty(&Message::Request(
            RequestMsg::BodyEdited(hover.clone())
        )));
        assert!(!marks_session_dirty(&Message::Request(
            RequestMsg::TestScriptEdited(hover)
        )));
        assert!(marks_session_dirty(&Message::Request(
            RequestMsg::BodyEdited(EditorMessage::CharacterInput('x'))
        )));

        for msg in [
            SidebarMsg::ItemPressed("r".into()),
            SidebarMsg::ItemDragStarted("r".into()),
            SidebarMsg::ItemDragLeave("r".into()),
            SidebarMsg::ItemDragCancel,
            SidebarMsg::ContextMenuOpened("r".into()),
            SidebarMsg::ContextMenuClosed,
        ] {
            assert!(!marks_session_dirty(&Message::Sidebar(msg)));
        }
        // The release is what clicks (opens a tab) or drops (moves a row).
        assert!(marks_session_dirty(&Message::Sidebar(SidebarMsg::ItemDragEnd)));
    }
}

#[cfg(test)]
mod open_cost {
    use std::fmt::Write as _;
    use std::time::{Duration, Instant};

    use crate::app::AppState;
    use crate::domain::collection::SavedRequest;
    use crate::domain::history::HistoryEntry;
    use crate::domain::request::{BodyType, KeyValue};
    use crate::message::{Message, RequestMsg, SidebarMsg};
    use crate::state::tabs::TabManager;

    const LARGE_BODY: usize = 5 * 1024 * 1024;
    const RUNS: usize = 5;
    const COLLECTION: &str = "c";

    fn json_body(min_bytes: usize) -> String {
        let mut body = String::from("[\n");
        let mut id = 0;
        while body.len() < min_bytes {
            let _ = writeln!(body, "  {{\"id\": {id}, \"name\": \"user-{id}\", \"email\": \"user{id}@example.com\"}},");
            id += 1;
        }
        body.push_str("  {}\n]");
        body
    }

    fn request(id: &str, body: String) -> SavedRequest {
        let mut req = SavedRequest::new_in_group(COLLECTION.to_owned(), None, id.to_owned(), 0);
        req.id = id.to_owned();
        req.url = "https://api.example.com/users?page=1".to_owned();
        req.headers = vec![KeyValue {
            key: "Accept".to_owned(),
            value: "application/json".to_owned(),
            ..KeyValue::new_empty()
        }];
        req.body_type = if body.is_empty() { BodyType::None } else { BodyType::Json };
        req.body = body;
        req
    }

    fn history_entry(request: SavedRequest) -> HistoryEntry {
        HistoryEntry {
            id: request.id.clone(),
            timestamp: 0,
            method: request.method.as_str().to_owned(),
            url: request.url.clone(),
            status: 200,
            duration_ms: 2,
            request,
            omitted: Vec::new(),
        }
    }

    /// Fastest of `RUNS` runs of `messages`, each starting from a lone
    /// default tab so the tab under test is built every time.
    fn fastest(state: &mut AppState, messages: impl Fn() -> Vec<Message>) -> Duration {
        (0..RUNS)
            .map(|_| {
                state.tabs = TabManager::default();
                state.sidebar.last_press = None;
                let batch = messages();
                let start = Instant::now();
                for msg in batch {
                    let _ = super::update(state, msg);
                }
                start.elapsed()
            })
            .min()
            .unwrap_or_default()
    }

    /// Opens a new tab, then an empty, a small and a 5 MB request with a
    /// click on its tree row and from history, through the full update. A
    /// history entry is read off the UI thread, so its timing starts when
    /// the entry lands. Run with `--ignored --nocapture`.
    #[test]
    #[ignore = "bench: opening a request from the tree and history"]
    fn open_request_time() {
        let dir = crate::test_support::TempDir::new("open-cost");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        let cases = [
            ("empty", request("empty", String::new())),
            ("small", request("small", json_body(256))),
            ("5 MB", request("large", json_body(LARGE_BODY))),
        ];
        state.requests.insert(COLLECTION.to_owned(), cases.iter().map(|(_, r)| r.clone()).collect());

        let new_tab = fastest(&mut state, || vec![Message::Request(RequestMsg::NewTab)]);
        println!("new tab: {new_tab:?}");
        for (label, req) in &cases {
            let opened = |state: &AppState| state.tabs.active_tab().saved_as.as_ref().map(|(_, id)| id) == Some(&req.id);
            let drafted = |state: &AppState| state.tabs.tabs.len() == 2 && state.tabs.active_tab().title == req.name;
            let tree = fastest(&mut state, || {
                vec![
                    Message::Sidebar(SidebarMsg::ItemPressed(req.id.clone())),
                    Message::Sidebar(SidebarMsg::ItemDragEnd),
                ]
            });
            assert!(opened(&state), "a click on the row opens the {label} request");
            let entry = history_entry(req.clone());
            let history = fastest(&mut state, || vec![Message::Sidebar(SidebarMsg::HistoryRequestLoaded(Ok(Some(entry.clone()))))]);
            assert!(drafted(&state), "the history entry opens the {label} request as a draft");
            println!("{label} request: {tree:?} from the tree, {history:?} from history");
        }
    }
}

#[cfg(test)]
mod modal_focus_tests {
    use futures_util::StreamExt as _;
    use iced::advanced::renderer::Headless;
    use iced::advanced::widget::operation::{self, Operation, Outcome, focusable};
    use iced::{Pixels, Size, Task};
    use iced_runtime::Action;
    use iced_runtime::user_interface::{Cache, UserInterface};

    use crate::message::{AppMsg, Message, RequestMsg};
    use crate::test_support::TempDir;
    use crate::ui::request::url_bar::URL_EDITOR_ID;

    /// Tab and Shift+Tab presses, more than the dialog has fields.
    const PRESSES: usize = 12;
    const TASK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

    type Ui<'a> = UserInterface<'a, Message, iced::Theme, iced::Renderer>;

    /// Runs the widget operations `task` carries on `ui`, as the runtime would.
    fn apply(ui: &mut Ui<'_>, renderer: &iced::Renderer, task: Task<Message>) {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else { return };
        // One action at a time: a widget operation's task only ends once the
        // operation has run. The timeout turns a stream that never ends into
        // a failure instead of a hung test run.
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        let drained = runtime.block_on(async {
            tokio::time::timeout(TASK_TIMEOUT, async {
                while let Some(action) = stream.next().await {
                    if let Action::Widget(mut op) = action {
                        loop {
                            ui.operate(renderer, op.as_mut());
                            match op.finish() {
                                Outcome::Chain(next) => op = next,
                                _ => break,
                            }
                        }
                    }
                }
            })
            .await
        });
        assert!(drained.is_ok(), "a focus task never finished");
    }

    fn outcome<T: 'static>(ui: &mut Ui<'_>, renderer: &iced::Renderer, op: impl Operation<T> + 'static) -> Option<T> {
        let mut op: Box<dyn Operation<T>> = Box::new(op);
        loop {
            ui.operate(renderer, &mut operation::black_box(op.as_mut()));
            match op.finish() {
                Outcome::Some(value) => return Some(value),
                Outcome::Chain(next) => op = next,
                Outcome::None => return None,
            }
        }
    }

    #[test]
    fn tab_and_shift_tab_stay_inside_an_open_dialog() {
        let dir = TempDir::new("modal-focus");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        state.tabs.active_tab_mut().set_url("https://api.example.com/users".to_owned());
        state.tabs.reveal_active();
        let _ = super::update(&mut state, Message::Request(RequestMsg::SaveRequest));
        assert!(state.modal_open(), "the Save dialog is up");

        let mut renderer = iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
            crate::ui::theme::UI_FONT,
            Pixels(16.0),
            Some("tiny-skia"),
        ))
        .expect("software renderer");
        let presses: Vec<Task<Message>> = (0..PRESSES)
            .map(|step| {
                let key = if step % 3 == 2 { AppMsg::FocusPreviousField } else { AppMsg::FocusNextField };
                super::update(&mut state, Message::App(key))
            })
            .collect();
        let mut ui = UserInterface::build(crate::ui::layout::view(&state), Size::new(1280.0, 800.0), Cache::default(), &mut renderer);
        let url_bar = || focusable::is_focused(URL_EDITOR_ID.into());
        let in_modal = || operation::scope(crate::ui::layout::MODAL_ID.into(), focusable::count());

        let fields = outcome(&mut ui, &renderer, in_modal()).map_or(0, |c| c.total);
        assert!(fields >= 2, "the dialog has fields to cycle through: {fields}");
        let mut landed = 0;
        for task in presses {
            apply(&mut ui, &renderer, task);
            assert_eq!(outcome(&mut ui, &renderer, url_bar()), Some(false), "Tab reached the URL bar behind the dialog");
            let anywhere = outcome(&mut ui, &renderer, focusable::count()).and_then(|c| c.focused);
            let inside = outcome(&mut ui, &renderer, in_modal()).and_then(|c| c.focused);
            // Past the last field iced focuses nothing for one press, then wraps.
            assert!(anywhere.is_none() || inside.is_some(), "the focus left the dialog");
            landed += usize::from(inside.is_some());
        }
        assert!(landed > PRESSES / 2, "Tab moved through the dialog's fields");

        // The same presses without the scope walk out of the dialog.
        for _ in 0..4 * fields {
            apply(&mut ui, &renderer, crate::ui::widgets::focus_nav::focus_next());
            if outcome(&mut ui, &renderer, url_bar()) == Some(true) {
                return;
            }
        }
        panic!("unscoped traversal never reached the URL bar, so this test proves nothing");
    }
}
