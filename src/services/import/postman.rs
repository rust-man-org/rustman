use serde::Deserialize;

use crate::domain::{
    collection::{Collection, SavedRequest},
    group::{ImportedCollection, ImportedGroup},
    request::{ApiKeyLocation, AuthType, BodyType, FormField, FormFieldType, HttpMethod, KeyValue},
};

#[derive(Deserialize)]
struct PostmanCollection {
    info: PostmanInfo,
    item: Vec<PostmanItem>,
    /// Collection-level auth; folders and requests without their own
    /// inherit it (Postman's "Inherit auth from parent").
    auth: Option<serde_json::Value>,
    /// Collection variables. Rustman resolves `{{var}}` from environments,
    /// so these become an environment named after the collection.
    #[serde(default)]
    variable: Vec<PostmanVariable>,
}

#[derive(Deserialize)]
struct PostmanInfo {
    name: String,
    #[serde(rename = "_postman_id")]
    postman_id: Option<String>,
}

#[derive(Deserialize)]
struct PostmanVariable {
    key: Option<String>,
    value: Option<serde_json::Value>,
    #[serde(default)]
    disabled: bool,
}

/// Marks a Postman script imported as comments: Rustman scripts use their
/// own language, so the source is kept for porting but never run.
pub const SCRIPT_MARKER: &str = "// Postman script (not run in Rustman";

#[derive(Deserialize)]
#[serde(untagged)]
enum PostmanItem {
    Folder(PostmanFolder),
    Request(PostmanRequestItem),
}

