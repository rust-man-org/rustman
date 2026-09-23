//! HTML response bodies parsed into a small, renderer-agnostic model.
//!
//! HTML previews used to be drawn by an embedded native webview (WebKitGTK on
//! Linux, WebView2 on Windows, WKWebView on macOS) overlaid on the response
//! panel. That tied a preview to a native dependency most machines do not have,
//! and the child window cannot attach at all under native Wayland. This module
//! instead parses the response and keeps only what is worth showing in a viewer:
//! headings, paragraphs, emphasis, links, lists, quotes, code and tables.
//! `ui::response::html` draws that model with ordinary widgets, so every
//! platform takes the same code path and nothing native is needed to build or
//! run the app.
//!
//! The model is deliberately a subset. Scripts, styles and embedded media are
//! dropped rather than guessed at, and the raw response body stays one click
//! away in the response panel's source view.

use scraper::{ElementRef, Html, Node};

/// Nesting depth past which the walker stops descending.
///
/// `html5ever` happily builds a tree for `"<div>".repeat(100_000)`, and the walk
/// below is recursive, so an unbounded depth would be a stack overflow on
/// hostile (or merely generated) markup. Past this the content already collected
/// is kept and the subtree is skipped.
const MAX_DEPTH: usize = 64;

/// A parsed HTML body, ready to draw.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { level: u8, spans: Vec<Inline> },
    Paragraph(Vec<Inline>),
    /// Preformatted text (`<pre>`), whitespace preserved.
    Code(String),
    List(List),
    Quote(Vec<Block>),
    Table(Table),
    Rule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct List {
    pub ordered: bool,
    /// First number of an ordered list (`<ol start="3">`).
    pub start: u64,
    pub items: Vec<Vec<Block>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    pub headers: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
}

/// A run of text plus the formatting that applies to it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inline {
    pub text: String,
    pub style: InlineStyle,
    /// Absolute `http`/`https`/`mailto` URL when this run is a link. Anything
    /// else (relative, `javascript:`, `data:`, fragment) is not a link we are
    /// willing to hand to the OS opener, so it stays plain text.
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub code: bool,
}

/// Parses an HTML body into the preview model.
///
/// `base_url` is the URL the response came from and is used to resolve relative
/// links. Parsing never fails: `html5ever` recovers from malformed markup the
/// way a browser does, and unparsable input simply yields no blocks.
pub fn parse(source: &str, base_url: &str) -> Document {
    let html = Html::parse_document(source);
    let base = url::Url::parse(base_url).ok();
    let ctx = Ctx { base: base.as_ref() };
    let mut blocks = Vec::new();
    collect_blocks(html.root_element(), &ctx, 0, &mut blocks);
    Document { blocks }
}

struct Ctx<'a> {
    base: Option<&'a url::Url>,
}

/// Tags whose content carries nothing worth previewing.
fn is_skipped(tag: &str) -> bool {
    matches!(
        tag,
        "script"
            | "style"
            | "head"
            | "title"
            | "meta"
            | "link"
            | "base"
            | "template"
            | "noscript"
            | "svg"
            | "math"
            | "iframe"
            | "canvas"
            | "audio"
            | "video"
            | "object"
            | "embed"
            | "source"
            | "track"
            | "input"
            | "textarea"
            | "select"
            | "option"
            | "button"
    )
}

/// Tags that flow inside a line of text. Everything else is treated as a block,
/// which is the split the default browser stylesheet makes.
fn is_inline(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "abbr"
            | "b"
            | "bdi"
            | "bdo"
            | "br"
            | "cite"
            | "code"
            | "data"
            | "del"
            | "dfn"
            | "em"
            | "font"
            | "i"
            | "img"
            | "ins"
            | "kbd"
            | "label"
            | "mark"
            | "output"
            | "q"
            | "rp"
            | "rt"
            | "ruby"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strike"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "u"
            | "var"
            | "wbr"
    )
}

