pub mod httpie;
pub mod httpie_export;
pub mod postman;
pub mod resources;
pub mod swagger;

use serde::{Deserialize, Serialize};

use crate::domain::collection::{Collection, SavedRequest};
use crate::domain::group::ImportedGroup;

#[derive(Serialize, Deserialize)]
struct NativeCollectionFile {
    collection: Collection,
    #[serde(default)]
    requests: Vec<SavedRequest>,
    /// Folder structure inside the collection. Absent in files written before
    /// groups existed (issue #53) — those import as a flat request list.
    #[serde(default)]
    groups: Vec<ImportedGroup>,
}

pub fn native_export(
    collection: &Collection,
    requests: &[SavedRequest],
    groups: &[ImportedGroup],
) -> String {
    let file = NativeCollectionFile {
        collection: collection.clone(),
        requests: requests.to_vec(),
        groups: groups.to_vec(),
    };
    serde_json::to_string_pretty(&file).unwrap_or_else(|_| "{}".to_owned())
}

/// Every collection as one JSON array — the "export all" counterpart to
/// `native_export`. Re-importable via `native_import`, which accepts both
/// the single-object and the array shape.
pub fn native_export_all(collections: &[crate::domain::group::ImportedCollection]) -> String {
    let files: Vec<NativeCollectionFile> = collections
        .iter()
        .map(|ic| NativeCollectionFile {
            collection: ic.collection.clone(),
            requests: ic.requests.clone(),
            groups: ic.groups.clone(),
        })
        .collect();
    serde_json::to_string_pretty(&files).unwrap_or_else(|_| "[]".to_owned())
}

pub fn native_import(json: &str) -> Result<Vec<crate::domain::group::ImportedCollection>, String> {
    let convert = |file: NativeCollectionFile| crate::domain::group::ImportedCollection {
        collection: file.collection,
        requests: file.requests,
        groups: file.groups,
    };
    if let Ok(files) = serde_json::from_str::<Vec<NativeCollectionFile>>(json) {
        if !files.is_empty() {
            return Ok(files.into_iter().map(convert).collect());
        }
    }
    let file: NativeCollectionFile =
        serde_json::from_str(json).map_err(|e| format!("Not a Rustman collection: {e}"))?;
    Ok(vec![convert(file)])
}

/// What a pasted/dropped command line looks like, decided from its first
/// word only. Classification must stay O(prefix): this runs per keystroke in
/// the URL bar, and lowercasing megabytes of pasted text on every event used
/// to freeze input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PastedCommand {
    Curl,
    Httpie,
    Other,
}

pub fn classify_pasted_command(input: &str) -> PastedCommand {
    let head: String = input
        .trim_start()
        .chars()
        .take(16)
        .collect::<String>()
        .to_lowercase();
    if head.starts_with("curl ") || head.starts_with("curl\t") {
        PastedCommand::Curl
    } else if crate::services::import::httpie::is_httpie_command(input) {
        PastedCommand::Httpie
    } else {
        PastedCommand::Other
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;

    fn sample() -> crate::domain::group::ImportedCollection {
        crate::domain::group::ImportedCollection {
            collection: Collection {
                id: "c1".into(),
                name: "C".into(),
                created_at: 0,
            },
            requests: Vec::new(),
            groups: Vec::new(),
        }
    }

    #[test]
    fn export_all_round_trips_through_import() {
        let json = native_export_all(&[sample()]);
        let back = native_import(&json).expect("array form must import");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].collection.name, "C");
    }

    #[test]
    fn single_object_still_imports() {
        let json = r#"{"collection":{"id":"c1","name":"C","created_at":0}}"#;
        let back = native_import(json).expect("single-object form must import");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].collection.id, "c1");
    }
}
