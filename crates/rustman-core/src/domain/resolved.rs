//! A request with its `{{variables}}` expanded: the one input every
//! transport and exporter works from.

use super::collection::SavedRequest;
use super::environment::AppEnvironment;
use super::headers::Headers;
use super::model::{Auth, Body, Request};
use super::request::{
    ApiKeyLocation, FormField, FormFieldType, HttpMethod, KeyValue, graphql_body,
};
use crate::template::{resolve_url, substitute};
use crate::url::{encode_query_component, split_fragment};

/// What a request sends, variables expanded. File content is borrowed from
/// the request it was resolved from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRequest<'a> {
    pub method: HttpMethod,
    /// The URL with a scheme, without the query `query` carries.
    pub url: String,
    /// The enabled params, in order.
    pub query: Vec<(String, String)>,
    /// The enabled header rows in order, plus the `Content-Type` a JSON,
    /// GraphQL or text body implies when none is set.
    pub headers: Headers,
    pub auth: Auth<String>,
    pub body: ResolvedBody<'a>,
}

/// The body that is sent, variables expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedBody<'a> {
    None,
    Json(String),
    Text(String),
    /// The enabled fields.
    Form(Vec<ResolvedField<'a>>),
    GraphQl {
        query: String,
        variables: String,
    },
}

/// An enabled form field, its name and text expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedField<'a> {
    pub name: String,
    pub value: FieldValue<'a>,
}

/// A form field's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue<'a> {
    Text(String),
    /// A file, carried inline as base64 (`None` until one is picked).
    File {
        file_name: Option<&'a str>,
        data: Option<&'a str>,
        mime_type: Option<&'a str>,
    },
}

impl Request {
    /// The request as sent with `env`'s variables.
    pub fn resolve(&self, env: Option<&AppEnvironment>) -> ResolvedRequest<'_> {
        Sendable {
            method: &self.method,
            url: &self.url,
            params: &self.params,
            headers: &self.headers,
            auth: self.auth.effective(),
            body: self.body.effective(),
        }
        .resolve(env)
    }
}

impl SavedRequest {
    /// The request as sent with `env`'s variables.
    pub fn resolve(&self, env: Option<&AppEnvironment>) -> ResolvedRequest<'_> {
        Sendable {
            method: &self.method,
            url: &self.url,
            params: &self.params,
            headers: &self.headers,
            auth: self.effective_auth(),
            body: self.effective_body(),
        }
        .resolve(env)
    }
}

/// What resolving needs from a request, borrowed from either model.
struct Sendable<'a> {
    method: &'a HttpMethod,
    url: &'a str,
    params: &'a [KeyValue],
    headers: &'a [KeyValue],
    auth: Auth<&'a str>,
    body: Body<'a>,
}

impl<'a> Sendable<'a> {
    fn resolve(self, env: Option<&AppEnvironment>) -> ResolvedRequest<'a> {
        let sub = |text: &str| substitute(text, env);
        let query: Vec<(String, String)> = self
            .params
            .iter()
            .filter(|p| p.enabled && !p.is_blank())
            .map(|p| (sub(&p.key), sub(&p.value)))
            .collect();
        let auth = self.auth.map(sub);
        let carries_query = !query.is_empty()
            || matches!(&auth, Auth::ApiKey { name, location: ApiKeyLocation::Query, .. } if !name.is_empty());
        let mut url = resolve_url(self.url, env);
        // The query is sent from the params; a URL that still carries one
        // would send every value twice, which servers read as an array.
        if carries_query {
            let (before, fragment) = split_fragment(&url);
            if let Some(start) = before.find('?') {
                url = format!("{}{fragment}", &before[..start]);
            }
        }

        let mut headers: Headers = self
            .headers
            .iter()
            .filter(|h| h.enabled && !h.key.is_empty())
            .map(|h| (sub(&h.key), sub(&h.value)))
            .collect();
        let implied_type = match self.body {
            Body::Json(_) | Body::GraphQl { .. } => Some("application/json"),
            Body::Text(_) => Some("text/plain"),
            Body::None | Body::Form(_) => None,
        };
        if let Some(content_type) = implied_type
            && !headers.contains("content-type")
        {
            headers.push("Content-Type", content_type);
        }

        ResolvedRequest {
            method: self.method.clone(),
            url,
            query,
            headers,
            auth,
            body: resolve_body(self.body, env),
        }
    }
}

