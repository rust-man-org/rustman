//! The app's test fixtures: everything in `rustman_testkit`, plus golden-file
//! helpers and builders for the app's own types (tabs, the session, the git
//! repo registry).

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::domain::request::{ApiKeyLocation, AuthType, BodyType, HttpMethod};
use crate::message::{RequestTab, ResponseTab};
use crate::services::repos::GitRepo;
use crate::state::session::AppSession;
use crate::state::tabs::TabSnapshot;

pub use rustman_testkit::*;

// ── Golden files ──────────────────────────────────────────────────────────────

/// Golden files pin today's on-disk and export formats, so a refactor can't
/// change what users have saved without a test failing.
///
/// When a format change is intentional:
///
/// 1. Copy each golden it changes to `legacy/<name>_<yyyy_mm>.json`, so the
///    files users already have keep being tested for loading.
/// 2. Rewrite the goldens with `RUSTMAN_BLESS=1 cargo test` (refused in CI).
/// 3. Run `cargo test` once more without it (tests that read a golden may
///    have seen the old file during the blessing run), and review the diff
///    under `tests/fixtures/golden/` before committing it.
///
/// Files under `legacy/` are hand-written copies of what older versions
/// wrote: they are only ever read, never compared against or blessed.
pub fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/golden")
        .join(name)
}

fn blessing() -> bool {
    let bless = std::env::var_os("RUSTMAN_BLESS").is_some_and(|v| v == "1");
    assert!(
        !(bless && std::env::var_os("CI").is_some()),
        "RUSTMAN_BLESS is refused in CI: bless locally and commit the reviewed diff"
    );
    bless
}

/// The text of a golden or legacy fixture.
pub fn read_golden(name: &str) -> String {
    let path = golden_path(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "golden file {} is unreadable ({e}). Generate it with RUSTMAN_BLESS=1 cargo test",
            path.display()
        )
    })
}

/// A golden or legacy fixture, parsed.
pub fn golden_json(name: &str) -> Value {
    serde_json::from_str(&read_golden(name))
        .unwrap_or_else(|e| panic!("golden file {name} is not valid JSON: {e}"))
}

/// Assert that `actual` matches the golden file `name`, compared as JSON
/// values (key order and whitespace don't matter). With `RUSTMAN_BLESS=1`
/// the file is rewritten instead, but only when its content changed, so
/// re-blessing doesn't churn `HashMap` key order. The write goes through a
/// rename so a test reading the same golden never sees half a file.
pub fn assert_golden(name: &str, actual: &Value) {
    assert!(
        !name.starts_with("legacy/"),
        "{name}: legacy fixtures are only read, never compared or blessed"
    );
    let path = golden_path(name);
    if blessing() {
        let current = path.exists().then(|| golden_json(name));
        if current.as_ref() != Some(actual) {
            let mut text = serde_json::to_string_pretty(actual).expect("serializes");
            text.push('\n');
            std::fs::create_dir_all(path.parent().expect("golden dir")).expect("create dir");
            let tmp = path.with_extension("json.blessing");
            std::fs::write(&tmp, text).expect("write golden file");
            std::fs::rename(&tmp, &path).expect("move golden file into place");
        }
        return;
    }
    if let Some((at, want, got)) = first_difference(&golden_json(name), actual) {
        panic!(
            "{name} no longer matches its golden file.\n  at {at}:\n    golden:  {want}\n    current: {got}\n\
             If the format change is intentional, follow the steps on `golden_path`."
        );
    }
}

// ── Benches ───────────────────────────────────────────────────────────────────

/// The profile a bench runs on a copy of: `RM_BENCH_DATA`. Without it the
/// bench says it was skipped, straight to stderr so the line shows even
/// when the test harness captures output.
pub fn bench_data() -> Option<PathBuf> {
    let data = std::env::var_os("RM_BENCH_DATA").map(PathBuf::from);
    if data.is_none() {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stderr(),
            "skipped: set RM_BENCH_DATA to a directory holding rustman.db"
        );
    }
    data
}

// ── App builders ──────────────────────────────────────────────────────────────

