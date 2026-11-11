use rusqlite::{Connection, params};
use rustman_core::domain::collection::Collection;

use crate::error::{DbError, atomically};
use crate::groups;

pub(crate) fn all(conn: &Connection) -> Result<Vec<Collection>, DbError> {
    let mut stmt =
        conn.prepare("SELECT id, name, created_at FROM collections ORDER BY created_at ASC")?;
    let rows = stmt.query_map([], |r| {
        Ok(Collection {
            id: r.get(0)?,
            name: r.get(1)?,
            created_at: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn insert(conn: &Connection, c: &Collection) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO collections (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![c.id, c.name, c.created_at],
    )?;
    Ok(())
}

/// Inserts `c`, or renames the collection with its id.
pub(crate) fn upsert(conn: &Connection, c: &Collection) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO collections (id, name, created_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT(id) DO UPDATE SET name = excluded.name",
        params![c.id, c.name, c.created_at],
    )?;
    Ok(())
}

pub(crate) fn rename(conn: &Connection, id: &str, name: &str) -> Result<(), DbError> {
    conn.execute(
        "UPDATE collections SET name=?1 WHERE id=?2",
        params![name, id],
    )?;
    Ok(())
}

/// Deletes the collection with every request and group inside it.
pub(crate) fn delete(conn: &Connection, id: &str) -> Result<(), DbError> {
    atomically(conn, || {
        conn.execute("DELETE FROM requests WHERE collection_id=?1", params![id])?;
        let groups = groups::all(conn)?;
        let mut delete_group = conn.prepare_cached("DELETE FROM groups WHERE id=?1")?;
        for group in groups::descendants(&groups, id, true) {
            delete_group.execute(params![group])?;
        }
        conn.execute("DELETE FROM collections WHERE id=?1", params![id])?;
        Ok(())
    })
}
