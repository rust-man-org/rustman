//! The storage ports over SQLite, and the thread that runs them.

use rustman_core::domain::group::{Group, ImportedCollection};
use rustman_core::store::{Change, HistoryStore, SessionStore, Workspace, WorkspaceStore};
use rustman_store::{SqliteStore, StoreThread};
use rustman_testkit::{TempDir, as_written, environments, history_entry, sample_collection};
use serde::{Deserialize, Serialize};

fn store() -> SqliteStore {
    SqliteStore::open_in_memory().unwrap()
}

/// The changes that add `ic`, parents before children.
fn added(ic: &ImportedCollection) -> Vec<Change> {
    let mut changes = vec![Change::PutCollection(ic.collection.clone())];
    changes.extend(
        ic.all_groups()
            .into_iter()
            .map(|g| Change::AddGroup(Group::from(g))),
    );
    changes.extend(
        ic.all_requests()
            .into_iter()
            .map(|r| Change::PutRequest(Box::new(r.clone()))),
    );
    changes
}

fn ids<T>(items: &[T], id: impl Fn(&T) -> &str) -> Vec<&str> {
    items.iter().map(id).collect()
}

fn group_ids(ws: &Workspace) -> Vec<&str> {
    ids(&ws.groups, |g| g.id.as_str())
}

fn request_ids(ws: &Workspace) -> Vec<&str> {
    ids(&ws.requests, |r| r.id.as_str())
}

#[test]
fn a_workspace_written_as_changes_loads_back() {
    let mut store = store();
    let sample = sample_collection();
    let mut changes = added(&sample);
    changes.extend(environments().into_iter().map(Change::PutEnvironment));
    store.apply(&changes).unwrap();

    let ws = store.load_workspace().unwrap();

    assert_eq!(
        as_written(&ws.collections),
        as_written(&vec![sample.collection.clone()])
    );
    let mut requests: Vec<_> = sample.all_requests().into_iter().cloned().collect();
    requests.sort_by_key(|r| r.sort_order);
    assert_eq!(request_ids(&ws), ids(&requests, |r| r.id.as_str()));
    assert_eq!(ws.groups.len(), sample.all_groups().len());
    assert_eq!(ws.environments.len(), environments().len());
}

#[test]
fn every_kind_of_change_applies() {
    let mut store = store();
    let sample = sample_collection();
    store.apply(&added(&sample)).unwrap();
    let ws = store.load_workspace().unwrap();
    let (group, request) = (ws.groups[0].clone(), ws.requests[0].clone());
    let mut env = environments().remove(0);
    env.is_active = false;

    store
        .apply(&[
            Change::RenameCollection {
                id: sample.collection.id.clone(),
                name: "Renamed".into(),
            },
            Change::RenameGroup {
                id: group.id.clone(),
                name: "Folder".into(),
            },
            Change::PlaceGroup {
                id: group.id.clone(),
                parent_id: sample.collection.id.clone(),
                parent_is_collection: true,
                sort_order: 9,
            },
            Change::DeleteRequest {
                id: request.id.clone(),
            },
            Change::PutEnvironment(env.clone()),
            Change::ActivateEnvironment {
                id: Some(env.id.clone()),
            },
        ])
        .unwrap();

    let ws = store.load_workspace().unwrap();
    assert_eq!(ws.collections[0].name, "Renamed");
    let moved = ws.groups.iter().find(|g| g.id == group.id).unwrap();
    assert_eq!((moved.name.as_str(), moved.sort_order), ("Folder", 9));
    assert!(!request_ids(&ws).contains(&request.id.as_str()));
    assert!(
        ws.environments
            .iter()
            .find(|e| e.id == env.id)
            .unwrap()
            .is_active
    );

    store
        .apply(&[
            Change::DeleteGroup {
                id: group.id.clone(),
            },
            Change::DeleteEnvironment { id: env.id.clone() },
        ])
        .unwrap();
    let ws = store.load_workspace().unwrap();
    assert!(!group_ids(&ws).contains(&group.id.as_str()));
    assert!(ws.environments.iter().all(|e| e.id != env.id));

    store
        .apply(&[Change::DeleteCollection {
            id: sample.collection.id.clone(),
        }])
        .unwrap();
    let ws = store.load_workspace().unwrap();
    assert!(ws.collections.is_empty() && ws.groups.is_empty() && ws.requests.is_empty());
}

#[test]
fn a_failing_change_rolls_back_the_whole_batch() {
    let mut store = store();
    let sample = sample_collection();
    store
        .apply(&[Change::PutCollection(sample.collection.clone())])
        .unwrap();
    let group = Group::from(sample.all_groups()[0]);

    let err = store
        .apply(&[Change::AddGroup(group.clone()), Change::AddGroup(group)])
        .unwrap_err();

    assert!(err.to_string().contains("UNIQUE"), "{err}");
    let ws = store.load_workspace().unwrap();
    assert!(
        ws.groups.is_empty(),
        "the group added before the failure is gone"
    );
    assert_eq!(ws.collections.len(), 1);
    assert!(
        store.connection().is_autocommit(),
        "no transaction is left open"
    );
}

