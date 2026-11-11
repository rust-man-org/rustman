//! What actually goes over the wire. Each test sends to a one-shot server on
//! a free local port that records the raw request.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use base64::{Engine as _, engine::general_purpose};
use rustman_core::domain::collection::SavedRequest;
use rustman_core::domain::request::{
    ApiKeyLocation, AuthType, BodyType, FormField, FormFieldType, HttpMethod, KeyValue,
};
use rustman_core::domain::response::{HttpResponse, Redirect};
use rustman_core::run::{Runner, SendOptions};
use rustman_http::{ReqwestTransport, TlsOptions, build_client_with};

struct Captured {
    head: String,
    body: Vec<u8>,
}

impl Captured {
    fn header(&self, name: &str) -> Vec<&str> {
        self.head
            .lines()
            .filter_map(|line| line.split_once(':'))
            .filter(|(k, _)| k.trim().eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim())
            .collect()
    }

    fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The request line's target, e.g. `/a?q=1`.
    fn target(&self) -> &str {
        self.head.split(' ').nth(1).unwrap_or_default()
    }
}

/// How long the test server waits for the client before giving up, so a
/// broken client fails the test instead of hanging the suite.
const SERVER_PATIENCE: Duration = Duration::from_secs(10);

/// Serves one request with `response`, returning the URL to send to and a
/// handle that yields what the client sent.
fn serve_once(response: Vec<u8>) -> (String, JoinHandle<Captured>) {
    let (url, handle) = serve(vec![response]);
    (url, thread::spawn(move || handle.join().unwrap().remove(0)))
}

/// Serves one request per response, in order, on one address.
fn serve(responses: Vec<Vec<u8>>) -> (String, JoinHandle<Vec<Captured>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        responses
            .into_iter()
            .map(|response| answer(&listener, &response))
            .collect()
    });
    (url, handle)
}

fn answer(listener: &TcpListener, response: &[u8]) -> Captured {
    let mut stream = accept_within(listener, SERVER_PATIENCE);
    stream.set_read_timeout(Some(SERVER_PATIENCE)).unwrap();
    let mut raw = Vec::new();
    let mut buf = [0; 8192];
    let head_end = loop {
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0, "the client closed before sending a request head");
        raw.extend_from_slice(&buf[..n]);
        if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).into_owned();
    let length = head
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    while raw.len() < head_end + length {
        let n = stream.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..n]);
    }
    stream.write_all(response).unwrap();
    let end = raw.len().min(head_end + length);
    Captured {
        head,
        body: raw[head_end..end].to_vec(),
    }
}

fn accept_within(listener: &TcpListener, patience: Duration) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + patience;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                return stream;
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("no request arrived: {e}"),
        }
    }
}

fn ok() -> Vec<u8> {
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec()
}

fn post(url: String, body_type: BodyType) -> SavedRequest {
    let mut req = SavedRequest::new_in(String::new(), "test".to_owned());
    req.method = HttpMethod::Post;
    req.url = url;
    req.body_type = body_type;
    req
}

fn field(key: &str, value: &str) -> FormField {
    FormField {
        id: key.to_owned(),
        key: key.to_owned(),
        value: value.to_owned(),
        field_type: FormFieldType::Text,
        enabled: true,
        file_name: None,
        file_data: None,
        mime_type: None,
    }
}

fn file(key: &str, name: &str, bytes: &[u8], mime: &str) -> FormField {
    FormField {
        field_type: FormFieldType::File,
        file_name: Some(name.to_owned()),
        file_data: Some(general_purpose::STANDARD.encode(bytes)),
        mime_type: Some(mime.to_owned()),
        ..field(key, "")
    }
}

async fn send(req: &SavedRequest) -> HttpResponse {
    let transport = ReqwestTransport::new(build_client_with(TlsOptions::default()).unwrap());
    let options = SendOptions {
        timeout: Duration::from_secs(5),
        ..SendOptions::default()
    };
    HttpResponse::from(
        Runner::new(transport, options)
            .execute(req.resolve(None))
            .await,
    )
}

/// Sends `req` and asserts it went through, before anyone joins the server.
async fn send_to_test_server(req: &SavedRequest) -> HttpResponse {
    let response = send(req).await;
    assert_eq!(response.error, None);
    response
}

fn redirect(status: &str, location: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .into_bytes()
}

/// A GET carrying a bearer token and an API key row.
fn credentialed(url: String) -> SavedRequest {
    let mut req = post(url, BodyType::None);
    req.method = HttpMethod::Get;
    req.auth_type = AuthType::Bearer;
    "secret".clone_into(&mut req.bearer_token);
    req.headers = vec![KeyValue {
        id: "1".to_owned(),
        key: "X-Api-Key".to_owned(),
        value: "k".to_owned(),
        enabled: true,
    }];
    req
}

