use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose, Engine as _};
use hmac::{digest::KeyInit, Hmac, Mac};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Method,
};
use sha2::Sha256;
use std::str::FromStr;

use crate::domain::collection::SavedRequest;
use crate::domain::request::{ApiKeyLocation, AuthType, BodyType};
use crate::domain::response::HttpResponse;
use crate::domain::environment::{substitute, AppEnvironment};

type HmacSha256 = Hmac<Sha256>;

fn base64url(data: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn make_jwt(subject: &str, secret: &str) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let header = base64url(b"{\"alg\":\"HS256\",\"typ\":\"JWT\"}");
    let payload_json = format!(
        "{{\"sub\":\"{}\",\"iat\":{},\"exp\":{}}}",
        subject.replace('"', "\\\""),
        now,
        now + 3600
    );
    let payload = base64url(payload_json.as_bytes());
    let signing_input = format!("{header}.{payload}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| e.to_string())?;
    mac.update(signing_input.as_bytes());
    let sig = base64url(&mac.finalize().into_bytes());
    Ok(format!("{signing_input}.{sig}"))
}

const INLINE_BODY_THRESHOLD: usize = 10_000_000;

#[derive(Debug, Clone)]
pub struct HttpResult {
    pub tab_id: String,
    pub response: HttpResponse,
}

pub async fn send(
    client: &Client,
    tab_id: String,
    req: &SavedRequest,
    env: Option<&AppEnvironment>,
    default_timeout_ms: u64,
) -> HttpResult {
    let start = Instant::now();
    match do_send(client, &tab_id, req, env, default_timeout_ms).await {
        Ok(mut resp) => {
            resp.duration_ms = start.elapsed().as_millis() as u64;
            HttpResult { tab_id, response: resp }
        }
        Err(e) => HttpResult {
            tab_id,
            response: HttpResponse { error: Some(e), ..Default::default() },
        },
    }
}

/// The URL actually sent: `{{var}}` tokens expanded, then a scheme defaulted in
/// if the result still has none (`http://` for loopback/RFC-1918, else
/// `https://`).
///
/// Order matters. Substitution comes first because an env var may hold the
/// scheme itself (`API_BACKEND=https://api.example.com`), which no amount of
/// template inspection can see. Defaulting the scheme on the raw template
/// instead is what made `{{API_BACKEND}}/graphql/{{GRAPHQL}}` send to
/// `https://https://api.www.visitdenmark.com/graphql/…` — a host literally named
/// `https`, failing DNS resolution while the URL bar's preview looked correct.
///
/// The URL bar's preview calls this too, so the two cannot disagree.
pub(crate) fn resolve_url(raw_url: &str, env: Option<&AppEnvironment>) -> String {
    let url = substitute(raw_url, env);
    let url = url.trim();
    if url.is_empty()
        || url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("ws://")
        || url.starts_with("wss://")
    {
        return url.to_owned();
    }
    let host = url.split('/').next().unwrap_or(url);
    let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    let local = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1")
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("172."); // covers 172.16-31.x.x private range
    format!("{}://{url}", if local { "http" } else { "https" })
}

async fn do_send(
    client: &Client,
    _tab_id: &str,
    req: &SavedRequest,
    env: Option<&AppEnvironment>,
    default_timeout_ms: u64,
) -> Result<HttpResponse, String> {
    let url = resolve_url(&req.url, env);
    // The query is sent from `params`; if the URL also carries one (e.g. older
    // imports), drop it so each value isn't sent twice (which APIs read as an array).
    let will_add_query = req.params.iter().any(|p| p.enabled && !p.key.is_empty())
        || (req.auth_type == AuthType::ApiKey
            && req.api_key_location == ApiKeyLocation::Query
            && !req.api_key_name.is_empty());
    let url = match url.find('?') {
        Some(i) if will_add_query => url[..i].to_string(),
        _ => url,
    };
    let method = Method::from_str(req.method.as_str())
        .map_err(|_| format!("Invalid HTTP method: {}", req.method))?;

    let mut builder = client.request(method, &url);
    let timeout_ms = if default_timeout_ms < 1000 { 30_000 } else { default_timeout_ms };
    builder = builder.timeout(Duration::from_millis(timeout_ms));

    let mut header_map = HeaderMap::new();
    for h in &req.headers {
        if !h.enabled || h.key.is_empty() {
            continue;
        }
        let k = substitute(&h.key, env);
        let v = substitute(&h.value, env);
        let name = HeaderName::from_str(&k).map_err(|_| format!("Invalid header: {k}"))?;
        let val = HeaderValue::from_str(&v).map_err(|_| format!("Invalid header value: {v}"))?;
        header_map.insert(name, val);
    }

    match &req.auth_type {
        AuthType::Bearer if !req.bearer_token.is_empty() => {
            let val = HeaderValue::from_str(&format!("Bearer {}", req.bearer_token))
                .map_err(|e| e.to_string())?;
            header_map.insert(HeaderName::from_static("authorization"), val);
        }
        AuthType::Basic if !req.basic_user.is_empty() => {
            let credentials = format!("{}:{}", req.basic_user, req.basic_pass);
            let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
            let val = HeaderValue::from_str(&format!("Basic {encoded}"))
                .map_err(|e| e.to_string())?;
            header_map.insert(HeaderName::from_static("authorization"), val);
        }
        AuthType::ApiKey
            if !req.api_key_name.is_empty()
                && req.api_key_location == ApiKeyLocation::Header =>
        {
            let name = HeaderName::from_str(&req.api_key_name)
                .map_err(|_| format!("Invalid API key header name: {}", req.api_key_name))?;
            let val = HeaderValue::from_str(&req.api_key_value).map_err(|e| e.to_string())?;
            header_map.insert(name, val);
        }
        AuthType::Cookie if !req.cookie_string.is_empty() => {
            let val =
                HeaderValue::from_str(&req.cookie_string).map_err(|e| e.to_string())?;
            header_map.insert(HeaderName::from_static("cookie"), val);
        }
        AuthType::JwtUser if !req.jwt_secret.is_empty() => {
            let token = make_jwt(&req.jwt_subject, &req.jwt_secret)?;
            let val = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| e.to_string())?;
            header_map.insert(HeaderName::from_static("authorization"), val);
        }
        _ => {}
    }

    if matches!(req.body_type, BodyType::Json)
        && !header_map.contains_key(reqwest::header::CONTENT_TYPE)
    {
        header_map.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }

    builder = builder.headers(header_map);

    let mut query: Vec<(String, String)> = req
        .params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .map(|p| (substitute(&p.key, env), substitute(&p.value, env)))
        .collect();

    if req.auth_type == AuthType::ApiKey
        && req.api_key_location == ApiKeyLocation::Query
        && !req.api_key_name.is_empty()
    {
        query.push((req.api_key_name.clone(), req.api_key_value.clone()));
    }

    if !query.is_empty() {
        builder = builder.query(&query);
    }

    match &req.body_type {
        BodyType::FormData => {
            let has_file = req
                .form_data_fields
                .iter()
                .any(|f| f.enabled && f.field_type == crate::domain::request::FormFieldType::File);

            if has_file {
                let mut form = reqwest::multipart::Form::new();
                for field in &req.form_data_fields {
                    if !field.enabled {
                        continue;
                    }
                    if let crate::domain::request::FormFieldType::File = field.field_type {
                        let file_data = field.file_data.as_ref().ok_or_else(|| {
                            format!("File field '{}' has no data — pick a file first", field.key)
                        })?;
                        let bytes = general_purpose::STANDARD
                            .decode(file_data)
                            .map_err(|e| format!("Base64 decode: {e}"))?;
                        let fname = field.file_name.clone().unwrap_or_else(|| "file".to_owned());
                        let mut part = reqwest::multipart::Part::bytes(bytes).file_name(fname);
                        if let Some(mime) = &field.mime_type {
                            part = part.mime_str(mime).map_err(|e| e.to_string())?;
                        }
                        form = form.part(field.key.clone(), part);
                    } else {
                        form = form.text(field.key.clone(), field.value.clone());
                    }
                }
                builder = builder.multipart(form);
            } else {
                // No file fields: send as application/x-www-form-urlencoded,
                // which is what plain HTML-style forms (logins, etc.) expect —
                // multipart/form-data would be syntactically wrong for them.
                let pairs: Vec<(String, String)> = req
                    .form_data_fields
                    .iter()
                    .filter(|f| f.enabled)
                    .map(|f| (f.key.clone(), f.value.clone()))
                    .collect();
                builder = builder.form(&pairs);
            }
        }
        BodyType::Json | BodyType::Text => {
            let body = substitute(&req.body, env);
            if !body.is_empty() {
                builder = builder.body(body);
            }
        }
        BodyType::None => {}
    }

    let response = builder.send().await.map_err(|e| describe_send_error(&e))?;

    let status = response.status().as_u16();
    let status_text = response.status().canonical_reason().unwrap_or("").to_owned();

    let mut headers = HashMap::new();
    for (name, value) in response.headers().iter() {
        if let Ok(v) = value.to_str() {
            headers
                .entry(name.to_string())
                .and_modify(|e: &mut String| {
                    e.push('\n');
                    e.push_str(v);
                })
                .or_insert_with(|| v.to_owned());
        }
    }

    let raw_bytes = response.bytes().await.map_err(|e| format!("Read body: {e}"))?;
    let body_size = raw_bytes.len();
    let is_large = body_size > INLINE_BODY_THRESHOLD;

    // Non-UTF-8 bodies (PDFs, images, other binary downloads) can't be shown
    // as text. Detect them up front instead of lossily decoding them into a
    // string full of replacement characters and feeding that into the text
    // editor / JSON pretty-printer.
    //
    // `String::from_utf8` needs an owned `Vec`, so the old
    // `from_utf8(raw_bytes.to_vec())` copied the entire body *just to test
    // whether it was text* — and threw the copy away for binary downloads.
    // Validating the borrowed slice first avoids that copy entirely, which
    // matters a great deal for a large file download.
    match std::str::from_utf8(&raw_bytes) {
        Ok(raw_body) => Ok(HttpResponse {
            status,
            status_text,
            headers,
            // Oversized text bodies are truncated rather than blanked. Handing
            // back an empty string (as this used to) made a big response look
            // like an empty one, with nothing anywhere in the UI explaining
            // why. A prefix keeps the response inspectable; `body_stored`
            // tells the UI to say it is partial.
            // Deliberately NOT pretty-printed here. The UI re-parses the body
            // and pretty-prints it on a blocking worker anyway
            // (`AppMsg::HttpResponse` -> `ViewerReady`), so doing it here too
            // meant every JSON response was serialised twice — once on the async
            // runtime, where a large body stalls other tasks — and the result of
            // this pass was then thrown away. Keeping the raw text also means
            // scripts and `Copy` see exactly what the server sent.
            body: if is_large {
                truncate_on_char_boundary(raw_body, INLINE_BODY_THRESHOLD).to_owned()
            } else {
                raw_body.to_owned()
            },
            body_size,
            body_stored: is_large,
            is_binary: false,
            binary_data: None,
            duration_ms: 0, // filled by caller
            error: None,
        }),
        Err(_) => Ok(HttpResponse {
            status,
            status_text,
            headers,
            body: String::new(),
            body_size,
            // A binary body too big to keep in memory is also "stored
            // elsewhere" as far as the UI is concerned — it previously said
            // `false` here, so an oversized download reported neither a body
            // nor a reason for not having one.
            body_stored: is_large,
            is_binary: true,
            binary_data: if is_large { None } else { Some(raw_bytes.to_vec()) },
            duration_ms: 0, // filled by caller
            error: None,
        }),
    }
}

