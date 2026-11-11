//! The open-request tab strip: 100–180 px tabs (method, title, close or
//! unsaved dot), the new-request button, an overflow fade, and the
//! open-tabs menu. The active tab opens into the page below it.

use std::hash::{Hash, Hasher};

use iced::widget::{Space, button, column, container, hover, mouse_area, row, scrollable, stack, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, gradient};

use crate::app::AppState;
use crate::message::{AppMsg, MenuId, Message, PaletteMsg, RequestMsg};
use crate::state::tabs::{RequestTabState, StripLabel, TabManager};
use crate::ui::components::icon_button::{IconSize, icon_button, icon_button_toggled};
use crate::ui::components::Surface;
use crate::ui::components::kbd::{KeyCap, kbd};
use crate::ui::components::menu::{self, Align, Entry, Item};
use crate::ui::components::tooltip::shortcut;
use crate::ui::components::{hairline, status_dot, tooltip};
use crate::ui::icons;
use crate::ui::sidebar::collections::METHOD_COLUMN;
use crate::ui::theme::{
    CONTROL_MD, HAIRLINE, ICON_LG, MONO, RADIUS_XS, SPACE_LG, SPACE_MD, SPACE_XL, SPACE_XS,
    TAB_STRIP_HEIGHT, TEXT_CAPTION, TEXT_MD, TEXT_SM, Tokens,
};
use crate::ui::tree_label::{Face, Measured};

/// Widget id of the strip's scrollable, so the active tab can be revealed.
pub const TAB_STRIP_ID: &str = "tab-strip";

const TAB_MIN: f32 = 100.0;
const TAB_MAX: f32 = 180.0;
/// Space a tab spends besides its title: padding, the method, the gap and
/// the 20 px close/dot slot, plus its two side borders.
const PAD_LEFT: f32 = SPACE_LG;
const PAD_RIGHT: f32 = SPACE_XS + 2.0;
const END_SLOT: f32 = 20.0;
/// Width of the fade over the strip's right edge when tabs overflow.
const FADE: f32 = SPACE_XL + SPACE_MD;
/// Width of the tab-strip search when native decorations are on.
const SEARCH_WIDTH: f32 = 220.0;
/// Below this window width the tab-strip search collapses to an icon.
const SEARCH_COLLAPSE_BELOW: f32 = 1100.0;
const MENU_WIDTH: f32 = 400.0;

/// What a tab is called: a saved request's name, else its URL without the
/// scheme (which is the same on every tab), else its title.
pub(crate) fn tab_label(tab: &RequestTabState) -> &str {
    let url = tab.url.trim();
    if is_named(tab) || url.is_empty() {
        return tab.title.as_str();
    }
    for scheme in ["https://", "http://", "wss://", "ws://"] {
        // `get` rather than slicing: the URL is user text, and a byte index
        // can land inside a multi-byte character.
        if url.len() > scheme.len() && url.get(..scheme.len()).is_some_and(|p| p.eq_ignore_ascii_case(scheme)) {
            return &url[scheme.len()..];
        }
    }
    url
}

/// A saved request the user named (saving keeps "New Request" otherwise).
fn is_named(tab: &RequestTabState) -> bool {
    let title = tab.title.trim();
    tab.saved_as.is_some() && !title.is_empty() && title != "New Request"
}

fn method_text(tab: &RequestTabState) -> &str {
    if tab.is_websocket() { "WS" } else { tab.method.as_str() }
}

/// A dot instead of the close button: edits not yet saved, or a request
/// that was never saved at all.
pub(crate) fn has_unsaved_changes(tab: &RequestTabState) -> bool {
    tab.modified || (tab.saved_as.is_none() && !tab.url.trim().is_empty())
}

/// The tab width for a title `title_width` wide, and the title's budget.
fn tab_geometry(method_width: f32, title_width: f32) -> (f32, f32) {
    let chrome = PAD_LEFT + method_width + SPACE_MD + SPACE_MD + END_SLOT + PAD_RIGHT + 2.0 * HAIRLINE;
    let width = (chrome + title_width).clamp(TAB_MIN, TAB_MAX);
    (width, width - chrome)
}

fn label_key(tab: &RequestTabState) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (tab_label(tab), method_text(tab)).hash(&mut h);
    h.finish()
}

