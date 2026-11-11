use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A named set of `{{variable}}` values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEnvironment {
    pub id: String,
    pub name: String,
    pub variables: HashMap<String, String>,
    pub is_active: bool,
}
