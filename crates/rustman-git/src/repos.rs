//! The registry of collection repositories, kept in `git_repos.json` in the
//! data dir. The built-in "Local" repository is always present.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The id of the built-in repository under `<data_dir>/collections`.
pub const LOCAL_ID: &str = "local";

const REGISTRY_FILE: &str = "git_repos.json";
/// The registry as it was before the last save, read when the main file
/// can't be.
const BACKUP_FILE: &str = "git_repos.json.bak";
const LOCAL_ONLY_FILE: &str = "git_local_only.json";

/// A repository the app syncs collections with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepo {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub remote_url: Option<String>,
    /// The collections that live in this repository.
    #[serde(default)]
    pub collection_ids: Vec<String>,
}

#[derive(Default, Deserialize)]
struct Registry {
    repos: Vec<GitRepo>,
}

#[derive(Serialize)]
struct RegistryRef<'a> {
    repos: &'a [GitRepo],
}

#[derive(Default, Serialize, Deserialize)]
struct LocalOnly {
    collection_ids: Vec<String>,
}

/// The registered repositories, "Local" first. An unreadable registry falls
/// back to its backup; with neither, there is just "Local".
pub fn load(data_dir: &Path) -> Vec<GitRepo> {
    let mut repos = read_registry(&data_dir.join(REGISTRY_FILE))
        .or_else(|| read_registry(&data_dir.join(BACKUP_FILE)))
        .unwrap_or_default();
    if !repos.iter().any(|r| r.id == LOCAL_ID) {
        repos.insert(
            0,
            GitRepo {
                id: LOCAL_ID.to_owned(),
                name: "Local".to_owned(),
                path: data_dir.join("collections"),
                remote_url: None,
                collection_ids: Vec::new(),
            },
        );
    }
    repos
}

/// The repositories in the registry file at `path`; `None` when it is
/// missing or unreadable.
fn read_registry(path: &Path) -> Option<Vec<GitRepo>> {
    let json = match std::fs::read(path) {
        Ok(json) => json,
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        Err(e) => {
            eprintln!("rustman: cannot read {}: {e}", path.display());
            return None;
        }
    };
    match serde_json::from_slice::<Registry>(&json) {
        Ok(registry) => Some(registry.repos),
        Err(e) => {
            eprintln!("rustman: ignoring unreadable {}: {e}", path.display());
            None
        }
    }
}

/// Writes the registry atomically, first keeping the current file as the
/// backup if it is readable. A failed write is logged and leaves the
/// previous registry in place.
pub fn save(data_dir: &Path, repos: &[GitRepo]) {
    let path = data_dir.join(REGISTRY_FILE);
    let json = serde_json::to_vec_pretty(&RegistryRef { repos })
        .expect("the registry is plain data and always serializes");
    let saved = keep_backup(data_dir).and_then(|()| crate::atomic::write(&path, &json));
    if let Err(e) = saved {
        eprintln!("rustman: cannot write {}: {e}", path.display());
    }
}

