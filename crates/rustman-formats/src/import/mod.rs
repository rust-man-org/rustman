//! Collection files in, collection files out: Postman, OpenAPI/Swagger,
//! HTTPie exports and Rustman's own format, plus pasted HTTPie commands.

pub mod httpie;
pub mod httpie_export;
pub mod postman;
mod registry;
pub mod swagger;
mod yaml;

pub use registry::{Confidence, FormatRegistry, Importer};

use std::borrow::Cow;

use rustman_core::domain::collection::{Collection, SavedRequest};
use rustman_core::domain::group::{ImportedCollection, ImportedGroup};
use rustman_core::secrets::strip_collection_secrets;
use serde::{Deserialize, Serialize};

/// The collections an import produced, and what it couldn't carry over,
/// phrased for the user.
#[derive(Debug, Clone, Default)]
pub struct ImportOutcome {
    pub collections: Vec<ImportedCollection>,
    pub warnings: Vec<String>,
}

impl From<Vec<ImportedCollection>> for ImportOutcome {
    fn from(collections: Vec<ImportedCollection>) -> Self {
        Self {
            collections,
            warnings: Vec::new(),
        }
    }
}

/// What an export writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportOptions {
    /// Whether credentials go in the file. Off by default, so a file made to
    /// be shared holds none: each bearer token, Basic password, API key
    /// value, JWT secret, Cookie auth string and cookie row value is written
    /// empty, and so is the value of every credential header row
    /// ([`CREDENTIAL_HEADERS`](rustman_core::secrets::CREDENTIAL_HEADERS)).
    /// `"basic_pass": "hunter2"` goes out as `""`, while a value that is only
    /// placeholders, like `"bearer_token": "{{token}}"`, is kept: it names
    /// where the secret is, not the secret. Params are never touched, so a
    /// key typed into one stays in the file.
    pub include_secrets: bool,
}

impl ExportOptions {
    /// `collection` as an export with these options writes it.
    pub(crate) fn apply(self, collection: &ImportedCollection) -> Cow<'_, ImportedCollection> {
        if self.include_secrets {
            return Cow::Borrowed(collection);
        }
        let mut shared = collection.clone();
        strip_collection_secrets(&mut shared);
        Cow::Owned(shared)
    }
}

#[derive(Deserialize)]
struct NativeCollectionFile {
    collection: Collection,
    #[serde(default)]
    requests: Vec<SavedRequest>,
    /// Absent in files written before collections had folders.
    #[serde(default)]
    groups: Vec<ImportedGroup>,
}

#[derive(Serialize)]
struct NativeCollectionRef<'a> {
    collection: &'a Collection,
    requests: &'a [SavedRequest],
    groups: &'a [ImportedGroup],
}

impl<'a> From<&'a ImportedCollection> for NativeCollectionRef<'a> {
    fn from(ic: &'a ImportedCollection) -> Self {
        Self {
            collection: &ic.collection,
            requests: &ic.requests,
            groups: &ic.groups,
        }
    }
}

/// One collection in Rustman's native format.
pub fn native_export(collection: &ImportedCollection, options: ExportOptions) -> String {
    let collection = options.apply(collection);
    serde_json::to_string_pretty(&NativeCollectionRef::from(&*collection))
        .expect("plain data always serializes")
}

/// Every collection as one JSON array, which [`native_import`] reads back.
pub fn native_export_all(collections: &[ImportedCollection], options: ExportOptions) -> String {
    let collections: Vec<_> = collections.iter().map(|c| options.apply(c)).collect();
    let files: Vec<NativeCollectionRef<'_>> = collections.iter().map(|c| (&**c).into()).collect();
    serde_json::to_string_pretty(&files).expect("plain data always serializes")
}

/// A native collection file: one collection, or an array of them.
pub fn native_import(json: &str) -> Result<Vec<ImportedCollection>, String> {
    let convert = |file: NativeCollectionFile| ImportedCollection {
        collection: file.collection,
        requests: file.requests,
        groups: file.groups,
    };
    let files = if json.trim_start().starts_with('[') {
        serde_json::from_str::<Vec<NativeCollectionFile>>(json)
    } else {
        serde_json::from_str::<NativeCollectionFile>(json).map(|file| vec![file])
    };
    files
        .map(|files| files.into_iter().map(convert).collect())
        .map_err(|e| format!("Not a Rustman collection: {e}"))
}

/// A collection file in any supported format: see [`FormatRegistry::import`].
pub fn import_collection(text: &str) -> Result<ImportOutcome, String> {
    FormatRegistry::standard().import(text)
}

/// [`import_collection`] without its warnings, which go to stderr instead.
pub fn import_collection_file(text: &str) -> Result<Vec<ImportedCollection>, String> {
    let outcome = import_collection(text)?;
    for warning in &outcome.warnings {
        eprintln!("rustman: import: {warning}");
    }
    Ok(outcome.collections)
}

/// The kind of command a paste holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PastedCommand {
    Curl,
    Httpie,
    Other,
}

/// What a pasted or dropped text is, judged from its first few KB so it
/// stays cheap on megabyte pastes.
pub fn classify_pasted_command(input: &str) -> PastedCommand {
    if crate::curl::looks_like_command(input) {
        PastedCommand::Curl
    } else if httpie::is_httpie_command(input) {
        PastedCommand::Httpie
    } else {
        PastedCommand::Other
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;

    #[test]
    fn single_object_still_imports() {
        let json = r#"{"collection":{"id":"c1","name":"C","created_at":0}}"#;
        let back = native_import(json).expect("single-object form must import");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].collection.id, "c1");
    }
}

#[cfg(test)]
mod collection_file_tests {
    use super::*;

    #[test]
    fn errors_come_from_the_format_the_file_resembles() {
        let broken_postman = r#"{"info": {"_postman_id": "x", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/"}, "item": 5}"#;
        let err = import_collection_file(broken_postman).unwrap_err();
        assert!(
            err.starts_with("This looks like a Postman collection"),
            "{err}"
        );

        let err = import_collection_file("hello").unwrap_err();
        assert!(err.starts_with("Not a Postman"), "{err}");
    }

    /// Windows Notepad and PowerShell 5 start UTF-8 files with a byte order
    /// mark.
    #[test]
    fn a_file_saved_with_a_byte_order_mark_imports() {
        let postman = r#"{"info": {"_postman_id": "x", "name": "P", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/"}, "item": []}"#;
        let native = r#"{"collection":{"id":"c1","name":"C","created_at":0}}"#;
        let openapi = "openapi: 3.0.0\ninfo: {title: O, version: '1'}\npaths: {/a: {get: {}}}\n";
        for text in [postman, native, openapi] {
            let with_bom = format!("\u{feff}{text}");
            let names = |text: &str| {
                import_collection(text).map(|o| {
                    o.collections
                        .into_iter()
                        .map(|c| c.collection.name)
                        .collect::<Vec<_>>()
                })
            };
            assert_eq!(names(&with_bom), names(text), "{text}");
        }
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    #[test]
    fn pastes_are_classified_by_their_program() {
        assert_eq!(
            classify_pasted_command("$ curl https://x.io"),
            PastedCommand::Curl
        );
        assert_eq!(
            classify_pasted_command("HTTPS POST x.io a=1"),
            PastedCommand::Httpie
        );
        assert_eq!(classify_pasted_command("xh\tx.io"), PastedCommand::Httpie);
        assert_eq!(
            classify_pasted_command("https://x.io"),
            PastedCommand::Other
        );
        assert_eq!(classify_pasted_command("httpie"), PastedCommand::Other);
    }
}