#[test]
fn history_through_the_port_is_newest_first_and_limited() {
    let mut store = store();
    for i in 0..3 {
        let mut entry = history_entry();
        entry.id = format!("hist-{i}");
        entry.timestamp = i;
        store.append_history(entry).unwrap();
    }
    let recent = store.recent_history(2).unwrap();
    assert_eq!(ids(&recent, |e| e.id.as_str()), ["hist-2", "hist-1"]);
    store.clear_history().unwrap();
    assert!(store.recent_history(10).unwrap().is_empty());
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Session {
    tabs: Vec<String>,
}

#[test]
fn the_session_through_the_port_round_trips() {
    let mut store = store();
    assert_eq!(store.load_session::<Session>().unwrap(), None);
    let session = Session {
        tabs: vec!["a".into()],
    };
    store.save_session(&session).unwrap();
    assert_eq!(store.load_session::<Session>().unwrap(), Some(session));
}

#[test]
fn a_store_on_disk_keeps_what_was_written() {
    let dir = TempDir::new("workspace-port");
    let sample = sample_collection();
    SqliteStore::open(dir.path())
        .unwrap()
        .apply(&added(&sample))
        .unwrap();
    let ws = SqliteStore::open(dir.path())
        .unwrap()
        .load_workspace()
        .unwrap();
    assert_eq!(ws.collections.len(), 1);
    assert_eq!(ws.requests.len(), sample.all_requests().len());
}

#[tokio::test]
async fn the_thread_runs_jobs_in_order_and_answers_each() {
    let thread = StoreThread::spawn(store()).unwrap();
    let sample = sample_collection();
    let changes = added(&sample);

    let write = thread.run(move |store| store.apply(&changes));
    let read = thread.run(|store| store.load_workspace());

    write.await.unwrap();
    assert_eq!(
        read.await.unwrap().collections.len(),
        1,
        "the read ran after the write"
    );
}

#[tokio::test]
async fn a_dropped_job_still_runs() {
    let thread = StoreThread::spawn(store()).unwrap();
    drop(thread.run(|store| store.append_history(history_entry())));
    let recent = thread.run(|store| store.recent_history(10)).await.unwrap();
    assert_eq!(recent.len(), 1);
}

#[tokio::test]
async fn after_a_job_panics_every_job_reports_the_thread_stopped() {
    let thread = StoreThread::spawn(store()).unwrap();
    let crashed = thread.run(|_| -> Result<(), _> { panic!("job bug") });
    assert!(crashed.await.is_err());
    let err = thread
        .run(|store| store.recent_history(1))
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "the storage thread has stopped");
}

#[test]
fn putting_a_collection_again_renames_it_and_its_requests_can_be_cleared() {
    let mut store = store();
    let sample = sample_collection();
    store.apply(&added(&sample)).unwrap();
    let mut renamed = sample.collection.clone();
    renamed.name = "Renamed".into();

    store
        .apply(&[
            Change::PutCollection(renamed),
            Change::DeleteRequestsIn {
                collection_id: sample.collection.id.clone(),
            },
        ])
        .unwrap();

    let ws = store.load_workspace().unwrap();
    assert_eq!(ws.collections.len(), 1);
    assert_eq!(ws.collections[0].name, "Renamed");
    assert_eq!(ws.collections[0].created_at, sample.collection.created_at);
    assert!(ws.requests.is_empty());
    assert!(
        !ws.groups.is_empty(),
        "groups are another change's business"
    );
}

#[test]
fn a_blocking_run_waits_for_the_answer() {
    let thread = StoreThread::spawn(store()).unwrap();
    thread
        .run_blocking(|store| store.append_history(history_entry()))
        .unwrap();
    let recent = thread
        .run_blocking(|store| store.recent_history(10))
        .unwrap();
    assert_eq!(recent.len(), 1);
}

#[test]
fn renaming_a_request_changes_only_its_name() {
    let mut store = store();
    let sample = sample_collection();
    store.apply(&added(&sample)).unwrap();
    let mut request = sample.all_requests()[0].clone();
    request.body = "x".repeat(5_000_000);
    store
        .apply(&[Change::PutRequest(Box::new(request.clone()))])
        .unwrap();

    let start = std::time::Instant::now();
    store
        .apply(&[Change::RenameRequest {
            id: request.id.clone(),
            name: "Renamed \"quoted\"".into(),
        }])
        .unwrap();
    let renaming = start.elapsed();

    let ws = store.load_workspace().unwrap();
    let renamed = ws.requests.iter().find(|r| r.id == request.id).unwrap();
    request.name = "Renamed \"quoted\"".into();
    assert_eq!(as_written(renamed), as_written(&request));
    eprintln!("renaming a request with a 5 MB body: {renaming:?}");
}