fn resolve_body<'a>(body: Body<'a>, env: Option<&AppEnvironment>) -> ResolvedBody<'a> {
    let sub = |text: &str| substitute(text, env);
    match body {
        Body::None => ResolvedBody::None,
        Body::Json(text) => ResolvedBody::Json(sub(text)),
        Body::Text(text) => ResolvedBody::Text(sub(text)),
        Body::GraphQl { query, variables } => ResolvedBody::GraphQl {
            query: sub(query),
            variables: sub(variables),
        },
        Body::Form(fields) => ResolvedBody::Form(
            fields
                .iter()
                .filter(|f| f.enabled)
                .map(|f| resolve_field(f, env))
                .collect(),
        ),
    }
}

fn resolve_field<'a>(field: &'a FormField, env: Option<&AppEnvironment>) -> ResolvedField<'a> {
    ResolvedField {
        name: substitute(&field.key, env),
        value: match field.field_type {
            FormFieldType::Text => FieldValue::Text(substitute(&field.value, env)),
            FormFieldType::File => FieldValue::File {
                file_name: field.file_name.as_deref(),
                data: field.file_data.as_deref(),
                mime_type: field.mime_type.as_deref(),
            },
        },
    }
}

impl ResolvedBody<'_> {
    /// The body as the text that goes out, for scripts to read: a GraphQL
    /// request's JSON document, or its query when the variables can't be
    /// sent; nothing for no body or a form, which is no one text.
    pub fn sent_text(&self) -> String {
        match self {
            ResolvedBody::None | ResolvedBody::Form(_) => String::new(),
            ResolvedBody::Json(text) | ResolvedBody::Text(text) => text.clone(),
            ResolvedBody::GraphQl { query, variables } => {
                graphql_body(query, variables).unwrap_or_else(|_| query.clone())
            }
        }
    }
}

