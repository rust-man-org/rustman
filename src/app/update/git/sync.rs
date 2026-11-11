//! Applies what git brought in (a pull, a branch switch, a restore, a newly
//! added repository) to the workspace. A collection whose file exists but
//! can't be read is kept as it is: an unreadable file is not a deletion.

use std::collections::{HashMap, HashSet};

use iced::Task;

use super::jobs;
use crate::app::AppState;
use crate::domain::collection::SavedRequest;
use crate::domain::group::ImportedCollection;
use crate::message::{Message, RepoAddPayload, SidebarPanel, SyncPayload};
use crate::services::repos::{self, GitRepo};
use crate::services::vcs::{self, FileProblem, UnreadableFile};
use crate::state::git::GitJob;

pub(super) fn synced(state: &mut AppState, payload: SyncPayload) -> Task<Message> {
    let SyncPayload {
        repo_id,
        loaded,
        label,
        global_script,
    } = payload;
    let unreadable: HashSet<&str> = loaded
        .unreadable
        .iter()
        .filter_map(UnreadableFile::collection_id)
        .collect();
    let (collections, elsewhere) = split_claimed(state, &repo_id, loaded.collections);
    let incoming: HashSet<&str> = collections
        .iter()
        .map(|c| c.collection.id.as_str())
        .collect();

    let mut kept = Vec::new();
    for owned in super::owned_collection_ids(state, &repo_id) {
        if unreadable.contains(owned.as_str()) {
            kept.push(owned);
        } else if !incoming.contains(owned.as_str()) {
            super::remove_collection(state, &owned);
        }
    }
    let mut notes = unreadable_notes(&loaded.unreadable, &kept);
    notes.extend(elsewhere);

    let mut ids: Vec<String> = incoming.iter().map(|id| (*id).to_owned()).collect();
    ids.extend(kept);
    let before = open_requests(state);
    apply(state, collections);
    super::apply_global_script(state, global_script);
    resync_tabs(state, &before);
    if let Some(repo) = state.git_repos.iter_mut().find(|r| r.id == repo_id) {
        repo.collection_ids = ids;
    }
    repos::save(&state.data_dir, &state.git_repos);
    state.status_message = Some(with_notes(label, &notes));
    jobs::enqueue(state, GitJob::Refresh)
}

pub(super) fn repo_added(state: &mut AppState, payload: RepoAddPayload) -> Task<Message> {
    let RepoAddPayload {
        name,
        path,
        remote_url,
        loaded,
        global_script,
    } = payload;
    if let Some(reason) = already_added(&state.git_repos, Some(&path), remote_url.as_deref()) {
        state.status_message = Some(reason);
        return Task::none();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let (collections, elsewhere) = split_claimed(state, &id, loaded.collections);
    let mut notes = unreadable_notes(&loaded.unreadable, &[]);
    notes.extend(elsewhere);
    let count = collections.len();
    let ids: Vec<String> = collections
        .iter()
        .map(|c| c.collection.id.clone())
        .collect();
    if state.git.local_only.iter().any(|kept| ids.contains(kept)) {
        state.git.local_only.retain(|kept| !ids.contains(kept));
        repos::save_local_only(&state.data_dir, &state.git.local_only);
    }
    apply(state, collections);
    super::apply_global_script(state, global_script);
    state.git_repos.push(GitRepo {
        id: id.clone(),
        name: name.clone(),
        path,
        remote_url,
        collection_ids: ids,
    });
    repos::save(&state.data_dir, &state.git_repos);
    let label = if count > 0 {
        state.sidebar.panel = SidebarPanel::Collections;
        format!("Imported {count} collection(s) from {name}")
    } else {
        format!("Added {name}: no Rustman collections in it")
    };
    state.status_message = Some(with_notes(label, &notes));
    super::activate_repo(state, id)
}

/// Why a repository at `path`, or cloned from `remote`, can't be added.
pub(super) fn already_added(
    repos: &[GitRepo],
    path: Option<&std::path::Path>,
    remote: Option<&str>,
) -> Option<String> {
    let same_remote = |url: &str| remote.is_some_and(|r| repos::same_remote(r, url));
    repos.iter().find_map(|r| {
        if let Some(path) = path.filter(|p| *p == r.path) {
            Some(format!("{} is already added as {}", path.display(), r.name))
        } else if r.remote_url.as_deref().is_some_and(same_remote) {
            Some(format!("{} already syncs with that remote", r.name))
        } else {
            None
        }
    })
}

/// `collections` split into those `repo` may take and notes for the ones
/// another repository already holds: a collection lives in one repository.
fn split_claimed(
    state: &AppState,
    repo: &str,
    collections: Vec<ImportedCollection>,
) -> (Vec<ImportedCollection>, Vec<String>) {
    let claimed: HashSet<&str> = state
        .git_repos
        .iter()
        .filter(|r| r.id != repo && r.id != repos::LOCAL_ID)
        .flat_map(|r| r.collection_ids.iter().map(String::as_str))
        .collect();
    let (theirs, mine): (Vec<_>, Vec<_>) = collections
        .into_iter()
        .partition(|c| claimed.contains(c.collection.id.as_str()));
    let notes = theirs
        .iter()
        .map(|c| format!("{} is already in another repository", c.collection.name))
        .collect();
    (mine, notes)
}

/// Writes `collections` into the workspace, keeping the secrets git
/// doesn't carry.
fn apply(state: &mut AppState, mut collections: Vec<ImportedCollection>) {
    for incoming in &mut collections {
        if let Some(local) = state.requests.get(&incoming.collection.id) {
            vcs::keep_local_secrets(incoming, local);
        }
    }
    super::apply_sync(state, collections);
}

/// The saved request behind each linked tab, as JSON, to tell afterwards
/// which ones git changed.
fn open_requests(state: &AppState) -> HashMap<String, String> {
    state
        .tabs
        .tabs
        .iter()
        .filter_map(|tab| {
            Some((
                tab.id.clone(),
                as_json(saved_request(&state.requests, tab.saved_as.as_ref()?)?),
            ))
        })
        .collect()
}

/// The saved request a tab links to, if it still exists.
fn saved_request<'a>(
    requests: &'a HashMap<String, Vec<SavedRequest>>,
    (collection, request): &(String, String),
) -> Option<&'a SavedRequest> {
    requests.get(collection)?.iter().find(|r| &r.id == request)
}

