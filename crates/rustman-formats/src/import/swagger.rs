use serde_json::Value;

use rustman_core::domain::{
    collection::{Collection, SavedRequest},
    group::ImportedCollection,
    request::{BodyType, HttpMethod, KeyValue},
};
use rustman_core::url::percent_encode;

use super::{ImportOutcome, yaml};

type BodyOf = fn(&Value) -> (String, BodyType);

/// An OpenAPI 3.x or Swagger 2.0 spec (JSON or YAML) as one collection with
/// a request per operation.
pub fn import(input: &str) -> Result<ImportOutcome, String> {
    let v = parse_spec(input).map_err(|e| format!("Not a valid OpenAPI/Swagger file: {e}"))?;

    let obj = v.as_object().ok_or("spec must be a JSON object")?;
    let (base_url, body_of): (String, BodyOf) = if obj.contains_key("openapi") {
        (extract_base_url_oas3(&v), extract_request_body_oas3)
    } else if obj.contains_key("swagger") {
        (extract_base_url_swagger(&v), extract_request_body_swagger)
    } else {
        return Err("not an OpenAPI or Swagger spec (missing 'openapi' or 'swagger' key)".into());
    };

    let info = v
        .get("info")
        .and_then(Value::as_object)
        .ok_or("missing 'info'")?;
    let name = info
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Imported API")
        .to_owned();
    let paths = v
        .get("paths")
        .and_then(Value::as_object)
        .ok_or("missing 'paths'")?;

    let collection = Collection::new(name);
    let mut warnings = Vec::new();
    let requests = walk_paths(&collection.id, &base_url, paths, body_of, &mut warnings);
    Ok(ImportOutcome {
        collections: vec![ImportedCollection {
            collection,
            requests,
            groups: Vec::new(),
        }],
        warnings,
    })
}

/// JSON, or else YAML. A file that opens like JSON is held to JSON, so its
/// error says what is wrong with it as JSON.
fn parse_spec(input: &str) -> Result<Value, String> {
    match serde_json::from_str(input) {
        Ok(v) => Ok(v),
        Err(e) if input.trim_start().starts_with(['{', '[']) => Err(format!("Not valid JSON: {e}")),
        Err(_) => yaml::to_json(input).map_err(|e| format!("Not JSON or YAML: {e}")),
    }
}

// ── OpenAPI 3.x ───────────────────────────────────────────────────────────────

