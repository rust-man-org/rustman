//! The Git panel: what the user asks for becomes a queued job (see
//! [`jobs`]), whose result comes back as a message applied here or in
//! [`sync`].
//!
//! Git only ever sees saved requests. Unsaved tab edits stay in their tabs:
//! refreshing, switching repos or committing must never write them over the
//! saved versions behind the user's back.

mod jobs;
mod sync;
#[cfg(test)]
mod tests;
mod worker;

use std::collections::HashSet;
use std::path::PathBuf;

use iced::Task;

use rustman_core::store::Change;

use crate::app::AppState;
use crate::domain::group::ImportedCollection;
use crate::message::{GitMsg, GitView, Message};
use crate::services::repos::{self, GitRepo};
use crate::services::vcs;
use crate::state::git::{GitJob, GitPrompt, commit_identity};

pub(super) fn handle(state: &mut AppState, msg: GitMsg) -> Task<Message> {
    match msg {
        GitMsg::JobFinished(msg) => {
            let handled = handle(state, *msg);
            Task::batch([handled, jobs::finished(state)])
        }
        GitMsg::Refresh => jobs::enqueue(state, GitJob::Refresh),
        GitMsg::Loaded(view) => {
            loaded(state, *view);
            Task::none()
        }
        GitMsg::DefaultIdentityLoaded(identity) => {
            state.git.default_identity = identity;
            Task::none()
        }
        GitMsg::BranchLoaded { repo, branch } => {
            if repo == state.git_active_repo {
                state.git_branch = branch;
            }
            Task::none()
        }
        GitMsg::SelectRepo(id) => activate_repo(state, id),
        GitMsg::CloneRepo => clone(state),
        GitMsg::OpenFolder => pick_folder(),
        GitMsg::FolderPicked(picked) => picked.map_or_else(Task::none, probe),
        GitMsg::FolderProbed { path, folder } => folder_probed(state, path, folder),
        GitMsg::ConfirmInit => confirm_init(state),
        GitMsg::DismissPrompt => match state.git.prompt.take() {
            Some(GitPrompt::Switch { .. }) => {
                iced::widget::operation::focus(crate::ui::sidebar::git::COMMIT_INPUT_ID)
            }
            _ => Task::none(),
        },
        GitMsg::RepoAdded(payload) => sync::repo_added(state, *payload),
        GitMsg::Synced(payload) => sync::synced(state, *payload),
        GitMsg::RemoveRepo(id) => ask_remove_repo(state, &id),
        GitMsg::ConfirmRemoveRepo {
            id,
            keep_collections,
        } => remove_repo(state, &id, keep_collections),
        GitMsg::ReloadTab(tab) => {
            sync::reload_tab(state, &tab);
            Task::none()
        }
        GitMsg::KeepMine(tab) => {
            sync::keep_mine(state, &tab);
            Task::none()
        }
        GitMsg::SwitchBlocked { branch, changes } => {
            state.git.prompt = Some(GitPrompt::Switch { branch, changes });
            Task::none()
        }
        GitMsg::AskRestore(id) => {
            state.git_restore_confirm = Some(id);
            Task::none()
        }
        GitMsg::CancelRestore => {
            state.git_restore_confirm = None;
            Task::none()
        }
        GitMsg::DiffLoaded {
            repo,
            generation,
            diff,
        } => {
            if is_current(state, &repo, generation) {
                state.git_diff = Some(if diff.trim().is_empty() {
                    "No uncommitted changes.".to_owned()
                } else {
                    diff
                });
            }
            Task::none()
        }
        GitMsg::Done(message) => {
            state.status_message = Some(message);
            jobs::enqueue(state, GitJob::Refresh)
        }
        GitMsg::Error(error) => {
            state.status_message = Some(format!("Git error: {error}"));
            Task::none()
        }
        GitMsg::CloneUrlChanged(value) => edit(&mut state.git_clone_url, value),
        GitMsg::CommitMessageChanged(value) => edit(&mut state.git_commit_message, value),
        GitMsg::HistorySearchChanged(value) => edit(&mut state.git_history_search, value),
        GitMsg::RemoteUrlChanged(value) => edit(&mut state.git_remote_input, value),
        GitMsg::NewBranchNameChanged(value) => edit(&mut state.git_new_branch, value),
        action => act(state, action),
    }
}

fn edit(field: &mut String, value: String) -> Task<Message> {
    *field = value;
    Task::none()
}

