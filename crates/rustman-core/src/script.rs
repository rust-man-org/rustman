//! Pre-request and test scripts, run through a [`ScriptHost`] whatever
//! language implements it.
//!
//! Each phase runs the global script, then the request's own. A script's
//! effects are applied when it ends: environment changes at once, so the next
//! script reads them, and everything else collected in a [`ScriptsOutcome`].

use std::error::Error;
use std::fmt;

use crate::domain::environment::AppEnvironment;
use crate::domain::model::{BodyKind, Request};
use crate::domain::request::KeyValue;
use crate::domain::response::HttpResponse;

/// What a script can read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptInput {
    pub env: Vec<(String, String)>,
    /// The request's enabled headers before it is sent; the response's after.
    pub headers: Vec<(String, String)>,
    pub cookies: Vec<(String, String)>,
    /// The request body, in both phases.
    pub body: String,
    pub url: String,
    pub response: Option<ScriptResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptResponse {
    pub status: u16,
    pub body: String,
}

/// Something a script asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    SetEnv { name: String, value: String },
    SetHeader { name: String, value: String },
    SetBody(String),
    Test { name: String, passed: bool },
    Log(String),
}

/// Why a script didn't run to the end, worded for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError(String);

impl ScriptError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ScriptError {}

/// Something that runs a script and reports what it asked for.
pub trait ScriptHost {
    fn run(&self, source: &str, input: ScriptInput) -> Result<Vec<Effect>, ScriptError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
}

/// What a phase's scripts did, together.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptsOutcome {
    /// Headers set, in order, to add to the outgoing request.
    pub headers: Vec<(String, String)>,
    /// The body last set, to send instead of the request's.
    pub body: Option<String>,
    pub tests: Vec<TestResult>,
    pub logs: Vec<String>,
    /// Whether a script changed the environment, which then needs saving.
    pub env_changed: bool,
    /// `"<script> error: <why>"` for the script that failed. The scripts
    /// after it didn't run, and its own effects were dropped.
    pub error: Option<String>,
}

impl ScriptsOutcome {
    /// Sets the headers the scripts set on `request`, and makes the body
    /// they set the one it sends.
    fn apply_to(&self, request: &mut Request) {
        for (name, value) in &self.headers {
            set_header(&mut request.headers, name, value);
        }
        if let Some(body) = &self.body {
            request.body.text.clone_from(body);
            // The text is the whole body now: a GraphQL request's whole
            // document, and a body at all for one that had none or a form.
            request.body.kind = match request.body.kind {
                BodyKind::Json | BodyKind::GraphQl => BodyKind::Json,
                BodyKind::None | BodyKind::Text | BodyKind::Form => BodyKind::Text,
            };
        }
    }
}

/// Gives the enabled header rows named `name`, in any case, the one
/// `value`: the first of them takes it, under `name`, and the others go. A
/// header the request doesn't send yet is added as a new row.
fn set_header(rows: &mut Vec<KeyValue>, name: &str, value: &str) {
    let named = |row: &KeyValue| row.enabled && row.key.eq_ignore_ascii_case(name);
    let Some(first) = rows.iter().position(named) else {
        rows.push(KeyValue {
            id: uuid::Uuid::new_v4().to_string(),
            key: name.to_owned(),
            value: value.to_owned(),
            enabled: true,
        });
        return;
    };
    let mut index = 0;
    rows.retain(|row| {
        index += 1;
        index - 1 == first || !named(row)
    });
    let row = &mut rows[first];
    name.clone_into(&mut row.key);
    value.clone_into(&mut row.value);
}

impl ScriptInput {
    /// What a pre-request script reads: the request about to be resolved,
    /// so `{{variables}}` are still unfilled, and the body it would send.
    pub fn pre_request(request: &Request, env: Option<&AppEnvironment>) -> Self {
        Self {
            env: env_pairs(env),
            headers: enabled_pairs(&request.headers),
            cookies: cookie_pairs(request),
            body: request.resolve(None).body.sent_text(),
            url: request.url.clone(),
            response: None,
        }
    }

