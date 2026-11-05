//! Variable lookup shared by URL assistance and snippet previews.
use crate::domain::environment::AppEnvironment;
use std::ops::Range;

pub const MASK: &str = "••••••";

pub fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace('-', "_");
    [
        "token",
        "secret",
        "password",
        "passwd",
        "api_key",
        "apikey",
        "authorization",
        "cookie",
    ]
    .iter()
    .any(|part| key.contains(part))
}

pub fn preview(key: &str, value: &str) -> String {
    if sensitive(key) {
        MASK.into()
    } else {
        crate::services::format::head_capped(value, 512).to_owned()
    }
}

/// The unfinished token at the caret, including an existing closing pair
/// when editing a completed token. Offsets are UTF-8 byte positions like iced.
pub fn completion(text: &str, caret: usize) -> Option<(Range<usize>, &str)> {
    let before = text.get(..caret)?;
    let start = before.rfind("{{")?;
    let query = &text[start + 2..caret];
    if query.contains(['{', '}', '/', '?', '#']) {
        return None;
    }
    let end = text[caret..]
        .find("}}")
        .filter(|&n| !text[caret..caret + n].contains(['{', '/', '?', '#']))
        .map_or(caret, |n| caret + n + 2);
    Some((start..end, query.trim()))
}

pub fn candidates(env: Option<&AppEnvironment>, query: &str) -> Vec<String> {
    let query = query.to_lowercase();
    let mut keys: Vec<_> = env
        .into_iter()
        .flat_map(|e| e.variables.keys())
        .filter(|key| key.to_lowercase().starts_with(&query))
        .cloned()
        .collect();
    keys.sort_unstable();
    keys.truncate(8);
    keys
}

pub fn variable_at(text: &str, caret: usize) -> Option<String> {
    let mut from = 0;
    while let Some(start) = text[from..].find("{{").map(|i| i + from) {
        let end = text[start + 2..].find("}}")? + start + 2;
        if (start..end + 2).contains(&caret) {
            return Some(text[start + 2..end].trim().to_owned());
        }
        from = end + 2;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_replaces_only_the_current_token() {
        let text = "https://x/{{base_}}/users/{{id}}";
        let (range, query) = completion(text, 17).unwrap();
        assert_eq!(query, "base_");
        let mut replaced = text.to_owned();
        replaced.replace_range(range, "{{base_url}}");
        assert_eq!(replaced, "https://x/{{base_url}}/users/{{id}}");
        assert!(completion("{{base}}/users", 14).is_none());
        assert!(completion("é/{{ba", 1).is_none());
        assert_eq!(completion("é/{{ba", 7).unwrap().1, "ba");
    }
    #[test]
    fn preview_masks_secret_keys_and_hover_only_matches_tokens() {
        assert_eq!(preview("access_token", "private"), MASK);
        assert_eq!(
            preview("base_url", "http://localhost:3001"),
            "http://localhost:3001"
        );
        assert_eq!(
            variable_at("é/{{base_url}}/x", 7).as_deref(),
            Some("base_url")
        );
        assert!(variable_at("{{base_url}}/x", 12).is_none());
    }
}
