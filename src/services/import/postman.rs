use serde::Deserialize;

use crate::domain::{
    collection::{Collection, SavedRequest},
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

pub fn import(json: &str) -> Result<Vec<(Collection, Vec<SavedRequest>)>, String> {
    let col: PostmanCollection =
        serde_json::from_str(json).map_err(|e| format!("Invalid Postman JSON: {e}"))?;

    let collection = Collection {
        id: uuid::Uuid::new_v4().to_string(),
        name: col.info.name,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    let reqs = flatten_items(&collection.id, &col.item);
    Ok(vec![(collection, reqs)])
}

fn flatten_items(collection_id: &str, items: &[PostmanItem]) -> Vec<SavedRequest> {
    let mut out = Vec::new();
    for item in items {
        match item {
            PostmanItem::Request(ri) => out.push(convert_request(collection_id, ri)),
            PostmanItem::Folder(folder) => {
                out.extend(flatten_items(collection_id, &folder.item))
            }
        }
    }
    out
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
            Some("raw") => (b.raw.clone().unwrap_or_default(), BodyType::Json, vec![], String::new()),
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

pub fn export(collection: &Collection, requests: &[SavedRequest]) -> String {
    use serde_json::{json, Value};

    let items: Vec<Value> = requests
        .iter()
        .map(|req| {
            let headers: Vec<Value> = req
                .headers
                .iter()
                .map(|h| {
                    json!({ "key": h.key, "value": h.value, "disabled": !h.enabled })
                })
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
        })
        .collect();

    let col = json!({
        "info": {
            "_postman_id": collection.id,
            "name": collection.name,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
        },
        "item": items,
    });

    serde_json::to_string_pretty(&col).unwrap_or_default()
}
