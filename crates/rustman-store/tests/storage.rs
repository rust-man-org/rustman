//! Write → read round trips through a real database file, one table at a time.

use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rustman_core::domain::collection::{Collection, SavedRequest};
use rustman_core::domain::group::{Group, ImportedCollection};
use rustman_core::domain::history::{HISTORY_BODY_LIMIT, OmittedContent, OmittedPart};
use rustman_core::domain::request::FormFieldType;
use rustman_store::history_entry as history_entry_by_id;
use rustman_store::*;
use rustman_testkit::{
    COLLECTION_ID, TempDir, as_written, environments, form_fields, full_saved_request,
    history_entry, request_variants, sample_collection, second_collection,
};
use serde::{Deserialize, Serialize};

/// A fresh database in its own temp dir. Bind as `let (_dir, conn)` so the
/// connection drops before the directory is removed.
fn db() -> (TempDir, Connection) {
    let dir = TempDir::new("storage");
    let conn = open(dir.path()).expect("open db");
    (dir, conn)
}

fn groups_of(ic: &ImportedCollection) -> Vec<Group> {
    ic.all_groups().into_iter().map(Group::from).collect()
}

fn store(conn: &Connection, ic: &ImportedCollection) {
    create_collection(conn, &ic.collection).unwrap();
    for g in groups_of(ic) {
        create_group(conn, &g).unwrap();
    }
    for r in ic.all_requests() {
        create_request(conn, r).unwrap();
    }
}

fn ids(reqs: &[SavedRequest]) -> Vec<&str> {
    reqs.iter().map(|r| r.id.as_str()).collect()
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

/// Stands in for the app's session: the store only sees serde.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Session {
    theme_idx: usize,
    tabs: Vec<String>,
}

fn full_session() -> Session {
    Session {
        theme_idx: 3,
        tabs: vec!["tab-full".into(), "tab-scratch".into()],
    }
}

#[test]
fn collections_round_trip_in_creation_order() {
    let (_dir, conn) = db();
    let newer = Collection {
        id: "b".into(),
        name: "Newer".into(),
        created_at: 20,
    };
    let older = Collection {
        id: "a".into(),
        name: "Older".into(),
        created_at: 10,
    };
    create_collection(&conn, &newer).unwrap();
    create_collection(&conn, &older).unwrap();

    let got = get_collections(&conn).unwrap();
    assert_eq!(as_written(&got), as_written(&vec![older, newer]));

    update_collection(&conn, "a", "Renamed").unwrap();
    assert_eq!(get_collections(&conn).unwrap()[0].name, "Renamed");
    assert!(create_collection(&conn, &got[0]).is_err(), "ids are unique");
}

#[test]
fn deleting_a_collection_removes_only_its_requests_and_groups() {
    let (_dir, conn) = db();
    store(&conn, &sample_collection());
    store(&conn, &second_collection());

    delete_collection(&conn, COLLECTION_ID).unwrap();

    let left: Vec<String> = get_collections(&conn)
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(left, ["col-auth"]);
    assert!(get_requests(&conn, COLLECTION_ID).unwrap().is_empty());
    assert!(
        get_groups(&conn).unwrap().is_empty(),
        "nested groups go too"
    );
    assert_eq!(get_requests(&conn, "col-auth").unwrap().len(), 1);
}

#[test]
fn deleting_a_group_removes_itself_subgroups_and_their_requests() {
    let (_dir, conn) = db();
    let g1 = Group::new("col-1".into(), true, "Backend".into(), 0);
    let g2 = Group::new(g1.id.clone(), false, "Resource".into(), 0);
    create_group(&conn, &g1).unwrap();
    create_group(&conn, &g2).unwrap();
    let in_g1 = SavedRequest::new_in_group("col-1".into(), Some(g1.id.clone()), "a".into(), 0);
    let in_g2 = SavedRequest::new_in_group("col-1".into(), Some(g2.id.clone()), "b".into(), 0);
    let top = SavedRequest::new_in_group("col-1".into(), None, "c".into(), 0);
    for r in [&in_g1, &in_g2, &top] {
        create_request(&conn, r).unwrap();
    }

    delete_group(&conn, &g1.id).unwrap();

    assert!(
        get_groups(&conn).unwrap().is_empty(),
        "group and subgroup must be gone"
    );
    assert_eq!(
        ids(&get_requests(&conn, "col-1").unwrap()),
        [top.id.as_str()]
    );
}

