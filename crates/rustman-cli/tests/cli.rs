//! `rustman-cli` end to end: the built binary against a local server.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use rustman_core::domain::collection::{Collection, SavedRequest};
use rustman_core::domain::environment::AppEnvironment;
use rustman_core::domain::group::ImportedCollection;
use rustman_core::domain::request::{HttpMethod, KeyValue};
use rustman_core::store::{Change, SessionStore, WorkspaceStore};
use rustman_formats::import::{ExportOptions, native_export};
use rustman_store::SqliteStore;
use rustman_testkit::TempDir;

/// A local server: `/fail` answers 500 and anything else 200. Each request
/// head is passed on before the reply goes out, so once the CLI has exited,
/// [`heads`] holds everything it sent.
fn serve() -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (sent, heads) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            answer(stream.unwrap(), &sent);
        }
    });
    (base, heads)
}

fn heads(heads: &Receiver<String>) -> Vec<String> {
    heads.try_iter().collect()
}

fn answer(mut stream: TcpStream, heads: &Sender<String>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let mut raw = Vec::new();
    let mut buf = [0; 4096];
    let head = loop {
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0, "the client closed before sending a request head");
        raw.extend_from_slice(&buf[..n]);
        if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break String::from_utf8_lossy(&raw[..end]).into_owned();
        }
    };
    let status = if head.starts_with("GET /fail ") {
        "500 Internal Server Error"
    } else {
        "200 OK"
    };
    heads.send(head).unwrap();
    let reply = format!("HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    stream.write_all(reply.as_bytes()).unwrap();
}

fn request(collection: &Collection, name: &str, path: &str, order: i64) -> SavedRequest {
    let mut request = SavedRequest::new_in(collection.id.clone(), name.into());
    request.method = HttpMethod::Get;
    request.url = format!("{{{{base}}}}{path}");
    request.sort_order = order;
    request.headers = vec![KeyValue {
        id: "h".into(),
        key: "X-Token".into(),
        value: "{{token}}".into(),
        enabled: true,
    }];
    request
}

fn collection() -> (Collection, Vec<SavedRequest>) {
    let collection = Collection {
        id: "col-cli".into(),
        name: "Shop".into(),
        created_at: 0,
    };
    let requests = vec![
        request(&collection, "Healthy", "/ok", 0),
        request(&collection, "Broken", "/fail", 1),
    ];
    (collection, requests)
}

fn collection_file(dir: &TempDir) -> String {
    let (collection, requests) = collection();
    let path = dir.path().join("shop.json");
    let file = ImportedCollection {
        collection,
        requests,
        groups: Vec::new(),
    };
    std::fs::write(&path, native_export(&file, ExportOptions::default())).unwrap();
    path.to_string_lossy().into_owned()
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rustman-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_collection_file_runs_in_order_and_a_failure_sets_the_exit_status() {
    let dir = TempDir::new("cli-file");
    let file = collection_file(&dir);
    let (base, server) = serve();

    let output = cli(&[
        "run",
        &file,
        "--var",
        &format!("base={base}"),
        "--var",
        "token=t1",
    ]);

    let heads = heads(&server);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let printed = stdout(&output);
    let lines: Vec<&str> = printed.lines().collect();
    assert!(
        lines[0].starts_with("200 ") && lines[0].ends_with("GET Healthy"),
        "{printed}"
    );
    assert!(
        lines[1].starts_with("500 ") && lines[1].ends_with("GET Broken"),
        "{printed}"
    );
    assert!(printed.ends_with("2 requests, 1 failed\n"), "{printed}");
    assert!(heads[0].starts_with("GET /ok ") && heads[1].starts_with("GET /fail "));
    assert!(
        heads
            .iter()
            .all(|h| h.to_ascii_lowercase().contains("x-token: t1"))
    );
}

#[test]
fn json_output_reports_every_request() {
    let dir = TempDir::new("cli-json");
    let file = collection_file(&dir);
    let (base, server) = serve();

    let output = cli(&["run", &file, "--json", "--var", &format!("base={base}")]);

    heads(&server);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        (report["total"].as_u64(), report["failed"].as_u64()),
        (Some(2), Some(1))
    );
    let requests = report["requests"].as_array().unwrap();
    assert_eq!(requests[0]["status"], 200);
    assert_eq!(requests[0]["url"], format!("{base}/ok"));
    assert_eq!(requests[0]["size"], 2);
    assert_eq!(requests[1]["passed"], false);
}

#[test]
fn one_request_can_be_picked_by_name() {
    let dir = TempDir::new("cli-pick");
    let file = collection_file(&dir);
    let (base, server) = serve();

    let output = cli(&[
        "run",
        &file,
        "--request",
        "Healthy",
        "--var",
        &format!("base={base}"),
    ]);

    assert_eq!(heads(&server).len(), 1);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).ends_with("1 request, 0 failed\n"));
}

fn saved_workspace(data_dir: &Path, base: &str) {
    let (collection, requests) = collection();
    let env = AppEnvironment {
        id: "env-staging".into(),
        name: "Staging".into(),
        variables: [
            ("base".to_owned(), base.to_owned()),
            ("token".to_owned(), "from-env".to_owned()),
        ]
        .into(),
        is_active: false,
    };
    let mut changes = vec![
        Change::PutCollection(collection),
        Change::PutEnvironment(env),
    ];
    changes.extend(
        requests
            .into_iter()
            .map(|r| Change::PutRequest(Box::new(r))),
    );
    SqliteStore::open(data_dir)
        .unwrap()
        .apply(&changes)
        .unwrap();
}