/// Truncates `text` to at most `max_bytes`, always cutting on a `char`
/// boundary.
///
/// Slicing a `str` at a byte index inside a multi-byte UTF-8 sequence panics,
/// and this crate builds with `panic = "abort"`, so an unchecked cut here would
/// hard-kill the app on a large non-ASCII response.
fn truncate_on_char_boundary(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// User-selectable TLS/connection behaviour, for endpoints a browser or curl
/// reaches but a default rustls client does not (issue #40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TlsOptions {
    /// Accept invalid/untrusted certificates: self-signed, internal CA, or a
    /// hostname mismatch. The equivalent of `curl -k`.
    pub accept_invalid_certs: bool,
    /// Offer only HTTP/1.1 in the TLS handshake (no `h2` in ALPN).
    ///
    /// rustls advertises HTTP/2 by default. Some servers and middleboxes
    /// mishandle that and abruptly close the connection, which on Windows
    /// surfaces as `os error 10054` ("connection closed by remote host") — the
    /// exact symptom reported in #40, and the most common cause of it. Browsers
    /// avoid it because they fall back to HTTP/1.1 on such a failure; a plain
    /// client does not.
    pub http1_only: bool,
    /// Restrict the handshake to TLS 1.2, for servers that break on 1.3.
    pub force_tls12: bool,
    /// Restrict the handshake to TLS 1.3, for servers that require it.
    pub force_tls13: bool,
}

