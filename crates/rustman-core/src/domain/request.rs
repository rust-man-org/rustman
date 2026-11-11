use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::url::{decode_query_component, encode_query_component, split_fragment};

/// The HTTP methods a request can use.
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
    /// The method as it appears on the wire.
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

    /// Every method, in menu order.
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

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::all()
            .iter()
            .find(|m| m.as_str().eq_ignore_ascii_case(s))
            .cloned()
            .ok_or_else(|| format!("Unknown HTTP method: {}", s.to_ascii_uppercase()))
    }
}

/// How a request body is encoded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyType {
    None,
    Json,
    Text,
    FormData,
    GraphQL,
}

/// How a request authenticates.
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
    /// The id the auth picker uses for this type.
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

impl FromStr for AuthType {
    type Err = String;

    /// Unknown ids parse as [`AuthType::None`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "basic" => Self::Basic,
            "bearer" => Self::Bearer,
            "apikey" => Self::ApiKey,
            "jwt-user" => Self::JwtUser,
            "cookie" => Self::Cookie,
            _ => Self::None,
        })
    }
}

/// Where an API key is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyLocation {
    Header,
    Query,
}

impl ApiKeyLocation {
    /// The id the location picker uses.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Query => "query",
        }
    }
}

impl FromStr for ApiKeyLocation {
    type Err = ();

    /// Anything but `query` parses as [`ApiKeyLocation::Header`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(if s == "query" {
            Self::Query
        } else {
            Self::Header
        })
    }
}

/// One row of a params, headers or cookies table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValue {
    pub id: String,
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

impl KeyValue {
    /// An enabled, empty row with a fresh id.
    pub fn new_empty() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            key: String::new(),
            value: String::new(),
            enabled: true,
        }
    }

    /// Whether the row has neither a key nor a value. Any other params row
    /// is sent, `=value` and bare flags included.
    pub fn is_blank(&self) -> bool {
        self.key.is_empty() && self.value.is_empty()
    }
}

/// The params table for `url`'s query string, plus the *disabled* rows of
/// `existing`. Enabled rows are owned by the URL; disabled rows live only in
/// the table, so a re-derive must keep them.
pub fn sync_params_from_url(url: &str, existing: &[KeyValue]) -> Vec<KeyValue> {
    let (url, _) = split_fragment(url);
    let qs = url.split_once('?').map_or("", |(_, q)| q);
    let mut params: Vec<KeyValue> = qs
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (k, val) = pair.split_once('=').unwrap_or((pair, ""));
            KeyValue {
                id: uuid::Uuid::new_v4().to_string(),
                key: decode_query_component(k),
                value: decode_query_component(val),
                enabled: true,
            }
        })
        .collect();
    let kept = not_in_query(
        &params,
        existing.iter().filter(|p| !p.enabled && !p.is_blank()),
    );
    params.extend(kept);
    params
}

/// The `rows` the query rows `in_query` don't already hold. The URL owns the
/// enabled rows, so an enabled row is held by its key alone; a disabled row
/// only by its key and value, so an alternative value kept switched off
/// beside its enabled twin (`page=1` on, `page=2` off) is not dropped.
fn not_in_query<'a>(
    in_query: &[KeyValue],
    rows: impl Iterator<Item = &'a KeyValue>,
) -> Vec<KeyValue> {
    let keys: HashSet<&str> = in_query.iter().map(|p| p.key.as_str()).collect();
    let pairs: HashSet<(&str, &str)> = in_query
        .iter()
        .map(|p| (p.key.as_str(), p.value.as_str()))
        .collect();
    rows.filter(|p| {
        if p.enabled {
            !keys.contains(p.key.as_str())
        } else {
            !pairs.contains(&(p.key.as_str(), p.value.as_str()))
        }
    })
    .cloned()
    .collect()
}

/// `url` with its query rebuilt from the enabled params, before any
/// `#fragment`. A row with only a value is written `=value`, one with only a
/// key is a bare flag, and an empty row is skipped.
pub fn sync_url_from_params(url: &str, params: &[KeyValue]) -> String {
    let (url, fragment) = split_fragment(url);
    let base = url.split_once('?').map_or(url, |(b, _)| b);
    let qs: Vec<String> = params
        .iter()
        .filter(|p| p.enabled && !p.is_blank())
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
        format!("{base}{fragment}")
    } else {
        format!("{base}?{}{fragment}", qs.join("&"))
    }
}

/// A loaded URL and its stored params made consistent: the URL's query
/// united with the stored rows it doesn't hold, so later edits in the URL
/// bar can't drop saved params.
pub fn reconcile_url_params(url: &str, stored: &[KeyValue]) -> (String, Vec<KeyValue>) {
    let mut params = sync_params_from_url(url, &[]);
    let kept = not_in_query(&params, stored.iter().filter(|p| !p.is_blank()));
    params.extend(kept);
    let new_url = sync_url_from_params(url, &params);
    (new_url, params)
}

