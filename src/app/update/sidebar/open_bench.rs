//! Where the time goes when a history entry opens, against a copy of a real
//! profile:
//! `RM_BENCH_DATA=<dir holding rustman.db> cargo test bench_history_open -- --ignored --nocapture`

use std::borrow::Cow;
use std::path::Path;
use std::time::Instant;

use iced::advanced::graphics::text::font_system;
use iced::advanced::renderer::{Headless, Style};
use iced::advanced::text::{self, Paragraph as _};
use iced::{Color, Pixels, Size, alignment, mouse};
use iced_runtime::user_interface::{Cache, UserInterface};

use crate::app::AppState;
use crate::domain::history::HistoryEntry;
use crate::message::{Message, SidebarMsg, SidebarPanel};
use crate::services::storage;
use crate::state::tabs::{RequestTabState, make_code_editor};
use crate::ui::theme::{MONO, TEXT_MD, UI_FONT};

const FONTS: [&[u8]; 4] = [
    include_bytes!("../../../../assets/fonts/lucide.ttf"),
    include_bytes!("../../../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../../../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../../../../assets/fonts/NotoSans-Medium.ttf"),
];
const VIEWPORT: Size = Size::new(1280.0, 800.0);
/// The body pane's width in a 1280-wide window, for the wrap pass.
const BODY_WIDTH: f32 = 560.0;

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = f();
    eprintln!("BENCH {label}: {:?}", start.elapsed());
    out
}

pub(super) fn copy_dir(from: &Path, to: &Path) {
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

/// What the app registers at startup, so text measures as it does there.
pub(super) fn load_fonts() {
    let mut fonts = font_system().write().unwrap();
    for bytes in FONTS {
        fonts.load_font(Cow::Borrowed(bytes));
    }
}

fn char_width(content: &str) -> f32 {
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

/// Software frames of the whole window: view and layout, the widget draw
/// pass (where the editor canvases build their geometry), and rasterizing.
pub(super) struct Frames {
    renderer: iced::Renderer,
    cache: Option<Cache>,
}

impl Frames {
    pub(super) fn new() -> Self {
        let renderer = iced::futures::executor::block_on(<iced::Renderer as Headless>::new(UI_FONT, Pixels(16.0), Some("tiny-skia")))
            .expect("software renderer");
        Self { renderer, cache: None }
    }

    pub(super) fn frame(&mut self, label: &str, state: &AppState) {
        let theme = crate::ui::theme::Palette::iced_theme();
        let style = Style {
            text_color: state.tokens().text,
        };
        let start = Instant::now();
        let mut ui = UserInterface::build(crate::ui::layout::view(state), VIEWPORT, self.cache.take().unwrap_or_default(), &mut self.renderer);
        let built = start.elapsed();
        ui.draw(&mut self.renderer, &theme, &style, mouse::Cursor::Unavailable);
        let drawn = start.elapsed();
        self.cache = Some(ui.into_cache());
        let pixels = self.renderer.screenshot(Size::new(1280, 800), 1.0, Color::BLACK);
        let total = start.elapsed();
        if let Ok(shots) = std::env::var("RM_BENCH_SHOTS") {
            let path = Path::new(&shots).join(format!("{}.png", label.replace([' ', ':', '(', ')'], "_")));
            image::save_buffer(path, &pixels, 1280, 800, image::ColorType::Rgba8).unwrap();
        }
        eprintln!("BENCH {label}: view+layout {built:?}, draw {:?}, raster {:?}, total {total:?}", drawn.saturating_sub(built), total.saturating_sub(drawn));
    }
}

fn editor_parts(large: &str) {
    for run in 1..=3 {
        time(&format!("part: measure \"a\" in MONO, call {run}"), || char_width("a"));
        time(&format!("part: measure \"汉\" in MONO, call {run}"), || char_width("汉"));
    }
    time("part: CodeEditor::new(empty)", || iced_code_editor::CodeEditor::new("", "json"));
    let mut editor = iced_code_editor::CodeEditor::new("", "json");
    time("part: set_theme", || editor.set_theme(crate::ui::theme::Palette::code_editor_style()));
    time("part: set_font", || editor.set_font(MONO));
    time("part: set_font_size", || editor.set_font_size(TEXT_MD, true));
    time("part: make_code_editor(empty)", || make_code_editor("", "json"));
    time("part: CodeEditor::new(large)", || iced_code_editor::CodeEditor::new(large, "json"));
    let editor = time("part: make_code_editor(large)", || make_code_editor(large, "json"));
    time("part: wrap pass on the large body (first draw)", || editor.visual_line_count_for_test(BODY_WIDTH));
    time("part: wrap pass again (cached)", || editor.visual_line_count_for_test(BODY_WIDTH));
    for run in 1..=2 {
        time(&format!("part: RequestTabState::new, call {run}"), RequestTabState::new);
    }
}

#[test]
#[ignore = "bench: history open on a copy of RM_BENCH_DATA"]
fn bench_history_open() {
    let Some(data) = crate::test_support::bench_data() else {
        return;
    };
    let dir = crate::test_support::TempDir::new("bench-history");
    copy_dir(&data, dir.path());
    time("font registration", load_fonts);
    let mut state = time("boot::load", || crate::app::boot::load(dir.path().to_path_buf()));
    state.sidebar.panel = SidebarPanel::History;
    let mut frames = Frames::new();
    frames.frame("frame 1 (cold)", &state);
    frames.frame("frame 2", &state);
    time("history summaries (the boot read)", || {
        storage::history_summaries(&state.db(), crate::state::sidebar::HISTORY_LISTED).unwrap()
    });
    let full = time("all entries with requests (the old boot read)", || storage::get_history(&state.db()).unwrap());
    let mut by_size: Vec<(usize, &HistoryEntry)> = full
        .iter()
        .map(|e| (e.request.body.len() + e.request.form_data_fields.iter().map(|f| f.value.len()).sum::<usize>(), e))
        .collect();
    by_size.sort_unstable_by_key(|(size, _)| *size);
    let (small, large) = (by_size[0].1, by_size[by_size.len() - 1].1);
    eprintln!("BENCH entries: {}, largest body {} bytes, {} lines", by_size.len(), large.request.body.len(), large.request.body.lines().count());
    editor_parts(&large.request.body);
    for run in 1..=2 {
        time(&format!("from_saved(large), call {run}"), || RequestTabState::from_saved(&large.request));
    }
    for (label, entry) in [("small", small), ("large", large)] {
        time(&format!("open {label}: update (schedules the load)"), || {
            let _ = crate::app::update::update(&mut state, Message::Sidebar(SidebarMsg::HistoryEntryOpened(entry.id.clone())));
        });
        let loaded = time(&format!("open {label}: background load"), || {
            let conn = storage::open(dir.path()).unwrap();
            storage::history_entry(&conn, &entry.id).unwrap()
        });
        time(&format!("open {label}: update (opens the tab)"), || {
            let _ = crate::app::update::update(&mut state, Message::Sidebar(SidebarMsg::HistoryRequestLoaded(Ok(loaded))));
        });
        frames.frame(&format!("open {label}: first frame"), &state);
        frames.frame(&format!("open {label}: second frame"), &state);
        time(&format!("open {label}: autosave"), || crate::app::session::persist_session(&mut state));
    }
}
