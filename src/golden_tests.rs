//! Golden fixtures for every format Rustman writes to disk or exports: DB
//! rows and schema, the session, native collection exports, the git working
//! tree, the Postman export and the git repo registry.
//!
//! Each fixture has two tests:
//! - **reads back**: the golden file deserializes through the real entry point,
//!   the key fields have the expected values, and re-saving it loses nothing;
//! - **matches**: today's serializer, fed the builder in `test_support`,
//!   produces exactly the golden file (compared as JSON values).
//!
//! The `legacy/` fixtures are hand-written copies of files that Aug 2026
//! builds wrote (before groups, manual ordering and GraphQL variables), so
//! the refactor keeps loading what real users have saved.
//!
//! To accept an intentional format change: `RUSTMAN_BLESS=1 cargo test`,
//! then `cargo test`, then review the diff (see `test_support::golden_path`).

use std::collections::BTreeMap;

use rusqlite::Connection;
use rusqlite::types::ValueRef;
use serde_json::Value;

use crate::domain::collection::SavedRequest;
use crate::domain::group::{Group, ImportedCollection};
use crate::domain::model::Request;
use crate::domain::request::{ApiKeyLocation, AuthType, BodyType, FormFieldType, HttpMethod};
use crate::message::{RequestTab, ResponseTab};
use crate::services::{import, repos, storage, vcs};
use crate::state::session::AppSession;
use crate::test_support::*;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn read_json(name: &str) -> Value {
    golden_json(name)
}

fn decode<T: serde::de::DeserializeOwned>(name: &str) -> T {
    serde_json::from_str(&read_golden(name))
        .unwrap_or_else(|e| panic!("{name} no longer deserializes: {e}"))
}

/// Re-saving a decoded golden must reproduce it: nothing read is dropped.
fn assert_resaves_unchanged<T: serde::Serialize>(name: &str, decoded: &T) {
    assert_json_eq(
        &format!("{name} re-saved"),
        &read_json(name),
        &as_written(decoded),
    );
}

fn by_id(requests: &[&SavedRequest]) -> BTreeMap<String, Value> {
    requests
        .iter()
        .map(|r| (r.id.clone(), as_written(*r)))
        .collect()
}

fn flat_groups(ic: &ImportedCollection) -> Vec<Group> {
    ic.all_groups().into_iter().map(Group::from).collect()
}

/// Check an imported copy of `sample_collection()` kept its whole tree.
fn assert_is_sample_collection(ic: &ImportedCollection) {
    assert_matches_sample(ic, &sample_collection());
}

/// `sample_collection()` as git holds it, by the one rule for what's secret.
fn sample_collection_as_committed() -> ImportedCollection {
    let mut committed = sample_collection();
    rustman_core::secrets::strip_collection_secrets(&mut committed);
    committed
}

fn assert_matches_sample(ic: &ImportedCollection, expected: &ImportedCollection) {
    assert_eq!(ic.collection.id, COLLECTION_ID);
    assert_eq!(ic.collection.name, "Storefront API");
    assert_eq!(ic.collection.created_at, 1_785_000_000_000);

    let root: Vec<&str> = ic.requests.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(root, ["req-get", "req-put"]);
    let top: Vec<&str> = ic.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(top, ["Catalog", "Empty folder"]);
    let catalog = &ic.groups[0];
    assert!(catalog.parent_is_collection);
    assert_eq!(catalog.parent_id, COLLECTION_ID);
    assert_eq!(catalog.requests[0].group_id.as_deref(), Some("grp-catalog"));
    let products = &catalog.groups[0];
    assert_eq!(products.name, "Products");
    assert_eq!(products.parent_id, "grp-catalog");
    assert!(!products.parent_is_collection);
    assert_eq!(products.requests[0].id, "req-full");
    assert_eq!(
        products.requests[0].group_id.as_deref(),
        Some("grp-products")
    );

    assert_eq!(by_id(&ic.all_requests()), by_id(&expected.all_requests()));
}

// ── SQLite dumps ──────────────────────────────────────────────────────────────

/// Columns holding JSON text. Dumped as parsed JSON so `HashMap` key order
/// can't make the comparison flaky.
const JSON_COLUMNS: &[&str] = &["data", "variables", "request_json"];
/// Columns holding wall-clock time, left out of the dump.
const VOLATILE_COLUMNS: &[&str] = &["saved_at"];