/// The panel's actions on the active repository, each queued as a job.
fn act(state: &mut AppState, action: GitMsg) -> Task<Message> {
    let repo = state.git_active_repo.clone();
    let job = match action {
        GitMsg::Commit => return commit(state),
        GitMsg::SetRemote => {
            let url = state.git_remote_input.trim().to_owned();
            if url.is_empty() {
                return Task::none();
            }
            GitJob::SetRemote { repo, url }
        }
        GitMsg::CreateBranch => {
            let name = std::mem::take(&mut state.git_new_branch).trim().to_owned();
            if name.is_empty() {
                return Task::none();
            }
            GitJob::CreateBranch { repo, name }
        }
        GitMsg::Fetch => GitJob::Fetch { repo },
        GitMsg::Push => GitJob::Push { repo },
        GitMsg::Pull => GitJob::Pull { repo },
        GitMsg::SwitchBranch(branch) => GitJob::Switch {
            repo,
            branch,
            discard: false,
        },
        GitMsg::DiscardAndSwitch(branch) => {
            state.git.prompt = None;
            GitJob::Switch {
                repo,
                branch,
                discard: true,
            }
        }
        GitMsg::RestoreCommit(commit) => {
            state.git_restore_confirm = None;
            GitJob::Restore { repo, commit }
        }
        GitMsg::ToggleDiff => {
            if state.git_diff.take().is_some() {
                return Task::none();
            }
            GitJob::Diff { repo }
        }
        _ => return Task::none(),
    };
    jobs::enqueue(state, job)
}

/// Whether a result read for `repo` in `generation` still describes what
/// the panel shows.
fn is_current(state: &AppState, repo: &str, generation: u64) -> bool {
    repo == state.git_active_repo && generation == state.git.generation
}

fn loaded(state: &mut AppState, view: GitView) {
    if !is_current(state, &view.repo, view.generation) {
        return;
    }
    state.git_repo_summaries = view.summaries;
    state.git_status = view.status;
    state.git_branch = state.git_status.as_ref().map(|s| s.branch.clone());
    state.git_branches = view.branches;
    state.git_log = view.log;
    state.sidebar.git_removed_names = view.removed_names;
    state.git.warnings = view.warnings;
    state.git.repo_identity = view.identity;
    sync::prune_changed_upstream(state);
}

fn commit(state: &mut AppState) -> Task<Message> {
    let message = state.git_commit_message.trim().to_owned();
    if message.is_empty() {
        state.status_message = Some("Enter a commit message first".to_owned());
        return Task::none();
    }
    let repo = state.git_active_repo.clone();
    let Some(identity) = commit_identity(
        &state.git_user_name,
        &state.git_user_email,
        state.git.repo_identity.as_ref(),
    ) else {
        state.status_message =
            Some("Set your name and email in Settings → Git, or in your git config".to_owned());
        return Task::none();
    };
    state.git_commit_message.clear();
    jobs::enqueue(
        state,
        GitJob::Commit {
            repo,
            message,
            identity,
        },
    )
}

fn clone(state: &mut AppState) -> Task<Message> {
    let url = state.git_clone_url.trim().to_owned();
    if url.is_empty() {
        return Task::none();
    }
    if let Some(reason) = sync::already_added(&state.git_repos, None, Some(&url)) {
        state.status_message = Some(reason);
        return Task::none();
    }
    state.git_clone_url.clear();
    jobs::enqueue(state, GitJob::Clone { url })
}

fn pick_folder() -> Task<Message> {
    Task::perform(
        async {
            rfd::AsyncFileDialog::new()
                .pick_folder()
                .await
                .map(|h| h.path().to_path_buf())
        },
        |picked| Message::Git(GitMsg::FolderPicked(picked)),
    )
}

/// Looks at the picked folder before anything is added or created.
fn probe(path: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            let probed = path.clone();
            let folder = tokio::task::spawn_blocking(move || vcs::probe(&probed))
                .await
                .unwrap_or(vcs::Folder::Missing);
            (path, folder)
        },
        |(path, folder)| Message::Git(GitMsg::FolderProbed { path, folder }),
    )
}

/// A repository is added; any other folder is only made one once the user
/// says so.
fn folder_probed(state: &mut AppState, path: PathBuf, folder: vcs::Folder) -> Task<Message> {
    if let Some(reason) = sync::already_added(&state.git_repos, Some(&path), None) {
        state.status_message = Some(reason);
        return Task::none();
    }
    match folder {
        vcs::Folder::Repository => jobs::enqueue(state, GitJob::AddFolder { path, init: false }),
        vcs::Folder::Missing => {
            state.status_message = Some(format!("{} doesn't exist", path.display()));
            Task::none()
        }
        vcs::Folder::Plain { entries, enclosing } => {
            state.git.prompt = Some(GitPrompt::Init {
                path,
                entries,
                enclosing,
            });
            Task::none()
        }
    }
}

