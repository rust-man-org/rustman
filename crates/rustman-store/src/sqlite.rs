use std::borrow::Cow;
use std::path::Path;

use rusqlite::Connection;
use rustman_core::domain::history::{HISTORY_BODY_LIMIT, HistoryEntry};
use rustman_core::store::{
    Change, HistoryStore, SessionStore, StoreError, Workspace, WorkspaceStore,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{DbError, atomically};
use crate::{collections, environments, groups, history, requests, schema, session};

/// The SQLite database that holds a workspace, its history and the session.
#[derive(Debug)]
pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// Opens (creating if needed) the database in `data_dir`, migrated to
    /// today's schema.
    pub fn open(data_dir: &Path) -> Result<Self, StoreError> {
        Ok(Self {
            conn: schema::open(data_dir).map_err(DbError::from)?,
        })
    }

    /// A store over a connection already opened with today's schema, such
    /// as [`open_app_data`](crate::open_app_data)'s.
    pub fn from_connection(conn: Connection) -> Self {
        Self { conn }
    }

    /// An empty database that lives only as long as the store.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Ok(Self {
            conn: schema::open_in_memory().map_err(DbError::from)?,
        })
    }

    /// The connection, for the crate's free functions.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

impl WorkspaceStore for SqliteStore {
    fn load_workspace(&self) -> Result<Workspace, StoreError> {
        let conn = &self.conn;
        // One snapshot, so a concurrent writer can't be seen half done.
        Ok(atomically(conn, || {
            Ok(Workspace {
                collections: collections::all(conn)?,
                groups: groups::all(conn)?,
                requests: requests::all(conn)?,
                environments: environments::all(conn)?,
            })
        })?)
    }

    fn apply(&mut self, changes: &[Change]) -> Result<(), StoreError> {
        let conn = &self.conn;
        Ok(atomically(conn, || {
            changes.iter().try_for_each(|change| apply(conn, change))
        })?)
    }
}

fn apply(conn: &Connection, change: &Change) -> Result<(), DbError> {
    match change {
        Change::PutCollection(collection) => collections::upsert(conn, collection),
        Change::RenameCollection { id, name } => collections::rename(conn, id, name),
        Change::DeleteCollection { id } => collections::delete(conn, id),
        Change::AddGroup(group) => groups::insert(conn, group),
        Change::RenameGroup { id, name } => groups::rename(conn, id, name),
        Change::PlaceGroup {
            id,
            parent_id,
            parent_is_collection,
            sort_order,
        } => groups::place(conn, id, parent_id, *parent_is_collection, *sort_order),
        Change::DeleteGroup { id } => groups::delete(conn, id),
        Change::PutRequest(request) => requests::upsert(conn, request),
        Change::RenameRequest { id, name } => requests::rename(conn, id, name),
        Change::DeleteRequest { id } => requests::delete(conn, id),
        Change::DeleteRequestsIn { collection_id } => {
            requests::delete_in_collection(conn, collection_id)
        }
        Change::PutEnvironment(env) => environments::save(conn, env),
        Change::ActivateEnvironment { id } => environments::set_active(conn, id.as_deref()),
        Change::DeleteEnvironment { id } => environments::delete(conn, id),
    }
}

impl HistoryStore for SqliteStore {
    fn recent_history(&self, limit: usize) -> Result<Vec<HistoryEntry>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        Ok(history::recent(&self.conn, limit)?)
    }

    fn append_history(&mut self, entry: HistoryEntry) -> Result<(), StoreError> {
        Ok(history::add(
            &self.conn,
            Cow::Owned(entry),
            HISTORY_BODY_LIMIT,
        )?)
    }

    fn clear_history(&mut self) -> Result<(), StoreError> {
        Ok(history::clear(&self.conn)?)
    }
}

impl SessionStore for SqliteStore {
    fn save_session<S: Serialize + ?Sized>(&mut self, session: &S) -> Result<(), StoreError> {
        Ok(session::save(&self.conn, session)?)
    }

    fn load_session<S: DeserializeOwned>(&self) -> Result<Option<S>, StoreError> {
        Ok(session::load(&self.conn)?)
    }
}
