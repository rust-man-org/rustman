//! Fuzz and property tests for the cURL and HTTPie command importers.
//!
//! Release builds abort on panic, so the first property is that no paste,
//! however mangled, panics. The second is speed: a megabyte paste is parsed
//! in linear time. The third is that `export` and `parse` agree: a request
//! exported as a curl command imports back as the same request.

mod fuzz_support;

use std::time::Duration;

use base64::Engine as _;
use fuzz_support::{Rng, assert_linear, cases, fuzz, size};
use rustman_core::domain::headers::Headers;
use rustman_core::domain::model::Auth;
use rustman_core::domain::request::{ApiKeyLocation, BodyType, HttpMethod};
use rustman_core::domain::resolved::{FieldValue, ResolvedBody, ResolvedField, ResolvedRequest};
use rustman_core::text::is_json_document;
use rustman_core::url::percent_encode;
use rustman_formats::curl;
use rustman_formats::import::{classify_pasted_command, httpie};

// ── Generators ───────────────────────────────────────────────────────────────

/// Characters with a meaning to one of the three shell dialects.
const SHELL_SPECIALS: &[char] = &[
    ' ', '\t', '\n', '\r', '\'', '"', '$', '\\', '^', '`', '|', ';', '&', '<', '>', '#', '-', '=',
    '@', ':', '{', '}', '%', '+', '!', '*', '?', '(', ')', 'a', 'x', '0', '7', 'é', '😀',
    '\u{301}', '\u{202e}',
];

const PROGRAMS: &[&str] = &[
    "curl",
    "curl.exe",
    "/usr/bin/curl",
    "CURL",
    r"C:\bin\curl.exe",
    "$ curl",
    "% curl",
    "echo '{}' | curl",
    "cd x && curl",
    "# fetch\ncurl",
    "```\ncurl",
    "curling",
];

/// Short options: ones that take a value, flags, and ignored letters.
const SHORT: &[char] = &[
    'A', 'b', 'c', 'C', 'd', 'D', 'e', 'E', 'F', 'H', 'K', 'm', 'o', 'P', 'Q', 'r', 't', 'T', 'u',
    'U', 'w', 'x', 'X', 'y', 'Y', 'z', 'G', 'I', 'k', ':', 's', 'S', 'L', 'v', 'f', 'i', '#', '0',
    '-', 'é', '😀',
];

const LONG_FLAGS: &[&str] = &[
    "compressed",
    "get",
    "head",
    "insecure",
    "next",
    "digest",
    "ntlm",
    "ntlm-wb",
    "negotiate",
    "anyauth",
    "location",
    "silent",
    "some-future-option",
    "",
    "=",
    "é",
];

const LONG_VALUES: &[&str] = &[
    "request",
    "url",
    "url-query",
    "header",
    "user-agent",
    "referer",
    "cookie",
    "user",
    "oauth2-bearer",
    "data",
    "data-ascii",
    "data-binary",
    "data-raw",
    "data-urlencode",
    "json",
    "form",
    "form-string",
    "upload-file",
    "proxy",
    "preproxy",
    "socks5",
    "cert",
    "key",
    "aws-sigv4",
    "config",
    "unix-socket",
    "abstract-unix-socket",
    "output",
    "max-time",
    "write-out",
];

const VALUES: &[&str] = &[
    "https://x.io/a?b=1&c=2#frag",
    "{{base}}/users/{{id}}",
    "localhost:8080",
    "x.io",
    "http://ü.example/😀?q=é",
    "",
    "-",
    "@-",
    "@file.json",
    "Accept: */*",
    "X-Empty;",
    "Host:",
    ":",
    ";",
    "@headers.txt",
    "Cookie: a=1; b=2; Path=/; HttpOnly",
    "a:b:c",
    "a=1",
    "@",
    "=",
    "=x",
    "name@file",
    "a@b=c",
    "+raw=%20",
    "k=v=w",
    "{\"a\":1}",
    "[1,2",
    "f=@\"/tmp/a b.png\";type=image/png",
    "f=@",
    "f=<file",
    "f=\"unterminated",
    "f=@\"x\\",
    "a=b;type=",
    ";;;=",
    "a=b;filename=x;headers=y;encoder=z;type=t",
    "a=1; b=2",
    "jar.txt",
    "=;=;",
    "user:pass",
    "user",
    "POST",
    "patch",
    "https://ref.io;auto",
    ";auto",
    "%{http_code}",
];