fn confirm_init(state: &mut AppState) -> Task<Message> {
    match state.git.prompt.take() {
        Some(GitPrompt::Init { path, .. }) => {
            jobs::enqueue(state, GitJob::AddFolder { path, init: true })
        }
        other => {
            state.git.prompt = other;
            Task::none()
        }
    }
}

/// Asks what becomes of the repository's collections; one without any
/// goes at once.
fn ask_remove_repo(state: &mut AppState, id: &str) -> Task<Message> {
    let Some(repo) = state
        .git_repos
        .iter()
        .find(|r| r.id == id && r.id != repos::LOCAL_ID)
    else {
        return Task::none();
    };
    let collections = repo
        .collection_ids
        .iter()
        .filter(|c| state.collections.iter().any(|known| &known.id == *c))
        .count();
    if collections == 0 {
        return remove_repo(state, id, true);
    }
    state.git.prompt = Some(GitPrompt::RemoveRepo {
        id: id.to_owned(),
        name: repo.name.clone(),
        collections,
    });
    Task::none()
}

/// Forgets the repository; its folder and history stay on disk. Its
/// collections leave the workspace, or stay in it outside every
/// repository, so Local never adopts them and a Local branch switch can't
/// take them away.
fn remove_repo(state: &mut AppState, id: &str, keep_collections: bool) -> Task<Message> {
    state.git.prompt = None;
    let Some(index) = state
        .git_repos
        .iter()
        .position(|r| r.id == id && r.id != repos::LOCAL_ID)
    else {
        return Task::none();
    };
    let repo = state.git_repos.remove(index);
    if keep_collections {
        let kept = repo
            .collection_ids
            .iter()
            .filter(|c| !state.git.local_only.contains(c));
        state
            .git
            .local_only
            .extend(kept.cloned().collect::<Vec<_>>());
        repos::save_local_only(&state.data_dir, &state.git.local_only);
    } else {
        for collection in &repo.collection_ids {
            remove_collection(state, collection);
        }
    }
    repos::save(&state.data_dir, &state.git_repos);
    state.status_message = Some(format!(
        "Removed {} from Rustman; its folder is untouched",
        repo.name
    ));
    if state.git_active_repo == id {
        return activate_repo(state, repos::LOCAL_ID.to_owned());
    }
    jobs::enqueue(state, GitJob::Refresh)
}

/// Makes `id` the active repo. What the panel showed for the previous one
/// is cleared at once, so nothing on screen acts on the wrong repository.
/// The branch is read on its own first: HEAD is one file, while the full
/// refresh writes and diffs the working tree.
fn activate_repo(state: &mut AppState, id: String) -> Task<Message> {
    state.git_active_repo = id;
    state.git.generation += 1;
    state.git.prompt = None;
    state.git.warnings.clear();
    state.git.repo_identity = None;
    state.git_status = None;
    state.git_branches.clear();
    state.git_log.clear();
    state.git_diff = None;
    state.git_restore_confirm = None;
    Task::batch([branch_task(state), jobs::enqueue(state, GitJob::Refresh)])
}

/// Reads the active repo's branch off the UI thread.
pub(crate) fn branch_task(state: &AppState) -> Task<Message> {
    let repo = state.git_active_repo.clone();
    let Some(path) = jobs::repo_path(state, &repo) else {
        return Task::none();
    };
    Task::perform(
        async move {
            let branch = tokio::task::spawn_blocking(move || vcs::current_branch(&path))
                .await
                .ok()
                .flatten();
            (repo, branch)
        },
        |(repo, branch)| Message::Git(GitMsg::BranchLoaded { repo, branch }),
    )
}

/// Reads the global git identity off the UI thread, for Settings to show.
pub(crate) fn default_identity_task() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(vcs::default_identity)
                .await
                .ok()
                .flatten()
        },
        |identity| Message::Git(GitMsg::DefaultIdentityLoaded(identity)),
    )
}

/// What a job for the active repository writes to git: its collections as
/// saved, read the way the job reads them.
#[cfg(test)]
fn collections_for_active(state: &AppState) -> Vec<ImportedCollection> {
    let ids = owned_collection_ids(state, &state.git_active_repo);
    let store = state.store.as_ref().expect("tests boot with a store");
    store.now(move |s| jobs::read_collections(s.connection(), &ids))
}

