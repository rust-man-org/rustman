//! Palette search: everything the palette can show, matched, ranked,
//! deduplicated and capped once per query change.

use std::collections::HashSet;

use crate::domain::history::HistorySummary;
use crate::ui::sidebar::history::split_url;

use super::commands::COMMANDS;

/// Query prefix that restricts the palette to commands, as in VS Code.
pub const COMMAND_PREFIX: char = '>';

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Tabs,
    Requests,
    History,
    Commands,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Tabs => "Tabs",
            Group::Requests => "Requests",
            Group::History => "History",
            Group::Commands => "Commands",
        }
    }

    fn cap(self, browsing: bool) -> usize {
        match (self, browsing) {
            (Group::Tabs, _) => 9,
            (Group::Requests, true) => 2,
            (Group::History, true) => 1,
            (Group::Requests, false) => 20,
            (Group::History, false) => 10,
            (Group::Commands, _) => 10,
        }
    }
}

/// What confirming a hit does. Messages are built only on confirm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An open tab, by id: indices shift as tabs close.
    Tab(String),
    Request(String),
    History(String),
    Command(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub group: Group,
    pub target: Target,
    /// The method shown in the lead column; commands show an icon instead.
    pub method: Option<String>,
    pub label: String,
    /// Muted text after the label: a request's collection path.
    pub sub: String,
    /// History rows show the path as the label and the host muted before it.
    pub host: Option<String>,
    pub end: End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    None,
    Shortcut(String),
    Status { code: u16, ago: String },
}

/// One open tab, as the palette sees it.
pub struct TabItem<'a> {
    pub id: &'a str,
    /// Its place in the strip, for the Alt+N hint.
    pub index: usize,
    pub method: &'a str,
    pub title: &'a str,
    pub url: &'a str,
    pub path: String,
}

/// One saved request.
pub struct RequestItem<'a> {
    pub id: &'a str,
    pub method: &'a str,
    pub name: &'a str,
    pub url: &'a str,
    /// "Collection / Folder".
    pub path: String,
}

pub struct Corpus<'a> {
    /// Every tab but the active one.
    pub tabs: Vec<TabItem<'a>>,
    /// Saved requests open in any tab, the active one included; they are
    /// listed as tabs, or not at all.
    pub open: HashSet<&'a str>,
    pub requests: Vec<RequestItem<'a>>,
    pub history: &'a [HistorySummary],
    pub now_ms: i64,
}

/// The hits for `query`, grouped in display order.
pub fn search(corpus: &Corpus<'_>, query: &str) -> Vec<Hit> {
    let raw = query.trim_start();
    if let Some(rest) = raw.strip_prefix(COMMAND_PREFIX) {
        return commands(&rest.trim().to_lowercase(), false);
    }
    let query = raw.trim().to_lowercase();
    let browsing = query.is_empty();
    let mut hits = tabs(corpus, &query);
    hits.extend(requests(corpus, &query, browsing));
    hits.extend(history(corpus, &query, browsing));
    hits.extend(commands(&query, browsing));
    hits
}

/// How well `text` (lowercase) matches `query` (lowercase): at the start,
/// at a word start, anywhere, or not at all.
fn score(text: &str, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(0);
    }
    let at = text.find(query)?;
    let word_start = at == 0 || !text[..at].chars().next_back().is_some_and(char::is_alphanumeric);
    Some(match (at, word_start) {
        (0, _) => 3,
        (_, true) => 2,
        _ => 1,
    })
}

/// A match on the name outranks any match on secondary text.
fn rank(name: &str, secondary: &[&str], query: &str) -> Option<u8> {
    let named = score(&name.to_lowercase(), query).map(|s| s + 3);
    let other = secondary.iter().filter_map(|s| score(&s.to_lowercase(), query)).max();
    named.max(other)
}

/// Keeps the best `cap` items, best first; ties keep their original order.
fn best<T>(mut ranked: Vec<(u8, T)>, cap: usize) -> impl Iterator<Item = T> {
    ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    ranked.into_iter().take(cap).map(|(_, item)| item)
}

