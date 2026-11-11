pub use rustman_core::text::{head_capped, pretty_json};
pub use rustman_formats::html::html_preview_text;

// ── Response body search ──────────────────────────────────────────────────────
//
// The response viewer pages large bodies (see `state::tabs::page_starts`), so
// the editor only ever holds one page and its built-in Ctrl+F can only see
// that page. Search therefore runs here, over the whole display text, and the
// viewer is driven to whichever page holds the match. Everything below is
// pure (no widgets) so it can be tested directly.

/// Most matches one body search records. Past this the counter reads "N+"
/// and stepping cycles through the first N — enough for any real lookup,
/// while a one-letter query over a 30 MB body stays a bounded scan with a
/// bounded allocation instead of millions of ranges.
pub const MAX_SEARCH_MATCHES: usize = 50_000;

/// Longest query the find bar keeps, in bytes. A query is shaped by a text
/// input every frame, so a pasted megabyte must not be stored (see
/// `URL_GUARD_LEN` for what that costs); no real search needs more.
pub const MAX_SEARCH_QUERY_BYTES: usize = 1024;

/// Every non-overlapping occurrence of `query` in `text`, as ascending byte
/// ranges, plus whether the scan stopped at `cap` with more matches left.
///
/// Case-insensitive search folds ASCII byte-wise (fast path, no allocation)
/// and anything else with Unicode lowercase, char by char. Ranges are always
/// on `char` boundaries of `text`, so slicing with them never panics.
pub fn find_matches(
    text: &str,
    query: &str,
    case_sensitive: bool,
    cap: usize,
) -> (Vec<std::ops::Range<usize>>, bool) {
    let mut out = Vec::new();
    if query.is_empty() || cap == 0 {
        return (out, false);
    }
    let capped = if case_sensitive {
        find_exact(text, query, cap, &mut out)
    } else if query.is_ascii() {
        find_ascii_folded(
            text.as_bytes(),
            query.to_ascii_lowercase().as_bytes(),
            cap,
            &mut out,
        )
    } else {
        find_unicode_folded(text, query, cap, &mut out)
    };
    (out, capped)
}

/// Each finder pushes ranges into `out` and returns `true` when it found a
/// match past `cap` (i.e. the result is incomplete).
fn find_exact(
    text: &str,
    query: &str,
    cap: usize,
    out: &mut Vec<std::ops::Range<usize>>,
) -> bool {
    for (start, _) in text.match_indices(query) {
        if out.len() == cap {
            return true;
        }
        out.push(start..start + query.len());
    }
    false
}

/// `needle` is already ASCII-lowercased. ASCII bytes never occur inside a
/// multi-byte UTF-8 sequence, so every hit starts and ends on a char boundary.
///
/// Candidates are found by skimming for the needle's *rarest* byte rather
/// than its first: queries like `"name"` start with bytes that occur every
/// few characters of a JSON body, which would make nearly every position a
/// candidate to verify. Candidates still come in ascending order, so the
/// result is the same left-to-right, non-overlapping set.
fn find_ascii_folded(
    hay: &[u8],
    needle: &[u8],
    cap: usize,
    out: &mut Vec<std::ops::Range<usize>>,
) -> bool {
    let n = needle.len();
    if n == 0 || n > hay.len() {
        return false;
    }
    let k = (0..n).min_by_key(|&i| byte_commonness(needle[i])).unwrap_or(0);
    let (lo, up) = (needle[k], needle[k].to_ascii_uppercase());
    // Position of the rare byte for the last possible match start.
    let last = hay.len() - n + k;
    let mut i = k;
    while i <= last {
        let Some(off) = find_either_byte(&hay[i..=last], lo, up) else {
            break;
        };
        let hit = i + off;
        let p = hit - k;
        if hay[p..p + n].eq_ignore_ascii_case(needle) {
            if out.len() == cap {
                return true;
            }
            out.push(p..p + n);
            i = p + n + k;
        } else {
            i = hit + 1;
        }
    }
    false
}