#[derive(Deserialize)]
struct PostmanFolder {
    name: String,
    item: Vec<PostmanItem>,
    auth: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct PostmanRequestItem {
    name: String,
    request: PostmanRequest,
    #[serde(default)]
    event: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct PostmanRequest {
    method: Option<String>,
    url: Option<PostmanUrl>,
    header: Option<Vec<PostmanHeader>>,
    body: Option<PostmanBody>,
    auth: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PostmanUrl {
    Raw(String),
    Object(PostmanUrlObject),
}

#[derive(Deserialize)]
struct PostmanUrlObject {
    raw: Option<String>,
    query: Option<Vec<PostmanParam>>,
}

#[derive(Deserialize)]
struct PostmanHeader {
    key: String,
    value: String,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct PostmanParam {
    key: String,
    value: Option<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct PostmanBody {
    mode: Option<String>,
    raw: Option<String>,
    formdata: Option<Vec<PostmanFormField>>,
    urlencoded: Option<Vec<PostmanFormField>>,
    graphql: Option<PostmanGraphQl>,
    options: Option<serde_json::Value>,
}

/// Postman's GraphQL body: the query and its variables live side by side. Older
/// exports carry the variables as a JSON *string*, newer ones as an object, so
/// both shapes are accepted and normalised to text.
#[derive(Deserialize)]
struct PostmanGraphQl {
    query: Option<String>,
    variables: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct PostmanFormField {
    key: String,
    value: Option<String>,
    #[serde(rename = "type", default)]
    field_type: String,
    #[serde(default)]
    disabled: bool,
}

// ── Import ────────────────────────────────────────────────────────────────────

pub fn import(json: &str) -> Result<Vec<ImportedCollection>, String> {
    let col: PostmanCollection =
        serde_json::from_str(json).map_err(|e| format!("Invalid Postman JSON: {e}"))?;

    let collection = Collection {
        id: uuid::Uuid::new_v4().to_string(),
        name: col.info.name,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    // Folders become groups so the imported tree matches what the user sees
    // in Postman instead of being flattened into one long request list
    // (issue #53).
    let (requests, groups) = convert_items(
        &collection.id,
        &collection.id,
        true,
        0,
        &col.item,
        col.auth.as_ref(),
    );
    Ok(vec![ImportedCollection {
        collection,
        requests,
        groups,
    }])
}

/// The collection's enabled variables as an inactive environment named
/// after it. The id derives from the Postman id, so importing the same
/// collection again updates that environment instead of adding another.
pub fn collection_environment(json: &str) -> Option<crate::domain::environment::AppEnvironment> {
    let col: PostmanCollection = serde_json::from_str(json).ok()?;
    let variables: std::collections::HashMap<String, String> = col
        .variable
        .iter()
        .filter(|v| !v.disabled)
        .filter_map(|v| {
            let key = v.key.as_deref()?.trim();
            (!key.is_empty()).then(|| (key.to_owned(), value_text(v.value.as_ref())))
        })
        .collect();
    if variables.is_empty() {
        return None;
    }
    Some(crate::domain::environment::AppEnvironment {
        id: match &col.info.postman_id {
            Some(id) if !id.trim().is_empty() => format!("postman-collection-{id}"),
            _ => uuid::Uuid::new_v4().to_string(),
        },
        name: col.info.name.clone(),
        variables,
        is_active: false,
    })
}

/// Postman values are usually strings, sometimes numbers or booleans.
fn value_text(v: Option<&serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// A Postman environment file (`{name, values: [{key, value, enabled}]}`),
/// as exported per environment or found in a data dump's `environment/`.
/// Disabled values are skipped, matching what Postman resolves.
pub fn import_environment(
    json: &str,
) -> Result<crate::domain::environment::AppEnvironment, String> {
    #[derive(Deserialize)]
    struct EnvFile {
        id: Option<String>,
        name: Option<String>,
        values: Vec<EnvValue>,
    }
    #[derive(Deserialize)]
    struct EnvValue {
        key: Option<String>,
        value: Option<serde_json::Value>,
        #[serde(default = "enabled_default")]
        enabled: bool,
    }
    fn enabled_default() -> bool {
        true
    }
    let file: EnvFile =
        serde_json::from_str(json).map_err(|e| format!("Not a Postman environment: {e}"))?;
    let variables = file
        .values
        .iter()
        .filter(|v| v.enabled)
        .filter_map(|v| {
            let key = v.key.as_deref()?.trim();
            (!key.is_empty()).then(|| (key.to_owned(), value_text(v.value.as_ref())))
        })
        .collect();
    Ok(crate::domain::environment::AppEnvironment {
        id: match &file.id {
            Some(id) if !id.trim().is_empty() => format!("postman-env-{id}"),
            _ => uuid::Uuid::new_v4().to_string(),
        },
        name: file
            .name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "Imported environment".to_owned()),
        variables,
        is_active: false,
    })
}

/// A Postman data dump's `archive.json`: only an index of ids, no data.
pub fn is_dump_index(json: &str) -> bool {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(json) else {
        return false;
    };
    !map.is_empty()
        && map.keys().all(|k| {
            matches!(
                k.as_str(),
                "collection" | "environment" | "globals" | "headerPreset"
            )
        })
        && map.values().all(|v| v.is_object())
}

/// Read one auth parameter. v2.1 stores `[{key, value}]`, v2.0 an object.
fn auth_param(auth: &serde_json::Value, kind: &str, key: &str) -> String {
    match auth.get(kind) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .find(|i| i.get("key").and_then(|k| k.as_str()) == Some(key))
            .map(|i| value_text(i.get("value")))
            .unwrap_or_default(),
        Some(serde_json::Value::Object(map)) => value_text(map.get(key)),
        _ => String::new(),
    }
}

/// An item's own auth, or `None` when it says "inherit from parent".
fn own_auth(auth: Option<&serde_json::Value>) -> Option<&serde_json::Value> {
    auth.filter(|a| a.get("type").and_then(|t| t.as_str()) != Some("inherit"))
}

/// Copy a Postman auth block onto the request. Types Rustman has no
/// equivalent for (OAuth 2, Digest, AWS…) leave the request on No Auth.
fn apply_auth(req: &mut SavedRequest, auth: Option<&serde_json::Value>) {
    let Some(auth) = auth else { return };
    match auth.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        "bearer" => {
            req.auth_type = AuthType::Bearer;
            req.bearer_token = auth_param(auth, "bearer", "token");
        }
        "basic" => {
            req.auth_type = AuthType::Basic;
            req.basic_user = auth_param(auth, "basic", "username");
            req.basic_pass = auth_param(auth, "basic", "password");
        }
        "apikey" => {
            req.auth_type = AuthType::ApiKey;
            req.api_key_name = auth_param(auth, "apikey", "key");
            req.api_key_value = auth_param(auth, "apikey", "value");
            req.api_key_location = if auth_param(auth, "apikey", "in") == "query" {
                ApiKeyLocation::Query
            } else {
                ApiKeyLocation::Header
            };
        }
        _ => {}
    }
}

/// The request's own `prerequest` / `test` script source, commented out
/// under [`SCRIPT_MARKER`] so it is kept for porting but never executed.
fn script_source(events: &[serde_json::Value], listen: &str) -> String {
    let lines: Vec<String> = events
        .iter()
        .filter(|e| e.get("listen").and_then(|l| l.as_str()) == Some(listen))
        .filter_map(|e| e.get("script")?.get("exec").cloned())
        .flat_map(|exec| match exec {
            serde_json::Value::Array(lines) => lines
                .iter()
                .map(|l| l.as_str().unwrap_or_default().to_owned())
                .collect::<Vec<_>>(),
            serde_json::Value::String(s) => s.lines().map(str::to_owned).collect(),
            _ => Vec::new(),
        })
        .collect();
    if lines.iter().all(|l| l.trim().is_empty()) {
        return String::new();
    }
    let mut out = format!("{SCRIPT_MARKER} — port it to Rustman script)\n");
    for line in lines {
        out.push_str("// ");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Convert a Postman item list into (direct request leaves, child groups)
/// sharing one manual ordering sequence. `collection_id` is the root
/// collection every leaf request belongs to, regardless of nesting depth.
fn convert_items(
    collection_id: &str,
    parent_id: &str,
    parent_is_collection: bool,
    sort_order: i64,
    items: &[PostmanItem],
    inherited_auth: Option<&serde_json::Value>,
) -> (Vec<SavedRequest>, Vec<crate::domain::group::ImportedGroup>) {
    let mut requests = Vec::new();
    let mut groups = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let order = sort_order + i as i64;
        match item {
            PostmanItem::Request(ri) => {
                let mut req = convert_request(collection_id, ri);
                apply_auth(
                    &mut req,
                    own_auth(ri.request.auth.as_ref()).or(inherited_auth),
                );
                req.sort_order = order;
                requests.push(req);
            }
            PostmanItem::Folder(folder) => {
                let group_id = uuid::Uuid::new_v4().to_string();
                let (child_reqs, child_groups) = convert_items(
                    collection_id,
                    &group_id,
                    false,
                    0,
                    &folder.item,
                    own_auth(folder.auth.as_ref()).or(inherited_auth),
                );
                groups.push(crate::domain::group::ImportedGroup {
                    id: group_id,
                    parent_id: parent_id.to_owned(),
                    parent_is_collection,
                    name: folder.name.clone(),
                    sort_order: order,
                    requests: child_reqs,
                    groups: child_groups,
                });
            }
        }
    }
    (requests, groups)
}

fn convert_request(collection_id: &str, item: &PostmanRequestItem) -> SavedRequest {
    let r = &item.request;

    let method: HttpMethod = r
        .method
        .as_deref()
        .unwrap_or("GET")
        .parse()
        .unwrap_or(HttpMethod::Get);

    let (url, params) = match &r.url {
        Some(PostmanUrl::Raw(s)) => (s.clone(), vec![]),
        Some(PostmanUrl::Object(o)) => {
            let raw = o.raw.clone().unwrap_or_default();
            let params: Vec<KeyValue> = o
                .query
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|p| KeyValue {
                    id: uuid::Uuid::new_v4().to_string(),
                    key: p.key.clone(),
                    value: p.value.clone().unwrap_or_default(),
                    enabled: !p.disabled,
                })
                .collect();
            // The query is kept in `params`; strip it from the URL so it isn't
            // also sent inline (which would duplicate every query value).
            let url = match raw.find('?') {
                Some(i) if !params.is_empty() => raw[..i].to_string(),
                _ => raw,
            };
            (url, params)
        }
        None => (String::new(), vec![]),
    };

    let headers: Vec<KeyValue> = r
        .header
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|h| KeyValue {
            id: uuid::Uuid::new_v4().to_string(),
            key: h.key.clone(),
            value: h.value.clone(),
            enabled: !h.disabled,
        })
        .collect();

    let (body, body_type, form_fields, graphql_variables) = match &r.body {
        Some(b) => match b.mode.as_deref() {
            Some("raw") => {
                let raw = b.raw.clone().unwrap_or_default();
                let language = b
                    .options
                    .as_ref()
                    .and_then(|o| o.get("raw")?.get("language")?.as_str());
                // Postman marks the raw language; without it, sniff JSON.
                let is_json = match language {
                    Some(lang) => lang.eq_ignore_ascii_case("json"),
                    None => {
                        let t = raw.trim_start();
                        t.is_empty() || t.starts_with('{') || t.starts_with('[')
                    }
                };
                let body_type = if is_json {
                    BodyType::Json
                } else {
                    BodyType::Text
                };
                (raw, body_type, vec![], String::new())
            }
            // Text-only form fields go out url-encoded (see services::http).
            Some("urlencoded") => {
                let fields = b
                    .urlencoded
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|f| FormField {
                        id: uuid::Uuid::new_v4().to_string(),
                        key: f.key.clone(),
                        value: f.value.clone().unwrap_or_default(),
                        field_type: FormFieldType::Text,
                        enabled: !f.disabled,
                        file_name: None,
                        file_data: None,
                        mime_type: None,
                    })
                    .collect();
                (String::new(), BodyType::FormData, fields, String::new())
            }
            Some("graphql") => {
                let gql = b.graphql.as_ref();
                let query = gql.and_then(|g| g.query.clone()).unwrap_or_default();
                let variables = match gql.and_then(|g| g.variables.as_ref()) {
                    // Postman stores variables as a string in its own exports
                    // and as an object in newer ones; both become editor text.
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(v @ serde_json::Value::Object(_)) => {
                        serde_json::to_string_pretty(v).unwrap_or_default()
                    }
                    _ => String::new(),
                };
                (query, BodyType::GraphQL, vec![], variables)
            }
            Some("formdata") => {
                let fields = b
                    .formdata
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|f| FormField {
                        id: uuid::Uuid::new_v4().to_string(),
                        key: f.key.clone(),
                        value: f.value.clone().unwrap_or_default(),
                        field_type: if f.field_type == "file" {
                            FormFieldType::File
                        } else {
                            FormFieldType::Text
                        },
                        enabled: !f.disabled,
                        file_name: None,
                        file_data: None,
                        mime_type: None,
                    })
                    .collect();
                (String::new(), BodyType::FormData, fields, String::new())
            }
            _ => (String::new(), BodyType::None, vec![], String::new()),
        },
        None => (String::new(), BodyType::None, vec![], String::new()),
    };

    SavedRequest {
        id: uuid::Uuid::new_v4().to_string(),
        collection_id: collection_id.to_owned(),
        group_id: None,
        sort_order: 0,
        name: item.name.clone(),
        method,
        url,
        headers,
        params,
        body,
        body_type,
        graphql_variables,
        auth_type: AuthType::None,
        bearer_token: String::new(),
        basic_user: String::new(),
        basic_pass: String::new(),
        api_key_name: String::new(),
        api_key_value: String::new(),
        api_key_location: ApiKeyLocation::Header,
        form_data_fields: form_fields,
        cookie_string: String::new(),
        cookies: vec![],
        jwt_secret: String::new(),
        jwt_subject: String::new(),
        jwt_algo: "HS256".to_owned(),
        pre_request_script: script_source(&item.event, "prerequest"),
        test_script: script_source(&item.event, "test"),
    }
}

// ── Export ────────────────────────────────────────────────────────────────────

pub fn export(imported: &ImportedCollection) -> String {
    use serde_json::{Value, json};

    let mut items: Vec<Value> = imported.groups.iter().map(export_group).collect();
    items.extend(imported.requests.iter().map(export_request));

    let col = json!({
        "info": {
            "_postman_id": imported.collection.id,
            "name": imported.collection.name,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
        },
        "item": items,
    });

    serde_json::to_string_pretty(&col).unwrap_or_default()
}

/// One folder (with its requests and subfolders) as a Postman `item`.
fn export_group(group: &ImportedGroup) -> serde_json::Value {
    use serde_json::json;

    let mut items: Vec<serde_json::Value> = group.groups.iter().map(export_group).collect();
    items.extend(group.requests.iter().map(export_request));
    json!({ "name": group.name, "item": items })
}

fn export_request(req: &SavedRequest) -> serde_json::Value {
    use serde_json::{Value, json};
    let headers: Vec<Value> = req
        .headers
        .iter()
        .map(|h| json!({ "key": h.key, "value": h.value, "disabled": !h.enabled }))
        .collect();

    let body: Value = match req.body_type {
        BodyType::Json => json!({
            "mode": "raw",
            "raw": req.body,
            "options": { "raw": { "language": "json" } },
        }),
        BodyType::Text => json!({ "mode": "raw", "raw": req.body }),
        BodyType::FormData => {
            let fields: Vec<Value> = req
                        .form_data_fields
                        .iter()
                        .map(|f| {
                            json!({
                                "key": f.key,
                                "value": f.value,
                                "type": match f.field_type { FormFieldType::File => "file", FormFieldType::Text => "text" },
                            })
                        })
                        .collect();
            json!({ "mode": "formdata", "formdata": fields })
        }
        BodyType::GraphQL => json!({
            "mode": "graphql",
            "graphql": {
                "query": req.body,
                "variables": req.graphql_variables,
            },
        }),
        BodyType::None => json!({ "mode": "none" }),
    };

    json!({
        "name": req.name,
        "request": {
            "method": req.method.as_str(),
            "header": headers,
            "url": { "raw": req.url },
            "body": body,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        include_str!("../../../samples/postman/rustman-sample.postman_collection.json");

    #[test]
    fn bundled_sample_imports_cleanly() {
        let cols = import(SAMPLE).expect("sample must parse");
        assert_eq!(cols.len(), 1);
        let col = &cols[0];
        assert_eq!(col.collection.name, "Rustman Sample (Postman)");

        // Top level: users, posts, forms, graphql + one loose request.
        let top: Vec<&str> = col.groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(top, ["users", "posts", "forms", "graphql"]);
        assert_eq!(col.requests.len(), 1);
        assert_eq!(
            col.requests[0].url,
            "https://jsonplaceholder.typicode.com/todos/1"
        );

        let posts = &col.groups[1];
        assert_eq!(posts.groups.len(), 1, "posts/comments is nested");
        assert_eq!(posts.groups[0].name, "comments");

        let all: Vec<&SavedRequest> = col
            .all_groups()
            .iter()
            .flat_map(|g| g.requests.iter())
            .chain(col.requests.iter())
            .collect();
        assert_eq!(all.len(), 12);
        let by = |n: &str| *all.iter().find(|r| r.name == n).expect(n);

        let search = by("Search users");
        assert_eq!(search.params.len(), 3);
        assert!(!search.params[2].enabled, "disabled query param stays off");

        let create = by("Create post");
        assert_eq!(create.body_type, BodyType::Json);
        assert!(create.body.contains("Hello from Rustman"));
        assert!(!create.headers[1].enabled, "disabled header stays off");

        let form = by("Form-data echo");
        assert_eq!(form.body_type, BodyType::FormData);
        assert_eq!(form.form_data_fields.len(), 3);

        let gql = by("Country by code");
        assert_eq!(gql.body_type, BodyType::GraphQL);
        assert!(gql.graphql_variables.contains("\"ID\""));
    }

    #[test]
    fn auth_is_inherited_and_overridable() {
        let json = r#"{
          "info": {"name": "A", "_postman_id": "p1"},
          "auth": {"type": "bearer", "bearer": [{"key": "token", "value": "{{token}}"}]},
          "variable": [{"key": "base_url", "value": "https://x.test"}, {"key": "off", "value": "1", "disabled": true}],
          "item": [
            {"name": "F", "auth": {"type": "basic", "basic": [{"key": "username", "value": "u"}, {"key": "password", "value": "p"}]},
             "item": [{"name": "In folder", "request": {"method": "GET", "url": "{{base_url}}/a"}}]},
            {"name": "Inherits", "request": {"method": "GET", "url": "{{base_url}}/b"}},
            {"name": "Opts out", "request": {"method": "GET", "url": "{{base_url}}/c", "auth": {"type": "noauth"}}},
            {"name": "Key", "request": {"method": "GET", "url": "{{base_url}}/d",
              "auth": {"type": "apikey", "apikey": [{"key": "key", "value": "X-Key"}, {"key": "value", "value": "k"}, {"key": "in", "value": "query"}]}},
             "event": [{"listen": "test", "script": {"exec": ["pm.test(\"ok\", () => {});"]}}]}
          ]
        }"#;
        let col = import(json).unwrap().remove(0);
        let all = col.all_requests();
        let by = |n: &str| *all.iter().find(|r| r.name == n).expect(n);
        assert_eq!(by("In folder").auth_type, AuthType::Basic);
        assert_eq!(by("In folder").basic_user, "u");
        assert_eq!(by("Inherits").auth_type, AuthType::Bearer);
        assert_eq!(by("Inherits").bearer_token, "{{token}}");
        assert_eq!(by("Opts out").auth_type, AuthType::None);
        let key = by("Key");
        assert_eq!(key.api_key_location, ApiKeyLocation::Query);
        assert!(key.test_script.starts_with(SCRIPT_MARKER));
        assert!(key.test_script.contains("// pm.test"));

        let env = collection_environment(json).expect("variables map to an environment");
        assert_eq!(env.id, "postman-collection-p1");
        assert_eq!(
            env.variables.get("base_url").map(String::as_str),
            Some("https://x.test")
        );
        assert!(!env.variables.contains_key("off"));
    }

    #[test]
    fn environment_files_and_dump_index() {
        let env = import_environment(
            r#"{"id": "e1", "name": "Staging", "values": [
              {"key": "host", "value": "s.test", "enabled": true, "type": "default"},
              {"key": "old", "value": "x", "enabled": false}]}"#,
        )
        .unwrap();
        assert_eq!(env.id, "postman-env-e1");
        assert_eq!(env.name, "Staging");
        assert_eq!(env.variables.len(), 1);
        assert!(is_dump_index(
            r#"{"collection": {"a": true}, "environment": {"b": true}}"#
        ));
        assert!(!is_dump_index(r#"{"info": {"name": "x"}, "item": []}"#));
    }

    #[test]
    fn raw_language_picks_text_or_json() {
        let json = r#"{"info": {"name": "R"}, "item": [
          {"name": "xml", "request": {"method": "POST", "url": "u", "body": {"mode": "raw", "raw": "<a/>", "options": {"raw": {"language": "xml"}}}}},
          {"name": "form", "request": {"method": "POST", "url": "u", "body": {"mode": "urlencoded", "urlencoded": [{"key": "a", "value": "1"}]}}}
        ]}"#;
        let col = import(json).unwrap().remove(0);
        assert_eq!(col.requests[0].body_type, BodyType::Text);
        assert_eq!(col.requests[1].body_type, BodyType::FormData);
        assert_eq!(col.requests[1].form_data_fields.len(), 1);
    }
}
