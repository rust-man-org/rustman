use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::{Connection, Result as SqlResult, params};
use serde_json;

use crate::domain::{
    collection::{Collection, SavedRequest},
    environment::AppEnvironment,
    group::Group,
    history::HistoryEntry,
};
use crate::state::session::AppSession;

// Company-edition table owners (contract C11).
pub mod drafts;
pub mod examples;
pub mod meta;
pub mod secrets;

// ── DB init ───────────────────────────────────────────────────────────────────

pub fn open(data_dir: &PathBuf) -> SqlResult<Connection> {
    std::fs::create_dir_all(data_dir).ok();
    let path = data_dir.join("rustman.db");
    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;

         CREATE TABLE IF NOT EXISTS collections (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             created_at INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS groups (
             id TEXT PRIMARY KEY,
             parent_id TEXT NOT NULL,
             parent_is_collection INTEGER NOT NULL,
             name TEXT NOT NULL,
             sort_order INTEGER NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS environments (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             variables TEXT NOT NULL DEFAULT '{}',
             is_active INTEGER NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS history (
             id TEXT PRIMARY KEY,
             timestamp INTEGER NOT NULL,
             method TEXT NOT NULL,
             url TEXT NOT NULL,
             status INTEGER NOT NULL DEFAULT 0,
             duration INTEGER NOT NULL DEFAULT 0,
             request_json TEXT NOT NULL DEFAULT '{}'
         );

         CREATE TABLE IF NOT EXISTS session (
             key TEXT PRIMARY KEY DEFAULT 'current',
             data TEXT NOT NULL DEFAULT '{}',
             saved_at INTEGER NOT NULL DEFAULT 0
         );

         CREATE TABLE IF NOT EXISTS requests (
             id TEXT PRIMARY KEY,
             collection_id TEXT NOT NULL,
             data TEXT NOT NULL DEFAULT '{}'
         );",
    )?;
    migrate(&conn);
    examples::migrate(&conn)?;
    Ok(conn)
}

/// Additive schema changes for databases created by older versions. Each
/// statement is independent and its error ignored, so re-running against an
/// already-migrated database is a no-op.
fn migrate(conn: &Connection) {
    // Company-edition tables (contract C11): each workstream owns its DDL.
    drafts::migrate(conn);
    meta::migrate(conn);
    secrets::migrate(conn);

    // Issue #53: endpoints can live inside groups and carry a manual order.
    let _ = conn.execute("ALTER TABLE requests ADD COLUMN group_id TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE requests ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
        [],
    );
}

// ── Collections ───────────────────────────────────────────────────────────────

pub fn get_collections(conn: &Connection) -> Result<Vec<Collection>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, created_at FROM collections ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}

pub fn create_collection(conn: &Connection, c: &Collection) -> Result<(), String> {
    conn.execute(
        "INSERT INTO collections (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![c.id, c.name, c.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_collection(conn: &Connection, id: &str, name: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE collections SET name=?1 WHERE id=?2",
        params![name, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_collection(conn: &Connection, id: &str) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    // Cascade: the collection's requests and every group underneath it go
    // with it. (Requests used to be orphaned here; groups are new.)
    tx.execute("DELETE FROM requests WHERE collection_id=?1", params![id])
        .map_err(|e| e.to_string())?;
    let groups = get_groups(&tx)?;
    for g in descendant_groups(&groups, id, true) {
        tx.execute("DELETE FROM groups WHERE id=?1", params![g])
            .map_err(|e| e.to_string())?;
    }
    tx.execute("DELETE FROM collections WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// All groups transitively under `parent_id`, ordered parents-first so a
/// top-down delete never orphans a row before its parent is gone.
pub fn descendant_groups(
    groups: &[Group],
    parent_id: &str,
    parent_is_collection: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut changed = true;
    // Iterative deepening: each pass picks up groups whose parent is already
    // claimed, so the result is parents-before-children regardless of how the
    // flat list happens to be ordered.
    while changed {
        changed = false;
        for g in groups {
            if out.contains(&g.id) {
                continue;
            }
            let parent_matches =
                g.parent_id == parent_id && g.parent_is_collection == parent_is_collection;
            let parent_claimed = !g.parent_is_collection && out.contains(&g.parent_id);
            if parent_matches || parent_claimed {
                out.push(g.id.clone());
                changed = true;
            }
        }
    }
    out
}

// ── Groups ────────────────────────────────────────────────────────────────────

pub fn get_groups(conn: &Connection) -> Result<Vec<Group>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, parent_id, parent_is_collection, name, sort_order \
             FROM groups ORDER BY sort_order ASC, rowid ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Group {
                id: r.get(0)?,
                parent_id: r.get(1)?,
                parent_is_collection: r.get::<_, i64>(2)? != 0,
                name: r.get(3)?,
                sort_order: r.get::<_, i64>(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.map(|r| r.map_err(|e| e.to_string())).collect()
}

pub fn create_group(conn: &Connection, g: &Group) -> Result<(), String> {
    conn.execute(
        "INSERT INTO groups (id, parent_id, parent_is_collection, name, sort_order) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            g.id,
            g.parent_id,
            g.parent_is_collection as i64,
            g.name,
            g.sort_order
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_group(conn: &Connection, id: &str, name: &str) -> Result<(), String> {
    conn.execute("UPDATE groups SET name=?1 WHERE id=?2", params![name, id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_group_order(conn: &Connection, id: &str, sort_order: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE groups SET sort_order=?1 WHERE id=?2",
        params![sort_order, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Re-parent a group (drag into another folder or collection).
pub fn update_group_parent(
    conn: &Connection,
    id: &str,
    parent_id: &str,
    parent_is_collection: bool,
    sort_order: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE groups SET parent_id=?1, parent_is_collection=?2, sort_order=?3 WHERE id=?4",
        params![parent_id, parent_is_collection, sort_order, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete a group and everything under it: descendant groups plus every
/// request in the subtree.
pub fn delete_group(conn: &Connection, id: &str) -> Result<(), String> {
    let groups = get_groups(conn)?;
    // The target itself plus everything under it, parents first.
    let mut ids = vec![id.to_owned()];
    ids.extend(descendant_groups(&groups, id, false));
    for gid in ids {
        conn.execute("DELETE FROM requests WHERE group_id=?1", params![gid])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM groups WHERE id=?1", params![gid])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Environments ──────────────────────────────────────────────────────────────

pub fn get_environments(conn: &Connection) -> Result<Vec<AppEnvironment>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, variables, is_active FROM environments ORDER BY name ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let vars_json: String = r.get(2)?;
            let is_active: i32 = r.get(3)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                vars_json,
                is_active,
            ))
        })
        .map_err(|e| e.to_string())?;

    rows.map(|r| {
        r.map_err(|e| e.to_string())
            .and_then(|(id, name, vars_json, is_active)| {
                let variables: HashMap<String, String> =
                    serde_json::from_str(&vars_json).unwrap_or_default();
                Ok(AppEnvironment {
                    id,
                    name,
                    variables,
                    is_active: is_active != 0,
                })
            })
    })
    .collect()
}

pub fn save_environment(conn: &Connection, env: &AppEnvironment) -> Result<(), String> {
    if env.is_active {
        conn.execute("UPDATE environments SET is_active=0", [])
            .map_err(|e| e.to_string())?;
    }
    let vars = serde_json::to_string(&env.variables).unwrap_or_else(|_| "{}".to_owned());
    conn.execute(
        "INSERT OR REPLACE INTO environments (id,name,variables,is_active) VALUES (?1,?2,?3,?4)",
        params![env.id, env.name, vars, env.is_active as i32],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_environment(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM environments WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── History ───────────────────────────────────────────────────────────────────

pub fn get_history(conn: &Connection) -> Result<Vec<HistoryEntry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id,timestamp,method,url,status,duration,request_json \
             FROM history ORDER BY timestamp DESC LIMIT 100",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i32>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    rows.map(|r| {
        r.map_err(|e| e.to_string()).and_then(
            |(id, timestamp, method, url, status, duration, req_json)| {
                let request: SavedRequest =
                    serde_json::from_str(&req_json).map_err(|e| e.to_string())?;
                Ok(HistoryEntry {
                    id,
                    timestamp,
                    method,
                    url,
                    status,
                    duration_ms: duration,
                    request,
                })
            },
        )
    })
    .collect()
}

pub fn add_history(conn: &Connection, entry: &HistoryEntry) -> Result<(), String> {
    let req_json = serde_json::to_string(&entry.request).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO history (id,timestamp,method,url,status,duration,request_json) \
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            entry.id,
            entry.timestamp,
            entry.method,
            entry.url,
            entry.status,
            entry.duration_ms,
            req_json
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM history WHERE id NOT IN \
         (SELECT id FROM history ORDER BY timestamp DESC LIMIT 100)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn clear_history(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM history", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Session ───────────────────────────────────────────────────────────────────

pub fn save_session(conn: &Connection, session: &AppSession) -> Result<(), String> {
    let data = serde_json::to_string(session).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR REPLACE INTO session (key, data, saved_at) VALUES ('current', ?1, ?2)",
        params![data, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_session(conn: &Connection) -> Result<Option<AppSession>, String> {
    match conn.query_row("SELECT data FROM session WHERE key='current'", [], |r| {
        r.get::<_, String>(0)
    }) {
        Ok(data) => {
            let session = serde_json::from_str(&data).map_err(|e| e.to_string())?;
            Ok(Some(session))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// ── Requests ──────────────────────────────────────────────────────────────────

pub fn get_requests(conn: &Connection, collection_id: &str) -> Result<Vec<SavedRequest>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT data, group_id, sort_order FROM requests \
             WHERE collection_id=?1 ORDER BY sort_order ASC, rowid ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![collection_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    rows.map(|r| {
        r.map_err(|e| e.to_string())
            .and_then(|(data, group_id, sort_order)| {
                let mut req: SavedRequest =
                    serde_json::from_str(&data).map_err(|e| e.to_string())?;
                // group_id/sort_order are columns, not part of the JSON blob;
                // old rows may predate them, so only override when present.
                if group_id.is_some() || req.group_id.is_some() {
                    req.group_id = group_id.or(req.group_id);
                }
                if sort_order != 0 || req.sort_order != 0 {
                    req.sort_order = if sort_order != 0 {
                        sort_order
                    } else {
                        req.sort_order
                    };
                }
                Ok(req)
            })
    })
    .collect()
}

pub fn create_request(conn: &Connection, req: &SavedRequest) -> Result<(), String> {
    let data = serde_json::to_string(req).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO requests (id, collection_id, group_id, sort_order, data) \
         VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(id) DO UPDATE SET \
         collection_id=excluded.collection_id, group_id=excluded.group_id, \
         sort_order=excluded.sort_order, data=excluded.data",
        params![
            req.id,
            req.collection_id,
            req.group_id,
            req.sort_order,
            data
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_request_order(conn: &Connection, id: &str, sort_order: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE requests SET sort_order=?1 WHERE id=?2",
        params![sort_order, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_request(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM requests WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Reapply a collection without deleting retained request rows. Their local
/// examples must survive a pull or an idempotent import with the same IDs.
pub fn replace_collection_contents(
    conn: &Connection,
    collection: &Collection,
    groups: &[Group],
    requests: &[SavedRequest],
) -> Result<(), String> {
    replace_collection_and_environment(conn, collection, groups, requests, None)
}

pub fn replace_collection_and_environment(
    conn: &Connection,
    collection: &Collection,
    groups: &[Group],
    requests: &[SavedRequest],
    environment: Option<&AppEnvironment>,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let collection_id = &collection.id;
    tx.execute(
        "INSERT INTO collections (id,name,created_at) VALUES (?1,?2,?3)
        ON CONFLICT(id) DO UPDATE SET name=excluded.name",
        params![collection.id, collection.name, collection.created_at],
    )
    .map_err(|e| e.to_string())?;
    let old_requests = get_requests(&tx, collection_id)?;
    let old_groups = get_groups(&tx)?;
    for id in descendant_groups(&old_groups, collection_id, true) {
        tx.execute("DELETE FROM groups WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
    }
    for group in groups {
        create_group(&tx, group)?;
    }
    for request in requests {
        create_request(&tx, request)?;
    }
    let incoming: std::collections::HashSet<_> = requests.iter().map(|r| r.id.as_str()).collect();
    for old in old_requests {
        if !incoming.contains(old.id.as_str()) {
            delete_request(&tx, &old.id)?;
        }
    }
    if let Some(environment) = environment {
        save_environment(&tx, environment)?;
    }
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod group_cascade_tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE groups (
                id TEXT PRIMARY KEY,
                parent_id TEXT NOT NULL,
                parent_is_collection INTEGER NOT NULL,
                name TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE requests (
                id TEXT PRIMARY KEY,
                collection_id TEXT NOT NULL,
                data TEXT NOT NULL DEFAULT '{}'
            );",
        )
        .expect("schema");
        migrate(&conn);
        conn
    }

    fn request_ids(conn: &Connection) -> Vec<String> {
        let mut stmt = conn.prepare("SELECT id FROM requests ORDER BY id").unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn deleting_a_group_removes_itself_subgroups_and_their_requests() {
        let conn = memory_db();
        let g1 = Group::new("col-1".into(), true, "Backend".into(), 0);
        let g2 = Group::new(g1.id.clone(), false, "Resource".into(), 0);
        create_group(&conn, &g1).unwrap();
        create_group(&conn, &g2).unwrap();

        let r_in_g1 =
            SavedRequest::new_in_group("col-1".into(), Some(g1.id.clone()), "a".into(), 0);
        let r_in_g2 =
            SavedRequest::new_in_group("col-1".into(), Some(g2.id.clone()), "b".into(), 0);
        let r_top = SavedRequest::new_in_group("col-1".into(), None, "c".into(), 0);
        create_request(&conn, &r_in_g1).unwrap();
        create_request(&conn, &r_in_g2).unwrap();
        create_request(&conn, &r_top).unwrap();

        delete_group(&conn, &g1.id).unwrap();

        assert!(
            get_groups(&conn).unwrap().is_empty(),
            "group and subgroup must be gone"
        );
        assert_eq!(
            request_ids(&conn),
            vec![r_top.id],
            "only the top-level request survives"
        );
    }

    #[test]
    fn group_order_round_trips() {
        let conn = memory_db();
        let a = Group::new("col-1".into(), true, "a".into(), 2);
        let b = Group::new("col-1".into(), true, "b".into(), 0);
        create_group(&conn, &a).unwrap();
        create_group(&conn, &b).unwrap();
        update_group_order(&conn, &a.id, 5).unwrap();

        let groups = get_groups(&conn).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups.iter().find(|g| g.id == a.id).unwrap().sort_order, 5);
        assert_eq!(groups.iter().find(|g| g.id == b.id).unwrap().sort_order, 0);
    }
}