    /// Test scripts read the response's headers, but the request's body and
    /// URL as `env` fills them in: `body()` answers "what did I send".
    pub fn test(request: &Request, env: Option<&AppEnvironment>, response: &HttpResponse) -> Self {
        Sent::of(request, env).input(request, env, response)
    }
}

/// The URL and body text a request went out with.
struct Sent {
    url: String,
    body: String,
}

impl Sent {
    fn of(request: &Request, env: Option<&AppEnvironment>) -> Self {
        let resolved = request.resolve(env);
        Self {
            url: resolved.full_url(),
            body: resolved.body.sent_text(),
        }
    }

    fn input(
        &self,
        request: &Request,
        env: Option<&AppEnvironment>,
        response: &HttpResponse,
    ) -> ScriptInput {
        ScriptInput {
            env: env_pairs(env),
            headers: response
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            cookies: cookie_pairs(request),
            body: self.body.clone(),
            url: self.url.clone(),
            response: Some(ScriptResponse {
                status: response.status,
                body: response.body.clone(),
            }),
        }
    }
}

/// Runs the global pre-request script, then `request`'s own, and unless one
/// failed applies the headers and body they set to `request`, which is then
/// ready to resolve and send.
pub fn prepare(
    host: &impl ScriptHost,
    global: &str,
    request: &mut Request,
    env: Option<&mut AppEnvironment>,
) -> ScriptsOutcome {
    let outcome = run_pre_request(host, global, request, env);
    if outcome.error.is_none() {
        outcome.apply_to(request);
    }
    outcome
}

/// The pre-request scripts in order; the second sees the headers and body
/// the first set.
fn run_pre_request(
    host: &impl ScriptHost,
    global: &str,
    request: &Request,
    env: Option<&mut AppEnvironment>,
) -> ScriptsOutcome {
    let scripts = [
        ("Global pre-request script", global),
        ("Pre-request script", request.scripts.pre_request.as_str()),
    ];
    run_scripts(host, &scripts, env, |done, env| {
        if done.headers.is_empty() && done.body.is_none() {
            return ScriptInput::pre_request(request, env);
        }
        let mut so_far = request.clone();
        done.apply_to(&mut so_far);
        ScriptInput::pre_request(&so_far, env)
    })
}

/// Runs the global test script, then `request`'s own, against `response`.
/// `request` is the one that was sent, as [`prepare`] left it.
pub fn run_tests(
    host: &impl ScriptHost,
    global: &str,
    request: &Request,
    response: &HttpResponse,
    env: Option<&mut AppEnvironment>,
) -> ScriptsOutcome {
    let scripts = [
        ("Global test script", global),
        ("Test script", request.scripts.test.as_str()),
    ];
    // As sent, before a test script changes a variable it was sent with.
    let sent = Sent::of(request, env.as_deref());
    run_scripts(host, &scripts, env, |_, env| {
        sent.input(request, env, response)
    })
}

/// Runs the non-blank `scripts` in order until one fails, each with the
/// input `input` builds from what ran before it and the environment as it
/// left it.
fn run_scripts(
    host: &impl ScriptHost,
    scripts: &[(&str, &str)],
    mut env: Option<&mut AppEnvironment>,
    mut input: impl FnMut(&ScriptsOutcome, Option<&AppEnvironment>) -> ScriptInput,
) -> ScriptsOutcome {
    let mut outcome = ScriptsOutcome::default();
    for (label, source) in scripts.iter().filter(|(_, s)| !s.trim().is_empty()) {
        let effects = match host.run(source, input(&outcome, env.as_deref())) {
            Ok(effects) => effects,
            Err(e) => {
                outcome.error = Some(format!("{label} error: {e}"));
                break;
            }
        };
        for effect in effects {
            match effect {
                Effect::SetEnv { name, value } => {
                    if let Some(env) = env.as_deref_mut() {
                        env.variables.insert(name, value);
                        outcome.env_changed = true;
                    }
                }
                Effect::SetHeader { name, value } => outcome.headers.push((name, value)),
                Effect::SetBody(body) => outcome.body = Some(body),
                Effect::Test { name, passed } => outcome.tests.push(TestResult { name, passed }),
                Effect::Log(text) => outcome.logs.push(text),
            }
        }
    }
    outcome
}

