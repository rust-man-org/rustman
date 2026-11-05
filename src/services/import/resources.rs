//! Shared parser for the resource dialog: no widget or storage side effects.
use super::{PastedCommand, classify_pasted_command};

#[derive(Debug, Clone)]
pub enum Resource {
    Collections(Vec<crate::domain::group::ImportedCollection>),
    Command(crate::services::curl::ParsedCurl, usize),
    Commands(Vec<(crate::services::curl::ParsedCurl, usize)>),
    /// Postman environments, and collection variables mapped to one.
    Environments(Vec<crate::domain::environment::AppEnvironment>),
}

/// What an import will create, for the dialog's review panel.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub collections: usize,
    pub folders: usize,
    pub requests: usize,
    pub variables: usize,
    pub environments: usize,
    /// Requests whose Postman scripts were kept as comments.
    pub scripts_to_review: usize,
}

impl Summary {
    pub fn of(resources: &[Resource]) -> Self {
        let marker = super::postman::SCRIPT_MARKER;
        let mut s = Summary::default();
        for r in resources {
            match r {
                Resource::Collections(cols) => {
                    for c in cols {
                        s.collections += 1;
                        s.folders += c.all_groups().len();
                        let requests = c.all_requests();
                        s.requests += requests.len();
                        s.scripts_to_review += requests
                            .iter()
                            .filter(|r| {
                                r.pre_request_script.starts_with(marker)
                                    || r.test_script.starts_with(marker)
                            })
                            .count();
                    }
                }
                Resource::Command(..) => s.requests += 1,
                Resource::Commands(batch) => s.requests += batch.len(),
                Resource::Environments(envs) => {
                    s.environments += envs.len();
                    s.variables += envs.iter().map(|e| e.variables.len()).sum::<usize>();
                }
            }
        }
        s
    }
}

/// [`parse`] plus what rides along: a Postman collection's variables come
/// back as an environment, and a data dump's `archive.json` index (no data)
/// yields nothing instead of an error.
pub fn parse_all(input: &str) -> Result<Vec<Resource>, String> {
    if input.len() <= crate::consts::IMPORT_FILE_LIMIT && super::postman::is_dump_index(input) {
        return Ok(Vec::new());
    }
    let resource = parse(input)?;
    let mut out = Vec::with_capacity(2);
    if matches!(resource, Resource::Collections(_))
        && let Some(env) = super::postman::collection_environment(input)
    {
        out.push(resource);
        out.push(Resource::Environments(vec![env]));
    } else {
        out.push(resource);
    }
    Ok(out)
}

pub fn parse(input: &str) -> Result<Resource, String> {
    if input.len() > crate::consts::IMPORT_FILE_LIMIT {
        return Err("Resource exceeds the 32 MB import limit.".into());
    }
    if classify_pasted_command(input) != PastedCommand::Other {
        let mut commands = Vec::new();
        for (index, text) in split_commands(input)?.into_iter().enumerate() {
            let command = match classify_pasted_command(text) {
                PastedCommand::Curl => crate::services::curl::parse(text),
                PastedCommand::Httpie => super::httpie::parse(text),
                PastedCommand::Other => {
                    return Err(format!("Command {} is not cURL or HTTPie.", index + 1));
                }
            };
            if command
                .url
                .as_deref()
                .is_none_or(|url| url.trim().is_empty())
            {
                return Err(format!(
                    "Command {} needs a URL. Nothing was imported.",
                    index + 1
                ));
            }
            commands.push((command, text.len()));
        }
        return if commands.len() == 1 {
            let (command, len) = commands.pop().unwrap();
            Ok(Resource::Command(command, len))
        } else {
            Ok(Resource::Commands(commands))
        };
    }
    super::postman::import(input)
        .or_else(|_| super::native_import(input))
        .or_else(|_| super::httpie_export::import(input))
        .or_else(|_| super::swagger::import(input))
        .map(Resource::Collections)
        .or_else(|_| {
            super::postman::import_environment(input).map(|env| Resource::Environments(vec![env]))
        })
        .map_err(|_| "Unrecognized resource. Paste a cURL or HTTPie command, Postman collection or environment, OpenAPI spec, or Rustman JSON.".into())
}