fn keep_backup(data_dir: &Path) -> std::io::Result<()> {
    let path = data_dir.join(REGISTRY_FILE);
    match std::fs::read(&path) {
        Ok(current) if serde_json::from_slice::<Registry>(&current).is_ok() => {
            crate::atomic::write(&data_dir.join(BACKUP_FILE), &current)
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The collections kept out of every repository, Local included: those of
/// a removed repository whose collections the user chose to keep.
pub fn load_local_only(data_dir: &Path) -> Vec<String> {
    let path = data_dir.join(LOCAL_ONLY_FILE);
    match std::fs::read(&path) {
        Ok(json) => serde_json::from_slice::<LocalOnly>(&json).map_or_else(
            |e| {
                eprintln!("rustman: ignoring unreadable {}: {e}", path.display());
                Vec::new()
            },
            |file| file.collection_ids,
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            eprintln!("rustman: cannot read {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// Writes the collections kept out of every repository. A failed write is
/// logged.
pub fn save_local_only(data_dir: &Path, collection_ids: &[String]) {
    let path = data_dir.join(LOCAL_ONLY_FILE);
    let file = LocalOnly {
        collection_ids: collection_ids.to_vec(),
    };
    let json = serde_json::to_vec_pretty(&file).expect("plain data always serializes");
    if let Err(e) = crate::atomic::write(&path, &json) {
        eprintln!("rustman: cannot write {}: {e}", path.display());
    }
}

/// Whether two remote URLs name the same repository, `.git` and a trailing
/// slash aside.
pub fn same_remote(a: &str, b: &str) -> bool {
    let normal = |url: &str| {
        let url = url.trim().trim_end_matches('/');
        url.strip_suffix(".git").unwrap_or(url).to_ascii_lowercase()
    };
    normal(a) == normal(b)
}

/// A directory under `parent` for a clone named `name` that neither exists
/// nor is `taken` by a registered repository: `api`, then `api-2`, `api-3`…
pub fn unique_dir(parent: &Path, name: &str, taken: &[PathBuf]) -> PathBuf {
    let base = slug(name);
    (1..)
        .map(|n| match n {
            1 => parent.join(&base),
            n => parent.join(format!("{base}-{n}")),
        })
        .find(|dir| !dir.exists() && !taken.contains(dir))
        .expect("an unbounded search finds a free name")
}

/// A directory-safe name: lowercase alphanumerics, other runs as `-`, and
/// never a name Windows reserves for a device.
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
    match cleaned.trim_matches('-') {
        "" => "repo".to_owned(),
        device if rustman_core::text::is_windows_device_name(device) => format!("{device}-repo"),
        trimmed => trimmed.to_owned(),
    }
}

/// The repository name a clone URL implies: its last segment without `.git`.
pub fn name_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("repo")
        .trim_end_matches(".git")
        .to_owned()
}

/// The repository name a folder implies: its own name.
pub fn name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map_or_else(|| "repo".to_owned(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    use rustman_testkit::TempDir;

    fn repo(id: &str) -> GitRepo {
        GitRepo {
            id: id.into(),
            name: id.into(),
            path: PathBuf::from(format!("/repos/{id}")),
            remote_url: None,
            collection_ids: Vec::new(),
        }
    }

    #[test]
    fn slugs_are_directory_safe() {
        assert_eq!(slug("Team API (v2)"), "team-api--v2");
        assert_eq!(slug("!!!"), "repo");
        assert_eq!(slug("CON"), "con-repo");
        assert_eq!(slug("nul"), "nul-repo");
        assert_eq!(slug("console"), "console");
    }

    #[test]
    fn clones_of_same_named_repositories_get_their_own_directories() {
        let dir = TempDir::new("repos-unique");
        std::fs::create_dir(dir.path().join("api")).unwrap();
        let taken = [dir.path().join("api-2")];
        assert_eq!(
            unique_dir(dir.path(), "api", &taken),
            dir.path().join("api-3")
        );
        assert_eq!(
            unique_dir(dir.path(), "web", &taken),
            dir.path().join("web")
        );
    }

    #[test]
    fn local_only_collections_round_trip() {
        let dir = TempDir::new("repos-local-only");
        assert!(load_local_only(dir.path()).is_empty());
        save_local_only(dir.path(), &["a".into(), "b".into()]);
        assert_eq!(load_local_only(dir.path()), ["a", "b"]);
    }

    #[test]
    fn remotes_match_regardless_of_suffix_and_case() {
        assert!(same_remote(
            "https://example.invalid/Team/API.git",
            "https://example.invalid/team/api/"
        ));
        assert!(!same_remote(
            "https://example.invalid/a/api.git",
            "https://example.invalid/b/api.git"
        ));
    }

    #[test]
    fn saving_keeps_a_backup_that_loads_when_the_registry_is_corrupt() {
        let dir = TempDir::new("repos-backup");
        save(dir.path(), &[repo("first")]);
        save(dir.path(), &[repo("first"), repo("second")]);
        std::fs::write(dir.path().join(REGISTRY_FILE), "{\"repos\": [trunc").unwrap();

        let ids: Vec<String> = load(dir.path()).into_iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            [LOCAL_ID, "first"],
            "the registry before the last save"
        );

        save(dir.path(), &[repo("third")]);
        let backup = std::fs::read_to_string(dir.path().join(BACKUP_FILE)).unwrap();
        assert!(
            backup.contains("\"first\"") && !backup.contains("trunc"),
            "a corrupt file never replaces the backup"
        );
        let ids: Vec<String> = load(dir.path()).into_iter().map(|r| r.id).collect();
        assert_eq!(ids, [LOCAL_ID, "third"]);
    }

    #[test]
    fn names_come_from_the_last_url_or_path_segment() {
        assert_eq!(
            name_from_url("https://example.invalid/team/api.git/"),
            "api"
        );
        assert_eq!(name_from_url("git@example.invalid:team/api.git"), "api");
        assert_eq!(
            name_from_path(Path::new("/home/me/collections")),
            "collections"
        );
    }
}