/// A tab with every field set to a non-default value.
pub fn full_tab() -> TabSnapshot {
    let r = full_saved_request();
    TabSnapshot {
        id: "tab-full".into(),
        title: "Update product*".into(),
        method: r.method,
        url: r.url,
        headers: r.headers,
        params: r.params,
        body_type: r.body_type,
        body: r.body,
        graphql_variables: r.graphql_variables,
        form_fields: r.form_data_fields,
        auth_type: r.auth_type,
        bearer_token: r.bearer_token,
        basic_user: r.basic_user,
        basic_pass: r.basic_pass,
        api_key_name: r.api_key_name,
        api_key_value: r.api_key_value,
        api_key_location: r.api_key_location,
        cookie_string: r.cookie_string,
        cookies: r.cookies,
        jwt_secret: r.jwt_secret,
        jwt_subject: r.jwt_subject,
        jwt_algo: r.jwt_algo,
        pre_request_script: r.pre_request_script,
        test_script: r.test_script,
        saved_as: Some((COLLECTION_ID.into(), r.id)),
        active_request_tab: RequestTab::Scripts,
        active_response_tab: ResponseTab::Tests,
    }
}

/// An unsaved scratch tab: no `saved_as`, default sub-tabs.
pub fn scratch_tab() -> TabSnapshot {
    TabSnapshot {
        id: "tab-scratch".into(),
        title: "New Request".into(),
        method: HttpMethod::Post,
        url: "https://httpbin.org/anything".into(),
        headers: Vec::new(),
        params: Vec::new(),
        body_type: BodyType::Text,
        body: "draft".into(),
        graphql_variables: String::new(),
        form_fields: Vec::new(),
        auth_type: AuthType::None,
        bearer_token: String::new(),
        basic_user: String::new(),
        basic_pass: String::new(),
        api_key_name: String::new(),
        api_key_value: String::new(),
        api_key_location: ApiKeyLocation::Header,
        cookie_string: String::new(),
        cookies: Vec::new(),
        jwt_secret: String::new(),
        jwt_subject: String::new(),
        jwt_algo: "HS256".into(),
        pre_request_script: String::new(),
        test_script: String::new(),
        saved_as: None,
        active_request_tab: RequestTab::default(),
        active_response_tab: ResponseTab::default(),
    }
}

/// A session with every field the golden pins set to a non-default value. The
/// values are exactly representable floats so they print and re-read
/// unchanged. Fields added later stay default (and unserialized) until the
/// golden is deliberately re-blessed to pin them.
pub fn full_session() -> AppSession {
    AppSession {
        tabs: vec![full_tab(), scratch_tab()],
        active_tab: 1,
        active_env_id: Some("env-prod".into()),
        sidebar_panel: "History".into(),
        sidebar_width: 312.5,
        sidebar_collapsed: true,
        theme_idx: 3,
        horizontal_layout: true,
        panel_split: 35,
        ui_scale: 1.25,
        default_timeout_ms: 45_000,
        global_pre_request_script: GLOBAL_PRE_REQUEST_SCRIPT.into(),
        global_test_script: GLOBAL_TEST_SCRIPT.into(),
        git_user_name: Some("Ada Lovelace".into()),
        git_user_email: Some("ada@example.invalid".into()),
        tls_accept_invalid_certs: true,
        tls_http1_only: true,
        tls_force_tls12: true,
        tls_force_tls13: true,
        auto_check_updates: true,
        ..AppSession::default()
    }
}

pub fn git_repos() -> Vec<GitRepo> {
    vec![
        GitRepo {
            id: crate::services::repos::LOCAL_ID.into(),
            name: "Local".into(),
            path: PathBuf::from("/data/rustman/collections"),
            remote_url: None,
            collection_ids: vec![COLLECTION_ID.into()],
        },
        GitRepo {
            id: "team-api".into(),
            name: "team-api".into(),
            path: PathBuf::from("/data/rustman/repos/team-api"),
            remote_url: Some("https://example.invalid/team/api.git".into()),
            collection_ids: vec!["col-auth".into(), "col-billing".into()],
        },
    ]
}
