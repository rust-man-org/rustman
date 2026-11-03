//! Local draft orchestration (workstream 01, contract C9).
//!
//! The rules, in one place:
//! - Only requests in a **shared** collection (owned by a repo with a remote)
//!   get drafts. Purely local collections keep saving straight to base.
//! - Editing a shared request writes its draft, never the base row, so the
//!   repo writer (which reads `state.requests`) never sees local edits.
//! - Sync replaces base rows; [`rebase_all_after_sync`] then merges every
//!   draft onto its new base. Drafts are never deleted as a side effect.
//! - Persist first, then mutate memory: a failed write leaves state as it was.

use std::collections::HashMap;

use crate::app::AppState;
use crate::domain::collection::SavedRequest;
use crate::domain::draft::RequestDraft;
use crate::services::repos::{GitRepo, LOCAL_ID};
use crate::services::storage;

/// Whether `collection_id` is synced with a remote repository.
pub(crate) fn is_shared(state: &AppState, collection_id: &str) -> bool {
    is_shared_in(&state.git_repos, collection_id)
}

fn is_shared_in(repos: &[GitRepo], collection_id: &str) -> bool {
    repos.iter().any(|r| {
        r.id != LOCAL_ID
            && r.remote_url.as_deref().is_some_and(|u| !u.trim().is_empty())
            && r.collection_ids.iter().any(|c| c == collection_id)
    })
}

fn find_base<'a>(
    requests: &'a HashMap<String, Vec<SavedRequest>>,
    request_id: &str,
) -> Option<&'a SavedRequest> {
    requests.values().flatten().find(|r| r.id == request_id)
}

/// What the user sees and sends: the draft if there is one, else `base`.
pub(crate) fn effective_request(state: &AppState, base: &SavedRequest) -> SavedRequest {
    state
        .drafts
        .get(&base.id)
        .map(|d| d.data.clone())
        .unwrap_or_else(|| base.clone())
}

/// Store `data` as the draft of its request. A draft that ends up equal to
/// its base (and has no open conflict) is deleted instead.
pub(crate) fn save_draft(state: &mut AppState, data: SavedRequest) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp_millis();
    let draft = match state.drafts.by_request.get(&data.id) {
        Some(existing) => {
            let mut d = existing.clone();
            d.set_data(data, now);
            d
        }
        None => {
            let base = find_base(&state.requests, &data.id)
                .ok_or_else(|| format!("request {} not found", data.id))?;
            RequestDraft::new(base, data, now)
        }
    };
    persist(state, draft)
}

fn persist(state: &mut AppState, draft: RequestDraft) -> Result<(), String> {
    if draft.is_collapsed() {
        if let Some(db) = &state.db {
            storage::drafts::delete_draft(db, &draft.request_id)?;
        }
        state.drafts.by_request.remove(&draft.request_id);
    } else {
        if let Some(db) = &state.db {
            storage::drafts::upsert_draft(db, &draft)?;
        }
        state.drafts.by_request.insert(draft.request_id.clone(), draft);
    }
    Ok(())
}

/// Save every modified tab of a shared request into its draft and clear the
/// tab's modified flag. Run **before** base rows are replaced, so edits made
/// while a pull was in flight are merged instead of overwritten. Stops at the
/// first failure; tabs not yet captured keep their edits and flag.
pub(crate) fn capture_tabs(state: &mut AppState) -> Result<(), String> {
    let pending: Vec<(usize, SavedRequest)> = state
        .tabs
        .tabs
        .iter()
        .enumerate()
        .filter(|(_, t)| t.modified)
        .filter_map(|(i, t)| {
            let (col, id) = t.saved_as.clone()?;
            is_shared(state, &col)
                .then(|| (i, crate::app::request_ops::saved_request_from_tab(t, col, id)))
        })
        .collect();
    for (idx, req) in pending {
        save_draft(state, req)?;
        state.tabs.tabs[idx].modified = false;
    }
    Ok(())
}

/// Merge every draft onto its (possibly new) base. Drafts whose base is gone
/// are kept and marked orphaned. Persistence errors are returned after all
/// drafts were processed; memory then holds the merged result.
pub(crate) fn rebase_all_after_sync(state: &mut AppState) -> Result<(), String> {
    let ids: Vec<String> = state.drafts.by_request.keys().cloned().collect();
    let mut first_err = None;
    for id in ids {
        let Some(draft) = state.drafts.by_request.get(&id).cloned() else {
            continue;
        };
        match find_base(&state.requests, &id).cloned() {
            Some(new_base) => {
                if let Err(e) = persist(state, draft.rebase(&new_base)) {
                    first_err.get_or_insert(e);
                }
            }
            None => {
                if let Some(d) = state.drafts.by_request.get_mut(&id) {
                    d.orphaned = true;
                }
            }
        }
    }
    first_err.map_or(Ok(()), Err)
}

