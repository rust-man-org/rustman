//! Shared parser for the resource dialog: no widget or storage side effects.
use super::{PastedCommand, classify_pasted_command};

#[derive(Debug, Clone)]
pub enum Resource {
    Collections(Vec<crate::domain::group::ImportedCollection>),
    Command(crate::services::curl::ParsedCurl, usize),
}

pub fn parse(input: &str) -> Result<Resource, String> {
    if input.len() > crate::consts::IMPORT_FILE_LIMIT {
        return Err("Resource exceeds the 32 MB import limit.".into());
    }
    let parsed = match classify_pasted_command(input) {
        PastedCommand::Curl => Some(crate::services::curl::parse(input)),
        PastedCommand::Httpie => Some(super::httpie::parse(input)),
        PastedCommand::Other => None,
    };
    if let Some(command) = parsed {
        if command
            .url
            .as_deref()
            .is_none_or(|url| url.trim().is_empty())
        {
            return Err("The command needs a URL. Nothing was imported.".into());
        }
        return Ok(Resource::Command(command, input.len()));
    }
    super::postman::import(input)
        .or_else(|_| super::native_import(input))
        .or_else(|_| super::httpie_export::import(input))
        .or_else(|_| super::swagger::import(input))
        .map(Resource::Collections)
        .map_err(|_| "Unrecognized resource. Paste a cURL or HTTPie command, Postman collection, OpenAPI spec, or Rustman JSON.".into())
}

/// Parse everything before applying anything. Bounded reads keep a dropped
/// folder from allocating arbitrarily large files; symlinks are not followed.
pub fn read_paths(paths: Vec<std::path::PathBuf>) -> Result<Vec<Resource>, String> {
    use std::io::Read;
    let mut pending = paths;
    let mut resources = Vec::new();
    let mut total = 0;
    let mut visited = 0;
    while let Some(path) = pending.pop() {
        visited += 1;
        if visited > 1024 {
            return Err("Select fewer files (maximum 1024 folder entries).".into());
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
                let child = entry.map_err(|e| e.to_string())?.path();
                // Ignore unrelated files in folders; explicitly selected files
                // may have any extension (e.g. a saved curl command).
                if child.is_dir()
                    || child.extension().and_then(|s| s.to_str()).is_some_and(|s| {
                        matches!(
                            s.to_ascii_lowercase().as_str(),
                            "json" | "yaml" | "yml" | "txt" | "curl" | "sh"
                        )
                    })
                {
                    pending.push(child);
                }
                if pending.len() + visited > 1024 {
                    return Err("Select fewer files (maximum 1024 folder entries).".into());
                }
            }
            continue;
        }
        let mut bytes = Vec::new();
        let remaining = crate::consts::IMPORT_FILE_LIMIT.saturating_sub(total);
        std::fs::File::open(&path)
            .map_err(|e| e.to_string())?
            .take(remaining as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        total += bytes.len();
        if total > crate::consts::IMPORT_FILE_LIMIT {
            return Err("Selection exceeds the 32 MB import limit.".into());
        }
        let input = String::from_utf8(bytes)
            .map_err(|_| format!("{} is not UTF-8 text.", path.display()))?;
        resources.push(parse(&input).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    if resources.is_empty() {
        return Err("No supported resources found in this selection.".into());
    }
    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_curl_and_rejects_incomplete_commands() {
        assert!(matches!(
            parse("curl https://example.com"),
            Ok(Resource::Command(_, _))
        ));
        assert!(parse("curl -X POST").is_err());
        assert!(parse("unrecognized text").is_err());
    }
    #[test]
    fn accepts_postman_json() {
        let json = r#"{"info":{"name":"Imported","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[]}"#;
        assert!(matches!(parse(json), Ok(Resource::Collections(_))));
    }

    #[test]
    fn folder_import_parses_supported_files_and_reports_invalid_file() {
        let dir = std::env::temp_dir().join(format!("rustman-resources-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested/request.curl"), "curl https://example.com").unwrap();
        std::fs::write(dir.join("ignored.png"), [0, 255]).unwrap();
        assert_eq!(read_paths(vec![dir.clone()]).unwrap().len(), 1);
        std::fs::write(dir.join("invalid.json"), "not JSON").unwrap();
        let error = read_paths(vec![dir.clone()]).unwrap_err();
        assert!(error.contains("invalid.json"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn oversized_files_are_rejected_before_parsing() {
        let path =
            std::env::temp_dir().join(format!("rustman-resource-{}.txt", uuid::Uuid::new_v4()));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(crate::consts::IMPORT_FILE_LIMIT as u64 + 1)
            .unwrap();
        drop(file);
        assert!(
            read_paths(vec![path.clone()])
                .unwrap_err()
                .contains("32 MB")
        );
        std::fs::remove_file(path).unwrap();
    }
}
