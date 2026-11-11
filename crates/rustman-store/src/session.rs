use rusqlite::{Connection, OptionalExtension, params};
use rustman_core::time::now_millis;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::DbError;

pub(crate) fn save<T: Serialize + ?Sized>(conn: &Connection, session: &T) -> Result<(), DbError> {
    let data = serde_json::to_string(session)?;
    conn.execute(
        "INSERT OR REPLACE INTO session (key, data, saved_at) VALUES ('current', ?1, ?2)",
        params![data, now_millis()],
    )?;
    Ok(())
}

/// The saved session, or `None` before the first save. A session that no
/// longer decodes is an error, not an empty session.
pub(crate) fn load<T: DeserializeOwned>(conn: &Connection) -> Result<Option<T>, DbError> {
    let decoded = conn
        .query_row("SELECT data FROM session WHERE key='current'", [], |r| {
            Ok(serde_json::from_str::<T>(r.get_ref(0)?.as_str()?))
        })
        .optional()?;
    Ok(decoded.transpose()?)
}

/// The length of the JSON array at `path` in the saved session, `None` when
/// there is no session, no array there, or no valid JSON at all.
pub(crate) fn array_len(conn: &Connection, path: &str) -> Result<Option<usize>, DbError> {
    let len: Option<i64> = conn
        .query_row(
            "SELECT CASE WHEN json_valid(data) THEN json_array_length(data, ?1) END \
             FROM session WHERE key='current'",
            params![path],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(len.and_then(|n| usize::try_from(n).ok()))
}

/// Copies the saved session row, decodable or not, under a new key that
/// `save` never writes. `None` when there is no saved session.
pub(crate) fn back_up(conn: &Connection) -> Result<Option<String>, DbError> {
    let key = format!("session_unreadable_{}", now_millis());
    let copied = conn.execute(
        "INSERT OR REPLACE INTO session (key, data, saved_at)
         SELECT ?1, data, saved_at FROM session WHERE key='current'",
        params![key],
    )?;
    Ok((copied > 0).then_some(key))
}
