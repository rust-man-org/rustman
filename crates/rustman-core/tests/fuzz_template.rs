//! Fuzz and property tests for templating and the text and URL helpers
//! every crate leans on: `{{variable}}` substitution, URL resolution,
//! byte-budget cuts, percent-encoding and the params/query sync.
//!
//! Runs are seeded, so failures reproduce. `RUSTMAN_FUZZ_CASES` scales the
//! case counts up for long runs.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use rustman_core::domain::environment::AppEnvironment;
use rustman_core::domain::headers::Headers;
use rustman_core::domain::model::Auth;
use rustman_core::domain::request::{
    HttpMethod, KeyValue, reconcile_url_params, sync_params_from_url, sync_url_from_params,
};
use rustman_core::domain::resolved::{ResolvedBody, ResolvedRequest};
use rustman_core::template::{resolve_url, substitute};
use rustman_core::text::{head_capped, human_size, is_json_document, pretty_json};
use rustman_core::url::percent_encode;

// ── Support ──────────────────────────────────────────────────────────────────

/// xorshift64*: small, fast and good enough to drive input generators.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self((seed ^ 0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next_u64() % n as u64).expect("below n")
    }

    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    fn char(&mut self) -> char {
        const TRICKY: &[char] = &[
            '{',
            '}',
            ' ',
            '\t',
            '\n',
            '%',
            '?',
            '#',
            '&',
            '=',
            '+',
            '/',
            ':',
            '\0',
            'é',
            '\u{301}',
            '\u{202e}',
            '€',
            '中',
            '😀',
            '\u{10ffff}',
        ];
        match self.below(3) {
            0 => char::from(u8::try_from(self.range(0x20, 0x7e)).expect("ascii")),
            1 => *self.pick(TRICKY),
            _ => char::from_u32(u32::try_from(self.range(0xa0, 0xd7ff)).expect("bmp"))
                .expect("below the surrogates"),
        }
    }

    fn text(&mut self, max_len: usize) -> String {
        let len = self.below(max_len + 1);
        (0..len).map(|_| self.char()).collect()
    }

    fn text_from(&mut self, alphabet: &[char], max_len: usize) -> String {
        let len = self.below(max_len + 1);
        (0..len).map(|_| *self.pick(alphabet)).collect()
    }
}

