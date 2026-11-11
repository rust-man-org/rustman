//! Readable text from HTML bodies. There is no embedded web engine, so the
//! response viewer shows this instead of tag soup.

use std::iter::Peekable;
use std::str::Chars;

use rustman_core::domain::response::HttpResponse;
use rustman_core::text::head_capped;

/// Input past this size isn't converted: the preview would be unreadable
/// anyway, and the source stays one copy away.
const PREVIEW_INPUT_CAP: usize = 1_000_000;

/// Readable text for an HTML response, from at most its first megabyte.
/// `None` for non-HTML and empty bodies.
pub fn html_preview_text(resp: &HttpResponse) -> Option<String> {
    if !resp.is_html() || resp.body.is_empty() {
        return None;
    }
    let input = head_capped(&resp.body, PREVIEW_INPUT_CAP);
    let mut text = html_to_text(input);
    if input.len() < resp.body.len() {
        text.push_str("\n\n[…preview truncated at 1 MB — copy the response for the full source…]");
    }
    Some(text)
}

/// Best-effort text from HTML, in one pass: `<script>`, `<style>` and
/// comments are dropped, block tags break lines, links keep their target in
/// parentheses, entities are decoded and whitespace collapses the way a
/// browser collapses it.
pub fn html_to_text(html: &str) -> String {
    let mut converter = Converter {
        chars: html.chars().peekable(),
        out: String::with_capacity(html.len().min(512_000)),
        links: Vec::new(),
    };
    while let Some(c) = converter.chars.next() {
        match c {
            '<' => converter.markup(),
            '&' => converter.entity(),
            ' ' | '\t' | '\r' | '\n' => {
                if !converter.out.ends_with([' ', '\n']) {
                    converter.out.push(' ');
                }
            }
            c => converter.out.push(c),
        }
    }
    collapse_blank_lines(&converter.out)
}

struct Converter<'a> {
    chars: Peekable<Chars<'a>>,
    out: String,
    /// Targets of the open `<a>` tags, emitted when each one closes.
    links: Vec<String>,
}

impl Converter<'_> {
    /// After a `<`: a tag, comment or directive, or a literal `<` when no
    /// tag can start here (`1 < 2`).
    fn markup(&mut self) {
        match self.chars.peek() {
            Some('!') => {
                self.chars.next();
                if self.chars.next_if_eq(&'-').is_some() && self.chars.next_if_eq(&'-').is_some() {
                    self.skip_comment();
                } else {
                    self.skip_past('>');
                }
            }
            Some('?') => self.skip_past('>'),
            Some(c) if *c == '/' || c.is_ascii_alphabetic() => self.tag(),
            _ => self.out.push('<'),
        }
    }

    fn tag(&mut self) {
        let raw = self.raw_tag();
        let closing = raw.starts_with('/');
        let body = raw.trim_start_matches('/');
        let name_end = body
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(body.len());
        let name = body[..name_end].to_ascii_lowercase();
        match name.as_str() {
            "script" | "style" if !closing => self.skip_raw_text(&name),
            "a" if !closing => self.links.push(href(&body[name_end..])),
            "a" => {
                if let Some(href) = self.links.pop().filter(|h| !h.is_empty()) {
                    self.out.push_str(" (");
                    self.out.push_str(&href);
                    self.out.push(')');
                }
            }
            "br" => self.out.push('\n'),
            "li" if !closing => self.out.push_str("\n• "),
            "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "nav" | "ul"
            | "ol" | "table" | "tr" | "blockquote" | "form" | "hr" | "pre" | "dd" | "dt" | "h1"
            | "h2" | "h3" | "h4" | "h5" | "h6" => self.out.push('\n'),
            _ => {}
        }
    }

    /// The tag's text up to its closing `>`, which may not sit in a quoted
    /// attribute value. Only a quote right after `=` opens one: elsewhere,
    /// as in `alt=don't`, it is part of the text.
    fn raw_tag(&mut self) -> String {
        let mut raw = String::new();
        let mut quote = None;
        let mut value_next = false;
        for c in self.chars.by_ref() {
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else {
                match c {
                    '>' => break,
                    '"' | '\'' if value_next => quote = Some(c),
                    _ => {}
                }
                value_next = c == '=' || (value_next && c.is_whitespace());
            }
            raw.push(c);
        }
        raw
    }

    /// Skips a `<script>` or `<style>` body up to its closing tag.
    fn skip_raw_text(&mut self, name: &str) {
        while let Some(c) = self.chars.next() {
            if c != '<' {
                continue;
            }
            let closing = self.chars.next_if_eq(&'/').is_some();
            let mut tag = String::new();
            while let Some(c) = self.chars.next_if(char::is_ascii_alphanumeric) {
                tag.push(c);
            }
            if closing && tag.eq_ignore_ascii_case(name) {
                self.skip_past('>');
                return;
            }
        }
    }

    /// Skips a comment after its `<!--`, ending it where a browser does: at
    /// `-->` or `--!>`, or at once for the empty `<!-->` and `<!--->`.
    fn skip_comment(&mut self) {
        let mut dashes = 0;
        let mut empty = true;
        while let Some(c) = self.chars.next() {
            match c {
                '-' => dashes += 1,
                '>' if empty || dashes >= 2 => return,
                '!' if dashes >= 2 && self.chars.next_if_eq(&'>').is_some() => return,
                _ => {
                    dashes = 0;
                    empty = false;
                }
            }
        }
    }

    fn skip_past(&mut self, end: char) {
        for c in self.chars.by_ref() {
            if c == end {
                return;
            }
        }
    }

    /// After a `&`: the decoded entity, or the text as written when it is
    /// unknown. A name holds only letters, digits and `#`, so a bare `&`
    /// (`Q&A</b>`) leaves the markup after it alone.
    fn entity(&mut self) {
        let mut name = String::new();
        while let Some(c) = self
            .chars
            .next_if(|c| c.is_ascii_alphanumeric() || *c == '#')
        {
            name.push(c);
        }
        let terminated = self.chars.next_if_eq(&';').is_some();
        match decode_entity(&name) {
            Some(c) => self.out.push(c),
            None => {
                self.out.push('&');
                self.out.push_str(&name);
                if terminated {
                    self.out.push(';');
                }
            }
        }
    }
}

