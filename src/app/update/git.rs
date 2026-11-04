use std::collections::HashSet;
use std::path::PathBuf;

use iced::Task;

use crate::app::AppState;
use crate::domain::group::ImportedCollection;
use crate::message::{GitMsg, GitView, Message, RepoAddPayload, RepoSummary, SyncPayload};
use crate::services::repos::{self, GitRepo};
use crate::services::{storage, vcs};

pub(super) fn handle(state: &mut AppState, msg: GitMsg) -> Task<Message> {
    match msg {
        GitMsg::Refresh => {
            crate::app::request_ops::flush_modified_tabs(state);
            return refresh_task(state);
        }
        GitMsg::Loaded(view) => {
            let view = *view;
            state.git_repo_summaries = view.summaries;
            state.git_status = view.status;
            state.git_branches = view.branches;
            state.git_log = view.log;
            state.git_busy = false;
            let repo_path = active_path(state);
            if let Some(id) = vcs::resolve_identity(&repo_path) {
                state.git_user_name = id.name;
                state.git_user_email = id.email;
            }
        }
        GitMsg::SelectRepo(id) => {
            crate::app::request_ops::flush_modified_tabs(state);
            state.git_active_repo = id;
            state.git_diff = None;
            return refresh_task(state);
        }
        GitMsg::CloneUrlChanged(value) => state.git_clone_url = value,
        GitMsg::CloneRepo => {
            let url = state.git_clone_url.trim().to_owned();
            if url.is_empty() {
                return Task::none();
            }
            let name = repos::name_from_url(&url);
            let dest = state.data_dir.join("repos").join(repos::slug(&name));
            state.git_clone_url.clear();
            state.git_busy = true;
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        vcs::clone_repo(&url, &dest)?;
                        let collections = vcs::load_collections_from(&dest)?;
                        let global_script = vcs::read_global_script(&dest);
                        Ok::<RepoAdd, String>(RepoAdd {
                            name,
                            path: dest,
                            remote_url: Some(url),
                            collections,
                            global_script,
                        })
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                },
                repo_added,
            );
        }
        GitMsg::OpenFolder => {
            return Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                |picked| Message::Git(GitMsg::FolderPicked(picked)),
            );
        }
        GitMsg::FolderPicked(None) => {}
        GitMsg::FolderPicked(Some(path)) => {
            let name = repos::name_from_path(&path);
            state.git_busy = true;
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let collections = vcs::load_collections_from(&path)?;
                        let remote_url = vcs::remote_url(&path);
                        let global_script = vcs::read_global_script(&path);
                        Ok::<RepoAdd, String>(RepoAdd {
                            name,
                            path,
                            remote_url,
                            collections,
                            global_script,
                        })
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                },
                repo_added,
            );
        }
        GitMsg::RepoAdded(payload) => {
            let RepoAddPayload {
                name,
                path,
                remote_url,
                collections,
                global_script,
            } = *payload;
            let count = collections.len();
            let ids: Vec<String> = collections
                .iter()
                .map(|ic| ic.collection.id.clone())
                .collect();
            if let Err(e) = apply_sync(state, collections) {
                state.status_message = Some(format!("Some local drafts could not be saved: {e}"));
            }
            apply_global_script(state, global_script);
            let id = uuid::Uuid::new_v4().to_string();
            state.git_repos.push(GitRepo {
                id: id.clone(),
                name: name.clone(),
                path,
                remote_url,
                collection_ids: ids,
            });
            repos::save(&state.data_dir, &state.git_repos);
            state.git_active_repo = id;
            state.git_busy = false;
            if count > 0 {
                state.sidebar.panel = crate::message::SidebarPanel::Collections;
                state.status_message = Some(format!("Imported {count} collection(s) from {name}"));
            } else {
                state.status_message =
                    Some(format!("Added {name} — no rustman collections found in it"));
            }
            return refresh_task(state);
        }
        GitMsg::RemoveRepo(id) => {
            if id == repos::LOCAL_ID {
                return Task::none();
            }
            state.git_repos.retain(|r| r.id != id);
            repos::save(&state.data_dir, &state.git_repos);
            if state.git_active_repo == id {
                state.git_active_repo = repos::LOCAL_ID.to_owned();
            }
            return refresh_task(state);
        }
        GitMsg::CommitMessageChanged(value) => state.git_commit_message = value,
        GitMsg::HistorySearchChanged(value) => state.git_history_search = value,
        GitMsg::RemoteUrlChanged(value) => state.git_remote_input = value,
        GitMsg::NewBranchNameChanged(value) => state.git_new_branch = value,
        GitMsg::Commit => {
            let message = state.git_commit_message.trim().to_owned();
            if message.is_empty() {
                state.status_message = Some("Enter a commit message first".to_owned());
                return Task::none();
            }
            crate::app::request_ops::flush_modified_tabs(state);
            let repo_path = active_path(state);
            let collections = collections_for_active(state);
            let global_pre_request_script = state.global_pre_request_editor.text();
            let global_test_script = state.global_test_editor.text();
            let identity = match vcs::resolve_identity(&repo_path) {
                Some(id) => id,
                None => {
                    state.status_message = Some(
                        "Set git user.name and user.email first:\n  git config user.name \"Your Name\"\n  git config user.email \"you@example.com\""
                            .to_owned(),
                    );
                    return Task::none();
                }
            };
            state.git_commit_message.clear();
            state.git_busy = true;
            return blocking(move || {
                vcs::write_working_tree(
                    &repo_path,
                    &collections,
                    &global_pre_request_script,
                    &global_test_script,
                )?;
                vcs::commit(&repo_path, &identity, &message)?;
                Ok(format!(
                    "Committed as {} <{}>",
                    identity.name, identity.email
                ))
            });
        }
        GitMsg::SetRemote => {
            let url = state.git_remote_input.trim().to_owned();
            if url.is_empty() {
                return Task::none();
            }
            let repo_path = active_path(state);
            return blocking(move || {
                vcs::set_remote(&repo_path, &url)?;
                Ok("Remote saved".to_owned())
            });
        }
        GitMsg::CreateBranch => {
            let name = state.git_new_branch.trim().to_owned();
            if name.is_empty() {
                return Task::none();
            }
            let repo_path = active_path(state);
            state.git_new_branch.clear();
            return blocking(move || {
                vcs::create_branch(&repo_path, &name)?;
                Ok(format!("Created branch {name}"))
            });
        }
        GitMsg::Fetch => {
            let repo_path = active_path(state);
            state.git_busy = true;
            return blocking(move || {
                vcs::fetch(&repo_path)?;
                Ok("Fetched from origin".to_owned())
            });
        }
        GitMsg::Push => {
            let repo_path = active_path(state);
            state.git_busy = true;
            return blocking(move || {
                vcs::push(&repo_path)?;
                Ok("Pushed to origin".to_owned())
            });
        }
        GitMsg::Pull => {
            let repo_path = active_path(state);
            let repo_id = state.git_active_repo.clone();
            state.git_busy = true;
            return sync_task(repo_id, move || {
                let summary = vcs::pull(&repo_path)?;
                let collections = vcs::load_collections_from(&repo_path)?;
                let global_script = vcs::read_global_script(&repo_path);
                Ok((collections, global_script, format!("Pull: {summary}")))
            });
        }
        GitMsg::SwitchBranch(name) => {
            let repo_path = active_path(state);
            let repo_id = state.git_active_repo.clone();
            state.git_busy = true;
            return sync_task(repo_id, move || {
                vcs::checkout_branch(&repo_path, &name)?;
                let collections = vcs::load_collections_from(&repo_path)?;
                let global_script = vcs::read_global_script(&repo_path);
                Ok((collections, global_script, format!("Switched to {name}")))
            });
        }
        GitMsg::AskRestore(id) => {
            state.git_restore_confirm = Some(id);
        }
        GitMsg::CancelRestore => {
            state.git_restore_confirm = None;
        }
        GitMsg::RestoreCommit(id) => {
            state.git_restore_confirm = None;
            let repo_path = active_path(state);
            let repo_id = state.git_active_repo.clone();
            let short: String = id.chars().take(7).collect();
            state.git_busy = true;
            return sync_task(repo_id, move || {
                let collections = vcs::read_commit_collections(&repo_path, &id)?;
                let global_script = vcs::read_commit_global_script(&repo_path, &id);
                Ok((collections, global_script, format!("Restored {short}")))
            });
        }
        GitMsg::ToggleDiff => {
            if state.git_diff.is_some() {
                state.git_diff = None;
            } else {
                let repo_path = active_path(state);
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || vcs::working_diff(&repo_path))
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|r| r)
                    },
                    |res| match res {
                        Ok(diff) => Message::Git(GitMsg::DiffLoaded(diff)),
                        Err(e) => Message::Git(GitMsg::Error(e)),
                    },
                );
            }
        }
        GitMsg::DiffLoaded(diff) => {
            state.git_diff = Some(if diff.trim().is_empty() {
                "No uncommitted changes.".to_owned()
            } else {
                diff
            });
        }
        GitMsg::Synced(payload) => {
            let SyncPayload {
                repo_id,
                collections,
                label,
                global_script,
            } = *payload;
            // Save in-flight edits of open shared tabs as drafts before any
            // base row is replaced (01, C9). On failure apply nothing: tabs
            // and base stay as they are, and the next sync re-applies the
            // pulled tree. (No refresh_task here — it would write the stale
            // state back over the freshly pulled working tree.)
            if let Err(e) = crate::app::drafts::capture_tabs(state) {
                state.status_message = Some(format!(
                    "Sync not applied: could not save your local edits ({e}). Sync again to retry."
                ));
                state.git_busy = false;
                return Task::none();
            }
            let ids: Vec<String> = collections
                .iter()
                .map(|ic| ic.collection.id.clone())
                .collect();

            let incoming: HashSet<&String> = ids.iter().collect();
            for owned in owned_collection_ids(state, &repo_id) {
                if !incoming.contains(&owned) {
                    remove_collection(state, &owned);
                }
            }

            let draft_err = apply_sync(state, collections).err();
            apply_global_script(state, global_script);
            crate::app::drafts::refresh_open_tabs(state);
            if let Some(repo) = state.git_repos.iter_mut().find(|r| r.id == repo_id) {
                repo.collection_ids = ids;
            }
            repos::save(&state.data_dir, &state.git_repos);
            state.status_message = Some(match draft_err {
                Some(e) => format!("{label} — some local drafts could not be saved: {e}"),
                None => label,
            });
            state.git_busy = false;
            return refresh_task(state);
        }
        GitMsg::Done(message) => {
            state.status_message = Some(message);
            state.git_busy = false;
            return refresh_task(state);
        }
        GitMsg::Error(error) => {
            state.git_busy = false;
            state.status_message = Some(format!("Git error: {error}"));
        }
    }
    Task::none()
}