fn cases(default: usize) -> usize {
    std::env::var("RUSTMAN_FUZZ_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn long_run() -> bool {
    std::env::var("RUSTMAN_FUZZ_CASES").is_ok_and(|v| v.parse::<usize>().is_ok())
}

fn env(vars: &[(&str, &str)]) -> AppEnvironment {
    AppEnvironment {
        id: "e".into(),
        name: "e".into(),
        variables: vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<HashMap<_, _>>(),
        is_active: true,
    }
}

/// The fastest of five runs each of `small` and `large`, taken in turn so a
/// burst of load lands on both sizes instead of inflating one of them.
fn best_of_5_alternating(mut small: impl FnMut(), mut large: impl FnMut()) -> (Duration, Duration) {
    fn time(f: &mut impl FnMut()) -> Duration {
        let start = Instant::now();
        f();
        start.elapsed()
    }
    let (mut t_small, mut t_large) = (Duration::MAX, Duration::MAX);
    for _ in 0..5 {
        t_small = t_small.min(time(&mut small));
        t_large = t_large.min(time(&mut large));
    }
    (t_small, t_large)
}

const BRACES: &[char] = &['{', '{', '}', '}', ' ', 'a', 'b', 'é', '😀', '\n', '$', '%'];

// ── substitute ───────────────────────────────────────────────────────────────

#[test]
fn substitution_with_no_variables_is_the_identity() {
    let mut rng = Rng::new(0x7_0001);
    let empty = env(&[]);
    for _ in 0..cases(20_000) {
        let text = if rng.chance(50) {
            rng.text_from(BRACES, 40)
        } else {
            rng.text(40)
        };
        assert_eq!(substitute(&text, None), text);
        assert_eq!(substitute(&text, Some(&empty)), text, "{text:?}");
    }
}

/// Text built from literal chunks without braces and `{{ name }}`
/// placeholders, and what it expands to.
fn template(rng: &mut Rng, vars: &AppEnvironment) -> (String, String) {
    let names: Vec<&String> = vars.variables.keys().collect();
    let (mut text, mut expanded) = (String::new(), String::new());
    for _ in 0..rng.range(1, 8) {
        if rng.chance(50) {
            let chunk = rng.text(10).replace(['{', '}'], "");
            text.push_str(&chunk);
            expanded.push_str(&chunk);
        } else {
            let name = *rng.pick(&names);
            let pad = |rng: &mut Rng| " ".repeat(rng.below(3));
            text.push_str(&format!("{{{{{}{name}{}}}}}", pad(rng), pad(rng)));
            expanded.push_str(&vars.variables[name]);
        }
    }
    (text, expanded)
}

#[test]
fn defined_variables_expand_once() {
    let mut rng = Rng::new(0x7_0002);
    // A value that itself looks like a variable is not expanded again.
    let vars = env(&[
        ("a", "1"),
        ("base_url", "https://api.example.com"),
        ("ü", "é😀"),
        ("self", "{{self}}"),
        ("other", "{{a}}"),
        ("empty", ""),
        ("braces", "}}{{"),
    ]);
    for _ in 0..cases(20_000) {
        let (text, expanded) = template(&mut rng, &vars);
        assert_eq!(substitute(&text, Some(&vars)), expanded, "{text:?}");
    }
}

#[test]
fn unknown_and_unclosed_placeholders_stay_literal() {
    let vars = env(&[("a", "1")]);
    for (text, want) in [
        ("{{missing}}", "{{missing}}"),
        ("{{", "{{"),
        ("}}", "}}"),
        ("{{a", "{{a"),
        ("{{a}", "{{a}"),
        ("{{}}", "{{}}"),
        ("{{ }}", "{{ }}"),
        ("{{a}}}", "1}"),
        ("é{{a}}😀", "é1😀"),
        ("{{a}}{{a}}", "11"),
    ] {
        assert_eq!(substitute(text, Some(&vars)), want, "{text:?}");
    }
}

#[test]
fn a_stray_open_brace_does_not_hide_the_next_variable() {
    let vars = env(&[("token", "t0k"), ("a", "1")]);
    assert_eq!(
        substitute(r#"{"open": "{{", "auth": "{{token}}"}"#, Some(&vars)),
        r#"{"open": "{{", "auth": "t0k"}"#
    );
    assert_eq!(substitute("{{{a}}}", Some(&vars)), "{1}");
}

/// Builds an input of about `n` bytes.
type Shape = fn(usize) -> String;

#[test]
fn substitution_is_linear_in_the_input() {
    let vars = env(&[("a", "value")]);
    let n = if long_run() { 1_000_000 } else { 200_000 };
    let shapes: Vec<(&str, Shape)> = vec![
        ("open braces only", |n| "{{".repeat(n / 2)),
        ("close braces only", |n| "}}".repeat(n / 2)),
        ("single braces", |n| "{}".repeat(n / 2)),
        ("defined placeholders", |n| "{{a}}".repeat(n / 5)),
        ("unknown placeholders", |n| "{{b}}".repeat(n / 5)),
        ("one huge name", |n| format!("{{{{{}}}}}", " ".repeat(n))),
        ("opens then a close", |n| {
            format!("{}}}}}", "{{ ".repeat(n / 3))
        }),
    ];
    for (label, make) in shapes {
        let (small, large) = (make(n / 4), make(n));
        let (t_small, t_large) = best_of_5_alternating(
            || drop(substitute(&small, Some(&vars))),
            || drop(substitute(&large, Some(&vars))),
        );
        let ratio = t_large.as_secs_f64() / t_small.as_secs_f64().max(1e-6);
        eprintln!(
            "{label}: {} B in {t_small:?}, {} B in {t_large:?}",
            small.len(),
            large.len()
        );
        assert!(
            t_small < Duration::from_millis(2) || ratio < 10.0,
            "{label}: 4x the input took {ratio:.1}x as long"
        );
    }
}

// ── resolve_url ──────────────────────────────────────────────────────────────

const URL_CHARS: &[char] = &[
    'a', 'x', '.', '/', ':', '1', '0', '7', '2', '9', '[', ']', '{', '}', ' ', 'é', '?', '#', '@',
    '%',
];

#[test]
fn resolved_urls_have_one_scheme_and_resolve_to_themselves() {
    let mut rng = Rng::new(0x7_0003);
    let vars = env(&[
        ("base", "https://api.example.com"),
        ("host", "localhost:8080"),
        ("blank", "  "),
    ]);
    for _ in 0..cases(20_000) {
        let mut raw = rng.text_from(URL_CHARS, 24);
        if rng.chance(30) {
            raw = format!(
                "{{{{{}}}}}{raw}",
                rng.pick(&["base", "host", "blank", "nope"])
            );
        }
        if rng.chance(20) {
            raw = format!(
                "{}{raw}",
                rng.pick(&["http://", "HTTPS://", "ws://", "ftp://", " "])
            );
        }
        let url = resolve_url(&raw, Some(&vars));
        assert!(
            url.is_empty()
                || ["http://", "https://", "ws://", "wss://"]
                    .iter()
                    .any(|s| url.starts_with(s)),
            "{raw:?} -> {url:?}"
        );
        assert_eq!(resolve_url(&url, None), url, "not idempotent for {raw:?}");
    }
}

#[test]
fn local_hosts_are_told_from_public_ones() {
    assert_eq!(resolve_url("172.217.1.1/x", None), "https://172.217.1.1/x");
    assert_eq!(
        resolve_url("10.example.com/x", None),
        "https://10.example.com/x"
    );
    assert_eq!(resolve_url("[::1]:8080/x", None), "http://[::1]:8080/x");
    assert_eq!(resolve_url("172.16.0.1/x", None), "http://172.16.0.1/x");
}

// ── head_capped ──────────────────────────────────────────────────────────────

#[test]
fn head_capped_cuts_at_the_last_char_boundary_for_every_budget() {
    let mut rng = Rng::new(0x7_0004);
    for _ in 0..cases(2_000) {
        let text = rng.text(40);
        for max in 0..=text.len() + 2 {
            let head = head_capped(&text, max);
            assert!(text.starts_with(head));
            assert!(head.len() <= max);
            // The longest such prefix: the next character would not fit.
            if let Some(next) = text[head.len()..].chars().next() {
                assert!(head.len() + next.len_utf8() > max, "{text:?} at {max}");
            }
        }
    }
}

// ── Percent-encoding and the params table ────────────────────────────────────

fn percent_decode(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).expect("ascii hex");
            out.push(u8::from_str_radix(hex, 16).expect("hex"));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

#[test]
fn percent_encoding_is_unreserved_ascii_and_decodes_back() {
    let mut rng = Rng::new(0x7_0005);
    for _ in 0..cases(20_000) {
        let text = rng.text(30);
        let encoded = percent_encode(&text);
        assert!(
            encoded
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~%".contains(&b)),
            "{encoded}"
        );
        assert_eq!(percent_decode(&encoded), text.as_bytes());
    }
}

fn row(key: String, value: String, enabled: bool) -> KeyValue {
    KeyValue {
        id: "id".into(),
        key,
        value,
        enabled,
    }
}

const QUERY_CHARS: &[char] = &[
    'a', 'B', '1', ' ', '&', '=', '#', '%', '+', '?', '/', 'é', '😀', ';', ',', '[', ']',
];

#[test]
fn params_survive_a_trip_through_the_url() {
    let mut rng = Rng::new(0x7_0006);
    for _ in 0..cases(10_000) {
        let params: Vec<KeyValue> = (0..rng.range(1, 5))
            .map(|_| {
                let mut key = rng.text_from(QUERY_CHARS, 8);
                if key.is_empty() {
                    key.push('k');
                }
                row(key, rng.text_from(QUERY_CHARS, 8), true)
            })
            .collect();
        let url = sync_url_from_params("https://x.io/a", &params);
        let back = sync_params_from_url(&url, &[]);
        let pairs = |rows: &[KeyValue]| -> Vec<(String, String)> {
            rows.iter()
                .map(|r| (r.key.clone(), r.value.clone()))
                .collect()
        };
        assert_eq!(pairs(&back), pairs(&params), "{url}");
    }
}

#[test]
fn any_query_string_decodes_without_panicking() {
    let mut rng = Rng::new(0x7_0007);
    let alphabet = [
        '%', '2', 'f', 'F', 'z', '0', '+', '&', '=', '?', '#', 'é', 'a',
    ];
    for _ in 0..cases(20_000) {
        let url = format!("https://x.io/?{}", rng.text_from(&alphabet, 20));
        let params = sync_params_from_url(&url, &[]);
        let (rebuilt, merged) = reconcile_url_params(&url, &params);
        let (again, merged_again) = reconcile_url_params(&rebuilt, &merged);
        assert_eq!(again, rebuilt, "reconcile is not idempotent for {url}");
        let named = |rows: &[KeyValue]| rows.iter().filter(|r| !r.key.is_empty()).count();
        assert_eq!(named(&merged_again), named(&merged), "{url}");
    }
}

#[test]
fn params_go_before_the_fragment() {
    let params = [row("q".into(), "1".into(), true)];
    assert_eq!(
        sync_url_from_params("https://x.io/a#top", &params),
        "https://x.io/a?q=1#top"
    );
    let sent = ResolvedRequest {
        method: HttpMethod::Get,
        url: "https://x.io/a#top".into(),
        query: vec![("q".into(), "1".into())],
        headers: Headers::default(),
        auth: Auth::None,
        body: ResolvedBody::None,
    };
    assert_eq!(sent.full_url(), "https://x.io/a?q=1#top");
}

// ── JSON helpers ─────────────────────────────────────────────────────────────

#[test]
fn json_helpers_accept_anything() {
    let mut rng = Rng::new(0x7_0008);
    let alphabet = [
        '{', '}', '[', ']', '"', ':', ',', '1', 'e', '-', '.', 'n', 'u', 'l', ' ', '\\',
    ];
    for _ in 0..cases(20_000) {
        let text = rng.text_from(&alphabet, 30);
        let is_document = is_json_document(&text);
        if let Some(pretty) = pretty_json(&text, rng.chance(50)) {
            let a: serde_json::Value = serde_json::from_str(&text).expect("parsed once");
            let b: serde_json::Value = serde_json::from_str(&pretty).expect("pretty is JSON");
            assert_eq!(a, b);
        } else {
            assert!(!is_document, "{text:?}");
        }
    }
    // Past serde_json's recursion limit: `is_json_document` skips values
    // iteratively and may accept them, `pretty_json` refuses. Neither may
    // overflow the stack.
    for deep in [
        "[".repeat(100_000),
        format!("{}{}", "[".repeat(100_000), "]".repeat(100_000)),
        format!("{}1{}", "{\"a\":".repeat(100_000), "}".repeat(100_000)),
    ] {
        let _ = is_json_document(&deep);
        assert_eq!(pretty_json(&deep, false), None);
    }
    for bytes in [0, 1023, 1024, 1_048_575, 1_048_576, usize::MAX] {
        assert!(!human_size(bytes).is_empty());
    }
}
