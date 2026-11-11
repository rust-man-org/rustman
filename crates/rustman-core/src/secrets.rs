//! Which saved values are credentials, so a shared file can leave them out.

use crate::domain::collection::SavedRequest;
use crate::domain::group::{ImportedCollection, ImportedGroup};
use crate::template::is_only_placeholders;

/// Whether `value` is a credential to leave out of a shared file. A value
/// that is only `{{variable}}` placeholders names where the secret is kept
/// rather than holding it, so it isn't one.
pub fn is_secret_value(value: &str) -> bool {
    !value.trim().is_empty() && !is_only_placeholders(value)
}

/// Header rows whose value is a credential, matched case-insensitively.
pub const CREDENTIAL_HEADERS: [&str; 6] = [
    "Authorization",
    "Proxy-Authorization",
    "Cookie",
    "X-Api-Key",
    "Api-Key",
    "X-Auth-Token",
];

/// Blanks every credential [`is_secret_value`] flags in `request`: the
/// bearer token, Basic password, API key value, JWT secret and Cookie auth
/// string whichever auth is active, every cookie row's value, and the value
/// of each [`CREDENTIAL_HEADERS`] row. Names, enabled flags and row order
/// stay. Params are left alone: a credential can't be told from their names.
pub fn strip_secrets(request: &mut SavedRequest) {
    let auth = [
        &mut request.bearer_token,
        &mut request.basic_pass,
        &mut request.api_key_value,
        &mut request.jwt_secret,
        &mut request.cookie_string,
    ];
    let cookies = request.cookies.iter_mut().map(|row| &mut row.value);
    let headers = request
        .headers
        .iter_mut()
        .filter(|row| {
            CREDENTIAL_HEADERS
                .iter()
                .any(|name| name.eq_ignore_ascii_case(row.key.trim()))
        })
        .map(|row| &mut row.value);
    for value in auth.into_iter().chain(cookies).chain(headers) {
        if is_secret_value(value) {
            value.clear();
        }
    }
}

/// [`strip_secrets`] for every request in `collection`, folders included.
pub fn strip_collection_secrets(collection: &mut ImportedCollection) {
    collection.requests.iter_mut().for_each(strip_secrets);
    collection.groups.iter_mut().for_each(strip_group_secrets);
}

fn strip_group_secrets(group: &mut ImportedGroup) {
    group.requests.iter_mut().for_each(strip_secrets);
    group.groups.iter_mut().for_each(strip_group_secrets);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::request::KeyValue;

    fn row(key: &str, value: &str, enabled: bool) -> KeyValue {
        KeyValue {
            id: key.into(),
            key: key.into(),
            value: value.into(),
            enabled,
        }
    }

    fn rows(rows: &[KeyValue]) -> Vec<(&str, &str, bool)> {
        rows.iter()
            .map(|r| (r.key.as_str(), r.value.as_str(), r.enabled))
            .collect()
    }

    #[test]
    fn placeholders_are_references_not_secrets() {
        assert!(is_secret_value("hunter2"));
        assert!(is_secret_value("Bearer {{token}}"));
        assert!(is_secret_value("{{user}}:{{pass}}"));
        assert!(!is_secret_value("{{token}}"));
        assert!(!is_secret_value(" {{a}}{{b}} "));
        assert!(!is_secret_value(""));
    }

    #[test]
    fn every_credential_is_blanked_and_references_are_kept() {
        let mut request = SavedRequest::new_in("c".into(), "r".into());
        request.bearer_token = "t0k".into();
        request.basic_user = "admin".into();
        request.basic_pass = "{{password}}".into();
        request.api_key_name = "X-Key".into();
        request.api_key_value = "k3y".into();
        request.jwt_secret = "signing".into();
        request.jwt_subject = "user-1".into();
        request.cookie_string = "sid=abc".into();

        strip_secrets(&mut request);

        assert_eq!(request.bearer_token, "");
        assert_eq!(request.basic_pass, "{{password}}");
        assert_eq!(request.api_key_value, "");
        assert_eq!(request.jwt_secret, "");
        assert_eq!(request.cookie_string, "");
        assert_eq!(
            (
                request.basic_user.as_str(),
                request.api_key_name.as_str(),
                request.jwt_subject.as_str()
            ),
            ("admin", "X-Key", "user-1")
        );
    }

    #[test]
    fn credential_headers_and_cookie_rows_lose_their_values_in_place() {
        let mut request = SavedRequest::new_in("c".into(), "r".into());
        request.headers = vec![
            row("authorization", "Bearer t0k", true),
            row("X-Tag", "keep-me", true),
            row("Proxy-Authorization", "Basic eHk=", false),
            row("COOKIE", "sid=1", true),
            row("x-api-key", "k3y", true),
            row("Api-Key", "{{api_key}}", true),
            row("X-Auth-Token", "tok", true),
            row("X-Api-Key-Id", "not-a-secret-name", true),
        ];
        request.cookies = vec![row("session", "abc", true), row("csrf", "{{csrf}}", false)];
        request.params = vec![row("token", "in-a-param", true)];

        strip_secrets(&mut request);

        assert_eq!(
            rows(&request.headers),
            [
                ("authorization", "", true),
                ("X-Tag", "keep-me", true),
                ("Proxy-Authorization", "", false),
                ("COOKIE", "", true),
                ("x-api-key", "", true),
                ("Api-Key", "{{api_key}}", true),
                ("X-Auth-Token", "", true),
                ("X-Api-Key-Id", "not-a-secret-name", true),
            ]
        );
        assert_eq!(
            rows(&request.cookies),
            [("session", "", true), ("csrf", "{{csrf}}", false)]
        );
        assert_eq!(rows(&request.params), [("token", "in-a-param", true)]);
    }
}