/// Refresh open tabs of saved requests from their effective request, in
/// place. Tabs with uncaptured local edits (purely local collections) are
/// left alone rather than overwritten.
pub(crate) fn refresh_open_tabs(state: &mut AppState) {
    for i in 0..state.tabs.tabs.len() {
        let tab = &state.tabs.tabs[i];
        if tab.modified {
            continue;
        }
        let Some((_, req_id)) = tab.saved_as.clone() else {
            continue;
        };
        let Some(base) = find_base(&state.requests, &req_id) else {
            continue;
        };
        let effective = effective_request(state, base);
        let tab = &mut state.tabs.tabs[i];
        tab.apply_saved(&effective);
        tab.modified = false;
    }
}

/// End-to-end flows through the real handlers, on a throwaway data dir.
#[cfg(test)]
mod flow_tests {
    use super::*;
    use crate::domain::collection::Collection;
    use crate::domain::draft::DraftField;
    use crate::domain::group::ImportedCollection;
    use crate::message::{GitMsg, Message, SyncPayload};
    use crate::state::tabs::make_code_editor;

    const COL: &str = "col-shared";
    const REQ: &str = "req-login";
    const REPO: &str = "repo-1";

    struct Env {
        state: AppState,
        dir: std::path::PathBuf,
    }

