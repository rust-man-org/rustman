use serde_json::Value;

use crate::domain::{
    collection::{Collection, SavedRequest},
    request::{ApiKeyLocation, AuthType, BodyType, HttpMethod, KeyValue},
};

/// Try to parse `input` as an OpenAPI 3.x or Swagger 2.0 spec (JSON or YAML),
/// returning one collection per spec file.
pub fn import(input: &str) -> Result<Vec<(Collection, Vec<SavedRequest>)>, String> {
    let v: Value = serde_json::from_str(input)
        .or_else(|_| serde_yaml::from_str(input).map_err(|e| format!("Not JSON or YAML: {e}")))
        .map_err(|e| format!("Not a valid OpenAPI/Swagger file: {e}"))?;

    let obj = v.as_object().ok_or("spec must be a JSON object")?;

    if obj.contains_key("openapi") {
        import_openapi_v3(&v)
    } else if obj.contains_key("swagger") {
        import_swagger_v2(&v)
    } else {
        Err("not an OpenAPI or Swagger spec (missing 'openapi' or 'swagger' key)".into())
    }
}

// ── OpenAPI 3.x ───────────────────────────────────────────────────────────────

fn import_openapi_v3(v: &Value) -> Result<Vec<(Collection, Vec<SavedRequest>)>, String> {
    let info = v.get("info").and_then(|i| i.as_object()).ok_or("missing 'info'")?;
    let name = info
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("Imported API")
        .to_owned();

    let base_url = extract_base_url_oas3(v);
    let paths = v.get("paths").and_then(|p| p.as_object()).ok_or("missing 'paths'")?;

    let collection = Collection {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let col_id = collection.id.clone();

    let items = walk_paths(&col_id, &base_url, paths, extract_request_body_oas3);

    Ok(vec![(collection, items)])
}

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
    let rb = match op.get("requestBody").and_then(|b| b.as_object()) {
        Some(b) => b,
        None => return (String::new(), BodyType::None),
    };
    // Try JSON first, then text, then first available
    let content = match rb.get("content").and_then(|c| c.as_object()) {
        Some(c) => c,
        None => return (String::new(), BodyType::None),
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
    // Fallback: first content type
    if let Some((_, media)) = content.iter().next() {
        let example = build_example_from_schema(media.get("schema"));
        if !example.is_empty() {
            return (example, BodyType::Json);
        }
    }
    (String::new(), BodyType::None)
}

// ── Swagger 2.0 ───────────────────────────────────────────────────────────────

fn import_swagger_v2(v: &Value) -> Result<Vec<(Collection, Vec<SavedRequest>)>, String> {
    let info = v.get("info").and_then(|i| i.as_object()).ok_or("missing 'info'")?;
    let name = info
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("Imported API")
        .to_owned();

    let base_url = extract_base_url_swagger(v);
    let paths = v.get("paths").and_then(|p| p.as_object()).ok_or("missing 'paths'")?;

    let collection = Collection {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let col_id = collection.id.clone();

    let items = walk_paths(&col_id, &base_url, paths, extract_request_body_swagger);

    Ok(vec![(collection, items)])
}

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
        format!("{}://{}{}", scheme, host, base).trim_end_matches('/').to_owned()
    }
}

