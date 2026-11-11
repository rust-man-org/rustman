//! Runs queued git jobs, one at a time. A job takes its snapshot when it
//! starts, not when it was queued, so a refresh waiting behind a pull
//! writes what the pull brought in.
//!
//! The collections a job writes to git are read by id on the store thread,
//! after every write queued before, and serialized on the blocking pool:
//! nothing the size of a collection is copied on the UI thread.

use std::path::PathBuf;
use std::pin::Pin;

use iced::Task;
use rustman_core::store::StoreError;

use super::worker::{self, Tree};
use crate::app::AppState;
use crate::domain::group::ImportedCollection;
use crate::message::{GitMsg, Message};
use crate::services::storage::{self, Connection};
use crate::state::git::GitJob;

type Work = Pin<Box<dyn Future<Output = GitMsg> + Send>>;

pub(super) fn enqueue(state: &mut AppState, job: GitJob) -> Task<Message> {
    state.git.jobs.push(job);
    start_next(state)
}

/// Called with every job's result message, after it is handled.
pub(super) fn finished(state: &mut AppState) -> Task<Message> {
    state.git.jobs.finished();
    start_next(state)
}

fn start_next(state: &mut AppState) -> Task<Message> {
    while let Some(job) = state.git.jobs.start_next() {
        if let Some(work) = prepare(state, job) {
            return Task::perform(work, |msg| Message::Git(GitMsg::JobFinished(Box::new(msg))));
        }
        state.git.jobs.finished();
    }
    Task::none()
}

/// The work for `job`, or `None` when its repository was removed meanwhile.
fn prepare(state: &AppState, job: GitJob) -> Option<Work> {
    match job {
        GitJob::Refresh => {
            let repos = state.git_repos.clone();
            let (active, generation) = (state.git_active_repo.clone(), state.git.generation);
            with_tree(state, &state.git_active_repo, move |tree| {
                worker::refresh(&repos, active, generation, &tree)
            })
        }
        GitJob::Diff { repo } => {
            let path = repo_path(state, &repo)?;
            let generation = state.git.generation;
            Some(blocking(move || worker::diff(repo, generation, &path)))
        }
        GitJob::Commit {
            repo,
            message,
            identity,
        } => {
            let committed = super::owned_collection_ids(state, &repo);
            let note = super::unsaved_note(super::unsaved_request_tabs(state, &committed));
            with_tree(state, &repo, move |tree| {
                worker::commit(&tree, &identity, &message, note)
            })
        }
        GitJob::SetRemote { repo, url } => {
            let path = repo_path(state, &repo)?;
            Some(blocking(move || worker::set_remote(&path, &url)))
        }
        GitJob::CreateBranch { repo, name } => {
            let path = repo_path(state, &repo)?;
            Some(blocking(move || worker::create_branch(&path, &name)))
        }
        GitJob::Fetch { repo } => {
            let path = repo_path(state, &repo)?;
            Some(blocking(move || worker::fetch(&path)))
        }
        GitJob::Push { repo } => {
            let path = repo_path(state, &repo)?;
            Some(blocking(move || worker::push(&path)))
        }
        GitJob::Pull { repo } => {
            let id = repo.clone();
            with_tree(state, &id, move |tree| worker::pull(repo, &tree))
        }
        GitJob::Switch {
            repo,
            branch,
            discard,
        } => {
            let id = repo.clone();
            with_tree(state, &id, move |tree| {
                worker::switch(repo, &tree, branch, discard)
            })
        }
        GitJob::Restore { repo, commit } => {
            let path = repo_path(state, &repo)?;
            Some(blocking(move || worker::restore(repo, &path, &commit)))
        }
        GitJob::AddFolder { path, init } => Some(blocking(move || worker::add_folder(path, init))),
        GitJob::Clone { url } => {
            let parent = state.data_dir.join("repos");
            let taken: Vec<PathBuf> = state.git_repos.iter().map(|r| r.path.clone()).collect();
            Some(blocking(move || worker::clone(url, &parent, &taken)))
        }
    }
}

fn blocking(work: impl FnOnce() -> GitMsg + Send + 'static) -> Work {
    Box::pin(async move {
        tokio::task::spawn_blocking(work)
            .await
            .unwrap_or_else(|e| GitMsg::Error(e.to_string()))
    })
}

/// `work` with the working tree `repo` should hold. Without a readable
/// store nothing is written: a tree missing collections would delete their
/// files.
fn with_tree(
    state: &AppState,
    repo: &str,
    work: impl FnOnce(Tree) -> GitMsg + Send + 'static,
) -> Option<Work> {
    let path = repo_path(state, repo)?;
    let ids = super::owned_collection_ids(state, repo);
    let pre_request_script = state.global_pre_request_editor.text();
    let test_script = state.global_test_editor.text();
    let saved = state
        .store
        .as_ref()
        .map(|store| store.read(move |s| read_collections(s.connection(), &ids)));
    Some(Box::pin(async move {
        let collections = match saved {
            Some(saved) => saved.await,
            None => Err("no saved collections to write: storage is unavailable".to_owned()),
        };
        match collections {
            Ok(collections) => {
                let tree = Tree {
                    path,
                    collections,
                    pre_request_script,
                    test_script,
                };
                blocking(move || work(tree)).await
            }
            Err(e) => GitMsg::Error(e),
        }
    }))
}

/// The collections `ids` names, as saved. Every one must be found: a
/// collection not saved yet must not read as one to delete from git.
pub(super) fn read_collections(
    conn: &Connection,
    ids: &[String],
) -> Result<Vec<ImportedCollection>, StoreError> {
    let collections = storage::get_collections(conn).map_err(StoreError::new)?;
    let groups = storage::get_groups(conn).map_err(StoreError::new)?;
    let mut out = Vec::with_capacity(ids.len());
    for collection in collections.into_iter().filter(|c| ids.contains(&c.id)) {
        let requests = storage::get_requests(conn, &collection.id).map_err(StoreError::new)?;
        out.push(ImportedCollection::from_parts(
            collection, &groups, &requests,
        ));
    }
    if out.len() == ids.len() {
        Ok(out)
    } else {
        Err(StoreError::new("a collection isn't saved yet; try again"))
    }
}

pub(super) fn repo_path(state: &AppState, repo: &str) -> Option<PathBuf> {
    state
        .git_repos
        .iter()
        .find(|r| r.id == repo)
        .map(|r| r.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_collection_missing_from_the_store_stops_the_write() {
        let dir = crate::test_support::TempDir::new("git-jobs-unsaved");
        let state = crate::app::boot::load(dir.path().to_path_buf());
        let store = state.store.as_ref().unwrap();
        let read = store.now(|s| Ok(read_collections(s.connection(), &["unsaved".to_owned()])));
        assert!(read.is_err(), "a short list would delete the collection's file");
    }

    #[test]
    fn a_job_for_a_removed_repository_is_skipped() {
        let dir = crate::test_support::TempDir::new("git-jobs");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let gone = GitJob::Fetch {
            repo: "removed".into(),
        };
        let _ = enqueue(&mut state, gone);
        assert!(!state.git.jobs.is_busy(), "nothing left to run");
        let _ = enqueue(
            &mut state,
            GitJob::Fetch {
                repo: crate::services::repos::LOCAL_ID.into(),
            },
        );
        assert!(state.git.jobs.is_busy());
    }
}