impl ResolvedRequest<'_> {
    /// The URL with the query, and an API key sent in the query, added the
    /// way the URL bar writes them: after any query the URL already has and
    /// before its `#fragment`.
    pub fn full_url(&self) -> String {
        let api_key = match &self.auth {
            Auth::ApiKey {
                name,
                value,
                location: ApiKeyLocation::Query,
            } if !name.is_empty() => Some((name, value)),
            _ => None,
        };
        let (base, fragment) = split_fragment(&self.url);
        let mut url = base.to_owned();
        let mut has_query = base.contains('?');
        for (key, value) in self.query.iter().map(|(k, v)| (k, v)).chain(api_key) {
            if !has_query {
                url.push('?');
                has_query = true;
            } else if !url.ends_with(['?', '&']) {
                url.push('&');
            }
            url.push_str(&encode_query_component(key));
            if !value.is_empty() {
                url.push('=');
                url.push_str(&encode_query_component(value));
            }
        }
        url.push_str(fragment);
        url
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::domain::model::{AuthKind, BodyKind};
    use crate::domain::request::{AuthType, BodyType};

    fn env(vars: &[(&str, &str)]) -> AppEnvironment {
        AppEnvironment {
            id: "e".into(),
            name: "e".into(),
            variables: vars
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<HashMap<_, _>>(),
            is_active: true,
        }
    }

    fn row(key: &str, value: &str, enabled: bool) -> KeyValue {
        KeyValue {
            id: key.into(),
            key: key.into(),
            value: value.into(),
            enabled,
        }
    }

    fn request() -> Request {
        SavedRequest::new_in("c".into(), "r".into()).into()
    }

    #[test]
    fn variables_are_expanded_everywhere_that_is_sent() {
        let mut req = request();
        req.url = "{{host}}/items?page=1".into();
        req.params = vec![row("page", "{{page}}", true), row("off", "x", false)];
        req.headers = vec![row("X-Tenant", "{{tenant}}", true), row("", "x", true)];
        req.auth.active = AuthKind::Bearer;
        req.auth.bearer.token = "{{token}}".into();
        req.body.kind = BodyKind::Json;
        req.body.text = "{\"id\": \"{{id}}\"}".into();
        let vars = env(&[
            ("host", "api.example.com"),
            ("page", "2"),
            ("tenant", "acme"),
            ("token", "t0k"),
            ("id", "7"),
        ]);

        let resolved = req.resolve(Some(&vars));

        assert_eq!(resolved.url, "https://api.example.com/items");
        assert_eq!(resolved.query, [("page".to_owned(), "2".to_owned())]);
        assert_eq!(
            resolved.headers.iter().collect::<Vec<_>>(),
            [("X-Tenant", "acme"), ("Content-Type", "application/json")]
        );
        assert_eq!(
            resolved.auth,
            Auth::Bearer {
                token: "t0k".into()
            }
        );
        assert_eq!(resolved.body, ResolvedBody::Json("{\"id\": \"7\"}".into()));
        assert_eq!(resolved.full_url(), "https://api.example.com/items?page=2");
    }

    #[test]
    fn an_explicit_content_type_is_kept() {
        let mut req = request();
        req.body.kind = BodyKind::GraphQl;
        req.headers = vec![row("content-type", "application/graphql+json", true)];
        let resolved = req.resolve(None);
        assert_eq!(resolved.headers.len(), 1);
        assert_eq!(
            resolved.headers.get("Content-Type"),
            Some("application/graphql+json")
        );
    }

    #[test]
    fn a_text_body_is_sent_as_plain_text_unless_a_type_is_set() {
        let mut req = request();
        req.body.kind = BodyKind::Text;
        req.body.text = "hello".into();
        let resolved = req.resolve(None);
        assert_eq!(resolved.headers.get("Content-Type"), Some("text/plain"));
        assert_eq!(resolved.body, ResolvedBody::Text("hello".into()));

        req.headers = vec![row("content-type", "text/csv", true)];
        let resolved = req.resolve(None);
        assert_eq!(
            resolved.headers.iter().collect::<Vec<_>>(),
            [("content-type", "text/csv")]
        );
    }

    #[test]
    fn a_query_api_key_goes_into_the_url_and_replaces_its_written_query() {
        let mut req = request();
        req.url = "https://x.io/a?stale=1".into();
        req.auth.active = AuthKind::ApiKey;
        req.auth.api_key.name = "key".into();
        req.auth.api_key.value = "a b".into();
        req.auth.api_key.location = ApiKeyLocation::Query;
        let resolved = req.resolve(None);
        assert_eq!(resolved.url, "https://x.io/a");
        assert_eq!(resolved.full_url(), "https://x.io/a?key=a%20b");
    }

    #[test]
    fn the_query_goes_before_the_fragment() {
        let mut req = request();
        req.url = "https://x.io/a?stale=1#top".into();
        req.params = vec![row("q", "1", true), row("", "v", true), row("", "", true)];
        let resolved = req.resolve(None);
        assert_eq!(resolved.url, "https://x.io/a#top");
        assert_eq!(resolved.full_url(), "https://x.io/a?q=1&=v#top");
    }

    #[test]
    fn a_saved_request_resolves_like_its_model() {
        let mut saved = SavedRequest::new_in("c".into(), "r".into());
        saved.url = "{{host}}/a?x=1".into();
        saved.params = vec![row("q", "{{q}}", true)];
        saved.headers = vec![row("X-A", "{{q}}", true)];
        saved.api_key_name = "key".into();
        saved.api_key_value = "{{q}}".into();
        saved.api_key_location = ApiKeyLocation::Query;
        saved.body = "{\"a\": \"{{q}}\"}".into();
        let vars = env(&[("host", "x.io"), ("q", "v")]);
        for auth_type in [AuthType::None, AuthType::ApiKey, AuthType::Bearer] {
            for body_type in [BodyType::None, BodyType::Json, BodyType::GraphQL] {
                saved.auth_type = auth_type.clone();
                saved.body_type = body_type.clone();
                let model = Request::from(saved.clone());
                assert_eq!(
                    saved.resolve(Some(&vars)),
                    model.resolve(Some(&vars)),
                    "{auth_type:?} {body_type:?}"
                );
            }
        }
    }

    #[test]
    fn only_enabled_form_fields_are_sent() {
        let mut req = request();
        req.body.kind = BodyKind::Form;
        req.body.form = vec![
            FormField {
                id: "1".into(),
                key: "{{k}}".into(),
                value: "{{v}}".into(),
                field_type: FormFieldType::Text,
                enabled: true,
                file_name: None,
                file_data: None,
                mime_type: None,
            },
            FormField {
                id: "2".into(),
                key: "off".into(),
                value: String::new(),
                field_type: FormFieldType::Text,
                enabled: false,
                file_name: None,
                file_data: None,
                mime_type: None,
            },
        ];
        let resolved = req.resolve(Some(&env(&[("k", "name"), ("v", "rustman")])));
        assert_eq!(
            resolved.body,
            ResolvedBody::Form(vec![ResolvedField {
                name: "name".into(),
                value: FieldValue::Text("rustman".into()),
            }])
        );
    }
}
