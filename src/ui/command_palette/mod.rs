//! The command palette: one search over open tabs, saved requests, history
//! and commands. Results are computed in update; the view only lays them out.

pub mod commands;
pub mod search;

use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{Message, PaletteMsg};
use crate::ui::components::Surface;
use crate::ui::components::kbd::{KeyCap, kbd};
use crate::ui::components::menu::panel_style;
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_XL, HAIRLINE, ICON_LG, ICON_MD, MONO, RADIUS_SM, SPACE_LG, SPACE_MD, SPACE_XL, SPACE_XS,
    TEXT_CAPTION, TEXT_LG, TEXT_MD, TEXT_SM, Tokens, UI_FONT_MEDIUM,
};

use search::{End, Group, Hit};

use crate::ui::tree_label::{Face, Measured};

/// Inner width of a key cap beyond its label: padding and border.
const KBD_CHROME: f32 = 2.0 * (SPACE_XS + 2.0) + 2.0 * HAIRLINE;
/// A sub or host cut shorter than this says nothing; it is left out.
const MIN_KEPT_CHARS: usize = 4;

/// Cuts every hit's text to its row, once per search, so each row is one
/// line however long a name or URL is: labels lead and are cut with "…"
/// only when they alone overflow; the muted text fills what is left.
pub fn fit_rows(hits: &mut [Hit]) {
    for hit in hits {
        let room = text_room(&hit.end);
        if let Some(host) = hit.host.take() {
            // History keeps the start of the URL: the host, then the path.
            let host = Measured::new(host, TEXT_SM, Face::Mono);
            if let Some(cut) = host.fitted(room) {
                hit.host = Some(cut);
                hit.label.clear();
            } else {
                let left = room - host.width();
                hit.host = Some(host.into_text());
                fit(&mut hit.label, TEXT_SM, Face::Mono, left);
            }
            continue;
        }
        let label = Measured::new(std::mem::take(&mut hit.label), TEXT_MD, Face::Regular);
        if let Some(cut) = label.fitted(room) {
            hit.label = cut;
            hit.sub.clear();
            continue;
        }
        let left = room - label.width() - SPACE_MD;
        hit.label = label.into_text();
        fit(&mut hit.sub, TEXT_SM, Face::Regular, left);
    }
}

/// `text` cut to `room` px, or emptied when too little of it would show.
fn fit(text: &mut String, size: f32, face: Face, room: f32) {
    let measured = Measured::new(std::mem::take(text), size, face);
    *text = match measured.fitted(room) {
        None => measured.into_text(),
        Some(cut) if cut.chars().count() > MIN_KEPT_CHARS => cut,
        Some(_) => String::new(),
    };
}

/// What a row leaves for its texts: the card less the row padding, the
/// lead column and the trailing items, with room for the Enter key cap a
/// selected row shows in place of a shortcut.
fn text_room(end: &End) -> f32 {
    let cap = |label: &str| Measured::new(label, TEXT_CAPTION, Face::Regular).width() + KBD_CHROME;
    let enter = TEXT_CAPTION + KBD_CHROME;
    let trailing = match end {
        End::None => enter,
        End::Shortcut(keys) => cap(keys).max(enter),
        End::Status { code, ago } => {
            let status = if *code == 0 { "Error".to_owned() } else { code.to_string() };
            let texts = Measured::new(status, TEXT_SM, Face::Regular).width() + Measured::new(ago.as_str(), TEXT_SM, Face::Regular).width();
            texts + 2.0 * SPACE_LG + enter
        }
    };
    WIDTH - 2.0 * HAIRLINE - 2.0 * SPACE_LG - LEAD - SPACE_LG - trailing
}

pub const SEARCH_ID: &str = "palette-search";
pub const LIST_ID: &str = "palette-list";
pub const WIDTH: f32 = 560.0;
/// The tallest the palette gets; longer result lists scroll.
pub const MAX_HEIGHT: f32 = 420.0;
const INPUT_HEIGHT: f32 = 44.0;
pub const ROW: f32 = CONTROL_XL;
pub const HEADER: f32 = 24.0;
/// The list's own vertical padding.
pub const LIST_PAD: f32 = SPACE_XS;
/// Room for the method or icon in front of each label.
const LEAD: f32 = 52.0;
/// Distance from the top of the window.
const TOP: f32 = 88.0;

/// Results of the current query, computed when it changes.
#[derive(Debug, Clone, Default)]
pub struct Results {
    pub hits: Vec<Hit>,
    /// Vertical scroll offset of the list, to keep the selection in view.
    pub scroll_y: f32,
}

/// The list's visible height: everything under the search field and its
/// rule, inside the card's border.
pub const fn list_viewport() -> f32 {
    MAX_HEIGHT - INPUT_HEIGHT - 3.0 * HAIRLINE
}