/// Split pasted commands only at unquoted shell boundaries. Quoted JSON,
/// multiline payloads, escaped quotes and line continuations stay intact.
fn split_commands(input: &str) -> Result<Vec<&str>, String> {
    let mut commands = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut start = 0;
    let mut chars = input.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if escaped {
            escaped = false;
            if ch == '\r' && chars.peek().is_some_and(|(_, c)| *c == '\n') {
                chars.next();
            }
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        let mut end = index + ch.len_utf8();
        let separator = if ch == '&' && chars.peek().is_some_and(|(_, c)| *c == '&') {
            chars.next();
            end += 1;
            true
        } else {
            ch == ';'
        };
        let next_command = classify_pasted_command(&input[end..]) != PastedCommand::Other;
        if separator || (ch == '\n' && next_command) {
            let command = input[start..index].trim();
            if !command.is_empty() {
                commands.push(command);
            }
            start = end;
            if commands.len() >= 1024 {
                return Err("Import at most 1024 commands at once.".into());
            }
        }
    }
    if quote.is_some() {
        return Err("A command has an unclosed quote.".into());
    }
    let last = input[start..].trim();
    if !last.is_empty() {
        commands.push(last);
    }
    Ok(commands)
}

pub fn command_count(resources: &[Resource]) -> usize {
    resources
        .iter()
        .map(|r| match r {
            Resource::Command(..) => 1,
            Resource::Commands(c) => c.len(),
            _ => 0,
        })
        .sum()
}

/// Repeated pastes often have no trailing newline. Add a command boundary
/// only outside quotes, never inside an incomplete quoted payload.
pub fn needs_paste_separator(before: &str, pasted: &str) -> bool {
    !before.is_empty()
        && !before.ends_with(['\r', '\n'])
        && !before.trim_end().ends_with('\\')
        && classify_pasted_command(before) != PastedCommand::Other
        && classify_pasted_command(pasted) != PastedCommand::Other
        && split_commands(before).is_ok()
}