struct RepoAdd {
    name: String,
    path: PathBuf,
    remote_url: Option<String>,
    collections: Vec<ImportedCollection>,
    global_script: Option<(String, String)>,
}

fn repo_added(res: Result<RepoAdd, String>) -> Message {
    match res {
        Ok(add) => Message::Git(GitMsg::RepoAdded(Box::new(RepoAddPayload {
            name: add.name,
            path: add.path,
            remote_url: add.remote_url,
            collections: add.collections,
            global_script: add.global_script,
        }))),
        Err(e) => Message::Git(GitMsg::Error(e)),
    }
}

fn active_path(state: &AppState) -> PathBuf {
    state
        .git_repos
        .iter()
        .find(|r| r.id == state.git_active_repo)
        .map(|r| r.path.clone())
        .unwrap_or_else(|| state.data_dir.join("collections"))
}

fn collections_for_active(state: &AppState) -> Vec<ImportedCollection> {
    if state.git_active_repo == repos::LOCAL_ID {
        let claimed: HashSet<String> = state
            .git_repos
            .iter()
            .filter(|r| r.id != repos::LOCAL_ID)
            .flat_map(|r| r.collection_ids.iter().cloned())
            .collect();
        state
            .collections
            .iter()
            .filter(|c| !claimed.contains(&c.id))
            .map(|c| crate::app::tree::collection_from_state(state, c))
            .collect()
    } else {
        let ids: Vec<String> = state
            .git_repos
            .iter()
            .find(|r| r.id == state.git_active_repo)
            .map(|r| r.collection_ids.clone())
            .unwrap_or_default();
        state
            .collections
            .iter()
            .filter(|c| ids.contains(&c.id))
            .map(|c| crate::app::tree::collection_from_state(state, c))
            .collect()
    }
}