/// Rough frequency of a lowercase ASCII byte in response bodies (JSON,
/// text, markup); higher is more common. Only used to pick which needle byte
/// to skim for, so a coarse ranking is plenty.
fn byte_commonness(b: u8) -> u8 {
    match b {
        b' ' | b'"' | b'\n' => 255,
        b'e' | b':' | b',' => 250,
        b't' | b'a' | b'o' | b'i' | b'n' | b's' | b'r' => 240,
        b'0'..=b'9' | b'{' | b'}' | b'l' | b'c' | b'd' | b'u' | b'h' | b'm' => 200,
        b'a'..=b'z' | b'\t' | b'\r' | b'-' | b'_' | b'.' | b'/' => 120,
        _ => 60,
    }
}

/// Position of the first `a` or `b` byte. Scans in fixed chunks with a
/// branch-free test per chunk (which the compiler vectorizes) so a
/// multi-megabyte body is skimmed at memory speed rather than byte by byte.
fn find_either_byte(hay: &[u8], a: u8, b: u8) -> Option<usize> {
    const CHUNK: usize = 32;
    let mut base = 0;
    let mut chunks = hay.chunks_exact(CHUNK);
    for chunk in &mut chunks {
        let hit = chunk.iter().fold(false, |acc, &x| acc | (x == a) | (x == b));
        if hit {
            return chunk.iter().position(|&x| x == a || x == b).map(|p| base + p);
        }
        base += CHUNK;
    }
    chunks
        .remainder()
        .iter()
        .position(|&x| x == a || x == b)
        .map(|p| base + p)
}

/// Unicode case-insensitive search: both sides compared as lowercase chars.
fn find_unicode_folded(
    text: &str,
    query: &str,
    cap: usize,
    out: &mut Vec<std::ops::Range<usize>>,
) -> bool {
    let folded: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let Some(&first) = folded.first() else {
        return false;
    };
    if first.is_ascii() {
        // Skim for the ASCII first char byte-wise (an ASCII byte is always a
        // char start, so every hit and `hit + 1` are char boundaries), and
        // verify each hit with the full fold.
        let bytes = text.as_bytes();
        let (lo, up) = (first as u8, (first as u8).to_ascii_uppercase());
        let mut i = 0;
        while let Some(off) = find_either_byte(&bytes[i..], lo, up) {
            let p = i + off;
            match folded_match_end(text, p, &folded) {
                Some(end) => {
                    if out.len() == cap {
                        return true;
                    }
                    out.push(p..end);
                    i = end;
                }
                None => i = p + 1,
            }
        }
        return false;
    }
    let mut next_free = 0;
    for (p, c) in text.char_indices() {
        if p < next_free || c.to_lowercase().next() != Some(first) {
            continue;
        }
        if let Some(end) = folded_match_end(text, p, &folded) {
            if out.len() == cap {
                return true;
            }
            out.push(p..end);
            next_free = end;
        }
    }
    false
}

/// End byte of a case-folded match of `folded` starting at `start`, if any.
/// A char whose lowercase expands past the end of the query still counts
/// whole, so the end is always a char boundary.
fn folded_match_end(text: &str, start: usize, folded: &[char]) -> Option<usize> {
    let mut qi = 0;
    for (off, c) in text[start..].char_indices() {
        for lc in c.to_lowercase() {
            if qi == folded.len() {
                break;
            }
            if lc != folded[qi] {
                return None;
            }
            qi += 1;
        }
        if qi == folded.len() {
            return Some(start + off + c.len_utf8());
        }
    }
    None
}

/// Where a byte offset of the full viewer text lands in the paged viewer:
/// the page, and the 0-based line and column *within that page's buffer* —
/// the coordinates the code editor works in (columns count `char`s, lines
/// split on `\n` only, like the editor's own buffer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagePosition {
    pub page: usize,
    pub line: usize,
    pub col: usize,
}

/// Map `offset` in `text` to its [`PagePosition`], given the page start
/// offsets (`state::tabs::page_starts`). Offsets are clamped and floored to
/// a char boundary, so any input is safe. A page can begin mid-line (pages
/// cut at a byte budget), in which case its line 0 starts at the page start.
pub fn locate_in_pages(text: &str, page_starts: &[usize], offset: usize) -> PagePosition {
    let offset = head_capped(text, offset).len();
    let page = page_starts.partition_point(|&s| s <= offset).saturating_sub(1);
    let page_start = page_starts.get(page).copied().unwrap_or(0).min(offset);
    let before = &text.as_bytes()[page_start..offset];
    let line = before.iter().filter(|&&b| b == b'\n').count();
    let line_start = before
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(page_start, |i| page_start + i + 1);
    PagePosition {
        page,
        line,
        col: text[line_start..offset].chars().count(),
    }
}