#[test]
fn group_order_round_trips() {
    let (_dir, conn) = db();
    let a = Group::new("col-1".into(), true, "a".into(), 2);
    let b = Group::new("col-1".into(), true, "b".into(), 0);
    create_group(&conn, &a).unwrap();
    create_group(&conn, &b).unwrap();
    update_group_placement(&conn, &a.id, "col-1", true, 5).unwrap();

    let groups = get_groups(&conn).unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups.iter().find(|g| g.id == a.id).unwrap().sort_order, 5);
    assert_eq!(groups.iter().find(|g| g.id == b.id).unwrap().sort_order, 0);
}

#[test]
fn group_placement_moves_a_folder_under_another_parent() {
    let (_dir, conn) = db();
    let a = Group::new("col-1".into(), true, "a".into(), 0);
    let b = Group::new("col-1".into(), true, "b".into(), 1);
    create_group(&conn, &a).unwrap();
    create_group(&conn, &b).unwrap();
    let find_b = |conn: &Connection| {
        get_groups(conn)
            .unwrap()
            .into_iter()
            .find(|g| g.id == b.id)
            .unwrap()
    };

    update_group_placement(&conn, &b.id, &a.id, false, 0).unwrap();
    let moved = find_b(&conn);
    assert_eq!(moved.parent_id, a.id);
    assert!(!moved.parent_is_collection);

    update_group_placement(&conn, &b.id, "col-2", true, 3).unwrap();
    let moved = find_b(&conn);
    assert_eq!(moved.parent_id, "col-2");
    assert!(moved.parent_is_collection);
    assert_eq!(moved.sort_order, 3);
}

