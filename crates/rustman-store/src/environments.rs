use std::collections::HashMap;

use rusqlite::{Connection, params};
use rustman_core::domain::environment::AppEnvironment;

use crate::error::{DbError, atomically};

pub(crate) fn all(conn: &Connection) -> Result<Vec<AppEnvironment>, DbError> {
    let mut stmt =
        conn.prepare("SELECT id, name, variables, is_active FROM environments ORDER BY name ASC")?;
    let rows = stmt.query_map([], |r| {
        let id: String = r.get(0)?;
        Ok(AppEnvironment {
            variables: variables(r.get_ref(2)?.as_str()?, &id),
            id,
            name: r.get(1)?,
            is_active: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// An environment whose variables don't decode still loads, empty, so it
/// can be fixed from the UI rather than vanishing.
fn variables(json: &str, env_id: &str) -> HashMap<String, String> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        eprintln!("rustman: environment {env_id} has unreadable variables: {e}");
        HashMap::new()
    })
}

/// Saves `env`, deactivating every other environment when it is active.
///
/// Variables that didn't decode load as an empty map; saving that empty map
/// back keeps the stored text, so a rename or an activation can't destroy
/// what is still there to be repaired.
pub(crate) fn save(conn: &Connection, env: &AppEnvironment) -> Result<(), DbError> {
    let variables = serde_json::to_string(&env.variables)?;
    atomically(conn, || {
        if env.is_active {
            conn.execute("UPDATE environments SET is_active=0", [])?;
        }
        conn.execute(
            "INSERT INTO environments (id, name, variables, is_active) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 is_active = excluded.is_active,
                 variables = CASE
                     WHEN excluded.variables = '{}' AND NOT json_valid(environments.variables)
                     THEN environments.variables
                     ELSE excluded.variables
                 END",
            params![env.id, env.name, variables, env.is_active],
        )?;
        Ok(())
    })
}

/// Makes `id` the only active environment; no id activates none.
pub(crate) fn set_active(conn: &Connection, id: Option<&str>) -> Result<(), DbError> {
    conn.execute(
        "UPDATE environments SET is_active = (id IS ?1)",
        params![id],
    )?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: &str) -> Result<(), DbError> {
    conn.execute("DELETE FROM environments WHERE id=?1", params![id])?;
    Ok(())
}