fn env_pairs(env: Option<&AppEnvironment>) -> Vec<(String, String)> {
    env.map(|e| {
        e.variables
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    })
    .unwrap_or_default()
}

fn enabled_pairs(rows: &[KeyValue]) -> Vec<(String, String)> {
    rows.iter()
        .filter(|kv| kv.enabled && !kv.key.is_empty())
        .map(|kv| (kv.key.clone(), kv.value.clone()))
        .collect()
}

/// The structured cookies, then the ones in the raw `Cookie` string under
/// Auth: a browser's `Cookie:` header is naturally pasted there, and
/// `cookie()` should find its names too.
fn cookie_pairs(request: &Request) -> Vec<(String, String)> {
    let mut pairs = enabled_pairs(&request.script_cookies);
    pairs.extend(request.auth.cookie.cookies.split(';').filter_map(|part| {
        let (name, value) = part.split_once('=')?;
        let name = name.trim();
        (!name.is_empty()).then(|| (name.to_owned(), value.trim().to_owned()))
    }));
    pairs
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::domain::collection::SavedRequest;

    /// Answers each script by name with canned effects, and records the
    /// input each one was given. A script named `fail` fails.
    #[derive(Default)]
    pub(crate) struct FakeHost {
        pub(crate) effects: Vec<(&'static str, Vec<Effect>)>,
        pub(crate) inputs: RefCell<Vec<(String, ScriptInput)>>,
    }

    impl ScriptHost for FakeHost {
        fn run(&self, source: &str, input: ScriptInput) -> Result<Vec<Effect>, ScriptError> {
            self.inputs.borrow_mut().push((source.to_owned(), input));
            if source == "fail" {
                return Err(ScriptError::new("line 1: boom"));
            }
            Ok(self
                .effects
                .iter()
                .find(|(name, _)| *name == source)
                .map(|(_, effects)| effects.clone())
                .unwrap_or_default())
        }
    }

    pub(crate) fn set_env(name: &str, value: &str) -> Effect {
        Effect::SetEnv {
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }

    pub(crate) fn set_header(name: &str, value: &str) -> Effect {
        Effect::SetHeader {
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }

    pub(crate) fn env() -> AppEnvironment {
        AppEnvironment {
            id: "e".into(),
            name: "e".into(),
            variables: [("token".to_owned(), "old".to_owned())].into(),
            is_active: true,
        }
    }

    fn request(pre_request: &str, test: &str) -> Request {
        let mut request: Request = SavedRequest::new_in("c".into(), "r".into()).into();
        request.scripts.pre_request = pre_request.to_owned();
        request.scripts.test = test.to_owned();
        request.body.kind = BodyKind::Text;
        request.body.text = "typed".to_owned();
        request
    }

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

    fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    /// `set_header` sets the header: every enabled row of that name, in any
    /// case, becomes the one value, where the first of them was.
    #[test]
    fn set_header_replaces_the_rows_of_that_name_in_place() {
        let host = FakeHost {
            effects: vec![
                ("global", vec![set_header("x-token", "new")]),
                ("own", vec![set_header("X-Added", "1")]),
            ],
            ..FakeHost::default()
        };
        let mut sent = request("own", "");
        sent.headers = vec![
            row("Accept", "*/*", true),
            row("X-Token", "old", true),
            row("X-TOKEN", "older", true),
            row("X-Token", "off", false),
        ];

        assert_eq!(prepare(&host, "global", &mut sent, None).error, None);

        assert_eq!(
            host.inputs.borrow()[1].1.headers,
            pairs(&[("Accept", "*/*"), ("x-token", "new")]),
            "the next script reads the value set"
        );
        assert_eq!(
            rows(&sent.headers),
            [
                ("Accept", "*/*", true),
                ("x-token", "new", true),
                ("X-Token", "off", false),
                ("X-Added", "1", true)
            ]
        );
    }

    /// The body a script sets is the one sent, whatever the request had;
    /// `body()` reads the body that goes out.
    #[test]
    fn set_body_sends_that_body_and_body_reads_what_is_sent() {
        use crate::domain::model::Body;
        let host = FakeHost {
            effects: vec![("global", vec![Effect::SetBody("{\"a\": 1}".into())])],
            ..FakeHost::default()
        };
        for kind in [BodyKind::None, BodyKind::Form, BodyKind::Text] {
            let mut sent = request("own", "");
            sent.body.kind = kind;
            prepare(&host, "global", &mut sent, None);
            assert_eq!(sent.body.effective(), Body::Text("{\"a\": 1}"), "{kind:?}");
        }
        let mut graphql = request("own", "");
        graphql.body.kind = BodyKind::GraphQl;
        graphql.body.text = "{ me }".into();
        assert_eq!(
            ScriptInput::pre_request(&graphql, None).body,
            r#"{"query":"{ me }"}"#
        );
        prepare(&host, "global", &mut graphql, None);
        assert_eq!(
            graphql.body.effective(),
            Body::Json("{\"a\": 1}"),
            "the whole document"
        );

        let mut none = request("", "");
        none.body.kind = BodyKind::None;
        assert_eq!(
            ScriptInput::pre_request(&none, None).body,
            "",
            "nothing is sent"
        );
    }

    /// In a test script, `body()` and `url()` are what went out, with the
    /// `{{variables}}` filled in as they were sent.
    #[test]
    fn test_scripts_read_the_sent_body_and_url_with_variables_filled_in() {
        let host = FakeHost {
            effects: vec![("global", vec![set_env("token", "changed")])],
            ..FakeHost::default()
        };
        let mut sent = request("", "own");
        sent.url = "{{host}}/items?t={{token}}".into();
        sent.params = vec![row("t", "{{token}}", true)];
        sent.body.kind = BodyKind::Json;
        sent.body.text = "{\"token\": \"{{token}}\"}".into();
        let mut env = env();
        env.variables
            .insert("host".into(), "api.example.com".into());

        run_tests(
            &host,
            "global",
            &sent,
            &HttpResponse::default(),
            Some(&mut env),
        );

        let inputs = host.inputs.borrow();
        let own = &inputs[1].1;
        assert_eq!(own.url, "https://api.example.com/items?t=old");
        assert_eq!(
            own.body, "{\"token\": \"old\"}",
            "as sent, before the global test script's change"
        );
    }

    #[test]
    fn the_request_script_sees_what_the_global_one_set() {
        let host = FakeHost {
            effects: vec![(
                "global",
                vec![
                    set_env("token", "new"),
                    set_header("X-A", "1"),
                    Effect::SetBody("from global".into()),
                    Effect::Log("hi".into()),
                ],
            )],
            ..FakeHost::default()
        };
        let mut env = env();

        let outcome = run_pre_request(&host, "global", &request("own", ""), Some(&mut env));

        let inputs = host.inputs.borrow();
        let (_, own) = &inputs[1];
        assert_eq!(own.env, [("token".to_owned(), "new".to_owned())]);
        assert_eq!(own.headers, [("X-A".to_owned(), "1".to_owned())]);
        assert_eq!(own.body, "from global");
        assert_eq!(
            inputs[0].1.body, "typed",
            "the global script saw the typed body"
        );
        assert!(outcome.env_changed && outcome.error.is_none());
        assert_eq!(outcome.logs, ["hi"]);
        assert_eq!(env.variables["token"], "new");
    }

    #[test]
    fn a_failing_script_stops_the_phase_and_names_itself() {
        let host = FakeHost {
            effects: vec![("global", vec![set_env("token", "new")])],
            ..FakeHost::default()
        };
        let mut env = env();

        let outcome = run_pre_request(&host, "global", &request("fail", ""), Some(&mut env));
        assert_eq!(
            outcome.error.as_deref(),
            Some("Pre-request script error: line 1: boom")
        );
        assert!(outcome.env_changed, "the global script's change stands");

        let outcome = run_tests(
            &host,
            "fail",
            &request("", "never"),
            &HttpResponse::default(),
            None,
        );
        assert_eq!(
            outcome.error.as_deref(),
            Some("Global test script error: line 1: boom")
        );
        assert_eq!(
            host.inputs.borrow().len(),
            3,
            "the request's test script didn't run"
        );
    }

    #[test]
    fn blank_scripts_do_not_run_and_no_environment_takes_no_changes() {
        let host = FakeHost {
            effects: vec![("own", vec![set_env("token", "new")])],
            ..FakeHost::default()
        };
        let outcome = run_pre_request(&host, "  \n", &request("own", ""), None);
        assert_eq!(host.inputs.borrow().len(), 1);
        assert!(!outcome.env_changed);
    }

    #[test]
    fn test_scripts_read_the_response_and_the_request_body() {
        let host = FakeHost {
            effects: vec![(
                "own",
                vec![Effect::Test {
                    name: "ok".into(),
                    passed: true,
                }],
            )],
            ..FakeHost::default()
        };
        let mut response = HttpResponse {
            status: 201,
            body: "made".into(),
            ..HttpResponse::default()
        };
        response.headers.insert("x-id".into(), "7".into());

        let outcome = run_tests(&host, "", &request("", "own"), &response, None);

        let input = &host.inputs.borrow()[0].1;
        assert_eq!(input.headers, [("x-id".to_owned(), "7".to_owned())]);
        assert_eq!(input.body, "typed");
        assert_eq!(
            input.response,
            Some(ScriptResponse {
                status: 201,
                body: "made".into()
            })
        );
        assert_eq!(
            outcome.tests,
            [TestResult {
                name: "ok".into(),
                passed: true
            }]
        );
    }

    #[test]
    fn cookies_come_from_the_list_and_the_raw_cookie_string() {
        let mut request = request("", "");
        request.script_cookies = vec![KeyValue {
            id: "1".into(),
            key: "session".into(),
            value: "abc".into(),
            enabled: true,
        }];
        request.auth.cookie.cookies = "a=1; =skip; b = 2 ;junk".into();
        assert_eq!(
            ScriptInput::pre_request(&request, None).cookies,
            [
                ("session".to_owned(), "abc".to_owned()),
                ("a".to_owned(), "1".to_owned()),
                ("b".to_owned(), "2".to_owned())
            ]
        );
    }

    #[test]
    fn a_failed_phase_leaves_the_request_as_it_was() {
        let host = FakeHost {
            effects: vec![("global", vec![set_header("X-A", "1")])],
            ..FakeHost::default()
        };
        let mut sent = request("own", "");
        assert!(prepare(&host, "global", &mut sent, None).error.is_none());
        assert_eq!(sent.headers.len(), 1);

        let mut failed = request("fail", "");
        assert!(prepare(&host, "global", &mut failed, None).error.is_some());
        assert!(failed.headers.is_empty());
    }

    #[test]
    fn script_changes_apply_to_the_outgoing_request() {
        let outcome = ScriptsOutcome {
            headers: vec![("X-A".into(), "1".into())],
            body: Some("swapped".into()),
            ..ScriptsOutcome::default()
        };
        let mut request = request("", "");
        outcome.apply_to(&mut request);
        assert_eq!(request.body.text, "swapped");
        assert_eq!(
            (request.headers[0].key.as_str(), request.headers[0].enabled),
            ("X-A", true)
        );
    }
}
