use crate::domain::example::RequestExample;
use rusqlite::{Connection, params};

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS request_examples (
        id TEXT PRIMARY KEY,
        request_id TEXT NOT NULL REFERENCES requests(id) ON DELETE CASCADE,
        data TEXT NOT NULL
    ); CREATE INDEX IF NOT EXISTS request_examples_parent ON request_examples(request_id);",
    )
}

pub fn load(conn: &Connection) -> Result<Vec<RequestExample>, String> {
    let mut stmt = conn
        .prepare("SELECT data FROM request_examples ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| {
        let data = r.map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    })
    .collect()
}

pub fn save(conn: &Connection, example: &RequestExample) -> Result<(), String> {
    let data = serde_json::to_string(example).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO request_examples (id, request_id, data) VALUES (?1, ?2, ?3)
        ON CONFLICT(id) DO UPDATE SET request_id=excluded.request_id, data=excluded.data",
        params![example.id, example.request_id, data],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM request_examples WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn endpoint_example_storage_exists_on_boot() {
        let dir = std::env::temp_dir().join(format!("rustman-example-{}", uuid::Uuid::new_v4()));
        let db = super::super::open(&dir).unwrap();
        let count: i64 = db
            .query_row("SELECT count(*) FROM request_examples", [], |r| r.get(0))
            .expect("endpoint example storage must be created on boot");
        assert_eq!(count, 0);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn endpoint_example_action_is_in_native_request_menu() {
        let source = include_str!("../../ui/sidebar/collections.rs");
        let menu = source
            .split("fn request_menu(")
            .nth(1)
            .unwrap()
            .split("fn add_menu(")
            .next()
            .unwrap();
        assert!(
            menu.contains("\"Add example\""),
            "native endpoint menu must offer Add example"
        );
    }

    #[test]
    fn endpoint_example_survives_parent_save_and_cascades_on_delete() {
        use crate::domain::{
            collection::{Collection, SavedRequest},
            example::RequestExample,
        };
        let dir = std::env::temp_dir().join(format!("rustman-example-{}", uuid::Uuid::new_v4()));
        let db = super::super::open(&dir).unwrap();
        let col = Collection {
            id: "col".into(),
            name: "Collection".into(),
            created_at: 0,
        };
        super::super::create_collection(&db, &col).unwrap();
        let mut req = SavedRequest::new_in(col.id, "Login".into());
        super::super::create_request(&db, &req).unwrap();
        let example = RequestExample::new(req.clone(), None);
        super::save(&db, &example).unwrap();
        req.name = "Renamed endpoint".into();
        super::super::create_request(&db, &req).unwrap();
        assert_eq!(
            super::load(&db).unwrap().len(),
            1,
            "updating a parent must preserve its examples"
        );
        drop(db);
        let db = super::super::open(&dir).unwrap();
        assert_eq!(super::load(&db).unwrap()[0].request.name, "Login");
        super::super::delete_request(&db, &req.id).unwrap();
        assert!(super::load(&db).unwrap().is_empty());
        assert!(
            super::save(&db, &example).is_err(),
            "missing parent must be rejected"
        );
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
