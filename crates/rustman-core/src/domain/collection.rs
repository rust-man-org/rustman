use serde::{Deserialize, Serialize};

use super::request::{ApiKeyLocation, AuthType, BodyType, FormField, HttpMethod, KeyValue};

fn default_jwt_algo() -> String {
    "HS256".to_owned()
}

/// A named top-level container of requests and groups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    /// Milliseconds since the Unix epoch; collections list oldest first.
    pub created_at: i64,
}

/// Whether `id` may identify a collection. Ids name files in git
/// repositories, so only 1 to 64 ASCII letters, digits, `-` and `_` pass, and
/// never a name Windows reserves for a device.
pub fn is_valid_collection_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && !crate::text::is_windows_device_name(id)
}

impl Collection {
    /// An empty collection with a fresh id, created now.
    pub fn new(name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            created_at: crate::time::now_millis(),
        }
    }
}

/// A request as saved in a collection, a history entry or a git file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedRequest {
    pub id: String,
    pub collection_id: String,
    /// The enclosing group, or `None` for a direct child of `collection_id`.
    #[serde(default)]
    pub group_id: Option<String>,
    /// Manual order among the siblings that share this request's parent.
    #[serde(default)]
    pub sort_order: i64,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<KeyValue>,
    pub params: Vec<KeyValue>,
    pub body: String,
    pub body_type: BodyType,
    /// GraphQL variables as typed (JSON text); the query itself is `body`.
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
    /// A copy of this request under a fresh id, in `collection_id`, named `name`.
    pub fn duplicate_in(&self, collection_id: String, name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            collection_id,
            name,
            ..self.clone()
        }
    }

    /// An empty GET request directly under `collection_id`.
    pub fn new_in(collection_id: String, name: String) -> Self {
        Self::new_in_group(collection_id, None, name, 0)
    }

    /// An empty GET request under `group_id` (or the collection itself), at
    /// position `sort_order` among its siblings.
    pub fn new_in_group(
        collection_id: String,
        group_id: Option<String>,
        name: String,
        sort_order: i64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            collection_id,
            group_id,
            sort_order,
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
            jwt_algo: default_jwt_algo(),
            pre_request_script: String::new(),
            test_script: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_collection_id;

    #[test]
    fn collection_ids_are_safe_file_names() {
        assert!(is_valid_collection_id(
            "9e8d7c6b-5a4f-4e3d-8c2b-1a0f9e8d7c6b"
        ));
        assert!(is_valid_collection_id("col_storefront-2"));
        assert!(is_valid_collection_id(&"a".repeat(64)));
        for bad in [
            "",
            "../../x",
            "/etc/passwd",
            "a/b",
            "a\\b",
            "a.b",
            "con",
            "Nul",
            "é",
            &"a".repeat(65),
        ] {
            assert!(!is_valid_collection_id(bad), "{bad:?}");
        }
    }
}
