//! Lenient row reading: one unreadable row (written by a newer version, say)
//! is logged and skipped, and stays in the table, instead of failing the
//! whole load and hiding everything else.

use serde::de::DeserializeOwned;

/// The row if it read cleanly and decoded, else `None` (logged).
pub(crate) fn readable<T>(row: rusqlite::Result<Option<T>>, what: &str) -> Option<T> {
    row.unwrap_or_else(|e| {
        eprintln!("rustman: skipping unreadable {what}: {e}");
        None
    })
}

/// `json` decoded as `T`, or `None` (logged) when it isn't valid for `T`.
pub(crate) fn decode<T: DeserializeOwned>(json: &str, what: &str, id: &str) -> Option<T> {
    serde_json::from_str(json)
        .map_err(|e| eprintln!("rustman: skipping unreadable {what} {id}: {e}"))
        .ok()
}