fn dump_table(conn: &Connection, table: &str) -> Value {
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .unwrap();
    let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
    let mut rows = stmt.query([]).unwrap();
    let mut out = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let mut obj = serde_json::Map::new();
        for (i, col) in columns.iter().enumerate() {
            if VOLATILE_COLUMNS.contains(&col.as_str()) {
                continue;
            }
            let value = match row.get_ref(i).unwrap() {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => Value::from(n),
                ValueRef::Real(f) => Value::from(f),
                ValueRef::Text(t) => {
                    let text = std::str::from_utf8(t).unwrap();
                    if JSON_COLUMNS.contains(&col.as_str()) {
                        json(text)
                    } else {
                        Value::from(text)
                    }
                }
                ValueRef::Blob(_) => panic!("{table}.{col}: unexpected blob"),
            };
            obj.insert(col.clone(), value);
        }
        out.push(Value::Object(obj));
    }
    Value::Array(out)
}

fn dump_tables(conn: &Connection, tables: &[&str]) -> Value {
    let mut obj = serde_json::Map::new();
    for table in tables {
        obj.insert((*table).to_owned(), dump_table(conn, table));
    }
    Value::Object(obj)
}

/// Insert dumped rows back, re-encoding JSON columns as text.
fn load_tables(conn: &Connection, dump: &Value) {
    for (table, rows) in dump.as_object().unwrap() {
        for row in rows.as_array().unwrap() {
            let row = row.as_object().unwrap();
            let columns: Vec<&str> = row.keys().map(String::as_str).collect();
            let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
            let values: Vec<rusqlite::types::Value> = row
                .values()
                .map(|v| match v {
                    Value::Null => rusqlite::types::Value::Null,
                    Value::Number(n) => rusqlite::types::Value::Integer(n.as_i64().unwrap()),
                    Value::String(s) => rusqlite::types::Value::Text(s.clone()),
                    other => rusqlite::types::Value::Text(other.to_string()),
                })
                .collect();
            conn.execute(
                &format!(
                    "INSERT INTO {table} ({}) VALUES ({})",
                    columns.join(", "),
                    placeholders.join(", ")
                ),
                rusqlite::params_from_iter(values),
            )
            .unwrap_or_else(|e| panic!("insert into {table}: {e}"));
        }
    }
}

