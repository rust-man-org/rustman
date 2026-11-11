//! Startup, shutdown, everyday-action and idle costs on a copy of a real profile and a synthetic one: `RM_BENCH_DATA=<dir holding rustman.db> cargo test --bin rustman app::boot::bench -- --ignored --nocapture --test-threads=1`

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use futures_util::StreamExt as _;
use iced::advanced::graphics::text::font_system;
use iced::advanced::renderer::{Headless, Style};
use iced::advanced::text::{self, Paragraph as _};
use iced::{Color, Pixels, Size, Task, alignment, mouse};
use iced_runtime::Action;
use iced_runtime::user_interface::{Cache, UserInterface};

use crate::app::AppState;
use crate::domain::collection::{Collection, SavedRequest};
use crate::domain::group::Group;
use crate::domain::request::{BodyType, KeyValue};
use crate::domain::response::HttpResponse;
use crate::jobs::JobKind;
use crate::message::{AppMsg, ImportMsg, Message, PaletteMsg, RequestMsg, RequestTab, ResponseMsg, SettingsMsg, SidebarMsg};
use crate::services::storage;
use crate::state::session::AppSession;
use crate::state::tabs::{RequestTabState, TabManager, TabSnapshot};
use crate::test_support::bench_data;
use crate::ui::theme::{MONO, TEXT_MD, UI_FONT};

const FONTS: [&[u8]; 4] = [
    include_bytes!("../../../assets/fonts/lucide.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../../../assets/fonts/NotoSans-Medium.ttf"),
];
const VIEWPORT: Size = Size::new(1280.0, 800.0);
const CHILD: &str = "RM_BENCH_CHILD";
const SYNTH_TABS: usize = 50;
const SYNTH_FOLDERS: usize = 30;
const SYNTH_REQUESTS: usize = 2000;
const LARGE_BODY: usize = 5 * 1024 * 1024;
const LARGE_RESPONSE: usize = 10 * 1024 * 1024;

fn ms(d: Duration) -> String {
    format!("{:.2}", d.as_secs_f64() * 1e3)
}

fn report(scenario: &str, stage: &str, d: Duration) {
    eprintln!("BENCH {scenario} | {stage} | {} ms", ms(d));
}

fn note(scenario: &str, what: &str) {
    eprintln!("BENCH {scenario} | {what}");
}

fn timed<T>(scenario: &str, stage: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = f();
    report(scenario, stage, start.elapsed());
    out
}

fn data() -> Option<PathBuf> {
    std::env::var_os("RM_BENCH_DATA").map(PathBuf::from)
}

/// A copy of a profile on the data's own filesystem, so SQLite pays real
/// fsyncs (the test temp dir is a tmpfs here).
struct Scratch(PathBuf);