fn word(rng: &mut Rng) -> String {
    match rng.below(4) {
        0 => rng.text(12),
        1 => rng.text_from(SHELL_SPECIALS, 12),
        _ => (*rng.pick(VALUES)).to_owned(),
    }
}

/// `word` quoted the way one of the dialects might, sometimes badly.
fn quoted(rng: &mut Rng, word: &str) -> String {
    match rng.below(9) {
        0 => word.to_owned(),
        1 => format!("'{}'", word.replace('\'', r"'\''")),
        2 => {
            let escaped: String = word
                .chars()
                .flat_map(|c| match c {
                    '"' | '\\' | '$' | '`' => vec!['\\', c],
                    c => vec![c],
                })
                .collect();
            format!("\"{escaped}\"")
        }
        3 => format!("$'{}{}'", word.replace('\'', r"\'"), ansi_c_escapes(rng)),
        4 => {
            let escaped: String = word
                .chars()
                .flat_map(|c| match c {
                    '"' => vec!['^', '\\', '^', '"'],
                    '&' | '|' | '<' | '>' | '^' | '%' | '{' | '}' => vec!['^', c],
                    c => vec![c],
                })
                .collect();
            format!("^\"{escaped}^\"")
        }
        5 => format!("\"{}\"", word.replace('"', "`\"")),
        6 => {
            // Unterminated or mismatched quoting.
            let open = *rng.pick(&["'", "\"", "$'", "^\"", "$\""]);
            format!("{open}{word}")
        }
        7 => format!("{word}\\"),
        _ => format!("'{word}'\"{word}\"{word}"),
    }
}

/// ANSI-C escapes, valid and not: bad code points, overlong octal, a
/// trailing backslash, `\c` before a multi-byte character.
fn ansi_c_escapes(rng: &mut Rng) -> String {
    const ESCAPES: &[&str] = &[
        r"\n",
        r"\t",
        r"\x41",
        r"\x",
        r"\xZ",
        r"\u00e9",
        r"\uD800",
        r"\uDFFF",
        r"\u",
        r"\U0010FFFF",
        r"\U00110000",
        r"\UFFFFFFFF",
        r"\U",
        r"\777",
        r"\0",
        r"\303\251",
        r"\303",
        r"\cA",
        r"\c😀",
        r"\c",
        r"\e",
        r"\q",
        r"\'",
        r#"\""#,
        r"\?",
        r"\\",
        r"\é",
    ];
    (0..rng.below(4)).map(|_| *rng.pick(ESCAPES)).collect()
}

fn separator(rng: &mut Rng) -> &'static str {
    rng.pick::<&str>(&[
        " ",
        " ",
        " ",
        "  ",
        "\t",
        " \\\n  ",
        " \\\r\n  ",
        " \\ ",
        "\n",
        "\n  ",
        "\r\n",
        " ^\n  ",
        " ^\r\n  ",
        " `\n  ",
        " | ",
        "; ",
        " && ",
        " > out.json ",
        " # comment\n",
        "&",
        "|",
        "<",
        "\n```\n",
    ])
}

fn option(rng: &mut Rng) -> String {
    match rng.below(7) {
        0 => {
            let letters: String = (0..rng.range(1, 4)).map(|_| *rng.pick(SHORT)).collect();
            let attached = if rng.chance(40) {
                word(rng)
            } else {
                String::new()
            };
            quoted(rng, &format!("-{letters}{attached}"))
        }
        1 => format!("--{}", rng.pick(LONG_FLAGS)),
        2 => {
            let name = rng.pick(LONG_VALUES);
            let value = word(rng);
            let value = quoted(rng, &value);
            if rng.chance(30) {
                format!("--{name}={value}")
            } else {
                format!("--{name} {value}")
            }
        }
        3 => {
            let value = word(rng);
            quoted(rng, &value)
        }
        4 => {
            let value = word(rng);
            format!("-{} {}", rng.pick(SHORT), quoted(rng, &value))
        }
        5 => rng.text_from(SHELL_SPECIALS, 6),
        _ => rng.text(6),
    }
}

