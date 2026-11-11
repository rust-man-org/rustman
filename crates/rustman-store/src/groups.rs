use std::collections::HashSet;

use rusqlite::{Connection, params};
use rustman_core::domain::group::Group;

use crate::error::{DbError, atomically};

pub(crate) fn all(conn: &Connection) -> Result<Vec<Group>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, parent_is_collection, name, sort_order \
         FROM groups ORDER BY sort_order ASC, rowid ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Group {
            id: r.get(0)?,
            parent_id: r.get(1)?,
            parent_is_collection: r.get(2)?,
            name: r.get(3)?,
            sort_order: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn insert(conn: &Connection, g: &Group) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO groups (id, parent_id, parent_is_collection, name, sort_order) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            g.id,
            g.parent_id,
            g.parent_is_collection,
            g.name,
            g.sort_order
        ],
    )?;
    Ok(())
}

pub(crate) fn rename(conn: &Connection, id: &str, name: &str) -> Result<(), DbError> {
    conn.execute("UPDATE groups SET name=?1 WHERE id=?2", params![name, id])?;
    Ok(())
}

pub(crate) fn place(
    conn: &Connection,
    id: &str,
    parent_id: &str,
    parent_is_collection: bool,
    sort_order: i64,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE groups SET parent_id=?1, parent_is_collection=?2, sort_order=?3 WHERE id=?4",
        params![parent_id, parent_is_collection, sort_order, id],
    )?;
    Ok(())
}

/// Deletes the group, every group under it and every request in that subtree.
pub(crate) fn delete(conn: &Connection, id: &str) -> Result<(), DbError> {
    atomically(conn, || {
        let mut ids = vec![id.to_owned()];
        ids.extend(descendants(&all(conn)?, id, false));
        let mut delete_requests = conn.prepare_cached("DELETE FROM requests WHERE group_id=?1")?;
        let mut delete_group = conn.prepare_cached("DELETE FROM groups WHERE id=?1")?;
        for group in &ids {
            delete_requests.execute(params![group])?;
            delete_group.execute(params![group])?;
        }
        Ok(())
    })
}

/// Ids of every group transitively under `parent_id`, parents before
/// children whatever order `groups` is in.
pub(crate) fn descendants(
    groups: &[Group],
    parent_id: &str,
    parent_is_collection: bool,
) -> Vec<String> {
    let mut claimed: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    let mut changed = true;
    while changed {
        changed = false;
        for g in groups {
            if claimed.contains(g.id.as_str()) {
                continue;
            }
            let direct = g.parent_id == parent_id && g.parent_is_collection == parent_is_collection;
            let nested = !g.parent_is_collection && claimed.contains(g.parent_id.as_str());
            if direct || nested {
                claimed.insert(&g.id);
                out.push(g.id.clone());
                changed = true;
            }
        }
    }
    out
}
