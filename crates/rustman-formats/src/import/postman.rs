//! Postman collections: import of the v2.0 and v2.1 formats, export as v2.1.

use serde::Deserialize;
use serde_json::{Value, json};

use rustman_core::domain::{
    collection::{Collection, SavedRequest},
    group::{ImportedCollection, ImportedGroup},
    request::{
        ApiKeyLocation, AuthType, BodyType, FormField, FormFieldType, HttpMethod, KeyValue,
        reconcile_url_params,
    },
};
use rustman_core::text::is_json_document;

use super::{ExportOptions, ImportOutcome};

// ── The Postman format ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PostmanCollection {
    info: PostmanInfo,
    item: Vec<PostmanItem>,
    /// Inherited by every folder and request that doesn't set its own.
    auth: Option<PostmanAuth>,
}

#[derive(Deserialize)]
struct PostmanInfo {
    name: String,
}

/// One entry of an `item` list. Postman tells a folder from a request only
/// by which field is present, so an entry is read loosely first and sorted
/// out in `TryFrom<RawItem>`. That way a broken entry is reported by name,
/// with the real reason, instead of as "did not match any variant".
///
/// Both payloads are boxed: each is hundreds of bytes, mostly unused `Option`s.
#[derive(Deserialize)]
#[serde(try_from = "RawItem")]
enum PostmanItem {
    Folder(Box<PostmanFolder>),
    Request(Box<PostmanRequestItem>),
}

#[derive(Deserialize)]
#[serde(expecting = "a Postman item (a request or a folder)")]
struct RawItem {
    #[serde(default)]
    name: String,
    item: Option<Vec<PostmanItem>>,
    /// Raw JSON, parsed in `try_from`, so its errors can name the item.
    request: Option<Value>,
    /// A folder's auth. A request's auth lives inside `request`.
    auth: Option<Value>,
}

struct PostmanFolder {
    name: String,
    item: Vec<PostmanItem>,
    auth: Option<PostmanAuth>,
}

struct PostmanRequestItem {
    name: String,
    request: PostmanRequest,
}

/// Why an entry of an `item` list can't be imported. `serde_json` appends the
/// line and column of the entry.
#[derive(Debug)]
enum ItemError {
    /// No `request` and no `item`: neither a request nor a folder.
    Unrecognised { name: String },
    Request {
        name: String,
        reason: serde_json::Error,
    },
    FolderAuth {
        name: String,
        reason: serde_json::Error,
    },
}

impl std::fmt::Display for ItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unrecognised { name } => write!(
                f,
                "item \"{name}\" is neither a request nor a folder: it has no \"request\" or \"item\""
            ),
            Self::Request { name, reason } => write!(f, "request \"{name}\" is invalid: {reason}"),
            Self::FolderAuth { name, reason } => {
                write!(f, "folder \"{name}\" has invalid auth: {reason}")
            }
        }
    }
}

impl TryFrom<RawItem> for PostmanItem {
    type Error = ItemError;

    fn try_from(raw: RawItem) -> Result<Self, ItemError> {
        let RawItem {
            name,
            item,
            request,
            auth,
        } = raw;
        if let Some(item) = item {
            let auth = auth
                .map(serde_json::from_value)
                .transpose()
                .map_err(|reason| ItemError::FolderAuth {
                    name: name.clone(),
                    reason,
                })?;
            return Ok(Self::Folder(Box::new(PostmanFolder { name, item, auth })));
        }
        let request = match request {
            // Both schema versions allow the short form `"request": "<url>"`.
            Some(Value::String(url)) => PostmanRequest::from_url(url),
            Some(value) => serde_json::from_value(value).map_err(|reason| ItemError::Request {
                name: name.clone(),
                reason,
            })?,
            None => return Err(ItemError::Unrecognised { name }),
        };
        Ok(Self::Request(Box::new(PostmanRequestItem {
            name,
            request,
        })))
    }
}

#[derive(Deserialize, Default)]
struct PostmanRequest {
    method: Option<String>,
    url: Option<PostmanUrl>,
    header: Option<Vec<PostmanHeader>>,
    body: Option<PostmanBody>,
    auth: Option<PostmanAuth>,
}