/// Appends the blocks produced by `parent`'s children, buffering runs of inline
/// content into paragraphs.
fn collect_blocks(parent: ElementRef<'_>, ctx: &Ctx<'_>, depth: usize, out: &mut Vec<Block>) {
    if depth > MAX_DEPTH {
        return;
    }
    let mut paragraph: Vec<Inline> = Vec::new();
    for child in parent.children() {
        match child.value() {
            Node::Text(text) => push_text(&mut paragraph, text, InlineStyle::default()),
            Node::Element(el) => {
                let tag = el.name();
                if is_skipped(tag) {
                    continue;
                }
                let Some(element) = ElementRef::wrap(child) else {
                    continue;
                };
                if is_inline(tag) {
                    inline_element(
                        tag,
                        element,
                        ctx,
                        InlineStyle::default(),
                        &mut paragraph,
                        depth + 1,
                    );
                } else {
                    flush_paragraph(&mut paragraph, out);
                    out.extend(block_element(tag, element, ctx, depth + 1));
                }
            }
            _ => {}
        }
    }
    flush_paragraph(&mut paragraph, out);
}

/// The blocks a block-level element contributes.
fn block_element(tag: &str, el: ElementRef<'_>, ctx: &Ctx<'_>, depth: usize) -> Vec<Block> {
    if depth > MAX_DEPTH {
        return Vec::new();
    }
    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag.as_bytes()[1] - b'0';
            let mut spans = Vec::new();
            collect_inline(el, ctx, InlineStyle::default(), &mut spans, depth + 1);
            let mut blocks = Vec::new();
            // Reuse the paragraph flush so heading text is trimmed and collapsed
            // exactly like body text.
            flush_paragraph(&mut spans, &mut blocks);
            match blocks.pop() {
                Some(Block::Paragraph(spans)) => vec![Block::Heading { level, spans }],
                _ => Vec::new(),
            }
        }
        "p" | "div" | "section" | "article" | "aside" | "nav" | "main" | "header" | "footer"
        | "form" | "fieldset" | "figure" | "address" | "details" | "summary" | "dialog"
        | "center" | "caption" | "dd" | "dt" | "html" | "body" => {
            let mut blocks = Vec::new();
            collect_blocks(el, ctx, depth + 1, &mut blocks);
            blocks
        }
        "ul" | "ol" => vec![Block::List(list(el, ctx, tag == "ol", depth))],
        "blockquote" => {
            let mut blocks = Vec::new();
            collect_blocks(el, ctx, depth + 1, &mut blocks);
            vec![Block::Quote(blocks)]
        }
        "pre" => vec![Block::Code(el.text().collect::<String>().trim_matches('\n').to_owned())],
        "hr" => vec![Block::Rule],
        "table" => {
            let table = table(el, ctx, depth);
            if table.headers.is_empty() && table.rows.is_empty() {
                Vec::new()
            } else {
                vec![Block::Table(table)]
            }
        }
        // Unknown or layout-only elements: show what is inside them.
        _ => {
            let mut blocks = Vec::new();
            collect_blocks(el, ctx, depth + 1, &mut blocks);
            blocks
        }
    }
}

/// Appends the inline content of `parent`, carrying the formatting inherited
/// from its ancestors.
fn collect_inline(
    parent: ElementRef<'_>,
    ctx: &Ctx<'_>,
    style: InlineStyle,
    out: &mut Vec<Inline>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    for child in parent.children() {
        match child.value() {
            Node::Text(text) => push_text(out, text, style),
            Node::Element(el) => {
                let tag = el.name();
                if is_skipped(tag) {
                    continue;
                }
                let Some(element) = ElementRef::wrap(child) else {
                    continue;
                };
                inline_element(tag, element, ctx, style, out, depth + 1);
            }
            _ => {}
        }
    }
}

/// Appends one inline element's content, applying whatever formatting the
/// element itself carries. Shared by block and inline contexts so `<br>`,
/// `<img>` and `<strong>` behave the same wherever they appear.
fn inline_element(
    tag: &str,
    element: ElementRef<'_>,
    ctx: &Ctx<'_>,
    style: InlineStyle,
    out: &mut Vec<Inline>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    match tag {
        "br" => push_run(out, "\n", style),
        "wbr" => {}
        "img" => push_alt_text(out, element, style),
        "a" => {
            let link = element.attr("href").and_then(|href| resolve_link(href, ctx));
            collect_inline(element, ctx, style, out, depth);
            if let Some(url) = link {
                // Every run just appended (and everything merged into it)
                // belongs to this anchor. A nested anchor's runs already carry
                // their own link, which stops the walk.
                for run in out.iter_mut().rev().take_while(|run| run.link.is_none()) {
                    run.link = Some(url.clone());
                }
            }
        }
        "b" | "strong" => {
            collect_inline(element, ctx, InlineStyle { bold: true, ..style }, out, depth)
        }
        "i" | "em" | "cite" | "dfn" | "q" | "var" => {
            collect_inline(element, ctx, InlineStyle { italic: true, ..style }, out, depth)
        }
        "u" | "ins" => {
            collect_inline(element, ctx, InlineStyle { underline: true, ..style }, out, depth)
        }
        "s" | "strike" | "del" => {
            collect_inline(element, ctx, InlineStyle { strike: true, ..style }, out, depth)
        }
        "code" | "kbd" | "samp" => {
            collect_inline(element, ctx, InlineStyle { code: true, ..style }, out, depth)
        }
        _ => collect_inline(element, ctx, style, out, depth),
    }
}

