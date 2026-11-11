use rusqlite::{Connection, Row, params};
use rustman_core::domain::collection::SavedRequest;

use crate::error::DbError;
use crate::rows::{decode, readable};

const COLUMNS: &str = "data, group_id, sort_order, collection_id";

/// A collection's requests, by manual order then insertion order.
pub(crate) fn in_collection(
    conn: &Connection,
    collection_id: &str,
) -> Result<Vec<SavedRequest>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM requests WHERE collection_id=?1 ORDER BY sort_order ASC, rowid ASC"
    ))?;
    let rows = stmt.query_map(params![collection_id], request_row)?;
    Ok(rows.filter_map(|row| readable(row, "request")).collect())
}

/// Every request, by manual order then insertion order.
pub(crate) fn all(conn: &Connection) -> Result<Vec<SavedRequest>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM requests ORDER BY sort_order ASC, rowid ASC"
    ))?;
    let rows = stmt.query_map([], request_row)?;
    Ok(rows.filter_map(|row| readable(row, "request")).collect())
}

fn request_row(r: &Row<'_>) -> rusqlite::Result<Option<SavedRequest>> {
    let Some(mut req) = decode::<SavedRequest>(
        r.get_ref(0)?.as_str()?,
        "request in collection",
        r.get_ref(3)?.as_str()?,
    ) else {
        return Ok(None);
    };
    // The placement columns win when set; rows older than the columns carry
    // their placement only in the JSON.
    let group_id: Option<String> = r.get(1)?;
    if group_id.is_some() {
        req.group_id = group_id;
    }
    let sort_order: i64 = r.get(2)?;
    if sort_order != 0 {
        req.sort_order = sort_order;
    }
    Ok(Some(req))
}

/// Inserts `req`, or replaces the request with its id.
pub(crate) fn upsert(conn: &Connection, req: &SavedRequest) -> Result<(), DbError> {
    let data = serde_json::to_string(req)?;
    conn.execute(
        "INSERT OR REPLACE INTO requests (id, collection_id, group_id, sort_order, data) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            req.id,
            req.collection_id,
            req.group_id,
            req.sort_order,
            data
        ],
    )?;
    Ok(())
}

/// Sets the stored request's name in place, so a large body isn't decoded
/// and written again.
pub(crate) fn rename(conn: &Connection, id: &str, name: &str) -> Result<(), DbError> {
    conn.execute(
        "UPDATE requests SET data = json_set(data, '$.name', ?2) WHERE id = ?1",
        params![id, name],
    )?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute("DELETE FROM requests WHERE id=?1", params![id])?;
    Ok(())
}

pub(crate) fn delete_in_collection(conn: &Connection, collection_id: &str) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM requests WHERE collection_id=?1",
        params![collection_id],
    )?;
    Ok(())
}