fn refresh_task(state: &AppState) -> Task<Message> {
    let repos = state.git_repos.clone();
    let active = state.git_active_repo.clone();
    let active_collections = collections_for_active(state);
    let global_pre_request_script = state.global_pre_request_editor.text();
    let global_test_script = state.global_test_editor.text();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                load_view(
                    repos,
                    active,
                    active_collections,
                    global_pre_request_script,
                    global_test_script,
                )
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r)
        },
        |res| match res {
            Ok(view) => Message::Git(GitMsg::Loaded(view)),
            Err(e) => Message::Git(GitMsg::Error(e)),
        },
    )
}

fn load_view(
    repos: Vec<GitRepo>,
    active: String,
    active_collections: Vec<ImportedCollection>,
    global_pre_request_script: String,
    global_test_script: String,
) -> Result<Box<GitView>, String> {
    let mut summaries = Vec::new();
    let mut status = None;
    let mut branches = Vec::new();
    let mut log = Vec::new();
    for repo in &repos {
        if repo.id == active {
            let _ = vcs::write_working_tree(
                &repo.path,
                &active_collections,
                &global_pre_request_script,
                &global_test_script,
            );
        }
        match vcs::status(&repo.path) {
            Ok(st) => {
                summaries.push(RepoSummary {
                    id: repo.id.clone(),
                    branch: st.branch.clone(),
                    ahead: st.ahead,
                    behind: st.behind,
                    changes: st.changes.len(),
                });
                if repo.id == active {
                    branches = vcs::branches(&repo.path).unwrap_or_default();
                    log = vcs::full_log(&repo.path, 50);
                    status = Some(st);
                }
            }
            Err(_) => summaries.push(RepoSummary {
                id: repo.id.clone(),
                branch: "—".to_owned(),
                ahead: 0,
                behind: 0,
                changes: 0,
            }),
        }
    }
    Ok(Box::new(GitView {
        summaries,
        status,
        branches,
        log,
    }))
}