fn decode_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let code = name.strip_prefix('#')?;
            let value = match code.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => code.parse().ok()?,
            };
            match value {
                0 => Some(char::REPLACEMENT_CHARACTER),
                value => char::from_u32(value),
            }
        }
    }
}

/// The `href` value in a tag's attribute text, however it is quoted or
/// spaced (`href="u"`, `href='u'`, `href=u`, `href = "u"`). Empty when
/// absent; `data-href` and `hrefx` don't count.
fn href(attributes: &str) -> String {
    let tokens: Vec<&str> = attributes.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        let Some(rest) = token
            .get(..4)
            .filter(|name| name.eq_ignore_ascii_case("href"))
            .map(|_| &token[4..])
        else {
            continue;
        };
        let value = match rest.strip_prefix('=') {
            Some("") => tokens.get(i + 1).copied(),
            Some(value) => Some(value),
            None if rest.is_empty() && tokens.get(i + 1) == Some(&"=") => {
                tokens.get(i + 2).copied()
            }
            None if rest.is_empty() => tokens.get(i + 1).and_then(|t| t.strip_prefix('=')),
            None => return String::new(),
        };
        return strip_quotes(value.unwrap_or_default()).to_owned();
    }
    String::new()
}

fn strip_quotes(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// Collapses runs of three or more newlines (a block's open and close each
/// emit one) to two, and trims.
fn collapse_blank_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newlines = 0;
    for c in text.chars() {
        if c == '\n' {
            newlines += 1;
            if newlines > 2 {
                continue;
            }
        } else {
            newlines = 0;
        }
        out.push(c);
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
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
            html_to_text("<p>a &amp; b &lt;c&gt; &#65; &#x41; &#39; &unknown;</p>"),
            "a & b <c> A A ' &unknown;"
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
            (
                "<a href= \"https://x.test/e\">t</a>",
                "t (https://x.test/e)",
            ),
            (
                "<a href =\"https://x.test/f\">t</a>",
                "t (https://x.test/f)",
            ),
            ("<a data-href=\"https://x.test/nope\">t</a>", "t"),
            ("<a hrefx=\"https://x.test/nope\">t</a>", "t"),
            ("<a>bare</a>", "bare"),
        ] {
            assert_eq!(html_to_text(tag), want, "for {tag:?}");
        }
    }

    #[test]
    fn directives_and_unclosed_skip_do_not_swallow_text() {
        assert_eq!(html_to_text("<?xml version=\"1.0\"?><p>hi</p>"), "hi");
        assert_eq!(html_to_text("<p>before</p><script>alert(1)"), "before");
    }

    #[test]
    fn previews_are_for_html_and_capped() {
        let mut resp = HttpResponse {
            body: "<p>hi</p>".into(),
            ..HttpResponse::default()
        };
        assert_eq!(
            html_preview_text(&resp),
            None,
            "not HTML without the header"
        );
        resp.headers
            .insert("Content-Type".into(), "text/html; charset=utf-8".into());
        assert_eq!(html_preview_text(&resp).as_deref(), Some("hi"));

        resp.body = format!("<p>{}</p>", "a".repeat(PREVIEW_INPUT_CAP));
        assert!(html_preview_text(&resp).unwrap().ends_with("full source…]"));
    }
}