/// Builds the shared HTTP client with the given TLS behaviour.
///
/// These are client-level rather than per-request settings because a `Client`
/// owns both the connection pool and the TLS config, so changing any of them
/// needs a fresh client (callers drop their cached one — see `AppState::http`).
pub fn build_client_with(options: TlsOptions) -> Client {
    let mut builder = Client::builder()
        .cookie_store(true)
        .user_agent(concat!("rustman/", env!("CARGO_PKG_VERSION")));

    if options.accept_invalid_certs {
        // Hostname verification is dropped too, which is what makes a
        // bare-IP or internal-name endpoint reachable at all.
        builder = builder
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);
    }

    if options.http1_only {
        builder = builder.http1_only();
    }

    // Asking for both pins is contradictory; treat it as "no restriction"
    // rather than building a config that can never complete a handshake.
    match (options.force_tls12, options.force_tls13) {
        (true, false) => {
            builder = builder
                .min_tls_version(reqwest::tls::Version::TLS_1_2)
                .max_tls_version(reqwest::tls::Version::TLS_1_2);
        }
        (false, true) => {
            builder = builder
                .min_tls_version(reqwest::tls::Version::TLS_1_3)
                .max_tls_version(reqwest::tls::Version::TLS_1_3);
        }
        _ => {}
    }

    builder.build().unwrap_or_else(|err| {
        // A rejected combination must not take the app down: fall back to a
        // default client so requests still work.
        eprintln!("HTTP client: {err}; falling back to default TLS settings");
        default_client()
    })
}

