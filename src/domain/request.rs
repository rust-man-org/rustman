use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }

    pub fn all() -> &'static [HttpMethod] {
        &[
            Self::Get,
            Self::Post,
            Self::Put,
            Self::Patch,
            Self::Delete,
            Self::Head,
            Self::Options,
        ]
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for HttpMethod {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            other => Err(format!("Unknown HTTP method: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyType {
    None,
    Json,
    Text,
    FormData,
    GraphQL,
}

impl BodyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Json => "json",
            Self::Text => "text",
            Self::FormData => "form-data",
            Self::GraphQL => "graphql",
        }
    }
}

impl std::str::FromStr for BodyType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(Self::Json),
            "text" => Ok(Self::Text),
            "form-data" => Ok(Self::FormData),
            "graphql" => Ok(Self::GraphQL),
            _ => Ok(Self::None),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthType {
    None,
    Basic,
    Bearer,
    ApiKey,
    JwtUser,
    Cookie,
}

impl AuthType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Basic => "basic",
            Self::Bearer => "bearer",
            Self::ApiKey => "apikey",
            Self::JwtUser => "jwt-user",
            Self::Cookie => "cookie",
        }
    }
}

impl std::str::FromStr for AuthType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "basic" => Ok(Self::Basic),
            "bearer" => Ok(Self::Bearer),
            "apikey" => Ok(Self::ApiKey),
            "jwt-user" => Ok(Self::JwtUser),
            "cookie" => Ok(Self::Cookie),
            _ => Ok(Self::None),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyLocation {
    Header,
    Query,
}

impl ApiKeyLocation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Query => "query",
        }
    }
}

impl std::str::FromStr for ApiKeyLocation {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "query" { Ok(Self::Query) } else { Ok(Self::Header) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    pub id: String,
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

impl KeyValue {
    pub fn new_empty() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            key: String::new(),
            value: String::new(),
            enabled: true,
        }
    }
}

// ── URL query string ⇄ params table ───────────────────────────────────────────
//
// The URL bar holds the full URL (query included) and the params table is a
// two-way-synced view of its query. Enabled rows are owned by the URL; disabled
// rows live only in the table.

/// Decode `%xx` and `+` sequences in a URL query component.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                match (
                    (bytes[i + 1] as char).to_digit(16),
                    (bytes[i + 2] as char).to_digit(16),
                ) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi * 16 + lo) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Encode only the characters that would otherwise break query parsing or fail to
/// round-trip through [`percent_decode`]; everything else stays readable.
fn encode_query_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '&' => out.push_str("%26"),
            '=' => out.push_str("%3D"),
            '#' => out.push_str("%23"),
            '%' => out.push_str("%25"),
            '+' => out.push_str("%2B"),
            _ => out.push(c),
        }
    }
    out
}

/// Derive the params table from a URL's query string. Enabled rows come from the
/// query; existing *disabled* rows are preserved (they live only in the table).
pub fn sync_params_from_url(url: &str, existing: &[KeyValue]) -> Vec<KeyValue> {
    let qs = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut params: Vec<KeyValue> = qs
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (k, val) = pair.split_once('=').unwrap_or((pair, ""));
            KeyValue {
                id: uuid::Uuid::new_v4().to_string(),
                key: percent_decode(k),
                value: percent_decode(val),
                enabled: true,
            }
        })
        .collect();
    let keys: std::collections::HashSet<String> = params.iter().map(|p| p.key.clone()).collect();
    for p in existing {
        if !p.enabled && !p.key.is_empty() && !keys.contains(&p.key) {
            params.push(p.clone());
        }
    }
    params
}

/// Rebuild a URL's query string from the enabled params, keeping the base intact.
pub fn sync_url_from_params(url: &str, params: &[KeyValue]) -> String {
    let base = url.split_once('?').map(|(b, _)| b).unwrap_or(url);
    let qs: Vec<String> = params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .map(|p| {
            if p.value.is_empty() {
                encode_query_component(&p.key)
            } else {
                format!(
                    "{}={}",
                    encode_query_component(&p.key),
                    encode_query_component(&p.value)
                )
            }
        })
        .collect();
    if qs.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", qs.join("&"))
    }
}

