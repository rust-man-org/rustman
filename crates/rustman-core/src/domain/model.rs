//! The request model: one shape for a request as edited, stored and sent.
//!
//! The settings of every auth scheme and body kind are kept, so switching
//! between them never loses what was typed; [`AuthSettings::effective`] and
//! [`BodySettings::effective`] are the only way to read what is actually
//! sent. On disk a [`Request`] is written as a [`SavedRequest`], byte for
//! byte.

use serde::{Deserialize, Serialize};

use super::collection::SavedRequest;
use super::ids::{CollectionId, GroupId, RequestId};
use super::request::{ApiKeyLocation, AuthType, BodyType, FormField, HttpMethod, KeyValue};

/// The only JWT algorithm Rustman signs with.
const JWT_ALGO: &str = "HS256";

/// A request in a collection, a tab or a history entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "SavedRequest")]
pub struct Request {
    pub id: RequestId,
    pub collection_id: CollectionId,
    /// The enclosing group, or `None` for a direct child of the collection.
    pub group_id: Option<GroupId>,
    /// Manual order among the siblings that share this request's parent.
    pub sort_order: i64,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub params: Vec<KeyValue>,
    pub headers: Vec<KeyValue>,
    pub body: BodySettings,
    pub auth: AuthSettings,
    /// What the scripting `cookie()` built-in reads. Not sent.
    pub script_cookies: Vec<KeyValue>,
    pub scripts: Scripts,
}

/// The pre-request and test scripts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scripts {
    pub pre_request: String,
    pub test: String,
}

/// Which body kind is sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BodyKind {
    #[default]
    None,
    Json,
    Text,
    Form,
    GraphQl,
}

/// The body as edited: the active kind plus what every kind holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BodySettings {
    pub kind: BodyKind,
    /// The JSON or text body, or the GraphQL query.
    pub text: String,
    /// GraphQL variables as typed (JSON text).
    pub graphql_variables: String,
    pub form: Vec<FormField>,
}

/// The body that is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body<'a> {
    None,
    Json(&'a str),
    Text(&'a str),
    Form(&'a [FormField]),
    GraphQl { query: &'a str, variables: &'a str },
}

impl BodySettings {
    /// The body of the active kind.
    pub fn effective(&self) -> Body<'_> {
        select_body(self.kind, &self.text, &self.graphql_variables, &self.form)
    }
}

fn select_body<'a>(
    kind: BodyKind,
    text: &'a str,
    variables: &'a str,
    form: &'a [FormField],
) -> Body<'a> {
    match kind {
        BodyKind::None => Body::None,
        BodyKind::Json => Body::Json(text),
        BodyKind::Text => Body::Text(text),
        BodyKind::Form => Body::Form(form),
        BodyKind::GraphQl => Body::GraphQl {
            query: text,
            variables,
        },
    }
}

/// Which auth scheme is sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AuthKind {
    #[default]
    None,
    Basic,
    Bearer,
    ApiKey,
    Cookie,
    Jwt,
}

/// The auth as edited: the active scheme plus what every scheme holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSettings {
    pub active: AuthKind,
    pub basic: BasicAuth,
    pub bearer: BearerAuth,
    pub api_key: ApiKeyAuth,
    pub cookie: CookieAuth,
    pub jwt: JwtAuth,
}

/// HTTP Basic credentials.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BasicAuth {
    pub user: String,
    pub pass: String,
}

/// A bearer token, sent as `Authorization: Bearer …`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BearerAuth {
    pub token: String,
}

/// An API key sent as a header or a query parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyAuth {
    pub name: String,
    pub value: String,
    pub location: ApiKeyLocation,
}

/// A `Cookie` header value, sent as is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CookieAuth {
    pub cookies: String,
}

/// An HS256 token signed with `secret` for `subject` when sent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JwtAuth {
    pub secret: String,
    pub subject: String,
}

/// The auth that is sent, over borrowed (`&str`) or resolved (`String`) text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth<S> {
    None,
    Basic {
        user: S,
        pass: S,
    },
    Bearer {
        token: S,
    },
    ApiKey {
        name: S,
        value: S,
        location: ApiKeyLocation,
    },
    Cookie {
        cookies: S,
    },
    Jwt {
        secret: S,
        subject: S,
    },
}

