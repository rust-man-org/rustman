use serde::Serialize as _;

use crate::domain::response::HttpResponse;

/// Longest char-boundary prefix of `text` within `max_bytes`. Slicing a `str`
/// mid-character panics — fatal under this crate's `panic = "abort"` — so
/// every display cap in the app floors through this instead of hand-rolled
/// byte loops.
pub fn head_capped(text: &str, max_bytes: usize) -> &str {
    let mut end = max_bytes.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

pub fn pretty_json(text: &str, use_tabs: bool) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    pretty_value(&value, use_tabs)
}

pub fn pretty_value(value: &serde_json::Value, use_tabs: bool) -> Option<String> {
    if use_tabs {
        let mut buf = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        value.serialize(&mut ser).ok()?;
        String::from_utf8(buf).ok()
    } else {
        serde_json::to_string_pretty(value).ok()
    }
}

/// Best-effort readable text from an HTML body. Tag soup is unreadable and
/// there is no embedded web engine, so this keeps something useful on screen.
///
/// Single pass with no allocations beyond the output: `<script>`, `<style>`
/// and comments are dropped; block tags break lines; links keep their target
/// in parens; entities (named, decimal, hex) are decoded; runs of spaces
/// collapse the way a browser would. Always whole chars — never slices.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(512_000));
    let mut chars = html.chars().peekable();
    // Tag whose raw content is being skipped (`script`/`style`), if any.
    let mut skipping: Option<bool> = None; // true = script, false = style
    // Hrefs of open `<a>` tags (browsers forbid nesting; the stack is
    // paranoia for malformed pages).
    let mut links: Vec<String> = Vec::new();
    // Open `<pre>` / `<textarea>` depth: whitespace is kept as written there.
    // Debug dumps (Laravel `dd()`, Symfony VarDumper, `var_dump` in `<pre>`)
    // are only readable with their line breaks and indentation.
    let mut pre_depth = 0usize;

    while let Some(c) = chars.next() {
        if c != '<' {
            if skipping.is_some() {
                continue;
            }
            if c == '&' {
                push_entity(&mut chars, &mut out);
            } else if pre_depth > 0 && c != '\r' {
                out.push(c);
            } else if c == ' ' || c == '\t' || c == '\r' || c == '\n' {
                // Collapse whitespace runs like a renderer would.
                if !out.ends_with([' ', '\n']) {
                    out.push(' ');
                }
            } else {
                out.push(c);
            }
            continue;
        }

        // Inside skipped content only the matching close tag matters.
        if skipping.is_some() {
            let want = if skipping == Some(true) {
                "script"
            } else {
                "style"
            };
            let is_close = chars.peek() == Some(&'/');
            if is_close {
                chars.next();
            }
            let mut tok = String::new();
            while let Some(&p) = chars.peek() {
                if p.is_ascii_alphanumeric() {
                    tok.push(p);
                    chars.next();
                } else {
                    break;
                }
            }
            if is_close && tok.eq_ignore_ascii_case(want) {
                skipping = None;
                for p in chars.by_ref() {
                    if p == '>' {
                        break;
                    }
                }
            }
            continue;
        }

        // Only `/name`, `name`, `!directive` and `?pi` can open a tag; a bare
        // `<` before anything else (space, digit, emoji…) is literal text.
        // Without this, `1 < 2` swallows everything up to the next `>`.
        match chars.peek() {
            Some(&'/') | Some(&'!') | Some(&'?') => {}
            Some(c) if c.is_ascii_alphabetic() => {}
            _ => {
                out.push('<');
                continue;
            }
        }

        // Comment or doctype: `<!-- ... -->`, `<!DOCTYPE ...>`.
        if chars.peek() == Some(&'!') {
            chars.next();
            if chars.peek() == Some(&'-') {
                chars.next();
                if chars.peek() == Some(&'-') {
                    chars.next();
                    skip_until(&mut chars, "-->");
                    continue;
                }
            }
            skip_until(&mut chars, ">");
            continue;
        }
        if chars.peek() == Some(&'?') {
            chars.next();
            skip_until(&mut chars, ">");
            continue;
        }

        // Collect the raw tag up to its unquoted '>'. Attribute parsing
        // happens on the string afterwards, where token boundaries are
        // obvious — an incremental machine kept misrouting values.
        let mut raw = String::new();
        let mut in_quote = None;
        for p in chars.by_ref() {
            if let Some(q) = in_quote {
                raw.push(p);
                if p == q {
                    in_quote = None;
                }
                continue;
            }
            match p {
                '"' | '\'' => {
                    in_quote = Some(p);
                    raw.push(p);
                }
                '>' => break,
                _ => raw.push(p),
            }
        }
        let closing = raw.starts_with('/');
        let body = raw.trim_start_matches('/');
        let name_end = body
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(body.len());
        let tag = body[..name_end].to_ascii_lowercase();

        if !closing && (tag == "script" || tag == "style") {
            skipping = Some(tag == "script");
            continue;
        }
        if !closing && tag == "a" {
            links.push(extract_href(&body[name_end..]));
        }
        if matches!(tag.as_str(), "pre" | "textarea") {
            pre_depth = if closing {
                pre_depth.saturating_sub(1)
            } else {
                pre_depth + 1
            };
        }
        handle_tag_end(&tag, closing, &mut links, &mut out);
    }

    // Collapse 3+ newlines (block open + close each emit one) and trim.
    let mut collapsed = String::with_capacity(out.len());
    let mut newlines = 0;
    for c in out.chars() {
        if c == '\n' {
            newlines += 1;
            if newlines <= 2 {
                collapsed.push(c);
            }
        } else {
            newlines = 0;
            collapsed.push(c);
        }
    }
    collapsed.trim().to_owned()
}