/// Refreshes each tab's strip label whose title, URL or method changed.
pub(crate) fn sync_strip_labels(tabs: &mut TabManager) {
    for tab in &mut tabs.tabs {
        let key = label_key(tab);
        if tab.strip_label.key == key && key != 0 {
            continue;
        }
        let method = Measured::new(method_text(tab), TEXT_CAPTION, Face::Mono).width();
        let title = Measured::new(tab_label(tab), TEXT_SM, Face::Regular);
        let (width, budget) = tab_geometry(method, title.width());
        let fitted = title.fitted(budget);
        tab.strip_label = StripLabel {
            key,
            cut: fitted.is_some(),
            text: fitted.unwrap_or_else(|| title.into_text()),
            width,
        };
    }
}

pub(super) fn multi_tab_bar<'a>(state: &'a AppState, show_search: bool, t: &'a Tokens) -> Element<'a, Message> {
    let window_width = state.chrome.window.map_or(f32::MAX, |w| w.width);
    let base = column![Space::new().height(TAB_STRIP_HEIGHT - HAIRLINE), hairline(t)];
    let mut strip = row![strip_tabs(state, t)].align_y(Alignment::Center).height(TAB_STRIP_HEIGHT);
    if !state.tabs.shows_start() {
        strip = strip.push(open_tabs_menu(state, t));
    }
    if show_search {
        strip = strip.push(compact_search(t, window_width < SEARCH_COLLAPSE_BELOW));
    }
    stack![base, strip.padding(Padding { right: SPACE_MD, ..Padding::ZERO })]
        .width(Length::Fill)
        .into()
}

/// The scrolling row of tabs plus the new-request button, with a fade over
/// the right edge while it overflows.
fn strip_tabs<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let content_width: f32 = state.tabs.tabs.iter().map(|tab| tab.strip_label.width).sum::<f32>() + SPACE_XS + CONTROL_MD;
    iced::widget::responsive(move |size| {
        let mut tabs = row![].height(TAB_STRIP_HEIGHT).align_y(Alignment::Center);
        if !state.tabs.shows_start() {
            for (i, tab) in state.tabs.tabs.iter().enumerate() {
                tabs = tabs.push(tab_view(tab, i, i == state.tabs.active, t));
            }
        }
        tabs = tabs.push(Space::new().width(SPACE_XS)).push(new_tab_button(t));
        let scroller = scrollable(tabs)
            .id(TAB_STRIP_ID)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(0).scroller_width(0),
            ))
            .on_scroll(|v| {
                Message::App(AppMsg::TabStripScrolled {
                    offset: v.absolute_offset().x,
                    viewport: v.bounds().width,
                    content: v.content_bounds().width,
                })
            })
            .width(Length::Fill);
        let scroller = crate::ui::widgets::wheel_horizontal::wheel_horizontal(scroller);
        let strip = state.chrome.strip;
        let left = strip.offset > 0.5;
        let right = content_width - strip.offset > size.width + 0.5;
        if !(left || right) {
            return scroller;
        }
        let mut layers = stack![scroller];
        if left {
            layers = layers.push(container(fade(t, false)).width(Length::Fill).align_x(Alignment::Start));
        }
        if right {
            layers = layers.push(container(fade(t, true)).width(Length::Fill).align_x(Alignment::End));
        }
        layers.into()
    })
    .into()
}

/// A fade over one edge of the strip; `rightward` fades toward the right.
fn fade(t: &Tokens, rightward: bool) -> Element<'_, Message> {
    let (from, to) = (Color { a: 0.0, ..t.canvas }, t.canvas);
    let (start, end) = if rightward { (from, to) } else { (to, from) };
    container(Space::new().width(FADE).height(TAB_STRIP_HEIGHT - HAIRLINE))
        .style(move |_| container::Style {
            background: Some(Background::Gradient(
                gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                    .add_stop(0.0, start)
                    .add_stop(1.0, end)
                    .into(),
            )),
            ..Default::default()
        })
        .into()
}