impl Scratch {
    fn copy_of(from: &Path, label: &str) -> Self {
        let parent = from.parent().unwrap_or(from);
        let path = parent.join(format!("bench-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        copy_dir(from, &path);
        Self(path)
    }

    fn path(&self) -> PathBuf {
        self.0.clone()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Runs one `#[ignore]`d child test in a new process, so process-wide caches
/// (the font system, syntect, the glyph metrics) start cold.
fn in_fresh_process(test: &str, envs: &[(&str, &str)]) {
    let status = Command::new(std::env::current_exe().unwrap())
        .args([&format!("app::boot::bench::{test}"), "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .envs(envs.iter().copied())
        .status()
        .unwrap();
    assert!(status.success(), "{test} failed");
}

fn is_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

fn load_fonts() {
    let mut fonts = font_system().write().unwrap();
    for bytes in FONTS {
        fonts.load_font(std::borrow::Cow::Borrowed(bytes));
    }
}

fn glyph_width(content: &str) -> f32 {
    let text = text::Text {
        content,
        bounds: Size::INFINITE,
        size: Pixels(TEXT_MD),
        line_height: text::LineHeight::default(),
        font: MONO,
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::default(),
    };
    <iced::Renderer as text::Renderer>::Paragraph::with_text(text).min_width()
}

/// Whole-window frames: view+layout (what iced rebuilds after every
/// message) and the draw pass (editor canvases build geometry here). The
/// app renders on wgpu, so the tiny-skia raster only runs with
/// `RM_BENCH_RENDER` set.
struct Frames {
    renderer: iced::Renderer,
    cache: Option<Cache>,
    render: bool,
}

struct Frame {
    layout: Duration,
    draw: Duration,
    render: Duration,
}

impl Frames {
    fn new() -> Self {
        let renderer = iced::futures::executor::block_on(<iced::Renderer as Headless>::new(UI_FONT, Pixels(16.0), Some("tiny-skia")))
            .expect("software renderer");
        Self {
            renderer,
            cache: None,
            render: std::env::var_os("RM_BENCH_RENDER").is_some(),
        }
    }

    fn frame(&mut self, scenario: &str, label: &str, state: &AppState) -> Frame {
        let theme = crate::ui::theme::Palette::iced_theme();
        let style = Style {
            text_color: state.tokens().text,
        };
        let start = Instant::now();
        let mut ui = UserInterface::build(crate::ui::layout::view(state), VIEWPORT, self.cache.take().unwrap_or_default(), &mut self.renderer);
        let layout = start.elapsed();
        ui.draw(&mut self.renderer, &theme, &style, mouse::Cursor::Unavailable);
        let drawn = start.elapsed();
        self.cache = Some(ui.into_cache());
        if self.render {
            let _ = self.renderer.screenshot(Size::new(1280, 800), 1.0, Color::BLACK);
        }
        let total = start.elapsed();
        let frame = Frame {
            layout,
            draw: drawn - layout,
            render: total - drawn,
        };
        let render = if self.render { format!(", render {} ms", ms(frame.render)) } else { String::new() };
        note(scenario, &format!("{label}: view+layout {} ms, draw {} ms{render}", ms(frame.layout), ms(frame.draw)));
        frame
    }
}

/// The messages `task` produces, as the runtime would deliver them.
fn outputs(task: Task<Message>) -> Vec<Message> {
    let Some(stream) = iced_runtime::task::into_stream(task) else { return Vec::new() };
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(
        stream
            .filter_map(|action| async move {
                match action {
                    Action::Output(message) => Some(message),
                    _ => None,
                }
            })
            .collect(),
    )
}

fn send(state: &mut AppState, msg: Message) -> Task<Message> {
    crate::app::update::update(state, msg)
}

/// `update` on the UI thread, then the task's background work, then the
/// messages it produced, each timed.
fn round_trip(scenario: &str, label: &str, state: &mut AppState, msg: Message) {
    let task = timed(scenario, &format!("{label}: update"), || send(state, msg));
    let follow_ups = timed(scenario, &format!("{label}: background task"), || outputs(task));
    for (i, msg) in follow_ups.into_iter().enumerate() {
        let task = timed(scenario, &format!("{label}: update for result {i}"), || send(state, msg));
        drop(task);
    }
}

fn json_array(min_bytes: usize, pretty: bool) -> String {
    let mut body = String::from("[");
    let mut id = 0;
    while body.len() < min_bytes {
        if pretty {
            let _ = write!(body, "\n  {{\"id\": {id}, \"name\": \"user-{id}\", \"email\": \"user{id}@example.com\", \"active\": true}},");
        } else {
            let _ = write!(body, "{{\"id\":{id},\"name\":\"user-{id}\",\"email\":\"user{id}@example.com\",\"active\":true}},");
        }
        id += 1;
    }
    body.push_str("{}]");
    body
}

fn small_request(collection: &str, group: Option<String>, i: usize) -> SavedRequest {
    let mut req = SavedRequest::new_in_group(collection.to_owned(), group, format!("Request {i}"), i as i64);
    req.url = format!("https://api.example.com/v1/resource/{i}?page=1&limit=20");
    req.headers = vec![KeyValue {
        key: "Accept".to_owned(),
        value: "application/json".to_owned(),
        ..KeyValue::new_empty()
    }];
    req.body_type = BodyType::Json;
    req.body = format!("{{\"id\": {i}, \"name\": \"item-{i}\", \"tags\": [\"a\", \"b\"]}}");
    req
}

fn snapshot_of(req: &SavedRequest) -> TabSnapshot {
    (&RequestTabState::from_saved(req)).into()
}

/// Adds 2,000 requests in 30 folders and a 50-tab session (one tab holds a
/// 5 MB body) to the profile in `dir`.
fn make_synthetic(dir: &Path) -> String {
    let conn = storage::open(dir).unwrap();
    let col = Collection {
        id: "synthetic".to_owned(),
        name: "Synthetic".to_owned(),
        created_at: 1,
    };
    conn.execute_batch("BEGIN").unwrap();
    storage::create_collection(&conn, &col).unwrap();
    let mut requests = Vec::with_capacity(SYNTH_REQUESTS);
    for f in 0..SYNTH_FOLDERS {
        let group = Group {
            id: format!("folder-{f}"),
            parent_id: col.id.clone(),
            parent_is_collection: true,
            name: format!("Folder {f}"),
            sort_order: f as i64,
        };
        storage::create_group(&conn, &group).unwrap();
        for i in (f..SYNTH_REQUESTS).step_by(SYNTH_FOLDERS) {
            let req = small_request(&col.id, Some(group.id.clone()), i);
            storage::create_request(&conn, &req).unwrap();
            requests.push(req);
        }
    }
    conn.execute_batch("COMMIT").unwrap();

    let mut session: AppSession = storage::load_session(&conn).ok().flatten().unwrap_or_default();
    let mut large = requests[0].clone();
    large.body = json_array(LARGE_BODY, true);
    let mut tabs = vec![snapshot_of(&large)];
    tabs[0].active_request_tab = RequestTab::Body;
    tabs.extend(requests[1..SYNTH_TABS].iter().map(snapshot_of));
    session.tabs = tabs;
    session.active_tab = SYNTH_TABS - 1;
    storage::save_session(&conn, &session).unwrap();
    col.id
}

fn wal_bytes(dir: &Path) -> u64 {
    std::fs::metadata(dir.join("rustman.db-wal")).map_or(0, |m| m.len())
}

fn body_bytes(snap: &TabSnapshot) -> usize {
    snap.body.len() + snap.graphql_variables.len() + snap.pre_request_script.len() + snap.test_script.len()
}

// ── Startup ───────────────────────────────────────────────────────────────────

/// Startup in fresh processes: the app's own order, the same split into
/// stages, and the profile scaled up.
#[test]
#[ignore = "bench: startup on a copy of RM_BENCH_DATA"]
fn bench_startup() {
    if is_child() || bench_data().is_none() {
        return;
    }
    in_fresh_process("startup_as_app", &[("RM_BENCH_PROFILE", "real")]);
    in_fresh_process("startup_stages", &[]);
    in_fresh_process("startup_as_app", &[("RM_BENCH_PROFILE", "synthetic")]);
}

/// What runs before the first frame, in the order the app runs it.
#[test]
#[ignore = "bench: child process of bench_startup, not run directly"]
fn startup_as_app() {
    let (true, Some(data)) = (is_child(), data()) else { return };
    let profile = std::env::var("RM_BENCH_PROFILE").unwrap_or_default();
    let s = format!("startup/{profile}");
    let s = s.as_str();
    let scratch = Scratch::copy_of(&data, "startup");
    if profile == "synthetic" {
        make_synthetic(&scratch.path());
    }
    let process = Instant::now();
    timed(s, "window icon decode (app::run)", crate::app::app_icon);
    let executor = timed(s, "tokio runtime (iced executor)", || <iced::executor::Default as iced::Executor>::new().unwrap());
    let state = timed(s, "boot::load (cold: font system starts in the first editor)", || super::load(scratch.path()));
    let snaps: Vec<TabSnapshot> = state.tabs.tabs.iter().map(Into::into).collect();
    let bytes: usize = snaps.iter().map(body_bytes).sum();
    note(s, &format!("restored {} tabs, {} code editors, {bytes} bytes of editor text", snaps.len(), snaps.len() * 5));
    let task = timed(s, "boot tasks: build (git branch + update check)", || {
        Task::batch([crate::app::update::git_branch_task(&state)])
    });
    let _ = timed(s, "git branch task (runs off the UI thread)", || outputs(task));
    timed(s, "compositor font registration (4 fonts)", load_fonts);
    let mut frames = Frames::new();
    let first = frames.frame(s, "first frame", &state);
    report(s, "process start to first frame, CPU stages (no window/wgpu)", process.elapsed() - first.render);
    frames.frame(s, "second frame", &state);
    let mut state = state;
    let active = state.tabs.active;
    state.tabs.tabs[active].active_request_tab = RequestTab::Body;
    frames.frame(s, "first frame with a code editor on screen (syntect loads)", &state);
    frames.frame(s, "next frame", &state);
    drop(executor);
}

/// The same work split into the stages `boot::load` runs, fonts first.
#[test]
#[ignore = "bench: child process of bench_startup, not run directly"]
fn startup_stages() {
    let (true, Some(data)) = (is_child(), data()) else { return };
    let s = "startup stages/real";
    let scratch = Scratch::copy_of(&data, "stages");
    let dir = scratch.path();
    timed(s, "font system init (fontdb scans system fonts)", || {
        let _ = font_system();
    });
    timed(s, "register the 4 bundled fonts", load_fonts);
    timed(s, "measure 'a' in MONO", || glyph_width("a"));
    timed(s, "measure '汉' in MONO (fallback font search)", || glyph_width("汉"));
    let conn = timed(s, "DB open + migrations", || storage::open(&dir).unwrap());
    let collections = timed(s, "collections", || storage::get_collections(&conn).unwrap());
    timed(s, "history summaries", || storage::history_summaries(&conn, crate::state::sidebar::HISTORY_LISTED).unwrap());
    timed(s, "groups", || storage::get_groups(&conn).unwrap());
    timed(s, "environments", || storage::get_environments(&conn).unwrap());
    timed(s, "requests (one query per collection)", || {
        for c in &collections {
            storage::get_requests(&conn, &c.id).unwrap();
        }
    });
    let session: AppSession = timed(s, "session read + decode", || storage::load_session(&conn).unwrap().unwrap_or_default());
    let tabs = timed(s, "restore tabs (from_snapshot: 5 code editors each)", || {
        session.tabs.iter().cloned().map(RequestTabState::from_snapshot).collect::<Vec<_>>()
    });
    for snap in session.tabs.iter().filter(|t| body_bytes(t) > 0).take(2) {
        let snap = snap.clone();
        let bytes = body_bytes(&snap);
        timed(s, &format!("  one tab with {bytes} bytes of editor text"), || RequestTabState::from_snapshot(snap));
    }
    timed(s, "  one empty tab", RequestTabState::new);
    timed(s, "git repo registry", || crate::services::repos::load(&dir));
    timed(s, "git identity (repo config)", || crate::services::vcs::resolve_identity(&dir.join("collections")));
    drop((tabs, conn));
    timed(s, "boot::load again (warm caches)", || super::load(dir.clone()));
}

// ── Shutdown ──────────────────────────────────────────────────────────────────

/// What a close request and the teardown after it cost, real and synthetic.
#[test]
#[ignore = "bench: close and teardown on a copy of RM_BENCH_DATA"]
fn bench_shutdown() {
    let (false, Some(data)) = (is_child(), bench_data()) else { return };
    load_fonts();
    for profile in ["real", "synthetic"] {
        let s = format!("shutdown/{profile}");
        let s = s.as_str();
        let scratch = Scratch::copy_of(&data, "shutdown");
        let dir = scratch.path();
        if profile == "synthetic" {
            make_synthetic(&dir);
        }
        let mut state = super::load(dir.clone());
        for _ in 0..5 {
            crate::app::session::persist_session(&mut state);
        }
        note(s, &format!("WAL after 5 autosaves: {} bytes", wal_bytes(&dir)));
        let session = timed(s, "persist: snapshot every tab (editor content copies)", || {
            crate::app::session::session_from_state(&state)
        });
        let json = timed(s, "persist: serde_json::to_string", || serde_json::to_string(&session).unwrap());
        note(s, &format!("session blob {} bytes, {} tabs", json.len(), session.tabs.len()));
        let db = &state.db();
        timed(s, "persist: SQLite upsert + WAL fsync", || {
            db.execute(
                "INSERT OR REPLACE INTO session (key, data, saved_at) VALUES ('current', ?1, 0)",
                [&json],
            )
            .unwrap()
        });
        timed(s, "persist_session total (what WindowCloseRequested runs)", || {
            crate::app::session::persist_session(&mut state)
        });
        state.session_dirty = false;
        let task = timed(s, "update(WindowCloseRequested) total", || {
            send(&mut state, Message::App(AppMsg::WindowCloseRequested(iced::window::Id::unique())))
        });
        drop(task);
        note(s, &format!("dirty after the close handler: {}", state.session_dirty));
        let tabs = std::mem::replace(&mut state.tabs, TabManager { tabs: Vec::new(), active: 0 });
        timed(s, "teardown: drop tabs", || drop(tabs));
        let wal = wal_bytes(&dir);
        let store = state.store.take();
        timed(s, &format!("teardown: stop the store thread and close SQLite (checkpoints a {wal}-byte WAL)"), || drop(store));
        timed(s, "teardown: drop the rest of the state", || drop(state));
    }
    let s = "shutdown/runtime";
    let runtime = <iced::executor::Default as iced::Executor>::new().unwrap();
    let body = json_array(LARGE_RESPONSE, false);
    runtime.spawn(async move {
        let _ = tokio::task::spawn_blocking(move || {
            let value: serde_json::Value = serde_json::from_str(&body).unwrap();
            rustman_core::text::pretty_value(&value, false)
        })
        .await;
    });
    std::thread::sleep(Duration::from_millis(5));
    timed(s, "drop the tokio runtime with a 10 MB pretty-print in flight", || drop(runtime));
    let runtime = <iced::executor::Default as iced::Executor>::new().unwrap();
    timed(s, "drop an idle tokio runtime", || drop(runtime));
}

// ── Everyday actions ──────────────────────────────────────────────────────────

/// Update plus the frame iced draws after it, on a 50-tab, 2,000-request profile.
#[test]
#[ignore = "bench: everyday actions on a copy of RM_BENCH_DATA"]
fn bench_everyday() {
    let (false, Some(data)) = (is_child(), bench_data()) else { return };
    load_fonts();
    let scratch = Scratch::copy_of(&data, "everyday");
    let dir = scratch.path();
    let synth_id = make_synthetic(&dir);
    let mut state = super::load(dir.clone());
    let mut frames = Frames::new();
    frames.frame("warm-up", "first frame", &state);

    let s = "switch tab";
    for (label, i) in [("to a small tab", 10), ("to another small tab", 11), ("to the 5 MB-body tab", 0), ("back to a small tab", 12)] {
        let _ = timed(s, &format!("{label}: update"), || send(&mut state, Message::Request(RequestMsg::SwitchTab(i))));
        frames.frame(s, &format!("{label}: frame"), &state);
        frames.frame(s, &format!("{label}: next frame"), &state);
    }
    timed(s, "of which persist_session (50 tabs, 5 MB body)", || crate::app::session::persist_session(&mut state));

    let s = "type in URL bar";
    let _ = send(&mut state, Message::Request(RequestMsg::SwitchTab(10)));
    frames.frame(s, "settle", &state);
    for c in ['/', 'x', 'y'] {
        let action = iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert(c));
        let _ = timed(s, &format!("'{c}': update"), || send(&mut state, Message::Request(RequestMsg::UrlEdited(action))));
        frames.frame(s, &format!("'{c}': frame"), &state);
    }

    let s = "type in 5 MB body";
    let _ = send(&mut state, Message::Request(RequestMsg::SwitchTab(0)));
    frames.frame(s, "open the body (first wrap pass)", &state);
    frames.frame(s, "settle", &state);
    for c in ['a', 'b', 'c'] {
        let _ = timed(s, &format!("'{c}': update"), || {
            send(&mut state, Message::Request(RequestMsg::BodyEdited(iced_code_editor::Message::CharacterInput(c))))
        });
        frames.frame(s, &format!("'{c}': frame"), &state);
    }
    note(s, &format!("dirty after typing: {} (the 3 s autosave then runs persist_session)", state.session_dirty));

    let s = "format 5 MB body";
    round_trip(s, "FormatBody", &mut state, Message::Request(RequestMsg::FormatBody));
    frames.frame(s, "frame", &state);

    let s = "send + 10 MB response";
    let _ = send(&mut state, Message::Request(RequestMsg::SwitchTab(10)));
    frames.frame(s, "settle", &state);
    let task = timed(s, "Send: update (until the request leaves)", || send(&mut state, Message::Request(RequestMsg::Send)));
    drop(task);
    let tab = state.tabs.active_tab_mut();
    let (generation, _) = tab.jobs.start(JobKind::Request);
    tab.is_loading = true;
    let request = tab.to_request();
    let tab_id = tab.id.clone();
    frames.frame(s, "loading frame", &state);
    let body = json_array(LARGE_RESPONSE, false);
    let response = HttpResponse {
        status: 200,
        status_text: "OK".to_owned(),
        headers: [("content-type".to_owned(), "application/json".to_owned())].into(),
        body_size: body.len(),
        body,
        duration_ms: 120,
        ..HttpResponse::default()
    };
    let result = crate::services::http::HttpResult {
        tab_id,
        env_id: None,
        request,
        response,
    };
    round_trip(s, "HttpResponse", &mut state, Message::App(AppMsg::HttpResponse { generation, result }));
    frames.frame(s, "viewer first page: frame", &state);
    frames.frame(s, "viewer first page: next frame", &state);
    let _ = timed(s, "page 2: update", || send(&mut state, Message::Response(ResponseMsg::ViewerPageNext)));
    frames.frame(s, "page 2: frame", &state);

    let s = "search in 10 MB response";
    let _ = send(&mut state, Message::Response(ResponseMsg::SearchOpen));
    for query in ["u", "user-9", "user-99999"] {
        let _ = timed(s, &format!("'{query}': update"), || {
            send(&mut state, Message::Response(ResponseMsg::SearchQueryChanged(query.to_owned())))
        });
        frames.frame(s, &format!("'{query}': frame"), &state);
    }
    let _ = send(&mut state, Message::Response(ResponseMsg::SearchClose));

    let s = "palette";
    let _ = timed(s, "open: update", || send(&mut state, Message::Palette(PaletteMsg::Open)));
    frames.frame(s, "open: frame", &state);
    for query in ["r", "re", "req", "request 19"] {
        let _ = timed(s, &format!("'{query}': update"), || send(&mut state, Message::Palette(PaletteMsg::QueryChanged(query.to_owned()))));
        frames.frame(s, &format!("'{query}': frame"), &state);
    }
    let _ = send(&mut state, Message::Palette(PaletteMsg::Close));

    let s = "settings";
    let _ = timed(s, "open: update", || {
        send(&mut state, Message::Settings(SettingsMsg::PageSelected(crate::ui::settings::Page::General)))
    });
    frames.frame(s, "open: frame", &state);
    frames.frame(s, "open: next frame", &state);
    let _ = timed(s, "close: update", || send(&mut state, Message::Settings(SettingsMsg::Closed)));
    frames.frame(s, "close: frame", &state);

    let s = "sidebar tree";
    let click = |state: &mut AppState, id: &str| {
        let _ = send(state, Message::Sidebar(SidebarMsg::ItemPressed(id.to_owned())));
        let _ = send(state, Message::Sidebar(SidebarMsg::ItemDragEnd));
    };
    state.sidebar.expanded.remove(&synth_id);
    frames.frame(s, "settle", &state);
    timed(s, "expand the 30-folder collection: update", || click(&mut state, &synth_id));
    frames.frame(s, "expand the 30-folder collection: frame", &state);
    timed(s, "expand all 30 folders (2,000 rows): update", || {
        for f in 0..SYNTH_FOLDERS {
            click(&mut state, &format!("folder-{f}"));
        }
    });
    frames.frame(s, "2,000 rows visible: frame", &state);
    frames.frame(s, "2,000 rows visible: next frame", &state);
    timed(s, "collapse the collection: update", || click(&mut state, &synth_id));
    frames.frame(s, "collapse the collection: frame", &state);

    let s = "import Postman (2,000 requests)";
    let postman = postman_collection();
    note(s, &format!("file {} bytes", postman.len()));
    let outcome = timed(s, "parse (off the UI thread)", || crate::services::import::import_collection(&postman).unwrap());
    let _ = timed(s, "CollectionsImported: update (DB writes on the UI thread)", || {
        send(&mut state, Message::Import(ImportMsg::CollectionsImported(outcome)))
    });
    frames.frame(s, "frame", &state);

    let s = "idle";
    crate::app::session::persist_session(&mut state);
    note(
        s,
        &format!(
            "after the autosave: dirty {}, tabs loading {}, ws subscriptions {}",
            state.session_dirty,
            state.tabs.tabs.iter().filter(|t| t.is_loading).count(),
            state.tabs.tabs.iter().filter(|t| t.ws.connecting || t.ws.connected).count()
        ),
    );
}

fn postman_collection() -> String {
    let folders: Vec<serde_json::Value> = (0..SYNTH_FOLDERS)
        .map(|f| {
            let items: Vec<serde_json::Value> = (f..SYNTH_REQUESTS)
                .step_by(SYNTH_FOLDERS)
                .map(|i| {
                    serde_json::json!({
                        "name": format!("Request {i}"),
                        "request": {
                            "method": "POST",
                            "header": [{"key": "Content-Type", "value": "application/json"}],
                            "url": {"raw": format!("https://api.example.com/v1/items/{i}?page=1"), "host": ["api", "example", "com"]},
                            "body": {"mode": "raw", "raw": format!("{{\"id\": {i}, \"name\": \"item-{i}\"}}")}
                        }
                    })
                })
                .collect();
            serde_json::json!({"name": format!("Folder {f}"), "item": items})
        })
        .collect();
    serde_json::json!({
        "info": {"name": "Big import", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
        "item": folders
    })
    .to_string()
}