impl<S> Auth<S> {
    /// The same auth with `f` applied to every text field, e.g. to expand
    /// `{{variables}}`.
    pub fn map<T>(self, mut f: impl FnMut(S) -> T) -> Auth<T> {
        match self {
            Self::None => Auth::None,
            Self::Basic { user, pass } => Auth::Basic {
                user: f(user),
                pass: f(pass),
            },
            Self::Bearer { token } => Auth::Bearer { token: f(token) },
            Self::ApiKey {
                name,
                value,
                location,
            } => Auth::ApiKey {
                name: f(name),
                value: f(value),
                location,
            },
            Self::Cookie { cookies } => Auth::Cookie {
                cookies: f(cookies),
            },
            Self::Jwt { secret, subject } => Auth::Jwt {
                secret: f(secret),
                subject: f(subject),
            },
        }
    }
}

impl AuthSettings {
    /// The auth of the active scheme.
    pub fn effective(&self) -> Auth<&str> {
        select_auth(
            self.active,
            AuthFields {
                user: &self.basic.user,
                pass: &self.basic.pass,
                token: &self.bearer.token,
                key_name: &self.api_key.name,
                key_value: &self.api_key.value,
                key_location: self.api_key.location,
                cookies: &self.cookie.cookies,
                jwt_secret: &self.jwt.secret,
                jwt_subject: &self.jwt.subject,
            },
        )
    }
}

/// Every scheme's settings, borrowed, for picking the active one.
#[derive(Clone, Copy)]
struct AuthFields<'a> {
    user: &'a str,
    pass: &'a str,
    token: &'a str,
    key_name: &'a str,
    key_value: &'a str,
    key_location: ApiKeyLocation,
    cookies: &'a str,
    jwt_secret: &'a str,
    jwt_subject: &'a str,
}

fn select_auth(kind: AuthKind, f: AuthFields<'_>) -> Auth<&str> {
    match kind {
        AuthKind::None => Auth::None,
        AuthKind::Basic => Auth::Basic {
            user: f.user,
            pass: f.pass,
        },
        AuthKind::Bearer => Auth::Bearer { token: f.token },
        AuthKind::ApiKey => Auth::ApiKey {
            name: f.key_name,
            value: f.key_value,
            location: f.key_location,
        },
        AuthKind::Cookie => Auth::Cookie { cookies: f.cookies },
        AuthKind::Jwt => Auth::Jwt {
            secret: f.jwt_secret,
            subject: f.jwt_subject,
        },
    }
}

impl SavedRequest {
    /// The auth of the active scheme; see [`AuthSettings::effective`].
    pub fn effective_auth(&self) -> Auth<&str> {
        select_auth(
            AuthKind::from(self.auth_type.clone()),
            AuthFields {
                user: &self.basic_user,
                pass: &self.basic_pass,
                token: &self.bearer_token,
                key_name: &self.api_key_name,
                key_value: &self.api_key_value,
                key_location: self.api_key_location,
                cookies: &self.cookie_string,
                jwt_secret: &self.jwt_secret,
                jwt_subject: &self.jwt_subject,
            },
        )
    }

    /// The body of the active kind; see [`BodySettings::effective`].
    pub fn effective_body(&self) -> Body<'_> {
        select_body(
            BodyKind::from(self.body_type.clone()),
            &self.body,
            &self.graphql_variables,
            &self.form_data_fields,
        )
    }
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            active: AuthKind::None,
            basic: BasicAuth::default(),
            bearer: BearerAuth::default(),
            api_key: ApiKeyAuth {
                name: String::new(),
                value: String::new(),
                location: ApiKeyLocation::Header,
            },
            cookie: CookieAuth::default(),
            jwt: JwtAuth::default(),
        }
    }
}

impl From<BodyType> for BodyKind {
    fn from(kind: BodyType) -> Self {
        match kind {
            BodyType::None => Self::None,
            BodyType::Json => Self::Json,
            BodyType::Text => Self::Text,
            BodyType::FormData => Self::Form,
            BodyType::GraphQL => Self::GraphQl,
        }
    }
}

impl From<BodyKind> for BodyType {
    fn from(kind: BodyKind) -> Self {
        match kind {
            BodyKind::None => Self::None,
            BodyKind::Json => Self::Json,
            BodyKind::Text => Self::Text,
            BodyKind::Form => Self::FormData,
            BodyKind::GraphQl => Self::GraphQL,
        }
    }
}

