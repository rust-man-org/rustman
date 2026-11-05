use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const LOCAL_ID: &str = "local";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepo {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub collection_ids: Vec<String>,
    /// An import-only source (e.g. a cloned Postman export): its collections
    /// are read into the app but its working tree is never rewritten.
    #[serde(default)]
    pub protected: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Registry {
    repos: Vec<GitRepo>,
}

fn registry_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("git_repos.json")
}

pub fn load(data_dir: &PathBuf) -> Vec<GitRepo> {
    let mut registry: Registry = std::fs::read_to_string(registry_path(data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    if !registry.repos.iter().any(|r| r.id == LOCAL_ID) {
        registry.repos.insert(
            0,
            GitRepo {
                id: LOCAL_ID.to_owned(),
                name: "Local".to_owned(),
                path: data_dir.join("collections"),
                remote_url: None,
                collection_ids: Vec::new(),
                protected: false,
            },
        );
    }
    registry.repos
}

pub fn save(data_dir: &PathBuf, repos: &[GitRepo]) {
    let registry = Registry {
        repos: repos.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&registry) {
        let _ = std::fs::write(registry_path(data_dir), json);
    }
}

pub fn slug(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "repo".to_owned()
    } else {
        trimmed
    }
}

pub fn name_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_owned()
}

pub fn name_from_path(path: &PathBuf) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| "repo".to_owned())
}