    impl Drop for Env {
        fn drop(&mut self) {
            self.state.db = None; // close SQLite before deleting its file
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn base_req(url: &str, body: &str) -> SavedRequest {
        let mut r = SavedRequest::new_in(COL.into(), "Login".into());
        r.id = REQ.into();
        r.url = url.into();
        r.body = body.into();
        r
    }

    fn collection(req: SavedRequest) -> ImportedCollection {
        ImportedCollection {
            collection: Collection {
                id: COL.into(),
                name: "Shared".into(),
                created_at: 0,
            },
            requests: vec![req],
            groups: Vec::new(),
        }
    }

    /// A state with one request in a collection owned by a repo that has a
    /// remote, and that request open in the active tab.
    fn env(shared: bool) -> Env {
        let dir = std::env::temp_dir().join(format!("rustman-test-{}", uuid::Uuid::new_v4()));
        let (mut state, _) = crate::app::boot::init_at(dir.clone());
        crate::app::tree::insert_collection(
            &mut state,
            collection(base_req("https://a.test/v1", "base-body")),
        );
        state.git_repos.push(GitRepo {
            id: REPO.into(),
            name: "company".into(),
            path: dir.join("repo"),
            remote_url: shared.then(|| "https://github.com/o/r.git".to_owned()),
            collection_ids: vec![COL.into()],
        });
        let base = find_base(&state.requests, REQ).unwrap().clone();
        state.tabs.open_request(&base);
        Env { state, dir }
    }

    fn edit_body(state: &mut AppState, body: &str) {
        let tab = state.tabs.active_tab_mut();
        tab.body_editor = make_code_editor(body, "json");
        tab.modified = true;
    }

    fn sync(state: &mut AppState, upstream: Vec<ImportedCollection>) {
        let _ = crate::app::update::update(
            state,
            Message::Git(GitMsg::Synced(Box::new(SyncPayload {
                repo_id: REPO.into(),
                collections: upstream,
                label: "Synced".into(),
                global_script: None,
            }))),
        );
    }

    fn serialized_view(state: &AppState) -> String {
        let col = state.collections.iter().find(|c| c.id == COL).unwrap();
        serde_json::to_string(&crate::app::tree::collection_from_state(state, col).requests)
            .unwrap()
    }

    fn base_body(state: &AppState) -> String {
        find_base(&state.requests, REQ).unwrap().body.clone()
    }

    fn outgoing(state: &AppState) -> SavedRequest {
        let tab = state.tabs.active_tab();
        crate::app::request_ops::saved_request_from_tab(tab, COL.into(), REQ.into())
    }

    #[test]
    fn edits_to_a_shared_request_are_not_shared() {
        let mut e = env(true);
        let before = serialized_view(&e.state);
        edit_body(&mut e.state, "mine");
        crate::app::request_ops::flush_modified_tabs(&mut e.state);

        assert_eq!(serialized_view(&e.state), before, "repo view must not change");
        assert_eq!(base_body(&e.state), "base-body");
        assert_eq!(e.state.drafts.get(REQ).unwrap().data.body, "mine");
        assert!(!e.state.tabs.active_tab().modified);
    }

    #[test]
    fn ctrl_s_on_a_shared_request_writes_the_draft_not_the_base() {
        let mut e = env(true);
        edit_body(&mut e.state, "mine");
        let _ = crate::app::request_ops::save_request(&mut e.state);
        assert_eq!(base_body(&e.state), "base-body");
        assert_eq!(e.state.drafts.get(REQ).unwrap().data.body, "mine");
    }

    #[test]
    fn local_collections_keep_saving_to_base() {
        let mut e = env(false);
        edit_body(&mut e.state, "mine");
        crate::app::request_ops::flush_modified_tabs(&mut e.state);
        assert_eq!(base_body(&e.state), "mine");
        assert!(e.state.drafts.get(REQ).is_none());
    }

    #[test]
    fn sync_merges_upstream_url_with_unsaved_local_body() {
        let mut e = env(true);
        // Edited while the pull was in flight, never saved.
        edit_body(&mut e.state, "mine");
        sync(&mut e.state, vec![collection(base_req("https://a.test/v2", "base-body"))]);

        let tab = e.state.tabs.active_tab();
        assert_eq!(tab.url, "https://a.test/v2", "upstream URL reaches the open tab");
        assert_eq!(tab.body_editor.content(), "mine", "local body survives");
        let sent = outgoing(&e.state);
        assert_eq!((sent.url.as_str(), sent.body.as_str()), ("https://a.test/v2", "mine"));
        assert_eq!(base_body(&e.state), "base-body", "base never gets the local edit");

        // Ctrl+S after the refresh must not bring the old URL back.
        let _ = crate::app::request_ops::save_request(&mut e.state);
        let d = e.state.drafts.get(REQ).unwrap();
        assert_eq!(d.data.url, "https://a.test/v2");
        assert_eq!(d.data.body, "mine");
    }

    #[test]
    fn sync_keeps_response_and_tab_identity() {
        let mut e = env(true);
        let id_before = e.state.tabs.active_tab().id.clone();
        e.state.tabs.active_tab_mut().active_request_tab =
            crate::message::RequestTab::Body;
        edit_body(&mut e.state, "mine");
        sync(&mut e.state, vec![collection(base_req("https://a.test/v2", "base-body"))]);
        let tab = e.state.tabs.active_tab();
        assert_eq!(tab.id, id_before);
        assert_eq!(tab.active_request_tab, crate::message::RequestTab::Body);
    }

    #[test]
    fn conflict_is_durable_across_resync_and_reload() {
        let mut e = env(true);
        edit_body(&mut e.state, "mine");
        let theirs = || vec![collection(base_req("https://a.test/v1", "theirs"))];
        sync(&mut e.state, theirs());
        sync(&mut e.state, theirs());

        let d = e.state.drafts.get(REQ).unwrap();
        assert_eq!(d.conflicts, vec![DraftField::Body]);
        assert_eq!(d.data.body, "mine");
        assert_eq!(e.state.tabs.active_tab().body_editor.content(), "mine");

        let stored = storage::drafts::get_drafts(e.state.db.as_ref().unwrap()).unwrap();
        assert_eq!(stored[0].conflicts, vec![DraftField::Body]);
    }

    #[test]
    fn endpoint_removed_upstream_keeps_an_orphaned_draft() {
        let mut e = env(true);
        edit_body(&mut e.state, "mine");
        sync(&mut e.state, Vec::new()); // repo no longer has the collection

        let d = e.state.drafts.get(REQ).expect("draft kept");
        assert!(d.orphaned);
        assert_eq!(d.data.body, "mine");
        let stored = storage::drafts::get_drafts(e.state.db.as_ref().unwrap()).unwrap();
        assert_eq!(stored.len(), 1, "row survives for recovery");
    }

    #[test]
    fn failed_capture_applies_nothing() {
        let mut e = env(true);
        edit_body(&mut e.state, "mine");
        e.state
            .db
            .as_ref()
            .unwrap()
            .execute("DROP TABLE request_drafts", [])
            .unwrap();
        sync(&mut e.state, vec![collection(base_req("https://a.test/v2", "theirs"))]);

        let base = find_base(&e.state.requests, REQ).unwrap();
        assert_eq!(base.url, "https://a.test/v1", "incoming collection not applied");
        let tab = e.state.tabs.active_tab();
        assert!(tab.modified, "edit still pending in the tab");
        assert_eq!(tab.body_editor.content(), "mine");
    }

    #[test]
    fn reopening_a_request_shows_its_draft() {
        let mut e = env(true);
        edit_body(&mut e.state, "mine");
        crate::app::request_ops::flush_modified_tabs(&mut e.state);
        e.state.tabs.tabs.clear();
        e.state.tabs.tabs.push(crate::state::tabs::RequestTabState::new());
        e.state.tabs.active = 0;

        let base = find_base(&e.state.requests, REQ).unwrap().clone();
        let _ = crate::app::update::update(
            &mut e.state,
            Message::Sidebar(crate::message::SidebarMsg::RequestOpened(base)),
        );
        assert_eq!(e.state.tabs.active_tab().body_editor.content(), "mine");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(id: &str, remote: Option<&str>, cols: &[&str]) -> GitRepo {
        GitRepo {
            id: id.into(),
            name: id.into(),
            path: std::path::PathBuf::from("."),
            remote_url: remote.map(Into::into),
            collection_ids: cols.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    #[test]
    fn only_collections_of_a_repo_with_remote_are_shared() {
        let repos = vec![
            repo(LOCAL_ID, Some("https://x"), &["local-col"]),
            repo("r1", Some("https://github.com/o/r.git"), &["shared-col"]),
            repo("r2", None, &["no-remote-col"]),
            repo("r3", Some("  "), &["blank-remote-col"]),
        ];
        assert!(is_shared_in(&repos, "shared-col"));
        assert!(!is_shared_in(&repos, "local-col"));
        assert!(!is_shared_in(&repos, "no-remote-col"));
        assert!(!is_shared_in(&repos, "blank-remote-col"));
        assert!(!is_shared_in(&repos, "unknown"));
    }
}