/// Where to select a match in the paged viewer: the position of its start,
/// and how many `char`s of it to select from there. A match that straddles a
/// page break (pages cut at a byte budget, so this happens) belongs to the
/// page it starts on and is selected up to that page's end — the rest is
/// simply not in that page's buffer.
pub fn match_selection(
    text: &str,
    page_starts: &[usize],
    range: std::ops::Range<usize>,
) -> (PagePosition, usize) {
    let pos = locate_in_pages(text, page_starts, range.start);
    let page_end = page_starts
        .get(pos.page + 1)
        .copied()
        .unwrap_or(text.len())
        .min(text.len());
    let start = head_capped(text, range.start).len();
    let end = head_capped(text, range.end.min(page_end)).len().max(start);
    (pos, text[start..end].chars().count())
}

/// Byte offset within `page` of editor position (`line`, `col`) — the inverse
/// of [`locate_in_pages`] for one page. Out-of-range positions clamp to the
/// end of the line / page, like the editor's own cursor does.
pub fn offset_in_page(page: &str, line: usize, col: usize) -> usize {
    let bytes = page.as_bytes();
    let mut line_start = 0;
    for _ in 0..line {
        match bytes[line_start..].iter().position(|&b| b == b'\n') {
            Some(i) => line_start += i + 1,
            None => return page.len(),
        }
    }
    let rest = &page[line_start..];
    let line_text = &rest[..rest.find('\n').unwrap_or(rest.len())];
    line_start
        + line_text
            .char_indices()
            .nth(col)
            .map_or(line_text.len(), |(i, _)| i)
}

/// Counter text for the find bar: "3 / 41", "41 results" (no current match
/// yet), "No results", with "+" when the scan was capped.
pub fn match_counter(current: Option<usize>, count: usize, capped: bool) -> String {
    let plus = if capped { "+" } else { "" };
    match current {
        _ if count == 0 => "No results".to_owned(),
        Some(i) => format!(
            "{} / {}{plus}",
            group_thousands(i + 1),
            group_thousands(count)
        ),
        None if count == 1 && !capped => "1 result".to_owned(),
        None => format!("{}{plus} results", group_thousands(count)),
    }
}

fn group_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Find-bar state for one response viewer: the query, its matches over the
/// whole display text (not just the visible page), and which one is current.
///
/// Matches are computed when the query, case mode or text changes — never
/// while rendering — and stepping is index arithmetic over them.
#[derive(Debug, Default)]
pub struct BodySearch {
    /// Whether the find bar is showing.
    pub open: bool,
    pub query: String,
    pub case_sensitive: bool,
    /// Ascending, non-overlapping byte ranges into the searched text.
    pub matches: Vec<std::ops::Range<usize>>,
    /// The scan stopped at [`MAX_SEARCH_MATCHES`]; more matches exist.
    pub capped: bool,
    /// Index into `matches` of the selected match. `None` until the user
    /// lands on one, and again after they move elsewhere (page turn, click),
    /// so the next step starts from where they are rather than jumping back.
    pub current: Option<usize>,
    /// Set when revealing a match had to switch viewer pages: the fresh
    /// page editor learns its real viewport size only on its first layout,
    /// so the reveal is repeated then to land the match properly in view.
    pub reveal_pending: Option<std::time::Instant>,
}

impl BodySearch {
    /// Show the bar. Matches are only kept while it is open, so reopening
    /// with a remembered query rescans; re-opening an open bar (Ctrl+F again
    /// to re-focus the field) keeps the current match.
    pub fn open(&mut self, text: &str) {
        if !self.open {
            self.open = true;
            self.rescan(text);
        }
    }

    /// Hide the bar. The query is remembered for next time (as editors
    /// do); the matches are dropped so a closed bar costs nothing and never
    /// rescans on new responses.
    pub fn close(&mut self) {
        self.open = false;
        self.matches = Vec::new();
        self.capped = false;
        self.forget_position();
    }