impl PostmanRequest {
    /// The short form `"request": "<url>"`, which means a plain GET.
    fn from_url(url: String) -> Self {
        Self {
            url: Some(PostmanUrl::Raw(url)),
            ..Self::default()
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PostmanUrl {
    Raw(String),
    Object(Box<PostmanUrlObject>),
}

#[derive(Deserialize)]
struct PostmanUrlObject {
    raw: Option<String>,
    protocol: Option<String>,
    /// Labels (`["api", "example", "com"]`) or the whole host as one string.
    host: Option<Value>,
    port: Option<String>,
    /// Segments, each a string or a `{"value": …}` object, or one string.
    path: Option<Value>,
    query: Option<Vec<PostmanParam>>,
}

impl PostmanUrlObject {
    /// `raw`, or the URL rebuilt from its parts when a file leaves `raw` out.
    fn url(&self) -> String {
        if let Some(raw) = &self.raw {
            return raw.clone();
        }
        let mut url = String::new();
        if let Some(protocol) = &self.protocol {
            url.push_str(protocol);
            url.push_str("://");
        }
        url.push_str(&joined(self.host.as_ref(), "."));
        if let Some(port) = &self.port {
            url.push(':');
            url.push_str(port);
        }
        let path = joined(self.path.as_ref(), "/");
        if !path.is_empty() {
            url.push('/');
            url.push_str(path.trim_start_matches('/'));
        }
        url
    }
}

/// A host or path given either as one string or as a list of parts.
fn joined(parts: Option<&Value>, separator: &str) -> String {
    let part = |value: &Value| match value {
        Value::String(text) => text.clone(),
        other => other
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    };
    match parts {
        Some(Value::Array(parts)) => parts.iter().map(part).collect::<Vec<_>>().join(separator),
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

#[derive(Deserialize)]
struct PostmanHeader {
    key: String,
    value: String,
    #[serde(default)]
    disabled: bool,
}

/// A query param or an `urlencoded` body field.
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
    options: Option<PostmanBodyOptions>,
    formdata: Option<Vec<PostmanFormField>>,
    urlencoded: Option<Vec<PostmanParam>>,
    graphql: Option<PostmanGraphQl>,
}

#[derive(Deserialize)]
struct PostmanBodyOptions {
    raw: Option<PostmanRawOptions>,
}

/// What a raw body's text is: `json`, `text`, `xml`, `html` or `javascript`.
#[derive(Deserialize)]
struct PostmanRawOptions {
    language: Option<String>,
}

/// Postman's GraphQL body: the query and its variables live side by side. Older
/// exports carry the variables as a JSON *string*, newer ones as an object, so
/// both shapes are accepted and normalised to text.
#[derive(Deserialize)]
struct PostmanGraphQl {
    query: Option<String>,
    variables: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostmanFormField {
    key: String,
    value: Option<String>,
    #[serde(rename = "type", default)]
    field_type: String,
    #[serde(default)]
    disabled: bool,
    /// A file field's path: one string, or a list when it holds several files.
    src: Option<Value>,
    content_type: Option<String>,
}

impl PostmanFormField {
    /// The first file a file field names.
    fn file_name(&self) -> Option<String> {
        match self.src.as_ref()? {
            Value::String(path) => Some(path.clone()),
            Value::Array(paths) => paths.iter().find_map(Value::as_str).map(str::to_owned),
            _ => None,
        }
        .filter(|path| !path.is_empty())
    }
}

/// An `auth` block. Only the settings of schemes Rustman can send are read;
/// any other scheme is known by its `type` alone.
#[derive(Deserialize)]
struct PostmanAuth {
    #[serde(rename = "type")]
    kind: String,
    bearer: Option<AuthSettings>,
    basic: Option<AuthSettings>,
    apikey: Option<AuthSettings>,
    jwt: Option<AuthSettings>,
}

/// One scheme's settings: v2.1 writes a `[{"key", "value"}]` list, v2.0 a
/// plain object.
#[derive(Deserialize)]
#[serde(untagged)]
enum AuthSettings {
    List(Vec<AuthSetting>),
    Map(serde_json::Map<String, Value>),
}

#[derive(Deserialize)]
struct AuthSetting {
    key: String,
    #[serde(default)]
    value: Value,
}

impl AuthSettings {
    /// The setting `key` as text. Booleans and numbers (`isSecretBase64Encoded:
    /// false`) come back in their JSON spelling.
    fn get(&self, key: &str) -> Option<String> {
        let value = match self {
            Self::List(list) => list.iter().find(|s| s.key == key).map(|s| &s.value),
            Self::Map(map) => map.get(key),
        };
        match value? {
            Value::Null => None,
            Value::String(text) => Some(text.clone()),
            other => Some(other.to_string()),
        }
    }
}

// ── Import ────────────────────────────────────────────────────────────────────

/// A v2.0 or v2.1 collection file, with warnings for what it held that
/// Rustman can't represent.
pub fn import(json: &str) -> Result<ImportOutcome, String> {
    let file: PostmanCollection =
        serde_json::from_str(json).map_err(|e| format!("Invalid Postman JSON: {e}"))?;
    let (collection, warnings) = convert(file);
    Ok(ImportOutcome {
        collections: vec![collection],
        warnings,
    })
}

/// The collection a parsed file describes, plus warnings about what it
/// held that Rustman can't represent.
fn convert(file: PostmanCollection) -> (ImportedCollection, Vec<String>) {
    let collection = Collection::new(file.info.name);
    let mut importer = Importer {
        collection_id: collection.id.clone(),
        warnings: Vec::new(),
    };
    let root_auth = importer.auth(file.auth.as_ref(), &Auth::None, &collection.name);
    // Folders become groups, so the imported tree matches what the user sees
    // in Postman.
    let root = Parent {
        group_id: None,
        auth: &root_auth,
    };
    let (requests, groups) = importer.items(root, &file.item);
    let converted = ImportedCollection {
        collection,
        requests,
        groups,
    };
    (converted, importer.warnings)
}

/// One import run: the collection every request belongs to, at any depth,
/// and the warnings collected on the way.
struct Importer {
    collection_id: String,
    warnings: Vec<String>,
}

/// Where converted items attach, and the auth they inherit from there.
#[derive(Clone, Copy)]
struct Parent<'a> {
    /// The enclosing folder's group id; `None` at the collection root.
    group_id: Option<&'a str>,
    auth: &'a Auth,
}

impl Importer {
    /// One `item` list as (requests, folders). Both share one ordering
    /// sequence, as they do in Postman.
    fn items(
        &mut self,
        parent: Parent<'_>,
        items: &[PostmanItem],
    ) -> (Vec<SavedRequest>, Vec<ImportedGroup>) {
        let mut requests = Vec::new();
        let mut groups = Vec::new();
        for (order, item) in (0_i64..).zip(items) {
            match item {
                PostmanItem::Request(item) => requests.push(self.request(parent, item, order)),
                PostmanItem::Folder(folder) => groups.push(self.folder(parent, folder, order)),
            }
        }
        (requests, groups)
    }

    fn folder(
        &mut self,
        parent: Parent<'_>,
        folder: &PostmanFolder,
        sort_order: i64,
    ) -> ImportedGroup {
        let id = uuid::Uuid::new_v4().to_string();
        let auth = self.auth(folder.auth.as_ref(), parent.auth, &folder.name);
        let inside = Parent {
            group_id: Some(&id),
            auth: &auth,
        };
        let (requests, groups) = self.items(inside, &folder.item);
        ImportedGroup {
            parent_id: parent.group_id.unwrap_or(&self.collection_id).to_owned(),
            parent_is_collection: parent.group_id.is_none(),
            name: folder.name.clone(),
            sort_order,
            requests,
            groups,
            id,
        }
    }

    fn request(
        &mut self,
        parent: Parent<'_>,
        item: &PostmanRequestItem,
        sort_order: i64,
    ) -> SavedRequest {
        let r = &item.request;
        // The group id is what places the request inside its folder: the
        // tree is rebuilt from it after the import is saved.
        let mut req = SavedRequest::new_in_group(
            self.collection_id.clone(),
            parent.group_id.map(str::to_owned),
            item.name.clone(),
            sort_order,
        );
        req.method = match r.method.as_deref().map(str::parse) {
            None => HttpMethod::Get,
            Some(Ok(method)) => method,
            Some(Err(_)) => {
                self.warnings.push(format!(
                    "\"{}\" uses {}, which Rustman can't send, so it was imported as GET",
                    item.name,
                    r.method.as_deref().unwrap_or_default()
                ));
                HttpMethod::Get
            }
        };
        (req.url, req.params) = url_and_params(r.url.as_ref());
        req.headers = r
            .header
            .iter()
            .flatten()
            .map(|h| key_value(&h.key, &h.value, h.disabled))
            .collect();
        ImportedBody::from_postman(r.body.as_ref()).apply_to(&mut req);
        self.auth(r.auth.as_ref(), parent.auth, &item.name)
            .apply_to(&mut req);
        req
    }

    /// The auth `item` ends up with. A Postman item inherits its parent's
    /// auth unless it sets its own, so both a missing block and `inherit`
    /// mean `inherited`.
    fn auth(&mut self, own: Option<&PostmanAuth>, inherited: &Auth, item: &str) -> Auth {
        let Some(own) = own.filter(|a| a.kind != "inherit") else {
            return inherited.clone();
        };
        Auth::from_postman(own).unwrap_or_else(|| {
            self.warnings.push(format!(
                "\"{item}\" uses {} auth, which Rustman can't send, so it was imported without auth",
                own.kind
            ));
            Auth::None
        })
    }
}

/// The URL and its params table. A query given as `url.query` moves into the
/// table and out of the URL, so it isn't sent twice.
fn url_and_params(url: Option<&PostmanUrl>) -> (String, Vec<KeyValue>) {
    let object = match url {
        None => return (String::new(), Vec::new()),
        Some(PostmanUrl::Raw(raw)) => return (raw.clone(), Vec::new()),
        Some(PostmanUrl::Object(object)) => object,
    };
    let raw = object.url();
    let params: Vec<KeyValue> = object
        .query
        .iter()
        .flatten()
        .map(|p| key_value(&p.key, p.value.as_deref().unwrap_or_default(), p.disabled))
        .collect();
    let url = match raw.find('?') {
        Some(i) if !params.is_empty() => raw[..i].to_owned(),
        _ => raw,
    };
    (url, params)
}

fn key_value(key: &str, value: &str, disabled: bool) -> KeyValue {
    KeyValue {
        id: uuid::Uuid::new_v4().to_string(),
        key: key.to_owned(),
        value: value.to_owned(),
        enabled: !disabled,
    }
}

fn form_field(key: &str, value: Option<&str>, disabled: bool) -> FormField {
    FormField {
        id: uuid::Uuid::new_v4().to_string(),
        key: key.to_owned(),
        value: value.unwrap_or_default().to_owned(),
        field_type: FormFieldType::Text,
        enabled: !disabled,
        file_name: None,
        file_data: None,
        mime_type: None,
    }
}

/// A `formdata` row. A file field keeps the path it named as its file name;
/// its content isn't read, so it has to be picked again before sending.
fn formdata_field(f: &PostmanFormField) -> FormField {
    let file = f.field_type == "file";
    FormField {
        field_type: if file {
            FormFieldType::File
        } else {
            FormFieldType::Text
        },
        file_name: if file { f.file_name() } else { None },
        mime_type: f.content_type.clone(),
        ..form_field(&f.key, f.value.as_deref(), f.disabled)
    }
}

/// The body fields of a `SavedRequest`, as one Postman body fills them.
struct ImportedBody {
    body_type: BodyType,
    text: String,
    graphql_variables: String,
    form_fields: Vec<FormField>,
}

impl ImportedBody {
    fn new(body_type: BodyType, text: String) -> Self {
        Self {
            body_type,
            text,
            graphql_variables: String::new(),
            form_fields: Vec::new(),
        }
    }

    fn form(form_fields: Vec<FormField>) -> Self {
        Self {
            form_fields,
            ..Self::new(BodyType::FormData, String::new())
        }
    }

    fn from_postman(body: Option<&PostmanBody>) -> Self {
        let Some(body) = body else {
            return Self::new(BodyType::None, String::new());
        };
        match body.mode.as_deref() {
            Some("raw") => {
                let text = body.raw.clone().unwrap_or_default();
                let language = body
                    .options
                    .as_ref()
                    .and_then(|o| o.raw.as_ref())
                    .and_then(|r| r.language.as_deref());
                Self::new(raw_body_type(language, &text), text)
            }
            Some("graphql") => Self::graphql(body.graphql.as_ref()),
            Some("formdata") => {
                Self::form(body.formdata.iter().flatten().map(formdata_field).collect())
            }
            // Rustman sends a form without file fields as
            // application/x-www-form-urlencoded, so this keeps the request as is.
            Some("urlencoded") => Self::form(
                body.urlencoded
                    .iter()
                    .flatten()
                    .map(|p| form_field(&p.key, p.value.as_deref(), p.disabled))
                    .collect(),
            ),
            _ => Self::new(BodyType::None, String::new()),
        }
    }

    fn graphql(graphql: Option<&PostmanGraphQl>) -> Self {
        let query = graphql.and_then(|g| g.query.clone()).unwrap_or_default();
        let graphql_variables = match graphql.and_then(|g| g.variables.as_ref()) {
            // Postman stores variables as a string in its own exports
            // and as an object in newer ones; both become editor text.
            Some(Value::String(text)) => text.clone(),
            Some(object @ Value::Object(_)) => {
                serde_json::to_string_pretty(object).expect("a JSON value always serializes")
            }
            _ => String::new(),
        };
        Self {
            graphql_variables,
            ..Self::new(BodyType::GraphQL, query)
        }
    }

    fn apply_to(self, req: &mut SavedRequest) {
        req.body_type = self.body_type;
        req.body = self.text;
        req.graphql_variables = self.graphql_variables;
        req.form_data_fields = self.form_fields;
    }
}

/// A raw body's type from Postman's `language`. Every language but `json`
/// (text, xml, html, javascript) is text Rustman sends as is. Older exports
/// have no language, so then the text itself decides.
fn raw_body_type(language: Option<&str>, text: &str) -> BodyType {
    let is_json = match language {
        Some(language) => language == "json",
        None => is_json_document(text),
    };
    if is_json {
        BodyType::Json
    } else {
        BodyType::Text
    }
}

/// A request's auth once Postman's inheritance is resolved, limited to the
/// schemes Rustman can send.
#[derive(Debug, Clone)]
enum Auth {
    None,
    Bearer {
        token: String,
    },
    Basic {
        user: String,
        pass: String,
    },
    ApiKey {
        name: String,
        value: String,
        location: ApiKeyLocation,
    },
    Jwt {
        secret: String,
        subject: String,
    },
}

impl Auth {
    /// `auth` as Rustman sends it, or `None` for a scheme it can't send
    /// (oauth2, digest, awsv4, …).
    fn from_postman(auth: &PostmanAuth) -> Option<Self> {
        let setting = |settings: &Option<AuthSettings>, key: &str| {
            settings
                .as_ref()
                .and_then(|s| s.get(key))
                .unwrap_or_default()
        };
        match auth.kind.as_str() {
            "noauth" => Some(Self::None),
            "bearer" => Some(Self::Bearer {
                token: setting(&auth.bearer, "token"),
            }),
            "basic" => Some(Self::Basic {
                user: setting(&auth.basic, "username"),
                pass: setting(&auth.basic, "password"),
            }),
            "apikey" => Some(Self::ApiKey {
                name: setting(&auth.apikey, "key"),
                value: setting(&auth.apikey, "value"),
                location: if setting(&auth.apikey, "in") == "query" {
                    ApiKeyLocation::Query
                } else {
                    ApiKeyLocation::Header
                },
            }),
            "jwt" => jwt_from_postman(auth.jwt.as_ref()),
            _ => None,
        }
    }

    fn apply_to(self, req: &mut SavedRequest) {
        req.auth_type = match self {
            Self::None => AuthType::None,
            Self::Bearer { token } => {
                req.bearer_token = token;
                AuthType::Bearer
            }
            Self::Basic { user, pass } => {
                req.basic_user = user;
                req.basic_pass = pass;
                AuthType::Basic
            }
            Self::ApiKey {
                name,
                value,
                location,
            } => {
                req.api_key_name = name;
                req.api_key_value = value;
                req.api_key_location = location;
                AuthType::ApiKey
            }
            Self::Jwt { secret, subject } => {
                req.jwt_secret = secret;
                req.jwt_subject = subject;
                AuthType::JwtUser
            }
        };
    }
}

/// Postman's "JWT Bearer" scheme, when it is in the one shape Rustman signs:
/// HS256 with a plain-text secret, sent as `Authorization: Bearer`, with no
/// extra JWT headers and no claim besides `sub`. (`iat` and `exp` may be
/// there too: Rustman sets those itself when it sends.)
fn jwt_from_postman(settings: Option<&AuthSettings>) -> Option<Auth> {
    let setting = |key: &str, default: &str| {
        settings
            .and_then(|s| s.get(key))
            .unwrap_or_else(|| default.to_owned())
    };
    let signs_like_rustman = setting("algorithm", "HS256") == "HS256"
        && setting("isSecretBase64Encoded", "false") == "false"
        && setting("addTokenTo", "header") == "header"
        && setting("headerPrefix", "Bearer") == "Bearer"
        && json_object(&setting("header", ""))?.is_empty();
    if !signs_like_rustman {
        return None;
    }
    let claims = json_object(&setting("payload", ""))?;
    if claims
        .keys()
        .any(|claim| !matches!(claim.as_str(), "sub" | "iat" | "exp"))
    {
        return None;
    }
    let subject = match claims.get("sub") {
        None => String::new(),
        Some(Value::String(subject)) => subject.clone(),
        Some(_) => return None,
    };
    Some(Auth::Jwt {
        secret: setting("secret", ""),
        subject,
    })
}

/// `text` as a JSON object; blank text is an empty one.
fn json_object(text: &str) -> Option<serde_json::Map<String, Value>> {
    if text.trim().is_empty() {
        return Some(serde_json::Map::new());
    }
    serde_json::from_str(text).ok()
}

// ── Export ────────────────────────────────────────────────────────────────────

/// The collection as a Postman v2.1 file.
pub fn export(imported: &ImportedCollection, options: ExportOptions) -> String {
    let imported = options.apply(imported);
    let col = json!({
        "info": {
            "_postman_id": imported.collection.id,
            "name": imported.collection.name,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
        },
        "item": export_items(&imported.groups, &imported.requests),
    });

    serde_json::to_string_pretty(&col).expect("a JSON value always serializes")
}

/// One parent's children as a Postman `item` list: folders first, then
/// requests, in the order the sidebar shows them.
fn export_items(groups: &[ImportedGroup], requests: &[SavedRequest]) -> Vec<Value> {
    groups
        .iter()
        .map(export_group)
        .chain(requests.iter().map(export_request))
        .collect()
}

fn export_group(group: &ImportedGroup) -> Value {
    json!({ "name": group.name, "item": export_items(&group.groups, &group.requests) })
}

fn export_request(req: &SavedRequest) -> Value {
    let mut headers: Vec<Value> = req
        .headers
        .iter()
        .map(|h| json!({ "key": h.key, "value": h.value, "disabled": !h.enabled }))
        .collect();
    if req.auth_type == AuthType::Cookie && !req.cookie_string.is_empty() {
        // Postman has no cookie auth. The header is what Rustman sends for
        // it, so the exported request still carries the session.
        headers.push(json!({ "key": "Cookie", "value": req.cookie_string, "disabled": false }));
    }

    json!({
        "name": req.name,
        "request": {
            "auth": export_auth(req),
            "method": req.method.as_str(),
            "header": headers,
            "url": export_url(req),
            "body": export_body(req),
        }
    })
}

/// `url.raw` holds only the enabled params, so the `query` list is where a
/// disabled one survives the trip.
fn export_url(req: &SavedRequest) -> Value {
    let (raw, params) = reconcile_url_params(&req.url, &req.params);
    let query: Vec<Value> = params
        .iter()
        .map(|p| json!({ "key": p.key, "value": p.value, "disabled": !p.enabled }))
        .collect();
    if query.is_empty() {
        json!({ "raw": raw })
    } else {
        json!({ "raw": raw, "query": query })
    }
}

fn export_body(req: &SavedRequest) -> Value {
    match req.body_type {
        BodyType::Json => raw_body(&req.body, "json"),
        BodyType::Text => raw_body(&req.body, "text"),
        BodyType::FormData => export_form(&req.form_data_fields),
        BodyType::GraphQL => json!({
            "mode": "graphql",
            "graphql": {
                "query": req.body,
                "variables": req.graphql_variables,
            },
        }),
        BodyType::None => json!({ "mode": "none" }),
    }
}

fn raw_body(text: &str, language: &str) -> Value {
    json!({
        "mode": "raw",
        "raw": text,
        "options": { "raw": { "language": language } },
    })
}

/// A form with no file field goes out as `urlencoded`, which is how Rustman
/// sends it. Any file field, even a disabled one, makes it `formdata`, since
/// `urlencoded` has no way to keep that field.
fn export_form(fields: &[FormField]) -> Value {
    if !fields.iter().any(|f| f.field_type == FormFieldType::File) {
        let rows: Vec<Value> = fields
            .iter()
            .map(|f| json!({ "key": f.key, "value": f.value, "disabled": !f.enabled }))
            .collect();
        return json!({ "mode": "urlencoded", "urlencoded": rows });
    }
    let rows: Vec<Value> = fields
        .iter()
        .map(|f| {
            let field_type = match f.field_type {
                FormFieldType::File => "file",
                FormFieldType::Text => "text",
            };
            let mut row =
                json!({ "key": f.key, "value": f.value, "type": field_type, "disabled": !f.enabled });
            if f.field_type == FormFieldType::File
                && let Some(name) = &f.file_name
            {
                row["src"] = json!(name);
            }
            if let Some(mime) = &f.mime_type {
                row["contentType"] = json!(mime);
            }
            row
        })
        .collect();
    json!({ "mode": "formdata", "formdata": rows })
}

/// The request's auth as a Postman `auth` block. Rustman has no auth
/// inheritance, so "no auth" is written as `noauth` instead of being left
/// for Postman to fill from a parent.
fn export_auth(req: &SavedRequest) -> Value {
    let setting = |key: &str, value: &str| json!({ "key": key, "value": value, "type": "string" });
    match req.auth_type {
        // Cookie auth travels as a header; see `export_request`.
        AuthType::None | AuthType::Cookie => json!({ "type": "noauth" }),
        AuthType::Bearer => json!({
            "type": "bearer",
            "bearer": [setting("token", &req.bearer_token)],
        }),
        AuthType::Basic => json!({
            "type": "basic",
            "basic": [
                setting("username", &req.basic_user),
                setting("password", &req.basic_pass),
            ],
        }),
        AuthType::ApiKey => json!({
            "type": "apikey",
            "apikey": [
                setting("key", &req.api_key_name),
                setting("value", &req.api_key_value),
                setting("in", req.api_key_location.as_str()),
            ],
        }),
        // The JWT Rustman signs: HS256, `sub` as the only claim it is given,
        // sent as a bearer token.
        AuthType::JwtUser => json!({
            "type": "jwt",
            "jwt": [
                setting("algorithm", "HS256"),
                setting("secret", &req.jwt_secret),
                { "key": "isSecretBase64Encoded", "value": false, "type": "boolean" },
                setting("payload", &json!({ "sub": req.jwt_subject }).to_string()),
                setting("addTokenTo", "header"),
                setting("headerPrefix", "Bearer"),
            ],
        }),
    }
}

#[cfg(test)]
mod import_export_tests {
    use super::*;
    use rustman_testkit::{blank_request, form_fields, request_variants, sample_collection};

    /// Round trips compare every field, credentials included.
    const WITH_SECRETS: ExportOptions = ExportOptions {
        include_secrets: true,
    };

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn import_one(text: &str) -> ImportedCollection {
        let mut collections = import(text).expect("imports").collections;
        assert_eq!(collections.len(), 1);
        collections.remove(0)
    }

    fn storefront() -> ImportedCollection {
        import_one(&fixture("postman-v2.1-storefront.json"))
    }

    fn group<'a>(groups: &'a [ImportedGroup], name: &str) -> &'a ImportedGroup {
        groups
            .iter()
            .find(|g| g.name == name)
            .unwrap_or_else(|| panic!("no folder {name}"))
    }

    fn request<'a>(ic: &'a ImportedCollection, name: &str) -> &'a SavedRequest {
        ic.all_requests()
            .into_iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("no request {name}"))
    }

    fn names(requests: &[SavedRequest]) -> Vec<(&str, i64)> {
        requests
            .iter()
            .map(|r| (r.name.as_str(), r.sort_order))
            .collect()
    }

    fn kvs(rows: &[KeyValue]) -> Vec<(&str, &str, bool)> {
        rows.iter()
            .map(|r| (r.key.as_str(), r.value.as_str(), r.enabled))
            .collect()
    }

    /// The URL and params a tab shows once it opens `r`. The import moves a
    /// query into the params table, so this is how two requests compare.
    fn as_opened(r: &SavedRequest) -> (String, Vec<(String, String, bool)>) {
        let (url, params) = reconcile_url_params(&r.url, &r.params);
        let params = params
            .into_iter()
            .map(|p| (p.key, p.value, p.enabled))
            .collect();
        (url, params)
    }

    fn form_rows(r: &SavedRequest) -> Vec<(String, String, FormFieldType, bool)> {
        r.form_data_fields
            .iter()
            .map(|f| {
                (
                    f.key.clone(),
                    f.value.clone(),
                    f.field_type.clone(),
                    f.enabled,
                )
            })
            .collect()
    }

    /// A collection holding only `req`.
    fn collection_of(req: SavedRequest) -> ImportedCollection {
        ImportedCollection {
            collection: sample_collection().collection,
            requests: vec![req],
            groups: Vec::new(),
        }
    }

    /// `req` exported alone and imported back.
    fn round_trip(req: SavedRequest) -> SavedRequest {
        import_one(&export(&collection_of(req), WITH_SECRETS))
            .requests
            .remove(0)
    }

    // ── v2.1 ──────────────────────────────────────────────────────────────

    #[test]
    fn v2_1_folders_become_nested_groups_in_file_order() {
        let ic = storefront();
        assert_eq!(ic.collection.name, "Storefront API");

        let top: Vec<(&str, i64)> = ic
            .groups
            .iter()
            .map(|g| (g.name.as_str(), g.sort_order))
            .collect();
        assert_eq!(top, [("Auth", 0), ("Catalog", 1)]);
        // Folders and requests share one ordering sequence per parent.
        assert_eq!(
            names(&ic.requests),
            [("Update cart note", 2), ("Delete cart", 3)]
        );

        let auth = group(&ic.groups, "Auth");
        assert_eq!(auth.parent_id, ic.collection.id);
        assert!(auth.parent_is_collection);
        assert_eq!(names(&auth.requests), [("Login", 0), ("Refresh token", 1)]);

        let catalog = group(&ic.groups, "Catalog");
        assert_eq!(names(&catalog.requests), [("Health", 1)]);
        let products = group(&catalog.groups, "Products");
        assert_eq!(products.parent_id, catalog.id);
        assert!(!products.parent_is_collection);
        assert_eq!(products.sort_order, 0);
        assert_eq!(
            names(&products.requests),
            [
                ("List products", 0),
                ("Upload product image", 1),
                ("Search (GraphQL)", 2)
            ]
        );

        assert_eq!(ic.all_requests().len(), 8);
        assert!(
            ic.all_requests()
                .iter()
                .all(|r| r.collection_id == ic.collection.id)
        );
        let mut ids: Vec<&str> = ic.all_groups().iter().map(|g| g.id.as_str()).collect();
        ids.push(&ic.collection.id);
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 4, "collection and group ids are distinct");
    }

    #[test]
    fn v2_1_methods_urls_query_params_and_headers() {
        let ic = storefront();

        let list = request(&ic, "List products");
        assert_eq!(list.method, HttpMethod::Get);
        // The query moves into `params`, so it isn't sent twice.
        assert_eq!(list.url, "{{base_url}}/products");
        assert_eq!(
            kvs(&list.params),
            [
                ("page", "2", true),
                ("limit", "20", true),
                ("sort", "price", false)
            ]
        );
        assert_eq!(
            kvs(&list.headers),
            [
                ("Accept", "application/json", true),
                ("X-Debug", "1", false)
            ]
        );

        assert_eq!(
            request(&ic, "Health").url,
            "{{base_url}}/health",
            "string URL"
        );
        assert!(request(&ic, "Health").params.is_empty());
        assert_eq!(request(&ic, "Delete cart").method, HttpMethod::Delete);
        assert_eq!(request(&ic, "Update cart note").method, HttpMethod::Put);
        assert_eq!(
            request(&ic, "Upload product image").url,
            "{{base_url}}/products/:id/images",
            "path variables stay in the URL"
        );
    }

    #[test]
    fn v2_1_body_modes() {
        let ic = storefront();

        let login = request(&ic, "Login");
        assert_eq!(login.body_type, BodyType::Json);
        assert_eq!(
            login.body,
            "{\n  \"email\": \"{{email}}\",\n  \"password\": \"{{password}}\"\n}"
        );

        let upload = request(&ic, "Upload product image");
        assert_eq!(upload.body_type, BodyType::FormData);
        let fields: Vec<(&str, &str, &FormFieldType, bool)> = upload
            .form_data_fields
            .iter()
            .map(|f| (f.key.as_str(), f.value.as_str(), &f.field_type, f.enabled))
            .collect();
        assert_eq!(
            fields,
            [
                ("caption", "Front view", &FormFieldType::Text, true),
                ("image", "", &FormFieldType::File, true),
                ("draft", "true", &FormFieldType::Text, false),
            ]
        );
        assert!(
            upload
                .form_data_fields
                .iter()
                .all(|f| f.file_data.is_none())
        );

        let search = request(&ic, "Search (GraphQL)");
        assert_eq!(search.body_type, BodyType::GraphQL);
        assert_eq!(
            search.body,
            "query Search($q: String!) {\n  search(q: $q) { id name }\n}"
        );
        assert_eq!(search.graphql_variables, "{\n  \"q\": \"lamp\"\n}");

        let delete = request(&ic, "Delete cart");
        assert_eq!(
            (delete.body_type.clone(), delete.body.as_str()),
            (BodyType::None, "")
        );
    }

    #[test]
    fn v2_1_variable_placeholders_survive_verbatim() {
        let ic = storefront();
        for r in ic.all_requests() {
            assert!(r.url.starts_with("{{base_url}}/"), "{}: {}", r.name, r.url);
        }
        assert!(request(&ic, "Login").body.contains("{{password}}"));
    }

    #[test]
    fn graphql_variables_given_as_an_object_become_editor_text() {
        let text = r#"{
            "info": {"name": "G", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
            "item": [{
                "name": "Q",
                "request": {
                    "method": "POST",
                    "url": "https://example.invalid/graphql",
                    "body": {"mode": "graphql", "graphql": {"query": "{ me }", "variables": {"id": 7}}}
                }
            }]
        }"#;
        let ic = import_one(text);
        assert_eq!(ic.requests[0].graphql_variables, "{\n  \"id\": 7\n}");
    }

    // ── v2.0 ──────────────────────────────────────────────────────────────

    #[test]
    fn v2_0_collection_imports() {
        let ic = import_one(&fixture("postman-v2.0-users.json"));
        assert_eq!(ic.collection.name, "Legacy Users API");

        let users = group(&ic.groups, "Users");
        assert_eq!(users.sort_order, 0);
        assert_eq!(
            names(&users.requests),
            [("Get user", 0), ("Create user", 1)]
        );
        assert_eq!(
            names(&ic.requests),
            [("Search users", 1), ("Upload avatar", 2)]
        );

        let get = request(&ic, "Get user");
        assert_eq!(get.method, HttpMethod::Get);
        assert_eq!(get.url, "https://api.example.com/users/1");
        assert_eq!(kvs(&get.headers), [("Accept", "application/json", true)]);
        assert_eq!(
            get.body_type,
            BodyType::None,
            "an empty body object is no body"
        );

        let create = request(&ic, "Create user");
        assert_eq!(create.method, HttpMethod::Post);
        assert_eq!(
            (create.body_type.clone(), create.body.as_str()),
            (BodyType::Json, "{\"name\": \"Ada\"}")
        );

        let search = request(&ic, "Search users");
        assert_eq!(search.url, "https://api.example.com/users");
        assert_eq!(
            kvs(&search.params),
            [("q", "ada", true), ("active", "true", true)]
        );

        let upload = request(&ic, "Upload avatar");
        assert_eq!(upload.body_type, BodyType::FormData);
        let types: Vec<(&str, &FormFieldType, bool)> = upload
            .form_data_fields
            .iter()
            .map(|f| (f.key.as_str(), &f.field_type, f.enabled))
            .collect();
        assert_eq!(
            types,
            [
                ("avatar", &FormFieldType::File, true),
                ("note", &FormFieldType::Text, true)
            ]
        );
    }

    // ── Export → import ───────────────────────────────────────────────────

    #[test]
    fn export_then_import_keeps_the_tree_and_each_request() {
        let original = sample_collection();
        let back = import_one(&export(&original, WITH_SECRETS));

        assert_eq!(back.collection.name, original.collection.name);
        let tree = |ic: &ImportedCollection| -> Vec<(String, Vec<String>)> {
            ic.all_groups()
                .iter()
                .map(|g| {
                    let mut children: Vec<String> =
                        g.groups.iter().map(|c| c.name.clone()).collect();
                    children.extend(g.requests.iter().map(|r| r.name.clone()));
                    (g.name.clone(), children)
                })
                .collect()
        };
        assert_eq!(tree(&back), tree(&original));
        assert_eq!(back.requests.len(), original.requests.len());

        for want in original.all_requests() {
            let got = request(&back, &want.name);
            assert_eq!(got.method, want.method, "{}", want.name);
            assert_eq!(as_opened(got), as_opened(want), "{}", want.name);
            assert_eq!(kvs(&got.headers), kvs(&want.headers), "{}", want.name);
            assert_eq!(got.body, want.body, "{}", want.name);
            assert_eq!(
                got.graphql_variables, want.graphql_variables,
                "{}",
                want.name
            );
            assert_eq!(got.body_type, want.body_type, "{}", want.name);
            // Only the active body is exported, so form fields only travel
            // with form-data requests.
            if want.body_type == BodyType::FormData {
                assert_eq!(form_rows(got), form_rows(want), "{}", want.name);
            }
        }
    }

    // ── Malformed input ───────────────────────────────────────────────────

    fn import_error(text: &str) -> String {
        match import(text) {
            Ok(_) => panic!("expected an error for {text:?}"),
            Err(e) => {
                assert!(e.starts_with("Invalid Postman JSON: "), "{e}");
                e
            }
        }
    }

    #[test]
    fn malformed_files_are_rejected_with_a_useful_error() {
        let e = import_error("");
        assert!(e.contains("EOF"), "{e}");

        let e = import_error("hello");
        assert!(e.contains("expected value at line 1 column 1"), "{e}");

        let full = fixture("postman-v2.1-storefront.json");
        let e = import_error(&full[..full.len() / 2]);
        assert!(e.contains("EOF while parsing"), "{e}");

        let e = import_error(r#"{"item": []}"#);
        assert!(e.contains("missing field `info`"), "{e}");

        // A Postman v1 collection is a different format altogether.
        let e = import_error(
            r#"{"id": "x", "name": "Old", "order": [], "folders": [], "requests": []}"#,
        );
        assert!(e.contains("missing field `info`"), "{e}");

        let e = import_error(r#"{"info": {"schema": "x"}, "item": []}"#);
        assert!(e.contains("missing field `name`"), "{e}");

        let e = import_error(r#"{"info": {"name": "x"}, "item": 5}"#);
        assert!(e.contains("invalid type: integer `5`"), "{e}");

        let e = import_error(
            r#"{"info": {"name": "x"}, "item": [{"name": "neither folder nor request"}]}"#,
        );
        assert!(
            e.contains(
                "item \"neither folder nor request\" is neither a request nor a folder: \
                 it has no \"request\" or \"item\""
            ),
            "{e}"
        );
        assert!(e.contains("line 1 column"), "points at the item: {e}");

        let e = import_error(
            r#"{"info": {"name": "x"}, "item": [{"name": "Bad headers", "request": {"header": 5}}]}"#,
        );
        assert!(
            e.contains(
                "request \"Bad headers\" is invalid: invalid type: integer `5`, expected a sequence"
            ),
            "{e}"
        );
        assert!(e.contains("line 1 column"), "points at the item: {e}");

        let e = import_error(
            r#"{"info": {"name": "x"}, "item": [{"name": "Admin", "item": [], "auth": {"basic": []}}]}"#,
        );
        assert!(
            e.contains("folder \"Admin\" has invalid auth: missing field `type`"),
            "{e}"
        );

        let e = import_error(r#"{"info": {"name": "x"}, "item": [5]}"#);
        assert!(
            e.contains("expected a Postman item (a request or a folder)"),
            "{e}"
        );

        // A Rustman export is not mistaken for a Postman collection.
        let ic = sample_collection();
        let native = crate::import::native_export(&ic, ExportOptions::default());
        import_error(&native);
    }

    // ── Regression guards ─────────────────────────────────────────────────

    #[test]
    fn requests_inside_folders_carry_their_folder_group_id() {
        // The app rebuilds the tree from `group_id` and `parent_id` once the
        // import is saved, so those must match the nesting.
        let ic = storefront();
        assert!(ic.requests.iter().all(|r| r.group_id.is_none()));
        for g in &ic.groups {
            assert_eq!(
                (g.parent_id.as_str(), g.parent_is_collection),
                (ic.collection.id.as_str(), true),
                "{}",
                g.name
            );
        }
        for g in ic.all_groups() {
            for r in &g.requests {
                assert_eq!(
                    r.group_id.as_deref(),
                    Some(g.id.as_str()),
                    "{} in {}",
                    r.name,
                    g.name
                );
            }
            for child in &g.groups {
                assert_eq!(
                    (child.parent_id.as_str(), child.parent_is_collection),
                    (g.id.as_str(), false),
                    "{} in {}",
                    child.name,
                    g.name
                );
            }
        }
    }

    #[test]
    fn request_auth_is_imported() {
        let ic = storefront();
        let bearer = request(&ic, "Update cart note");
        assert_eq!(
            (bearer.auth_type.clone(), bearer.bearer_token.as_str()),
            (AuthType::Bearer, "{{token}}")
        );
        let basic = request(&ic, "Refresh token");
        assert_eq!(basic.auth_type, AuthType::Basic);
        assert_eq!(
            (basic.basic_user.as_str(), basic.basic_pass.as_str()),
            ("storefront-web", "client-secret")
        );
        let api_key = request(&ic, "Upload product image");
        assert_eq!(api_key.auth_type, AuthType::ApiKey);
        assert_eq!(
            (
                api_key.api_key_name.as_str(),
                api_key.api_key_value.as_str()
            ),
            ("X-Api-Key", "{{api_key}}")
        );
        assert_eq!(api_key.api_key_location, ApiKeyLocation::Header);

        let original = sample_collection();
        let back = import_one(&export(&original, WITH_SECRETS));
        for want in original.all_requests() {
            assert_eq!(
                request(&back, &want.name).auth_type,
                want.auth_type,
                "{}",
                want.name
            );
        }
    }

    #[test]
    fn raw_text_body_keeps_its_type() {
        assert_eq!(
            request(&storefront(), "Update cart note").body_type,
            BodyType::Text
        );

        let original = sample_collection();
        let text = original
            .requests
            .iter()
            .find(|r| r.body_type == BodyType::Text)
            .unwrap();
        let back = import_one(&export(&original, WITH_SECRETS));
        assert_eq!(request(&back, &text.name).body_type, BodyType::Text);
    }

    #[test]
    fn export_keeps_disabled_params_and_form_fields() {
        let original = sample_collection();
        let by_id = |id: &str| -> SavedRequest {
            original
                .all_requests()
                .into_iter()
                .find(|r| r.id == id)
                .unwrap()
                .clone()
        };
        let back = import_one(&export(&original, WITH_SECRETS));

        let upload = by_id("req-patch");
        let enabled = |r: &SavedRequest| -> Vec<(String, bool)> {
            r.form_data_fields
                .iter()
                .map(|f| (f.key.clone(), f.enabled))
                .collect()
        };
        assert_eq!(
            enabled(request(&back, &upload.name)),
            enabled(&upload),
            "a disabled field must not be sent after a round trip"
        );

        let full = by_id("req-full");
        let got = request(&back, &full.name);
        assert!(
            got.params.iter().any(|p| p.key == "draft" && !p.enabled),
            "disabled param lost: {:?}",
            kvs(&got.params)
        );
    }

    #[test]
    fn request_given_as_a_url_string_imports() {
        let text = r#"{
            "info": {"name": "Short form", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
            "item": [{"name": "Ping", "request": "https://example.invalid/ping"}]
        }"#;
        let ic = import_one(text);
        assert_eq!(ic.requests[0].method, HttpMethod::Get);
        assert_eq!(ic.requests[0].url, "https://example.invalid/ping");
    }

    // ── Auth ──────────────────────────────────────────────────────────────

    /// A request's auth type and the settings that type sends.
    fn active_auth(r: &SavedRequest) -> (AuthType, Vec<&str>) {
        let settings = match r.auth_type {
            AuthType::None => vec![],
            AuthType::Bearer => vec![r.bearer_token.as_str()],
            AuthType::Basic => vec![r.basic_user.as_str(), r.basic_pass.as_str()],
            AuthType::ApiKey => vec![
                r.api_key_name.as_str(),
                r.api_key_value.as_str(),
                r.api_key_location.as_str(),
            ],
            AuthType::JwtUser => vec![
                r.jwt_secret.as_str(),
                r.jwt_subject.as_str(),
                r.jwt_algo.as_str(),
            ],
            AuthType::Cookie => vec![r.cookie_string.as_str()],
        };
        (r.auth_type.clone(), settings)
    }

    #[test]
    fn every_auth_postman_can_hold_survives_export_and_import() {
        let mut covered = Vec::new();
        for want in request_variants() {
            if want.auth_type == AuthType::Cookie {
                continue; // Postman has no cookie auth: see the next test.
            }
            let got = round_trip(want.clone());
            assert_eq!(active_auth(&got), active_auth(&want), "{}", want.name);
            covered.push(active_auth(&want).0);
        }
        for auth in [
            AuthType::None,
            AuthType::Bearer,
            AuthType::Basic,
            AuthType::ApiKey,
            AuthType::JwtUser,
        ] {
            assert!(covered.contains(&auth), "{auth:?} not exercised");
        }
        let locations: Vec<ApiKeyLocation> = request_variants()
            .into_iter()
            .filter(|r| r.auth_type == AuthType::ApiKey)
            .map(|r| r.api_key_location)
            .collect();
        assert!(locations.contains(&ApiKeyLocation::Header));
        assert!(locations.contains(&ApiKeyLocation::Query));
    }

    #[test]
    fn cookie_auth_is_exported_as_the_cookie_header_it_sends() {
        let want = request_variants()
            .into_iter()
            .find(|r| r.auth_type == AuthType::Cookie)
            .unwrap();
        let got = round_trip(want.clone());
        assert_eq!(got.auth_type, AuthType::None);
        assert_eq!(kvs(&got.headers), [("Cookie", "sid=xyz", true)]);
    }

    #[test]
    fn auth_is_inherited_from_the_collection_and_folders() {
        let ic = storefront();
        let delete = request(&ic, "Delete cart");
        assert_eq!(
            (delete.auth_type.clone(), delete.bearer_token.as_str()),
            (AuthType::Bearer, "{{token}}"),
            "no auth of its own: the collection's applies"
        );
        assert_eq!(request(&ic, "Health").auth_type, AuthType::Bearer);
        assert_eq!(
            request(&ic, "Login").auth_type,
            AuthType::None,
            "noauth overrides the collection"
        );

        let text = r#"{
            "info": {"name": "Inheritance"},
            "auth": {"type": "bearer", "bearer": [{"key": "token", "value": "root-token"}]},
            "item": [
                {
                    "name": "Admin",
                    "auth": {"type": "basic", "basic": [
                        {"key": "username", "value": "admin"},
                        {"key": "password", "value": "pw"}
                    ]},
                    "item": [
                        {"name": "Silent", "request": {"url": "https://example.invalid/a"}},
                        {"name": "Explicit", "request": {"url": "https://example.invalid/b", "auth": {"type": "inherit"}}},
                        {"name": "Public", "auth": {"type": "noauth"}, "item": [
                            {"name": "Open", "request": "https://example.invalid/c"}
                        ]}
                    ]
                },
                {"name": "Root", "request": {"url": "https://example.invalid/d", "auth": null}}
            ]
        }"#;
        let ic = import_one(text);
        for name in ["Silent", "Explicit"] {
            assert_eq!(
                active_auth(request(&ic, name)),
                (AuthType::Basic, vec!["admin", "pw"]),
                "{name}"
            );
        }
        assert_eq!(request(&ic, "Open").auth_type, AuthType::None);
        assert_eq!(
            active_auth(request(&ic, "Root")),
            (AuthType::Bearer, vec!["root-token"])
        );
    }

    #[test]
    fn v2_0_auth_settings_given_as_an_object_import() {
        let ic = import_one(&fixture("postman-v2.0-users.json"));
        assert_eq!(
            active_auth(request(&ic, "Create user")),
            (AuthType::Basic, vec!["admin", "hunter2"])
        );
        assert_eq!(request(&ic, "Get user").auth_type, AuthType::None);
    }

    #[test]
    fn a_postman_jwt_rustman_can_sign_imports_as_jwt_auth() {
        let text = r#"{
            "info": {"name": "JWT"},
            "item": [{"name": "Me", "request": {"url": "https://example.invalid/me", "auth": {
                "type": "jwt",
                "jwt": [
                    {"key": "secret", "value": "{{jwt_secret}}"},
                    {"key": "payload", "value": "{\"sub\": \"ada\", \"iat\": 1}"},
                    {"key": "algorithm", "value": "HS256"},
                    {"key": "isSecretBase64Encoded", "value": false},
                    {"key": "addTokenTo", "value": "header"},
                    {"key": "headerPrefix", "value": "Bearer"},
                    {"key": "queryParamKey", "value": "token"},
                    {"key": "header", "value": "{}"}
                ]
            }}}]
        }"#;
        assert_eq!(
            active_auth(&import_one(text).requests[0]),
            (AuthType::JwtUser, vec!["{{jwt_secret}}", "ada", "HS256"])
        );
    }

    #[test]
    fn auth_rustman_cannot_send_imports_as_none_with_a_warning() {
        let text = r#"{
            "info": {"name": "Odd auth"},
            "item": [
                {"name": "OAuth", "request": {"url": "https://example.invalid", "auth": {
                    "type": "oauth2", "oauth2": [{"key": "accessToken", "value": "t"}]
                }}},
                {"name": "Signed", "auth": {"type": "awsv4"}, "item": [
                    {"name": "Inside", "request": "https://example.invalid/s3"}
                ]},
                {"name": "RS256", "request": {"url": "https://example.invalid", "auth": {
                    "type": "jwt", "jwt": [{"key": "algorithm", "value": "RS256"}]
                }}},
                {"name": "Claims", "request": {"url": "https://example.invalid", "auth": {
                    "type": "jwt", "jwt": [{"key": "payload", "value": "{\"sub\": \"a\", \"role\": \"admin\"}"}]
                }}}
            ]
        }"#;
        let (ic, warnings) = convert(serde_json::from_str(text).unwrap());
        assert_eq!(ic.all_requests().len(), 4);
        assert!(
            ic.all_requests()
                .iter()
                .all(|r| r.auth_type == AuthType::None)
        );
        let dropped = |item: &str, kind: &str| {
            format!(
                "\"{item}\" uses {kind} auth, which Rustman can't send, so it was imported without auth"
            )
        };
        assert_eq!(
            warnings,
            [
                dropped("OAuth", "oauth2"),
                dropped("Signed", "awsv4"),
                dropped("RS256", "jwt"),
                dropped("Claims", "jwt"),
            ]
        );
    }

    // ── Bodies ────────────────────────────────────────────────────────────

    #[test]
    fn raw_body_type_follows_the_language_then_the_text() {
        assert_eq!(raw_body_type(Some("json"), "{{payload}}"), BodyType::Json);
        for language in ["text", "xml", "html", "javascript"] {
            assert_eq!(
                raw_body_type(Some(language), "{\"a\": 1}"),
                BodyType::Text,
                "{language}"
            );
        }
        assert_eq!(raw_body_type(None, "{\"a\": 1}"), BodyType::Json);
        assert_eq!(raw_body_type(None, "[1, 2]"), BodyType::Json);
        assert_eq!(raw_body_type(None, "<note>hi</note>"), BodyType::Text);
        assert_eq!(raw_body_type(None, ""), BodyType::Text);
    }

    #[test]
    fn urlencoded_bodies_import_as_text_form_fields() {
        let ic = storefront();
        let refresh = request(&ic, "Refresh token");
        assert_eq!(refresh.body_type, BodyType::FormData);
        assert_eq!(
            form_rows(refresh),
            [
                (
                    "grant_type".to_owned(),
                    "refresh_token".to_owned(),
                    FormFieldType::Text,
                    true
                ),
                (
                    "refresh_token".to_owned(),
                    "{{refresh_token}}".to_owned(),
                    FormFieldType::Text,
                    true
                ),
            ]
        );
    }

    #[test]
    fn forms_without_file_fields_round_trip_as_urlencoded() {
        let mut login = blank_request("req-login", "Log in");
        login.method = HttpMethod::Post;
        login.body_type = BodyType::FormData;
        let text_field = form_fields().remove(0);
        let mut off = text_field.clone();
        off.key = "remember".into();
        off.enabled = false;
        login.form_data_fields = vec![text_field, off];

        let exported: Value =
            serde_json::from_str(&export(&collection_of(login.clone()), WITH_SECRETS)).unwrap();
        assert_eq!(exported["item"][0]["request"]["body"]["mode"], "urlencoded");

        let got = round_trip(login.clone());
        assert_eq!(got.body_type, BodyType::FormData);
        assert_eq!(form_rows(&got), form_rows(&login));
    }

    #[test]
    fn a_disabled_file_field_keeps_a_form_as_formdata() {
        let upload = request_variants()
            .into_iter()
            .find(|r| r.id == "req-patch")
            .unwrap();
        assert!(
            upload
                .form_data_fields
                .iter()
                .any(|f| f.field_type == FormFieldType::File && !f.enabled)
        );
        let exported: Value =
            serde_json::from_str(&export(&collection_of(upload), WITH_SECRETS)).unwrap();
        assert_eq!(exported["item"][0]["request"]["body"]["mode"], "formdata");
    }

    #[test]
    fn file_fields_import_their_path_and_type() {
        let ic = import_one(
            r#"{"info": {"name": "c"}, "item": [{"name": "up", "request": {"method": "POST",
                "body": {"mode": "formdata", "formdata": [
                    {"key": "caption", "type": "text", "value": "hi"},
                    {"key": "img", "type": "file", "src": "front.png", "contentType": "image/png"}]}}}]}"#,
        );
        let fields = &ic.requests[0].form_data_fields;
        assert_eq!(
            (
                fields[0].file_name.as_deref(),
                fields[0].mime_type.as_deref()
            ),
            (None, None)
        );
        assert_eq!(fields[1].field_type, FormFieldType::File);
        assert_eq!(fields[1].file_name.as_deref(), Some("front.png"));
        assert_eq!(fields[1].mime_type.as_deref(), Some("image/png"));
        assert_eq!(fields[1].file_data, None, "the content is not in the file");
    }

    #[test]
    fn file_fields_keep_their_file_name_and_type_both_ways() {
        let upload = request_variants()
            .into_iter()
            .find(|r| r.id == "req-patch")
            .unwrap();
        let exported: Value =
            serde_json::from_str(&export(&collection_of(upload.clone()), WITH_SECRETS)).unwrap();
        let rows = &exported["item"][0]["request"]["body"]["formdata"];
        assert_eq!(rows[1]["src"], "front.png");
        assert_eq!(rows[1]["contentType"], "image/png");
        assert!(rows[0].get("src").is_none(), "text fields carry no file");

        let back = round_trip(upload);
        let file = &back.form_data_fields[1];
        assert_eq!(file.file_name.as_deref(), Some("front.png"));
        assert_eq!(file.mime_type.as_deref(), Some("image/png"));
    }

    #[test]
    fn a_file_field_listing_several_files_keeps_the_first() {
        let ic = import_one(
            r#"{"info": {"name": "c"}, "item": [{"name": "up", "request": {"method": "POST",
                "body": {"mode": "formdata", "formdata": [
                    {"key": "f", "type": "file", "src": ["/tmp/a.png", "/tmp/b.png"]}]}}}]}"#,
        );
        let field = &ic.requests[0].form_data_fields[0];
        assert_eq!(field.file_name.as_deref(), Some("/tmp/a.png"));
    }

    #[test]
    fn a_url_without_raw_is_rebuilt_from_its_parts() {
        let ic = import_one(
            r#"{"info": {"name": "c"}, "item": [{"name": "r", "request": {"method": "GET",
                "url": {"protocol": "https", "host": ["api", "example", "com"], "port": "8443",
                        "path": ["v1", {"type": "string", "value": "users"}],
                        "query": [{"key": "page", "value": "2"}]}}}]}"#,
        );
        let req = &ic.requests[0];
        assert_eq!(req.url, "https://api.example.com:8443/v1/users");
        assert_eq!(kvs(&req.params), [("page", "2", true)]);
    }

    #[test]
    fn methods_rustman_cannot_send_import_as_get_with_a_warning() {
        let outcome = import(
            r#"{"info": {"name": "c"}, "item": [{"name": "Find", "request": {"method": "PROPFIND", "url": "https://x.io"}}]}"#,
        )
        .unwrap();
        assert_eq!(outcome.collections[0].requests[0].method, HttpMethod::Get);
        assert_eq!(
            outcome.warnings,
            ["\"Find\" uses PROPFIND, which Rustman can't send, so it was imported as GET"]
        );
    }
}