#[test]
fn requests_round_trip_every_field() {
    let (_dir, conn) = db();
    let mut originals = request_variants();
    originals.push(full_saved_request());
    for r in &originals {
        create_request(&conn, r).unwrap();
    }

    let loaded = get_requests(&conn, COLLECTION_ID).unwrap();
    assert_eq!(loaded.len(), originals.len());
    for original in &originals {
        let back = loaded.iter().find(|r| r.id == original.id).unwrap();
        assert_eq!(as_written(back), as_written(original), "{}", original.id);
    }
    assert!(
        get_requests(&conn, "some-other-collection")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn requests_load_in_sort_order_then_insertion_order() {
    let (_dir, conn) = db();
    for (id, order) in [
        ("late", 5),
        ("first-tie", 1),
        ("second-tie", 1),
        ("early", 0),
    ] {
        let mut r = SavedRequest::new_in(COLLECTION_ID.into(), id.into());
        r.id = id.into();
        r.sort_order = order;
        create_request(&conn, &r).unwrap();
    }
    let loaded = get_requests(&conn, COLLECTION_ID).unwrap();
    assert_eq!(ids(&loaded), ["early", "first-tie", "second-tie", "late"]);
}

#[test]
fn saving_a_request_again_replaces_it_and_delete_removes_one() {
    let (_dir, conn) = db();
    let mut r = full_saved_request();
    create_request(&conn, &r).unwrap();
    r.name = "Renamed".into();
    r.group_id = None;
    r.sort_order = 2;
    create_request(&conn, &r).unwrap();
    let other = request_variants().remove(0);
    create_request(&conn, &other).unwrap();

    let loaded = get_requests(&conn, COLLECTION_ID).unwrap();
    assert_eq!(loaded.len(), 2);
    let back = loaded.iter().find(|x| x.id == r.id).unwrap();
    assert_eq!(as_written(back), as_written(&r));

    delete_request(&conn, &r.id).unwrap();
    assert_eq!(
        ids(&get_requests(&conn, COLLECTION_ID).unwrap()),
        [other.id.as_str()]
    );

    delete_requests_for_collection(&conn, COLLECTION_ID).unwrap();
    assert!(get_requests(&conn, COLLECTION_ID).unwrap().is_empty());
}

#[test]
fn one_corrupt_request_does_not_hide_the_rest() {
    let (_dir, conn) = db();
    let good = SavedRequest::new_in("c".into(), "good".into());
    create_request(&conn, &good).unwrap();
    conn.execute(
        "INSERT INTO requests (id, collection_id, data, group_id, sort_order) \
         VALUES ('bad', 'c', '{not json', NULL, 1)",
        [],
    )
    .unwrap();

    let loaded = get_requests(&conn, "c").unwrap();
    assert_eq!(ids(&loaded), [good.id.as_str()]);
    assert_eq!(
        count(&conn, "requests"),
        2,
        "the unreadable row stays in the table"
    );
}

#[test]
fn groups_round_trip() {
    let (_dir, conn) = db();
    let groups = groups_of(&sample_collection());
    for g in &groups {
        create_group(&conn, g).unwrap();
    }
    let loaded = get_groups(&conn).unwrap();
    let order: Vec<&str> = loaded.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(order, ["grp-catalog", "grp-products", "grp-empty"]);
    for g in &groups {
        let back = loaded.iter().find(|x| x.id == g.id).unwrap();
        assert_eq!(as_written(back), as_written(g));
    }

    update_group(&conn, "grp-empty", "Archive").unwrap();
    let renamed = get_groups(&conn)
        .unwrap()
        .into_iter()
        .find(|g| g.id == "grp-empty")
        .unwrap();
    assert_eq!(renamed.name, "Archive");
}

#[test]
fn environments_round_trip_with_one_active() {
    let (_dir, conn) = db();
    for env in environments() {
        save_environment(&conn, &env).unwrap();
    }
    let loaded = get_environments(&conn).unwrap();
    let names: Vec<&str> = loaded.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Development", "Production"], "sorted by name");
    for (back, original) in loaded.iter().zip(environments()) {
        assert_eq!(back.id, original.id);
        assert_eq!(back.variables, original.variables);
        assert_eq!(back.is_active, original.is_active);
    }

    let mut dev = loaded[0].clone();
    dev.is_active = true;
    save_environment(&conn, &dev).unwrap();
    let active: Vec<String> = get_environments(&conn)
        .unwrap()
        .into_iter()
        .filter(|e| e.is_active)
        .map(|e| e.id)
        .collect();
    assert_eq!(active, ["env-dev"], "activating one deactivates the rest");

    delete_environment(&conn, "env-prod").unwrap();
    assert_eq!(get_environments(&conn).unwrap().len(), 1);
}

#[test]
fn an_environment_with_unreadable_variables_loads_empty() {
    let (_dir, conn) = db();
    conn.execute(
        "INSERT INTO environments (id, name, variables, is_active) VALUES ('e', 'E', '{oops', 1)",
        [],
    )
    .unwrap();
    let loaded = get_environments(&conn).unwrap();
    assert_eq!(loaded.len(), 1);
    assert!(loaded[0].variables.is_empty());
    assert!(loaded[0].is_active);
}

#[test]
fn activating_an_environment_deactivates_the_rest_in_one_write() {
    let (_dir, conn) = db();
    for env in environments() {
        save_environment(&conn, &env).unwrap();
    }
    let active = |conn: &Connection| -> Vec<String> {
        get_environments(conn)
            .unwrap()
            .into_iter()
            .filter(|e| e.is_active)
            .map(|e| e.id)
            .collect()
    };

    set_active_environment(&conn, Some("env-dev")).unwrap();
    assert_eq!(active(&conn), ["env-dev"]);
    set_active_environment(&conn, None).unwrap();
    assert!(active(&conn).is_empty());
}

#[test]
fn resaving_an_unreadable_environment_keeps_its_variables_text() {
    let (_dir, conn) = db();
    conn.execute(
        "INSERT INTO environments (id, name, variables, is_active) VALUES ('e', 'E', '{oops', 0)",
        [],
    )
    .unwrap();
    let mut env = get_environments(&conn).unwrap().remove(0);
    env.name = "Renamed".into();
    save_environment(&conn, &env).unwrap();

    let stored: String = conn
        .query_row(
            "SELECT variables FROM environments WHERE id = 'e'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, "{oops");
    assert_eq!(get_environments(&conn).unwrap()[0].name, "Renamed");

    env.variables.insert("fixed".into(), "1".into());
    save_environment(&conn, &env).unwrap();
    assert_eq!(get_environments(&conn).unwrap()[0].variables["fixed"], "1");
}

#[test]
fn history_round_trips_newest_first() {
    let (_dir, conn) = db();
    let older = history_entry();
    let mut newer = history_entry();
    newer.id = "hist-2".into();
    newer.timestamp += 1_000;
    newer.status = 404;
    newer.request = full_saved_request();
    add_history(&conn, &older).unwrap();
    add_history(&conn, &newer).unwrap();

    let loaded = get_history(&conn).unwrap();
    assert_eq!(as_written(&loaded), as_written(&vec![newer, older]));

    clear_history(&conn).unwrap();
    assert!(get_history(&conn).unwrap().is_empty());
}

#[test]
fn history_keeps_only_the_newest_100() {
    let (_dir, conn) = db();
    for i in 0..105 {
        let mut e = history_entry();
        e.id = format!("hist-{i:03}");
        e.timestamp = i;
        add_history(&conn, &e).unwrap();
    }
    let loaded = get_history(&conn).unwrap();
    assert_eq!(loaded.len(), 100);
    assert_eq!(loaded[0].id, "hist-104");
    assert_eq!(loaded[99].id, "hist-005");
    assert_eq!(
        count(&conn, "history"),
        100,
        "older rows are deleted, not just hidden"
    );
}

#[test]
fn one_unreadable_history_row_does_not_hide_the_rest() {
    let (_dir, conn) = db();
    add_history(&conn, history_entry()).unwrap();
    conn.execute(
        "INSERT INTO history (id, timestamp, method, url, status, duration, request_json) \
         VALUES ('bad', 1, 'GET', 'x', 0, 0, '{not json')",
        [],
    )
    .unwrap();
    let loaded = get_history(&conn).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "hist-1");
}

#[test]
fn summaries_list_history_without_reading_any_request() {
    let (_dir, conn) = db();
    let small = history_entry();
    let mut huge = history_entry();
    huge.id = "hist-huge".into();
    huge.timestamp += 1_000;
    huge.request.body = "x".repeat(5_000_000);
    add_history(&conn, &small).unwrap();
    add_history(&conn, &huge).unwrap();
    conn.authorizer(Some(|ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Read {
            table_name: "history",
            column_name: "request_json",
        } => Authorization::Deny,
        _ => Authorization::Allow,
    }))
    .unwrap();

    let summaries = history_summaries(&conn, 100).unwrap();

    assert!(get_history(&conn).is_err(), "reading requests is denied");
    assert_eq!(summaries, [huge.summary(), small.summary()]);
    assert_eq!(history_summaries(&conn, 1).unwrap(), [huge.summary()]);
}

#[test]
fn one_history_entry_loads_whole_and_an_unreadable_one_is_none() {
    let (_dir, conn) = db();
    let mut entry = history_entry();
    entry.request = full_saved_request();
    add_history(&conn, &entry).unwrap();
    conn.execute(
        "INSERT INTO history (id, timestamp, method, url, status, duration, request_json) \
         VALUES ('bad', 1, 'GET', 'x', 0, 0, '{not json')",
        [],
    )
    .unwrap();

    let loaded = history_entry_by_id(&conn, &entry.id).unwrap();
    assert_eq!(as_written(&loaded), as_written(&Some(entry)));
    assert!(history_entry_by_id(&conn, "bad").unwrap().is_none());
    assert!(history_entry_by_id(&conn, "missing").unwrap().is_none());
    assert_eq!(
        history_summaries(&conn, 100).unwrap().len(),
        2,
        "summaries still list it"
    );
}

fn stored_request_json(conn: &Connection, id: &str) -> String {
    conn.query_row(
        "SELECT request_json FROM history WHERE id = ?1",
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn history_keeps_a_body_of_exactly_the_limit_and_omits_one_byte_more() {
    let (_dir, conn) = db();
    let mut kept = history_entry();
    kept.request.body = "x".repeat(HISTORY_BODY_LIMIT);
    let mut cut = history_entry();
    cut.id = "hist-cut".into();
    cut.request.body = "x".repeat(HISTORY_BODY_LIMIT + 1);
    add_history(&conn, &kept).unwrap();
    add_history(&conn, &cut).unwrap();

    let kept_back = history_entry_by_id(&conn, &kept.id).unwrap().unwrap();
    assert_eq!(kept_back.request.body.len(), HISTORY_BODY_LIMIT);
    assert!(kept_back.omitted.is_empty());
    assert_eq!(
        stored_request_json(&conn, &kept.id),
        serde_json::to_string(&kept.request).unwrap(),
        "an entry that lost nothing is stored as before"
    );

    let cut_back = history_entry_by_id(&conn, "hist-cut").unwrap().unwrap();
    assert!(cut_back.request.body.is_empty());
    assert_eq!(
        cut_back.omitted,
        [OmittedPart {
            what: OmittedContent::Body,
            bytes: (HISTORY_BODY_LIMIT + 1) as u64
        }]
    );
    assert_eq!(
        cut_back.request.url, cut.request.url,
        "the rest of the request is kept"
    );
}

#[test]
fn history_omits_each_form_file_over_the_limit_on_its_own() {
    let (_dir, conn) = db();
    let mut entry = history_entry();
    // Whole base64 groups, so the decoded size is exact.
    let file = |key: &str, decoded_bytes: usize| {
        assert_eq!(decoded_bytes % 3, 0);
        let mut field = form_fields().remove(0);
        field.key = key.into();
        field.field_type = FormFieldType::File;
        field.file_data = Some("A".repeat(decoded_bytes / 3 * 4));
        field
    };
    entry.request.form_data_fields = vec![file("big", HISTORY_BODY_LIMIT + 2), file("small", 3)];
    add_history(&conn, &entry).unwrap();

    let back = history_entry_by_id(&conn, &entry.id).unwrap().unwrap();
    let data: Vec<_> = back
        .request
        .form_data_fields
        .iter()
        .map(|f| f.file_data.as_deref())
        .collect();
    assert_eq!(data, [None, Some("AAAA")]);
    assert_eq!(
        back.omitted,
        [OmittedPart {
            what: OmittedContent::FormFile("big".into()),
            bytes: (HISTORY_BODY_LIMIT + 2) as u64
        }]
    );
    assert_eq!(get_history(&conn).unwrap()[0].omitted, back.omitted);
}

#[test]
fn history_is_indexed_newest_first() {
    let (_dir, conn) = db();
    let plan: String = conn
        .query_row(
            "EXPLAIN QUERY PLAN SELECT id FROM history ORDER BY timestamp DESC LIMIT 100",
            [],
            |r| r.get(3),
        )
        .unwrap();
    assert!(plan.contains("history_by_time"), "{plan}");
}

#[test]
fn session_round_trips_and_is_overwritten_in_place() {
    let (_dir, conn) = db();
    assert!(
        load_session::<Session>(&conn).unwrap().is_none(),
        "nothing saved yet"
    );

    let first = Session {
        theme_idx: 1,
        ..Session::default()
    };
    save_session(&conn, &first).unwrap();
    save_session(&conn, &full_session()).unwrap();

    let loaded: Session = load_session(&conn).unwrap().expect("a saved session");
    assert_eq!(loaded, full_session());
    assert_eq!(count(&conn, "session"), 1);
}

#[test]
fn an_unreadable_session_is_an_error_not_an_empty_session() {
    let (_dir, conn) = db();
    conn.execute(
        "INSERT INTO session (key, data, saved_at) VALUES ('current', '{not json', 0)",
        [],
    )
    .unwrap();
    assert!(load_session::<Session>(&conn).is_err());
}

#[test]
fn a_session_backup_survives_the_next_save() {
    let (_dir, conn) = db();
    assert_eq!(back_up_session(&conn).unwrap(), None, "nothing to back up");
    conn.execute(
        "INSERT INTO session (key, data, saved_at) VALUES ('current', '{not json', 0)",
        [],
    )
    .unwrap();

    let key = back_up_session(&conn).unwrap().expect("a backup key");
    save_session(&conn, &full_session()).unwrap();

    let kept: String = conn
        .query_row("SELECT data FROM session WHERE key = ?1", [&key], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(kept, "{not json");
    assert!(key.starts_with("session_unreadable_"), "{key}");
    assert_eq!(
        load_session::<Session>(&conn).unwrap(),
        Some(full_session())
    );
}

#[test]
fn everything_survives_reopening_the_database() {
    let dir = TempDir::new("storage-reopen");
    let sample = sample_collection();
    {
        let conn = open(dir.path()).unwrap();
        store(&conn, &sample);
        for env in environments() {
            save_environment(&conn, &env).unwrap();
        }
        add_history(&conn, history_entry()).unwrap();
        save_session(&conn, &full_session()).unwrap();
    }

    let conn = open(dir.path()).unwrap();
    assert_eq!(
        as_written(&get_collections(&conn).unwrap()),
        as_written(&vec![sample.collection.clone()])
    );
    assert_eq!(get_groups(&conn).unwrap().len(), 3);
    assert_eq!(
        get_requests(&conn, COLLECTION_ID).unwrap().len(),
        sample.all_requests().len()
    );
    assert_eq!(get_environments(&conn).unwrap().len(), 2);
    assert_eq!(get_history(&conn).unwrap().len(), 1);
    assert_eq!(
        load_session::<Session>(&conn).unwrap(),
        Some(full_session())
    );
}

#[test]
fn opening_fails_when_the_data_dir_cannot_be_created() {
    let dir = TempDir::new("storage-blocked");
    let blocked = dir.path().join("file");
    std::fs::write(&blocked, "").unwrap();
    let err = open(&blocked.join("data")).unwrap_err();
    assert!(err.to_string().contains("cannot create"), "{err}");
}
