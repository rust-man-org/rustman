//! Rustman's SQLite persistence: collections, groups, requests,
//! environments, history and the session blob.
//!
//! [`SqliteStore`] implements the storage ports of `rustman_core::store`,
//! and [`StoreThread`] runs one off the caller's thread. The free functions
//! over a [`Connection`] are the older interface, with errors as text.
//!
//! Every multi-statement write runs in one savepoint, so it either lands
//! whole or not at all.

mod collections;
mod data_dir;
mod environments;
mod error;
mod groups;
mod history;
mod requests;
mod rows;
mod schema;
mod session;
mod sqlite;
mod thread;

use std::borrow::Cow;

use rustman_core::domain::collection::{Collection, SavedRequest};
use rustman_core::domain::environment::AppEnvironment;
use rustman_core::domain::group::Group;
use rustman_core::domain::history::{HISTORY_BODY_LIMIT, HistoryEntry, HistorySummary};
use rustman_core::store::StoreError;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::DbError;

pub use data_dir::{AppData, DataDirLock, ReadOnly, open_app_data};
pub use rusqlite::Connection;
pub use schema::open;
pub use sqlite::SqliteStore;
pub use thread::StoreThread;

fn reported<T>(result: Result<T, DbError>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

impl From<DbError> for StoreError {
    fn from(e: DbError) -> Self {
        Self::new(e.to_string())
    }
}

/// Every collection, oldest first.
pub fn get_collections(conn: &Connection) -> Result<Vec<Collection>, String> {
    reported(collections::all(conn))
}

/// Adds a collection; its id must be new.
pub fn create_collection(conn: &Connection, c: &Collection) -> Result<(), String> {
    reported(collections::insert(conn, c))
}

/// Renames a collection.
pub fn update_collection(conn: &Connection, id: &str, name: &str) -> Result<(), String> {
    reported(collections::rename(conn, id, name))
}

/// Deletes a collection with every group and request inside it.
pub fn delete_collection(conn: &Connection, id: &str) -> Result<(), String> {
    reported(collections::delete(conn, id))
}

/// Every group, by manual order then insertion order.
pub fn get_groups(conn: &Connection) -> Result<Vec<Group>, String> {
    reported(groups::all(conn))
}

/// Adds a group; its id must be new.
pub fn create_group(conn: &Connection, g: &Group) -> Result<(), String> {
    reported(groups::insert(conn, g))
}

/// Renames a group.
pub fn update_group(conn: &Connection, id: &str, name: &str) -> Result<(), String> {
    reported(groups::rename(conn, id, name))
}

/// Moves a group under a new parent at a new position, in one write.
pub fn update_group_placement(
    conn: &Connection,
    id: &str,
    parent_id: &str,
    parent_is_collection: bool,
    sort_order: i64,
) -> Result<(), String> {
    reported(groups::place(
        conn,
        id,
        parent_id,
        parent_is_collection,
        sort_order,
    ))
}

/// Deletes a group with every group and request under it.
pub fn delete_group(conn: &Connection, id: &str) -> Result<(), String> {
    reported(groups::delete(conn, id))
}

/// Every environment, by name.
pub fn get_environments(conn: &Connection) -> Result<Vec<AppEnvironment>, String> {
    reported(environments::all(conn))
}

/// Inserts or replaces an environment. Saving an active one deactivates the rest.
pub fn save_environment(conn: &Connection, env: &AppEnvironment) -> Result<(), String> {
    reported(environments::save(conn, env))
}

/// Makes `id` the only active environment, in one write; `None` activates
/// none.
pub fn set_active_environment(conn: &Connection, id: Option<&str>) -> Result<(), String> {
    reported(environments::set_active(conn, id))
}

/// Deletes an environment.
pub fn delete_environment(conn: &Connection, id: &str) -> Result<(), String> {
    reported(environments::delete(conn, id))
}

/// The newest history entries, newest first. Unreadable rows are skipped.
pub fn get_history(conn: &Connection) -> Result<Vec<HistoryEntry>, String> {
    reported(history::recent(conn, history::KEEP))
}

/// The newest history entries without their requests, newest first, at
/// most `limit`. Requests are never read, so this stays fast however large
/// they are.
pub fn history_summaries(conn: &Connection, limit: usize) -> Result<Vec<HistorySummary>, String> {
    reported(history::summaries(
        conn,
        i64::try_from(limit).unwrap_or(i64::MAX),
    ))
}

/// The history entry with `id`, or `None` when there is none or its request
/// doesn't decode.
pub fn history_entry(conn: &Connection, id: &str) -> Result<Option<HistoryEntry>, String> {
    reported(history::entry(conn, id))
}

/// Records a history entry, dropping the oldest beyond the retention limit.
/// Request bodies and form files over [`HISTORY_BODY_LIMIT`] are left out,
/// and listed in the stored entry's `omitted`.
/// Pass the entry by value where it isn't needed afterwards: a borrowed one
/// is copied when it has parts to leave out.
pub fn add_history<'a>(
    conn: &Connection,
    entry: impl Into<Cow<'a, HistoryEntry>>,
) -> Result<(), String> {
    reported(history::add(conn, entry.into(), HISTORY_BODY_LIMIT))
}

/// Deletes all history.
pub fn clear_history(conn: &Connection) -> Result<(), String> {
    reported(history::clear(conn))
}

/// Saves the session blob, replacing the previous one.
pub fn save_session<T: Serialize + ?Sized>(conn: &Connection, session: &T) -> Result<(), String> {
    reported(session::save(conn, session))
}

/// The saved session blob, or `None` if none was saved yet.
pub fn load_session<T: DeserializeOwned>(conn: &Connection) -> Result<Option<T>, String> {
    reported(session::load(conn))
}

/// Keeps a copy of the saved session blob under a new key, which it returns,
/// so a session this version can't read survives the next save.
pub fn back_up_session(conn: &Connection) -> Result<Option<String>, String> {
    reported(session::back_up(conn))
}

/// How many items the saved session holds in its JSON array at `path`
/// (e.g. `$.tabs`), read without decoding it, so a caller can tell when
/// lenient decoding dropped some.
pub fn session_array_len(conn: &Connection, path: &str) -> Result<Option<usize>, String> {
    reported(session::array_len(conn, path))
}

/// A collection's requests, by manual order then insertion order.
/// Unreadable rows are skipped.
pub fn get_requests(conn: &Connection, collection_id: &str) -> Result<Vec<SavedRequest>, String> {
    reported(requests::in_collection(conn, collection_id))
}

/// Inserts a request, or replaces the one with its id.
pub fn create_request(conn: &Connection, req: &SavedRequest) -> Result<(), String> {
    reported(requests::upsert(conn, req))
}

/// Deletes a request.
pub fn delete_request(conn: &Connection, id: &str) -> Result<(), String> {
    reported(requests::delete(conn, id))
}

/// Deletes every request in a collection.
pub fn delete_requests_for_collection(
    conn: &Connection,
    collection_id: &str,
) -> Result<(), String> {
    reported(requests::delete_in_collection(conn, collection_id))
}