fn tabs(corpus: &Corpus<'_>, query: &str) -> Vec<Hit> {
    let ranked = corpus
        .tabs
        .iter()
        .filter_map(|tab| Some((rank(tab.title, &[tab.url, &tab.path], query)?, tab)))
        .collect();
    best(ranked, Group::Tabs.cap(query.is_empty()))
        .map(|tab| Hit {
            group: Group::Tabs,
            target: Target::Tab(tab.id.to_owned()),
            method: Some(tab.method.to_owned()),
            label: tab.title.to_owned(),
            sub: tab.path.clone(),
            host: None,
            end: if tab.index < 9 {
                End::Shortcut(format!("Alt {}", tab.index + 1))
            } else {
                End::None
            },
        })
        .collect()
}

fn requests(corpus: &Corpus<'_>, query: &str, browsing: bool) -> Vec<Hit> {
    let ranked = corpus
        .requests
        .iter()
        .filter(|r| !corpus.open.contains(r.id))
        .filter_map(|r| Some((rank(r.name, &[r.url, &r.path], query)?, r)))
        .collect();
    best(ranked, Group::Requests.cap(browsing))
        .map(|r| Hit {
            group: Group::Requests,
            target: Target::Request(r.id.to_owned()),
            method: Some(r.method.to_owned()),
            label: r.name.to_owned(),
            sub: r.path.clone(),
            host: None,
            end: End::None,
        })
        .collect()
}

fn history(corpus: &Corpus<'_>, query: &str, browsing: bool) -> Vec<Hit> {
    let mut seen = HashSet::new();
    let ranked = corpus
        .history
        .iter()
        .filter(|e| seen.insert((e.method.as_str(), e.url.as_str())))
        .filter_map(|e| Some((rank(&e.url, &[&e.method], query)?, e)))
        .collect();
    best(ranked, Group::History.cap(browsing))
        .map(|e| {
            let (host, path) = split_url(&e.url);
            Hit {
                group: Group::History,
                target: Target::History(e.id.clone()),
                method: Some(e.method.clone()),
                label: path.to_owned(),
                sub: String::new(),
                host: Some(host.to_owned()),
                end: End::Status {
                    code: u16::try_from(e.status).unwrap_or(0),
                    ago: ago(corpus.now_ms - e.timestamp),
                },
            }
        })
        .collect()
}

fn commands(query: &str, browsing: bool) -> Vec<Hit> {
    let hit = |index: usize| {
        let command = &COMMANDS[index];
        Hit {
            group: Group::Commands,
            target: Target::Command(index),
            method: None,
            label: command.label.to_owned(),
            sub: String::new(),
            host: None,
            end: command.shortcut().map_or(End::None, End::Shortcut),
        }
    };
    if browsing {
        return (0..COMMANDS.len()).filter(|&i| COMMANDS[i].suggested).map(hit).collect();
    }
    let ranked = COMMANDS
        .iter()
        .enumerate()
        .filter_map(|(i, c)| Some((rank(c.label, &[], query)?, i)))
        .collect();
    best(ranked, Group::Commands.cap(false)).map(hit).collect()
}