/// The scroll offset that brings tab `index` fully into a `viewport` wide
/// strip scrolled to `offset`, or `None` when it is already visible.
pub fn reveal_offset(widths: &[f32], index: usize, viewport: f32, offset: f32) -> Option<f32> {
    let start: f32 = widths.iter().take(index).sum();
    let end = start + widths.get(index)?;
    if start < offset {
        Some(start)
    } else if end > offset + viewport {
        Some(end - viewport)
    } else {
        None
    }
}

/// Scrolls the strip so the active tab is in view.
pub(crate) fn reveal_active_tab(state: &AppState) -> iced::Task<Message> {
    let widths: Vec<f32> = state.tabs.tabs.iter().map(|tab| tab.strip_label.width).collect();
    let strip = state.chrome.strip;
    let viewport = if strip.viewport > 0.0 { strip.viewport } else { estimated_viewport(state) };
    match reveal_offset(&widths, state.tabs.active, viewport, strip.offset) {
        Some(x) => iced::widget::operation::scroll_to(
            TAB_STRIP_ID,
            scrollable::AbsoluteOffset { x: Some(x), y: None },
        ),
        None => iced::Task::none(),
    }
}

/// The strip's width before it has reported one: the window less the
/// sidebar and the strip's own trailing controls.
fn estimated_viewport(state: &AppState) -> f32 {
    let window = state.chrome.window.map_or(crate::app::WINDOW_SIZE.width, |w| w.width);
    let sidebar = if state.sidebar.collapsed { 0.0 } else { state.sidebar_width + HAIRLINE };
    window - crate::ui::theme::RAIL_WIDTH - sidebar - CONTROL_MD - SPACE_MD - SPACE_XS - CONTROL_MD
}

fn tab_view<'a>(tab: &'a RequestTabState, index: usize, active: bool, t: &'a Tokens) -> Element<'a, Message> {
    let label = &tab.strip_label;
    let method_color = t.method_color(method_text(tab));
    let content = |end: Element<'a, Message>| {
        row![
            text(method_text(tab)).font(MONO).size(TEXT_CAPTION).color(method_color),
            text(label.text.as_str())
                .size(TEXT_SM)
                .wrapping(iced_core::text::Wrapping::None)
                .width(Length::Fill),
            end,
        ]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center)
        .padding(Padding {
            left: PAD_LEFT,
            right: PAD_RIGHT,
            ..Padding::ZERO
        })
        .height(Length::Fill)
    };
    let rest: Element<'a, Message> = if has_unsaved_changes(tab) {
        container(status_dot::dot(t.text_muted)).center(END_SLOT).into()
    } else if active {
        close_button(&tab.id, t)
    } else {
        Space::new().width(END_SLOT).into()
    };
    let body = hover(content(rest), content(close_button(&tab.id, t)));
    let card = container(body)
        .width(label.width)
        .height(TAB_STRIP_HEIGHT)
        .style(move |_| tab_style(t, active, index == 0));
    let card: Element<'a, Message> = if label.cut || is_named(tab) {
        tooltip(t, card, tab_hover_text(tab), None)
    } else {
        card.into()
    };
    mouse_area(card)
        .on_press(Message::Request(RequestMsg::SwitchTab(index)))
        .on_middle_press(Message::Request(RequestMsg::CloseTab(tab.id.clone())))
        .into()
}

/// The full title, and for a named request the URL it points at.
fn tab_hover_text(tab: &RequestTabState) -> String {
    let url = crate::services::format::head_capped(tab.url.trim(), crate::consts::HOVER_TIP_BYTES);
    if is_named(tab) && !url.is_empty() {
        format!("{}\n{} {url}", tab.title, method_text(tab))
    } else {
        tab_label(tab).to_owned()
    }
}

fn close_button<'a>(id: &str, t: &'a Tokens) -> Element<'a, Message> {
    let x = button(container(icons::close().size(TEXT_SM)).center(Length::Fill))
        .on_press(Message::Request(RequestMsg::CloseTab(id.to_owned())))
        .width(END_SLOT)
        .height(END_SLOT)
        .padding(0)
        .style(move |_, status| {
            let mut style = crate::ui::components::icon_button::style(t, status, false);
            style.border.radius = RADIUS_XS.into();
            style
        });
    tooltip(t, x, "Close", Some(shortcut("W")))
}

