use std::path::Path;

use rusqlite::{Connection, Transaction, params};

use crate::data_dir::{create_private_dir, fs_error, restrict};

pub(crate) const DB_FILE: &str = "rustman.db";

type Migration = fn(&Transaction<'_>) -> rusqlite::Result<()>;

/// One step per schema version, in order. A database's `user_version` counts
/// the steps already applied.
const MIGRATIONS: &[Migration] = &[initial_tables, request_placement, history_by_time];

/// Opens (creating if needed) `rustman.db` in `data_dir` and brings its
/// schema up to date. The database holds credentials, so it and its
/// journal are readable by their owner only.
pub fn open(data_dir: &Path) -> rusqlite::Result<Connection> {
    create_private_dir(data_dir).map_err(|e| fs_error("create", data_dir, &e))?;
    let path = data_dir.join(DB_FILE);
    let mut conn = Connection::open(&path)?;
    // Before the journal exists: SQLite gives it the database's mode.
    restrict(&path, 0o600).map_err(|e| fs_error("protect", &path, &e))?;
    // In WAL mode NORMAL keeps the database consistent through a crash and
    // skips the fsync on every commit; only the last commits can be lost.
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;",
    )?;
    for journal in ["-wal", "-shm"] {
        let journal = data_dir.join(format!("{DB_FILE}{journal}"));
        if journal.exists() {
            restrict(&journal, 0o600).map_err(|e| fs_error("protect", &journal, &e))?;
        }
    }
    migrate(&mut conn)?;
    Ok(conn)
}

/// Whether the database was written by a build with migrations this one
/// lacks.
pub(crate) fn is_newer(conn: &Connection) -> rusqlite::Result<bool> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    Ok(usize::try_from(version).is_ok_and(|v| v > MIGRATIONS.len()))
}

/// A new in-memory database with today's schema.
pub(crate) fn open_in_memory() -> rusqlite::Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Applies the steps a database hasn't had yet, each in its own transaction.
/// A database from a newer version is left as it is.
pub(crate) fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let applied: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (version, step) in (1_i64..).zip(MIGRATIONS).skip_while(|(v, _)| *v <= applied) {
        let tx = conn.transaction()?;
        step(&tx)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}

/// The schema of the first releases. `IF NOT EXISTS` because databases from
/// before `user_version` was tracked already have these tables.
fn initial_tables(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS collections (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             created_at INTEGER NOT NULL
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
    )
}

/// Folders, and a group and manual order for requests.
fn request_placement(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS groups (
             id TEXT PRIMARY KEY,
             parent_id TEXT NOT NULL,
             parent_is_collection INTEGER NOT NULL,
             name TEXT NOT NULL,
             sort_order INTEGER NOT NULL DEFAULT 0
         );",
    )?;
    add_column(tx, "requests", "group_id", "TEXT")?;
    add_column(tx, "requests", "sort_order", "INTEGER NOT NULL DEFAULT 0")
}

/// History is listed newest first.
fn history_by_time(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch("CREATE INDEX IF NOT EXISTS history_by_time ON history (timestamp DESC)")
}

/// Adds a column unless it is already there, which it is in databases that
/// were migrated before `user_version` was tracked.
fn add_column(
    tx: &Transaction<'_>,
    table: &str,
    column: &str,
    declaration: &str,
) -> rusqlite::Result<()> {
    let exists: bool = tx.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2",
        params![table, column],
        |r| r.get(0),
    )?;
    if !exists {
        tx.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {declaration}"
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn latest() -> i64 {
        i64::try_from(MIGRATIONS.len()).unwrap()
    }

    fn user_version(conn: &Connection) -> i64 {
        conn.pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn a_fresh_database_gets_every_migration() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(user_version(&conn), latest());
    }

    #[test]
    fn migrating_twice_changes_nothing() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(user_version(&conn), latest());
    }

    #[test]
    fn an_untracked_but_already_migrated_database_is_adopted() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.pragma_update(None, "user_version", 0).unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(user_version(&conn), latest());
    }

    #[test]
    fn a_newer_database_is_left_alone() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let newer = latest() + 5;
        conn.pragma_update(None, "user_version", newer).unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(user_version(&conn), newer);
    }

    #[test]
    fn a_failed_step_rolls_back_and_keeps_the_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE VIEW requests AS SELECT 1 AS id")
            .unwrap();
        assert!(migrate(&mut conn).is_err(), "a view takes no new columns");
        assert_eq!(user_version(&conn), 1, "the first step stays applied");
        let groups: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'groups'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(groups, 0, "the failed step's table is rolled back");
    }
}