/// Appends an image's alternative text, since a preview cannot draw the image
/// itself. An `alt`-less image contributes nothing — that is how a browser
/// treats it too, and it keeps decorative and tracking pixels out of the text.
fn push_alt_text(out: &mut Vec<Inline>, element: ElementRef<'_>, style: InlineStyle) {
    let alt = element.attr("alt").map(str::trim).filter(|alt| !alt.is_empty());
    let text = match alt {
        Some(alt) => alt.to_owned(),
        None => return,
    };
    push_run(out, &text, InlineStyle { italic: true, ..style });
}

fn resolve_link(href: &str, ctx: &Ctx<'_>) -> Option<String> {
    let href = href.trim();
    // A fragment cannot navigate anywhere in a static preview.
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    let url = match ctx.base {
        Some(base) => base.join(href).ok()?,
        None => url::Url::parse(href).ok()?,
    };
    match url.scheme() {
        "http" | "https" | "mailto" => Some(url.to_string()),
        _ => None,
    }
}

fn list(el: ElementRef<'_>, ctx: &Ctx<'_>, ordered: bool, depth: usize) -> List {
    let start = el.attr("start").and_then(|start| start.trim().parse().ok()).unwrap_or(1);
    let items = el
        .children()
        .filter_map(ElementRef::wrap)
        .filter(|child| child.value().name() == "li")
        .map(|item| {
            let mut blocks = Vec::new();
            collect_blocks(item, ctx, depth + 1, &mut blocks);
            blocks
        })
        .collect();
    List { ordered, start, items }
}

fn table(el: ElementRef<'_>, ctx: &Ctx<'_>, depth: usize) -> Table {
    // Nested tables are flattened into the outer one: a preview does not need
    // the layout machinery of a browser.
    let in_header = |row: &ElementRef<'_>| {
        row.ancestors()
            .filter_map(ElementRef::wrap)
            .any(|ancestor| ancestor.value().name() == "thead")
    };
    let rows: Vec<ElementRef<'_>> = el
        .children()
        .filter_map(ElementRef::wrap)
        .flat_map(|child| match child.value().name() {
            "thead" | "tbody" | "tfoot" => child.child_elements().collect::<Vec<_>>(),
            _ => vec![child],
        })
        .filter(|row| row.value().name() == "tr")
        .collect();

    let parsed: Vec<(bool, Vec<Vec<Inline>>)> = rows
        .iter()
        .map(|row| {
            let mut header_cells = true;
            let mut cells = Vec::new();
            for cell in row.child_elements() {
                let tag = cell.value().name();
                if tag != "td" && tag != "th" {
                    continue;
                }
                header_cells &= tag == "th";
                let mut spans = Vec::new();
                collect_inline(cell, ctx, InlineStyle::default(), &mut spans, depth + 1);
                let mut blocks = Vec::new();
                flush_paragraph(&mut spans, &mut blocks);
                cells.push(match blocks.pop() {
                    Some(Block::Paragraph(spans)) => spans,
                    _ => Vec::new(),
                });
            }
            let is_header = !cells.is_empty() && (header_cells || in_header(row));
            (is_header, cells)
        })
        .filter(|(_, cells)| !cells.is_empty())
        .collect();

    let width = parsed.iter().map(|(_, cells)| cells.len()).max().unwrap_or(0);
    let mut table = Table::default();
    for (is_header, mut cells) in parsed {
        // Ragged tables are padded so every row lines up.
        cells.resize(width, Vec::new());
        if is_header && table.headers.is_empty() {
            table.headers = cells;
        } else {
            table.rows.push(cells);
        }
    }
    table
}

