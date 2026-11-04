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
}

#[derive(Deserialize)]
struct PostmanInfo {
    name: String,
}

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
}

#[derive(Deserialize)]
struct PostmanRequestItem {
    name: String,
    request: PostmanRequest,
}

#[derive(Deserialize)]
struct PostmanRequest {
    method: Option<String>,
    url: Option<PostmanUrl>,
    header: Option<Vec<PostmanHeader>>,
    body: Option<PostmanBody>,
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
    graphql: Option<PostmanGraphQl>,
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
    let (requests, groups) = convert_items(&collection.id, &collection.id, true, 0, &col.item);
    Ok(vec![ImportedCollection {
        collection,
        requests,
        groups,
    }])
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
) -> (Vec<SavedRequest>, Vec<crate::domain::group::ImportedGroup>) {
    let mut requests = Vec::new();
    let mut groups = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let order = sort_order + i as i64;
        match item {
            PostmanItem::Request(ri) => {
                let mut req = convert_request(collection_id, ri);
                req.sort_order = order;
                requests.push(req);
            }
            PostmanItem::Folder(folder) => {
                let group_id = uuid::Uuid::new_v4().to_string();
                let (child_reqs, child_groups) =
                    convert_items(collection_id, &group_id, false, 0, &folder.item);
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
            Some("raw") => (
                b.raw.clone().unwrap_or_default(),
                BodyType::Json,
                vec![],
                String::new(),
            ),
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
        pre_request_script: String::new(),
        test_script: String::new(),
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
        assert_eq!(col.requests[0].url, "https://jsonplaceholder.typicode.com/todos/1");

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
}
