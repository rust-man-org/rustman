//! History rows prepared for display: grouped by day, URLs split so the
//! path leads and the host can be muted.

use chrono::{Datelike, Local, NaiveDate};

use crate::app::AppState;
use crate::domain::history::HistorySummary;
use crate::ui::sidebar::collections::METHOD_COLUMN;
use crate::ui::theme::{SPACE_MD, TEXT_SM};
use crate::ui::tree_label::{Face, Measured};

/// A host cut shorter than this says nothing; it is left out instead.
const MIN_HOST_CHARS: usize = 4;

/// History rows ready to draw, rebuilt only when the list, the day or the
/// sidebar width changes.
#[derive(Debug, Clone, Default)]
pub struct HistoryCache {
    key: Option<Key>,
    lines: Vec<Line>,
}

/// History only grows at the front and shrinks from the back or all at
/// once, so its length and both ends identify its content.
#[derive(Debug, Clone, PartialEq)]
struct Key {
    len: usize,
    newest: String,
    oldest: String,
    today: NaiveDate,
    width_bits: u32,
}

impl Key {
    fn matches(&self, history: &[HistorySummary], today: NaiveDate, width: f32) -> bool {
        self.len == history.len()
            && history.first().is_some_and(|e| e.id == self.newest)
            && history.last().is_some_and(|e| e.id == self.oldest)
            && self.today == today
            && self.width_bits == width.to_bits()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Day(String),
    Entry(EntryLine),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryLine {
    pub id: String,
    pub method: String,
    /// Shown muted after the path, as far as it fits.
    pub host: Option<String>,
    pub path: String,
    pub status: String,
    pub code: u16,
    /// Full URL, status, duration and time, for the tooltip.
    pub summary: String,
}

impl HistoryCache {
    pub fn sync(&mut self, state: &AppState) -> &[Line] {
        let today = Local::now().date_naive();
        let width = state.sidebar_width;
        if !self.key.as_ref().is_some_and(|k| k.matches(&state.history, today, width)) {
            self.lines = lines(&state.history, today, width);
            self.key = Some(Key {
                len: state.history.len(),
                newest: state.history.first().map(|e| e.id.clone()).unwrap_or_default(),
                oldest: state.history.last().map(|e| e.id.clone()).unwrap_or_default(),
                today,
                width_bits: width.to_bits(),
            });
        }
        &self.lines
    }
}

fn lines(history: &[HistorySummary], today: NaiveDate, width: f32) -> Vec<Line> {
    sections(history, today)
        .into_iter()
        .map(|item| match item {
            Item::Day(label) => Line::Day(label),
            Item::Entry(i) => Line::Entry(entry_line(&history[i], width)),
        })
        .collect()
}

fn entry_line(entry: &HistorySummary, width: f32) -> EntryLine {
    let code = u16::try_from(entry.status).unwrap_or(0);
    let status = if code == 0 { "Error".to_owned() } else { code.to_string() };
    let (host, path) = split_url(&entry.url);
    let status_width = Measured::new(status.as_str(), TEXT_SM, Face::Medium).width();
    let budget = width - 3.0 * SPACE_MD - *METHOD_COLUMN - status_width;
    let (host, path) = fit_url(host, path, budget);
    let time = chrono::DateTime::from_timestamp_millis(entry.timestamp)
        .map(|t| t.with_timezone(&Local).format("%H:%M").to_string())
        .unwrap_or_default();
    EntryLine {
        id: entry.id.clone(),
        method: entry.method.clone(),
        host,
        path,
        summary: format!("{} {}\n{status} · {} · {time}", entry.method, entry.url, duration(entry.duration_ms)),
        status,
        code,
    }
}

/// Path and host at mono [`TEXT_SM`] in `budget` px. The path leads and is
/// cut only when it alone overflows; the host fills what room is left.
fn fit_url(host: &str, path: &str, budget: f32) -> (Option<String>, String) {
    let path = Measured::new(path, TEXT_SM, Face::Mono);
    if let Some(cut) = path.fitted(budget) {
        return (None, cut);
    }
    let room = budget - path.width() - SPACE_MD;
    let host = Measured::new(host, TEXT_SM, Face::Mono);
    let host = match host.fitted(room) {
        None => Some(host.into_text()),
        Some(cut) if cut.chars().count() > MIN_HOST_CHARS => Some(cut),
        Some(_) => None,
    };
    (host, path.into_text())
}

/// `url` without its scheme, split into host (with port) and the rest.
/// The rest always starts with `/`, `?` or `#` unless the URL is empty.
pub fn split_url(url: &str) -> (&str, &str) {
    let bare = url.split_once("://").map_or(url, |(_, rest)| rest);
    let cut = bare.find(['/', '?', '#']).unwrap_or(bare.len());
    let (host, rest) = bare.split_at(cut);
    if rest.is_empty() && !host.is_empty() {
        (host, "/")
    } else {
        (host, rest)
    }
}

/// "Today", "Yesterday", a weekday within the last week, else the date.
pub fn day_label(day: NaiveDate, today: NaiveDate) -> String {
    match (today - day).num_days() {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        2..=6 => day.format("%A").to_string(),
        _ if day.year() == today.year() => day.format("%b %-d").to_string(),
        _ => day.format("%b %-d, %Y").to_string(),
    }
}

/// "842 ms", "1.24 s", "2m 05s".
pub fn duration(ms: i64) -> String {
    let ms = ms.max(0);
    if ms < 1000 {
        format!("{ms} ms")
    } else if ms < 60_000 {
        format!("{}.{:02} s", ms / 1000, ms % 1000 / 10)
    } else {
        format!("{}m {:02}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

pub fn local_day(timestamp_ms: i64) -> Option<NaiveDate> {
    chrono::DateTime::from_timestamp_millis(timestamp_ms).map(|t| t.with_timezone(&Local).date_naive())
}

/// One entry, or the header that starts a day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Day(String),
    Entry(usize),
}

/// The list, newest first, with a header before each new day. Entries
/// without a valid timestamp fall under the day of the entry before them.
pub fn sections(entries: &[HistorySummary], today: NaiveDate) -> Vec<Item> {
    let mut items = Vec::with_capacity(entries.len() + 8);
    let mut current = None;
    for (i, entry) in entries.iter().enumerate() {
        let day = local_day(entry.timestamp).or(current);
        if day != current {
            if let Some(day) = day {
                items.push(Item::Day(day_label(day, today)));
            }
            current = day;
        }
        items.push(Item::Entry(i));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn urls_split_into_host_and_path() {
        assert_eq!(split_url("http://127.0.0.1:18931/auth/me"), ("127.0.0.1:18931", "/auth/me"));
        assert_eq!(split_url("https://api.acme.com/users?page=1"), ("api.acme.com", "/users?page=1"));
        assert_eq!(split_url("https://api.acme.com"), ("api.acme.com", "/"));
        assert_eq!(split_url("api.acme.com?x=1"), ("api.acme.com", "?x=1"));
        assert_eq!(split_url("{{base_url}}/users"), ("{{base_url}}", "/users"));
        assert_eq!(split_url(""), ("", ""));
    }

    #[test]
    fn days_are_labelled_relative_to_today() {
        let today = date(2026, 11, 12);
        assert_eq!(day_label(today, today), "Today");
        assert_eq!(day_label(date(2026, 11, 11), today), "Yesterday");
        assert_eq!(day_label(date(2026, 11, 9), today), "Monday");
        assert_eq!(day_label(date(2026, 3, 3), today), "Mar 3");
        assert_eq!(day_label(date(2025, 12, 31), today), "Dec 31, 2025");
    }

    #[test]
    fn the_path_leads_and_the_host_takes_what_room_is_left() {
        let char_px = Measured::new("x", TEXT_SM, Face::Mono).width();
        let both = fit_url("api.acme.com", "/users", 6.0 * char_px + SPACE_MD + 12.0 * char_px + 1.0);
        assert_eq!(both, (Some("api.acme.com".into()), "/users".into()));
        let (host, path) = fit_url("api.acme.com", "/users", 6.0 * char_px + SPACE_MD + 7.0 * char_px);
        assert_eq!(path, "/users");
        assert!(host.is_some_and(|h| h.starts_with("api.") && h.ends_with('…')));
        assert_eq!(fit_url("api.acme.com", "/users", 6.0 * char_px + 1.0), (None, "/users".into()));
        let (host, cut) = fit_url("api.acme.com", "/users/42/orders", 8.0 * char_px);
        assert!(host.is_none() && cut.starts_with("/users") && cut.ends_with('…'));
    }

    #[test]
    fn durations_use_readable_units() {
        assert_eq!(duration(0), "0 ms");
        assert_eq!(duration(842), "842 ms");
        assert_eq!(duration(1240), "1.24 s");
        assert_eq!(duration(125_000), "2m 05s");
        assert_eq!(duration(-5), "0 ms");
    }

    #[test]
    fn each_new_day_gets_one_header() {
        let at = |d: u32, h: u32| {
            chrono::TimeZone::with_ymd_and_hms(&Local, 2026, 11, d, h, 0, 0)
                .unwrap()
                .timestamp_millis()
        };
        let entry = |timestamp| HistorySummary {
            id: String::new(),
            timestamp,
            method: "GET".into(),
            url: String::new(),
            status: 200,
            duration_ms: 1,
        };
        let entries = [entry(at(12, 15)), entry(at(12, 9)), entry(i64::MAX), entry(at(10, 9))];
        assert_eq!(
            sections(&entries, date(2026, 11, 12)),
            vec![
                Item::Day("Today".into()),
                Item::Entry(0),
                Item::Entry(1),
                Item::Entry(2),
                Item::Day("Tuesday".into()),
                Item::Entry(3),
            ]
        );
    }
}