/// Readable text for an HTML response body, capped at 1 MB of input.
/// Returns `None` for non-HTML and empty bodies.
///
/// Conversion is linear and one-shot per response, but there is no point
/// rendering megabytes of preview text — the full source stays one copy away.
pub fn html_preview_text(resp: &HttpResponse) -> Option<String> {
    if !resp.is_html() || resp.body.is_empty() {
        return None;
    }

    const INPUT_CAP: usize = 1_000_000;
    let capped = resp.body.len() > INPUT_CAP;
    let slice_end = if capped {
        head_capped(&resp.body, INPUT_CAP).len()
    } else {
        resp.body.len()
    };
    let mut text = html_to_text(&resp.body[..slice_end]);
    if capped {
        text.push_str("\n\n[…preview truncated at 1 MB — copy the response for the full source…]");
    }
    Some(text)
}

/// The `href` value from a tag's attribute text (`<a ...>` remainder),
/// tolerant of quoting and spacing (`href="u"`, `href='u'`, `href=u`,
/// `href = "u"`). Empty when absent. Only standalone `href` matches, so
/// `data-href` and `hrefx` are ignored.
fn extract_href(rest: &str) -> String {
    let toks: Vec<&str> = rest.split_whitespace().collect();
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i];
        let is_href = t.get(..4).is_some_and(|h| h.eq_ignore_ascii_case("href"));
        if !is_href {
            i += 1;
            continue;
        }
        match t[4..].chars().next() {
            // Bare `href`: the value follows `=`.
            None => {
                if toks.get(i + 1) == Some(&"=") {
                    let val = toks.get(i + 2).copied().unwrap_or("");
                    return strip_quotes(val).to_owned();
                }
                return String::new();
            }
            Some(c) if c == '=' || c.is_whitespace() => {
                let mut v = t[4..].trim_start_matches(['=', ' ', '\t']);
                if v.is_empty() {
                    // `href = "u"`: value lives in later tokens.
                    i += 1;
                    v = toks.get(i).copied().unwrap_or("");
                    if v == "=" {
                        i += 1;
                        v = toks.get(i).copied().unwrap_or("");
                    }
                }
                return strip_quotes(v).to_owned();
            }
            // `hrefx=...` and friends: not our attribute.
            _ => return String::new(),
        }
    }
    String::new()
}

fn strip_quotes(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        &v[1..v.len() - 1]
    } else {
        v
    }
}
fn handle_tag_end(tag: &str, closing: bool, links: &mut Vec<String>, out: &mut String) {
    match tag {
        "br" => out.push('\n'),
        "a" if closing => {
            if let Some(href) = links.pop() {
                if !href.is_empty() {
                    out.push_str(" (");
                    out.push_str(&href);
                    out.push(')');
                }
            }
        }
        "li" if !closing => out.push_str("\n• "),
        "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "nav" | "ul"
        | "ol" | "table" | "tr" | "blockquote" | "form" | "hr" | "pre" | "dd" | "dt" | "h1"
        | "h2" | "h3" | "h4" | "h5" | "h6" => out.push('\n'),
        _ => {}
    }
}

/// Skip chars until (and including) the `pat` terminator.
fn skip_until(chars: &mut std::iter::Peekable<std::str::Chars>, pat: &str) {
    let mut window = String::new();
    for c in chars.by_ref() {
        window.push(c);
        if window.len() > pat.len() {
            window.remove(0);
        }
        if window == pat {
            break;
        }
    }
}