fn blocking<F>(op: F) -> Task<Message>
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
{
    Task::perform(
        async move {
            tokio::task::spawn_blocking(op)
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
        },
        |res| match res {
            Ok(message) => Message::Git(GitMsg::Done(message)),
            Err(e) => Message::Git(GitMsg::Error(e)),
        },
    )
}

type SyncResult = (Vec<ImportedCollection>, Option<(String, String)>, String);

fn sync_task<F>(repo_id: String, op: F) -> Task<Message>
where
    F: FnOnce() -> Result<SyncResult, String> + Send + 'static,
{
    Task::perform(
        async move {
            tokio::task::spawn_blocking(op)
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
        },
        move |res| match res {
            Ok((collections, global_script, label)) => {
                Message::Git(GitMsg::Synced(Box::new(SyncPayload {
                    repo_id: repo_id.clone(),
                    collections,
                    label,
                    global_script,
                })))
            }
            Err(e) => Message::Git(GitMsg::Error(e)),
        },
    )
}

fn owned_collection_ids(state: &AppState, repo_id: &str) -> Vec<String> {
    let all: Vec<String> = state.collections.iter().map(|c| c.id.clone()).collect();
    owned_ids(&state.git_repos, &all, repo_id)
}

fn owned_ids(repos: &[GitRepo], all_collection_ids: &[String], repo_id: &str) -> Vec<String> {
    if repo_id == repos::LOCAL_ID {
        let claimed: HashSet<&String> = repos
            .iter()
            .filter(|r| r.id != repos::LOCAL_ID)
            .flat_map(|r| r.collection_ids.iter())
            .collect();
        all_collection_ids
            .iter()
            .filter(|id| !claimed.contains(id))
            .cloned()
            .collect()
    } else {
        repos
            .iter()
            .find(|r| r.id == repo_id)
            .map(|r| r.collection_ids.clone())
            .unwrap_or_default()
    }
}


