use rusqlite::Connection;

/// A failed read or write. Its message is the underlying SQLite or JSON
/// error's, except a write to a read-only database, which says why.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DbError {
    #[error("{}", sql_message(.0))]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn sql_message(error: &rusqlite::Error) -> String {
    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ReadOnly) {
        "Not saved: Rustman is read-only in this window".to_owned()
    } else {
        error.to_string()
    }
}

/// Runs `write` all-or-nothing. A savepoint rather than `BEGIN`, so it nests
/// inside a transaction the caller may already hold on this connection.
pub(crate) fn atomically<T>(
    conn: &Connection,
    write: impl FnOnce() -> Result<T, DbError>,
) -> Result<T, DbError> {
    conn.execute_batch("SAVEPOINT store_write")?;
    let outcome = write().and_then(|value| {
        conn.execute_batch("RELEASE store_write")?;
        Ok(value)
    });
    if outcome.is_err() {
        conn.execute_batch("ROLLBACK TO store_write; RELEASE store_write")?;
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (v INTEGER)").unwrap();
        conn
    }

    fn rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap()
    }

    fn insert_then_fail(conn: &Connection) -> Result<(), DbError> {
        atomically(conn, || {
            conn.execute("INSERT INTO t VALUES (1)", [])?;
            conn.execute("INSERT INTO missing VALUES (2)", [])?;
            Ok(())
        })
    }

    #[test]
    fn a_failed_write_leaves_nothing_behind() {
        let conn = conn();
        assert!(insert_then_fail(&conn).is_err());
        assert_eq!(rows(&conn), 0);
        assert!(conn.is_autocommit(), "no transaction is left open");
    }

    #[test]
    fn a_successful_write_commits() {
        let conn = conn();
        atomically(&conn, || Ok(conn.execute("INSERT INTO t VALUES (1)", [])?)).unwrap();
        assert_eq!(rows(&conn), 1);
        assert!(conn.is_autocommit());
    }

    #[test]
    fn it_nests_inside_the_callers_transaction() {
        let conn = conn();
        conn.execute_batch("BEGIN; INSERT INTO t VALUES (0);")
            .unwrap();
        assert!(insert_then_fail(&conn).is_err());
        assert_eq!(rows(&conn), 1, "only the inner write is undone");
        conn.execute_batch("COMMIT").unwrap();
        assert_eq!(rows(&conn), 1);
    }
}