fn extract_base_url_oas3(v: &Value) -> String {
    v.get("servers")
        .and_then(|s| s.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .map(|u| u.trim_end_matches('/').to_owned())
        .unwrap_or_default()
}

fn extract_request_body_oas3(op: &Value) -> (String, BodyType) {
    let Some(content) = op
        .get("requestBody")
        .and_then(|b| b.get("content"))
        .and_then(Value::as_object)
    else {
        return (String::new(), BodyType::None);
    };
    let preferred = ["application/json", "*/*", "text/plain"];
    for mt in &preferred {
        if let Some(media) = content.get(*mt) {
            let example = build_example_from_schema(media.get("schema"));
            if !example.is_empty() {
                let bt = if *mt == "application/json" || mt.ends_with("+json") {
                    BodyType::Json
                } else {
                    BodyType::Text
                };
                return (example, bt);
            }
        }
    }
    if let Some((_, media)) = content.iter().next() {
        let example = build_example_from_schema(media.get("schema"));
        if !example.is_empty() {
            return (example, BodyType::Json);
        }
    }
    (String::new(), BodyType::None)
}

// ── Swagger 2.0 ───────────────────────────────────────────────────────────────

fn extract_base_url_swagger(v: &Value) -> String {
    let scheme = v
        .get("schemes")
        .and_then(|s| s.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.as_str())
        .unwrap_or("https");
    let host = v.get("host").and_then(|h| h.as_str()).unwrap_or("");
    let base = v.get("basePath").and_then(|b| b.as_str()).unwrap_or("");
    if host.is_empty() {
        String::new()
    } else {
        format!("{scheme}://{host}{base}")
            .trim_end_matches('/')
            .to_owned()
    }
}

fn extract_request_body_swagger(op: &Value) -> (String, BodyType) {
    let Some(params) = op.get("parameters").and_then(Value::as_array) else {
        return (String::new(), BodyType::None);
    };
    for p in params {
        if p.get("in").and_then(|i| i.as_str()) == Some("body") {
            let schema = p.get("schema");
            let example = build_example_from_schema(schema);
            if !example.is_empty() {
                return (example, BodyType::Json);
            }
        }
    }
    (String::new(), BodyType::None)
}

// ── Shared ────────────────────────────────────────────────────────────────────

/// One request per operation, the query folded into the URL.
fn walk_paths(
    col_id: &str,
    base_url: &str,
    paths: &serde_json::Map<String, Value>,
    body_of: BodyOf,
    warnings: &mut Vec<String>,
) -> Vec<SavedRequest> {
    const METHODS: [&str; 8] = [
        "get", "post", "put", "patch", "delete", "head", "options", "trace",
    ];
    let mut out = Vec::new();
    for (path, path_item) in paths {
        let Some(po) = path_item.as_object() else {
            continue;
        };
        for method_str in METHODS {
            let Some(op) = po.get(method_str) else {
                continue;
            };
            let label = format!("{} {path}", method_str.to_uppercase());
            let name = op
                .get("summary")
                .or_else(|| op.get("operationId"))
                .and_then(Value::as_str)
                .map_or_else(|| label.clone(), str::to_owned);
            let mut req = SavedRequest::new_in(col_id.to_owned(), name);
            req.method = method_str.parse().unwrap_or_else(|_| {
                warnings.push(format!(
                    "{label} uses a method Rustman can't send, so it was imported as GET"
                ));
                HttpMethod::Get
            });
            req.url = operation_url(base_url, path, op);
            req.headers = extract_headers(op);
            (req.body, req.body_type) = body_of(op);
            out.push(req);
        }
    }
    out
}

fn operation_url(base_url: &str, path: &str, op: &Value) -> String {
    let mut url = format!("{base_url}{path}");
    let raw_params = op.get("parameters").and_then(Value::as_array);
    let query: Vec<String> = extract_kv_params(raw_params, "query")
        .iter()
        .filter(|p| p.enabled)
        .map(|p| format!("{}={}", percent_encode(&p.key), percent_encode(&p.value)))
        .collect();
    if !query.is_empty() {
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&query.join("&"));
    }
    url
}

fn extract_kv_params(raw: Option<&Vec<Value>>, in_type: &str) -> Vec<KeyValue> {
    let Some(arr) = raw else { return vec![] };
    arr.iter()
        .filter(|p| p.get("in").and_then(|i| i.as_str()) == Some(in_type))
        .map(|p| {
            let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let example = p
                .get("example")
                .or_else(|| p.get("schema").and_then(|s| s.get("default")))
                .or_else(|| p.get("schema").and_then(|s| s.get("example")));
            let value = example.and_then(value_to_string).unwrap_or_default();
            let required = p
                .get("required")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            KeyValue {
                id: uuid::Uuid::new_v4().to_string(),
                key: name.to_owned(),
                value,
                enabled: required,
            }
        })
        .collect()
}

fn extract_headers(op: &Value) -> Vec<KeyValue> {
    let raw = op.get("parameters").and_then(|p| p.as_array());
    let Some(arr) = raw else { return vec![] };
    arr.iter()
        .filter(|p| p.get("in").and_then(|i| i.as_str()) == Some("header"))
        .map(|p| {
            let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let example = p
                .get("example")
                .or_else(|| p.get("schema").and_then(|s| s.get("default")));
            let value = example.and_then(value_to_string).unwrap_or_default();
            let required = p
                .get("required")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            KeyValue {
                id: uuid::Uuid::new_v4().to_string(),
                key: name.to_owned(),
                value,
                enabled: required,
            }
        })
        .collect()
}