impl From<AuthType> for AuthKind {
    fn from(kind: AuthType) -> Self {
        match kind {
            AuthType::None => Self::None,
            AuthType::Basic => Self::Basic,
            AuthType::Bearer => Self::Bearer,
            AuthType::ApiKey => Self::ApiKey,
            AuthType::Cookie => Self::Cookie,
            AuthType::JwtUser => Self::Jwt,
        }
    }
}

impl From<AuthKind> for AuthType {
    fn from(kind: AuthKind) -> Self {
        match kind {
            AuthKind::None => Self::None,
            AuthKind::Basic => Self::Basic,
            AuthKind::Bearer => Self::Bearer,
            AuthKind::ApiKey => Self::ApiKey,
            AuthKind::Cookie => Self::Cookie,
            AuthKind::Jwt => Self::JwtUser,
        }
    }
}

impl From<SavedRequest> for Request {
    fn from(r: SavedRequest) -> Self {
        Self {
            id: r.id.into(),
            collection_id: r.collection_id.into(),
            group_id: r.group_id.map(Into::into),
            sort_order: r.sort_order,
            name: r.name,
            method: r.method,
            url: r.url,
            params: r.params,
            headers: r.headers,
            body: BodySettings {
                kind: r.body_type.into(),
                text: r.body,
                graphql_variables: r.graphql_variables,
                form: r.form_data_fields,
            },
            auth: AuthSettings {
                active: r.auth_type.into(),
                basic: BasicAuth {
                    user: r.basic_user,
                    pass: r.basic_pass,
                },
                bearer: BearerAuth {
                    token: r.bearer_token,
                },
                api_key: ApiKeyAuth {
                    name: r.api_key_name,
                    value: r.api_key_value,
                    location: r.api_key_location,
                },
                cookie: CookieAuth {
                    cookies: r.cookie_string,
                },
                jwt: JwtAuth {
                    secret: r.jwt_secret,
                    subject: r.jwt_subject,
                },
            },
            script_cookies: r.cookies,
            scripts: Scripts {
                pre_request: r.pre_request_script,
                test: r.test_script,
            },
        }
    }
}

impl From<Request> for SavedRequest {
    fn from(r: Request) -> Self {
        let AuthSettings {
            active,
            basic,
            bearer,
            api_key,
            cookie,
            jwt,
        } = r.auth;
        Self {
            id: r.id.into(),
            collection_id: r.collection_id.into(),
            group_id: r.group_id.map(Into::into),
            sort_order: r.sort_order,
            name: r.name,
            method: r.method,
            url: r.url,
            headers: r.headers,
            params: r.params,
            body: r.body.text,
            body_type: r.body.kind.into(),
            graphql_variables: r.body.graphql_variables,
            auth_type: active.into(),
            bearer_token: bearer.token,
            basic_user: basic.user,
            basic_pass: basic.pass,
            api_key_name: api_key.name,
            api_key_value: api_key.value,
            api_key_location: api_key.location,
            form_data_fields: r.body.form,
            cookie_string: cookie.cookies,
            cookies: r.script_cookies,
            jwt_secret: jwt.secret,
            jwt_subject: jwt.subject,
            jwt_algo: JWT_ALGO.to_owned(),
            pre_request_script: r.scripts.pre_request,
            test_script: r.scripts.test,
        }
    }
}

/// [`SavedRequest`]'s serialized shape over borrowed data, so writing a
/// request copies nothing. Field names and order must match it exactly; the
/// golden tests hold them to it.
#[derive(Serialize)]
struct SavedRequestRef<'a> {
    id: &'a RequestId,
    collection_id: &'a CollectionId,
    group_id: &'a Option<GroupId>,
    sort_order: i64,
    name: &'a str,
    method: &'a HttpMethod,
    url: &'a str,
    headers: &'a [KeyValue],
    params: &'a [KeyValue],
    body: &'a str,
    body_type: BodyType,
    graphql_variables: &'a str,
    auth_type: AuthType,
    bearer_token: &'a str,
    basic_user: &'a str,
    basic_pass: &'a str,
    api_key_name: &'a str,
    api_key_value: &'a str,
    api_key_location: &'a ApiKeyLocation,
    form_data_fields: &'a [FormField],
    cookie_string: &'a str,
    cookies: &'a [KeyValue],
    jwt_secret: &'a str,
    jwt_subject: &'a str,
    jwt_algo: &'a str,
    pre_request_script: &'a str,
    test_script: &'a str,
}