/// Decode one `&...;` entity (caller already consumed `&`). An unterminated
/// `&foo` (no `;`) is emitted literally, terminator included when present.
fn push_entity(chars: &mut std::iter::Peekable<std::str::Chars>, out: &mut String) {
    let mut entity = String::new();
    let mut terminated = false;
    for c in chars.by_ref() {
        if c == ';' {
            terminated = true;
            break;
        }
        entity.push(c);
        if entity.len() > 10 {
            break;
        }
    }
    let decoded = match entity.as_str() {
        "amp" => Some("&".to_owned()),
        "lt" => Some("<".to_owned()),
        "gt" => Some(">".to_owned()),
        "quot" => Some("\"".to_owned()),
        "apos" | "#39" => Some("'".to_owned()),
        "nbsp" => Some(" ".to_owned()),
        _ if entity.starts_with("#x") || entity.starts_with("#X") => {
            u32::from_str_radix(entity.trim_start_matches(['#', 'x', 'X']), 16)
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string())
        }
        _ if entity.starts_with('#') => entity[1..]
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(|c| c.to_string()),
        _ => None,
    };
    match decoded {
        Some(s) => out.push_str(&s),
        None => {
            out.push('&');
            out.push_str(&entity);
            if terminated {
                out.push(';');
            }
        }
    }
}

#[cfg(test)]
mod html_tests {
    use super::*;

    #[test]
    fn drops_script_style_and_comments() {
        let html = "<html><head><style>.a{color:red}</style><script>alert(1)</script></head>\
            <body><!-- hidden -->Hello</body></html>";
        assert_eq!(html_to_text(html), "Hello");
    }

    #[test]
    fn blocks_break_lines_and_links_keep_targets() {
        let html = "<div>First</div><p>Click <a href=\"https://x.test/y\">here</a> now</p>";
        let text = html_to_text(html);
        assert!(text.contains("First"), "{text:?}");
        assert!(text.contains("here (https://x.test/y)"), "{text:?}");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(
            html_to_text("<p>a &amp; b &lt;c&gt; &#65; &#x41; &unknown;</p>"),
            "a & b <c> A A &unknown;"
        );
    }

    #[test]
    fn bare_angle_bracket_is_literal() {
        assert_eq!(html_to_text("<p>1 < 2</p>"), "1 < 2");
    }

    #[test]
    fn unclosed_tag_at_eof_does_not_hang_or_panic() {
        let text = html_to_text("<div>oops<span>no close");
        assert!(text.contains("oops"), "{text:?}");
    }

    #[test]
    fn href_parsing_tolerates_spacing_and_quoting() {
        for (tag, want) in [
            ("<a href=\"https://x.test/a\">t</a>", "t (https://x.test/a)"),
            ("<a href='https://x.test/b'>t</a>", "t (https://x.test/b)"),
            ("<a href=https://x.test/c>t</a>", "t (https://x.test/c)"),
            (
                "<a href = \"https://x.test/d\">t</a>",
                "t (https://x.test/d)",
            ),
            ("<a data-href=\"https://x.test/nope\">t</a>", "t"),
            ("<a>bare</a>", "bare"),
        ] {
            assert_eq!(html_to_text(tag), want, "for {tag:?}");
        }
    }

    #[test]
    fn directives_and_unclosed_skip_do_not_swallow_text() {
        assert_eq!(html_to_text("<?xml version=\"1.0\"?><p>hi</p>"), "hi");
        // Unclosed script: rest is code, but preceding text survives.
        assert_eq!(html_to_text("<p>before</p><script>alert(1)"), "before");
    }

    #[test]
    fn laravel_dd_dump_keeps_its_lines() {
        // Shape of Symfony VarDumper HTML, which Laravel `dd()` returns.
        let html = "<script> Sfdump = window.Sfdump || (function (doc) {})(document)</script>\
            <style>pre.sf-dump{display:block}</style>\
            <pre class=sf-dump id=sf-dump-1 data-indent-pad=\"  \">\
            <span class=sf-dump-note>array:2</span> [<samp data-depth=1 class=sf-dump-expanded>\n  \
            \"<span class=sf-dump-key>name</span>\" =&gt; \"<span class=sf-dump-str title=\"5 characters\">Yosua</span>\"\n  \
            \"<span class=sf-dump-key>id</span>\" =&gt; <span class=sf-dump-num>1</span>\n\
            </samp>]\n</pre><script>Sfdump(\"sf-dump-1\")</script>";
        assert_eq!(
            html_to_text(html),
            "array:2 [\n  \"name\" => \"Yosua\"\n  \"id\" => 1\n]"
        );
    }
}
