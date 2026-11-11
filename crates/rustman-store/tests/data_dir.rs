//! One writer per data directory, read-only on a newer schema, and files
//! private to their owner.

use rustman_core::domain::collection::Collection;
use rustman_store::{
    ReadOnly, create_collection, get_collections, open, open_app_data, save_session,
    session_array_len,
};
use rustman_testkit::TempDir;

fn collection(id: &str) -> Collection {
    Collection {
        id: id.into(),
        name: id.into(),
        created_at: 0,
    }
}

#[test]
fn a_second_process_gets_the_data_read_only_until_the_first_lets_go() {
    let dir = TempDir::new("data-dir-in-use");
    let first = open_app_data(dir.path()).unwrap();
    assert_eq!(first.read_only, None);
    create_collection(&first.conn, &collection("kept")).unwrap();

    let second = open_app_data(dir.path()).unwrap();
    assert_eq!(second.read_only, Some(ReadOnly::InUse));
    assert_eq!(
        get_collections(&second.conn).unwrap().len(),
        1,
        "it can still read"
    );
    let err = create_collection(&second.conn, &collection("lost")).unwrap_err();
    assert_eq!(err, "Not saved: Rustman is read-only in this window");

    drop((first, second));
    let third = open_app_data(dir.path()).unwrap();
    assert_eq!(third.read_only, None);
}

#[test]
fn a_database_from_a_newer_build_is_opened_read_only() {
    let dir = TempDir::new("data-dir-newer");
    let conn = open(dir.path()).unwrap();
    create_collection(&conn, &collection("theirs")).unwrap();
    conn.pragma_update(None, "user_version", 999).unwrap();
    drop(conn);

    let data = open_app_data(dir.path()).unwrap();

    assert_eq!(data.read_only, Some(ReadOnly::NewerSchema));
    assert_eq!(get_collections(&data.conn).unwrap()[0].id, "theirs");
    assert!(create_collection(&data.conn, &collection("ours")).is_err());
    assert!(
        save_session(&data.conn, "{}").is_err(),
        "the session isn't overwritten"
    );
    let version: i64 = data
        .conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 999);
}

#[cfg(unix)]
#[test]
fn the_data_is_readable_by_its_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new("data-dir-modes");
    let data_dir = dir.path().join("rustman");
    let data = open_app_data(&data_dir).unwrap();
    create_collection(&data.conn, &collection("c")).unwrap();

    let mode = |name: &str| {
        std::fs::metadata(data_dir.join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode(""), 0o700);
    for file in ["rustman.db", "rustman.db-wal", "rustman.lock"] {
        assert_eq!(mode(file), 0o600, "{file}");
    }
}

#[test]
fn the_saved_session_s_arrays_can_be_counted_without_decoding() {
    let dir = TempDir::new("data-dir-session-len");
    let conn = open(dir.path()).unwrap();
    assert_eq!(session_array_len(&conn, "$.tabs").unwrap(), None);
    save_session(
        &conn,
        &serde_json::json!({ "tabs": [1, {"x": 2}, "three"] }),
    )
    .unwrap();
    assert_eq!(session_array_len(&conn, "$.tabs").unwrap(), Some(3));
    assert_eq!(session_array_len(&conn, "$.missing").unwrap(), None);
}