/// A plausible-to-mangled curl command.
fn command(rng: &mut Rng) -> String {
    let mut s = (*rng.pick(PROGRAMS)).to_owned();
    for _ in 0..rng.range(0, 14) {
        s.push_str(separator(rng));
        s.push_str(&option(rng));
    }
    if rng.chance(15) {
        s.push_str(rng.pick::<&str>(&[
            "```", " \\", " ^", " `", "'", "\"", "$'", "\\", "^", "`", "$",
        ]));
    }
    s
}

/// Real pastes from browsers, docs and shells, as seeds for mutation.
const REAL_COMMANDS: &[&str] = &[
    r#"curl 'https://api.example.com/v1/graphql' \
  -H 'accept: */*' \
  -H 'authorization: Bearer eyJ.abc.def' \
  -H 'content-type: application/json' \
  -b 'sid=abc+1/2==; theme=dark' \
  --data-raw $'{"q":"hi!","x":"caf\u00e9 \303\251"}' \
  --compressed"#,
    "curl ^\"https://x.io/a?b=1^&c=2^\" ^\r\n  -H ^\"accept: */*^\" ^\r\n  --data-raw ^\"^{^\\^\"a^\\^\":^\\^\"x^\\^\\y^\\^\"^}^\"",
    "curl.exe -X POST `\n  -H \"Content-Type: application/json\" `\n  -d '{\\\"a\\\":1}' `\n  C:\\tmp\\x",
    "$ curl -sSLo out.txt -u admin:pw -G https://x.io/s?a=1#top -d q=rust --data-urlencode 'name=a b&c' | jq .",
    r#"curl https://x.io -F 'file=@"/tmp/my file.png";type=image/png' -F 'doc=@/tmp/a.pdf;filename=x.pdf' -F 'note=a;b' --form-string 'raw=@x' -T up.bin --next -X DELETE https://x.io/b"#,
    "curl -k --digest -u a:b -x http://proxy:8080 -A 'Agent/1' -e 'https://ref.io;auto' --url-query 'q=a b' --url-query '+raw=%20' --json '{\"a\":' --json '1}' https://x.io",
    "http --auth-type=bearer -a tok POST :8080/api a=1 b:='{\"c\":[1,2]}' q==x X-H:v f@./file.txt",
    "xh -f PUT https://x.io/a name=é 'Cookie:a=1'",
];

// ── No panics ────────────────────────────────────────────────────────────────

/// Everything a paste goes through when it is dropped on the app.
fn import_everything(input: &str) {
    let parsed = curl::parse(input);
    assert!(parsed.method.is_some(), "parse always picks a method");
    let _ = classify_pasted_command(input);
    let _ = httpie::parse(input);
}

#[test]
fn random_bytes_never_panic() {
    fuzz(
        0xC0_01,
        cases(4000),
        |rng| {
            let len = rng.below(200);
            let text = String::from_utf8_lossy(&rng.bytes(len)).into_owned();
            if rng.chance(50) {
                format!("curl {text}")
            } else {
                text
            }
        },
        import_everything,
    );
}

#[test]
fn shell_metacharacter_soup_never_panics() {
    fuzz(
        0xC0_02,
        cases(6000),
        |rng| {
            let soup = rng.text_from(SHELL_SPECIALS, 120);
            match rng.below(3) {
                0 => format!("curl {soup}"),
                1 => format!("curl ^\"x^\" {soup}"),
                _ => format!("curl.exe `\n {soup}"),
            }
        },
        import_everything,
    );
}

#[test]
fn grammar_generated_commands_never_panic() {
    fuzz(0xC0_03, cases(6000), command, import_everything);
}

#[test]
fn mutated_real_commands_never_panic() {
    fuzz(
        0xC0_04,
        cases(6000),
        |rng| {
            let base = *rng.pick(REAL_COMMANDS);
            rng.mutate(base)
        },
        import_everything,
    );
}

#[test]
fn every_prefix_of_a_real_command_parses() {
    for command in REAL_COMMANDS {
        for (i, _) in command.char_indices() {
            import_everything(&command[..i]);
        }
    }
}

// ── Linear time ──────────────────────────────────────────────────────────────

/// What a megabyte paste may cost in a debug build. Release is ~10x faster.
const MEGABYTE_BUDGET: Duration = Duration::from_millis(1500);

fn repeat_n(piece: &str, total: usize) -> String {
    piece.repeat((total / piece.len()).max(1))
}

