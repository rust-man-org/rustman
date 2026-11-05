//! Shared parser for the resource dialog: no widget or storage side effects.
use super::{PastedCommand, classify_pasted_command};

#[derive(Debug, Clone)]
pub enum Resource {
    Collections(Vec<crate::domain::group::ImportedCollection>),
    Command(crate::services::curl::ParsedCurl, usize),
    Commands(Vec<(crate::services::curl::ParsedCurl, usize)>),
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
        .map_err(|_| "Unrecognized resource. Paste a cURL or HTTPie command, Postman collection, OpenAPI spec, or Rustman JSON.".into())
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
}
