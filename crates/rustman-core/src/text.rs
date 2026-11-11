//! Small text helpers shared by the app, the CLI and the formats.

/// The longest prefix of `text` that fits in `max_bytes` and ends on a char
/// boundary. Slicing a `str` mid-character panics, and release builds abort
/// on panic, so every byte-budget cut goes through this.
pub fn head_capped(text: &str, max_bytes: usize) -> &str {
    let mut end = max_bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Whether `text` parses as a JSON object or array: the shape of a JSON body.
pub fn is_json_document(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with(['{', '['])
        && serde_json::from_str::<serde::de::IgnoredAny>(trimmed).is_ok()
}

/// `text` re-indented as JSON, or `None` when it isn't JSON.
pub fn pretty_json(text: &str, use_tabs: bool) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    Some(pretty_value(&value, use_tabs))
}

/// `value` printed one member per line, indented by two spaces or a tab.
pub fn pretty_value(value: &serde_json::Value, use_tabs: bool) -> String {
    if !use_tabs {
        return serde_json::to_string_pretty(value).expect("a JSON value always serializes");
    }
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    serde::Serialize::serialize(
        value,
        &mut serde_json::Serializer::with_formatter(&mut buf, formatter),
    )
    .expect("a JSON value always serializes");
    String::from_utf8(buf).expect("serde_json writes UTF-8")
}

/// A byte count for display: `512 B`, `1.5 KB`, `5.0 MB`.
pub fn human_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = 1024 * KB;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

/// Whether Windows reserves `stem` for a device (`con`, `nul`, `com1`, …), in
/// which case no file can be named `stem` or `stem.<anything>`.
pub fn is_windows_device_name(stem: &str) -> bool {
    let lower = stem.to_ascii_lowercase();
    matches!(lower.as_str(), "con" | "prn" | "aux" | "nul")
        || matches!(
            lower.as_bytes(),
            [b'c', b'o', b'm', d] | [b'l', b'p', b't', d] if d.is_ascii_digit()
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_documents_are_objects_or_arrays_that_parse() {
        assert!(is_json_document(" {\"a\": [1, 2]}"));
        assert!(is_json_document("[]"));
        assert!(!is_json_document("{oops"));
        assert!(!is_json_document("\"just a string\""));
        assert!(!is_json_document("42"));
    }

    #[test]
    fn json_is_re_indented_with_spaces_or_tabs() {
        assert_eq!(
            pretty_json("{\"a\":[1]}", false).unwrap(),
            "{\n  \"a\": [\n    1\n  ]\n}"
        );
        assert_eq!(pretty_json("{\"a\":1}", true).unwrap(), "{\n\t\"a\": 1\n}");
        assert_eq!(pretty_json("not json", false), None);
    }

    #[test]
    fn human_size_picks_the_unit() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(5_242_880), "5.0 MB");
    }

    #[test]
    fn windows_device_names_are_recognised_in_any_case() {
        for name in ["con", "CON", "Nul", "aux", "prn", "com1", "LPT9"] {
            assert!(is_windows_device_name(name), "{name}");
        }
        for name in ["console", "com", "com10", "lpt", "nul1", "api"] {
            assert!(!is_windows_device_name(name), "{name}");
        }
    }
}