fn parse_only(input: &str) {
    let _ = curl::parse(input);
}

/// Pastes that are huge in one dimension each: a long word, many options,
/// deep quoting.
type Shape = (&'static str, fn(usize) -> String);

const LINEAR_SHAPES: &[Shape] = &[
    ("one huge word", |n| {
        format!("curl https://x.io/{}", "a".repeat(n))
    }),
    ("single-quoted body", |n| {
        format!("curl -d '{}' x.io", repeat_n("a b ", n))
    }),
    ("double-quoted escapes", |n| {
        format!("curl -d \"{}\" x.io", repeat_n("\\\"a\\\\ \\$", n))
    }),
    ("ANSI-C escapes", |n| {
        format!("curl -d $'{}' x.io", repeat_n(r"\x41\u00e9\303\251\n", n))
    }),
    ("cmd caret quoting", |n| {
        format!("curl ^\"https://x.io/{}^\"", repeat_n("a^&b^\\^\"", n))
    }),
    ("PowerShell backticks", |n| {
        format!("curl.exe `\n -d \"{}\" x.io", repeat_n("a``b`\"", n))
    }),
    ("many headers", |n| {
        format!("curl https://x.io{}", repeat_n(" -H 'X-Tag: value'", n))
    }),
    ("many data pieces", |n| {
        format!("curl https://x.io{}", repeat_n(" -d a=1", n))
    }),
    ("many continued lines", |n| {
        format!("curl https://x.io{}", repeat_n(" \\\n  -H 'a: b'", n))
    }),
    ("many lines starting with options", |n| {
        format!("curl https://x.io{}", repeat_n("\n  -H 'a: b'", n))
    }),
    ("many piped commands", |n| {
        format!("{}curl x.io", repeat_n("echo a | ", n))
    }),
    ("operators only", |n| "|;&<>".repeat(n / 5)),
    ("short flag bundle", |n| format!("curl -{}", "s".repeat(n))),
    ("bundle of value-less flags", |n| {
        format!("curl -{}", "G".repeat(n))
    }),
    ("form parameter tail", |n| {
        format!("curl x.io -F 'a=b{}'", repeat_n(";type=x", n))
    }),
    ("quoted form value", |n| {
        format!("curl x.io -F 'a=@\"{}\"'", repeat_n("\\\"x", n))
    }),
    ("many positionals", |n| format!("curl{}", repeat_n(" a", n))),
    ("backslash-space continuations", |n| {
        format!("curl{} x.io", repeat_n(" \\ ", n))
    }),
    ("comment lines", |n| {
        format!("{}curl x.io", repeat_n("# x\n", n))
    }),
    ("unterminated quote", |n| format!("curl '{}", "a".repeat(n))),
    ("caret soup", |n| format!("curl ^\"{}", "^".repeat(n))),
];

#[test]
fn megabyte_pastes_parse_in_linear_time() {
    // 160 KB by default; a long run uses the full megabyte.
    let n = size(40_000, 250_000);
    for (label, make) in LINEAR_SHAPES {
        assert_linear(label, n, make, Some(MEGABYTE_BUDGET), parse_only);
    }
}

fn distinct_proxies(n: usize) -> String {
    let mut s = String::from("curl x.io");
    for i in 0..n / 12 {
        s.push_str(&format!(" -x p{i:07}"));
    }
    s
}

fn distinct_cookies(n: usize) -> String {
    let mut s = String::from("curl x.io -b '");
    for i in 0..n / 12 {
        s.push_str(&format!("c{i:07}=1; "));
    }
    s.push('\'');
    s
}

fn url_queries(n: usize) -> String {
    format!("curl https://x.io{}", repeat_n(" --url-query q=v", n))
}

/// A long URL and many `--url-query` pieces: `append_query` rescans the
/// whole URL for `#` and `?` for each piece.
fn url_queries_on_a_long_url(n: usize) -> String {
    format!(
        "curl https://x.io/{}{}",
        "a".repeat(n / 2),
        repeat_n(" --url-query q=v", n / 2)
    )
}

// Notes are deduped, same-named cookies replaced and query pieces appended
// once per item, so none of them may cost O(n) per item.

#[test]
fn many_distinct_notes_parse_in_linear_time() {
    assert_linear(
        "distinct proxies",
        size(40_000, 250_000),
        distinct_proxies,
        Some(MEGABYTE_BUDGET),
        parse_only,
    );
}

#[test]
fn many_cookies_parse_in_linear_time() {
    // Every distinct cookie is kept once, in command order.
    let cookies = curl::parse(&distinct_cookies(12_000)).cookies;
    assert_eq!(cookies.len(), 1_000);
    assert_eq!(cookies[999], ("c0000999".to_owned(), "1".to_owned()));

    assert_linear(
        "distinct cookies",
        size(40_000, 250_000),
        distinct_cookies,
        Some(MEGABYTE_BUDGET),
        parse_only,
    );
}

#[test]
fn many_url_query_pieces_parse_in_linear_time() {
    // Each piece is appended once, in command order.
    let url = curl::parse(&url_queries(1_600)).url.expect("a URL");
    let query = url.split_once('?').map(|(_, query)| query);
    assert_eq!(query, Some(vec!["q=v"; 100].join("&").as_str()), "{url}");

    assert_linear(
        "--url-query pieces",
        size(40_000, 250_000),
        url_queries,
        Some(MEGABYTE_BUDGET),
        parse_only,
    );
}

#[test]
fn url_query_pieces_on_a_long_url_parse_in_linear_time() {
    assert_linear(
        "--url-query pieces on a long URL",
        size(40_000, 250_000),
        url_queries_on_a_long_url,
        Some(MEGABYTE_BUDGET),
        parse_only,
    );
}

// ── export → parse round trip ────────────────────────────────────────────────

const HEADER_NAMES: &[&str] = &[
    "Accept",
    "X-Api-Key",
    "x-tag",
    "If-None-Match",
    "Content-Type",
    "X-Ünïcode",
    "Authorization",
];

/// Header and body text: quotes, shell metacharacters, unicode, no CR/LF
/// for header values.
fn field_text(rng: &mut Rng, allow_newlines: bool) -> String {
    let text = match rng.below(3) {
        0 => rng.text(24),
        1 => rng.text_from(SHELL_SPECIALS, 24),
        _ => (*rng.pick(VALUES)).to_owned(),
    };
    if allow_newlines {
        text
    } else {
        text.replace(['\r', '\n', '\u{85}', '\u{2028}'], "")
    }
}

fn token(rng: &mut Rng) -> String {
    const TOKEN: &[char] = &[
        'a', 'b', 'X', 'Z', '0', '9', '-', '_', '.', '!', '#', '$', '%', '&', '\'', '*', '+', '^',
        '`', '|', '~',
    ];
    let mut name = rng.text_from(TOKEN, 10);
    if name.is_empty() {
        name.push('h');
    }
    name
}

fn method(rng: &mut Rng) -> HttpMethod {
    rng.pick(HttpMethod::all()).clone()
}

fn round_trip_request(rng: &mut Rng) -> ResolvedRequest<'static> {
    let path: String = rng.text_from(SHELL_SPECIALS, 16).replace(['\r', '\n'], "");
    let url = format!("https://{}.example/{path}", rng.pick(&["api", "ü", "x-1"]));
    let query = (0..rng.below(3))
        .map(|_| (token(rng), field_text(rng, false)))
        .collect();
    let mut headers = Headers::default();
    for _ in 0..rng.below(4) {
        let name = if rng.chance(60) {
            (*rng.pick(HEADER_NAMES)).to_owned()
        } else {
            token(rng)
        };
        let value = field_text(rng, false).trim().to_owned();
        headers.push(name, if value.is_empty() { "v".into() } else { value });
    }
    let auth = match rng.below(5) {
        0 => Auth::Bearer { token: token(rng) },
        1 => Auth::Basic {
            user: token(rng),
            pass: field_text(rng, false),
        },
        2 => Auth::ApiKey {
            name: token(rng),
            value: token(rng),
            location: if rng.chance(50) {
                ApiKeyLocation::Header
            } else {
                ApiKeyLocation::Query
            },
        },
        _ => Auth::None,
    };
    let body = match rng.below(3) {
        0 => ResolvedBody::None,
        1 => ResolvedBody::Text(field_text(rng, true)),
        _ => ResolvedBody::Json(format!(
            "{{\"k\": {}}}",
            serde_json::to_string(&field_text(rng, true)).expect("a string serializes")
        )),
    };
    ResolvedRequest {
        method: method(rng),
        url,
        query,
        headers,
        auth,
        body,
    }
}

