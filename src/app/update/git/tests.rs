//! The panel's handling of git work, driven through the handler with the
//! blocking halves run inline against throwaway repositories.

use std::path::Path;

use super::worker::{self, Tree};
use super::*;
use crate::message::{RepoAddPayload, RepoHealth, SyncPayload};
use crate::services::vcs::{FileProblem, LoadedCollections, UnreadableFile};
use crate::test_support::{COLLECTION_ID, TempDir, sample_collection, second_collection};

fn app(label: &str) -> (AppState, TempDir) {
    let dir = TempDir::new(label);
    (crate::app::boot::load(dir.path().join("data")), dir)
}

/// Runs whatever job the handler started, inline, and applies its result.
fn settle(state: &mut AppState, msg: GitMsg) {
    let _ = handle(state, msg);
}

fn repo_at(path: &Path, id: &str) -> GitRepo {
    GitRepo {
        id: id.into(),
        name: id.into(),
        path: path.to_path_buf(),
        remote_url: None,
        collection_ids: Vec::new(),
    }
}

fn tree(path: &Path, collections: Vec<ImportedCollection>) -> Tree {
    Tree {
        path: path.to_path_buf(),
        collections,
        pre_request_script: String::new(),
        test_script: String::new(),
    }
}

fn with_collections(state: &mut AppState, collections: Vec<ImportedCollection>) {
    for c in collections {
        crate::app::tree::insert_collection(state, c);
    }
}

fn names(state: &AppState) -> Vec<String> {
    let mut out: Vec<String> = state.collections.iter().map(|c| c.name.clone()).collect();
    out.sort();
    out
}

fn synced(repo: &str, loaded: LoadedCollections) -> GitMsg {
    GitMsg::Synced(Box::new(SyncPayload {
        repo_id: repo.into(),
        loaded,
        label: "Pull: done".into(),
        global_script: None,
    }))
}

// ── Opening folders (bugs 1 and 2) ────────────────────────────────────────────

#[test]
fn opening_a_plain_folder_asks_before_initialising_it() {
    let (mut state, dir) = app("git-open-plain");
    let folder = dir.path().join("project");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("package.json"), "{}").unwrap();
    let folder_state = vcs::probe(&folder);

    settle(
        &mut state,
        GitMsg::FolderProbed {
            path: folder.clone(),
            folder: folder_state,
        },
    );
    assert!(matches!(
        state.git.prompt,
        Some(GitPrompt::Init { entries: 1, .. })
    ));
    assert!(!state.git.jobs.is_busy(), "nothing runs before the answer");
    assert!(!folder.join(".git").exists());

    settle(&mut state, GitMsg::DismissPrompt);
    assert!(state.git.prompt.is_none());
    assert!(!folder.join(".git").exists(), "cancelling creates nothing");

    settle(
        &mut state,
        GitMsg::FolderProbed {
            path: folder.clone(),
            folder: vcs::probe(&folder),
        },
    );
    settle(&mut state, GitMsg::ConfirmInit);
    assert!(
        state.git.jobs.is_busy(),
        "initialising is queued once confirmed"
    );
    let GitMsg::RepoAdded(added) = worker::add_folder(folder.clone(), true) else {
        panic!("the folder is added");
    };
    assert!(folder.join(".git").is_dir());
    assert_eq!(
        std::fs::read_to_string(folder.join("package.json")).unwrap(),
        "{}"
    );
    assert!(added.loaded.collections.is_empty());
}

