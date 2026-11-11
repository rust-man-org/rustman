//! The identity plain `git commit` would use, for when the user set no
//! override in Rustman. A repository's own `user.email`, or one a
//! conditional include sets for it, is never bypassed: a commit pushed with
//! the wrong email can't be taken back.

use std::path::Path;

use git2::{Config, Repository};

use super::error::GitError;
use super::{GitIdentity, remote};

/// `user.name` and `user.email` as git resolves them in the repository at
/// `data_dir` (its own config, conditional includes, the global config),
/// if both are set.
pub fn resolve_identity(data_dir: &Path) -> Option<GitIdentity> {
    lookup(Some(data_dir))
}

/// The user's global git identity, if both parts are set.
pub fn default_identity() -> Option<GitIdentity> {
    lookup(None)
}

fn lookup(repo: Option<&Path>) -> Option<GitIdentity> {
    let get = |key: &str| match remote::config_value(repo, key) {
        Ok(value) => value,
        // Without a `git` to ask, libgit2 reads the same files; it knows
        // most include conditions, but not all of them.
        Err(GitError::Spawn(_)) => from_files(repo, key),
        Err(e) => {
            eprintln!("rustman: reading git's {key} failed: {e}");
            None
        }
    };
    Some(GitIdentity {
        name: get("user.name")?,
        email: get("user.email")?,
    })
}

fn from_files(repo: Option<&Path>, key: &str) -> Option<String> {
    let config = match repo {
        Some(path) => Repository::open(path).ok()?.config().ok()?,
        None => Config::open_default().ok()?,
    };
    config.get_string(key).ok().filter(|v| !v.is_empty())
}
