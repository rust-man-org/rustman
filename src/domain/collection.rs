use serde::{Deserialize, Serialize};

use super::request::{ApiKeyLocation, AuthType, BodyType, FormField, HttpMethod, KeyValue};

fn default_jwt_algo() -> String { "HS256".to_owned() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedRequest {
    pub id: String,
    pub collection_id: String,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub params: Vec<KeyValue>,
    pub body: String,
    pub body_type: BodyType,
    /// The GraphQL *variables*, as typed (JSON text). Only meaningful when
    /// `body_type` is `GraphQL` — the query itself is `body`.
    #[serde(default)]
    pub graphql_variables: String,
    pub auth_type: AuthType,
    pub bearer_token: String,
    pub basic_user: String,
    pub basic_pass: String,
    pub api_key_name: String,
    pub api_key_value: String,
    pub api_key_location: ApiKeyLocation,
    pub form_data_fields: Vec<FormField>,
    pub cookie_string: String,
    pub cookies: Vec<KeyValue>,
    #[serde(default)]
    pub jwt_secret: String,
    #[serde(default)]
    pub jwt_subject: String,
    #[serde(default = "default_jwt_algo")]
    pub jwt_algo: String,
    pub pre_request_script: String,
    pub test_script: String,
}

impl SavedRequest {
    /// A copy of an existing request saved as a *new* row: fresh id, same
    /// collection, the given name.
    ///
    /// Every other field — method, url, headers, params, body, auth, form
    /// data, cookies, scripts — is carried over, so the name is the only
    /// thing the caller has to decide.
    pub fn duplicate_in(&self, collection_id: String, name: String) -> Self {
        Self { id: uuid::Uuid::new_v4().to_string(), collection_id, name, ..self.clone() }
    }

    pub fn new_in(collection_id: String, name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            collection_id,
            name,
            method: HttpMethod::Get,
            url: String::new(),
            headers: Vec::new(),
            params: Vec::new(),
            body: String::new(),
            body_type: BodyType::None,
            graphql_variables: String::new(),
            auth_type: AuthType::None,
            bearer_token: String::new(),
            basic_user: String::new(),
            basic_pass: String::new(),
            api_key_name: String::new(),
            api_key_value: String::new(),
            api_key_location: ApiKeyLocation::Header,
            form_data_fields: Vec::new(),
            cookie_string: String::new(),
            cookies: Vec::new(),
            jwt_secret: String::new(),
            jwt_subject: String::new(),
            jwt_algo: "HS256".to_owned(),
            pre_request_script: String::new(),
            test_script: String::new(),
        }
    }
}
