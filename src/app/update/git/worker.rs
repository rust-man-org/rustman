//! The blocking halves of git jobs. Each runs on the blocking pool with a
//! snapshot of what it needs and answers with the message that applies its
//! result; none of them sees the app state.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::domain::group::ImportedCollection;
use crate::message::{GitMsg, GitView, RepoAddPayload, RepoHealth, RepoSummary, SyncPayload};
use crate::services::repos::{self, GitRepo};
use crate::services::vcs::{self, Checkout, GitError, GitIdentity, Skipped};

/// The log length the panel lists.
const LOG_LIMIT: usize = 50;

/// What a repository's working tree should hold: the saved collections it
/// owns and the global scripts.
pub(super) struct Tree {
    pub path: PathBuf,
    pub collections: Vec<ImportedCollection>,
    pub pre_request_script: String,
    pub test_script: String,
}

impl Tree {
    fn write(&self) -> Result<Skipped, GitError> {
        vcs::write_working_tree(
            &self.path,
            &self.collections,
            &self.pre_request_script,
            &self.test_script,
        )
    }
}

fn error(e: &GitError) -> GitMsg {
    GitMsg::Error(e.to_string())
}

fn done(result: Result<String, GitError>) -> GitMsg {
    result.map_or_else(|e| error(&e), GitMsg::Done)
}

/// Writes the active repository's tree, then reads every repository's
/// state. The Local repository is the app's own and is created on demand;
/// any other one that went missing is reported, never recreated.
pub(super) fn refresh(repos: &[GitRepo], active: String, generation: u64, tree: &Tree) -> GitMsg {
    let mut view = GitView {
        repo: active,
        generation,
        summaries: Vec::new(),
        status: None,
        branches: Vec::new(),
        log: Vec::new(),
        removed_names: HashMap::new(),
        warnings: Vec::new(),
        identity: None,
    };
    for repo in repos {
        if repo.id == repos::LOCAL_ID
            && let Err(e) = vcs::init(&repo.path)
        {
            view.warnings
                .push(format!("Couldn't create the Local repository: {e}"));
        }
        let is_active = repo.id == view.repo;
        let written = is_active.then(|| tree.write());
        let health = match vcs::status(&repo.path) {
            Ok(status) => {
                let health = RepoHealth::Ready {
                    branch: status.branch.clone(),
                    changes: status.changes.len(),
                };
                if is_active {
                    view.warnings.extend(match written {
                        Some(Ok(skipped)) => describe(&skipped, &tree.collections),
                        Some(Err(e)) => vec![format!("Couldn't update the working tree: {e}")],
                        None => Vec::new(),
                    });
                    view.branches = vcs::branches(&repo.path).unwrap_or_default();
                    view.log = vcs::full_log(&repo.path, LOG_LIMIT);
                    view.removed_names = removed_collection_names(&repo.path, &status, &view.log);
                    view.identity = vcs::resolve_identity(&repo.path);
                    view.status = Some(status);
                }
                health
            }
            Err(e) => {
                if is_active {
                    view.warnings.push(format!(
                        "{e}. Nothing was written; bring the folder back or remove {} here.",
                        repo.name
                    ));
                }
                match e {
                    GitError::Missing(_) => RepoHealth::Missing,
                    GitError::NotARepository(_) => RepoHealth::NotARepository,
                    e => RepoHealth::Failed(e.to_string()),
                }
            }
        };
        view.summaries.push(RepoSummary {
            id: repo.id.clone(),
            health,
        });
    }
    GitMsg::Loaded(Box::new(view))
}

/// Panel lines for what a working tree write left out.
fn describe(skipped: &Skipped, collections: &[ImportedCollection]) -> Vec<String> {
    let name = |id: &str| {
        collections
            .iter()
            .find(|c| c.collection.id == id)
            .map_or(id, |c| c.collection.name.as_str())
            .to_owned()
    };
    let invalid = skipped
        .invalid_ids
        .iter()
        .map(|id| format!("{} isn't in git: its id can't name a file.", name(id)));
    let unreadable = skipped
        .unreadable
        .iter()
        .map(|u| format!("{u}. Rustman leaves it alone."));
    invalid.chain(unreadable).collect()
}

/// Collection names, read from the last commit, for the collection files
/// the working tree deletes; the workspace no longer knows them.
fn removed_collection_names(
    path: &Path,
    status: &vcs::RepoStatus,
    log: &[vcs::CommitInfo],
) -> HashMap<String, String> {
    let deleted: HashSet<&str> = status
        .changes
        .iter()
        .filter(|c| c.state == "deleted")
        .filter_map(|c| c.path.strip_suffix(".json"))
        .collect();
    let Some(head) = log.first().filter(|_| !deleted.is_empty()) else {
        return HashMap::new();
    };
    vcs::read_commit_collections(path, &head.id)
        .unwrap_or_default()
        .collections
        .into_iter()
        .filter(|c| deleted.contains(c.collection.id.as_str()))
        .map(|c| (c.collection.id, c.collection.name))
        .collect()
}

