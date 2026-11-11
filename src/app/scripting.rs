//! Scripts in the app: the shared engine host, and saving what scripts
//! change.

pub(crate) use rustman_script::Engine;

use rustman_core::script::ScriptsOutcome;
use rustman_core::store::Change;

use super::AppState;

/// Saves environment `env_id` once a script has changed it, as a manual edit
/// would.
pub(crate) fn save_env_changes(state: &AppState, env_id: Option<&str>, outcome: &ScriptsOutcome) {
    if outcome.env_changed
        && let Some(env) = state.environments.iter().find(|e| Some(e.id.as_str()) == env_id)
    {
        state.write(vec![Change::PutEnvironment(env.clone())]);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rustman_core::script::ScriptInput;

    use crate::domain::environment::AppEnvironment;
    use crate::domain::request::KeyValue;
    use crate::domain::response::HttpResponse;
    use crate::state::tabs::RequestTabState;

    fn pre_request_host_input(tab: &RequestTabState, env: Option<&AppEnvironment>) -> ScriptInput {
        ScriptInput::pre_request(&tab.to_request(), env)
    }

    fn test_host_input(tab: &RequestTabState, env: Option<&AppEnvironment>, resp: &HttpResponse) -> ScriptInput {
        ScriptInput::test(&tab.to_request(), env, resp)
    }

    fn env(vars: &[(&str, &str)]) -> AppEnvironment {
        AppEnvironment {
            id: "env-1".into(),
            name: "Test".into(),
            variables: vars
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            is_active: true,
        }
    }

    fn kv(key: &str, value: &str, enabled: bool) -> KeyValue {
        KeyValue {
            id: uuid::Uuid::new_v4().to_string(),
            key: key.into(),
            value: value.into(),
            enabled,
        }
    }

    #[test]
    fn url_and_body_are_carried_in_both_pre_request_and_test_host_input() {
        let mut tab = RequestTabState::new();
        tab.url = "https://api.test/orders".to_owned();
        tab.body_type = crate::domain::request::BodyType::Json;
        tab.reset_body_editor(r#"{"qty": 1}"#);
        let resp = HttpResponse {
            status: 201,
            status_text: "Created".into(),
            headers: HashMap::new(),
            body: "{}".into(),
            body_size: 2,
            wire_size: None,
            body_stored: true,
            is_binary: false,
            binary_data: None,
            duration_ms: 5,
            error: None,
            redirects: Vec::new(),
        };

        let pre = pre_request_host_input(&tab, None);
        assert_eq!(pre.url, "https://api.test/orders");
        assert_eq!(pre.body, r#"{"qty": 1}"#);

        let test = test_host_input(&tab, None, &resp);
        assert_eq!(test.url, "https://api.test/orders");
        assert_eq!(test.body, r#"{"qty": 1}"#);
    }

    #[test]
    fn pre_request_input_finds_a_cookie_pasted_into_the_raw_cookie_string_field() {
        // Regression test: cookie() used to only see the structured Cookies
        // list, so pasting a real browser `Cookie:` header into the "Cookie
        // string" field under Auth (the natural thing to do with one) meant
        // any name in it was silently invisible to scripts.
        let mut tab = RequestTabState::new();
        tab.cookie_string =
            "sessionToken=16a2233f; accessToken=eyJhbGci.abc.def; planName=BFHL Health Prime"
                .to_owned();

        let input = pre_request_host_input(&tab, None);

        assert!(
            input
                .cookies
                .contains(&("accessToken".to_string(), "eyJhbGci.abc.def".to_string()))
        );
        assert!(
            input
                .cookies
                .contains(&("sessionToken".to_string(), "16a2233f".to_string()))
        );
    }

    #[test]
    fn structured_cookies_list_still_works_alongside_the_raw_cookie_string() {
        let mut tab = RequestTabState::new();
        tab.cookies = vec![kv("session_id", "abc", true)];
        tab.cookie_string = "accessToken=xyz".to_owned();

        let input = pre_request_host_input(&tab, None);

        assert!(
            input
                .cookies
                .contains(&("session_id".to_string(), "abc".to_string()))
        );
        assert!(
            input
                .cookies
                .contains(&("accessToken".to_string(), "xyz".to_string()))
        );
    }

    #[test]
    fn pre_request_input_only_includes_enabled_headers_and_cookies() {
        let mut tab = RequestTabState::new();
        tab.headers = vec![kv("Accept", "json", true), kv("X-Disabled", "no", false)];
        tab.cookies = vec![kv("session", "abc", true)];
        let e = env(&[("base_url", "https://api.test")]);

        let input = pre_request_host_input(&tab, Some(&e));

        assert_eq!(
            input.env,
            vec![("base_url".to_string(), "https://api.test".to_string())]
        );
        assert_eq!(
            input.headers,
            vec![("Accept".to_string(), "json".to_string())]
        );
        assert_eq!(
            input.cookies,
            vec![("session".to_string(), "abc".to_string())]
        );
        assert!(input.response.is_none());
    }

    #[test]
    fn test_input_carries_the_request_body_distinct_from_the_response_body() {
        // body() in a test script means "what did I send" (for debugging),
        // which is different from response.text()/response.json() ("what
        // came back") — the two must not be conflated.
        let mut tab = RequestTabState::new();
        tab.body_type = crate::domain::request::BodyType::Json;
        tab.reset_body_editor(r#"{"amount": 42}"#);
        let resp = HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: HashMap::new(),
            body: "response body".into(),
            body_size: 13,
            wire_size: None,
            body_stored: true,
            is_binary: false,
            binary_data: None,
            duration_ms: 5,
            error: None,
            redirects: Vec::new(),
        };

        let input = test_host_input(&tab, None, &resp);

        assert_eq!(input.body, r#"{"amount": 42}"#);
        assert_eq!(input.response.unwrap().body, "response body");
    }

    #[test]
    fn test_input_uses_response_headers_not_request_headers_and_carries_status_body() {
        let mut tab = RequestTabState::new();
        tab.headers = vec![kv("X-Request-Only", "req", true)];
        let mut resp_headers = HashMap::new();
        resp_headers.insert("Content-Type".to_string(), "application/json".to_string());
        let resp = HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: resp_headers,
            body: "{\"ok\":true}".into(),
            body_size: 11,
            wire_size: None,
            body_stored: true,
            is_binary: false,
            binary_data: None,
            duration_ms: 5,
            error: None,
            redirects: Vec::new(),
        };

        let input = test_host_input(&tab, None, &resp);

        assert_eq!(
            input.headers,
            vec![("Content-Type".to_string(), "application/json".to_string())]
        );
        let response = input.response.expect("response input should be set");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "{\"ok\":true}");
    }
}