/// The active tab is a canvas-colored card with side borders whose bottom
/// covers the strip's hairline, so it opens into the page.
fn tab_style(t: &Tokens, active: bool, first: bool) -> container::Style {
    container::Style {
        background: active.then_some(Background::Color(t.canvas)),
        text_color: Some(if active { t.text } else { t.text_muted }),
        border: Border::default(),
        shadow: iced::Shadow::default(),
        snap: true,
    }
    .border(if active && !first {
        Border {
            color: t.border,
            width: HAIRLINE,
            radius: 0.0.into(),
        }
    } else {
        Border::default()
    })
}

fn new_tab_button(t: &Tokens) -> Element<'_, Message> {
    tooltip(
        t,
        icon_button(t, icons::plus().size(ICON_LG), IconSize::Medium).on_press(Message::Request(RequestMsg::NewTab)),
        "New request",
        Some(shortcut("T")),
    )
}

fn open_tabs_menu<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let open = state.chrome.open_menu == Some(MenuId::OpenTabs);
    let trigger = icon_button_toggled(t, icons::chevron_down(), IconSize::Medium, open)
        .on_press(Message::App(AppMsg::MenuToggled(MenuId::OpenTabs)));
    let panel = open.then(|| {
        let entries: Vec<Entry<'a, Message>> = state
            .tabs
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let method = text(method_text(tab))
                    .font(MONO)
                    .size(TEXT_CAPTION)
                    .color(t.on_raised.methods.for_method(method_text(tab)))
                    .width(*METHOD_COLUMN);
                Item::new(tab_label(tab), Message::Request(RequestMsg::SwitchTab(i)))
                    .lead(method)
                    .dot(has_unsaved_changes(tab).then_some(t.on_raised.text_muted))
                    .checked(i == state.tabs.active)
                    .into()
            })
            .collect();
        menu::panel(t, entries, MENU_WIDTH)
    });
    tooltip(
        t,
        menu::popover(trigger, panel, Align::End, Message::App(AppMsg::MenuDismissed)),
        "All open tabs",
        None,
    )
}

fn compact_search(t: &Tokens, collapsed: bool) -> Element<'_, Message> {
    let open = Message::Palette(PaletteMsg::Open);
    if collapsed {
        return tooltip(t, icon_button(t, icons::search(), IconSize::Medium).on_press(open), "Search or run command", Some(shortcut("K")));
    }
    let field = button(
        row![
            container(icons::search().size(TEXT_MD + 1.0)).center(ICON_LG),
            text("Search or run command").size(TEXT_SM),
            Space::new().width(Length::Fill),
            kbd(t, KeyCap::Text(PALETTE_KEYS), Surface::Canvas),
        ]
        .spacing(SPACE_MD)
        .align_y(Alignment::Center)
        .height(Length::Fill),
    )
    .on_press(open)
    .width(SEARCH_WIDTH)
    .height(CONTROL_MD)
    .padding(Padding {
        left: SPACE_MD,
        right: SPACE_XS - 1.0,
        ..Padding::ZERO
    })
    .style(move |_, status| super::title_bar::search_style(t, status));
    container(field).padding(Padding { left: SPACE_MD, ..Padding::ZERO }).into()
}

#[cfg(target_os = "macos")]
const PALETTE_KEYS: &str = "Cmd K";
#[cfg(not(target_os = "macos"))]
const PALETTE_KEYS: &str = "Ctrl K";

#[cfg(test)]
mod tests {
    use super::*;

    fn tab_with_url(url: &str) -> RequestTabState {
        let mut tab = RequestTabState::new();
        tab.url = url.to_owned();
        tab
    }

    #[test]
    fn websocket_tabs_are_labelled_ws_not_a_method() {
        for url in ["ws://localhost:8080", "WSS://UPPER.example.com", "  wss://x.example.com"] {
            assert_eq!(method_text(&tab_with_url(url)), "WS", "{url}");
        }
        assert_eq!(method_text(&tab_with_url("https://example.com/wss://nested")), "GET");
    }

