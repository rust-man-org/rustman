//! A collection repository: `{collection_id}.json` per collection plus an
//! optional `global.json` holding the global scripts. Local operations use
//! libgit2; clone, fetch, pull and push shell out to the system `git` so they
//! reuse the user's SSH keys and credential helpers.
//!
//! Operations work on an existing repository and fail with
//! [`GitError::Missing`] or [`GitError::NotARepository`] otherwise; only
//! [`init`] creates one.

mod error;
mod files;
mod history;
mod identity;
mod remote;
mod secrets;

use std::path::{Path, PathBuf};

use git2::{ErrorCode, Repository};

pub use error::GitError;
pub use files::{
    FileProblem, LoadedCollections, Skipped, UnreadableFile, load_collections_from,
    read_commit_collections, read_commit_global_script, read_global_script, write_working_tree,
};
pub use history::{
    BranchInfo, Checkout, CommitInfo, FileChange, RepoStatus, branches, checkout_branch, commit,
    create_branch, current_branch, full_log, status, working_diff,
};
pub use identity::{default_identity, resolve_identity};
pub use remote::{clone_repo, fetch, pull, push, remote_url, set_remote};
pub use secrets::keep_local_secrets;

/// The author identity commits are made with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
}

/// What a folder picked by the user holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Folder {
    Repository,
    Missing,
    /// An existing directory that is not the root of a repository.
    Plain {
        /// How many entries it holds: initialising a busy folder deserves a
        /// second look.
        entries: usize,
        /// The root of the repository this folder sits inside, if any.
        enclosing: Option<PathBuf>,
    },
}

/// What `path` holds. Never creates or changes anything.
pub fn probe(path: &Path) -> Folder {
    if Repository::open(path).is_ok() {
        return Folder::Repository;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return Folder::Missing;
    };
    let enclosing = Repository::discover(path)
        .ok()
        .and_then(|repo| repo.workdir().map(Path::to_path_buf));
    Folder::Plain {
        entries: entries.count(),
        enclosing,
    }
}

/// Makes `path` a repository, creating the directory if needed. An existing
/// repository is left as it is.
pub fn init(path: &Path) -> Result<(), GitError> {
    std::fs::create_dir_all(path)?;
    match open(path) {
        Ok(_) => Ok(()),
        Err(GitError::NotARepository(_)) => Ok(Repository::init(path).map(drop)?),
        Err(e) => Err(e),
    }
}

/// The repository whose root is exactly `path`; parents are not searched.
fn open(path: &Path) -> Result<Repository, GitError> {
    Repository::open(path).map_err(|e| {
        if !path.is_dir() {
            GitError::Missing(path.to_owned())
        } else if e.code() == ErrorCode::NotFound {
            GitError::NotARepository(path.to_owned())
        } else {
            e.into()
        }
    })
}
