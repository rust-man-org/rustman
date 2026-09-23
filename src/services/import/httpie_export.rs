use std::str::FromStr;

use serde::Deserialize;

use crate::domain::{
    collection::{Collection, SavedRequest},
    request::{BodyType, HttpMethod, KeyValue},
};

#[derive(Deserialize)]
struct HttpieExport {
    name: Option<String>,
    requests: Vec<HttpieRequest>,
}

#[derive(Deserialize)]
struct HttpieRequest {
    name: Option<String>,
    method: Option<String>,
    url: Option<String>,
    headers: Option<std::collections::HashMap<String, String>>,
    body: Option<String>,
    #[serde(rename = "body_type")]
    body_type: Option<String>,
}

pub fn import(json: &str) -> Result<Vec<(Collection, Vec<SavedRequest>)>, String> {
    let export: HttpieExport =
        serde_json::from_str(json).map_err(|e| format!("Not a valid HTTPie export: {e}"))?;

    let col_name = export.name.unwrap_or_else(|| "Imported from HTTPie".to_owned());

    let mut requests = Vec::new();

    for req in export.requests {
        let name = req.name.unwrap_or_else(|| "Untitled".to_owned());
        let method = req
            .method
            .and_then(|m| HttpMethod::from_str(&m).ok())
            .unwrap_or(HttpMethod::Get);
        let url = req.url.unwrap_or_default();

        let mut headers = Vec::new();
        if let Some(h) = req.headers {
            for (key, value) in h {
                let disabled = key.starts_with('!');
                let clean_key = key.trim_start_matches('!');
                headers.push(KeyValue { id: uuid::Uuid::new_v4().to_string(), key: clean_key.to_owned(), value, enabled: !disabled });
            }
        }

        let body = req.body.unwrap_or_default();

        let body_type = match req.body_type.as_deref() {
            Some("json") => BodyType::Json,
            Some("graphql") => BodyType::GraphQL,
            Some("text") | Some("plain") => BodyType::Text,
            Some("form") | Some("form-data") => BodyType::FormData,
            _ => {
                let trimmed = body.trim();
                if trimmed.starts_with('{') || trimmed.starts_with('[') {
                    BodyType::Json
                } else if !body.is_empty() {
                    BodyType::Text
                } else {
                    BodyType::None
                }
            }
        };

        requests.push(SavedRequest::new_in(String::new(), name));
        let req = requests.last_mut().unwrap();
        req.method = method;
        req.url = url;
        req.headers = headers;
        req.body = body;
        req.body_type = body_type;
    }

    if requests.is_empty() {
        return Err("No requests found in HTTPie export".to_owned());
    }

    let col_id = uuid::Uuid::new_v4().to_string();
    let collection = Collection {
        id: col_id.clone(),
        name: col_name,
        created_at: chrono::Utc::now().timestamp(),
    };

    for req in &mut requests {
        req.collection_id = col_id.clone();
    }

    Ok(vec![(collection, requests)])
}
