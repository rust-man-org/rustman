//! Sample workspace for trying the UI (`scripts/ui.sh -x`): one local
//! collection with nested folders, requests for every method against the
//! public JSONPlaceholder API (so Send works), and a matching environment.
//!
//! Only runs with `RUSTMAN_SAMPLE=1`, and only into a data folder that has
//! no collections yet, so it can never mix into real data.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::{
    domain::{
        collection::{Collection, SavedRequest},
        environment::AppEnvironment,
        group::Group,
        request::{BodyType, HttpMethod, KeyValue},
    },
    services::storage,
};

pub(crate) const COLLECTION_NAME: &str = "Sample API";
const BASE_URL: &str = "https://jsonplaceholder.typicode.com";

pub(crate) fn wanted() -> bool {
    std::env::var_os("RUSTMAN_SAMPLE").is_some_and(|v| v == "1")
}

/// Seed the sample into `conn` when it holds no collections. Returns
/// whether anything was written.
pub(crate) fn seed_if_empty(conn: &Connection) -> Result<bool, String> {
    if !storage::get_collections(conn)?.is_empty() {
        return Ok(false);
    }
    let col = Collection {
        id: uuid::Uuid::new_v4().to_string(),
        name: COLLECTION_NAME.to_owned(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    storage::create_collection(conn, &col)?;

    // Sample API ─ users ─ …
    //            ├ posts ─ comments ─ …
    //            └ Health check
    let users = Group::new(col.id.clone(), true, "users".into(), 0);
    let posts = Group::new(col.id.clone(), true, "posts".into(), 1);
    let comments = Group::new(posts.id.clone(), false, "comments".into(), 0);
    for g in [&users, &posts, &comments] {
        storage::create_group(conn, g)?;
    }

    let post_json = r#"{
  "title": "Hello from Rustman",
  "body": "Sample request body",
  "userId": 1
}"#;
    let patch_json = r#"{ "title": "Patched title" }"#;
    let requests: [(Option<&Group>, &str, HttpMethod, &str, Option<&str>); 9] = [
        (Some(&users), "List users", HttpMethod::Get, "/users", None),
        (Some(&users), "Get user", HttpMethod::Get, "/users/1", None),
        (Some(&posts), "List posts", HttpMethod::Get, "/posts?userId=1", None),
        (Some(&posts), "Create post", HttpMethod::Post, "/posts", Some(post_json)),
        (Some(&posts), "Update post", HttpMethod::Put, "/posts/1", Some(post_json)),
        (Some(&posts), "Patch post title", HttpMethod::Patch, "/posts/1", Some(patch_json)),
        (Some(&posts), "Delete post", HttpMethod::Delete, "/posts/1", None),
        (Some(&comments), "Comments of post", HttpMethod::Get, "/posts/1/comments", None),
        (None, "Health check", HttpMethod::Get, "/todos/1", None),
    ];
    for (i, (group, name, method, path, body)) in requests.into_iter().enumerate() {
        let mut r = SavedRequest::new_in_group(
            col.id.clone(),
            group.map(|g| g.id.clone()),
            name.to_owned(),
            i as i64,
        );
        r.method = method;
        r.url = format!("{{{{base_url}}}}{path}");
        if let Some(body) = body {
            r.body = body.to_owned();
            r.body_type = BodyType::Json;
            r.headers = vec![KeyValue {
                key: "Content-Type".into(),
                value: "application/json".into(),
                ..KeyValue::new_empty()
            }];
        }
        storage::create_request(conn, &r)?;
    }

    storage::save_environment(
        conn,
        &AppEnvironment {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Sample".into(),
            variables: HashMap::from([("base_url".to_owned(), BASE_URL.to_owned())]),
            is_active: true,
        },
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (std::path::PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!("rustman-sample-{}", uuid::Uuid::new_v4()));
        let conn = storage::open(&dir).expect("open db");
        (dir, conn)
    }

    #[test]
    fn seeds_tree_and_environment_once() {
        let (dir, conn) = temp_db();
        assert!(seed_if_empty(&conn).unwrap());

        let cols = storage::get_collections(&conn).unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].name, COLLECTION_NAME);
        let groups = storage::get_groups(&conn).unwrap();
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().any(|g| g.name == "comments" && !g.parent_is_collection));
        let reqs = storage::get_requests(&conn, &cols[0].id).unwrap();
        assert_eq!(reqs.len(), 9);
        assert!(reqs.iter().all(|r| r.url.starts_with("{{base_url}}/")));
        let envs = storage::get_environments(&conn).unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].variables["base_url"], BASE_URL);

        // Second run: data present, nothing written.
        assert!(!seed_if_empty(&conn).unwrap());
        assert_eq!(storage::get_collections(&conn).unwrap().len(), 1);
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn never_touches_a_folder_with_collections() {
        let (dir, conn) = temp_db();
        let mine = Collection {
            id: "mine".into(),
            name: "Mine".into(),
            created_at: 0,
        };
        storage::create_collection(&conn, &mine).unwrap();
        assert!(!seed_if_empty(&conn).unwrap());
        let cols = storage::get_collections(&conn).unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].name, "Mine");
        drop(conn);
        let _ = std::fs::remove_dir_all(dir);
    }
}