#[test]
fn a_refresh_of_an_opened_project_keeps_its_own_json_files() {
    let dir = TempDir::new("git-open-project");
    let project = dir.path().join("web");
    vcs::init(&project).unwrap();
    std::fs::write(project.join("package.json"), r#"{"name": "web"}"#).unwrap();
    std::fs::write(project.join("tsconfig.json"), "{ // jsonc\n}").unwrap();
    let repos = [repo_at(&project, "web")];

    for _ in 0..2 {
        worker::refresh(
            &repos,
            "web".into(),
            0,
            &tree(&project, vec![second_collection()]),
        );
    }
    assert!(project.join("package.json").exists());
    assert!(project.join("tsconfig.json").exists());
    assert!(project.join("col-auth.json").exists());
}

#[test]
fn a_missing_repository_is_reported_and_never_recreated() {
    let dir = TempDir::new("git-missing");
    let gone = dir.path().join("unmounted");
    let repos = [repo_at(&gone, "team")];

    let GitMsg::Loaded(view) = worker::refresh(
        &repos,
        "team".into(),
        0,
        &tree(&gone, vec![second_collection()]),
    ) else {
        panic!("a refresh always answers with a view");
    };
    assert_eq!(view.summaries[0].health, RepoHealth::Missing);
    assert!(
        view.warnings[0].contains("does not exist"),
        "{:?}",
        view.warnings
    );
    assert!(!gone.exists(), "nothing was recreated");
}

#[test]
fn the_local_repository_is_created_on_first_refresh() {
    let (state, _dir) = app("git-local");
    let local = state.git_repos[0].clone();
    worker::refresh(
        &state.git_repos,
        local.id.clone(),
        0,
        &tree(&local.path, Vec::new()),
    );
    assert!(local.path.join(".git").is_dir());
}

#[test]
fn the_same_folder_or_remote_is_not_added_twice() {
    let (mut state, dir) = app("git-dup");
    let path = dir.path().join("api");
    let payload = |remote: Option<&str>| {
        GitMsg::RepoAdded(Box::new(RepoAddPayload {
            name: "api".into(),
            path: path.clone(),
            remote_url: remote.map(str::to_owned),
            loaded: LoadedCollections::default(),
            global_script: None,
        }))
    };
    settle(
        &mut state,
        payload(Some("https://example.invalid/team/api.git")),
    );
    assert_eq!(state.git_repos.len(), 2);
    settle(&mut state, payload(None));
    assert_eq!(state.git_repos.len(), 2, "same folder");
    assert!(
        state
            .status_message
            .as_deref()
            .unwrap()
            .contains("already added")
    );

    state.git_clone_url = "https://example.invalid/team/api/".into();
    settle(&mut state, GitMsg::CloneRepo);
    assert!(
        state
            .status_message
            .as_deref()
            .unwrap()
            .contains("already syncs")
    );
    assert!(
        !state.git_clone_url.is_empty(),
        "same remote: nothing is cloned"
    );
}

// ── Syncing (bug 5 and secrets) ───────────────────────────────────────────────

#[test]
fn an_unreadable_file_keeps_its_collection_while_a_deleted_one_goes() {
    let (mut state, _dir) = app("git-unreadable");
    with_collections(&mut state, vec![sample_collection(), second_collection()]);
    let loaded = LoadedCollections {
        collections: Vec::new(),
        unreadable: vec![UnreadableFile {
            file: format!("{COLLECTION_ID}.json"),
            problem: FileProblem::Malformed("EOF while parsing".into()),
        }],
    };

    settle(&mut state, synced(repos::LOCAL_ID, loaded));

    assert_eq!(
        names(&state),
        ["Storefront API"],
        "col-auth's file is gone, so it goes too"
    );
    assert!(state.requests.contains_key(COLLECTION_ID));
    let status = state.status_message.clone().unwrap();
    assert!(
        status.contains("col-storefront.json") && status.contains("left as it was"),
        "{status}"
    );
}

#[test]
fn pulled_files_without_secrets_keep_the_local_ones() {
    let (mut state, _dir) = app("git-secrets-pull");
    with_collections(&mut state, vec![sample_collection()]);
    let repo = TempDir::new("git-secrets-repo");
    vcs::init(repo.path()).unwrap();
    vcs::write_working_tree(repo.path(), &[sample_collection()], "", "").unwrap();
    let loaded = vcs::load_collections_from(repo.path()).unwrap();

    settle(&mut state, synced(repos::LOCAL_ID, loaded));

    let full = state.requests[COLLECTION_ID]
        .iter()
        .find(|r| r.id == "req-full")
        .unwrap();
    assert_eq!(full.bearer_token, "bearer-token-123");
    assert_eq!(full.jwt_secret, "jwt-signing-secret");
    assert_eq!(full.cookie_string, "session=abc; theme=dark");
    let cookies: Vec<&str> = full.cookies.iter().map(|c| c.value.as_str()).collect();
    assert_eq!(
        cookies,
        ["abc", "dark"],
        "cookie rows came back blank from git"
    );
}

#[test]
fn a_legacy_repository_with_secrets_fills_an_empty_workspace() {
    let (mut state, dir) = app("git-secrets-legacy");
    let repo = dir.path().join("legacy");
    vcs::init(&repo).unwrap();
    std::fs::write(
        repo.join(format!("{COLLECTION_ID}.json")),
        crate::test_support::read_golden("legacy/git_collection_with_secrets.json"),
    )
    .unwrap();
    let GitMsg::RepoAdded(added) = worker::add_folder(repo, false) else {
        panic!("the repository is added");
    };

    settle(&mut state, GitMsg::RepoAdded(added));

    let full = state.requests[COLLECTION_ID]
        .iter()
        .find(|r| r.id == "req-full")
        .unwrap();
    assert_eq!(full.bearer_token, "bearer-token-123");
    assert_eq!(full.basic_pass, "s3cret: with spaces");
}

// ── Branch switches (bug 4) ───────────────────────────────────────────────────

#[test]
fn a_saved_but_uncommitted_edit_blocks_a_switch_until_discarded() {
    let dir = TempDir::new("git-switch");
    let path = dir.path();
    vcs::init(path).unwrap();
    let identity = vcs::GitIdentity {
        name: "T".into(),
        email: "t@example.invalid".into(),
    };
    vcs::write_working_tree(path, &[second_collection()], "", "").unwrap();
    vcs::commit(path, &identity, "first").unwrap();
    vcs::create_branch(path, "feature").unwrap();
    let mut edited = second_collection();
    edited.collection.name = "Saved, never written to git".into();

    let blocked = worker::switch(
        "r".into(),
        &tree(path, vec![edited]),
        "feature".into(),
        false,
    );
    let GitMsg::SwitchBlocked { branch, changes } = blocked else {
        panic!("the switch must be refused, got {blocked:?}");
    };
    assert_eq!(
        (branch.as_str(), changes.as_slice()),
        ("feature", ["col-auth.json".to_owned()].as_slice())
    );

    let (mut state, _app_dir) = app("git-switch-prompt");
    settle(&mut state, GitMsg::SwitchBlocked { branch, changes });
    assert!(matches!(state.git.prompt, Some(GitPrompt::Switch { .. })));
    settle(&mut state, GitMsg::DiscardAndSwitch("feature".into()));
    assert!(state.git.prompt.is_none());
    assert!(state.git.jobs.is_busy(), "the discarding switch is queued");
}

// ── Stale results (bug 7) ─────────────────────────────────────────────────────

#[test]
fn results_for_another_repository_or_an_earlier_switch_are_dropped() {
    let (mut state, dir) = app("git-stale");
    state.git_repos.push(repo_at(&dir.path().join("b"), "b"));
    let view = |repo: &str, generation| {
        GitMsg::Loaded(Box::new(GitView {
            repo: repo.into(),
            generation,
            summaries: Vec::new(),
            status: None,
            branches: Vec::new(),
            log: Vec::new(),
            removed_names: std::collections::HashMap::new(),
            warnings: vec![format!("from {repo}")],
            identity: None,
        }))
    };
    let before = state.git.generation;
    settle(&mut state, GitMsg::SelectRepo("b".into()));
    let now = state.git.generation;

    settle(&mut state, view(repos::LOCAL_ID, now));
    settle(&mut state, view("b", before));
    settle(
        &mut state,
        GitMsg::DiffLoaded {
            repo: "b".into(),
            generation: before,
            diff: "old".into(),
        },
    );
    assert!(state.git.warnings.is_empty() && state.git_diff.is_none());

    settle(&mut state, view("b", now));
    assert_eq!(state.git.warnings, ["from b"]);
}

// ── Open tabs after a pull (bug 6) ────────────────────────────────────────────

/// Local holds `second_collection()`, open in the active tab; the pull brings
/// in the same request renamed.
fn pulled_rename(state: &mut AppState) -> (String, GitMsg) {
    with_collections(state, vec![second_collection()]);
    let request = state.requests["col-auth"][0].clone();
    state.tabs.open_request(&request);
    let tab = state.tabs.active_tab().id.clone();
    let mut pulled = second_collection();
    pulled.requests[0].name = "Renamed upstream".into();
    let loaded = LoadedCollections {
        collections: vec![pulled],
        unreadable: Vec::new(),
    };
    (tab, synced(repos::LOCAL_ID, loaded))
}

#[test]
fn a_clean_tab_shows_the_pulled_version_and_stays_the_same_tab() {
    let (mut state, _dir) = app("git-tab-clean");
    let (tab, pull) = pulled_rename(&mut state);
    state.tabs.active_tab_mut().active_request_tab = crate::message::RequestTab::Scripts;

    settle(&mut state, pull);

    let shown = state.tabs.active_tab();
    assert_eq!(shown.id, tab, "in-flight responses still find it");
    assert_eq!(shown.title, "Renamed upstream");
    assert_eq!(
        shown.active_request_tab,
        crate::message::RequestTab::Scripts
    );
    assert!(!shown.modified && state.git.changed_upstream.is_empty());
}

#[test]
fn a_pull_never_loses_the_pulled_version_or_the_unsaved_edits() {
    let (mut state, _dir) = app("git-tab-dirty");
    let (tab, pull) = pulled_rename(&mut state);
    state
        .tabs
        .active_tab_mut()
        .set_url("https://my.unsaved.edit".into());
    state.tabs.active_tab_mut().modified = true;

    settle(&mut state, pull);

    assert_eq!(
        state.requests["col-auth"][0].name, "Renamed upstream",
        "the pull is saved"
    );
    let open = state.tabs.active_tab();
    assert_eq!(
        (open.id.as_str(), open.url.as_str()),
        (tab.as_str(), "https://my.unsaved.edit")
    );
    assert!(open.modified);
    assert_eq!(state.git.changed_upstream, std::slice::from_ref(&tab));

    settle(&mut state, GitMsg::KeepMine(tab.clone()));
    assert!(state.git.changed_upstream.is_empty());
    assert!(state.tabs.active_tab().modified, "kept edits stay unsaved");
    assert_eq!(
        state.requests["col-auth"][0].name, "Renamed upstream",
        "nothing saved over it"
    );

    state.git.changed_upstream.push(tab.clone());
    settle(&mut state, GitMsg::ReloadTab(tab));
    let reloaded = state.tabs.active_tab();
    assert_eq!(reloaded.title, "Renamed upstream");
    assert!(!reloaded.modified && state.git.changed_upstream.is_empty());
}

// ── Removing a repository (bug 11) ────────────────────────────────────────────

fn with_team_repo(state: &mut AppState, dir: &Path) {
    with_collections(state, vec![second_collection()]);
    let mut team = repo_at(&dir.join("team"), "team");
    team.collection_ids = vec!["col-auth".into()];
    state.git_repos.push(team);
}

#[test]
fn removing_a_repository_asks_what_becomes_of_its_collections() {
    let (mut state, dir) = app("git-remove-ask");
    with_team_repo(&mut state, dir.path());
    settle(&mut state, GitMsg::RemoveRepo("team".into()));
    assert!(matches!(
        state.git.prompt,
        Some(GitPrompt::RemoveRepo { collections: 1, .. })
    ));
    assert_eq!(
        state.git_repos.len(),
        2,
        "nothing removed before the answer"
    );
}

#[test]
fn kept_collections_stay_out_of_local() {
    let (mut state, dir) = app("git-remove-keep");
    with_team_repo(&mut state, dir.path());
    settle(
        &mut state,
        GitMsg::ConfirmRemoveRepo {
            id: "team".into(),
            keep_collections: true,
        },
    );

    assert_eq!(state.git_repos.len(), 1);
    assert_eq!(names(&state), ["Auth service"], "still in the workspace");
    assert!(
        owned_collection_ids(&state, repos::LOCAL_ID).is_empty(),
        "Local doesn't adopt it"
    );
    assert_eq!(repos::load_local_only(&state.data_dir), ["col-auth"]);
}

#[test]
fn removed_collections_leave_the_workspace() {
    let (mut state, dir) = app("git-remove-delete");
    with_team_repo(&mut state, dir.path());
    settle(
        &mut state,
        GitMsg::ConfirmRemoveRepo {
            id: "team".into(),
            keep_collections: false,
        },
    );
    assert_eq!(state.git_repos.len(), 1);
    assert!(names(&state).is_empty());
}

// ── Commit identity (bug 12) ──────────────────────────────────────────────────

fn identity(name: &str, email: &str) -> vcs::GitIdentity {
    vcs::GitIdentity {
        name: name.into(),
        email: email.into(),
    }
}

#[test]
fn without_any_identity_a_commit_keeps_the_typed_message() {
    let (mut state, _dir) = app("git-identity-missing");
    state.git_user_name.clear();
    state.git_user_email.clear();
    state.git.repo_identity = None;
    state.git_commit_message = "my message".into();
    settle(&mut state, GitMsg::Commit);
    assert_eq!(state.git_commit_message, "my message");
    assert!(
        state
            .status_message
            .as_deref()
            .unwrap()
            .contains("Settings → Git")
    );
}

#[test]
fn the_shown_author_is_the_commit_author() {
    let (mut state, dir) = app("git-identity-shown");
    let repo = dir.path().join("work");
    vcs::init(&repo).unwrap();
    state.git_user_name.clear();
    state.git_user_email.clear();
    state.git.repo_identity = Some(identity("Work Ada", "ada@work.invalid"));
    let shown = crate::state::git::commit_identity(
        &state.git_user_name,
        &state.git_user_email,
        state.git.repo_identity.as_ref(),
    )
    .unwrap();

    worker::commit(
        &tree(&repo, vec![second_collection()]),
        &shown,
        "first",
        None,
    );

    // Run from a git hook, these would point the CLI at the outer repository.
    let author = std::process::Command::new("git")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .arg("-C")
        .arg(&repo)
        .args(["log", "-1", "--format=%an <%ae>"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&author.stdout).trim(),
        format!("{} <{}>", shown.name, shown.email)
    );
}

#[test]
fn a_loaded_view_brings_the_repositorys_identity_without_touching_settings() {
    let (mut state, _dir) = app("git-identity-loaded");
    state.git_user_name = "Override".into();
    state.git_user_email = String::new();
    let view = GitView {
        repo: state.git_active_repo.clone(),
        generation: state.git.generation,
        summaries: Vec::new(),
        status: None,
        branches: Vec::new(),
        log: Vec::new(),
        removed_names: std::collections::HashMap::new(),
        warnings: Vec::new(),
        identity: Some(identity("Work Ada", "ada@work.invalid")),
    };
    settle(&mut state, GitMsg::Loaded(Box::new(view)));
    assert_eq!(
        (state.git_user_name.as_str(), state.git_user_email.as_str()),
        ("Override", "")
    );
    assert_eq!(
        state.git.repo_identity,
        Some(identity("Work Ada", "ada@work.invalid"))
    );
}