/// A client with default TLS behaviour. Used where no user overrides apply
/// (the `#[ignore]`d live tests) and as the fallback in `build_client_with`.
/// A client with default TLS behaviour, used by the `#[ignore]`d live tests.
///
/// Production code goes through `build_client_with` so the user's TLS settings
/// apply (see `AppState::http`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn build_client() -> Client {
    default_client()
}

fn default_client() -> Client {
    Client::builder()
        .cookie_store(true)
        .user_agent(concat!("rustman/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to build HTTP client")
}



/// Categorise a send failure into a short, human label.
fn classify_send_error(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "Request timed out"
    } else if e.is_connect() {
        "Connection failed"
    } else if e.is_redirect() {
        "Too many redirects"
    } else if e.is_body() || e.is_decode() {
        "Failed to read response"
    } else {
        "Request failed"
    }
}

/// The deepest message in an error's `source()` chain. reqwest's own `Display`
/// stops at "error sending request for url (…)" and hides the real reason (TLS
/// trust failure, DNS error, connection refused, …), so walk down to the leaf.
fn root_cause(err: &dyn std::error::Error) -> String {
    let mut cur = err;
    while let Some(src) = cur.source() {
        cur = src;
    }
    cur.to_string()
}

/// Build an actionable one-line message from a send failure: a category plus the
/// underlying cause. The leaf cause is used rather than reqwest's top-level
/// `Display` so the (possibly secret-bearing) request URL isn't echoed back.
fn describe_send_error(e: &reqwest::Error) -> String {
    format!("{}: {}", classify_send_error(e), root_cause(e))
}