#[tokio::test]
async fn a_redirect_to_another_host_carries_none_of_the_request_s_headers() {
    let (landing, landed) = serve_once(ok());
    let (start, first) = serve_once(redirect("302 Found", &format!("{landing}/landing")));

    let response = send_to_test_server(&credentialed(start)).await;

    let first = first.join().unwrap();
    assert_eq!(first.header("x-api-key"), ["k"]);
    assert_eq!(first.header("authorization"), ["Bearer secret"]);
    let landed = landed.join().unwrap();
    assert!(landed.head.starts_with("GET /landing "), "{}", landed.head);
    assert!(landed.header("x-api-key").is_empty(), "{}", landed.head);
    assert!(landed.header("authorization").is_empty(), "{}", landed.head);
    assert_eq!(response.status, 200);
    assert_eq!(
        response.redirects,
        [Redirect {
            status: 302,
            to: format!("{landing}/landing")
        }]
    );
}

#[tokio::test]
async fn a_redirect_loop_stops_after_ten_hops_keeping_same_origin_headers() {
    let (url, server) = serve(vec![redirect("301 Moved Permanently", "/again"); 11]);

    let response = send(&credentialed(url)).await;

    assert_eq!(
        response.error.as_deref(),
        Some("Too many redirects (more than 10)")
    );
    let hops = server.join().unwrap();
    assert_eq!(
        hops[10].header("x-api-key"),
        ["k"],
        "the same origin keeps its headers"
    );
}

#[tokio::test]
async fn multipart_carries_text_and_file_parts() {
    let (url, server) = serve_once(ok());
    let mut req = post(format!("{url}/upload"), BodyType::FormData);
    req.form_data_fields = vec![
        field("name", "rustman"),
        file("avatar", "avatar.png", b"fake png bytes", "image/png"),
    ];

    send_to_test_server(&req).await;
    let wire = server.join().unwrap();
    assert!(wire.header("content-type")[0].starts_with("multipart/form-data; boundary="));
    let body = wire.body_text();
    assert!(body.contains("name=\"name\"\r\n\r\nrustman\r\n"), "{body}");
    assert!(body.contains("filename=\"avatar.png\""), "{body}");
    assert!(
        body.contains("Content-Type: image/png\r\n\r\nfake png bytes"),
        "{body}"
    );
}

#[tokio::test]
async fn file_parts_keep_their_mime_type() {
    const XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
    let (url, server) = serve_once(ok());
    let mut req = post(format!("{url}/upload"), BodyType::FormData);
    req.form_data_fields = vec![file("report", "report.xlsx", b"fake xlsx bytes", XLSX)];

    send_to_test_server(&req).await;
    let body = server.join().unwrap().body_text();
    assert!(body.contains(&format!("Content-Type: {XLSX}")), "{body}");
}

#[tokio::test]
async fn text_only_forms_are_urlencoded() {
    let (url, server) = serve_once(ok());
    let mut req = post(format!("{url}/login"), BodyType::FormData);
    req.form_data_fields = vec![field("conta", "sync_dep_1"), field("modulo", "SYNC")];

    send_to_test_server(&req).await;
    let wire = server.join().unwrap();
    assert_eq!(
        wire.header("content-type"),
        ["application/x-www-form-urlencoded"]
    );
    assert_eq!(wire.body_text(), "conta=sync_dep_1&modulo=SYNC");
}

#[tokio::test]
async fn text_bodies_are_sent_as_plain_text() {
    let (url, server) = serve_once(ok());
    let mut req = post(url, BodyType::Text);
    req.body = "hello".to_owned();

    send_to_test_server(&req).await;
    let wire = server.join().unwrap();
    assert_eq!(wire.header("content-type"), ["text/plain"]);
    assert_eq!(wire.body_text(), "hello");
}

fn param(key: &str, value: &str) -> KeyValue {
    KeyValue {
        id: key.to_owned(),
        key: key.to_owned(),
        value: value.to_owned(),
        enabled: true,
    }
}

#[tokio::test]
async fn a_bare_flag_is_sent_without_an_equals_sign() {
    let (url, server) = serve_once(ok());
    let mut req = post(format!("{url}/a?stale=1#top"), BodyType::None);
    req.method = HttpMethod::Get;
    req.params = vec![param("debug", ""), param("", "v"), param("q", "a b")];
    req.auth_type = AuthType::ApiKey;
    "key".clone_into(&mut req.api_key_name);
    "k 1".clone_into(&mut req.api_key_value);
    req.api_key_location = ApiKeyLocation::Query;

    send_to_test_server(&req).await;
    assert_eq!(server.join().unwrap().target(), "/a?debug&=v&q=a+b&key=k+1");
}

#[tokio::test]
async fn a_url_without_params_gets_no_trailing_question_mark() {
    let (url, server) = serve_once(ok());
    let mut req = post(format!("{url}/plain"), BodyType::None);
    req.method = HttpMethod::Get;

    send_to_test_server(&req).await;
    assert_eq!(server.join().unwrap().target(), "/plain");
}