#[test]
fn a_saved_collection_runs_with_a_saved_environment() {
    let dir = TempDir::new("cli-store");
    let (base, server) = serve();
    saved_workspace(dir.path(), &base);
    let data_dir = dir.path().to_string_lossy().into_owned();

    let output = cli(&[
        "run",
        "Shop",
        "--data-dir",
        &data_dir,
        "--env",
        "Staging",
        "--request",
        "Healthy",
    ]);

    let heads = heads(&server);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        heads[0].to_ascii_lowercase().contains("x-token: from-env"),
        "{heads:?}"
    );
}

#[test]
fn nothing_to_run_is_a_usage_error() {
    let dir = TempDir::new("cli-errors");
    let data_dir = dir.path().to_string_lossy().into_owned();

    let output = cli(&["run", "Shop", "--data-dir", &data_dir]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("no saved Rustman data in"),
        "{}",
        stderr(&output)
    );
    assert!(
        !dir.path().join("rustman.db").exists(),
        "no database is created"
    );

    let output = cli(&["fly"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("unknown command 'fly'"));
    assert!(stderr(&output).contains("Usage: rustman-cli run"));

    let file = collection_file(&dir);
    let output = cli(&["run", &file, "--request", "Nope"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("no request is called 'Nope'"));
}

#[test]
fn a_postman_collection_runs_too() {
    let dir = TempDir::new("cli-postman");
    let file = dir.path().join("postman.json");
    std::fs::write(
        &file,
        r#"{
            "info": {
                "name": "PM",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [{ "name": "Ping", "request": { "method": "GET", "url": "{{base}}/ok" } }]
        }"#,
    )
    .unwrap();
    let (base, server) = serve();

    let output = cli(&[
        "run",
        &file.to_string_lossy(),
        "--var",
        &format!("base={base}"),
    ]);

    assert!(heads(&server)[0].starts_with("GET /ok "));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("GET Ping"));
}

/// A file with `Login`, whose test script stores a token, then `Profile`,
/// which sends it, and whose own test fails.
fn scripted_file(dir: &TempDir) -> String {
    let (collection, _) = collection();
    let mut login = request(&collection, "Login", "/login", 0);
    login.headers.clear();
    login.test_script =
        "set_env(\"token\", \"from-login\")\ntest(\"logged in\", response.status == 200)".into();
    let mut profile = request(&collection, "Profile", "/profile", 1);
    profile.test_script = "test(\"is created\", response.status == 201)".into();
    let path = dir.path().join("scripted.json");
    let file = ImportedCollection {
        collection,
        requests: vec![login, profile],
        groups: Vec::new(),
    };
    std::fs::write(&path, native_export(&file, ExportOptions::default())).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn a_script_sets_a_variable_the_next_request_sends_and_a_failed_test_fails_the_run() {
    let dir = TempDir::new("cli-scripts");
    let file = scripted_file(&dir);
    let (base, server) = serve();

    let output = cli(&["run", &file, "--var", &format!("base={base}")]);

    let heads = heads(&server);
    assert!(
        heads[1]
            .to_ascii_lowercase()
            .contains("x-token: from-login"),
        "{heads:?}"
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let printed = stdout(&output);
    assert!(printed.contains("failed: is created"), "{printed}");
    assert!(!printed.contains("failed: logged in"), "{printed}");
    assert!(printed.ends_with("2 requests, 1 failed\n"), "{printed}");
}

#[test]
fn json_output_lists_each_test() {
    let dir = TempDir::new("cli-scripts-json");
    let file = scripted_file(&dir);
    let (base, server) = serve();

    let output = cli(&["run", &file, "--json", "--var", &format!("base={base}")]);

    heads(&server);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["requests"][1]["tests"],
        serde_json::json!([{ "name": "is created", "passed": false }])
    );
    assert_eq!(report["requests"][0]["passed"], true);
}

#[test]
fn no_scripts_runs_none() {
    let dir = TempDir::new("cli-no-scripts");
    let file = scripted_file(&dir);
    let (base, server) = serve();

    let output = cli(&[
        "run",
        &file,
        "--no-scripts",
        "--var",
        &format!("base={base}"),
        "--var",
        "token=typed",
    ]);

    let heads = heads(&server);
    assert!(
        heads[1].to_ascii_lowercase().contains("x-token: typed"),
        "{heads:?}"
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

#[test]
fn a_saved_collection_runs_the_app_s_global_scripts() {
    let dir = TempDir::new("cli-global");
    let (base, server) = serve();
    saved_workspace(dir.path(), &base);
    let mut store = SqliteStore::open(dir.path()).unwrap();
    store
        .save_session(&serde_json::json!({ "global_test_script": "test(\"global\", true)" }))
        .unwrap();
    drop(store);
    let data_dir = dir.path().to_string_lossy().into_owned();

    let output = cli(&[
        "run",
        "Shop",
        "--data-dir",
        &data_dir,
        "--request",
        "Healthy",
        "--json",
    ]);

    heads(&server);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["requests"][0]["tests"],
        serde_json::json!([{ "name": "global", "passed": true }])
    );
}