/// Build a minimal JSON example body from a schema object.
fn build_example_from_schema(schema: Option<&Value>) -> String {
    let Some(s) = schema else {
        return String::new();
    };
    let example = match s.get("type").and_then(|t| t.as_str()) {
        Some("object") => {
            let mut map = serde_json::Map::new();
            if let Some(props) = s.get("properties").and_then(|p| p.as_object()) {
                for (k, v) in props {
                    let val = schema_to_example(v);
                    map.insert(k.clone(), val);
                }
            }
            serde_json::Value::Object(map)
        }
        Some("array") => {
            let items = s.get("items");
            let item = items.and_then(|i| {
                let ex = build_example_from_schema(Some(i));
                if ex.is_empty() {
                    None
                } else {
                    serde_json::from_str(&ex).ok()
                }
            });
            serde_json::Value::Array(item.into_iter().collect())
        }
        Some("string") => serde_json::Value::String(
            s.get("example")
                .and_then(|e| e.as_str())
                .unwrap_or("string")
                .to_owned(),
        ),
        Some("number" | "integer") => s
            .get("example")
            .cloned()
            .unwrap_or(serde_json::Value::Number(serde_json::Number::from(0))),
        Some("boolean") => s
            .get("example")
            .and_then(serde_json::Value::as_bool)
            .map_or(serde_json::Value::Bool(false), serde_json::Value::Bool),
        _ => s
            .get("example")
            .or_else(|| s.get("default"))
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    };
    serde_json::to_string_pretty(&example).expect("a JSON value always serializes")
}