fn body_text<'a>(body: &'a ResolvedBody<'_>) -> Option<&'a str> {
    match body {
        ResolvedBody::Json(text) | ResolvedBody::Text(text) if !text.trim().is_empty() => {
            Some(text)
        }
        _ => None,
    }
}

/// The headers `parse` should recover: explicit ones in order, auth that
/// went out as an option, and the form Content-Type curl itself adds.
fn expected_headers(req: &ResolvedRequest<'_>) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut basic = None;
    match &req.auth {
        Auth::Bearer { token } if !token.is_empty() => {
            headers.push(("Authorization".into(), format!("Bearer {token}")));
        }
        Auth::ApiKey {
            name,
            value,
            location: ApiKeyLocation::Header,
        } if !name.is_empty() && !value.is_empty() => {
            headers.push((name.clone(), value.clone()));
        }
        Auth::Basic { user, pass } if !user.is_empty() => basic = Some(format!("{user}:{pass}")),
        _ => {}
    }
    headers.extend(
        req.headers
            .iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned())),
    );
    let has = |headers: &[(String, String)], name: &str| {
        headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
    };
    if let Some(credentials) = basic
        && !has(&headers, "authorization")
    {
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        headers.push(("Authorization".into(), format!("Basic {encoded}")));
    }
    if body_text(&req.body).is_some_and(|b| !is_json_document(b)) && !has(&headers, "content-type")
    {
        headers.push((
            "Content-Type".into(),
            "application/x-www-form-urlencoded".into(),
        ));
    }
    headers
}

