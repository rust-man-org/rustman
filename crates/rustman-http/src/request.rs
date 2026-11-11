use base64::{Engine as _, engine::general_purpose};
use reqwest::header::{ACCEPT_ENCODING, HeaderMap, HeaderName, HeaderValue};
use reqwest::multipart::{Form, Part};
use reqwest::{Client, Method, RequestBuilder, Url};
use rustman_core::domain::headers::Headers;
use rustman_core::domain::model::Auth;
use rustman_core::domain::request::{ApiKeyLocation, HttpMethod, graphql_body};
use rustman_core::domain::resolved::{FieldValue, ResolvedBody, ResolvedField, ResolvedRequest};

use crate::auth::auth_header;
use crate::error::SendError;
use crate::response::ACCEPTED_ENCODINGS;

/// A request ready to send.
pub(crate) struct Built {
    pub(crate) request: reqwest::Request,
    /// Whether it carries anything beyond its URL (headers, auth or a body)
    /// that a redirect to plain HTTP would expose.
    pub(crate) has_payload: bool,
}

pub(crate) fn build(client: &Client, request: ResolvedRequest<'_>) -> Result<Built, SendError> {
    let ResolvedRequest {
        method,
        url,
        query,
        headers,
        auth,
        body,
    } = request;
    let has_payload =
        !headers.is_empty() || !matches!(auth, Auth::None) || !matches!(body, ResolvedBody::None);
    let query_key = match &auth {
        Auth::ApiKey {
            name,
            value,
            location: ApiKeyLocation::Query,
        } if !name.is_empty() => Some((name.clone(), value.clone())),
        _ => None,
    };
    let builder = client.request(reqwest_method(&method), url);
    let mut request = with_body(builder.headers(header_map(headers, auth)?), body)?
        .build()
        .map_err(SendError::Transport)?;
    append_query(request.url_mut(), query.iter().chain(&query_key));
    Ok(Built {
        request,
        has_payload,
    })
}

/// Adds `pairs` after the URL's own query. A pair without a value goes as a
/// bare flag (`?debug`), the way the URL bar writes it.
fn append_query<'a>(url: &mut Url, pairs: impl IntoIterator<Item = &'a (String, String)>) {
    let mut pairs = pairs.into_iter().peekable();
    // Even an untouched serializer leaves a `?` on a URL that had no query.
    if pairs.peek().is_none() {
        return;
    }
    let mut query = url.query_pairs_mut();
    for (key, value) in pairs {
        if value.is_empty() {
            query.append_key_only(key);
        } else {
            query.append_pair(key, value);
        }
    }
}

fn reqwest_method(method: &HttpMethod) -> Method {
    match method {
        HttpMethod::Get => Method::GET,
        HttpMethod::Post => Method::POST,
        HttpMethod::Put => Method::PUT,
        HttpMethod::Patch => Method::PATCH,
        HttpMethod::Delete => Method::DELETE,
        HttpMethod::Head => Method::HEAD,
        HttpMethod::Options => Method::OPTIONS,
    }
}

/// The header rows, then the auth header, which replaces any row with its
/// name, then the encodings Rustman decodes unless the rows name their own.
fn header_map(headers: Headers, auth: Auth<String>) -> Result<HeaderMap, SendError> {
    let mut map = HeaderMap::with_capacity(headers.len() + 2);
    for (name, value) in headers {
        let name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| SendError::HeaderName(name))?;
        let value = HeaderValue::try_from(value)
            .map_err(|_| SendError::HeaderValue(name.as_str().to_owned()))?;
        // Two rows with one name are two headers on the wire, as in curl.
        map.append(name, value);
    }
    if let Some((name, value)) = auth_header(auth)? {
        map.insert(name, value);
    }
    if !map.contains_key(ACCEPT_ENCODING) {
        map.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static(ACCEPTED_ENCODINGS),
        );
    }
    Ok(map)
}

fn with_body(builder: RequestBuilder, body: ResolvedBody<'_>) -> Result<RequestBuilder, SendError> {
    Ok(match body {
        ResolvedBody::None => builder,
        ResolvedBody::Json(text) | ResolvedBody::Text(text) if text.is_empty() => builder,
        ResolvedBody::Json(text) | ResolvedBody::Text(text) => builder.body(text),
        ResolvedBody::GraphQl { query, variables }
            if query.trim().is_empty() && variables.trim().is_empty() =>
        {
            builder
        }
        ResolvedBody::GraphQl { query, variables } => {
            builder.body(graphql_body(&query, &variables).map_err(SendError::GraphQl)?)
        }
        ResolvedBody::Form(fields) => with_form(builder, fields)?,
    })
}

/// Multipart when any field is a file; otherwise urlencoded, which is what
/// login-style form endpoints accept.
fn with_form(
    builder: RequestBuilder,
    fields: Vec<ResolvedField<'_>>,
) -> Result<RequestBuilder, SendError> {
    if !fields
        .iter()
        .any(|f| matches!(f.value, FieldValue::File { .. }))
    {
        let pairs: Vec<(&str, &str)> = fields
            .iter()
            .filter_map(|f| match &f.value {
                FieldValue::Text(text) => Some((f.name.as_str(), text.as_str())),
                FieldValue::File { .. } => None,
            })
            .collect();
        return Ok(builder.form(&pairs));
    }
    let mut form = Form::new();
    for ResolvedField { name, value } in fields {
        form = match value {
            FieldValue::Text(text) => form.text(name, text),
            FieldValue::File {
                file_name,
                data,
                mime_type,
            } => {
                let data = data.ok_or_else(|| SendError::MissingFile(name.clone()))?;
                form.part(name, file_part(data, file_name, mime_type)?)
            }
        };
    }
    Ok(builder.multipart(form))
}

fn file_part(
    data: &str,
    file_name: Option<&str>,
    mime_type: Option<&str>,
) -> Result<Part, SendError> {
    let part = Part::bytes(general_purpose::STANDARD.decode(data)?)
        .file_name(file_name.unwrap_or("file").to_owned());
    match mime_type {
        Some(mime) => part.mime_str(mime).map_err(SendError::Mime),
        None => Ok(part),
    }
}
