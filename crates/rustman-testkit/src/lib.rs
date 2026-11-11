//! Deterministic fixtures for Rustman's tests: domain builders with fixed ids
//! and timestamps (so their serialized form can be compared byte-for-byte
//! against golden files), JSON comparison helpers and self-cleaning temp dirs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rustman_core::domain::collection::{Collection, SavedRequest};
use rustman_core::domain::environment::AppEnvironment;
use rustman_core::domain::group::{ImportedCollection, ImportedGroup};
use rustman_core::domain::history::HistoryEntry;
use rustman_core::domain::request::{
    ApiKeyLocation, AuthType, BodyType, FormField, FormFieldType, HttpMethod, KeyValue,
};
use serde_json::Value;

/// A fresh directory under the system temp dir, removed on drop, so a test
/// that panics halfway still cleans up. Tests never touch the user's real
/// data directory.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("rustman-test-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            eprintln!("could not remove temp dir {}: {e}", self.0.display());
        }
    }
}

// ── JSON ──────────────────────────────────────────────────────────────────────

/// Parse JSON text the way it would be read back from disk.
pub fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| {
        let head: String = text.chars().take(80).collect();
        panic!("not valid JSON ({e}): {head}")
    })
}

/// `value` as the real serializer writes it, parsed back. Going through the
/// text (not `serde_json::to_value`) matters: an `f32` such as `0.1` only
/// compares equal once it has been printed and re-read.
pub fn as_written<T: serde::Serialize>(value: &T) -> Value {
    json(&serde_json::to_string(value).expect("serializes"))
}

/// Assert two JSON documents are equal, naming the first place they differ.
pub fn assert_json_eq(what: &str, expected: &Value, actual: &Value) {
    if let Some((at, want, got)) = first_difference(expected, actual) {
        panic!("{what}: JSON differs at {at}:\n  expected: {want}\n  actual:   {got}");
    }
}

/// The JSON pointer and both values at the first place `a` and `b` differ.
pub fn first_difference(a: &Value, b: &Value) -> Option<(String, String, String)> {
    difference_at("", a, b)
}

fn difference_at(at: &str, a: &Value, b: &Value) -> Option<(String, String, String)> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                let here = format!("{at}/{k}");
                match y.get(k) {
                    Some(vb) => {
                        if let Some(d) = difference_at(&here, va, vb) {
                            return Some(d);
                        }
                    }
                    None => return Some((here, va.to_string(), "(missing)".into())),
                }
            }
            y.keys()
                .find(|k| !x.contains_key(*k))
                .map(|k| (format!("{at}/{k}"), "(missing)".into(), y[k].to_string()))
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                if let Some(d) = difference_at(&format!("{at}/{i}"), va, vb) {
                    return Some(d);
                }
            }
            (x.len() != y.len()).then(|| {
                (
                    format!("{at} (array length)"),
                    x.len().to_string(),
                    y.len().to_string(),
                )
            })
        }
        _ => (a != b).then(|| (at.to_owned(), a.to_string(), b.to_string())),
    }
}

// ── Exhaustive variant lists ──────────────────────────────────────────────────
//
// The `match`es are there so adding a variant fails to compile until the
// fixtures cover it too.

pub fn every_auth_type() -> Vec<AuthType> {
    let all = [
        AuthType::None,
        AuthType::Basic,
        AuthType::Bearer,
        AuthType::ApiKey,
        AuthType::JwtUser,
        AuthType::Cookie,
    ];
    for a in &all {
        match a {
            AuthType::None
            | AuthType::Basic
            | AuthType::Bearer
            | AuthType::ApiKey
            | AuthType::JwtUser
            | AuthType::Cookie => {}
        }
    }
    all.to_vec()
}

pub fn every_body_type() -> Vec<BodyType> {
    let all = [
        BodyType::None,
        BodyType::Json,
        BodyType::Text,
        BodyType::FormData,
        BodyType::GraphQL,
    ];
    for b in &all {
        match b {
            BodyType::None
            | BodyType::Json
            | BodyType::Text
            | BodyType::FormData
            | BodyType::GraphQL => {}
        }
    }
    all.to_vec()
}

// ── Builders ──────────────────────────────────────────────────────────────────

pub const COLLECTION_ID: &str = "col-storefront";

pub fn kv(id: &str, key: &str, value: &str, enabled: bool) -> KeyValue {
    KeyValue {
        id: id.into(),
        key: key.into(),
        value: value.into(),
        enabled,
    }
}

/// A request with every field empty or default, under fixed ids.
pub fn blank_request(id: &str, name: &str) -> SavedRequest {
    SavedRequest {
        id: id.into(),
        collection_id: COLLECTION_ID.into(),
        group_id: None,
        sort_order: 0,
        name: name.into(),
        method: HttpMethod::Get,
        url: String::new(),
        headers: Vec::new(),
        params: Vec::new(),
        body: String::new(),
        body_type: BodyType::None,
        graphql_variables: String::new(),
        auth_type: AuthType::None,
        bearer_token: String::new(),
        basic_user: String::new(),
        basic_pass: String::new(),
        api_key_name: String::new(),
        api_key_value: String::new(),
        api_key_location: ApiKeyLocation::Header,
        form_data_fields: Vec::new(),
        cookie_string: String::new(),
        cookies: Vec::new(),
        jwt_secret: String::new(),
        jwt_subject: String::new(),
        jwt_algo: "HS256".into(),
        pre_request_script: String::new(),
        test_script: String::new(),
    }
}

