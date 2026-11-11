//! What a curl command means as a request.
//!
//! [`shell`](super::shell) splits the paste into words and
//! [`options`](super::options) groups them into arguments; this module
//! applies each argument the way curl would — `-d` pieces join with `&`, `-G`
//! moves them into the query, `-u` becomes Basic auth, an explicit `-H`
//! overrides the headers curl adds itself — and records what it could not
//! carry over (a body read from `@file`, digest auth, a proxy) as notes.

use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

use base64::{Engine as _, engine::general_purpose};
use rustman_core::domain::headers::Headers;
use rustman_core::text::{head_capped, is_json_document};
use rustman_core::url::percent_encode;

use super::options::{Arg, parse_args};
use super::shell;

/// A request recovered from a curl (or HTTPie) command.
#[derive(Debug, Clone, Default)]
pub struct ParsedCurl {
    pub method: Option<String>,
    pub url: Option<String>,
    pub header: Headers,
    pub body: Option<String>,
    /// In command order. Values are kept verbatim: cookies are not
    /// form-encoded, so decoding `+` or `%2B` would corrupt base64 values.
    pub cookies: Vec<(String, String)>,
    pub form: Vec<CurlForm>,
    /// Parts of the command that could not be imported, phrased for the user.
    pub notes: Vec<String>,
}

/// One `-F` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurlForm {
    pub key: String,
    /// The text value, or the file path for file fields.
    pub value: String,
    pub is_file: bool,
    /// `;type=` from `-F 'f=@a.png;type=image/png'`.
    pub content_type: Option<String>,
}

/// Bytes of a paste inspected to tell whether it is a curl command.
const SNIFF_BYTES: usize = 4096;

/// The request a pasted curl command describes. Never fails: what can't be
/// imported is listed in [`ParsedCurl::notes`].
pub fn parse(input: &str) -> ParsedCurl {
    let commands = shell::commands(input);
    // `echo … | curl …` or `cd x && curl …`: import the curl part.
    let index = commands
        .iter()
        .position(|words| invokes_curl(words))
        .unwrap_or(0);
    let words = commands.into_iter().nth(index).unwrap_or_default();

    let mut words = words.into_iter().peekable();
    words.next_if(|w| is_prompt(w));
    words.next_if(|w| is_curl_program(w));

    let mut builder = Builder::default();
    for arg in parse_args(words) {
        if builder.apply(arg).is_break() {
            break;
        }
    }
    builder.finish()
}

/// Whether some command in the first few KB of `text` runs curl.
pub(crate) fn looks_like_command(text: &str) -> bool {
    shell::commands(head_capped(text, SNIFF_BYTES))
        .iter()
        .any(|words| invokes_curl(words))
}

/// Whether a command runs curl, possibly after a documentation prompt.
fn invokes_curl(words: &[String]) -> bool {
    match words {
        [first, ..] if is_curl_program(first) => true,
        [prompt, second, ..] => is_prompt(prompt) && is_curl_program(second),
        _ => false,
    }
}

/// A shell prompt pasted along with a documented command (`$ curl …`).
fn is_prompt(word: &str) -> bool {
    matches!(word, "$" | "%")
}

/// Whether `word` names curl (`curl`, `curl.exe`, `/usr/bin/curl`).
fn is_curl_program(word: &str) -> bool {
    let name = word.rsplit(['/', '\\']).next().unwrap_or(word);
    !word.contains("://")
        && (name.eq_ignore_ascii_case("curl") || name.eq_ignore_ascii_case("curl.exe"))
}

/// Where a request body came from, which decides its default Content-Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodySource {
    /// `-d` and friends: curl labels it a form post.
    Data,
    /// `--json`: curl labels it JSON.
    Json,
}

#[derive(Default)]
struct Builder {
    method: Option<String>,
    head: bool,
    get: bool,
    upload: bool,
    /// From `--url`: certainly URLs.
    urls: Vec<String>,
    /// Bare words: URLs to curl, but possibly the value of an option this
    /// parser does not know takes one.
    positionals: Vec<String>,
    url_queries: Vec<String>,
    headers: Headers,
    /// Lowercased names from `-H 'Name:'`, which stop curl adding a header.
    removed: HashSet<String>,
    user_agent: Option<String>,
    referer: Option<String>,
    basic: Option<String>,
    /// `--digest` and the like, which import as Basic auth if at all.
    unsupported_auth: Option<String>,
    bearer: Option<String>,
    body: Option<String>,
    body_source: Option<BodySource>,
    cookies: Cookies,
    form: Vec<CurlForm>,
    notes: Vec<String>,
    noted: HashSet<String>,
}

