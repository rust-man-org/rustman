//! SQLite tables for workstream 01 (local drafts, C9). Owned by 01.
//!
//! `request_drafts` never leaves this machine: nothing here is read by the
//! repo writer. No foreign key to `requests`, because sync deletes and
//! recreates base rows and a cascade would wipe drafts.

use rusqlite::{Connection, params};

use crate::domain::draft::{DraftField, RequestDraft};

/// Additive DDL. Idempotent, like the rest of `storage::migrate`.
pub(super) fn migrate(conn: &Connection) {
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS request_drafts (
             request_id    TEXT PRIMARY KEY,
             collection_id TEXT NOT NULL,
             data          TEXT NOT NULL,
             base          TEXT NOT NULL,
             updated_at    INTEGER NOT NULL,
             conflicts     TEXT NOT NULL DEFAULT '[]'
         );",
    );
    // Older draft tables (if any) predate durable conflicts.
    let _ = conn.execute(
        "ALTER TABLE request_drafts ADD COLUMN conflicts TEXT NOT NULL DEFAULT '[]'",
        [],
    );
}

/// Insert or replace one draft, data + base + conflicts together.
pub fn upsert_draft(conn: &Connection, draft: &RequestDraft) -> Result<(), String> {
    let data = serde_json::to_string(&draft.data).map_err(|e| e.to_string())?;
    let base = serde_json::to_string(&draft.base).map_err(|e| e.to_string())?;
    let conflicts = serde_json::to_string(&draft.conflicts).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO request_drafts (request_id, collection_id, data, base, updated_at, conflicts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(request_id) DO UPDATE SET
             collection_id = excluded.collection_id,
             data = excluded.data,
             base = excluded.base,
             updated_at = excluded.updated_at,
             conflicts = excluded.conflicts",
        params![
            draft.request_id,
            draft.collection_id,
            data,
            base,
            draft.updated_at,
            conflicts
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_draft(conn: &Connection, request_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM request_drafts WHERE request_id = ?1",
        params![request_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// All drafts. Rows that no longer parse are skipped rather than failing
/// the whole load; unknown conflict field names are dropped (validated on
/// load, per C9).
pub fn get_drafts(conn: &Connection) -> Result<Vec<RequestDraft>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT request_id, collection_id, data, base, updated_at, conflicts
             FROM request_drafts",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        let (request_id, collection_id, data, base, updated_at, conflicts) =
            row.map_err(|e| e.to_string())?;
        let (Ok(data), Ok(base)) = (serde_json::from_str(&data), serde_json::from_str(&base))
        else {
            continue;
        };
        let raw: Vec<serde_json::Value> = serde_json::from_str(&conflicts).unwrap_or_default();
        let mut fields: Vec<DraftField> = Vec::new();
        for v in raw {
            if let Ok(f) = serde_json::from_value::<DraftField>(v) {
                if !fields.contains(&f) {
                    fields.push(f);
                }
            }
        }
        out.push(RequestDraft {
            request_id,
            collection_id,
            data,
            base,
            updated_at,
            conflicts: fields,
            upstream_changed: false,
            orphaned: false,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::collection::SavedRequest;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        migrate(&c);
        c
    }

    fn draft() -> RequestDraft {
        let mut base = SavedRequest::new_in("col".into(), "Login".into());
        base.id = "req-1".into();
        base.url = "https://a.test".into();
        let mut data = base.clone();
        data.url = "https://mine.test".into();
        RequestDraft::new(&base, data, 7)
    }

    #[test]
    fn migrate_is_idempotent() {
        let c = conn();
        migrate(&c);
        migrate(&c);
        upsert_draft(&c, &draft()).unwrap();
    }

    #[test]
    fn round_trip_keeps_data_base_and_conflicts() {
        let c = conn();
        let mut theirs = draft().base;
        theirs.url = "https://theirs.test".into();
        let d = draft().rebase(&theirs);
        assert_eq!(d.conflicts, vec![DraftField::Url]);
        upsert_draft(&c, &d).unwrap();

        let back = get_drafts(&c).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].data.url, "https://mine.test");
        assert_eq!(back[0].base.url, "https://theirs.test");
        assert_eq!(back[0].conflicts, vec![DraftField::Url]);
        assert_eq!(back[0].updated_at, 7);
    }

    #[test]
    fn upsert_replaces_and_delete_removes() {
        let c = conn();
        let mut d = draft();
        upsert_draft(&c, &d).unwrap();
        d.data.body = "changed".into();
        upsert_draft(&c, &d).unwrap();
        let back = get_drafts(&c).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].data.body, "changed");

        delete_draft(&c, "req-1").unwrap();
        assert!(get_drafts(&c).unwrap().is_empty());
    }

    #[test]
    fn unknown_conflict_names_are_dropped_on_load() {
        let c = conn();
        upsert_draft(&c, &draft()).unwrap();
        c.execute(
            "UPDATE request_drafts SET conflicts = '[\"url\",\"no_such_field\",\"url\"]'",
            [],
        )
        .unwrap();
        let back = get_drafts(&c).unwrap();
        assert_eq!(back[0].conflicts, vec![DraftField::Url]);
    }

    #[test]
    fn corrupt_row_is_skipped_not_fatal() {
        let c = conn();
        upsert_draft(&c, &draft()).unwrap();
        c.execute(
            "INSERT INTO request_drafts (request_id, collection_id, data, base, updated_at)
             VALUES ('bad', 'col', 'not json', '{}', 0)",
            [],
        )
        .unwrap();
        assert_eq!(get_drafts(&c).unwrap().len(), 1);
    }
}