/// Top and bottom of hit `index` inside the list, headers included.
pub fn row_span(hits: &[Hit], index: usize) -> (f32, f32) {
    let mut y = LIST_PAD;
    let mut group = None;
    for (i, hit) in hits.iter().enumerate() {
        if group != Some(hit.group) {
            y += HEADER;
            group = Some(hit.group);
        }
        if i == index {
            return (y, y + ROW);
        }
        y += ROW;
    }
    (y, y)
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let results = &state.palette_results;
    let mut card = column![input(state, t)];
    if !results.hits.is_empty() || !state.palette_query.trim().is_empty() {
        card = card.push(rule(t)).push(list(t, &results.hits, state.palette_selected));
    }
    let card = container(card).width(WIDTH).padding(HAIRLINE).style(move |_| panel_style(t));
    crate::ui::widgets::overlay::modal_layer(card, Message::Palette(PaletteMsg::Close), move |card| {
        container(column![Space::new().height(TOP), row![Space::new().width(Length::Fill), card, Space::new().width(Length::Fill)]])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(t.scrim)),
                ..Default::default()
            })
            .into()
    })
}

fn input<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let muted = t.on_raised.text_muted;
    let field = text_input("Search requests, tabs, history and commands", &state.palette_query)
        .id(SEARCH_ID)
        .on_input(|q| Message::Palette(PaletteMsg::QueryChanged(q)))
        .on_submit(Message::Palette(PaletteMsg::Confirm))
        .size(TEXT_LG)
        .padding(0)
        .style(move |_, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: muted,
            placeholder: muted,
            value: t.text,
            selection: crate::ui::components::field::selection(t),
        });
    container(
        row![icons::search().size(ICON_LG).color(muted), field, kbd(t, KeyCap::Text("Esc"), Surface::Raised)]
            .spacing(SPACE_LG)
            .align_y(Alignment::Center),
    )
    .height(INPUT_HEIGHT)
    .padding(Padding {
        left: SPACE_XL,
        right: SPACE_LG,
        ..Padding::ZERO
    })
    .align_y(Alignment::Center)
    .into()
}

fn rule<'a>(t: &'static Tokens) -> Element<'a, Message> {
    container(Space::new().width(Length::Fill).height(crate::ui::theme::HAIRLINE))
        .style(move |_| container::Style {
            background: Some(Background::Color(t.on_raised.border)),
            ..Default::default()
        })
        .into()
}

fn list<'a>(t: &'static Tokens, hits: &'a [Hit], selected: usize) -> Element<'a, Message> {
    if hits.is_empty() {
        return container(text("No matches").size(TEXT_SM).color(t.on_raised.text_muted))
            .padding([SPACE_XL, SPACE_XL])
            .into();
    }
    let mut rows = column![].padding([LIST_PAD, 0.0]);
    let mut group = None;
    for (i, hit) in hits.iter().enumerate() {
        if group != Some(hit.group) {
            rows = rows.push(header(t, hit.group));
            group = Some(hit.group);
        }
        rows = rows.push(hit_row(t, i, hit, i == selected));
    }
    container(
        crate::ui::components::scroll::vertical(t, rows)
            .id(LIST_ID)
            .on_scroll(|v| Message::Palette(PaletteMsg::Scrolled(v.absolute_offset().y))),
    )
    .max_height(list_viewport())
    .into()
}

fn header<'a>(t: &'static Tokens, group: Group) -> Element<'a, Message> {
    container(text(group.title()).size(TEXT_CAPTION).font(UI_FONT_MEDIUM).color(t.on_raised.text_muted))
        .height(HEADER)
        .padding(Padding {
            left: SPACE_XL,
            bottom: SPACE_XS,
            ..Padding::ZERO
        })
        .align_y(Alignment::End)
        .into()
}

