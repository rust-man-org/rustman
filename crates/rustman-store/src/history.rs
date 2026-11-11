use std::borrow::Cow;

use rusqlite::{Connection, OptionalExtension, Row, params};
use rustman_core::domain::collection::SavedRequest;
use rustman_core::domain::history::{HistoryEntry, HistorySummary, OmittedPart};
use serde::{Deserialize, Serialize};

use crate::error::{DbError, atomically};
use crate::rows::{decode, readable};

/// History keeps this many entries, newest first.
pub(crate) const KEEP: i64 = 100;

const ENTRY_COLUMNS: &str = "id, timestamp, method, url, status, duration, request_json";

/// The newest entries, at most `limit`. Unreadable rows are skipped.
pub(crate) fn recent(conn: &Connection, limit: i64) -> Result<Vec<HistoryEntry>, DbError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {ENTRY_COLUMNS} FROM history ORDER BY timestamp DESC LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit], entry_row)?;
    Ok(rows
        .filter_map(|row| readable(row, "history entry"))
        .collect())
}

/// The newest entries without their requests, which are never read.
pub(crate) fn summaries(conn: &Connection, limit: i64) -> Result<Vec<HistorySummary>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, method, url, status, duration \
         FROM history ORDER BY timestamp DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        Ok(Some(HistorySummary {
            id: r.get(0)?,
            timestamp: r.get(1)?,
            method: r.get(2)?,
            url: r.get(3)?,
            status: r.get(4)?,
            duration_ms: r.get(5)?,
        }))
    })?;
    Ok(rows
        .filter_map(|row| readable(row, "history entry"))
        .collect())
}

/// The entry with `id`, or `None` when there is none or it doesn't decode.
pub(crate) fn entry(conn: &Connection, id: &str) -> Result<Option<HistoryEntry>, DbError> {
    let entry = conn
        .query_row(
            &format!("SELECT {ENTRY_COLUMNS} FROM history WHERE id = ?1"),
            [id],
            entry_row,
        )
        .optional()?;
    Ok(entry.flatten())
}

fn entry_row(r: &Row<'_>) -> rusqlite::Result<Option<HistoryEntry>> {
    let id: String = r.get(0)?;
    let json = r.get_ref(6)?.as_str()?;
    let Some(request) = decode(json, "history entry", &id) else {
        return Ok(None);
    };
    Ok(Some(HistoryEntry {
        id,
        timestamp: r.get(1)?,
        method: r.get(2)?,
        url: r.get(3)?,
        status: r.get(4)?,
        duration_ms: r.get(5)?,
        request,
        omitted: omissions(json),
    }))
}

/// A history request as stored: the request, plus what was left out of it.
/// An entry that lost nothing is the request's JSON alone.
#[derive(Serialize)]
struct StoredRequest<'a> {
    #[serde(flatten)]
    request: &'a SavedRequest,
    #[serde(skip_serializing_if = "<[OmittedPart]>::is_empty")]
    history_omitted: &'a [OmittedPart],
}

/// What [`add`] left out of a stored request. Only entries that lost
/// something carry the key, so the rest skip the second parse.
fn omissions(json: &str) -> Vec<OmittedPart> {
    #[derive(Deserialize)]
    struct Omissions {
        #[serde(default)]
        history_omitted: Vec<OmittedPart>,
    }
    if !json.contains("\"history_omitted\"") {
        return Vec::new();
    }
    serde_json::from_str::<Omissions>(json).map_or_else(|_| Vec::new(), |o| o.history_omitted)
}

/// Records `entry` without request content over `limit` bytes, and drops
/// whatever falls out of the newest [`KEEP`].
pub(crate) fn add(
    conn: &Connection,
    mut entry: Cow<'_, HistoryEntry>,
    limit: usize,
) -> Result<(), DbError> {
    if entry.has_large_parts(limit) {
        entry.to_mut().omit_large_parts(limit);
    }
    let request = serde_json::to_string(&StoredRequest {
        request: &entry.request,
        history_omitted: &entry.omitted,
    })?;
    atomically(conn, || {
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
                request
            ],
        )?;
        conn.execute(
            "DELETE FROM history WHERE id NOT IN \
             (SELECT id FROM history ORDER BY timestamp DESC LIMIT ?1)",
            [KEEP],
        )?;
        Ok(())
    })
}

pub(crate) fn clear(conn: &Connection) -> Result<(), DbError> {
    conn.execute("DELETE FROM history", [])?;
    Ok(())
}