fn as_json(request: &SavedRequest) -> String {
    serde_json::to_string(request).expect("a saved request is plain data")
}

/// Brings tabs up to date with requests git changed. A clean tab shows the
/// new version and stays the same tab; one with unsaved edits keeps them
/// and is flagged until the user picks which version wins.
fn resync_tabs(state: &mut AppState, before: &HashMap<String, String>) {
    for tab in &mut state.tabs.tabs {
        let Some(saved) = tab
            .saved_as
            .as_ref()
            .and_then(|link| saved_request(&state.requests, link))
        else {
            continue;
        };
        if before.get(&tab.id) == Some(&as_json(saved)) {
            continue;
        }
        if !tab.modified {
            tab.reload_saved(saved);
        } else if !state.git.changed_upstream.contains(&tab.id) {
            state.git.changed_upstream.push(tab.id.clone());
        }
    }
}

/// Replaces the tab's edits with the version git brought in.
pub(super) fn reload_tab(state: &mut AppState, tab_id: &str) {
    state.git.changed_upstream.retain(|id| id != tab_id);
    let Some(tab) = state.tabs.tabs.iter_mut().find(|t| t.id == tab_id) else {
        return;
    };
    if let Some(saved) = tab
        .saved_as
        .as_ref()
        .and_then(|link| saved_request(&state.requests, link))
    {
        tab.reload_saved(saved);
    }
}

/// Keeps the tab's edits. They stay unsaved: only saving the tab, by the
/// user's hand, writes them over the version git brought in.
pub(super) fn keep_mine(state: &mut AppState, tab_id: &str) {
    state.git.changed_upstream.retain(|id| id != tab_id);
}

/// Drops flags for tabs that were closed or saved since.
pub(super) fn prune_changed_upstream(state: &mut AppState) {
    let tabs = &state.tabs.tabs;
    state
        .git
        .changed_upstream
        .retain(|id| tabs.iter().any(|t| &t.id == id && t.modified));
}

/// The unreadable files worth a word: those of collections that were kept,
/// and Rustman-shaped ones this version can't read. A `package.json` in the
/// folder is nobody's concern.
fn unreadable_notes(unreadable: &[UnreadableFile], kept: &[String]) -> Vec<String> {
    unreadable
        .iter()
        .filter(|u| {
            matches!(u.problem, FileProblem::Unsupported(_))
                || u.collection_id()
                    .is_some_and(|id| kept.iter().any(|k| k == id))
        })
        .map(|u| format!("{u}; left as it was"))
        .collect()
}

fn with_notes(label: String, notes: &[String]) -> String {
    if notes.is_empty() {
        label
    } else {
        format!("{label}. {}", notes.join(". "))
    }
}