#[tokio::test]
async fn the_url_s_own_query_is_sent_as_written() {
    let (url, server) = serve_once(ok());
    let mut req = post(format!("{url}/a?x=1&flag#top"), BodyType::None);
    req.method = HttpMethod::Get;

    send_to_test_server(&req).await;
    assert_eq!(server.join().unwrap().target(), "/a?x=1&flag");
}

#[tokio::test]
async fn repeated_header_rows_are_all_sent() {
    let (url, server) = serve_once(ok());
    let mut req = post(url, BodyType::None);
    req.headers = ["a", "b"]
        .iter()
        .map(|v| KeyValue {
            id: (*v).to_owned(),
            key: "X-Tag".to_owned(),
            value: (*v).to_owned(),
            enabled: true,
        })
        .collect();

    send_to_test_server(&req).await;
    assert_eq!(server.join().unwrap().header("x-tag"), ["a", "b"]);
}

#[tokio::test]
async fn gzip_responses_are_decoded() {
    // gzip of `{"hello":"world"}`.
    const GZIPPED: [u8; 37] = [
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 171, 86, 202, 72, 205, 201, 201, 87, 178, 82, 42, 207,
        47, 202, 73, 81, 170, 5, 0, 209, 65, 9, 216, 17, 0, 0, 0,
    ];
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        GZIPPED.len()
    )
    .into_bytes();
    response.extend_from_slice(&GZIPPED);
    let (url, server) = serve_once(response);
    let mut req = post(url, BodyType::None);
    req.method = HttpMethod::Get;
    req.headers = vec![KeyValue {
        id: "1".to_owned(),
        key: "Accept-Encoding".to_owned(),
        value: "gzip, deflate, br, zstd".to_owned(),
        enabled: true,
    }];

    let response = send_to_test_server(&req).await;
    let wire = server.join().unwrap();
    assert_eq!(wire.header("accept-encoding"), ["gzip, deflate, br, zstd"]);
    assert_eq!(response.body, r#"{"hello":"world"}"#);
    assert_eq!(response.body_size, 17);
    assert_eq!(response.headers["content-encoding"], "gzip");
    assert_eq!(response.headers["content-length"], "37");
}

#[tokio::test]
async fn every_decoded_encoding_is_asked_for_by_default() {
    let (url, server) = serve_once(ok());
    let mut req = post(url, BodyType::None);
    req.method = HttpMethod::Get;

    send_to_test_server(&req).await;
    assert_eq!(
        server.join().unwrap().header("accept-encoding"),
        ["zstd,gzip,deflate,br"]
    );
}

#[tokio::test]
async fn binary_bodies_are_kept_as_bytes() {
    let mut response =
        b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 4\r\nConnection: close\r\n\r\n"
            .to_vec();
    response.extend_from_slice(&[0x89, b'P', 0xff, 0x00]);
    let (url, server) = serve_once(response);
    let mut req = post(url, BodyType::None);
    req.method = HttpMethod::Get;

    let response = send_to_test_server(&req).await;
    server.join().unwrap();
    assert!(response.is_binary);
    assert!(response.body.is_empty());
    assert_eq!(response.body_size, 4);
    assert_eq!(
        response.binary_data.as_deref(),
        Some(&[0x89, b'P', 0xff, 0x00][..])
    );
    assert_eq!(response.headers["content-type"], "image/png");
}

fn ok_saying(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

/// With `HTTP_PROXY` set, requests go through the proxy, except to the
/// user's own machine: a proxy can't reach it, and browsers never send
/// loopback through one. The environment is set for a child run of this
/// test only, so no other test sees it.
#[tokio::test]
async fn loopback_skips_the_environment_s_proxy_and_other_hosts_use_it() {
    const CHILD: &str = "RUSTMAN_TEST_PROXY_CHILD";
    let get = |url: String| {
        let mut req = post(url, BodyType::None);
        req.method = HttpMethod::Get;
        req
    };
    if std::env::var_os(CHILD).is_some() {
        let (direct, server) = serve_once(ok_saying("direct"));
        let response = send(&get(format!("{direct}/local"))).await;
        assert_eq!(response.body, "direct", "{:?}", response.error);
        server.join().unwrap();
        let response = send(&get("http://proxied.invalid/remote".to_owned())).await;
        assert_eq!(response.body, "via proxy", "{:?}", response.error);
        return;
    }
    let (proxy, proxied) = serve_once(ok_saying("via proxy"));
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "loopback_skips_the_environment_s_proxy_and_other_hosts_use_it",
            "--exact",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env("HTTP_PROXY", &proxy);
    for name in [
        "NO_PROXY",
        "no_proxy",
        "ALL_PROXY",
        "all_proxy",
        "http_proxy",
        "REQUEST_METHOD",
    ] {
        child.env_remove(name);
    }
    let output = child.output().unwrap();
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{report}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        report.contains("1 passed"),
        "the child ran no test: {report}"
    );
    assert_eq!(
        proxied.join().unwrap().target(),
        "http://proxied.invalid/remote"
    );
}