fn assert_round_trips(req: &ResolvedRequest<'_>) {
    let command = curl::export(req).expect("no GraphQL here");
    let parsed = curl::parse(&command);
    let context = || format!("\n  request: {req:?}\n  command: {command}\n  parsed: {parsed:?}");
    assert_eq!(
        parsed.url.as_deref(),
        Some(req.full_url().as_str()),
        "url{}",
        context()
    );
    assert_eq!(
        parsed.method.as_deref(),
        Some(req.method.as_str()),
        "method{}",
        context()
    );
    let headers: Vec<(String, String)> = parsed
        .header
        .iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    assert_eq!(headers, expected_headers(req), "headers{}", context());
    assert_eq!(
        parsed.body.as_deref(),
        body_text(&req.body),
        "body{}",
        context()
    );
    assert!(
        parsed.cookies.is_empty() && parsed.form.is_empty() && parsed.notes.is_empty(),
        "extras{}",
        context()
    );
}

#[test]
fn exported_requests_import_back_unchanged() {
    let mut rng = Rng::new(0xC0_10);
    for _ in 0..cases(3000) {
        assert_round_trips(&round_trip_request(&mut rng));
    }
}

#[test]
fn exported_text_forms_import_back_as_their_urlencoded_body() {
    let mut rng = Rng::new(0xC0_11);
    for _ in 0..cases(2000) {
        let fields: Vec<ResolvedField<'static>> = (0..rng.range(1, 4))
            .map(|_| ResolvedField {
                name: field_text(&mut rng, true),
                value: FieldValue::Text(field_text(&mut rng, true)),
            })
            .filter(|f| !f.name.is_empty())
            .collect();
        if fields.is_empty() {
            continue;
        }
        let expected: Vec<String> = fields
            .iter()
            .map(|f| match &f.value {
                FieldValue::Text(text) => {
                    format!("{}={}", percent_encode(&f.name), percent_encode(text))
                }
                FieldValue::File { .. } => unreachable!("text fields only"),
            })
            .collect();
        let req = ResolvedRequest {
            method: HttpMethod::Post,
            url: "https://x.io/form".into(),
            query: Vec::new(),
            headers: Headers::default(),
            auth: Auth::None,
            body: ResolvedBody::Form(fields),
        };
        let command = curl::export(&req).expect("no GraphQL here");
        let parsed = curl::parse(&command);
        assert_eq!(
            parsed.body.as_deref(),
            Some(expected.join("&").as_str()),
            "{command}"
        );
        assert!(parsed.notes.is_empty(), "{command}: {:?}", parsed.notes);
    }
}