/// Make a loaded URL and its stored params mutually consistent: union the URL's
/// query with stored rows (deduped by key, keeping rows that live only in the
/// table) so typing in the URL bar later can't drop saved params.
pub fn reconcile_url_params(url: &str, stored: &[KeyValue]) -> (String, Vec<KeyValue>) {
    let mut params = sync_params_from_url(url, &[]);
    let keys: std::collections::HashSet<String> = params.iter().map(|p| p.key.clone()).collect();
    for p in stored {
        if !p.key.is_empty() && !keys.contains(&p.key) {
            params.push(p.clone());
        }
    }
    let new_url = sync_url_from_params(url, &params);
    (new_url, params)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub id: String,
    pub key: String,
    pub value: String,
    pub field_type: FormFieldType,
    pub enabled: bool,
    pub file_name: Option<String>,
    pub file_data: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormFieldType {
    Text,
    File,
}

/// Compile a GraphQL request into the body every GraphQL server accepts:
/// `{"query": "...", "variables": {...}}`.
///
/// `variables` is omitted when blank, and rejected — rather than sent as a
/// string — when it isn't a JSON object, since a server expecting a variables
/// map fails the whole operation on a stringified one.
pub fn graphql_body(query: &str, variables: &str) -> Result<String, String> {
    let mut body = serde_json::Map::new();
    body.insert("query".to_owned(), serde_json::Value::String(query.to_owned()));

    let variables = variables.trim();
    if !variables.is_empty() {
        let parsed: serde_json::Value = serde_json::from_str(variables)
            .map_err(|e| format!("GraphQL variables are not valid JSON: {e}"))?;
        if !parsed.is_object() {
            return Err(
                "GraphQL variables must be a JSON object, e.g. {\"id\": 1}".to_owned(),
            );
        }
        body.insert("variables".to_owned(), parsed);
    }

    serde_json::to_string(&serde_json::Value::Object(body))
        .map_err(|e| format!("GraphQL body: {e}"))
}

#[cfg(test)]
mod graphql_tests {
    use super::*;

    #[test]
    fn query_only_omits_variables() {
        assert_eq!(
            graphql_body("query { me { id } }", "").unwrap(),
            r#"{"query":"query { me { id } }"}"#
        );
        // Whitespace-only variables are still "no variables".
        assert_eq!(
            graphql_body("{ me }", "  \n ").unwrap(),
            r#"{"query":"{ me }"}"#
        );
    }

    #[test]
    fn variables_are_embedded_as_json_not_a_string() {
        let body = graphql_body("query($id: ID!) { user(id: $id) { name } }", r#"{"id": "42"}"#)
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(parsed["variables"].is_object());
        assert_eq!(parsed["variables"]["id"], "42");
    }

    #[test]
    fn invalid_variables_json_is_an_error() {
        let err = graphql_body("{ me }", "{oops").unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
        assert!(graphql_body("{ me }", "[1, 2]").is_err());
    }
}

#[cfg(test)]
mod query_sync_tests {
    use super::*;

    fn kv(key: &str, value: &str, enabled: bool) -> KeyValue {
        KeyValue { id: "x".into(), key: key.into(), value: value.into(), enabled }
    }

    #[test]
    fn params_derived_from_query() {
        let p = sync_params_from_url("https://x.com/a?one=1&two=2", &[]);
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].key.as_str(), p[0].value.as_str()), ("one", "1"));
        assert_eq!((p[1].key.as_str(), p[1].value.as_str()), ("two", "2"));
        assert!(p.iter().all(|p| p.enabled));
    }

    #[test]
    fn no_query_yields_empty() {
        assert!(sync_params_from_url("https://x.com/a", &[]).is_empty());
    }

    #[test]
    fn disabled_existing_preserved_enabled_dropped() {
        // Disabled rows live only in the table and survive a URL re-derive; an
        // enabled row absent from the query was deleted from the URL, so it goes.
        let existing = [kv("keep", "v", false), kv("gone", "v", true)];
        let p = sync_params_from_url("https://x.com/a?one=1", &existing);
        let keys: Vec<&str> = p.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, ["one", "keep"]);
    }

    #[test]
    fn rebuild_url_skips_disabled_and_empty() {
        let params = [kv("a", "1", true), kv("b", "2", false), kv("", "x", true)];
        assert_eq!(sync_url_from_params("https://x.com/p?old=1", &params), "https://x.com/p?a=1");
    }

    #[test]
    fn rebuild_encodes_specials() {
        let params = [kv("q", "a b&c", true)];
        assert_eq!(sync_url_from_params("https://x.com", &params), "https://x.com?q=a%20b%26c");
    }

    #[test]
    fn url_param_round_trip() {
        let url = "https://x.com/p?name=john%20doe&tag=a%26b";
        let params = sync_params_from_url(url, &[]);
        assert_eq!(params[0].value, "john doe");
        assert_eq!(params[1].value, "a&b");
        // params → url → params is stable
        let rebuilt = sync_url_from_params(url, &params);
        assert_eq!(sync_params_from_url(&rebuilt, &[]).iter().map(|p| (p.key.clone(), p.value.clone())).collect::<Vec<_>>(),
                   params.iter().map(|p| (p.key.clone(), p.value.clone())).collect::<Vec<_>>());
    }

    #[test]
    fn reconcile_folds_legacy_base_url_and_params() {
        // Old saved shape: base URL, query held only in the params table.
        let (url, params) = reconcile_url_params("https://x.com/a", &[kv("one", "1", true)]);
        assert_eq!(url, "https://x.com/a?one=1");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn reconcile_keeps_url_only_query() {
        let (url, params) = reconcile_url_params("https://x.com/a?one=1", &[]);
        assert_eq!(url, "https://x.com/a?one=1");
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn reconcile_is_idempotent_and_keeps_disabled() {
        let stored = [kv("one", "1", true), kv("two", "2", false)];
        let (url, params) = reconcile_url_params("https://x.com/a?one=1", &stored);
        assert_eq!(url, "https://x.com/a?one=1");
        assert_eq!(params.len(), 2);
        let (url2, params2) = reconcile_url_params(&url, &params);
        assert_eq!(url, url2);
        assert_eq!(params.len(), params2.len());
    }
}