/// Remove a collection, its folders and its requests from state and the DB.
fn remove_collection(state: &mut AppState, id: &str) {
    let group_ids = crate::app::tree::groups_under(state, id);
    state.groups.retain(|g| !group_ids.contains(&g.id));
    state.collections.retain(|c| c.id != id);
    let reqs = state.requests.remove(id).unwrap_or_default();
    if let Some(db) = &state.db {
        for req in &reqs {
            let _ = storage::delete_request(db, &req.id);
        }
        for gid in &group_ids {
            let _ = storage::delete_group(db, gid);
        }
        let _ = storage::delete_collection(db, id);
    }
}

/// Replace base rows with the pulled collections, then merge every local
/// draft onto its new base (01, C9). Drafts are never dropped here.
fn apply_sync(state: &mut AppState, collections: Vec<ImportedCollection>) -> Result<(), String> {
    for imported in collections {
        crate::app::tree::insert_collection(state, imported);
    }
    crate::app::drafts::rebase_all_after_sync(state)
}

/// Applies a repo's saved global script (if it has one) to live app state —
/// leaves the current global script alone if the repo has none saved, same
/// as `apply_sync` only ever adds/updates collections rather than wiping
/// ones a different repo owns.
fn apply_global_script(state: &mut AppState, global_script: Option<(String, String)>) {
    let Some((pre_request, test)) = global_script else {
        return;
    };
    state.global_pre_request_editor = iced::widget::text_editor::Content::with_text(&pre_request);
    state.global_test_editor = iced::widget::text_editor::Content::with_text(&test);
}

#[cfg(test)]
mod restore_tests {
    use super::*;

    fn repo(id: &str, cols: &[&str]) -> GitRepo {
        GitRepo {
            id: id.to_owned(),
            name: id.to_owned(),
            path: PathBuf::from("/tmp"),
            remote_url: None,
            collection_ids: cols.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn real_repo_owns_only_its_tracked_ids() {
        let repos = vec![
            repo(repos::LOCAL_ID, &[]),
            repo("r1", &["a", "b"]),
            repo("r2", &["c"]),
        ];
        let all = vec!["a".into(), "b".into(), "c".into(), "local".into()];
        // A restore on r1 must never reach c (r2) or local — only a, b are deletable.
        assert_eq!(owned_ids(&repos, &all, "r1"), vec!["a", "b"]);
    }

    #[test]
    fn local_owns_everything_unclaimed() {
        let repos = vec![repo(repos::LOCAL_ID, &[]), repo("r1", &["a", "b"])];
        let all = vec!["a".into(), "b".into(), "local1".into(), "local2".into()];
        assert_eq!(
            owned_ids(&repos, &all, repos::LOCAL_ID),
            vec!["local1", "local2"]
        );
    }
}