/// "just now", "50 min ago", "3 h ago", "2 d ago".
pub fn ago(elapsed_ms: i64) -> String {
    let minutes = elapsed_ms.max(0) / 60_000;
    match minutes {
        0 => "just now".to_owned(),
        1..=59 => format!("{minutes} min ago"),
        60..=1439 => format!("{} h ago", minutes / 60),
        _ => format!("{} d ago", minutes / 1440),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, method: &str, url: &str, minutes_ago: i64) -> HistorySummary {
        HistorySummary {
            id: id.into(),
            timestamp: -minutes_ago * 60_000,
            method: method.into(),
            url: url.into(),
            status: 200,
            duration_ms: 1,
        }
    }

    fn request<'a>(id: &'a str, name: &'a str, url: &'a str) -> RequestItem<'a> {
        RequestItem {
            id,
            method: "GET",
            name,
            url,
            path: "Acme API / Users".into(),
        }
    }

    fn labels(hits: &[Hit], group: Group) -> Vec<&str> {
        hits.iter().filter(|h| h.group == group).map(|h| h.label.as_str()).collect()
    }

    #[test]
    fn prefix_beats_word_start_beats_substring() {
        assert_eq!(score("users list", "users"), Some(3));
        assert_eq!(score("list users", "users"), Some(2));
        assert_eq!(score("superusers", "users"), Some(1));
        assert_eq!(score("orders", "users"), None);
        assert!(rank("Who am I", &["/auth/me"], "who") > rank("List users", &["/who/x"], "who"));
    }

    #[test]
    fn groups_come_in_order_and_requests_rank_by_match_quality() {
        let history = [entry("h1", "GET", "http://x/users", 5)];
        let corpus = Corpus {
            tabs: Vec::new(),
            open: HashSet::new(),
            requests: vec![request("1", "Deactivate user", "/users/3"), request("2", "User settings", "/settings")],
            history: &history,
            now_ms: 0,
        };
        let hits = search(&corpus, "user");
        assert_eq!(labels(&hits, Group::Requests), vec!["User settings", "Deactivate user"]);
        let groups: Vec<Group> = hits.iter().map(|h| h.group).collect();
        assert!(groups.windows(2).all(|w| w[0] <= w[1]), "{groups:?}");
    }

    #[test]
    fn open_tabs_and_repeated_history_are_listed_once() {
        let history = [
            entry("h1", "GET", "http://x/auth/me", 1),
            entry("h2", "GET", "http://x/auth/me", 9),
            entry("h3", "POST", "http://x/auth/me", 10),
        ];
        let corpus = Corpus {
            tabs: vec![TabItem {
                id: "tab-1",
                index: 1,
                method: "GET",
                title: "Who am I",
                url: "http://x/auth/me",
                path: String::new(),
            }],
            open: HashSet::from(["1"]),
            requests: vec![request("1", "Who am I", "/auth/me"), request("2", "Login", "/auth/login")],
            history: &history,
            now_ms: 0,
        };
        let hits = search(&corpus, "auth");
        assert_eq!(labels(&hits, Group::Tabs), vec!["Who am I"]);
        assert_eq!(labels(&hits, Group::Requests), vec!["Login"], "the open request is only a tab");
        let history: Vec<&Target> = hits.iter().filter(|h| h.group == Group::History).map(|h| &h.target).collect();
        assert_eq!(history, vec![&Target::History("h1".into()), &Target::History("h3".into())]);
        assert_eq!(hits[0].end, End::Shortcut("Alt 2".into()));
    }

    #[test]
    fn history_rows_lead_with_the_path() {
        let history = [entry("h1", "GET", "http://127.0.0.1:18931/auth/me", 50)];
        let corpus = Corpus {
            tabs: Vec::new(),
            open: HashSet::new(),
            requests: Vec::new(),
            history: &history,
            now_ms: 0,
        };
        let hit = search(&corpus, "auth").into_iter().find(|h| h.group == Group::History).unwrap();
        assert_eq!(hit.label, "/auth/me");
        assert_eq!(hit.host.as_deref(), Some("127.0.0.1:18931"));
        assert_eq!(hit.end, End::Status { code: 200, ago: "50 min ago".into() });
    }

    #[test]
    fn the_command_prefix_lists_only_commands() {
        let corpus = Corpus {
            tabs: Vec::new(),
            open: HashSet::new(),
            requests: vec![request("1", "New thing", "/new")],
            history: &[],
            now_ms: 0,
        };
        let hits = search(&corpus, "> new");
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|h| h.group == Group::Commands));
        assert_eq!(hits[0].label, "New Tab");
    }

    #[test]
    fn ages_round_down_to_the_largest_unit() {
        assert_eq!(ago(30_000), "just now");
        assert_eq!(ago(50 * 60_000), "50 min ago");
        assert_eq!(ago(3 * 3_600_000 + 1), "3 h ago");
        assert_eq!(ago(50 * 3_600_000), "2 d ago");
    }
}