pub fn form_fields() -> Vec<FormField> {
    vec![
        FormField {
            id: "ff-text".into(),
            key: "caption".into(),
            value: "Front view".into(),
            field_type: FormFieldType::Text,
            enabled: true,
            file_name: None,
            file_data: None,
            mime_type: None,
        },
        FormField {
            id: "ff-file".into(),
            key: "image".into(),
            value: String::new(),
            field_type: FormFieldType::File,
            enabled: false,
            file_name: Some("front.png".into()),
            // base64 of "hello world": file uploads live inside the model.
            file_data: Some("aGVsbG8gd29ybGQ=".into()),
            mime_type: Some("image/png".into()),
        },
    ]
}

/// One request with every field set to a non-default value. (`jwt_algo`
/// keeps `HS256`: it is the only value the app accepts.)
pub fn full_saved_request() -> SavedRequest {
    SavedRequest {
        id: "req-full".into(),
        collection_id: COLLECTION_ID.into(),
        group_id: Some("grp-products".into()),
        sort_order: 7,
        name: "Update product (every field)".into(),
        method: HttpMethod::Patch,
        url: "{{base_url}}/products/42?expand=variants&trace=on".into(),
        headers: vec![
            kv("h-ct", "Content-Type", "application/json", true),
            kv("h-debug", "X-Debug", "1", false),
        ],
        params: vec![
            kv("p-expand", "expand", "variants", true),
            kv("p-trace", "trace", "on", true),
            kv("p-draft", "draft", "true", false),
        ],
        body: "query Product($id: ID!) { product(id: $id) { id name } }".into(),
        body_type: BodyType::GraphQL,
        graphql_variables: "{\n  \"id\": \"42\"\n}".into(),
        auth_type: AuthType::JwtUser,
        bearer_token: "bearer-token-123".into(),
        basic_user: "alice".into(),
        basic_pass: "s3cret: with spaces".into(),
        api_key_name: "X-Api-Key".into(),
        api_key_value: "key-456".into(),
        api_key_location: ApiKeyLocation::Query,
        form_data_fields: form_fields(),
        cookie_string: "session=abc; theme=dark".into(),
        cookies: vec![
            kv("c-session", "session", "abc", true),
            kv("c-theme", "theme", "dark", false),
        ],
        jwt_secret: "jwt-signing-secret".into(),
        jwt_subject: "user-42".into(),
        jwt_algo: "HS256".into(),
        pre_request_script: "env.set(\"started\", now())\n".into(),
        test_script: "test(\"ok\", status == 200)\n".into(),
    }
}

/// Seven requests: one per HTTP method, covering every auth type, every body
/// type and both API-key locations, each with the fields its kind uses.
pub fn request_variants() -> Vec<SavedRequest> {
    let mut none = blank_request("req-get", "List products");
    none.url = "{{base_url}}/products?page=1".into();
    none.params = vec![kv("p-page", "page", "1", true)];

    let mut basic = blank_request("req-post", "Create product");
    basic.method = HttpMethod::Post;
    basic.url = "{{base_url}}/products".into();
    basic.auth_type = AuthType::Basic;
    basic.basic_user = "admin".into();
    basic.basic_pass = "hunter2".into();
    basic.body_type = BodyType::Json;
    basic.body = "{\n  \"name\": \"Lamp\",\n  \"price\": 12.5\n}".into();
    basic.headers = vec![kv("h-ct", "Content-Type", "application/json", true)];

    let mut bearer = blank_request("req-put", "Replace description");
    bearer.method = HttpMethod::Put;
    bearer.url = "{{base_url}}/products/1/description".into();
    bearer.auth_type = AuthType::Bearer;
    bearer.bearer_token = "{{token}}".into();
    bearer.body_type = BodyType::Text;
    bearer.body = "Plain text, with \"quotes\" and a\nnewline.".into();

    let mut api_key_header = blank_request("req-patch", "Upload image");
    api_key_header.method = HttpMethod::Patch;
    api_key_header.url = "{{base_url}}/products/1/image".into();
    api_key_header.auth_type = AuthType::ApiKey;
    api_key_header.api_key_name = "X-Api-Key".into();
    api_key_header.api_key_value = "abc123".into();
    api_key_header.api_key_location = ApiKeyLocation::Header;
    api_key_header.body_type = BodyType::FormData;
    api_key_header.form_data_fields = form_fields();

    let mut jwt = blank_request("req-delete", "Delete product");
    jwt.method = HttpMethod::Delete;
    jwt.url = "{{base_url}}/products/1".into();
    jwt.auth_type = AuthType::JwtUser;
    jwt.jwt_secret = "secret".into();
    jwt.jwt_subject = "admin".into();
    jwt.body_type = BodyType::GraphQL;
    jwt.body = "mutation { deleteProduct(id: 1) }".into();
    jwt.graphql_variables = "{\"dryRun\": true}".into();

    let mut cookie = blank_request("req-head", "Check session");
    cookie.method = HttpMethod::Head;
    cookie.url = "{{base_url}}/session".into();
    cookie.auth_type = AuthType::Cookie;
    cookie.cookie_string = "sid=xyz".into();
    cookie.cookies = vec![kv("c-sid", "sid", "xyz", true)];

    let mut api_key_query = blank_request("req-options", "CORS preflight");
    api_key_query.method = HttpMethod::Options;
    api_key_query.url = "{{base_url}}/products".into();
    api_key_query.auth_type = AuthType::ApiKey;
    api_key_query.api_key_name = "api_key".into();
    api_key_query.api_key_value = "q-789".into();
    api_key_query.api_key_location = ApiKeyLocation::Query;
    api_key_query.body_type = BodyType::Json;
    api_key_query.body = "{}".into();

    let mut out = vec![
        none,
        basic,
        bearer,
        api_key_header,
        jwt,
        cookie,
        api_key_query,
    ];
    for (i, r) in out.iter_mut().enumerate() {
        r.sort_order = i64::try_from(i).expect("a handful of variants");
    }
    out
}