/// Any request, every body and auth kind: export may refuse GraphQL
/// variables that aren't an object, and must not panic on anything else.
#[test]
fn export_never_panics_and_its_output_parses() {
    let mut rng = Rng::new(0xC0_12);
    for _ in 0..cases(3000) {
        let mut req = round_trip_request(&mut rng);
        req.url = field_text(&mut rng, true);
        req.auth = match rng.below(6) {
            0 => Auth::Jwt {
                secret: field_text(&mut rng, true),
                subject: field_text(&mut rng, true),
            },
            1 => Auth::Cookie {
                cookies: field_text(&mut rng, true),
            },
            2 => Auth::Basic {
                user: field_text(&mut rng, true),
                pass: field_text(&mut rng, true),
            },
            _ => req.auth,
        };
        req.body = match rng.below(4) {
            0 => ResolvedBody::GraphQl {
                query: field_text(&mut rng, true),
                variables: (*rng.pick(&["", "{\"a\": 1}", "[1]", "{oops", "null", " "])).to_owned(),
            },
            1 => ResolvedBody::Form(
                (0..rng.below(4))
                    .map(|_| ResolvedField {
                        name: field_text(&mut rng, true),
                        value: if rng.chance(30) {
                            FieldValue::File {
                                file_name: rng.chance(50).then_some("a;type=x\"b.png"),
                                data: None,
                                mime_type: None,
                            }
                        } else {
                            FieldValue::Text(field_text(&mut rng, true))
                        },
                    })
                    .collect(),
            ),
            _ => req.body,
        };
        if let Ok(command) = curl::export(&req) {
            import_everything(&command);
        }
    }
}

#[test]
fn an_exported_get_with_a_body_stays_a_get() {
    let req = ResolvedRequest {
        method: HttpMethod::Get,
        url: "https://x.io/search".into(),
        query: Vec::new(),
        headers: [("Content-Type", "application/json")].into_iter().collect(),
        auth: Auth::None,
        body: ResolvedBody::Json("{\"q\":1}".into()),
    };
    assert_round_trips(&req);
}

#[test]
fn an_exported_empty_header_is_sent_empty() {
    let req = ResolvedRequest {
        method: HttpMethod::Get,
        url: "https://x.io".into(),
        query: Vec::new(),
        headers: [("X-Empty", "")].into_iter().collect(),
        auth: Auth::None,
        body: ResolvedBody::None,
    };
    assert_round_trips(&req);
}

#[test]
fn data_urlencode_names_may_contain_an_at_sign() {
    // curl's data_urlencode() looks for '=' first and only falls back to
    // '@' when there is none, so this is name "user@host", content "x y".
    let parsed = curl::parse("curl https://x.io --data-urlencode 'user@host=x y'");
    assert_eq!(parsed.body.as_deref(), Some("user@host=x%20y"));
    assert!(parsed.notes.is_empty(), "{:?}", parsed.notes);
}

#[test]
fn an_exported_text_body_keeps_the_plain_text_type_it_is_sent_with() {
    let mut saved = rustman_testkit::blank_request("r", "r");
    saved.method = HttpMethod::Put;
    saved.url = "https://x.io/notes".into();
    saved.body_type = BodyType::Text;
    saved.body = "a=1 & b".into();
    let req = saved.resolve(None);
    let parsed = curl::parse(&curl::export(&req).expect("no GraphQL here"));
    assert_eq!(
        parsed.header.iter().collect::<Vec<_>>(),
        [("Content-Type", "text/plain")]
    );
    assert_eq!(parsed.body.as_deref(), Some("a=1 & b"));
    assert_round_trips(&req);
}

#[test]
fn a_removed_header_stops_curl_adding_its_own() {
    let parsed = curl::parse(
        "curl https://x.io -H 'Content-Type:' -d 'a b' -A agent -H 'User-Agent:' -u me:pw -H 'authorization: '",
    );
    assert!(parsed.header.is_empty(), "{:?}", parsed.header);
    assert_eq!(parsed.body.as_deref(), Some("a b"));
}

#[test]
fn url_query_names_may_contain_an_at_sign() {
    let parsed = curl::parse("curl https://x.io --url-query 'user@host=x y'");
    assert_eq!(parsed.url.as_deref(), Some("https://x.io?user@host=x%20y"));
    assert!(parsed.notes.is_empty(), "{:?}", parsed.notes);
}