impl Serialize for Request {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (auth, body) = (&self.auth, &self.body);
        SavedRequestRef {
            id: &self.id,
            collection_id: &self.collection_id,
            group_id: &self.group_id,
            sort_order: self.sort_order,
            name: &self.name,
            method: &self.method,
            url: &self.url,
            headers: &self.headers,
            params: &self.params,
            body: &body.text,
            body_type: body.kind.into(),
            graphql_variables: &body.graphql_variables,
            auth_type: auth.active.into(),
            bearer_token: &auth.bearer.token,
            basic_user: &auth.basic.user,
            basic_pass: &auth.basic.pass,
            api_key_name: &auth.api_key.name,
            api_key_value: &auth.api_key.value,
            api_key_location: &auth.api_key.location,
            form_data_fields: &body.form,
            cookie_string: &auth.cookie.cookies,
            cookies: &self.script_cookies,
            jwt_secret: &auth.jwt.secret,
            jwt_subject: &auth.jwt.subject,
            jwt_algo: JWT_ALGO,
            pre_request_script: &self.scripts.pre_request,
            test_script: &self.scripts.test,
        }
        .serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_scheme_filled() -> AuthSettings {
        AuthSettings {
            active: AuthKind::None,
            basic: BasicAuth {
                user: "u".into(),
                pass: "p".into(),
            },
            bearer: BearerAuth { token: "t".into() },
            api_key: ApiKeyAuth {
                name: "k".into(),
                value: "v".into(),
                location: ApiKeyLocation::Query,
            },
            cookie: CookieAuth {
                cookies: "c=1".into(),
            },
            jwt: JwtAuth {
                secret: "s".into(),
                subject: "sub".into(),
            },
        }
    }

    #[test]
    fn effective_auth_is_exactly_the_active_scheme() {
        let mut auth = every_scheme_filled();
        let cases = [
            (AuthKind::None, Auth::None),
            (
                AuthKind::Basic,
                Auth::Basic {
                    user: "u",
                    pass: "p",
                },
            ),
            (AuthKind::Bearer, Auth::Bearer { token: "t" }),
            (
                AuthKind::ApiKey,
                Auth::ApiKey {
                    name: "k",
                    value: "v",
                    location: ApiKeyLocation::Query,
                },
            ),
            (AuthKind::Cookie, Auth::Cookie { cookies: "c=1" }),
            (
                AuthKind::Jwt,
                Auth::Jwt {
                    secret: "s",
                    subject: "sub",
                },
            ),
        ];
        for (kind, expected) in cases {
            auth.active = kind;
            assert_eq!(auth.effective(), expected, "{kind:?}");
        }
    }

    #[test]
    fn effective_body_is_exactly_the_active_kind() {
        let mut body = BodySettings {
            kind: BodyKind::None,
            text: "{\"a\":1}".into(),
            graphql_variables: "{}".into(),
            form: Vec::new(),
        };
        assert_eq!(body.effective(), Body::None, "text is present but not sent");
        body.kind = BodyKind::Json;
        assert_eq!(body.effective(), Body::Json("{\"a\":1}"));
        body.kind = BodyKind::GraphQl;
        assert_eq!(
            body.effective(),
            Body::GraphQl {
                query: "{\"a\":1}",
                variables: "{}"
            }
        );
        body.kind = BodyKind::Form;
        assert_eq!(body.effective(), Body::Form(&[]));
    }

    #[test]
    fn kinds_map_to_the_legacy_enums_and_back() {
        for kind in [
            AuthKind::None,
            AuthKind::Basic,
            AuthKind::Bearer,
            AuthKind::ApiKey,
            AuthKind::Cookie,
            AuthKind::Jwt,
        ] {
            assert_eq!(AuthKind::from(AuthType::from(kind)), kind);
        }
        for kind in [
            BodyKind::None,
            BodyKind::Json,
            BodyKind::Text,
            BodyKind::Form,
            BodyKind::GraphQl,
        ] {
            assert_eq!(BodyKind::from(BodyType::from(kind)), kind);
        }
    }
}