fn variant(id: &str) -> SavedRequest {
    request_variants()
        .into_iter()
        .find(|r| r.id == id)
        .expect("known variant id")
}

/// A collection with root requests, a nested folder (Catalog → Products) and
/// an empty folder: the shape `app::tree::collection_from_state` produces,
/// where every request inside a folder carries that folder's `group_id`.
pub fn sample_collection() -> ImportedCollection {
    let mut catalog_req = variant("req-post");
    catalog_req.group_id = Some("grp-catalog".into());
    catalog_req.sort_order = 0;

    let mut full = full_saved_request();
    full.group_id = Some("grp-products".into());
    full.sort_order = 0;
    let mut upload = variant("req-patch");
    upload.group_id = Some("grp-products".into());
    upload.sort_order = 1;

    let mut root_first = variant("req-get");
    root_first.sort_order = 0;
    let mut root_second = variant("req-put");
    root_second.sort_order = 1;

    ImportedCollection {
        collection: Collection {
            id: COLLECTION_ID.into(),
            name: "Storefront API".into(),
            created_at: 1_785_000_000_000,
        },
        requests: vec![root_first, root_second],
        groups: vec![
            ImportedGroup {
                id: "grp-catalog".into(),
                parent_id: COLLECTION_ID.into(),
                parent_is_collection: true,
                name: "Catalog".into(),
                sort_order: 0,
                requests: vec![catalog_req],
                groups: vec![ImportedGroup {
                    id: "grp-products".into(),
                    parent_id: "grp-catalog".into(),
                    parent_is_collection: false,
                    name: "Products".into(),
                    sort_order: 0,
                    requests: vec![full, upload],
                    groups: Vec::new(),
                }],
            },
            ImportedGroup {
                id: "grp-empty".into(),
                parent_id: COLLECTION_ID.into(),
                parent_is_collection: true,
                name: "Empty folder".into(),
                sort_order: 1,
                requests: Vec::new(),
                groups: Vec::new(),
            },
        ],
    }
}

/// A second, flat collection, for the "export all" shape.
pub fn second_collection() -> ImportedCollection {
    let mut req = variant("req-head");
    req.collection_id = "col-auth".into();
    ImportedCollection {
        collection: Collection {
            id: "col-auth".into(),
            name: "Auth service".into(),
            created_at: 1_785_000_100_000,
        },
        requests: vec![req],
        groups: Vec::new(),
    }
}

pub const GLOBAL_PRE_REQUEST_SCRIPT: &str = "env.set(\"run_id\", uuid())\n";
pub const GLOBAL_TEST_SCRIPT: &str = "test(\"fast\", duration < 2000)\n";

pub fn environments() -> Vec<AppEnvironment> {
    let vars = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    };
    vec![
        AppEnvironment {
            id: "env-dev".into(),
            name: "Development".into(),
            variables: vars(&[
                ("base_url", "http://localhost:8080"),
                ("token", "dev-token"),
            ]),
            is_active: false,
        },
        AppEnvironment {
            id: "env-prod".into(),
            name: "Production".into(),
            variables: vars(&[
                ("base_url", "https://api.example.com"),
                ("token", "{{vault_token}}"),
                ("empty", ""),
            ]),
            is_active: true,
        },
    ]
}

pub fn history_entry() -> HistoryEntry {
    // A sent request is stored without its group placement.
    let mut request = variant("req-post");
    request.id = "hist-req".into();
    request.url = "https://api.example.com/products".into();
    HistoryEntry {
        id: "hist-1".into(),
        timestamp: 1_785_000_200_000,
        method: "POST".into(),
        url: request.url.clone(),
        status: 201,
        duration_ms: 87,
        request,
        omitted: Vec::new(),
    }
}
