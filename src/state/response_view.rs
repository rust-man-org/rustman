//! What the response header and the failure card show, derived once when a
//! response arrives so the views format nothing per frame.

use std::time::{Duration, Instant};

use crate::domain::request::HttpMethod;
use crate::domain::response::{HttpResponse, Redirect};

/// A request on the wire: when it left and where it went.
#[derive(Debug, Clone)]
pub struct Sent {
    pub started: Instant,
    /// `GET http://127.0.0.1:18931/users`
    pub request_line: String,
    /// "127.0.0.1:18931"
    pub host: String,
    /// "Waiting for 127.0.0.1:18931"
    pub waiting: String,
}

impl Sent {
    pub fn new(method: &HttpMethod, resolved_url: &str, started: Instant) -> Self {
        let host = authority(resolved_url).to_owned();
        Sent {
            started,
            request_line: format!("{} {resolved_url}", method.as_str()),
            waiting: format!("Waiting for {host}"),
            host,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseView {
    /// "200 OK", or the short failure reason ("Connection refused").
    pub status: String,
    /// "2 ms", or the time a failure took.
    pub elapsed: String,
    /// "1.4 KB"; a failure has none.
    pub size: Option<String>,
    /// "1.4 KB · 380 B on the wire", when the body arrived compressed.
    pub size_tip: Option<String>,
    /// The redirects followed on the way, when there were any.
    pub redirects: Option<Redirects>,
    pub failure: Option<Failure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirects {
    /// "↪ 2 redirects".
    pub label: String,
    /// One "302 → https://…" line per hop, in order.
    pub hops: String,
}

impl Redirects {
    fn of(hops: &[Redirect]) -> Option<Self> {
        let s = if hops.len() == 1 { "" } else { "s" };
        (!hops.is_empty()).then(|| Redirects {
            label: format!("↪ {} redirect{s}", hops.len()),
            hops: hops.iter().map(|h| format!("{} → {}", h.status, h.to)).collect::<Vec<_>>().join("\n"),
        })
    }
}

impl ResponseView {
    /// `elapsed` is how long the request was out, used when the transport
    /// failed before it could time the exchange.
    pub fn of(resp: &HttpResponse, sent: Option<&Sent>, elapsed: Duration, timeout_ms: u64) -> Self {
        match resp.error.as_deref() {
            Some(raw) => {
                let failure = Failure::new(raw, sent, elapsed, timeout_ms);
                ResponseView {
                    status: failure.kind.reason().to_owned(),
                    elapsed: failure.elapsed.clone(),
                    size: None,
                    size_tip: None,
                    redirects: None,
                    failure: Some(failure),
                }
            }
            None => ResponseView {
                status: status_line(resp),
                elapsed: duration_label(resp.duration_ms),
                size: Some(rustman_core::text::human_size(resp.body_size)),
                size_tip: wire_size_tip(resp),
                redirects: Redirects::of(&resp.redirects),
                failure: None,
            },
        }
    }
}

fn wire_size_tip(resp: &HttpResponse) -> Option<String> {
    let wire = usize::try_from(resp.wire_size?).ok()?;
    (wire != resp.body_size).then(|| {
        format!(
            "{} · {} on the wire",
            rustman_core::text::human_size(resp.body_size),
            rustman_core::text::human_size(wire)
        )
    })
}

fn status_line(resp: &HttpResponse) -> String {
    if resp.status_text.is_empty() {
        resp.status.to_string()
    } else {
        format!("{} {}", resp.status, resp.status_text)
    }
}

/// "2 ms" under a second, "1.24 s" from there.
pub fn duration_label(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{:.2} s", Duration::from_millis(ms).as_secs_f64())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    Refused,
    Dns,
    Tls,
    Timeout,
    /// Reached the network layer but failed some other way (reset, closed).
    Connect,
    /// A pre-request script threw before anything was sent.
    Script,
    Other,
}

impl FailureKind {
    /// The status-bar label for this failure.
    pub fn reason(self) -> &'static str {
        match self {
            FailureKind::Refused => "Connection refused",
            FailureKind::Dns => "DNS lookup failed",
            FailureKind::Tls => "TLS handshake failed",
            FailureKind::Timeout => "Timed out",
            FailureKind::Connect => "Connection failed",
            FailureKind::Script => "Script error",
            FailureKind::Other => "Request failed",
        }
    }
}

/// The failure card: a human title and hint first, the raw text behind
/// Details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailureKind,
    pub title: String,
    pub hint: String,
    /// The underlying cause, without the category prefix.
    pub detail: String,
    pub request: Option<String>,
    pub elapsed: String,
}

impl Failure {
    pub fn new(raw: &str, sent: Option<&Sent>, elapsed: Duration, timeout_ms: u64) -> Self {
        let kind = classify(raw);
        let host = sent.map_or("the server", |s| s.host.as_str());
        let (title, hint) = describe(kind, host, timeout_ms);
        Failure {
            kind,
            title,
            hint,
            detail: cause(raw).to_owned(),
            request: sent.map(|s| s.request_line.clone()),
            elapsed: duration_label(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)),
        }
    }
}

fn describe(kind: FailureKind, host: &str, timeout_ms: u64) -> (String, String) {
    match kind {
        FailureKind::Refused => (
            format!("Couldn't connect to {host}"),
            format!(
                "Nothing is accepting connections on {}. Check that the server is running and that the host and port are right.",
                port_phrase(host)
            ),
        ),
        FailureKind::Dns => (
            format!("DNS lookup failed for {}", host_name(host)),
            "The name didn't resolve. Check the spelling, or your network and DNS settings.".to_owned(),
        ),
        FailureKind::Tls => (
            "TLS handshake failed".to_owned(),
            format!(
                "{host} didn't present a certificate this machine trusts. For a self-signed server, allow invalid certificates in Settings."
            ),
        ),
        FailureKind::Timeout => (
            format!("Timed out after {}", whole_seconds(timeout_ms)),
            format!("{host} didn't answer in time. Check that it's reachable, or raise the timeout in Settings."),
        ),
        FailureKind::Connect => (
            format!("Couldn't connect to {host}"),
            "The connection dropped before a response arrived. Check the host and port, and any proxy or VPN.".to_owned(),
        ),
        FailureKind::Script => (
            "The pre-request script failed".to_owned(),
            "Nothing was sent. Fix the script and send again.".to_owned(),
        ),
        FailureKind::Other => (
            "The request failed".to_owned(),
            "No response arrived. The details below have the underlying error.".to_owned(),
        ),
    }
}

/// Classifies a transport error by its text: the HTTP layer reports a
/// category ("Connection failed: …") plus the deepest cause in the chain.
pub fn classify(raw: &str) -> FailureKind {
    let lower = raw.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    if lower.contains("script error") {
        FailureKind::Script
    } else if has(&["timed out", "timeout"]) {
        FailureKind::Timeout
    } else if has(&["dns error", "failed to lookup address", "name or service not known", "no such host", "nodename nor servname"]) {
        FailureKind::Dns
    } else if has(&["certificate", "tls", "handshake", "unknownissuer", "invalid peer"]) {
        FailureKind::Tls
    } else if has(&["connection refused", "os error 111", "os error 10061", "actively refused"]) {
        FailureKind::Refused
    } else if lower.starts_with("connection failed") {
        FailureKind::Connect
    } else {
        FailureKind::Other
    }
}

/// The cause after the HTTP layer's "Category: " prefix.
fn cause(raw: &str) -> &str {
    const CATEGORIES: [&str; 5] = [
        "Connection failed: ",
        "Request timed out: ",
        "Request failed: ",
        "Too many redirects: ",
        "Failed to read response: ",
    ];
    CATEGORIES
        .iter()
        .find_map(|p| raw.strip_prefix(p))
        .unwrap_or(raw)
        .trim()
}

/// `host[:port]` of a URL: no scheme, credentials, path, query or fragment.
pub fn authority(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let auth = &rest[..end];
    auth.rsplit_once('@').map_or(auth, |(_, host)| host)
}

/// The host without its port; IPv6 literals keep their brackets.
fn host_name(authority: &str) -> &str {
    match authority.rsplit_once(':') {
        Some((host, port))
            if !port.is_empty()
                && port.bytes().all(|b| b.is_ascii_digit())
                && (!host.contains(':') || host.ends_with(']')) =>
        {
            host
        }
        _ => authority,
    }
}

fn port_phrase(authority: &str) -> String {
    match authority.rsplit_once(':') {
        Some((_, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => format!("port {port}"),
        _ => authority.to_owned(),
    }
}

fn whole_seconds(ms: u64) -> String {
    format!("{} s", ms.div_ceil(1000))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent(url: &str) -> Sent {
        Sent::new(&HttpMethod::Get, url, Instant::now())
    }

    #[test]
    fn errors_classify_by_their_cause() {
        for (raw, kind) in [
            ("Connection failed: Connection refused (os error 111)", FailureKind::Refused),
            ("Connection failed: No connection could be made (os error 10061)", FailureKind::Refused),
            ("Connection failed: dns error: failed to lookup address information: Name or service not known", FailureKind::Dns),
            ("Connection failed: invalid peer certificate: UnknownIssuer", FailureKind::Tls),
            ("Request timed out: operation timed out", FailureKind::Timeout),
            ("Connection failed: connection reset by peer", FailureKind::Connect),
            ("Pre-request script error: x is not defined", FailureKind::Script),
            ("Global pre-request script error: boom", FailureKind::Script),
            ("Request failed: builder error", FailureKind::Other),
        ] {
            assert_eq!(classify(raw), kind, "{raw}");
        }
    }

    #[test]
    fn a_refused_connection_names_the_host_and_port() {
        let s = sent("http://127.0.0.1:18999/closed-port");
        let f = Failure::new("Connection failed: Connection refused (os error 111)", Some(&s), Duration::from_millis(3), 30_000);
        assert_eq!(f.title, "Couldn't connect to 127.0.0.1:18999");
        assert!(f.hint.contains("port 18999"));
        assert_eq!(f.detail, "Connection refused (os error 111)");
        assert_eq!(f.request.as_deref(), Some("GET http://127.0.0.1:18999/closed-port"));
        assert_eq!(f.elapsed, "3 ms");
    }

    #[test]
    fn titles_follow_the_kind() {
        let s = sent("https://api.exmaple.com/v1");
        let dns = Failure::new("Connection failed: dns error: failed to lookup address", Some(&s), Duration::ZERO, 30_000);
        assert_eq!(dns.title, "DNS lookup failed for api.exmaple.com");
        let slow = Failure::new("Request timed out: operation timed out", Some(&s), Duration::ZERO, 30_000);
        assert_eq!(slow.title, "Timed out after 30 s");
        let tls = Failure::new("Connection failed: invalid peer certificate: UnknownIssuer", Some(&s), Duration::ZERO, 30_000);
        assert_eq!(tls.title, "TLS handshake failed");
    }

    #[test]
    fn failed_responses_show_the_reason_and_time_in_the_header() {
        let resp = HttpResponse {
            error: Some("Connection failed: Connection refused (os error 111)".to_owned()),
            ..Default::default()
        };
        let view = ResponseView::of(&resp, None, Duration::from_millis(3), 30_000);
        assert_eq!(view.status, "Connection refused");
        assert_eq!(view.elapsed, "3 ms");
        assert_eq!(view.size, None);
        assert!(view.failure.is_some());
    }

    #[test]
    fn successful_responses_show_status_time_and_size() {
        let resp = HttpResponse {
            status: 200,
            status_text: "OK".to_owned(),
            duration_ms: 2,
            body_size: 1434,
            ..Default::default()
        };
        let view = ResponseView::of(&resp, None, Duration::ZERO, 30_000);
        assert_eq!(view.status, "200 OK");
        assert_eq!((view.elapsed.as_str(), view.size.as_deref()), ("2 ms", Some("1.4 KB")));
        assert_eq!(view.redirects, None, "none followed, none shown");
        assert_eq!(duration_label(1240), "1.24 s");
    }

    #[test]
    fn followed_redirects_are_counted_and_listed_hop_by_hop() {
        let hop = |status, to: &str| Redirect { status, to: to.to_owned() };
        let resp = HttpResponse {
            status: 200,
            redirects: vec![hop(301, "https://a.example/"), hop(302, "https://b.example/login")],
            ..Default::default()
        };
        let redirects = ResponseView::of(&resp, None, Duration::ZERO, 30_000).redirects.unwrap();
        assert_eq!(redirects.label, "↪ 2 redirects");
        assert_eq!(redirects.hops, "301 → https://a.example/\n302 → https://b.example/login");
        let one = HttpResponse { redirects: vec![hop(308, "https://c.example/")], ..Default::default() };
        assert_eq!(ResponseView::of(&one, None, Duration::ZERO, 30_000).redirects.unwrap().label, "↪ 1 redirect");
    }

    #[test]
    fn compressed_bodies_explain_their_wire_size() {
        let mut resp = HttpResponse {
            status: 200,
            body_size: 1434,
            wire_size: Some(380),
            ..Default::default()
        };
        let view = ResponseView::of(&resp, None, Duration::ZERO, 30_000);
        assert_eq!(view.size_tip.as_deref(), Some("1.4 KB · 380 B on the wire"));
        assert_eq!(view.size.as_deref(), Some("1.4 KB"), "the header text is unchanged");
        resp.wire_size = Some(1434);
        assert!(ResponseView::of(&resp, None, Duration::ZERO, 30_000).size_tip.is_none());
        resp.wire_size = None;
        assert!(ResponseView::of(&resp, None, Duration::ZERO, 30_000).size_tip.is_none());
    }

    #[test]
    fn authority_strips_everything_but_host_and_port() {
        assert_eq!(authority("http://127.0.0.1:18931/users?page=1"), "127.0.0.1:18931");
        assert_eq!(authority("https://user:pw@api.example.com#x"), "api.example.com");
        assert_eq!(authority("localhost:3000"), "localhost:3000");
        assert_eq!(host_name("api.example.com:443"), "api.example.com");
        assert_eq!(host_name("api.example.com"), "api.example.com");
        assert_eq!(host_name("[::1]:8080"), "[::1]");
    }
}