    #[test]
    fn tab_label_prefers_saved_name_then_schemeless_url() {
        let mut tab = tab_with_url("https://api.example.com/users");
        assert_eq!(tab_label(&tab), "api.example.com/users");
        tab.title = "List users".to_owned();
        tab.saved_as = Some(("col".to_owned(), "req".to_owned()));
        assert_eq!(tab_label(&tab), "List users");
        assert_eq!(tab_label(&tab_with_url("")), "New Request");
        let mut unnamed = tab_with_url("https://api.example.com/orders");
        unnamed.saved_as = Some(("col".to_owned(), "req".to_owned()));
        assert_eq!(tab_label(&unnamed), "api.example.com/orders");
        assert_eq!(tab_label(&tab_with_url("https://")), "https://");
    }

    #[test]
    fn tabs_are_between_100_and_180_px_wide() {
        let short = tab_geometry(20.0, 10.0).0;
        let long = tab_geometry(20.0, 900.0).0;
        assert!((short - TAB_MIN).abs() < f32::EPSILON);
        assert!((long - TAB_MAX).abs() < f32::EPSILON);
        let (mid, budget) = tab_geometry(20.0, 60.0);
        assert!(mid > TAB_MIN && mid < TAB_MAX);
        assert!((budget - 60.0).abs() < 1e-3, "a title that fits keeps its width");
    }

    #[test]
    fn long_titles_are_cut_to_fit_and_labels_refresh_only_on_change() {
        let mut tabs = TabManager::default();
        tabs.tabs[0].url = format!("https://api.example.com/{}", "segment/".repeat(20));
        sync_strip_labels(&mut tabs);
        let label = tabs.tabs[0].strip_label.clone();
        assert!(label.cut && label.text.ends_with('…'));
        assert!((label.width - TAB_MAX).abs() < f32::EPSILON);
        sync_strip_labels(&mut tabs);
        assert_eq!(tabs.tabs[0].strip_label, label, "unchanged inputs keep the label");
        tabs.tabs[0].url = "https://a.io".to_owned();
        sync_strip_labels(&mut tabs);
        assert_eq!(tabs.tabs[0].strip_label.text, "a.io");
    }

    #[test]
    fn revealing_scrolls_only_as_far_as_needed() {
        let widths = [150.0, 150.0, 150.0, 150.0];
        assert_eq!(reveal_offset(&widths, 0, 300.0, 0.0), None, "already visible");
        assert_eq!(reveal_offset(&widths, 3, 300.0, 0.0), Some(300.0), "off the right edge");
        assert_eq!(reveal_offset(&widths, 0, 300.0, 200.0), Some(0.0), "off the left edge");
        assert_eq!(reveal_offset(&widths, 1, 300.0, 100.0), None, "fully inside");
        assert_eq!(reveal_offset(&widths, 9, 300.0, 0.0), None, "no such tab");
    }

    #[test]
    fn unsaved_means_edited_or_never_saved() {
        let mut tab = tab_with_url("https://a.io");
        assert!(has_unsaved_changes(&tab), "a typed URL that was never saved");
        tab.saved_as = Some(("c".to_owned(), "r".to_owned()));
        assert!(!has_unsaved_changes(&tab));
        tab.modified = true;
        assert!(has_unsaved_changes(&tab));
        assert!(!has_unsaved_changes(&RequestTabState::new()), "a blank new tab");
    }

    #[test]
    fn the_open_tabs_menu_fits_every_method_with_a_gap() {
        for method in crate::domain::request::HttpMethod::all() {
            let label = Measured::new(method.as_str(), TEXT_CAPTION, Face::Mono).width();
            assert!(*METHOD_COLUMN - label >= SPACE_MD, "{} leaves {:.1}px", method.as_str(), *METHOD_COLUMN - label);
        }
    }
}

#[cfg(test)]
mod tab_label_tests {
    use super::*;

    fn label_for(url: &str) -> String {
        let mut tab = RequestTabState::new();
        tab.set_url(url.to_owned());
        tab_label(&tab).to_owned()
    }

    #[test]
    fn non_ascii_urls_never_panic() {
        for url in ["日本", "дом", "日本語のパス", "ws:/日", "é", "🦀🦀"] {
            assert_eq!(label_for(url), url);
        }
    }

    #[test]
    fn schemes_are_trimmed() {
        assert_eq!(label_for("https://x.io/a"), "x.io/a");
        assert_eq!(label_for("WSS://x.io"), "x.io");
        assert_eq!(label_for("https://日本.jp"), "日本.jp");
    }
}