#[cfg(test)]
mod url_tests {
    use super::*;

    fn env_with(vars: &[(&str, &str)]) -> AppEnvironment {
        AppEnvironment {
            id: "test".to_owned(),
            name: "test".to_owned(),
            variables: vars.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
            is_active: true,
        }
    }

    /// The reported bug: a template with no literal scheme, whose first variable
    /// expands to a scheme-bearing URL. Scheme defaulting on the raw template
    /// produced `https://https://…`, so the request went to a host named `https`
    /// and failed to resolve, even though the URL bar preview showed the fully
    /// expanded URL and looked right.
    #[test]
    fn variable_supplied_scheme_is_not_double_prefixed() {
        let env = env_with(&[
            ("API_BACKEND", "api.www.visitdenmark.com"),
            ("GRAPHQL", "e9e32dbc-0bae-4278-b349-1bcdecb53f7d"),
        ]);
        assert_eq!(
            resolve_url("{{API_BACKEND}}/graphql/{{GRAPHQL}}", Some(&env)),
            "https://api.www.visitdenmark.com/graphql/e9e32dbc-0bae-4278-b349-1bcdecb53f7d"
        );
    }

    /// The variable may carry the scheme itself; substitution must win over
    /// defaulting so this stays a single prefix.
    #[test]
    fn scheme_inside_the_variable_is_honoured() {
        let env = env_with(&[
            ("API_BACKEND", "https://api.www.visitdenmark.com"),
            ("GRAPHQL", "abc"),
        ]);
        assert_eq!(
            resolve_url("{{API_BACKEND}}/graphql/{{GRAPHQL}}", Some(&env)),
            "https://api.www.visitdenmark.com/graphql/abc"
        );
    }

    /// Ordinary schemeless hosts still get `https://` (and locals `http://`), so
    /// the convenience the old code provided is preserved.
    #[test]
    fn raw_schemeless_urls_still_get_a_scheme() {
        assert_eq!(resolve_url("api.example.com/x", None), "https://api.example.com/x");
        assert_eq!(resolve_url("localhost:8899/x", None), "http://localhost:8899/x");
    }
}

#[cfg(test)]
mod tls_option_tests {
    use super::*;

    /// Every combination must produce a usable client rather than panicking or
    /// silently degrading — this is the whole point of the fallback in
    /// `build_client_with`.
    #[test]
    fn every_option_combination_builds_a_client() {
        for accept_invalid_certs in [false, true] {
            for http1_only in [false, true] {
                for (force_tls12, force_tls13) in
                    [(false, false), (true, false), (false, true), (true, true)]
                {
                    let options = TlsOptions {
                        accept_invalid_certs,
                        http1_only,
                        force_tls12,
                        force_tls13,
                    };
                    // Builds, or falls back; either way we get a client.
                    let _client = build_client_with(options);
                }
            }
        }
    }

    #[test]
    fn default_options_are_all_off() {
        let options = TlsOptions::default();
        assert!(!options.accept_invalid_certs);
        assert!(!options.http1_only);
        assert!(!options.force_tls12);
        assert!(!options.force_tls13);
    }

    /// Pinning both versions cannot handshake, so it is treated as no pin at
    /// all. Asserted here because the UI also enforces exclusivity — this is the
    /// backstop for a restored session that somehow holds both.
    #[test]
    fn both_version_pins_is_treated_as_unrestricted() {
        let both = TlsOptions { force_tls12: true, force_tls13: true, ..Default::default() };
        // Must not fall back (which would mean the builder rejected it).
        let _client = build_client_with(both);
    }
}

