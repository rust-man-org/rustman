//! Fuzz and property tests for the HTML-to-text preview.
//!
//! Response bodies come from any server, so the converter must not panic on
//! tag soup, must stay linear on huge bodies, and must never show what a
//! browser hides (`<script>` and `<style>` bodies, comments).

mod fuzz_support;

use std::time::Duration;

use fuzz_support::{Rng, assert_linear, cases, check_all, fuzz, size};
use rustman_core::domain::response::HttpResponse;
use rustman_formats::html::{html_preview_text, html_to_text};

const TAGS: &[&str] = &[
    "<p>",
    "</p>",
    "<div class=\"a\">",
    "</div>",
    "<br>",
    "<br/>",
    "<li>",
    "<ul>",
    "<a href=\"https://x.test/a\">",
    "<a href='u'>",
    "<a href=u>",
    "<a href = \"v\">",
    "<a>",
    "</a>",
    "<A HREF=\"W\">",
    "<script>",
    "</script>",
    "<script type=\"x\">",
    "</script >",
    "<style>",
    "</style>",
    "<!--",
    "-->",
    "--->",
    "<!-- c -->",
    "<!DOCTYPE html>",
    "<?xml version=\"1.0\"?>",
    "<!",
    "<?",
    "<",
    ">",
    "</",
    "<a href=\"unterminated>",
    "<p title='it>s'>",
    "<h1>",
    "</h6>",
    "<pre>",
    "<table><tr><td>",
    "<img alt=\"é😀\">",
    "<x:y>",
    "<1>",
];

const ENTITIES: &[&str] = &[
    "&amp;",
    "&lt;",
    "&gt;",
    "&quot;",
    "&apos;",
    "&nbsp;",
    "&#65;",
    "&#x41;",
    "&#X41;",
    "&#0;",
    "&#xD800;",
    "&#xDFFF;",
    "&#x10FFFF;",
    "&#x110000;",
    "&#xFFFFFFFF;",
    "&#x100000000;",
    "&#99999999999999999999;",
    "&#-1;",
    "&#x+41;",
    "&#;",
    "&#x;",
    "&;",
    "&",
    "&&&",
    "&amp",
    "&unknown;",
    "&averyveryverylongname;",
    "&é;",
    "&😀;",
];

fn soup(rng: &mut Rng) -> String {
    let mut s = String::new();
    for _ in 0..rng.range(1, 24) {
        match rng.below(4) {
            0 => s.push_str(rng.pick::<&str>(TAGS)),
            1 => s.push_str(rng.pick::<&str>(ENTITIES)),
            2 => s.push_str(&rng.text(10)),
            _ => s.push_str(&rng.text_from(
                &[
                    '<', '>', '&', ';', '"', '\'', '/', '-', '!', ' ', '\n', 'a', '#', 'x',
                ],
                8,
            )),
        }
    }
    s
}

fn convert_everything(html: &str) {
    let text = html_to_text(html);
    assert_eq!(text, text.trim(), "the text is trimmed");
    let mut resp = HttpResponse {
        body: html.to_owned(),
        ..HttpResponse::default()
    };
    resp.headers
        .insert("Content-Type".into(), "text/html; charset=utf-8".into());
    let _ = html_preview_text(&resp);
}

#[test]
fn random_bytes_never_panic() {
    fuzz(
        0x4_0001,
        cases(5000),
        |rng| {
            let len = rng.below(300);
            String::from_utf8_lossy(&rng.bytes(len)).into_owned()
        },
        convert_everything,
    );
}

#[test]
fn tag_and_entity_soup_never_panics() {
    fuzz(0x4_0002, cases(20_000), soup, convert_everything);
}

#[test]
fn every_entity_edge_case_converts() {
    for entity in ENTITIES {
        for html in [
            (*entity).to_owned(),
            format!("a{entity}b"),
            format!("<p>{entity}</p>"),
            format!("{entity}{entity}"),
        ] {
            convert_everything(&html);
        }
    }
}

#[test]
fn numeric_entities_decode_to_their_character_or_stay_literal() {
    assert_eq!(html_to_text("&#x10FFFF;"), "\u{10ffff}");
    for literal in [
        "&#xD800;",
        "&#x110000;",
        "&#xFFFFFFFF;",
        "&#99999999999999999999;",
    ] {
        assert_eq!(html_to_text(literal), literal);
    }
}

#[test]
fn a_nul_entity_becomes_the_replacement_character() {
    assert_eq!(html_to_text("a&#0;b"), "a\u{fffd}b");
}

#[test]
fn numeric_entities_are_digits_only() {
    for literal in ["&#x+41;", "&#+65;", "&#-65;", "&#x;", "&#;"] {
        assert_eq!(html_to_text(literal), literal);
    }
}

/// Text a browser never shows, wrapped in random but well-formed markup.
const HIDDEN: &str = "HIDDEN_MARKER";

