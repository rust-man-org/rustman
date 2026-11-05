//! Bridges the `rustman-engine` scripting language into the app: builds its
//! read-only `HostInput` from tab/environment state, and applies the
//! `Effect`s a script produces back onto real app state.

use rustman_engine::{Effect, HostInput, ResponseInput};

use crate::domain::environment::AppEnvironment;
use crate::domain::response::HttpResponse;
use crate::state::tabs::{RequestTabState, TestResult};

use super::AppState;

/// What running a script actually changed, for the caller to react to
/// (e.g. merge extra headers into the outgoing request, show test results).
#[derive(Default)]
pub(crate) struct AppliedEffects {
    pub extra_headers: Vec<(String, String)>,
    pub test_results: Vec<TestResult>,
    /// Set when a pre-request script called `set_body(...)` — replaces the
    /// outgoing request body for this send only (the tab's own stored body
    /// is left untouched, same as headers).
    pub body_override: Option<String>,
    /// `print(...)` calls, in order, for the Tests tab's console section.
    pub logs: Vec<String>,
}

pub(crate) fn pre_request_host_input(
    tab: &RequestTabState,
    active_env: Option<&AppEnvironment>,
) -> HostInput {
    HostInput {
        env_vars: env_pairs(active_env),
        headers: enabled_pairs(&tab.headers),
        cookies: cookie_pairs(tab),
        body: tab.body_editor.content(),
        url: tab.url.clone(),
        response: None,
    }
}

pub(crate) fn test_host_input(
    tab: &RequestTabState,
    active_env: Option<&AppEnvironment>,
    resp: &HttpResponse,
) -> HostInput {
    HostInput {
        env_vars: env_pairs(active_env),
        // Response headers, not request headers — a test script inspects
        // what came back, same convention Postman's `pm.response` uses.
        headers: resp
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        cookies: cookie_pairs(tab),
        // Unlike headers(), body()/url() always describe the request that
        // was actually sent, in both script slots — debugging "what did I
        // just send" is a normal thing to want from inside a test script.
        body: tab.body_editor.content(),
        url: tab.url.clone(),
        response: Some(ResponseInput {
            status: resp.status,
            body: resp.body.clone(),
        }),
    }
}

/// Applies a script's effects to real app state: `SetEnv` writes to the
/// active environment (and persists it, same as a manual edit would),
/// `SetHeader`, `SetBody`, and `Test` are just returned for the caller to use
/// (a pre-request send merges headers into the outgoing request and can
/// override its body; a completed response stores test results on the tab).
pub(crate) fn apply_effects(state: &mut AppState, effects: Vec<Effect>) -> AppliedEffects {
    let mut extra_headers = Vec::new();
    let mut test_results = Vec::new();
    let mut body_override = None;
    let mut logs = Vec::new();
    let mut env_changed = false;

    for effect in effects {
        match effect {
            Effect::SetHeader(name, value) => extra_headers.push((name, value)),
            Effect::SetBody(value) => body_override = Some(value),
            Effect::Test { name, passed } => test_results.push(TestResult { name, passed }),
            Effect::Log(text) => logs.push(text),
            Effect::SetEnv(name, value) => {
                if let Some(env) = state.environments.iter_mut().find(|e| e.is_active) {
                    env.variables.insert(name, value);
                    env_changed = true;
                }
            }
        }
    }

    if env_changed
        && let Some(db) = &state.db
        && let Some(env) = state.environments.iter().find(|e| e.is_active)
    {
        let _ = crate::services::storage::save_environment(db, env);
    }

    AppliedEffects {
        extra_headers,
        test_results,
        body_override,
        logs,
    }
}

/// Runs `script_text` (a no-op returning empty effects if blank) against
/// `host_input`, applying its effects immediately. Used for both the global
/// and per-request script slots, which otherwise duplicate this exact
/// parse-run-apply-or-report-error shape twice per phase (pre-request and
/// test).
pub(crate) fn run_and_apply(
    state: &mut AppState,
    script_text: &str,
    host_input: HostInput,
) -> Result<AppliedEffects, String> {
    if script_text.trim().is_empty() {
        return Ok(AppliedEffects::default());
    }
    match rustman_engine::run(script_text, host_input) {
        Ok(outcome) => Ok(apply_effects(state, outcome.effects)),
        Err(err) => Err(err.to_string()),
    }
}

fn env_pairs(active_env: Option<&AppEnvironment>) -> Vec<(String, String)> {
    active_env
        .map(|e| {
            e.variables
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn enabled_pairs(items: &[crate::domain::request::KeyValue]) -> Vec<(String, String)> {
    items
        .iter()
        .filter(|kv| kv.enabled && !kv.key.is_empty())
        .map(|kv| (kv.key.clone(), kv.value.clone()))
        .collect()
}

/// Cookies a script can see: the structured Cookies list, plus whatever's in
/// the free-text "Cookie string" field under Auth (a real browser `Cookie:`
/// header pasted there is a completely natural thing to do, and `cookie()`
/// would otherwise silently miss every name in it — the two are separate,
/// disconnected representations of "the request's cookies" in the UI).
fn cookie_pairs(tab: &RequestTabState) -> Vec<(String, String)> {
    let mut pairs = enabled_pairs(&tab.cookies);
    pairs.extend(parse_cookie_string(&tab.cookie_string));
    pairs
}

/// Parses a raw `Cookie:` header value (`name1=value1; name2=value2`) into
/// pairs. Best-effort: skips any segment that isn't `name=value`.
fn parse_cookie_string(s: &str) -> Vec<(String, String)> {
    s.split(';')
        .filter_map(|part| {
            let (name, value) = part.split_once('=')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some((name.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::request::KeyValue;
    use std::collections::HashMap;

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
        tab.reset_body_editor(r#"{"qty": 1}"#);
        let resp = HttpResponse {
            status: 201,
            status_text: "Created".into(),
            headers: HashMap::new(),
            body: "{}".into(),
            body_size: 2,
            body_stored: true,
            is_binary: false,
            binary_data: None,
            duration_ms: 5,
            error: None,
            ..Default::default()
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
            input.env_vars,
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
    fn pre_request_input_carries_the_current_request_body() {
        let mut tab = RequestTabState::new();
        tab.reset_body_editor(r#"{"amount": 42}"#);

        let input = pre_request_host_input(&tab, None);

        assert_eq!(input.body, r#"{"amount": 42}"#);
    }

    #[test]
    fn test_input_carries_the_request_body_distinct_from_the_response_body() {
        // body() in a test script means "what did I send" (for debugging),
        // which is different from response.text()/response.json() ("what
        // came back") — the two must not be conflated.
        let mut tab = RequestTabState::new();
        tab.reset_body_editor(r#"{"amount": 42}"#);
        let resp = HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: HashMap::new(),
            body: "response body".into(),
            body_size: 13,
            body_stored: true,
            is_binary: false,
            binary_data: None,
            duration_ms: 5,
            error: None,
            ..Default::default()
        };

        let input = test_host_input(&tab, None, &resp);

        assert_eq!(input.body, r#"{"amount": 42}"#);
        assert_eq!(input.response.unwrap().body, "response body");
    }

    #[test]
    fn pre_request_input_with_no_active_environment_has_empty_env_vars() {
        let tab = RequestTabState::new();
        let input = pre_request_host_input(&tab, None);
        assert!(input.env_vars.is_empty());
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
            body_stored: true,
            is_binary: false,
            binary_data: None,
            duration_ms: 5,
            error: None,
            ..Default::default()
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