fn extract_request_body_swagger(op: &Value) -> (String, BodyType) {
    let params = match op.get("parameters").and_then(|p| p.as_array()) {
        Some(p) => p,
        None => return (String::new(), BodyType::None),
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

fn walk_paths(
    col_id: &str,
    base_url: &str,
    paths: &serde_json::Map<String, Value>,
    body_fn: impl Fn(&Value) -> (String, BodyType),
) -> Vec<SavedRequest> {
    let methods = ["get", "post", "put", "patch", "delete", "head", "options", "trace"];
    let mut out = Vec::new();

    for (path, path_item) in paths {
        let Some(po) = path_item.as_object() else { continue };
        for method_str in &methods {
            let Some(op) = po.get(*method_str) else { continue };
            let op_name = op
                .get("summary")
                .or_else(|| op.get("operationId"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_owned())
                .unwrap_or_else(|| format!("{} {}", method_str.to_uppercase(), path));

            let mut url = if base_url.is_empty() {
                path.clone()
            } else {
                format!("{}{}", base_url, path)
            };

            let (body, body_type) = body_fn(op);

            let raw_params = op.get("parameters").and_then(|p| p.as_array());
            let params = extract_kv_params(raw_params, "query");

            let query_string: Vec<String> = params
                .iter()
                .filter(|p| p.enabled)
                .map(|p| {
                    let k = urlencoding(&p.key);
                    let v = urlencoding(&p.value);
                    format!("{k}={v}")
                })
                .collect();
            if !query_string.is_empty() {
                let sep = if url.contains('?') { '&' } else { '?' };
                url.push(sep);
                url.push_str(&query_string.join("&"));
            }

            let headers = extract_headers(op);
            let method: HttpMethod = method_str.to_uppercase().parse().unwrap_or(HttpMethod::Get);

            out.push(SavedRequest {
                id: uuid::Uuid::new_v4().to_string(),
                collection_id: col_id.to_owned(),
                name: op_name,
                method,
                url,
                headers,
                params: vec![], // params already folded into URL above
                body,
                body_type,
                graphql_variables: String::new(),
                auth_type: AuthType::None,
                bearer_token: String::new(),
                basic_user: String::new(),
                basic_pass: String::new(),
                api_key_name: String::new(),
                api_key_value: String::new(),
                api_key_location: ApiKeyLocation::Header,
                form_data_fields: vec![],
                cookie_string: String::new(),
                cookies: vec![],
                jwt_secret: String::new(),
                jwt_subject: String::new(),
                jwt_algo: "HS256".to_owned(),
                pre_request_script: String::new(),
                test_script: String::new(),
            });
        }
    }

    out
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
            let value = example.and_then(|e| value_to_string(e)).unwrap_or_default();
            let required = p.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
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
            let value = example.and_then(|e| value_to_string(e)).unwrap_or_default();
            let required = p.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
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
    let Some(s) = schema else { return String::new() };
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
        Some("string") => serde_json::Value::String(s
            .get("example")
            .and_then(|e| e.as_str())
            .unwrap_or("string")
            .to_owned()),
        Some("number") | Some("integer") => s
            .get("example")
            .cloned()
            .unwrap_or(serde_json::Value::Number(serde_json::Number::from(0))),
        Some("boolean") => s
            .get("example")
            .and_then(|e| e.as_bool())
            .map(serde_json::Value::Bool)
            .unwrap_or(serde_json::Value::Bool(false)),
        _ => {
            // Try example/default directly
            s.get("example")
                .or_else(|| s.get("default"))
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        }
    };
    serde_json::to_string_pretty(&example).unwrap_or_default()
}

fn schema_to_example(s: &Value) -> Value {
    // Recurse if s has $ref — skip complex refs, just use "string"
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
            let item = s.get("items").map(|i| schema_to_example(i));
            Value::Array(item.into_iter().collect())
        }
        Some("string") => Value::String(
            s.get("example")
                .and_then(|e| e.as_str())
                .unwrap_or("string")
                .to_owned(),
        ),
        Some("number") | Some("integer") => s
            .get("example")
            .cloned()
            .unwrap_or(Value::Number(serde_json::Number::from(0))),
        Some("boolean") => s
            .get("example")
            .and_then(|e| e.as_bool())
            .map(Value::Bool)
            .unwrap_or(Value::Bool(false)),
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

fn urlencoding(s: &str) -> String {
    // Minimal percent-encoding for query values
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_petstore_oas3() {
        let spec = include_str!("../../../tests/fixtures/petstore-oas3.json");
        let result = import(spec);
        assert!(result.is_ok(), "OAS3 Petstore failed: {:?}", result.err());
        let data = result.unwrap();
        assert_eq!(data[0].0.name, "Swagger Petstore");
        assert!(data[0].1.len() >= 2, "expected >= 2 requests, got {}", data[0].1.len());
        let get_pets = &data[0].1[0];
        assert_eq!(get_pets.method.as_str(), "GET");
        assert!(get_pets.url.contains("petstore.swagger.io"));
        assert!(get_pets.url.contains("/pets"));
    }

    #[test]
    fn real_openapi_generator_sample() {
        let spec = include_str!("../../../tests/fixtures/storefront-oas3.json");
        let result = import(spec);
        assert!(result.is_ok(), "Storefront OAS3 failed: {:?}", result.err());
        let data = result.unwrap();
        assert_eq!(data[0].0.name, "E-commerce API");
        assert!(data[0].1.len() >= 3, "expected >= 3 endpoints, got {}", data[0].1.len());
    }

    #[test]
    fn real_petstore_swagger2() {
        let spec = include_str!("../../../tests/fixtures/petstore-swagger2.json");
        let result = import(spec);
        assert!(result.is_ok(), "Swagger 2 Petstore failed: {:?}", result.err());
        let data = result.unwrap();
        assert_eq!(data[0].0.name, "Swagger Petstore");
        assert!(data[0].1.len() > 10, "expected many endpoints, got {}", data[0].1.len());
        let has_full_url = data[0].1.iter().any(|r| r.url.contains("petstore.swagger.io"));
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
        let (col, reqs) = &data[0];
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
        let (col, reqs) = &data[0];
        assert_eq!(col.name, "User API");
        assert_eq!(reqs.len(), 1);
        assert!(reqs[0].url.starts_with("https://api.example.com/v1/users"));
    }

    #[test]
    fn rejects_non_spec() {
        let err = import("{\"foo\": \"bar\"}").unwrap_err();
        assert!(err.contains("openapi") || err.contains("swagger"));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = import("not json").unwrap_err();
        assert!(err.contains("JSON") || err.contains("YAML"));
    }

    #[test]
    fn yaml_input_detected() {
        let yaml = "openapi: 3.0.0\ninfo:\n  title: YAML Test\npaths: {}\n";
        let result = import(yaml);
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data[0].0.name, "YAML Test");
    }
}