/// One field of a form-data body: text, or a file carried inline as base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Whether a form field holds text or a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormFieldType {
    Text,
    File,
}

/// The JSON document GraphQL servers accept: `{"query": …, "variables": {…}}`.
///
/// Blank `variables` are left out. Anything but a JSON object is an error
/// rather than being sent as a string, which servers reject outright.
pub fn graphql_body(query: &str, variables: &str) -> Result<String, String> {
    #[derive(Serialize)]
    struct Body<'a> {
        query: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        variables: Option<serde_json::Value>,
    }

    let variables = variables.trim();
    let variables = if variables.is_empty() {
        None
    } else {
        let parsed: serde_json::Value = serde_json::from_str(variables)
            .map_err(|e| format!("GraphQL variables are not valid JSON: {e}"))?;
        if !parsed.is_object() {
            return Err("GraphQL variables must be a JSON object, e.g. {\"id\": 1}".to_owned());
        }
        Some(parsed)
    };

    serde_json::to_string(&Body { query, variables }).map_err(|e| format!("GraphQL body: {e}"))
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
        assert_eq!(
            graphql_body("{ me }", "  \n ").unwrap(),
            r#"{"query":"{ me }"}"#
        );
    }

    #[test]
    fn variables_are_embedded_as_json_not_a_string() {
        let body = graphql_body(
            "query($id: ID!) { user(id: $id) { name } }",
            r#"{"id": "42"}"#,
        )
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
        KeyValue {
            id: "x".into(),
            key: key.into(),
            value: value.into(),
            enabled,
        }
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
    fn disabled_existing_preserved_enabled_dropped() {
        let existing = [kv("keep", "v", false), kv("gone", "v", true)];
        let p = sync_params_from_url("https://x.com/a?one=1", &existing);
        let keys: Vec<&str> = p.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, ["one", "keep"]);
    }

    #[test]
    fn rebuild_url_skips_disabled_and_empty() {
        let params = [kv("a", "1", true), kv("b", "2", false), kv("", "", true)];
        assert_eq!(
            sync_url_from_params("https://x.com/p?old=1", &params),
            "https://x.com/p?a=1"
        );
    }

    #[test]
    fn empty_keys_and_bare_flags_survive_an_edit() {
        let url = "https://x.com/p?=v&debug&a=";
        let params = sync_params_from_url(url, &[]);
        assert_eq!(
            sync_url_from_params(url, &params),
            "https://x.com/p?=v&debug&a"
        );
    }

    #[test]
    fn the_fragment_stays_after_the_query() {
        let params = sync_params_from_url("https://x.com/p?a=1#top", &[]);
        assert_eq!(
            params.iter().map(|p| p.value.as_str()).collect::<Vec<_>>(),
            ["1"]
        );
        assert!(sync_params_from_url("https://x.com/p#top?a=1", &[]).is_empty());
        assert_eq!(
            sync_url_from_params("https://x.com/p?old#top", &params),
            "https://x.com/p?a=1#top"
        );
        assert_eq!(
            sync_url_from_params("https://x.com/p?old#top", &[]),
            "https://x.com/p#top"
        );
    }

    #[test]
    fn rebuild_encodes_specials() {
        let params = [kv("q", "a b&c", true)];
        assert_eq!(
            sync_url_from_params("https://x.com", &params),
            "https://x.com?q=a%20b%26c"
        );
    }

    #[test]
    fn reconcile_folds_legacy_base_url_and_params() {
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
    fn reconcile_keeps_stored_rows_without_a_key() {
        let stored = [kv("", "v", true), kv("", "off", false), kv("", "", true)];
        let (url, params) = reconcile_url_params("https://x.com/a", &stored);
        assert_eq!(url, "https://x.com/a?=v");
        assert_eq!(
            params
                .iter()
                .map(|p| (p.value.as_str(), p.enabled))
                .collect::<Vec<_>>(),
            [("v", true), ("off", false)]
        );
    }

    #[test]
    fn a_disabled_alternative_value_survives_its_enabled_twin() {
        let rows = [kv("page", "1", true), kv("page", "2", false)];
        let pairs = |rows: &[KeyValue]| -> Vec<(String, String, bool)> {
            rows.iter()
                .map(|r| (r.key.clone(), r.value.clone(), r.enabled))
                .collect()
        };
        let expected = pairs(&rows);

        let (url, opened) = reconcile_url_params("https://x.com/a?page=1", &rows);
        assert_eq!(url, "https://x.com/a?page=1");
        assert_eq!(pairs(&opened), expected, "opening the request");
        let typed = sync_params_from_url("https://x.com/a?page=1&", &opened);
        assert_eq!(pairs(&typed), expected, "typing in the URL bar");

        let retyped = sync_params_from_url("https://x.com/a?page=2", &[kv("page", "2", false)]);
        assert_eq!(
            pairs(&retyped),
            [("page".into(), "2".into(), true)],
            "typed back in, it is the enabled row"
        );
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