fn hiding_document(rng: &mut Rng) -> String {
    let mut s = String::new();
    for _ in 0..rng.range(1, 8) {
        match rng.below(6) {
            0 => s.push_str(&format!("<script>var x = '{HIDDEN}';</script>")),
            1 => s.push_str(&format!("<style>.{HIDDEN} {{ color: red }}</style>")),
            2 => s.push_str(&format!("<!-- {HIDDEN} -->")),
            3 => s.push_str(rng.pick::<&str>(&["<p>", "</p>", "<div>", "</div>", "<br>", "<li>"])),
            4 => s.push_str(&rng.text_from(
                &['a', ' ', '\n', 'é', '1', '.', ',', '>', '=', '(', ')'],
                12,
            )),
            // Bare ampersands are everywhere in real HTML: "Q&A", "AT&T".
            _ => s.push_str(rng.pick::<&str>(&["Q&A", "AT&T ", "a & b", "&amp;", "&copy;", "R&D"])),
        }
    }
    s
}

#[test]
fn script_style_and_comment_bodies_never_show() {
    check_all(0x4_0003, cases(5000), hiding_document, |html| {
        let text = html_to_text(html);
        assert!(!text.contains(HIDDEN), "{html:?} showed {text:?}");
    });
}

#[test]
fn a_bare_ampersand_does_not_swallow_the_next_tag() {
    assert_eq!(html_to_text("<b>Q&A</b> time"), "Q&A time");
    assert_eq!(html_to_text("AT&T<script>secret()</script>"), "AT&T");
}

#[test]
fn hidden_bodies_never_show_without_bare_ampersands() {
    check_all(
        0x4_0004,
        cases(5000),
        |rng| {
            hiding_document(rng)
                .replace("Q&A", "QA")
                .replace("AT&T ", "ATT ")
                .replace("a & b", "a b")
                .replace("R&D", "RD")
                .replace("&copy;", "(c)")
        },
        |html| {
            let text = html_to_text(html);
            assert!(!text.contains(HIDDEN), "{html:?} showed {text:?}");
        },
    );
}

#[test]
fn unclosed_constructs_end_at_end_of_input() {
    for html in [
        "<!-- never closed",
        "<script>never closed",
        "<style>never closed",
        "<a href=\"never closed",
        "<p title='x",
        "<!DOCTYPE",
        "<?xml",
        "&#x",
        "&",
        "<",
        "<a href=",
        "<a href",
        "<a href =",
    ] {
        convert_everything(html);
        convert_everything(&format!("before {html}"));
    }
}

// ── Linear time ──────────────────────────────────────────────────────────────

const BUDGET: Duration = Duration::from_secs(3);

type Shape = (&'static str, fn(usize) -> String);

fn repeat_n(piece: &str, total: usize) -> String {
    piece.repeat((total / piece.len()).max(1))
}

const SHAPES: &[Shape] = &[
    ("plain text", |n| "word ".repeat(n / 5)),
    ("paragraphs", |n| repeat_n("<p>hello <b>world</b></p>\n", n)),
    ("bare less-thans", |n| "<".repeat(n)),
    ("bare ampersands", |n| "&".repeat(n)),
    ("entities", |n| repeat_n("&amp;&#x41;&nbsp;&bogus;", n)),
    ("open anchors", |n| {
        repeat_n("<a href=\"https://x.test\">x", n)
    }),
    ("closed anchors", |n| {
        repeat_n("<a href=\"https://x.test\">x</a>", n)
    }),
    ("a script full of tags", |n| {
        format!("<script>{}</script>", repeat_n("<b></c><scrip>", n))
    }),
    ("an unclosed script", |n| {
        format!("<script>{}", "<".repeat(n))
    }),
    ("an unclosed comment", |n| {
        format!("<!--{}", "- -".repeat(n / 3))
    }),
    ("dashes in a comment", |n| {
        format!("<!--{}-->", "-".repeat(n))
    }),
    ("one huge tag", |n| {
        format!("<a {}>", "href=x ".repeat(n / 7))
    }),
    ("an unclosed quote in a tag", |n| {
        format!("<p title=\"{}", "a>".repeat(n / 2))
    }),
    ("blank lines", |n| repeat_n("<p>\n\n\n</p>", n)),
    ("multibyte text", |n| "é😀中".repeat(n / 9)),
];

#[test]
fn big_bodies_convert_in_linear_time() {
    // 600 KB by default; a long run goes to 10 MB, ten times the preview cap.
    let n = size(150_000, 2_500_000);
    for (label, make) in SHAPES {
        assert_linear(label, n, make, Some(BUDGET), |html| {
            let _ = html_to_text(html);
        });
    }
}

#[test]
fn quirky_markup_does_not_hide_the_rest_of_the_page() {
    // Browsers keep `'` as part of an unquoted value and end `<!-->` at once.
    assert_eq!(html_to_text("<img alt=don't>Hello"), "Hello");
    assert_eq!(html_to_text("<!-->Hello"), "Hello");
    assert_eq!(html_to_text("<!--->Hello"), "Hello");
    assert_eq!(html_to_text("<!-- a --!>Hello"), "Hello");
    assert_eq!(html_to_text("<!----!>Hello"), "Hello");
    assert_eq!(html_to_text("<!--!>x-->Hello"), "Hello");
    assert_eq!(html_to_text("<!---!>x-->Hello"), "Hello");
    assert_eq!(html_to_text("<!-- a -> b -->Hello"), "Hello");
    assert_eq!(html_to_text("<p title='it>s'>Hello"), "Hello");
}