/// Appends `text`, collapsing runs of whitespace the way HTML does.
fn push_text(out: &mut Vec<Inline>, text: &str, style: InlineStyle) {
    let mut collapsed = String::with_capacity(text.len());
    let mut in_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !in_space {
                collapsed.push(' ');
                in_space = true;
            }
        } else {
            collapsed.push(ch);
            in_space = false;
        }
    }
    // Never start a paragraph with a space, and never double the space that a
    // neighbouring run already ended with.
    if collapsed.starts_with(' ') && out.last().is_none_or(|run| run.text.ends_with([' ', '\n'])) {
        collapsed.remove(0);
    }
    push_run(out, &collapsed, style);
}

fn push_run(out: &mut Vec<Inline>, text: &str, style: InlineStyle) {
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(last) if last.style == style && last.link.is_none() => last.text.push_str(text),
        _ => out.push(Inline { text: text.to_owned(), style, link: None }),
    }
}

/// Ends the run of inline content in `buffer`, if any, as a paragraph.
fn flush_paragraph(buffer: &mut Vec<Inline>, out: &mut Vec<Block>) {
    if let Some(last) = buffer.last_mut() {
        let trimmed = last.text.trim_end().len();
        last.text.truncate(trimmed);
    }
    buffer.retain(|run| !run.text.is_empty());
    if !buffer.is_empty() {
        out.push(Block::Paragraph(std::mem::take(buffer)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(blocks: &[Block]) -> String {
        fn inline_text(spans: &[Inline]) -> String {
            spans.iter().map(|run| run.text.as_str()).collect()
        }
        blocks
            .iter()
            .map(|block| match block {
                Block::Heading { spans, .. } | Block::Paragraph(spans) => inline_text(spans),
                Block::Code(code) => code.clone(),
                Block::List(list) => {
                    list.items.iter().map(|item| text_of(item)).collect::<Vec<_>>().join("\n")
                }
                Block::Quote(blocks) => text_of(blocks),
                Block::Table(table) => table
                    .headers
                    .iter()
                    .chain(table.rows.iter().flatten())
                    .map(|cell| inline_text(cell))
                    .collect::<Vec<_>>()
                    .join("|"),
                Block::Rule => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn parse_body(body: &str) -> Document {
        parse(body, "https://api.example.com/v1/things")
    }

    #[test]
    fn decodes_entities_and_collapses_whitespace() {
        let doc = parse_body("<p>a &amp; b&nbsp;c &#65;   d\n\n  e</p>");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(text_of(&doc.blocks), "a & b c A d e");
    }

    #[test]
    fn keeps_one_space_across_inline_boundaries() {
        let doc = parse_body("<p>foo <strong>bold</strong> <em>italic</em></p>");
        let Block::Paragraph(spans) = &doc.blocks[0] else { panic!("expected a paragraph") };
        assert_eq!(
            spans.iter().map(|run| run.text.as_str()).collect::<String>(),
            "foo bold italic"
        );
        assert!(spans.iter().any(|run| run.style.bold));
        assert!(spans.iter().any(|run| run.style.italic));
    }

    #[test]
    fn separates_block_elements_into_their_own_blocks() {
        let doc = parse_body("<div>one</div><div>two</div><p>three</p>");
        assert_eq!(text_of(&doc.blocks), "one\ntwo\nthree");
    }

    #[test]
    fn strips_scripts_styles_and_head() {
        let doc = parse_body(
            "<html><head><title>t</title><style>p{color:red}</style></head>\
             <body><script>alert(1)</script><p>kept</p></body></html>",
        );
        assert_eq!(text_of(&doc.blocks), "kept");
    }

    #[test]
    fn headings_keep_level_and_inline_formatting() {
        let doc = parse_body("<h2>Hi <em>there</em></h2>");
        let Block::Heading { level, spans } = &doc.blocks[0] else { panic!("expected a heading") };
        assert_eq!(*level, 2);
        assert_eq!(spans.iter().map(|run| run.text.as_str()).collect::<String>(), "Hi there");
        assert!(spans.iter().any(|run| run.style.italic));
    }

    #[test]
    fn preformatted_text_is_kept_verbatim() {
        let doc = parse_body("<pre>line 1\n    indented\n</pre>");
        assert_eq!(doc.blocks[0], Block::Code("line 1\n    indented".to_owned()));
    }

    #[test]
    fn line_breaks_survive_inside_a_paragraph() {
        let doc = parse_body("<p>a<br>b</p>");
        assert_eq!(text_of(&doc.blocks), "a\nb");
    }

    #[test]
    fn lists_nest_within_items() {
        let doc = parse_body(
            "<ul><li>one<ul><li>nested</li></ul></li><li>two</li></ul>",
        );
        let Block::List(list) = &doc.blocks[0] else { panic!("expected a list") };
        assert!(!list.ordered);
        assert_eq!(list.items.len(), 2);
        assert_eq!(text_of(&list.items[0]), "one\nnested");
        assert_eq!(text_of(&list.items[1]), "two");
    }

    #[test]
    fn ordered_lists_keep_their_start() {
        let doc = parse_body("<ol start=\"3\"><li>three</li></ol>");
        let Block::List(list) = &doc.blocks[0] else { panic!("expected a list") };
        assert!(list.ordered);
        assert_eq!(list.start, 3);
    }

    #[test]
    fn tables_split_headers_from_rows() {
        let doc = parse_body(
            "<table><thead><tr><th>Name</th><th>Age</th></tr></thead>\
             <tbody><tr><td>Ada</td><td>36</td></tr><tr><td>Alan</td></tr></tbody></table>",
        );
        let Block::Table(table) = &doc.blocks[0] else { panic!("expected a table") };
        assert_eq!(text_of(&[Block::Paragraph(table.headers[0].clone())]), "Name");
        assert_eq!(table.headers.len(), 2);
        assert_eq!(table.rows.len(), 2);
        // A short row is padded so every row has the same number of cells.
        assert_eq!(table.rows[1].len(), 2);
        assert!(table.rows[1][1].is_empty());
    }

    #[test]
    fn header_less_tables_are_all_rows() {
        let doc = parse_body("<table><tr><td>a</td><td>b</td></tr></table>");
        let Block::Table(table) = &doc.blocks[0] else { panic!("expected a table") };
        assert!(table.headers.is_empty());
        assert_eq!(table.rows.len(), 1);
    }

    #[test]
    fn links_are_resolved_against_the_request_url() {
        let doc = parse_body(
            "<p><a href=\"/docs\">docs</a> <a href=\"https://x.test/y\">abs</a> \
             <a href=\"mailto:a@b.test\">mail</a></p>",
        );
        let Block::Paragraph(spans) = &doc.blocks[0] else { panic!("expected a paragraph") };
        let link_of = |text: &str| {
            spans
                .iter()
                .find(|run| run.text.contains(text))
                .and_then(|run| run.link.as_deref())
        };
        assert_eq!(link_of("docs"), Some("https://api.example.com/docs"));
        assert_eq!(link_of("abs"), Some("https://x.test/y"));
        assert_eq!(link_of("mail"), Some("mailto:a@b.test"));
        assert_eq!(spans.iter().map(|run| run.text.as_str()).collect::<String>(), "docs abs mail");
    }

    #[test]
    fn unsafe_link_schemes_stay_plain_text() {
        let doc = parse_body(
            "<p><a href=\"javascript:alert(1)\">js</a> <a href=\"data:text/html,x\">data</a> \
             <a href=\"#top\">frag</a> <a href=\"file:///etc/passwd\">file</a></p>",
        );
        let Block::Paragraph(spans) = &doc.blocks[0] else { panic!("expected a paragraph") };
        assert!(spans.iter().all(|run| run.link.is_none()));
        assert_eq!(spans.iter().map(|run| run.text.as_str()).collect::<String>(), "js data frag file");
    }

    #[test]
    fn images_fall_back_to_their_alt_text() {
        let doc = parse_body("<p>before <img src=\"a.png\" alt=\"chart\"><img src=\"b.png\"> after</p>");
        assert_eq!(text_of(&doc.blocks), "before chart after");
    }

    #[test]
    fn quotes_and_rules_become_blocks() {
        let doc = parse_body("<blockquote><p>quoted</p></blockquote><hr>");
        assert_eq!(doc.blocks[0], Block::Quote(vec![Block::Paragraph(vec![Inline {
            text: "quoted".to_owned(),
            ..Inline::default()
        }])]));
        assert_eq!(doc.blocks[1], Block::Rule);
    }

    #[test]
    fn deeply_nested_markup_does_not_overflow_the_stack() {
        // The walk is recursive, so an unbounded depth would be a stack
        // overflow on generated or hostile markup.
        let source = format!("text{}", "<div>".repeat(5_000));
        let doc = parse(&source, "https://api.example.com/");
        assert_eq!(doc.blocks.len(), 1);
    }

    #[test]
    fn malformed_markup_is_recovered() {
        let doc = parse_body("<p>unclosed <b>bold<p>second");
        assert_eq!(text_of(&doc.blocks), "unclosed bold\nsecond");
    }
}