fn dump_schema(conn: &Connection) -> Value {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut obj = serde_json::Map::new();
    for table in tables {
        let mut info = conn
            .prepare(
                "SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?1) ORDER BY cid",
            )
            .unwrap();
        let columns: Vec<Value> = info
            .query_map([&table], |r| {
                Ok(serde_json::json!({
                    "name": r.get::<_, String>(0)?,
                    "type": r.get::<_, String>(1)?,
                    "not_null": r.get::<_, i64>(2)? != 0,
                    "default": r.get::<_, Option<String>>(3)?,
                    "primary_key": r.get::<_, i64>(4)? != 0,
                }))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        obj.insert(table, Value::Array(columns));
    }
    Value::Object(obj)
}

/// Every table `storage::open` creates, except environments (own golden).
const DB_ROW_TABLES: &[&str] = &["collections", "groups", "requests", "history", "session"];

/// The tables as written through the storage API from the builders.
fn write_sample_db(conn: &Connection) {
    let sample = sample_collection();
    storage::create_collection(conn, &sample.collection).unwrap();
    for g in flat_groups(&sample) {
        storage::create_group(conn, &g).unwrap();
    }
    for r in sample.all_requests() {
        storage::create_request(conn, r).unwrap();
    }
    storage::add_history(conn, &history_entry()).unwrap();
    storage::save_session(conn, &full_session()).unwrap();
}

/// The schema of an Aug 2026 database, before groups and request ordering.
const AUG_2026_SCHEMA: &str = "
    CREATE TABLE collections (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE environments (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        variables TEXT NOT NULL DEFAULT '{}',
        is_active INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE history (
        id TEXT PRIMARY KEY,
        timestamp INTEGER NOT NULL,
        method TEXT NOT NULL,
        url TEXT NOT NULL,
        status INTEGER NOT NULL DEFAULT 0,
        duration INTEGER NOT NULL DEFAULT 0,
        request_json TEXT NOT NULL DEFAULT '{}'
    );
    CREATE TABLE session (
        key TEXT PRIMARY KEY DEFAULT 'current',
        data TEXT NOT NULL DEFAULT '{}',
        saved_at INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE requests (
        id TEXT PRIMARY KEY,
        collection_id TEXT NOT NULL,
        data TEXT NOT NULL DEFAULT '{}'
    );";

// ── SavedRequest ──────────────────────────────────────────────────────────────

#[test]
fn saved_request_full_golden_reads_back() {
    let name = "saved_request_full.json";
    let r: SavedRequest = decode(name);
    assert_eq!(r.id, "req-full");
    assert_eq!(r.group_id.as_deref(), Some("grp-products"));
    assert_eq!(r.sort_order, 7);
    assert_eq!(r.method, HttpMethod::Patch);
    assert_eq!(r.url, "{{base_url}}/products/42?expand=variants&trace=on");
    assert_eq!(r.headers.len(), 2);
    assert!(!r.headers[1].enabled);
    assert_eq!(r.params.len(), 3);
    assert_eq!(r.body_type, BodyType::GraphQL);
    assert!(r.body.starts_with("query Product"));
    assert_eq!(r.graphql_variables, "{\n  \"id\": \"42\"\n}");
    assert_eq!(r.auth_type, AuthType::JwtUser);
    assert_eq!(r.bearer_token, "bearer-token-123");
    assert_eq!(
        (r.basic_user.as_str(), r.basic_pass.as_str()),
        ("alice", "s3cret: with spaces")
    );
    assert_eq!(r.api_key_location, ApiKeyLocation::Query);
    assert_eq!(r.form_data_fields[0].field_type, FormFieldType::Text);
    let file = &r.form_data_fields[1];
    assert_eq!(file.field_type, FormFieldType::File);
    assert!(!file.enabled);
    assert_eq!(file.file_name.as_deref(), Some("front.png"));
    assert_eq!(file.file_data.as_deref(), Some("aGVsbG8gd29ybGQ="));
    assert_eq!(file.mime_type.as_deref(), Some("image/png"));
    assert_eq!(r.cookie_string, "session=abc; theme=dark");
    assert_eq!(r.cookies.len(), 2);
    assert_eq!(
        (r.jwt_secret.as_str(), r.jwt_subject.as_str()),
        ("jwt-signing-secret", "user-42")
    );
    assert_eq!(r.jwt_algo, "HS256");
    assert!(r.pre_request_script.contains("env.set"));
    assert!(r.test_script.contains("status == 200"));
    assert_resaves_unchanged(name, &r);
}

#[test]
fn saved_request_full_golden_matches_serializer() {
    assert_golden(
        "saved_request_full.json",
        &as_written(&full_saved_request()),
    );
}

#[test]
fn saved_request_variants_golden_reads_back() {
    let name = "saved_request_variants.json";
    let reqs: Vec<SavedRequest> = decode(name);
    let methods: Vec<&HttpMethod> = reqs.iter().map(|r| &r.method).collect();
    assert_eq!(methods, HttpMethod::all().iter().collect::<Vec<_>>());
    for auth in every_auth_type() {
        assert!(reqs.iter().any(|r| r.auth_type == auth), "{auth:?} missing");
    }
    for body in every_body_type() {
        assert!(reqs.iter().any(|r| r.body_type == body), "{body:?} missing");
    }
    for loc in [ApiKeyLocation::Header, ApiKeyLocation::Query] {
        assert!(
            reqs.iter()
                .any(|r| r.auth_type == AuthType::ApiKey && r.api_key_location == loc),
            "API key in {loc:?} missing"
        );
    }
    let basic = reqs
        .iter()
        .find(|r| r.auth_type == AuthType::Basic)
        .unwrap();
    assert_eq!(
        (basic.basic_user.as_str(), basic.basic_pass.as_str()),
        ("admin", "hunter2")
    );
    let bearer = reqs
        .iter()
        .find(|r| r.auth_type == AuthType::Bearer)
        .unwrap();
    assert_eq!(bearer.bearer_token, "{{token}}");
    assert_eq!(bearer.body_type, BodyType::Text);
    let cookie = reqs
        .iter()
        .find(|r| r.auth_type == AuthType::Cookie)
        .unwrap();
    assert_eq!(cookie.cookie_string, "sid=xyz");
    let jwt = reqs
        .iter()
        .find(|r| r.auth_type == AuthType::JwtUser)
        .unwrap();
    assert_eq!(jwt.graphql_variables, "{\"dryRun\": true}");
    assert_resaves_unchanged(name, &reqs);
}

#[test]
fn saved_request_variants_golden_matches_serializer() {
    assert_golden(
        "saved_request_variants.json",
        &as_written(&request_variants()),
    );
}

// ── Legacy SavedRequest (Aug 2026) ────────────────────────────────────────────

#[test]
fn legacy_saved_request_reads_with_defaults() {
    let r: SavedRequest = decode("legacy/saved_request_2026_08.json");
    assert_eq!(r.id, "5b1f3c2e-0d4a-4f7e-9a51-2c8e7d6b9a10");
    assert_eq!(r.collection_id, "9e8d7c6b-5a4f-4e3d-8c2b-1a0f9e8d7c6b");
    assert_eq!(r.method, HttpMethod::Get);
    assert_eq!(r.body_type, BodyType::Json);
    assert_eq!(r.auth_type, AuthType::None);
    assert_eq!(r.form_data_fields[0].field_type, FormFieldType::File);
    assert_eq!(r.form_data_fields[0].file_name, None);
    assert_eq!(r.test_script, "test(\"ok\", status == 200)");
    // Fields added after Aug 2026 take their defaults.
    assert_eq!(r.group_id, None);
    assert_eq!(r.sort_order, 0);
    assert_eq!(r.graphql_variables, "");
}

#[test]
fn legacy_saved_request_resaves_to_golden() {
    let r: SavedRequest = decode("legacy/saved_request_2026_08.json");
    assert_golden("legacy_saved_request_2026_08.resaved.json", &as_written(&r));
}

// ── Request model ─────────────────────────────────────────────────────────────

/// Every request the goldens hold: the builders, and what the legacy files
/// decode to.
fn every_saved_request() -> Vec<SavedRequest> {
    let mut all = vec![
        full_saved_request(),
        blank_request("req-blank", "Blank"),
        history_entry().request,
    ];
    all.extend(request_variants());
    all.extend(sample_collection().all_requests().into_iter().cloned());
    all.extend(second_collection().all_requests().into_iter().cloned());
    all.push(decode("legacy/saved_request_2026_08.json"));
    let legacy = import::native_import(&read_golden("legacy/collection_file_2026_08.json")).unwrap();
    all.extend(legacy[0].requests.iter().cloned());
    all
}

#[test]
fn the_request_model_converts_losslessly() {
    for saved in every_saved_request() {
        let model = Request::from(saved.clone());
        assert_json_eq(&saved.id, &as_written(&saved), &as_written(&model));
        assert_json_eq(
            &saved.id,
            &as_written(&saved),
            &as_written(&SavedRequest::from(model)),
        );
    }
}

#[test]
fn the_request_model_reads_and_writes_the_golden_requests() {
    let full: Request = decode("saved_request_full.json");
    assert_resaves_unchanged("saved_request_full.json", &full);
    let variants: Vec<Request> = decode("saved_request_variants.json");
    assert_resaves_unchanged("saved_request_variants.json", &variants);
    let legacy: Request = decode("legacy/saved_request_2026_08.json");
    assert_json_eq(
        "legacy request re-saved",
        &read_json("legacy_saved_request_2026_08.resaved.json"),
        &as_written(&legacy),
    );
}

// ── Legacy collection file (Aug 2026 native export and git file) ──────────────

fn assert_is_legacy_collection(collections: &[ImportedCollection]) {
    assert_eq!(collections.len(), 1);
    let ic = &collections[0];
    assert_eq!(ic.collection.name, "Users API");
    assert_eq!(ic.collection.created_at, 1_754_000_000_000);
    assert!(ic.groups.is_empty(), "files without `groups` import flat");
    let names: Vec<&str> = ic.requests.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Get profile", "Upload avatar"]);
    let upload = &ic.requests[1];
    assert_eq!(upload.auth_type, AuthType::Bearer);
    assert_eq!(upload.body_type, BodyType::FormData);
    assert_eq!(
        upload.form_data_fields[0].file_data.as_deref(),
        Some("iVBORw0KGgo=")
    );
    assert!(!upload.params[0].enabled);
    assert!(
        ic.requests
            .iter()
            .all(|r| r.group_id.is_none() && r.sort_order == 0)
    );
}

#[test]
fn legacy_collection_file_imports_natively() {
    let text = read_golden("legacy/collection_file_2026_08.json");
    assert_is_legacy_collection(&import::native_import(&text).unwrap());
    // The import picker reaches the same importer.
    assert_is_legacy_collection(&import::import_collection_file(&text).unwrap());
}

#[test]
fn legacy_collection_file_loads_from_git() {
    let repo = TempDir::new("git-legacy");
    let text = read_golden("legacy/collection_file_2026_08.json");
    std::fs::write(
        repo.path()
            .join("9e8d7c6b-5a4f-4e3d-8c2b-1a0f9e8d7c6b.json"),
        text,
    )
    .unwrap();
    assert_is_legacy_collection(&vcs::load_collections_from(repo.path()).unwrap().collections);
}

#[test]
fn legacy_collection_file_resaves_to_golden() {
    let imported = import::native_import(&read_golden("legacy/collection_file_2026_08.json"))
        .unwrap()
        .remove(0);
    let exported = import::native_export(&imported, WITH_SECRETS);
    assert_golden(
        "legacy_collection_file_2026_08.resaved.json",
        &json(&exported),
    );
}

// ── Session ───────────────────────────────────────────────────────────────────

#[test]
fn session_full_golden_reads_back() {
    let name = "session_full.json";
    let s: AppSession = decode(name);
    assert_eq!(s.tabs.len(), 2);
    assert_eq!(s.active_tab, 1);
    assert_eq!(s.active_env_id.as_deref(), Some("env-prod"));
    assert_eq!(s.sidebar_panel, "History");
    assert_eq!(s.sidebar_width, 312.5);
    assert!(s.sidebar_collapsed);
    assert_eq!(s.theme_idx, 3);
    assert!(s.horizontal_layout);
    assert_eq!(s.panel_split, 35);
    assert_eq!(s.ui_scale, 1.25);
    assert_eq!(s.default_timeout_ms, 45_000);
    assert_eq!(s.global_pre_request_script, GLOBAL_PRE_REQUEST_SCRIPT);
    assert_eq!(s.global_test_script, GLOBAL_TEST_SCRIPT);
    assert_eq!(s.git_user_name.as_deref(), Some("Ada Lovelace"));
    assert_eq!(s.git_user_email.as_deref(), Some("ada@example.invalid"));
    assert!(s.tls_accept_invalid_certs && s.tls_http1_only);
    assert!(s.tls_force_tls12 && s.tls_force_tls13);
    assert!(s.auto_check_updates);

    let tab = &s.tabs[0];
    assert_eq!(tab.title, "Update product*");
    assert_eq!(tab.method, HttpMethod::Patch);
    assert_eq!(tab.body_type, BodyType::GraphQL);
    assert_eq!(tab.graphql_variables, "{\n  \"id\": \"42\"\n}");
    assert_eq!(tab.auth_type, AuthType::JwtUser);
    assert_eq!(tab.form_fields[1].file_name.as_deref(), Some("front.png"));
    assert_eq!(
        tab.saved_as,
        Some((COLLECTION_ID.into(), "req-full".into()))
    );
    assert_eq!(tab.active_request_tab, RequestTab::Scripts);
    assert_eq!(tab.active_response_tab, ResponseTab::Tests);
    assert_eq!(s.tabs[1].saved_as, None);
    assert_resaves_unchanged(name, &s);
}

#[test]
fn session_full_golden_matches_serializer() {
    assert_golden("session_full.json", &as_written(&full_session()));
}

#[test]
fn legacy_session_reads_with_defaults() {
    let s: AppSession = decode("legacy/session_2026_08.json");
    assert_eq!(s.tabs.len(), 2, "no tab is dropped");
    assert_eq!(s.active_tab, 1);
    assert_eq!(s.active_env_id.as_deref(), Some("env-0001"));
    assert_eq!(s.sidebar_panel, "Environments");
    assert_eq!(s.theme_idx, 2);
    assert_eq!(s.default_timeout_ms, 10_000);
    assert_eq!(s.global_pre_request_script, "env.set(\"run\", \"1\")");
    assert!(s.tls_accept_invalid_certs);
    // Settings added after Aug 2026 take their defaults.
    assert_eq!(s.sidebar_width, 200.0);
    assert!(!s.sidebar_collapsed);
    assert!(!s.horizontal_layout);
    assert_eq!(s.panel_split, 50);
    assert_eq!(s.ui_scale, 1.0);
    assert_eq!(s.git_user_name, None);
    assert_eq!(s.git_user_email, None);
    assert!(!s.auto_check_updates);

    let upload = &s.tabs[0];
    assert_eq!(upload.body_type, BodyType::FormData);
    assert_eq!(upload.form_fields[0].file_name.as_deref(), Some("me.png"));
    assert_eq!(upload.graphql_variables, "");
    assert_eq!(upload.active_request_tab, RequestTab::Body);
    assert_eq!(upload.active_response_tab, ResponseTab::Headers);
    assert_eq!(
        upload.saved_as.as_ref().map(|(_, r)| r.as_str()),
        Some("7a6b5c4d-3e2f-4a1b-9c8d-7e6f5a4b3c2d")
    );

    // The oldest tab shape: no cookies, JWT fields or sub-tab memory.
    let scratch = &s.tabs[1];
    assert_eq!(scratch.auth_type, AuthType::Basic);
    assert_eq!(scratch.api_key_location, ApiKeyLocation::Query);
    assert!(scratch.cookies.is_empty());
    assert_eq!(scratch.jwt_algo, "HS256");
    assert_eq!(scratch.active_request_tab, RequestTab::default());
    assert_eq!(scratch.saved_as, None);
}

#[test]
fn legacy_session_resaves_to_golden() {
    let s: AppSession = decode("legacy/session_2026_08.json");
    assert_golden("legacy_session_2026_08.resaved.json", &as_written(&s));
}

// ── Native collection export ──────────────────────────────────────────────────

/// The export goldens pin every field, credentials included; the default,
/// which leaves them out, is pinned by rustman-formats' tests.
const WITH_SECRETS: import::ExportOptions = import::ExportOptions {
    include_secrets: true,
};

#[test]
fn native_export_golden_imports() {
    let name = "native_export.json";
    let imported = import::native_import(&read_golden(name)).unwrap();
    assert_eq!(imported.len(), 1);
    assert_is_sample_collection(&imported[0]);
    let ic = &imported[0];
    let again = import::native_export(ic, WITH_SECRETS);
    assert_json_eq("native export re-exported", &read_json(name), &json(&again));
}

#[test]
fn native_export_golden_matches_exporter() {
    let ic = sample_collection();
    let exported = import::native_export(&ic, WITH_SECRETS);
    assert_golden("native_export.json", &json(&exported));
}

#[test]
fn native_export_all_golden_imports() {
    let name = "native_export_all.json";
    let imported = import::native_import(&read_golden(name)).unwrap();
    assert_eq!(imported.len(), 2);
    assert_is_sample_collection(&imported[0]);
    assert_eq!(imported[1].collection.name, "Auth service");
    assert_eq!(imported[1].requests[0].auth_type, AuthType::Cookie);
    assert_eq!(imported[1].requests[0].collection_id, "col-auth");
    let again = import::native_export_all(&imported, WITH_SECRETS);
    assert_json_eq("export-all re-exported", &read_json(name), &json(&again));
}

#[test]
fn native_export_all_golden_matches_exporter() {
    let exported =
        import::native_export_all(&[sample_collection(), second_collection()], WITH_SECRETS);
    assert_golden("native_export_all.json", &json(&exported));
}

// ── Postman export ────────────────────────────────────────────────────────────

#[test]
fn postman_export_golden_imports() {
    let imported = import::postman::import(&read_golden("postman_export.json"))
        .unwrap()
        .collections;
    assert_eq!(imported.len(), 1);
    let ic = &imported[0];
    assert_eq!(ic.collection.name, "Storefront API");
    let top: Vec<&str> = ic.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(top, ["Catalog", "Empty folder"]);
    let products = &ic.groups[0].groups[0];
    assert_eq!(products.name, "Products");
    let full = &products.requests[0];
    assert_eq!(full.name, "Update product (every field)");
    assert_eq!(full.method, HttpMethod::Patch);
    assert_eq!(full.body_type, BodyType::GraphQL);
    assert_eq!(full.graphql_variables, "{\n  \"id\": \"42\"\n}");
    let upload = &products.requests[1];
    assert_eq!(upload.body_type, BodyType::FormData);
    let fields: Vec<(&str, &FormFieldType)> = upload
        .form_data_fields
        .iter()
        .map(|f| (f.key.as_str(), &f.field_type))
        .collect();
    assert_eq!(
        fields,
        [
            ("caption", &FormFieldType::Text),
            ("image", &FormFieldType::File)
        ]
    );
    let root: Vec<&str> = ic.requests.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(root, ["List products", "Replace description"]);
    assert_eq!(ic.all_requests().len(), 5);
}

#[test]
fn postman_export_golden_matches_exporter() {
    let exported = import::postman::export(&sample_collection(), WITH_SECRETS);
    assert_golden("postman_export.json", &json(&exported));
}

// ── Git working tree ──────────────────────────────────────────────────────────

const GIT_COLLECTION_FILE: &str = "git_working_tree/col-storefront.json";
const GIT_GLOBAL_FILE: &str = "git_working_tree/global.json";

#[test]
fn git_working_tree_golden_loads() {
    let repo = TempDir::new("git-golden-read");
    for name in [GIT_COLLECTION_FILE, GIT_GLOBAL_FILE] {
        let file = std::path::Path::new(name).file_name().unwrap();
        std::fs::write(repo.path().join(file), read_golden(name)).unwrap();
    }
    let loaded = vcs::load_collections_from(repo.path()).unwrap().collections;
    assert_eq!(loaded.len(), 1, "global.json is not a collection");
    assert_matches_sample(&loaded[0], &sample_collection_as_committed());
    assert_eq!(
        vcs::read_global_script(repo.path()),
        Some((GLOBAL_PRE_REQUEST_SCRIPT.into(), GLOBAL_TEST_SCRIPT.into()))
    );
}

/// Commits made before secrets were stripped still carry them; reading one
/// back keeps every value, and an empty local store takes them all.
#[test]
fn legacy_git_collection_with_secrets_keeps_them() {
    let repo = TempDir::new("git-legacy-secrets");
    std::fs::write(
        repo.path().join(format!("{COLLECTION_ID}.json")),
        read_golden("legacy/git_collection_with_secrets.json"),
    )
    .unwrap();
    let mut loaded = vcs::load_collections_from(repo.path()).unwrap().collections;
    assert_eq!(loaded.len(), 1);
    vcs::keep_local_secrets(&mut loaded[0], &[]);
    assert_is_sample_collection(&loaded[0]);
}

#[test]
fn git_working_tree_golden_matches_writer() {
    let repo = TempDir::new("git-golden-write");
    vcs::init(repo.path()).unwrap();
    vcs::write_working_tree(
        repo.path(),
        &[sample_collection()],
        GLOBAL_PRE_REQUEST_SCRIPT,
        GLOBAL_TEST_SCRIPT,
    )
    .unwrap();
    let mut files: Vec<String> = std::fs::read_dir(repo.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, [".git", "col-storefront.json", "global.json"]);
    for name in [GIT_COLLECTION_FILE, GIT_GLOBAL_FILE] {
        let file = std::path::Path::new(name).file_name().unwrap();
        let written = std::fs::read_to_string(repo.path().join(file)).unwrap();
        assert_golden(name, &json(&written));
    }
}

// ── Environments (DB rows) ────────────────────────────────────────────────────

#[test]
fn environments_golden_rows_load() {
    let dir = TempDir::new("env-golden-read");
    let conn = storage::open(dir.path()).unwrap();
    load_tables(&conn, &read_json("environments.json"));
    let envs = storage::get_environments(&conn).unwrap();
    let names: Vec<&str> = envs.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Development", "Production"]);
    assert!(!envs[0].is_active);
    assert!(envs[1].is_active);
    for (got, want) in envs.iter().zip(environments()) {
        assert_eq!(got.id, want.id);
        assert_eq!(got.variables, want.variables);
    }
    assert_eq!(envs[1].variables["token"], "{{vault_token}}");
    assert_eq!(envs[1].variables["empty"], "");
}

#[test]
fn environments_golden_matches_storage() {
    let dir = TempDir::new("env-golden-write");
    let conn = storage::open(dir.path()).unwrap();
    for env in environments() {
        storage::save_environment(&conn, &env).unwrap();
    }
    assert_golden("environments.json", &dump_tables(&conn, &["environments"]));
}

// ── DB rows: collections, groups, requests, history, session ──────────────────

#[test]
fn db_rows_golden_load() {
    let dir = TempDir::new("db-golden-read");
    let conn = storage::open(dir.path()).unwrap();
    load_tables(&conn, &read_json("db_rows.json"));
    let sample = sample_collection();

    let collections = storage::get_collections(&conn).unwrap();
    assert_eq!(collections.len(), 1);
    assert_json_eq(
        "collection",
        &as_written(&sample.collection),
        &as_written(&collections[0]),
    );

    let groups = storage::get_groups(&conn).unwrap();
    let mut got: Vec<Value> = groups.iter().map(as_written).collect();
    let mut want: Vec<Value> = flat_groups(&sample).iter().map(as_written).collect();
    got.sort_by_key(|v| v["id"].to_string());
    want.sort_by_key(|v| v["id"].to_string());
    assert_eq!(got, want);

    let requests = storage::get_requests(&conn, COLLECTION_ID).unwrap();
    let refs: Vec<&SavedRequest> = requests.iter().collect();
    assert_eq!(by_id(&refs), by_id(&sample.all_requests()));
    let full = requests.iter().find(|r| r.id == "req-full").unwrap();
    assert_eq!(full.group_id.as_deref(), Some("grp-products"));

    let history = storage::get_history(&conn).unwrap();
    assert_eq!(history.len(), 1);
    assert_json_eq(
        "history entry",
        &as_written(&history_entry()),
        &as_written(&history[0]),
    );

    let session = storage::load_session(&conn)
        .unwrap()
        .expect("a saved session");
    assert_json_eq(
        "session",
        &as_written(&full_session()),
        &as_written(&session),
    );
}

#[test]
fn db_rows_golden_matches_storage() {
    let dir = TempDir::new("db-golden-write");
    let conn = storage::open(dir.path()).unwrap();
    write_sample_db(&conn);
    assert_golden("db_rows.json", &dump_tables(&conn, DB_ROW_TABLES));
}

// ── DB schema ─────────────────────────────────────────────────────────────────

/// A database written by an Aug 2026 build (no `groups` table, no
/// `group_id`/`sort_order` columns) is migrated in place to today's schema
/// and every row in it still loads.
#[test]
fn aug_2026_database_upgrades_to_golden_schema_and_loads() {
    let dir = TempDir::new("db-legacy");
    let legacy_request = read_golden("legacy/saved_request_2026_08.json");
    let legacy_session = read_golden("legacy/session_2026_08.json");
    {
        let old = Connection::open(dir.path().join("rustman.db")).unwrap();
        old.execute_batch(AUG_2026_SCHEMA).unwrap();
        old.execute(
            "INSERT INTO collections (id, name, created_at) VALUES \
             ('9e8d7c6b-5a4f-4e3d-8c2b-1a0f9e8d7c6b', 'Users API', 1754000000000)",
            [],
        )
        .unwrap();
        old.execute(
            "INSERT INTO requests (id, collection_id, data) VALUES \
             ('5b1f3c2e-0d4a-4f7e-9a51-2c8e7d6b9a10', '9e8d7c6b-5a4f-4e3d-8c2b-1a0f9e8d7c6b', ?1)",
            [&legacy_request],
        )
        .unwrap();
        old.execute(
            "INSERT INTO history (id, timestamp, method, url, status, duration, request_json) \
             VALUES ('h-old', 1754000000500, 'GET', 'https://api.example.com/users/me', 200, 42, ?1)",
            [&legacy_request],
        )
        .unwrap();
        old.execute(
            "INSERT INTO environments (id, name, variables, is_active) VALUES \
             ('env-0001', 'Staging', '{\"base_url\":\"https://staging.example.com\"}', 1)",
            [],
        )
        .unwrap();
        old.execute(
            "INSERT INTO session (key, data, saved_at) VALUES ('current', ?1, 1754000000900)",
            [&legacy_session],
        )
        .unwrap();
    }

    let conn = storage::open(dir.path()).unwrap();
    assert_json_eq(
        "migrated schema",
        &read_json("db_schema.json"),
        &dump_schema(&conn),
    );

    assert_eq!(
        storage::get_collections(&conn).unwrap()[0].name,
        "Users API"
    );
    assert!(storage::get_groups(&conn).unwrap().is_empty());
    let requests = storage::get_requests(&conn, "9e8d7c6b-5a4f-4e3d-8c2b-1a0f9e8d7c6b").unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].name, "Get profile");
    assert_eq!(
        (requests[0].group_id.clone(), requests[0].sort_order),
        (None, 0)
    );
    let history = storage::get_history(&conn).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].duration_ms, 42);
    assert_eq!(history[0].request.name, "Get profile");
    let envs = storage::get_environments(&conn).unwrap();
    assert_eq!(envs[0].variables["base_url"], "https://staging.example.com");
    assert!(envs[0].is_active);
    let session = storage::load_session(&conn)
        .unwrap()
        .expect("a saved session");
    assert_eq!(session.tabs.len(), 2);
    assert_eq!(session.theme_idx, 2);

    // Re-opening an already migrated database changes nothing.
    drop(conn);
    let conn = storage::open(dir.path()).unwrap();
    assert_json_eq(
        "re-migrated schema",
        &read_json("db_schema.json"),
        &dump_schema(&conn),
    );
    assert_eq!(
        storage::get_requests(&conn, "9e8d7c6b-5a4f-4e3d-8c2b-1a0f9e8d7c6b")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn db_schema_golden_matches_fresh_database() {
    let dir = TempDir::new("db-schema");
    let conn = storage::open(dir.path()).unwrap();
    assert_golden("db_schema.json", &dump_schema(&conn));
}

// ── Git repo registry (git_repos.json) ────────────────────────────────────────

#[test]
fn git_repos_golden_loads() {
    let dir = TempDir::new("repos-golden-read");
    std::fs::write(
        dir.path().join("git_repos.json"),
        read_golden("git_repos.json"),
    )
    .unwrap();
    let repos = repos::load(dir.path());
    let ids: Vec<&str> = repos.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        [repos::LOCAL_ID, "team-api"],
        "no second Local repo is added"
    );
    assert_eq!(
        repos[1].path,
        std::path::PathBuf::from("/data/rustman/repos/team-api")
    );
    assert_eq!(
        repos[1].remote_url.as_deref(),
        Some("https://example.invalid/team/api.git")
    );
    assert_eq!(repos[1].collection_ids, ["col-auth", "col-billing"]);
    assert_eq!(repos[0].collection_ids, [COLLECTION_ID]);
}

#[test]
fn git_repos_golden_matches_writer() {
    let dir = TempDir::new("repos-golden-write");
    repos::save(dir.path(), &git_repos());
    let written = std::fs::read_to_string(dir.path().join("git_repos.json")).unwrap();
    assert_golden("git_repos.json", &json(&written));
}