    /// Replace the query (clamped to [`MAX_SEARCH_QUERY_BYTES`]) and rescan.
    pub fn set_query(&mut self, text: &str, query: &str) {
        self.query = head_capped(query, MAX_SEARCH_QUERY_BYTES).to_owned();
        self.rescan(text);
    }

    /// Flip case sensitivity and rescan.
    pub fn toggle_case(&mut self, text: &str) {
        self.case_sensitive = !self.case_sensitive;
        self.rescan(text);
    }

    /// Recompute matches for the current query. Drops the current match.
    pub fn rescan(&mut self, text: &str) {
        let (matches, capped) =
            find_matches(text, &self.query, self.case_sensitive, MAX_SEARCH_MATCHES);
        self.matches = matches;
        self.capped = capped;
        self.forget_position();
    }

    /// The searched text was replaced (new response): matches from the old
    /// text are meaningless. The bar, query and case mode carry over — like
    /// a browser's find bar across a reload — and an open bar's counter
    /// reflects the new body without moving the view.
    pub fn content_changed(&mut self, text: &str) {
        if self.open {
            self.rescan(text);
        } else {
            self.close();
        }
    }

    /// Forget the current match (the user moved away from it).
    pub fn forget_position(&mut self) {
        self.current = None;
        self.reveal_pending = None;
    }

    /// Byte range of the current match.
    pub fn current_range(&self) -> Option<std::ops::Range<usize>> {
        self.current.and_then(|i| self.matches.get(i).cloned())
    }

    /// Make the first match starting at or after `anchor` current, wrapping
    /// to the first match. Used when the query changes, so typing refines
    /// the search in place instead of jumping back to the top.
    pub fn select_from(&mut self, anchor: usize) -> Option<std::ops::Range<usize>> {
        let i = self.matches.partition_point(|m| m.start < anchor);
        self.current = match self.matches.len() {
            0 => None,
            len => Some(if i == len { 0 } else { i }),
        };
        self.current_range()
    }

    /// Step to the next (or previous) match, wrapping around the ends. With
    /// no current match, starts from `anchor` (the viewer's cursor): the
    /// first match at/after it going forward, the last before it going back.
    pub fn step(&mut self, forward: bool, anchor: usize) -> Option<std::ops::Range<usize>> {
        let len = self.matches.len();
        if len == 0 {
            self.current = None;
            return None;
        }
        let next = match self.current {
            Some(i) if forward => (i + 1) % len,
            Some(i) => (i + len - 1) % len,
            None => {
                let i = self.matches.partition_point(|m| m.start < anchor);
                match (forward, i) {
                    (true, i) if i == len => 0,
                    (true, i) => i,
                    (false, 0) => len - 1,
                    (false, i) => i - 1,
                }
            }
        };
        self.current = Some(next);
        self.current_range()
    }

    /// Counter text, or `None` while the query is empty.
    pub fn counter(&self) -> Option<String> {
        (!self.query.is_empty())
            .then(|| match_counter(self.current, self.matches.len(), self.capped))
    }

    /// Whether a non-empty query found nothing at all.
    pub fn no_results(&self) -> bool {
        !self.query.is_empty() && self.matches.is_empty()
    }
}

#[cfg(test)]
mod body_search_tests {
    use super::*;
    use crate::consts::{VIEWER_PAGE_BYTES, VIEWER_PAGE_LINES};
    use crate::state::tabs::page_starts;