fn hit_row<'a>(t: &'static Tokens, index: usize, hit: &'a Hit, selected: bool) -> Element<'a, Message> {
    let r = &t.on_raised;
    let lead: Element<'a, Message> = match (&hit.method, &hit.target) {
        (Some(method), _) => text(method.as_str())
            .size(TEXT_CAPTION)
            .font(MONO)
            .color(r.methods.for_method(method))
            .into(),
        (None, search::Target::Command(i)) => (commands::COMMANDS[*i].icon)().size(ICON_MD).color(r.text_muted).into(),
        (None, _) => Space::new().into(),
    };
    let mut content = row![container(lead).width(LEAD)].align_y(Alignment::Center);
    if let Some(host) = &hit.host {
        content = content.push(
            row![
                one_line(text(host.as_str()).size(TEXT_SM).font(MONO).color(r.text_muted)),
                one_line(text(hit.label.as_str()).size(TEXT_SM).font(MONO).color(t.text)),
            ]
            .align_y(Alignment::Center),
        );
    } else {
        content = content.push(one_line(text(hit.label.as_str()).size(TEXT_MD).color(t.text)));
    }
    if !hit.sub.is_empty() {
        content = content.push(Space::new().width(SPACE_MD)).push(
            text(hit.sub.as_str())
                .size(TEXT_SM)
                .color(r.text_muted)
                .wrapping(iced_core::text::Wrapping::None),
        );
    }
    content = content.push(Space::new().width(Length::Fill)).push(end(t, &hit.end, selected));
    let button = iced::widget::button(container(content).center_y(Length::Fill))
        .on_press(Message::Palette(PaletteMsg::ConfirmAt(index)))
        .width(Length::Fill)
        .height(ROW)
        .padding([0.0, SPACE_LG])
        .style(move |_, status| {
            use iced::widget::button::Status;
            let fill = if selected {
                Some(r.selected)
            } else {
                matches!(status, Status::Hovered | Status::Pressed).then_some(r.hover)
            };
            iced::widget::button::Style {
                background: fill.map(Background::Color),
                text_color: t.text,
                border: Border {
                    radius: RADIUS_SM.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });
    container(button).padding([0.0, SPACE_XS]).into()
}

fn end<'a>(t: &'static Tokens, end: &'a End, selected: bool) -> Element<'a, Message> {
    let r = &t.on_raised;
    let mut items = row![].spacing(SPACE_LG).align_y(Alignment::Center).padding(Padding {
        left: SPACE_LG,
        ..Padding::ZERO
    });
    if let End::Status { code, ago } = end {
        let status = if *code == 0 { "Error".to_owned() } else { code.to_string() };
        items = items
            .push(text(status).size(TEXT_SM).color(raised_status(t, *code)))
            .push(text(ago.as_str()).size(TEXT_SM).color(r.text_muted));
    }
    if selected {
        items = items.push(kbd(t, KeyCap::Icon(icons::ENTER_KEY), Surface::Raised));
    } else if let End::Shortcut(keys) = end {
        items = items.push(kbd(t, KeyCap::Text(keys), Surface::Raised));
    }
    items.into()
}

fn one_line(label: iced::widget::Text<'_>) -> iced::widget::Text<'_> {
    label.wrapping(iced_core::text::Wrapping::None)
}

/// Status colors lifted for the raised surface.
fn raised_status(t: &Tokens, code: u16) -> Color {
    let s = &t.on_raised.status;
    match code {
        200..=299 => s.success,
        300..=399 => t.accent_text,
        400..=499 => s.client_error,
        _ => s.error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use search::{Target, Hit};

    fn hit(group: Group) -> Hit {
        Hit {
            group,
            target: Target::Command(0),
            method: None,
            label: String::new(),
            sub: String::new(),
            host: None,
            end: End::None,
        }
    }

    #[test]
    fn row_spans_count_the_group_headers_above_them() {
        let hits = [hit(Group::Tabs), hit(Group::Tabs), hit(Group::Commands)];
        assert_eq!(row_span(&hits, 0), (LIST_PAD + HEADER, LIST_PAD + HEADER + ROW));
        assert_eq!(row_span(&hits, 2).0, LIST_PAD + 2.0 * HEADER + 2.0 * ROW);
    }

    fn width(text: &str, size: f32, face: Face) -> f32 {
        Measured::new(text, size, face).width()
    }

    #[test]
    fn a_long_history_url_fits_one_line_and_keeps_its_start() {
        let end = End::Status { code: 200, ago: "just now".into() };
        let path = format!("/v1/{}", "segment/".repeat(37));
        let mut hits = vec![Hit {
            group: Group::History,
            host: Some("api.example.com".into()),
            label: path.clone(),
            end: end.clone(),
            ..hit(Group::History)
        }];
        fit_rows(&mut hits);
        let row = &hits[0];
        let host = row.host.as_deref().unwrap();
        assert_eq!(host, "api.example.com", "the start of the URL stays");
        assert!(row.label.starts_with("/v1/segment/") && row.label.ends_with('…'), "{}", row.label);
        let used = width(host, TEXT_SM, Face::Mono) + width(&row.label, TEXT_SM, Face::Mono);
        assert!(used <= text_room(&end), "{used} px in {} px", text_room(&end));
    }

    #[test]
    fn long_names_are_cut_and_their_muted_text_gives_way_first() {
        let name = "Create a payment intent for the quarterly enterprise invoice batch run";
        let mut hits = vec![
            Hit { label: name.repeat(3), sub: "Payments".into(), ..hit(Group::Requests) },
            Hit { label: "Login".into(), sub: "Auth Service / ".repeat(20), ..hit(Group::Requests) },
        ];
        fit_rows(&mut hits);
        assert!(hits[0].label.ends_with('…') && hits[0].sub.is_empty());
        assert_eq!(hits[1].label, "Login");
        assert!(hits[1].sub.ends_with('…'));
        for row in &hits {
            let used = width(&row.label, TEXT_MD, Face::Regular) + SPACE_MD + width(&row.sub, TEXT_SM, Face::Regular);
            assert!(used <= text_room(&row.end) + SPACE_MD);
        }
    }
}