/// The collections `repo_id` holds. Local holds every one no other
/// repository claims, except those kept out of git altogether.
fn owned_collection_ids(state: &AppState, repo_id: &str) -> Vec<String> {
    let all: Vec<String> = state
        .collections
        .iter()
        .map(|c| c.id.clone())
        .filter(|id| !state.git.local_only.contains(id))
        .collect();
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
    let mut changes: Vec<Change> = reqs
        .into_iter()
        .map(|r| Change::DeleteRequest { id: r.id })
        .collect();
    changes.extend(group_ids.into_iter().map(|id| Change::DeleteGroup { id }));
    changes.push(Change::DeleteCollection { id: id.to_owned() });
    state.write(changes);
}

fn apply_sync(state: &mut AppState, collections: Vec<ImportedCollection>) {
    for imported in collections {
        crate::app::tree::insert_collection(state, imported);
    }
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

/// Tabs with unsaved edits to a request in the collections `committed`.
/// Those edits are not part of the commit until the user saves them.
fn unsaved_request_tabs(state: &AppState, committed: &[String]) -> usize {
    state
        .tabs
        .tabs
        .iter()
        .filter(|t| t.modified)
        .filter(|t| {
            t.saved_as
                .as_ref()
                .is_some_and(|(col, _)| committed.contains(col))
        })
        .count()
}

/// A note for the status line when a commit leaves unsaved edits out.
fn unsaved_note(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 tab has unsaved changes that aren't in this commit. Save it (Ctrl+S) to include them.".to_owned()),
        n => Some(format!("{n} tabs have unsaved changes that aren't in this commit. Save them (Ctrl+S) to include them.")),
    }
}

#[cfg(test)]
mod unsaved_tab_tests {
    use super::*;
    use crate::domain::collection::{Collection, SavedRequest};
    use crate::test_support::TempDir;

    /// Collection "c" in the local repo with saved request "r1", open in a
    /// tab that has unsaved edits.
    fn app_with_dirty_tab() -> (AppState, TempDir) {
        let dir = TempDir::new("git-unsaved");
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let col = Collection {
            id: "c".into(),
            name: "c".into(),
            created_at: 0,
        };
        let mut req = SavedRequest::new_in("c".into(), "r1".into());
        req.id = "r1".into();
        req.url = "https://saved.example".into();
        req.pre_request_script = "print(\"saved\")".into();
        let db = state.db();
        crate::services::storage::create_collection(&db, &col).unwrap();
        crate::services::storage::create_request(&db, &req).unwrap();
        state.collections.push(col);
        state.requests.entry("c".into()).or_default().push(req);

        let tab = state.tabs.active_tab_mut();
        tab.saved_as = Some(("c".into(), "r1".into()));
        tab.set_url("https://unsaved.example".into());
        tab.modified = true;
        (state, dir)
    }

    /// The commit writes `collections_for_active`; checking that data (and
    /// that no handler flushed the tab synchronously) covers what lands in
    /// git without running the blocking commit job.
    #[test]
    fn commits_write_saved_requests_not_tab_edits() {
        let (mut state, _dir) = app_with_dirty_tab();
        let _ = handle(&mut state, GitMsg::Refresh);
        let _ = handle(&mut state, GitMsg::SelectRepo("local".into()));
        state.git_commit_message = "msg".into();
        let _ = handle(&mut state, GitMsg::Commit);

        let committed = collections_for_active(&state);
        let request = &committed[0].requests[0];
        assert_eq!(request.url, "https://saved.example");
        assert_eq!(request.pre_request_script, "print(\"saved\")");
        let in_db = crate::services::storage::get_requests(&state.db(), "c").unwrap();
        assert_eq!(in_db[0].url, "https://saved.example");
        assert!(
            state.tabs.active_tab().modified,
            "the tab keeps its unsaved edits"
        );
    }

    #[test]
    fn the_commit_note_counts_only_tabs_of_committed_collections() {
        let (mut state, _dir) = app_with_dirty_tab();
        let committed = owned_collection_ids(&state, &state.git_active_repo);
        assert_eq!(unsaved_request_tabs(&state, &committed), 1);

        state.git_repos.push(GitRepo {
            id: "other".into(),
            name: "Other".into(),
            path: state.data_dir.join("repos/other"),
            remote_url: None,
            collection_ids: vec!["c".into()],
        });
        let local = owned_collection_ids(&state, &state.git_active_repo);
        assert!(local.is_empty(), "c now belongs to the other repo");
        assert_eq!(
            unsaved_request_tabs(&state, &local),
            0,
            "repo B's tab is not repo A's business"
        );
        assert!(unsaved_note(1).unwrap().starts_with("1 tab"));
        assert!(unsaved_note(0).is_none());
    }
}