pub fn common_origin(resources: &[Resource]) -> Option<String> {
    let mut urls = resources.iter().flat_map(|r| match r {
        Resource::Command(command, _) => vec![command],
        Resource::Commands(commands) => commands.iter().map(|(command, _)| command).collect(),
        _ => vec![],
    });
    let origin_of = |command: &crate::services::curl::ParsedCurl| {
        let url = reqwest::Url::parse(command.url.as_deref()?).ok()?;
        (matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none())
        .then(|| url.origin().ascii_serialization())
    };
    let first = origin_of(urls.next()?)?;
    for command in urls {
        if origin_of(command).as_deref() != Some(&first) {
            return None;
        }
    }
    Some(first)
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
        let remaining = crate::consts::IMPORT_TOTAL_LIMIT
            .saturating_sub(total)
            .min(crate::consts::IMPORT_FILE_LIMIT);
        std::fs::File::open(&path)
            .map_err(|e| e.to_string())?
            .take(remaining as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > crate::consts::IMPORT_FILE_LIMIT {
            return Err(format!(
                "{} exceeds the 32 MB import limit.",
                path.display()
            ));
        }
        total += bytes.len();
        if total > crate::consts::IMPORT_TOTAL_LIMIT {
            return Err("Selection exceeds the 256 MB import limit.".into());
        }
        let input = String::from_utf8(bytes)
            .map_err(|_| format!("{} is not UTF-8 text.", path.display()))?;
        let input = input
            .strip_prefix('\u{feff}')
            .map(str::to_owned)
            .unwrap_or(input);
        resources.extend(parse_all(&input).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    if resources.is_empty() {
        return Err("No supported resources found in this selection.".into());
    }
    Ok(resources)
}

/// How one scanned file can be imported (mockup `.imp-status`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanStatus {
    /// Imports as is.
    Ready,
    /// Imports, but has Postman scripts kept as comments.
    Review,
    /// A format Rustman reads but cannot import (Postman v1).
    Unsupported(String),
    /// Could not be read or parsed.
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanKind {
    Collection,
    Environment,
}

/// One file of a folder import, parsed on its own so a bad file only
/// marks its own row.
#[derive(Debug, Clone)]
pub struct ScanEntry {
    pub file: String,
    pub name: String,
    pub kind: ScanKind,
    pub summary: Summary,
    pub status: ScanStatus,
    pub resources: Vec<Resource>,
}

impl ScanEntry {
    pub fn importable(&self) -> bool {
        matches!(self.status, ScanStatus::Ready | ScanStatus::Review)
    }
}

/// Expand folders (one level of nesting per entry, symlinks skipped) into
/// the files a folder import looks at.
fn collect_files(paths: Vec<std::path::PathBuf>) -> Result<Vec<std::path::PathBuf>, String> {
    let mut pending = paths;
    let mut files = Vec::new();
    let mut visited = 0;
    while let Some(path) = pending.pop() {
        visited += 1;
        if visited > 2048 {
            return Err("Select fewer files (maximum 2048 folder entries).".into());
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
                let child = entry.map_err(|e| e.to_string())?.path();
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
            }
            continue;
        }
        files.push(path);
    }
    files.sort();
    Ok(files)
}

/// Scan files and folders for the import picker. Unlike [`read_paths`],
/// every file gets its own row and status; only the selection-wide size
/// limit stops the scan. A data dump's `archive.json` index is skipped.
pub fn scan_paths(paths: Vec<std::path::PathBuf>) -> Result<Vec<ScanEntry>, String> {
    use std::io::Read;
    let mut entries = Vec::new();
    let mut total = 0usize;
    for path in collect_files(paths)? {
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let error = |file: &str, msg: String| ScanEntry {
            name: file.to_owned(),
            file: file.to_owned(),
            kind: ScanKind::Collection,
            summary: Summary::default(),
            status: ScanStatus::Error(msg),
            resources: Vec::new(),
        };
        let mut bytes = Vec::new();
        let read = std::fs::File::open(&path).and_then(|f| {
            f.take(crate::consts::IMPORT_FILE_LIMIT as u64 + 1)
                .read_to_end(&mut bytes)
        });
        if let Err(e) = read {
            entries.push(error(&file, e.to_string()));
            continue;
        }
        if bytes.len() > crate::consts::IMPORT_FILE_LIMIT {
            entries.push(error(&file, "Larger than the 32 MB import limit".into()));
            continue;
        }
        total += bytes.len();
        if total > crate::consts::IMPORT_TOTAL_LIMIT {
            return Err("Selection exceeds the 256 MB import limit.".into());
        }
        let Ok(input) = String::from_utf8(bytes) else {
            entries.push(error(&file, "Not UTF-8 text".into()));
            continue;
        };
        // Windows tools often write a UTF-8 BOM; JSON parsers reject it.
        let input = input
            .strip_prefix('\u{feff}')
            .map(str::to_owned)
            .unwrap_or(input);
        if super::postman::is_dump_index(&input) {
            continue;
        }
        if super::postman::is_v1_collection(&input) {
            let mut e = error(&file, String::new());
            e.name = super::postman::v1_name(&input).unwrap_or(file.clone());
            e.status = ScanStatus::Unsupported("Postman v1 format is not supported".into());
            entries.push(e);
            continue;
        }
        match parse_all(&input) {
            Err(e) => {
                let mut msg = e;
                if let Err(json) = serde_json::from_str::<serde_json::Value>(&input) {
                    msg = format!("Invalid JSON: {json}");
                }
                entries.push(error(&file, msg));
            }
            Ok(resources) => {
                let summary = Summary::of(&resources);
                let (kind, name) = match resources.first() {
                    Some(Resource::Environments(envs)) => (
                        ScanKind::Environment,
                        envs.first().map(|e| e.name.clone()).unwrap_or_default(),
                    ),
                    Some(Resource::Collections(cols)) => (
                        ScanKind::Collection,
                        cols.first()
                            .map(|c| c.collection.name.clone())
                            .unwrap_or_default(),
                    ),
                    _ => (ScanKind::Collection, file.clone()),
                };
                entries.push(ScanEntry {
                    file: file.clone(),
                    name: if name.trim().is_empty() { file } else { name },
                    kind,
                    status: if summary.scripts_to_review > 0 {
                        ScanStatus::Review
                    } else {
                        ScanStatus::Ready
                    },
                    summary,
                    resources,
                });
            }
        }
    }
    if entries.is_empty() {
        return Err("No supported resources found in this selection.".into());
    }
    // Importable rows first (collections, then environments), then the
    // unsupported / broken files; each group by name.
    let rank = |e: &ScanEntry| {
        (
            !e.importable(),
            e.kind == ScanKind::Environment,
            e.name.to_lowercase(),
        )
    };
    entries.sort_by_key(rank);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_six_curl_commands_in_order_without_merging_payloads() {
        let command = "curl --request POST 'https://httpbin.org/post' \\\n  --header 'Content-Type: application/json' \\\n  --data-raw '{\"message\":\"curl inside JSON; still one body\"}'";
        let input = std::iter::repeat_n(command, 6)
            .collect::<Vec<_>>()
            .join("\n");
        let parsed = parse(&input).unwrap();
        let Resource::Commands(commands) = parsed else {
            panic!("expected a batch")
        };
        assert_eq!(commands.len(), 6);
        for (command, _) in commands {
            assert_eq!(command.url.as_deref(), Some("https://httpbin.org/post"));
            assert_eq!(command.method.as_deref(), Some("POST"));
            assert_eq!(
                command.body.as_deref(),
                Some("{\"message\":\"curl inside JSON; still one body\"}")
            );
        }
    }

    #[test]
    fn batch_boundaries_ignore_quoted_separators_and_multiline_payloads() {
        let input = "curl https://example.com/one --data-raw 'line one\ncurl inside body; && unchanged' ; curl https://example.com/two && curl 'https://example.com/three?q=1;2'";
        let Resource::Commands(commands) = parse(input).unwrap() else {
            panic!("expected batch")
        };
        assert_eq!(commands.len(), 3);
        assert_eq!(
            commands[0].0.body.as_deref(),
            Some("line one\ncurl inside body; && unchanged")
        );
        assert_eq!(
            commands[2].0.url.as_deref(),
            Some("https://example.com/three?q=1;2")
        );
        assert!(parse("curl https://example.com/ok\ncurl --data-raw 'broken").is_err());
    }

    #[test]
    fn batch_origin_requires_every_command_to_share_the_same_safe_origin() {
        let mixed = parse("curl https://one.example/a\ncurl https://two.example/b").unwrap();
        assert!(common_origin(&[mixed]).is_none());
        let same = parse("curl https://example.com/a\ncurl https://example.com/b?q=1").unwrap();
        assert_eq!(
            common_origin(&[same]).as_deref(),
            Some("https://example.com")
        );
    }
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

    #[test]
    fn scan_rows_each_file_with_its_own_status() {
        let dir = std::env::temp_dir().join(format!("rustman-scan-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("collection")).unwrap();
        std::fs::create_dir_all(dir.join("environment")).unwrap();
        std::fs::write(dir.join("archive.json"), r#"{"collection": {"a": true}}"#).unwrap();
        std::fs::write(
            dir.join("collection/a.json"),
            "\u{feff}{\"info\": {\"name\": \"Alpha\"}, \"item\": []}",
        )
        .unwrap();
        std::fs::write(
            dir.join("collection/old.json"),
            r#"{"name": "Old", "requests": []}"#,
        )
        .unwrap();
        std::fs::write(dir.join("collection/bad.json"), r#"{"info": "#).unwrap();
        std::fs::write(
            dir.join("environment/e.json"),
            r#"{"name": "Env", "values": [{"key": "k", "value": "v"}]}"#,
        )
        .unwrap();
        let rows = scan_paths(vec![dir.clone()]).unwrap();
        let status = |name: &str| {
            rows.iter()
                .find(|r| r.name == name)
                .map(|r| r.status.clone())
        };
        assert_eq!(rows.len(), 4, "archive.json is skipped");
        assert_eq!(status("Alpha"), Some(ScanStatus::Ready), "BOM is tolerated");
        assert!(matches!(status("Old"), Some(ScanStatus::Unsupported(_))));
        assert!(matches!(status("bad.json"), Some(ScanStatus::Error(_))));
        let order: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            order,
            ["Alpha", "Env", "bad.json", "Old"],
            "importable first (collections, environments), then the rest"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