fn schema_to_example(s: &Value) -> Value {
    // References aren't resolved; a placeholder keeps the example valid.
    if s.get("$ref").is_some() {
        return Value::String("string".to_owned());
    }
    match s.get("type").and_then(|t| t.as_str()) {
        Some("object") => {
            let mut map = serde_json::Map::new();
            if let Some(props) = s.get("properties").and_then(|p| p.as_object()) {
                for (k, v) in props {
                    map.insert(k.clone(), schema_to_example(v));
                }
            }
            Value::Object(map)
        }
        Some("array") => {
            let item = s.get("items").map(schema_to_example);
            Value::Array(item.into_iter().collect())
        }
        Some("string") => Value::String(
            s.get("example")
                .and_then(|e| e.as_str())
                .unwrap_or("string")
                .to_owned(),
        ),
        Some("number" | "integer") => s
            .get("example")
            .cloned()
            .unwrap_or(Value::Number(serde_json::Number::from(0))),
        Some("boolean") => s
            .get("example")
            .and_then(serde_json::Value::as_bool)
            .map_or(Value::Bool(false), Value::Bool),
        _ => s
            .get("example")
            .or_else(|| s.get("default"))
            .cloned()
            .unwrap_or(Value::Null),
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some(String::new()),
        _ => Some(v.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn import(input: &str) -> Result<Vec<ImportedCollection>, String> {
        super::import(input).map(|outcome| outcome.collections)
    }

    #[test]
    fn unsupported_methods_import_as_get_with_a_warning() {
        let spec = r#"{"openapi": "3.0.0", "info": {"title": "T"},
            "paths": {"/debug": {"trace": {"summary": "Trace it"}}}}"#;
        let outcome = super::import(spec).unwrap();
        assert_eq!(outcome.collections[0].requests[0].method, HttpMethod::Get);
        assert_eq!(
            outcome.warnings,
            ["TRACE /debug uses a method Rustman can't send, so it was imported as GET"]
        );
    }

    #[test]
    fn real_petstore_oas3() {
        let spec = include_str!("../../tests/fixtures/petstore-oas3.json");
        let result = import(spec);
        assert!(result.is_ok(), "OAS3 Petstore failed: {:?}", result.err());
        let data = result.unwrap();
        assert_eq!(data[0].collection.name, "Swagger Petstore");
        assert!(
            data[0].requests.len() >= 2,
            "expected >= 2 requests, got {}",
            data[0].requests.len()
        );
        let get_pets = &data[0].requests[0];
        assert_eq!(get_pets.method.as_str(), "GET");
        assert!(get_pets.url.contains("petstore.swagger.io"));
        assert!(get_pets.url.contains("/pets"));
    }

    #[test]
    fn real_openapi_generator_sample() {
        let spec = include_str!("../../tests/fixtures/storefront-oas3.json");
        let result = import(spec);
        assert!(result.is_ok(), "Storefront OAS3 failed: {:?}", result.err());
        let data = result.unwrap();
        assert_eq!(data[0].collection.name, "E-commerce API");
        assert!(
            data[0].requests.len() >= 3,
            "expected >= 3 endpoints, got {}",
            data[0].requests.len()
        );
    }

    #[test]
    fn real_petstore_swagger2() {
        let spec = include_str!("../../tests/fixtures/petstore-swagger2.json");
        let result = import(spec);
        assert!(
            result.is_ok(),
            "Swagger 2 Petstore failed: {:?}",
            result.err()
        );
        let data = result.unwrap();
        assert_eq!(data[0].collection.name, "Swagger Petstore");
        assert!(
            data[0].requests.len() > 10,
            "expected many endpoints, got {}",
            data[0].requests.len()
        );
        let has_full_url = data[0]
            .requests
            .iter()
            .any(|r| r.url.contains("petstore.swagger.io"));
        assert!(has_full_url, "expected full URL with host");
    }

    #[test]
    fn openapi_v3_simple() {
        let yaml = r#"
openapi: "3.0.0"
info:
  title: Pet Store
  version: "1.0"
paths:
  /pets:
    get:
      summary: List all pets
      parameters:
        - name: limit
          in: query
          schema:
            type: integer
      responses:
        "200":
          description: OK
    post:
      summary: Create a pet
      requestBody:
        content:
          application/json:
            schema:
              type: object
              properties:
                name:
                  type: string
                species:
                  type: string
      responses:
        "201":
          description: Created
  /pets/{id}:
    get:
      summary: Get pet by ID
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: OK
"#;
        let result = import(yaml);
        assert!(result.is_ok(), "import failed: {:?}", result.err());
        let data = result.unwrap();
        assert_eq!(data.len(), 1);
        let ic = &data[0];
        let col = &ic.collection;
        let reqs = &ic.requests;
        assert_eq!(col.name, "Pet Store");
        assert_eq!(reqs.len(), 3);
        // GET /pets
        assert_eq!(reqs[0].method.as_str(), "GET");
        assert!(reqs[0].url.contains("/pets"));
        // POST /pets has JSON body
        assert_eq!(reqs[1].method.as_str(), "POST");
        assert_eq!(reqs[1].body_type, BodyType::Json);
        assert!(reqs[1].body.contains("name"));
        // GET /pets/{id}
        assert_eq!(reqs[2].method.as_str(), "GET");
        assert!(reqs[2].url.contains("/pets/%7Bid%7D") || reqs[2].url.contains("/pets/{id}"));
    }

    #[test]
    fn swagger_v2_simple() {
        let json = r#"{
  "swagger": "2.0",
  "info": { "title": "User API", "version": "1.0" },
  "host": "api.example.com",
  "basePath": "/v1",
  "paths": {
    "/users": {
      "get": {
        "summary": "List users",
        "parameters": [
          { "name": "page", "in": "query", "type": "integer", "required": false }
        ],
        "responses": { "200": { "description": "OK" } }
      }
    }
  }
}"#;
        let result = import(json);
        assert!(result.is_ok(), "import failed: {:?}", result.err());
        let data = result.unwrap();
        assert_eq!(data.len(), 1);
        let ic = &data[0];
        let col = &ic.collection;
        let reqs = &ic.requests;
        assert_eq!(col.name, "User API");
        assert_eq!(reqs.len(), 1);
        assert!(reqs[0].url.starts_with("https://api.example.com/v1/users"));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = import("not json").unwrap_err();
        assert!(err.contains("JSON") || err.contains("YAML"));
    }
}