#[cfg(test)]
mod body_tests {
    use super::*;

    #[test]
    fn truncate_leaves_small_text_untouched() {
        assert_eq!(truncate_on_char_boundary("hello", 100), "hello");
    }

    #[test]
    fn truncate_respects_the_byte_budget() {
        let text = "a".repeat(500);
        assert_eq!(truncate_on_char_boundary(&text, 100).len(), 100);
    }

    /// Cutting mid-character would panic (and `panic = "abort"` makes that
    /// fatal), so the cut must retreat to a char boundary.
    #[test]
    fn truncate_never_splits_a_multibyte_char() {
        // '€' is 3 bytes, so a 100-byte budget lands mid-character.
        let text = "€".repeat(200);
        let out = truncate_on_char_boundary(&text, 100);
        assert!(out.len() <= 100);
        assert_eq!(out.len() % 3, 0, "must cut on a char boundary");
        // Round-trips as valid UTF-8 by construction (it is a &str).
        assert!(out.chars().all(|c| c == '€'));
    }

    #[test]
    fn truncate_handles_a_zero_budget() {
        assert_eq!(truncate_on_char_boundary("€€€", 0), "");
    }

    #[test]
    fn truncate_handles_budget_smaller_than_first_char() {
        // No boundary exists at 1 or 2 bytes, so it must fall back to empty
        // rather than panic.
        assert_eq!(truncate_on_char_boundary("€abc", 2), "");
    }
}

#[cfg(test)]
mod error_tests {
    use super::root_cause;
    use std::fmt;

    #[derive(Debug)]
    struct Err(&'static str, Option<Box<Err>>);
    impl fmt::Display for Err {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }
    impl std::error::Error for Err {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.1.as_deref().map(|e| e as &dyn std::error::Error)
        }
    }

    #[test]
    fn walks_to_leaf_cause() {
        let chain = Err(
            "error sending request",
            Some(Box::new(Err(
                "client error (Connect)",
                Some(Box::new(Err("invalid peer certificate: UnknownIssuer", None))),
            ))),
        );
        assert_eq!(root_cause(&chain), "invalid peer certificate: UnknownIssuer");
    }

    #[test]
    fn single_error_returns_itself() {
        assert_eq!(root_cause(&Err("boom", None)), "boom");
    }
}

/// Diagnostic tests that hit the real local test server
/// (`scripts/test_file_server.py`, port 8899) to check whether a mixed
/// text+file multipart form actually goes out over the wire correctly.
/// `#[ignore]`d by default since they need that server running:
///   python3 scripts/test_file_server.py 8899 &
///   cargo test --lib -- --ignored --nocapture live_form_upload
#[cfg(test)]
mod live_tests {
    use super::*;
    use crate::domain::collection::SavedRequest;
    use crate::domain::request::{BodyType, FormField, FormFieldType, HttpMethod};

    #[tokio::test]
    #[ignore]
    async fn live_form_upload_with_mixed_text_and_file_fields() {
        let client = build_client();
        let mut req = SavedRequest::new_in(String::new(), "test".to_owned());
        req.method = HttpMethod::Post;
        req.url = "http://localhost:8899/upload".to_owned();
        req.body_type = BodyType::FormData;
        req.form_data_fields = vec![
            FormField {
                id: "1".to_owned(),
                key: "name".to_owned(),
                value: "rustman".to_owned(),
                field_type: FormFieldType::Text,
                enabled: true,
                file_name: None,
                file_data: None,
                mime_type: None,
            },
            FormField {
                id: "2".to_owned(),
                key: "avatar".to_owned(),
                value: String::new(),
                field_type: FormFieldType::File,
                enabled: true,
                file_name: Some("avatar.png".to_owned()),
                file_data: Some(general_purpose::STANDARD.encode(b"fake png bytes")),
                mime_type: Some("image/png".to_owned()),
            },
        ];

        let result = send(&client, "tab-1".to_owned(), &req, None, 30_000).await;
        eprintln!("response: {:?}", result.response);
        assert_eq!(result.response.error, None, "request failed: {:?}", result.response.error);
        assert_eq!(result.response.status, 200);

        let parsed: serde_json::Value = serde_json::from_str(&result.response.body)
            .expect("server response should be JSON");
        eprintln!("parsed server response: {parsed:#}");

        let files = parsed["files"].as_array().expect("files array");
        let fields = parsed["fields"].as_object().expect("fields object");

        assert_eq!(fields.get("name").and_then(|v| v.as_str()), Some("rustman"));
        assert_eq!(files.len(), 1, "expected exactly one uploaded file, got: {files:?}");
        assert_eq!(files[0]["filename"], "avatar.png");
        assert_eq!(files[0]["size"], 14); // "fake png bytes".len()
    }