impl Builder {
    fn apply(&mut self, arg: Arg) -> ControlFlow<()> {
        let (name, value) = match arg {
            Arg::Positional(word) => {
                self.positionals.push(word);
                return ControlFlow::Continue(());
            }
            Arg::Opt { name, value } => (name, value.unwrap_or_default()),
        };
        match name.as_str() {
            "request" => self.method = Some(value.to_ascii_uppercase()),
            "url" => self.urls.push(value),
            "url-query" => self.url_queries.push(value),
            "header" => self.header(&value),
            "user-agent" => self.user_agent = Some(value),
            "referer" => {
                let referer = value.strip_suffix(";auto").unwrap_or(&value);
                self.referer = (!referer.is_empty()).then(|| referer.to_owned());
            }
            "cookie" => self.cookie(&value),
            "user" => self.basic = Some(value),
            "oauth2-bearer" => self.bearer = Some(value),
            "data" | "data-ascii" | "data-binary" => match value.strip_prefix('@') {
                Some(file) => self.note_file_body(file),
                None => self.append_data(&value, BodySource::Data),
            },
            "data-raw" => self.append_data(&value, BodySource::Data),
            "data-urlencode" => match parse_urlencode_spec(&value) {
                Ok(piece) => self.append_data(&piece, BodySource::Data),
                Err(file) => self.note_file_body(file),
            },
            "json" => {
                match value.strip_prefix('@') {
                    Some(file) => self.note_file_body(file),
                    // Repeated `--json` pieces concatenate, unlike `-d`'s `&`.
                    None => self.body.get_or_insert_default().push_str(&value),
                }
                self.body_source = Some(BodySource::Json);
            }
            "form" => self.form(&value, false),
            "form-string" => self.form(&value, true),
            "get" => self.get = true,
            "head" => self.head = true,
            "upload-file" => {
                self.upload = true;
                self.note(format!("The upload file {value} wasn't loaded — add it in Body."));
            }
            "insecure" => self.note(
                "-k: certificate checks stay on. Turn on \"Accept invalid certificates\" in Settings if the server needs it."
                    .to_owned(),
            ),
            "proxy" | "preproxy" | "socks4" | "socks4a" | "socks5" | "socks5-hostname" => {
                self.note(format!("The proxy {value} isn't applied."));
            }
            "cert" | "key" => self.note("Client certificates (--cert/--key) aren't supported.".to_owned()),
            "digest" | "ntlm" | "ntlm-wb" | "negotiate" | "anyauth" => {
                self.unsupported_auth.get_or_insert(name);
            }
            "aws-sigv4" => self.note("AWS SigV4 signing isn't supported.".to_owned()),
            "config" => self.note(format!("Options in config file {value} weren't read.")),
            "unix-socket" | "abstract-unix-socket" => {
                self.note(format!("Unix socket {value} isn't supported; the request goes over TCP."));
            }
            "next" => {
                self.note("The command holds several requests (--next); only the first was imported.".to_owned());
                return ControlFlow::Break(());
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn note(&mut self, note: String) {
        if self.noted.insert(note.clone()) {
            self.notes.push(note);
        }
    }

    fn note_file_body(&mut self, file: &str) {
        let source = if file == "-" { "stdin" } else { file };
        self.note(format!(
            "The body from {source} wasn't loaded — paste it into Body."
        ));
    }

    fn append_data(&mut self, data: &str, source: BodySource) {
        match &mut self.body {
            Some(body) if !body.is_empty() => {
                body.push('&');
                body.push_str(data);
            }
            body => *body = Some(data.to_owned()),
        }
        self.body_source.get_or_insert(source);
    }

    fn header(&mut self, raw: &str) {
        if let Some(file) = raw.strip_prefix('@') {
            self.note(format!("Headers from file {file} weren't read."));
            return;
        }
        let (name, value) = match raw.split_once(':') {
            // `Name:` with nothing after it tells curl to drop a header it
            // would add itself.
            Some((name, value)) if value.trim().is_empty() => {
                self.removed.insert(name.trim().to_ascii_lowercase());
                return;
            }
            Some((name, value)) => (name.trim(), value.trim()),
            // `Name;` sends the header with an empty value.
            None => match raw.trim().strip_suffix(';') {
                Some(name) => (name.trim(), ""),
                None => return,
            },
        };
        if name.is_empty() {
            return;
        }
        if name.eq_ignore_ascii_case("cookie") {
            self.cookies.parse(value);
        } else {
            self.headers.push(name, value);
        }
    }

    fn cookie(&mut self, value: &str) {
        // Without an `=`, curl reads the argument as a cookie file name.
        if value.contains('=') {
            self.cookies.parse(value);
        } else if !value.is_empty() {
            self.note(format!("Cookies from file {value} weren't read."));
        }
    }

    fn form(&mut self, raw: &str, literal: bool) {
        let Some((key, value)) = raw.split_once('=') else {
            return;
        };
        let key = key.to_owned();
        let field = if literal {
            CurlForm {
                key,
                value: value.to_owned(),
                is_file: false,
                content_type: None,
            }
        } else if let Some(spec) = value.strip_prefix('@') {
            let (path, params) = split_form_value(spec);
            CurlForm {
                key,
                value: path,
                is_file: true,
                content_type: params.content_type,
            }
        } else if let Some(file) = value.strip_prefix('<') {
            let (path, _) = split_form_value(file);
            self.note(format!(
                "Field {key}: the contents of {path} weren't loaded."
            ));
            CurlForm {
                key,
                value: String::new(),
                is_file: false,
                content_type: None,
            }
        } else {
            let (text, params) = split_form_value(value);
            CurlForm {
                key,
                value: text,
                is_file: false,
                content_type: params.content_type,
            }
        };
        self.form.push(field);
    }

    fn url(&mut self) -> Option<String> {
        self.positionals.retain(|word| !word.is_empty());
        if !self.urls.is_empty() {
            if self.urls.len() > 1 {
                self.note(format!(
                    "The command lists {} URLs; only the first was imported.",
                    self.urls.len()
                ));
            }
            return self.urls.first().cloned();
        }
        // Prefer what is unmistakably a URL, then anything shaped like a
        // host or path, then whatever is there.
        let strong = |w: &&String| w.contains("://") || w.starts_with("{{");
        let weak = |w: &&String| w.contains(['.', '/', ':']) || w.starts_with("localhost");
        let count = self.positionals.iter().filter(strong).count();
        if count > 1 {
            self.note(format!(
                "The command lists {count} URLs; only the first was imported."
            ));
        }
        self.positionals
            .iter()
            .find(strong)
            .or_else(|| self.positionals.iter().find(weak))
            .or_else(|| self.positionals.first())
            .cloned()
    }

    /// Whether an explicit `-H` sets header `name` or removes it, so curl
    /// doesn't add its own.
    fn sets_or_removes(&self, name: &str) -> bool {
        self.headers.contains(name) || self.removed.contains(&name.to_ascii_lowercase())
    }

    fn finish(mut self) -> ParsedCurl {
        let mut url = self.url();
        if let Some(scheme) = self.unsupported_auth.take() {
            let fallback = if self.basic.is_some() {
                "; the credentials were imported as Basic auth"
            } else {
                ""
            };
            self.note(format!("--{scheme} auth isn't supported{fallback}."));
        }

        // Headers curl would add itself, unless an explicit `-H` set them.
        let mut defaults: Vec<(&str, String)> = Vec::new();
        if let Some(agent) = self.user_agent.take() {
            defaults.push(("User-Agent", agent));
        }
        if let Some(referer) = self.referer.take() {
            defaults.push(("Referer", referer));
        }
        if let Some(token) = self.bearer.take() {
            defaults.push(("Authorization", format!("Bearer {token}")));
        } else if let Some(credentials) = self.basic.take() {
            // `-u user` makes curl prompt for the password; import it empty.
            let credentials = if credentials.contains(':') {
                credentials
            } else {
                format!("{credentials}:")
            };
            let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
            defaults.push(("Authorization", format!("Basic {encoded}")));
        }
        if self.body_source == Some(BodySource::Json) {
            defaults.push(("Content-Type", "application/json".to_owned()));
            defaults.push(("Accept", "application/json".to_owned()));
        }
        for (name, value) in defaults {
            if !self.sets_or_removes(name) {
                self.headers.push(name, value);
            }
        }

        let has_payload = self.body.is_some() || !self.form.is_empty();
        let method = self.method.take().unwrap_or_else(|| {
            let method = if self.head {
                "HEAD"
            } else if self.get {
                "GET"
            } else if self.upload {
                "PUT"
            } else if has_payload {
                "POST"
            } else {
                "GET"
            };
            method.to_owned()
        });

        if let Some(u) = url.as_mut() {
            let mut pieces = Vec::new();
            // `-G` sends the `-d` data as the query string instead.
            if self.get
                && let Some(data) = self.body.take()
            {
                pieces.push(data);
            }
            for spec in std::mem::take(&mut self.url_queries) {
                match spec.strip_prefix('+') {
                    Some(raw) => pieces.push(raw.to_owned()),
                    None => match parse_urlencode_spec(&spec) {
                        Ok(piece) => pieces.push(piece),
                        Err(file) => {
                            self.note(format!("The query from file {file} wasn't loaded."));
                        }
                    },
                }
            }
            append_query(u, &pieces);
        }

        // `-d` bodies go out as form posts unless a Content-Type says
        // otherwise. JSON-looking bodies are left to the app, which labels
        // them JSON: what the author meant, if not what curl itself does.
        if self.body_source == Some(BodySource::Data)
            && self.body.as_deref().is_some_and(|b| !is_json_document(b))
            && !self.sets_or_removes("content-type")
        {
            self.headers
                .push("Content-Type", "application/x-www-form-urlencoded");
        }

        ParsedCurl {
            method: Some(method),
            url,
            header: self.headers,
            body: self.body,
            cookies: self.cookies.pairs,
            form: self.form,
            notes: self.notes,
        }
    }
}

/// Parse a `--data-urlencode` spec (`content`, `=content`, `name=content`)
/// into the encoded piece to send, or return the file a `@file` /
/// `name@file` spec would read. Like curl, an `=` anywhere makes it content,
/// so `user@host=x` is the name `user@host`.
fn parse_urlencode_spec(spec: &str) -> Result<String, &str> {
    match spec.split_once('=') {
        Some(("", content)) => Ok(percent_encode(content)),
        Some((name, content)) => Ok(format!("{name}={}", percent_encode(content))),
        None => match spec.split_once('@') {
            Some((_, file)) => Err(file),
            None => Ok(percent_encode(spec)),
        },
    }
}

/// Append query pieces, before any `#fragment`.
fn append_query(url: &mut String, pieces: &[String]) {
    let mut pieces = pieces.iter().filter(|piece| !piece.is_empty()).peekable();
    if pieces.peek().is_none() {
        return;
    }
    let fragment = url.find('#').map(|i| url.split_off(i));
    let mut has_query = url.contains('?');
    for piece in pieces {
        if !has_query {
            url.push('?');
            has_query = true;
        } else if !url.ends_with(['?', '&']) {
            url.push('&');
        }
        url.push_str(piece);
    }
    if let Some(fragment) = fragment {
        url.push_str(&fragment);
    }
}

fn is_cookie_attribute(name: &str) -> bool {
    [
        "path",
        "domain",
        "expires",
        "max-age",
        "secure",
        "httponly",
        "samesite",
        "partitioned",
        "priority",
    ]
    .iter()
    .any(|attr| attr.eq_ignore_ascii_case(name))
}

/// Cookies in the order their names first appear, each with its last value.
#[derive(Default)]
struct Cookies {
    pairs: Vec<(String, String)>,
    index: HashMap<String, usize>,
}

impl Cookies {
    /// Adds `a=1; b=2`, replacing same-named entries. Attributes (`Path=/`,
    /// `HttpOnly`) from a pasted Set-Cookie line are skipped.
    fn parse(&mut self, s: &str) {
        for part in s.split(';') {
            let (name, value) = match part.split_once('=') {
                Some((name, value)) => (name.trim(), value.trim()),
                None => (part.trim(), ""),
            };
            if name.is_empty() || is_cookie_attribute(name) {
                continue;
            }
            match self.index.get(name) {
                Some(&i) => value.clone_into(&mut self.pairs[i].1),
                None => {
                    self.index.insert(name.to_owned(), self.pairs.len());
                    self.pairs.push((name.to_owned(), value.to_owned()));
                }
            }
        }
    }
}

#[derive(Default)]
struct FormParams {
    content_type: Option<String>,
}

/// Split a `-F` value into its content and trailing `;type=`-style
/// parameters. The content may be double-quoted to protect `;`s. Only a tail
/// made entirely of known parameters is split off, so a `;` inside ordinary
/// text survives.
fn split_form_value(spec: &str) -> (String, FormParams) {
    let (content, tail) = match spec.strip_prefix('"') {
        Some(quoted) => {
            let mut content = String::new();
            let mut end = quoted.len();
            let mut chars = quoted.char_indices();
            while let Some((i, c)) = chars.next() {
                match c {
                    '\\' => content.extend(chars.next().map(|(_, escaped)| escaped)),
                    '"' => {
                        end = i + 1;
                        break;
                    }
                    c => content.push(c),
                }
            }
            (content, &quoted[end..])
        }
        None => {
            let split = known_params_start(spec);
            (spec[..split].to_owned(), &spec[split..])
        }
    };

    let mut params = FormParams::default();
    for param in tail.split(';') {
        if let Some((key, value)) = param.split_once('=')
            && key.trim().eq_ignore_ascii_case("type")
        {
            params.content_type = Some(value.trim().to_owned());
        }
    }
    (content, params)
}

/// Byte index where a tail of only known `;key=value` form parameters
/// starts, or the length of `spec` when there is none.
fn known_params_start(spec: &str) -> usize {
    const KNOWN: [&str; 4] = ["type", "filename", "headers", "encoder"];
    let mut start = spec.len();
    for (i, _) in spec.rmatch_indices(';') {
        let is_param = spec[i + 1..start]
            .split_once('=')
            .is_some_and(|(key, _)| KNOWN.iter().any(|k| k.eq_ignore_ascii_case(key.trim())));
        if !is_param {
            break;
        }
        start = i;
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(p: &ParsedCurl) -> Vec<(&str, &str)> {
        p.header.iter().collect()
    }

    #[test]
    fn a_newline_ends_the_previous_command() {
        let p = parse("export URL=https://a.io\ncurl https://b.io");
        assert_eq!(p.url.as_deref(), Some("https://b.io"));
    }

    #[test]
    fn lines_starting_with_an_option_continue_the_command() {
        let p = parse("curl https://x.io/a\n  -H 'X-Tag: 1'\n  --data 'k=v'");
        assert_eq!(p.url.as_deref(), Some("https://x.io/a"));
        assert_eq!(p.header.get("x-tag"), Some("1"));
        assert_eq!(p.body.as_deref(), Some("k=v"));
    }

    #[test]
    fn a_trailing_backslash_adds_no_empty_url() {
        assert_eq!(
            parse("curl https://x.io \\").url.as_deref(),
            Some("https://x.io")
        );
        assert_eq!(parse("curl \\").url, None);
    }

    #[test]
    fn a_markdown_fence_is_not_a_powershell_paste() {
        let p = parse("```\ncurl 'https://x.io/a' -H 'A: b'\n```");
        assert_eq!(p.url.as_deref(), Some("https://x.io/a"));
        assert_eq!(p.header.get("a"), Some("b"));
    }

    #[test]
    fn unsupported_auth_mentions_basic_only_with_credentials() {
        assert_eq!(
            parse("curl --digest https://x.io").notes,
            ["--digest auth isn't supported."]
        );
        assert_eq!(
            parse("curl --digest -u me:pw https://x.io").notes,
            ["--digest auth isn't supported; the credentials were imported as Basic auth."]
        );
    }

    #[test]
    fn curl_commands_are_recognised_in_their_usual_disguises() {
        for paste in [
            "curl https://x.io",
            "CURL https://x.io",
            "curl.exe https://x.io",
            "/usr/bin/curl https://x.io",
            "$ curl https://x.io",
            "# fetch it\ncurl https://x.io",
            "echo '{}' | curl -d @- https://x.io",
        ] {
            assert!(looks_like_command(paste), "{paste}");
        }
        for paste in ["https://x.io/curl", "man curl", "curling https://x.io", ""] {
            assert!(!looks_like_command(paste), "{paste}");
        }
    }

    #[test]
    fn firefox_copy() {
        // Firefox's "Copy as cURL" of an image request.
        let p = parse(
            r#"curl 'https://i.ytimg.com/vi/cIFz8UUG8xA/hqdefault.jpg?sqp=-oaymwEcCNACELwBSFXyq4qpAw4IARUAAIhCGAFwAcABBg==&rs=AOn4CLAHiaaNh0SfmHW3eJLAzA48mPWhPw' \
  -H 'User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:158.0) Gecko/20100101 Firefox/158.0' \
  -H 'Accept: image/avif,image/jxl,image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5' \
  -H 'Accept-Language: en-US,en;q=0.9' \
  -H 'Accept-Encoding: gzip, deflate, br, zstd' \
  -H 'Referer: https://www.youtube.com/' \
  -H 'Sec-Fetch-Storage-Access: none' \
  -H 'Sec-Fetch-Dest: image' \
  -H 'Sec-Fetch-Mode: no-cors' \
  -H 'Sec-Fetch-Site: cross-site' \
  -H 'Connection: keep-alive' \
  -H 'Alt-Used: i.ytimg.com'"#,
        );
        assert_eq!(p.method.as_deref(), Some("GET"));
        assert_eq!(
            p.url.as_deref(),
            Some(
                "https://i.ytimg.com/vi/cIFz8UUG8xA/hqdefault.jpg?sqp=-oaymwEcCNACELwBSFXyq4qpAw4IARUAAIhCGAFwAcABBg==&rs=AOn4CLAHiaaNh0SfmHW3eJLAzA48mPWhPw"
            )
        );
        assert_eq!(p.header.len(), 11);
        assert_eq!(
            p.header.iter().next(),
            Some((
                "User-Agent",
                "Mozilla/5.0 (X11; Linux x86_64; rv:158.0) Gecko/20100101 Firefox/158.0"
            ))
        );
        assert_eq!(p.header.get("alt-used"), Some("i.ytimg.com"));
        assert_eq!(p.body, None);
        assert!(p.notes.is_empty());
    }

    #[test]
    fn chromium_bash_copy() {
        let p = parse(
            r#"curl 'https://api.example.com/v1/graphql' \
  -H 'accept: */*' \
  -H 'authorization: Bearer eyJ.abc.def' \
  -H 'content-type: application/json' \
  -b 'sid=abc+1/2==; theme=dark' \
  --data-raw $'{"q":"hi!","x":"café"}' \
  --compressed"#,
        );
        assert_eq!(p.method.as_deref(), Some("POST"));
        assert_eq!(p.url.as_deref(), Some("https://api.example.com/v1/graphql"));
        assert_eq!(
            headers(&p),
            [
                ("accept", "*/*"),
                ("authorization", "Bearer eyJ.abc.def"),
                ("content-type", "application/json")
            ]
        );
        assert_eq!(p.body.as_deref(), Some(r#"{"q":"hi!","x":"café"}"#));
        assert_eq!(
            p.cookies,
            [
                ("sid".into(), "abc+1/2==".into()),
                ("theme".into(), "dark".into())
            ]
        );
        assert!(p.notes.is_empty());
    }

    #[test]
    fn chromium_cmd_copy() {
        let p = parse(
            "curl ^\"https://x.io/u?id=1^\" ^\r\n  -H ^\"accept: application/json^\" ^\r\n  --data-raw ^\"^{^\\^\"a^\\^\":1^}^\"",
        );
        assert_eq!(p.url.as_deref(), Some("https://x.io/u?id=1"));
        assert_eq!(headers(&p), [("accept", "application/json")]);
        assert_eq!(p.body.as_deref(), Some(r#"{"a":1}"#));
        assert_eq!(p.method.as_deref(), Some("POST"));
    }

    #[test]
    fn powershell_continuations() {
        let p = parse(
            "curl.exe -X POST `\n  -H \"Content-Type: application/json\" `\n  https://x.io `\n  -d '{\"a\":1}'",
        );
        assert_eq!(p.url.as_deref(), Some("https://x.io"));
        assert_eq!(p.method.as_deref(), Some("POST"));
        assert_eq!(p.body.as_deref(), Some(r#"{"a":1}"#));
    }

    #[test]
    fn values_attached_to_short_options() {
        let p = parse("curl -H'X-Api-Key: k1' -uadmin:secret -XPUT -d'{\"a\":1}' https://x.io/a");
        assert_eq!(p.url.as_deref(), Some("https://x.io/a"));
        assert_eq!(p.method.as_deref(), Some("PUT"));
        assert_eq!(p.header.get("x-api-key"), Some("k1"));
        assert_eq!(
            p.header.get("authorization"),
            Some("Basic YWRtaW46c2VjcmV0")
        );
        assert_eq!(p.body.as_deref(), Some(r#"{"a":1}"#));
    }

    #[test]
    fn option_values_are_not_urls() {
        for cmd in [
            "curl -sSo out.txt https://x.io/a",
            "curl -sSLm 10 https://x.io/a",
            "curl --max-redirs 5 https://x.io/a",
            "curl --some-future-option 5 https://x.io/a",
            "curl -w '%{http_code}' -o /dev/null -s https://x.io/a",
        ] {
            assert_eq!(parse(cmd).url.as_deref(), Some("https://x.io/a"), "{cmd}");
        }
    }

    #[test]
    fn get_moves_data_into_the_query() {
        let p =
            parse("curl -G https://x.io/search?a=1#top -d q=rust --data-urlencode 'name=a b&c'");
        assert_eq!(p.method.as_deref(), Some("GET"));
        assert_eq!(
            p.url.as_deref(),
            Some("https://x.io/search?a=1&q=rust&name=a%20b%26c#top")
        );
        assert_eq!(p.body, None);
        assert!(!p.header.contains("content-type"));
    }

    #[test]
    fn data_urlencode_forms() {
        let p = parse(
            "curl https://x.io --data-urlencode 'hello world' --data-urlencode '=a&b' --data-urlencode 'k=v=w'",
        );
        assert_eq!(p.body.as_deref(), Some("hello%20world&a%26b&k=v%3Dw"));
        let p = parse("curl https://x.io --data-urlencode 'name@file.txt'");
        assert_eq!(p.body, None);
        assert_eq!(p.notes.len(), 1);
    }

    #[test]
    fn url_query() {
        let p = parse("curl https://x.io/a --url-query 'q=a b' --url-query '+raw=%20'");
        assert_eq!(p.url.as_deref(), Some("https://x.io/a?q=a%20b&raw=%20"));
    }

    #[test]
    fn form_posts_get_curls_content_type() {
        let p = parse(
            "curl -X POST https://auth.io/token -d grant_type=client_credentials -d client_id=abc",
        );
        assert_eq!(
            p.body.as_deref(),
            Some("grant_type=client_credentials&client_id=abc")
        );
        assert_eq!(
            p.header.get("content-type"),
            Some("application/x-www-form-urlencoded")
        );
        // An explicit Content-Type wins, and JSON is left for the app to label.
        let p = parse("curl https://x.io -H 'Content-Type: text/plain' -d hi");
        assert_eq!(headers(&p), [("Content-Type", "text/plain")]);
        let p = parse("curl https://x.io -d '{\"a\":1}'");
        assert!(p.header.is_empty());
    }

    #[test]
    fn json_option_defaults_yield_to_explicit_headers() {
        let p = parse(
            "curl -H 'content-type: application/vnd.api+json' --json '{\"a\":' --json '1}' https://x.io",
        );
        assert_eq!(p.body.as_deref(), Some(r#"{"a":1}"#));
        assert_eq!(
            headers(&p),
            [
                ("content-type", "application/vnd.api+json"),
                ("Accept", "application/json")
            ]
        );
    }

    #[test]
    fn headers_keep_order_and_duplicates() {
        let p = parse(
            "curl https://x.io -H 'X-Tag: a' -H 'Accept:application/json' -H 'X-Tag: b' -H 'X-Empty;' -H 'Host:'",
        );
        assert_eq!(
            headers(&p),
            [
                ("X-Tag", "a"),
                ("Accept", "application/json"),
                ("X-Tag", "b"),
                ("X-Empty", "")
            ]
        );
    }

    #[test]
    fn user_agent_referer_and_explicit_overrides() {
        let p = parse("curl -A 'Agent/1' -e 'https://ref.io;auto' https://x.io");
        assert_eq!(
            headers(&p),
            [("User-Agent", "Agent/1"), ("Referer", "https://ref.io")]
        );
        let p = parse(
            "curl -A 'Agent/1' -H 'user-agent: Mine' -u a:b -H 'Authorization: Bearer t' https://x.io",
        );
        assert_eq!(
            headers(&p),
            [("user-agent", "Mine"), ("Authorization", "Bearer t")]
        );
    }

    #[test]
    fn user_without_password() {
        let p = parse("curl -u bob https://x.io");
        assert_eq!(p.header.get("authorization"), Some("Basic Ym9iOg=="));
    }

    #[test]
    fn method_rules() {
        let method = |cmd: &str| parse(cmd).method.unwrap();
        assert_eq!(method("curl https://x.io"), "GET");
        assert_eq!(method("curl -d a=1 https://x.io"), "POST");
        assert_eq!(method("curl -F a=1 https://x.io"), "POST");
        assert_eq!(method("curl -I https://x.io"), "HEAD");
        assert_eq!(method("curl -T f.bin https://x.io"), "PUT");
        assert_eq!(method("curl -X post -d a https://x.io"), "POST");
        assert_eq!(method("curl --request=PATCH --url=https://x.io"), "PATCH");
    }

    #[test]
    fn file_references_become_notes() {
        let p = parse(
            "curl -X POST https://x.io -H 'Content-Type: application/json' -d @payload.json -b jar.txt",
        );
        assert_eq!(p.body, None);
        assert!(p.cookies.is_empty());
        assert_eq!(p.notes.len(), 2, "{:?}", p.notes);
    }

    #[test]
    fn forms() {
        let p = parse(
            r#"curl https://x.io -F 'file=@"/tmp/my file.png";type=image/png' -F 'doc=@/tmp/a.pdf;filename=x.pdf' -F 'note=a;b' -F 'meta={"k":1};type=application/json' --form-string 'raw=@not-a-file'"#,
        );
        let form: Vec<_> = p
            .form
            .iter()
            .map(|f| {
                (
                    f.key.as_str(),
                    f.value.as_str(),
                    f.is_file,
                    f.content_type.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            form,
            [
                ("file", "/tmp/my file.png", true, Some("image/png")),
                ("doc", "/tmp/a.pdf", true, None),
                ("note", "a;b", false, None),
                ("meta", r#"{"k":1}"#, false, Some("application/json")),
                ("raw", "@not-a-file", false, None),
            ]
        );
    }

    #[test]
    fn cookies_are_verbatim_and_skip_attributes() {
        let p = parse("curl https://x.io -b 'a=1%2B2; Path=/; HttpOnly' -H 'Cookie: b=x+y; a=3'");
        assert_eq!(
            p.cookies,
            [("a".into(), "3".into()), ("b".into(), "x+y".into())]
        );
    }

    #[test]
    fn pipelines_prompts_and_comments() {
        let p = parse("$ curl -s https://x.io/a | jq .");
        assert_eq!(p.url.as_deref(), Some("https://x.io/a"));
        let p = parse("echo '{}' | curl -d @- https://x.io/b");
        assert_eq!(p.url.as_deref(), Some("https://x.io/b"));
        let p = parse("# fetch it\ncurl https://x.io/c > out.json");
        assert_eq!(p.url.as_deref(), Some("https://x.io/c"));
    }

    #[test]
    fn next_stops_at_the_first_request() {
        let p = parse("curl https://x.io/a --next -X DELETE https://x.io/b");
        assert_eq!(p.url.as_deref(), Some("https://x.io/a"));
        assert_eq!(p.method.as_deref(), Some("GET"));
        assert_eq!(p.notes.len(), 1);
    }

    #[test]
    fn env_placeholders_survive() {
        let p = parse("curl \"{{base}}/users\" -H \"Authorization: Bearer $TOKEN\"");
        assert_eq!(p.url.as_deref(), Some("{{base}}/users"));
        assert_eq!(p.header.get("authorization"), Some("Bearer $TOKEN"));
    }

    #[test]
    fn notes_for_unsupported_options() {
        let p = parse("curl -k --digest -u a:b -x http://proxy:8080 https://x.io");
        assert_eq!(p.notes.len(), 3, "{:?}", p.notes);
        assert_eq!(p.url.as_deref(), Some("https://x.io"));
    }
}