pub(super) fn diff(repo: String, generation: u64, path: &Path) -> GitMsg {
    match vcs::working_diff(path) {
        Ok(diff) => GitMsg::DiffLoaded {
            repo,
            generation,
            diff,
        },
        Err(e) => error(&e),
    }
}

pub(super) fn commit(
    tree: &Tree,
    identity: &GitIdentity,
    message: &str,
    note: Option<String>,
) -> GitMsg {
    done(tree.write().and_then(|_| {
        vcs::commit(&tree.path, identity, message)?;
        let done = format!("Committed as {} <{}>.", identity.name, identity.email);
        Ok(match note {
            Some(note) => format!("{done} {note}"),
            None => done,
        })
    }))
}

pub(super) fn set_remote(path: &Path, url: &str) -> GitMsg {
    done(vcs::set_remote(path, url).map(|()| "Remote saved".to_owned()))
}

pub(super) fn create_branch(path: &Path, name: &str) -> GitMsg {
    done(vcs::create_branch(path, name).map(|()| format!("Created branch {name}")))
}

pub(super) fn fetch(path: &Path) -> GitMsg {
    done(vcs::fetch(path).map(|()| "Fetched from origin".to_owned()))
}

pub(super) fn push(path: &Path) -> GitMsg {
    done(vcs::push(path).map(|()| "Pushed to origin".to_owned()))
}

/// Saved edits go to disk before pulling, so git refuses to overwrite them
/// instead of the pull silently replacing them in the workspace.
pub(super) fn pull(repo: String, tree: &Tree) -> GitMsg {
    let pulled = tree.write().and_then(|_| vcs::pull(&tree.path));
    match pulled {
        Ok(summary) => synced(repo, &tree.path, format!("Pull: {summary}")),
        Err(e) => error(&e),
    }
}

/// A safe switch writes the saved collections first, so a saved but
/// uncommitted edit counts as a change the checkout refuses to overwrite.
pub(super) fn switch(repo: String, tree: &Tree, branch: String, discard: bool) -> GitMsg {
    let mode = if discard {
        Checkout::DiscardChanges
    } else {
        Checkout::Safe
    };
    let switched = if discard {
        vcs::checkout_branch(&tree.path, &branch, mode)
    } else {
        tree.write()
            .and_then(|_| vcs::checkout_branch(&tree.path, &branch, mode))
    };
    match switched {
        Ok(()) => synced(repo, &tree.path, format!("Switched to {branch}")),
        Err(GitError::Dirty(changes)) => GitMsg::SwitchBlocked { branch, changes },
        Err(e) => error(&e),
    }
}

pub(super) fn restore(repo: String, path: &Path, commit: &str) -> GitMsg {
    match vcs::read_commit_collections(path, commit) {
        Ok(loaded) => GitMsg::Synced(Box::new(SyncPayload {
            repo_id: repo,
            loaded,
            label: format!("Restored {}", short_hash(commit)),
            global_script: vcs::read_commit_global_script(path, commit),
        })),
        Err(e) => error(&e),
    }
}

fn short_hash(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// The working tree's collections after git changed it.
fn synced(repo: String, path: &Path, label: String) -> GitMsg {
    match vcs::load_collections_from(path) {
        Ok(loaded) => GitMsg::Synced(Box::new(SyncPayload {
            repo_id: repo,
            loaded,
            label,
            global_script: vcs::read_global_script(path),
        })),
        Err(e) => error(&e),
    }
}

pub(super) fn add_folder(path: PathBuf, init: bool) -> GitMsg {
    if init && let Err(e) = vcs::init(&path) {
        return error(&e);
    }
    let path = path.canonicalize().unwrap_or(path);
    let remote_url = vcs::remote_url(&path);
    added(repos::name_from_path(&path), path, remote_url)
}

/// Clones into a directory of its own under `parent`: two remotes may well
/// share a name.
pub(super) fn clone(url: String, parent: &Path, taken: &[PathBuf]) -> GitMsg {
    let name = repos::name_from_url(&url);
    let dest = repos::unique_dir(parent, &name, taken);
    match vcs::clone_repo(&url, &dest) {
        Ok(()) => added(name, dest, Some(url)),
        Err(e) => error(&e),
    }
}

fn added(name: String, path: PathBuf, remote_url: Option<String>) -> GitMsg {
    match vcs::load_collections_from(&path) {
        Ok(loaded) => GitMsg::RepoAdded(Box::new(RepoAddPayload {
            name,
            global_script: vcs::read_global_script(&path),
            path,
            remote_url,
            loaded,
        })),
        Err(e) => error(&e),
    }
}