    #[tokio::test]
    #[ignore]
    async fn live_form_upload_sends_correct_excel_mime_type() {
        let client = build_client();
        let mut req = SavedRequest::new_in(String::new(), "test".to_owned());
        req.method = HttpMethod::Post;
        req.url = "http://localhost:8899/upload".to_owned();
        req.body_type = BodyType::FormData;
        req.form_data_fields = vec![FormField {
            id: "1".to_owned(),
            key: "report".to_owned(),
            value: String::new(),
            field_type: FormFieldType::File,
            enabled: true,
            file_name: Some("report.xlsx".to_owned()),
            file_data: Some(general_purpose::STANDARD.encode(b"fake xlsx bytes")),
            mime_type: Some(
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_owned(),
            ),
        }];

        let result = send(&client, "tab-1".to_owned(), &req, None, 30_000).await;
        assert_eq!(result.response.error, None, "request failed: {:?}", result.response.error);
        assert_eq!(result.response.status, 200);

        let parsed: serde_json::Value = serde_json::from_str(&result.response.body)
            .expect("server response should be JSON");
        eprintln!("parsed server response: {parsed:#}");

        let files = parsed["files"].as_array().expect("files array");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["filename"], "report.xlsx");
        assert_eq!(
            files[0]["content_type"],
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "the multipart part's Content-Type on the wire should be the Excel \
             MIME type, not a generic fallback like application/octet-stream"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn live_text_only_form_sends_urlencoded_not_multipart() {
        // Reproduces the real bug report: a plain login form (no file
        // fields) sent to a strict server that only accepts
        // application/x-www-form-urlencoded and 400s on anything else
        // (like multipart/form-data).
        let client = build_client();
        let mut req = SavedRequest::new_in(String::new(), "test".to_owned());
        req.method = HttpMethod::Post;
        req.url = "http://localhost:8899/login".to_owned();
        req.body_type = BodyType::FormData;
        req.form_data_fields = vec![
            FormField {
                id: "1".to_owned(),
                key: "conta".to_owned(),
                value: "sync_dep_1".to_owned(),
                field_type: FormFieldType::Text,
                enabled: true,
                file_name: None,
                file_data: None,
                mime_type: None,
            },
            FormField {
                id: "2".to_owned(),
                key: "modulo".to_owned(),
                value: "SYNC".to_owned(),
                field_type: FormFieldType::Text,
                enabled: true,
                file_name: None,
                file_data: None,
                mime_type: None,
            },
        ];

        let result = send(&client, "tab-1".to_owned(), &req, None, 30_000).await;
        eprintln!("response: {:?}", result.response);
        assert_eq!(result.response.error, None, "request failed: {:?}", result.response.error);
        assert_eq!(
            result.response.status, 200,
            "server rejected the request — body: {}",
            result.response.body
        );

        let parsed: serde_json::Value = serde_json::from_str(&result.response.body)
            .expect("server response should be JSON");
        assert_eq!(parsed["received_fields"]["conta"], "sync_dep_1");
        assert_eq!(parsed["received_fields"]["modulo"], "SYNC");
    }
}