    fn found<'a>(text: &'a str, query: &str, case_sensitive: bool) -> Vec<&'a str> {
        let (ranges, capped) = find_matches(text, query, case_sensitive, usize::MAX);
        assert!(!capped);
        ranges.into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn case_insensitive_by_default_and_exact_on_request() {
        let text = "Foo foo FOO fOo";
        assert_eq!(found(text, "foo", false).len(), 4);
        assert_eq!(found(text, "FOO", false).len(), 4);
        assert_eq!(found(text, "foo", true), vec!["foo"]);
        assert_eq!(found(text, "nope", false), Vec::<&str>::new());
        assert!(find_matches(text, "", false, 10).0.is_empty());
    }

    #[test]
    fn matches_are_non_overlapping_left_to_right() {
        let (ranges, _) = find_matches("aaaaa", "aa", false, usize::MAX);
        assert_eq!(ranges, vec![0..2, 2..4]);
    }

    /// Byte ranges must land on char boundaries (slicing panics otherwise,
    /// fatal under `panic = "abort"`), for ASCII and non-ASCII queries alike.
    #[test]
    fn multi_byte_text_yields_char_boundary_ranges() {
        let text = "€uro héllo HÉLLO naïve NAÏVE €";
        assert_eq!(found(text, "héllo", false), vec!["héllo", "HÉLLO"]);
        assert_eq!(found(text, "HÉLLO", false), vec!["héllo", "HÉLLO"]);
        assert_eq!(found(text, "héllo", true), vec!["héllo"]);
        assert_eq!(found(text, "€", false), vec!["€", "€"]);
        assert_eq!(found(text, "LLO", false), vec!["llo", "LLO"]);
        assert_eq!(found(text, "naïve", false), vec!["naïve", "NAÏVE"]);
        // A byte-wise ASCII search must not report a hit inside a multi-byte
        // sequence: '€' is E2 82 AC, none of which is ASCII.
        assert!(found("€€€", "a", false).is_empty());
    }

    /// The fast path skims in 32-byte chunks; hits on either side of a chunk
    /// edge, and at the very end of the text, must all be found.
    #[test]
    fn hits_on_chunk_edges_and_text_end_are_found() {
        for pos in [0usize, 1, 30, 31, 32, 33, 63, 64, 65, 95, 96] {
            let mut text = "x".repeat(pos);
            text.push_str("NeEdLe");
            text.push_str(&"y".repeat(40));
            let (ranges, _) = find_matches(&text, "needle", false, usize::MAX);
            assert_eq!(ranges, vec![pos..pos + 6], "needle at {pos}");
        }
        let text = format!("{}tail", "z".repeat(100));
        assert_eq!(find_matches(&text, "TAIL", false, 9).0, vec![100..104]);
        assert!(find_matches("ab", "abc", false, 9).0.is_empty());
    }

    #[test]
    fn scan_stops_at_the_cap_and_says_so() {
        let text = "ab".repeat(10);
        let (ranges, capped) = find_matches(&text, "a", false, 3);
        assert_eq!(ranges.len(), 3);
        assert!(capped, "more matches remain");

        // Exactly `cap` matches is a complete result, not a capped one.
        let (ranges, capped) = find_matches("a a a", "a", false, 3);
        assert_eq!(ranges.len(), 3);
        assert!(!capped);

        for case_sensitive in [true, false] {
            let (_, capped) = find_matches("ééé", "é", case_sensitive, 2);
            assert!(capped);
        }
    }

    #[test]
    fn locate_maps_offsets_to_page_line_and_char_column() {
        // Line 1 is "ab€cd"; 'c' is char 3 but byte 5 of the line.
        let text = "é€\nab€cd\n";
        let c = text.find('c').unwrap();
        assert_eq!(
            locate_in_pages(text, &[0], c),
            PagePosition { page: 0, line: 1, col: 3 }
        );
        assert_eq!(
            locate_in_pages(text, &[0], 0),
            PagePosition { page: 0, line: 0, col: 0 }
        );
        // A page that starts mid-line: its line 0 begins at the page start.
        let text = "0123456789\nabc";
        let starts = [0, 4];
        assert_eq!(
            locate_in_pages(text, &starts, 3),
            PagePosition { page: 0, line: 0, col: 3 }
        );
        assert_eq!(
            locate_in_pages(text, &starts, 4),
            PagePosition { page: 1, line: 0, col: 0 }
        );
        assert_eq!(
            locate_in_pages(text, &starts, text.find('b').unwrap()),
            PagePosition { page: 1, line: 1, col: 1 }
        );
        // Out-of-range and mid-character offsets clamp instead of panicking.
        let _ = locate_in_pages("€", &[0], 1);
        let _ = locate_in_pages("abc", &[0], 99);
    }

    /// `offset_in_page` is the inverse of `locate_in_pages` on every char
    /// boundary, and clamps positions past the end like the editor does.
    #[test]
    fn editor_positions_round_trip_to_offsets() {
        let page = "{\n  \"név\": \"€uro\",\r\n  \"x\": 1\n}";
        for (offset, _) in page.char_indices() {
            let pos = locate_in_pages(page, &[0], offset);
            assert_eq!(offset_in_page(page, pos.line, pos.col), offset, "at {offset}");
        }
        assert_eq!(offset_in_page(page, 0, 99), 1, "col clamps to line end");
        assert_eq!(offset_in_page(page, 99, 0), page.len(), "line clamps to end");
    }

    /// The bug in issue #71: a match on a later page must be found and
    /// mapped to that page, at the right line/column of its buffer.
    #[test]
    fn match_on_the_last_page_maps_into_that_page() {
        let line = "{\"id\": 1, \"name\": \"filler\"}\n";
        // ~3.5 pages of filler, so the needle sits well inside page 4.
        let mut text = line.repeat((VIEWER_PAGE_BYTES * 7 / 2) / line.len());
        text.push_str("  \"needle\": \"Zürich\"\n}");
        let starts = page_starts(&text);
        assert_eq!(starts.len(), 4, "body must span several pages");

        let (ranges, _) = find_matches(&text, "ZÜRICH", false, MAX_SEARCH_MATCHES);
        assert_eq!(ranges.len(), 1);
        let (pos, chars) = match_selection(&text, &starts, ranges[0].clone());
        assert_eq!(pos.page, starts.len() - 1, "on the last page");
        assert_eq!(chars, "Zürich".chars().count());

        // Cross-check against the page buffer the editor would hold.
        let page = &text[starts[pos.page]..];
        let editor_line = page.split('\n').nth(pos.line).unwrap();
        let selected: String = editor_line.chars().skip(pos.col).take(chars).collect();
        assert_eq!(selected, "Zürich");
    }

    /// Pages cut at a byte budget, so a match can straddle a page break. It
    /// is still found (search runs over the whole text), belongs to the page
    /// it starts on, and selects only what that page holds.
    #[test]
    fn match_spanning_a_page_break_is_found_and_clamped_to_its_page() {
        // One long line, so the byte budget (not the line cap) cuts the page.
        let mut text = "x".repeat(VIEWER_PAGE_BYTES - 3);
        text.push_str("needle");
        text.push_str(&"x".repeat(100));
        let starts = page_starts(&text);
        assert_eq!(starts, vec![0, VIEWER_PAGE_BYTES]);

        let (ranges, _) = find_matches(&text, "needle", false, MAX_SEARCH_MATCHES);
        assert_eq!(ranges, vec![VIEWER_PAGE_BYTES - 3..VIEWER_PAGE_BYTES + 3]);
        let (pos, chars) = match_selection(&text, &starts, ranges[0].clone());
        assert_eq!(pos, PagePosition { page: 0, line: 0, col: VIEWER_PAGE_BYTES - 3 });
        assert_eq!(chars, 3, "only the part on page 0 is selectable there");

        // Same with multi-byte chars around a mid-character byte budget.
        let mut text = "€".repeat(VIEWER_PAGE_BYTES / 3);
        text.push_str("ÄÖÜ");
        let starts = page_starts(&text);
        assert!(starts.len() > 1);
        let (ranges, _) = find_matches(&text, "€äö", false, MAX_SEARCH_MATCHES);
        assert_eq!(ranges.len(), 1);
        // The break falls between "Ä" and "Ö": page 0 holds "€Ä" of the match.
        assert_eq!(starts[1], VIEWER_PAGE_BYTES);
        let (pos, chars) = match_selection(&text, &starts, ranges[0].clone());
        assert_eq!(pos.page, 0);
        assert_eq!(chars, 2);
    }

    /// The line cap can cut pages too; positions stay page-relative.
    #[test]
    fn line_capped_pages_restart_line_numbers() {
        let mut text = "\n".repeat(VIEWER_PAGE_LINES + 5);
        text.push_str("hit");
        let starts = page_starts(&text);
        assert_eq!(starts.len(), 2);
        let (ranges, _) = find_matches(&text, "HIT", false, MAX_SEARCH_MATCHES);
        let (pos, chars) = match_selection(&text, &starts, ranges[0].clone());
        assert_eq!(pos.page, 1);
        assert_eq!(chars, 3);
        let page = &text[starts[1]..];
        assert_eq!(page.split('\n').nth(pos.line), Some("hit"));
        assert_eq!(pos.col, 0);
    }

    #[test]
    fn stepping_wraps_around_both_ends() {
        let text = "a.a.a";
        let mut s = BodySearch::default();
        s.set_query(text, "A");
        assert_eq!(s.matches.len(), 3);
        assert_eq!(s.counter().as_deref(), Some("3 results"));

        assert_eq!(s.step(true, 0), Some(0..1));
        assert_eq!(s.counter().as_deref(), Some("1 / 3"));
        assert_eq!(s.step(true, 0), Some(2..3));
        assert_eq!(s.step(true, 0), Some(4..5));
        assert_eq!(s.step(true, 0), Some(0..1), "next wraps to the first");
        assert_eq!(s.step(false, 0), Some(4..5), "previous wraps to the last");
        assert_eq!(s.counter().as_deref(), Some("3 / 3"));
    }

    #[test]
    fn stepping_without_a_current_match_starts_from_the_anchor() {
        let text = "a.a.a";
        let mut s = BodySearch::default();
        s.set_query(text, "a");
        assert_eq!(s.step(true, 1), Some(2..3), "first match at/after the cursor");
        s.forget_position();
        assert_eq!(s.step(false, 2), Some(0..1), "last match before the cursor");
        s.forget_position();
        assert_eq!(s.step(true, 5), Some(0..1), "past the last wraps forward");
        s.forget_position();
        assert_eq!(s.step(false, 0), Some(4..5), "before the first wraps back");

        // Typing refines in place: anchored at the old match, not the top.
        assert_eq!(s.select_from(2), Some(2..3));
        assert_eq!(s.select_from(99), Some(0..1));
    }

    #[test]
    fn no_results_and_empty_query_are_distinct() {
        let mut s = BodySearch::default();
        assert_eq!(s.counter(), None);
        assert!(!s.no_results());
        s.set_query("abc", "zzz");
        assert_eq!(s.counter().as_deref(), Some("No results"));
        assert!(s.no_results());
        assert_eq!(s.step(true, 0), None);
        assert_eq!(s.current, None);
    }

    #[test]
    fn rescans_drop_the_current_match() {
        let mut s = BodySearch::default();
        s.open("one two one");
        s.set_query("one two one", "one");
        s.step(true, 0);
        s.content_changed("one");
        assert_eq!(s.matches, vec![0..3]);
        assert_eq!(s.current, None, "old index is meaningless for new text");
        assert_eq!(s.query, "one", "query carries over to the new body");

        s.toggle_case("One one");
        assert!(s.case_sensitive);
        assert_eq!(s.matches, vec![4..7]);
    }

    /// A closed bar holds no matches and does no work on new responses, but
    /// remembers its query and rescans when reopened.
    #[test]
    fn closed_bar_keeps_query_but_not_matches() {
        let mut s = BodySearch::default();
        s.open("abc abc");
        s.set_query("abc abc", "abc");
        s.step(true, 0);
        s.close();
        assert!(s.matches.is_empty());
        assert_eq!(s.current, None);
        s.content_changed("abc abc abc");
        assert!(s.matches.is_empty(), "no scan while closed");

        s.open("abc abc abc");
        assert_eq!(s.query, "abc");
        assert_eq!(s.matches.len(), 3);
        s.step(true, 0);
        s.open("abc abc abc");
        assert_eq!(s.current, Some(0), "re-focusing an open bar keeps position");
    }

    #[test]
    fn counter_labels() {
        assert_eq!(match_counter(None, 0, false), "No results");
        assert_eq!(match_counter(Some(2), 41, false), "3 / 41");
        assert_eq!(match_counter(None, 1, false), "1 result");
        assert_eq!(match_counter(None, 41, false), "41 results");
        assert_eq!(match_counter(Some(1233), 50_000, true), "1,234 / 50,000+");
        assert_eq!(match_counter(None, 50_000, true), "50,000+ results");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1_000_000), "1,000,000");
    }

    #[test]
    fn oversized_queries_are_clamped_on_a_char_boundary() {
        let mut s = BodySearch::default();
        let huge = "é".repeat(MAX_SEARCH_QUERY_BYTES);
        s.set_query("é", &huge);
        assert!(s.query.len() <= MAX_SEARCH_QUERY_BYTES);
        assert!(s.query.chars().all(|c| c == 'é'));
    }
}
