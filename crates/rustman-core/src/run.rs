//! Sending a resolved request through a transport, and what comes back.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::error::Error;
use std::fmt;
use std::string::FromUtf8Error;
use std::time::{Duration, Instant};

use crate::domain::environment::AppEnvironment;
use crate::domain::headers::Headers;
use crate::domain::model::{Request, Scripts};
use crate::domain::resolved::ResolvedRequest;
use crate::domain::response::{HttpResponse, Redirect};
use crate::script::{ScriptHost, ScriptsOutcome, prepare, run_tests};

/// How a request is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendOptions {
    /// For the whole exchange, from connecting to the body's last byte.
    pub timeout: Duration,
    /// Bodies are read up to this many decoded bytes. Reading stops there
    /// rather than buffering a body of any size: a small gzip bomb
    /// decompresses to gigabytes.
    pub body_limit: usize,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            body_limit: 10_000_000,
        }
    }
}

/// A response as it was received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// The status code's standard reason phrase; empty for unknown codes.
    pub reason: String,
    /// The headers on the wire, `Content-Encoding` and `Content-Length`
    /// included even though `body` is decoded.
    pub headers: Headers,
    /// The decoded body, at most [`SendOptions::body_limit`] bytes of it.
    pub body: Vec<u8>,
    /// The body went past the limit, so `body` holds only its start.
    pub truncated: bool,
    pub size: BodySize,
    /// The redirects followed to get here, in order.
    pub redirects: Vec<Redirect>,
}

/// Body bytes read, as sent and once decoded. A truncated body counts only
/// what was read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BodySize {
    pub wire: usize,
    pub decoded: usize,
}

/// Why no response came back, worded for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportError(String);

impl TransportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for TransportError {}

/// Something that sends a resolved request and reads back its response.
pub trait HttpTransport {
    fn send(
        &self,
        request: ResolvedRequest<'_>,
        options: &SendOptions,
    ) -> impl Future<Output = Result<Response, TransportError>> + Send;
}

/// A monotonic clock.
pub trait Clock {
    /// Time since a fixed origin; only differences between readings mean
    /// anything.
    fn now(&self) -> Duration;
}

/// The system's monotonic clock.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// A request that was sent: what came back, and how long it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution {
    pub duration: Duration,
    pub outcome: Result<Response, TransportError>,
}

/// Sends requests through a transport with fixed options, timing each.
#[derive(Debug, Clone)]
pub struct Runner<T, C = SystemClock> {
    transport: T,
    clock: C,
    options: SendOptions,
}

impl<T: HttpTransport> Runner<T> {
    pub fn new(transport: T, options: SendOptions) -> Self {
        Self::with_clock(transport, SystemClock::default(), options)
    }
}

impl<T: HttpTransport, C: Clock> Runner<T, C> {
    pub fn with_clock(transport: T, clock: C, options: SendOptions) -> Self {
        Self {
            transport,
            clock,
            options,
        }
    }

    pub async fn execute(&self, request: ResolvedRequest<'_>) -> Execution {
        let start = self.clock.now();
        let outcome = self.transport.send(request, &self.options).await;
        Execution {
            duration: self.clock.now().saturating_sub(start),
            outcome,
        }
    }

    /// Runs `request` the whole way: the pre-request scripts (`global`'s,
    /// then the request's), the send with what they changed and the
    /// environment as they left it, then the test scripts against the
    /// response, a failed one included.
    pub async fn run(
        &self,
        host: &impl ScriptHost,
        global: &Scripts,
        mut request: Request,
        mut env: Option<&mut AppEnvironment>,
    ) -> Run {
        let pre_request = prepare(host, &global.pre_request, &mut request, env.as_deref_mut());
        if pre_request.error.is_some() {
            return Run {
                request,
                pre_request,
                response: None,
                tests: ScriptsOutcome::default(),
            };
        }
        let response = HttpResponse::from(self.execute(request.resolve(env.as_deref())).await);
        let tests = run_tests(host, &global.test, &request, &response, env);
        Run {
            request,
            pre_request,
            response: Some(response),
            tests,
        }
    }
}

/// A request run with its scripts.
#[derive(Debug, Clone)]
pub struct Run {
    /// The request as sent, with the pre-request scripts' changes.
    pub request: Request,
    pub pre_request: ScriptsOutcome,
    /// `None` when a pre-request script failed, so nothing was sent.
    pub response: Option<HttpResponse>,
    pub tests: ScriptsOutcome,
}

impl From<Execution> for HttpResponse {
    /// A failure carries only its `error`. A body that isn't UTF-8 is kept as
    /// bytes, unless it was truncated: a partial binary can't be previewed.
    fn from(execution: Execution) -> Self {
        let response = match execution.outcome {
            Ok(response) => response,
            Err(TransportError(message)) => {
                return Self {
                    error: Some(message),
                    ..Self::default()
                };
            }
        };
        let mut out = Self {
            status: response.status,
            status_text: response.reason,
            headers: joined(response.headers),
            body_size: response.size.decoded,
            wire_size: Some(u64::try_from(response.size.wire).unwrap_or(u64::MAX)),
            body_stored: response.truncated,
            duration_ms: u64::try_from(execution.duration.as_millis()).unwrap_or(u64::MAX),
            redirects: response.redirects,
            ..Self::default()
        };
        match decode_text(response.body, response.truncated) {
            Ok(text) => out.body = text,
            Err(bytes) => {
                out.is_binary = true;
                out.binary_data = (!response.truncated).then_some(bytes);
            }
        }
        out
    }
}

/// Header values by name, repeats joined with `\n`.
fn joined(headers: Headers) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::with_capacity(headers.len());
    for (name, value) in headers {
        match out.entry(name) {
            Entry::Occupied(mut joined) => {
                let joined = joined.get_mut();
                joined.push('\n');
                joined.push_str(&value);
            }
            Entry::Vacant(slot) => {
                slot.insert(value);
            }
        }
    }
    out
}

/// The body as text, or its bytes when it isn't UTF-8. A truncated body may
/// end mid-character; that partial character is dropped rather than making
/// the whole body binary.
fn decode_text(body: Vec<u8>, truncated: bool) -> Result<String, Vec<u8>> {
    match String::from_utf8(body) {
        Ok(text) => Ok(text),
        Err(e) if truncated && e.utf8_error().error_len().is_none() => {
            let valid = e.utf8_error().valid_up_to();
            let mut bytes = e.into_bytes();
            bytes.truncate(valid);
            String::from_utf8(bytes).map_err(FromUtf8Error::into_bytes)
        }
        Err(e) => Err(e.into_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::future::ready;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::*;
    use crate::domain::collection::SavedRequest;
    use crate::script::tests::{FakeHost, env, set_env, set_header};
    use crate::script::{Effect, TestResult};

    /// Drives a future that never waits on I/O.
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
                return out;
            }
        }
    }

    /// Answers every request with `reply`, remembering the URLs it was sent.
    struct Fake {
        reply: Result<Response, TransportError>,
        sent: RefCell<Vec<String>>,
    }

    impl HttpTransport for Fake {
        fn send(
            &self,
            request: ResolvedRequest<'_>,
            _: &SendOptions,
        ) -> impl Future<Output = Result<Response, TransportError>> + Send {
            self.sent.borrow_mut().push(request.full_url());
            ready(self.reply.clone())
        }
    }

    /// Moves on by `step` each time it is read.
    struct Ticking {
        at: Cell<Duration>,
        step: Duration,
    }

    impl Clock for Ticking {
        fn now(&self) -> Duration {
            let now = self.at.get();
            self.at.set(now + self.step);
            now
        }
    }

    fn response(body: &[u8], truncated: bool) -> Response {
        Response {
            status: 200,
            reason: "OK".to_owned(),
            headers: [
                ("content-type", "text/plain"),
                ("set-cookie", "a=1"),
                ("set-cookie", "b=2"),
            ]
            .into_iter()
            .collect(),
            body: body.to_vec(),
            truncated,
            size: BodySize {
                wire: 3,
                decoded: body.len(),
            },
            redirects: Vec::new(),
        }
    }

    fn converted(body: &[u8], truncated: bool) -> HttpResponse {
        HttpResponse::from(Execution {
            duration: Duration::from_millis(12),
            outcome: Ok(response(body, truncated)),
        })
    }

    #[test]
    fn a_run_sends_the_resolved_request_and_times_it() {
        let runner = Runner::with_clock(
            Fake {
                reply: Ok(response(b"hi", false)),
                sent: RefCell::default(),
            },
            Ticking {
                at: Cell::new(Duration::from_secs(5)),
                step: Duration::from_millis(250),
            },
            SendOptions::default(),
        );
        let mut request = SavedRequest::new_in("c".into(), "r".into());
        request.url = "x.io/a".into();

        let execution = block_on(runner.execute(request.resolve(None)));

        assert_eq!(execution.duration, Duration::from_millis(250));
        assert_eq!(execution.outcome, Ok(response(b"hi", false)));
        assert_eq!(*runner.transport.sent.borrow(), ["https://x.io/a"]);
    }

    #[test]
    fn a_response_converts_with_its_headers_joined() {
        let out = converted(b"hello", false);
        assert_eq!((out.status, out.status_text.as_str()), (200, "OK"));
        assert_eq!(out.body, "hello");
        assert_eq!((out.body_size, out.wire_size), (5, Some(3)));
        assert_eq!(out.duration_ms, 12);
        assert_eq!(out.headers["set-cookie"], "a=1\nb=2");
        assert_eq!(out.headers["content-type"], "text/plain");
        assert!(!out.body_stored && !out.is_binary);
    }

    #[test]
    fn a_cut_inside_a_character_keeps_the_body_text() {
        let out = converted(&"€€".as_bytes()[..5], true);
        assert!(!out.is_binary && out.body_stored);
        assert_eq!(out.body, "€");
    }

    #[test]
    fn binary_bodies_keep_their_bytes_unless_cut() {
        let out = converted(&[0xff, 0x00, 0x01], false);
        assert!(out.is_binary);
        assert_eq!(out.binary_data.as_deref(), Some(&[0xff, 0x00, 0x01][..]));

        let cut = converted(&[0xff; 8], true);
        assert!(cut.is_binary && cut.body_stored);
        assert_eq!(cut.binary_data, None);
    }

    #[test]
    fn a_failure_carries_only_its_message() {
        let out = HttpResponse::from(Execution {
            duration: Duration::from_millis(40),
            outcome: Err(TransportError::new("Connection failed: refused")),
        });
        assert_eq!(out.error.as_deref(), Some("Connection failed: refused"));
        assert_eq!((out.status, out.duration_ms), (0, 0));
    }

    fn scripted_runner() -> Runner<Fake, Ticking> {
        Runner::with_clock(
            Fake {
                reply: Ok(response(b"hi", false)),
                sent: RefCell::default(),
            },
            Ticking {
                at: Cell::new(Duration::ZERO),
                step: Duration::from_millis(1),
            },
            SendOptions::default(),
        )
    }

    #[test]
    fn a_run_sends_what_the_pre_request_scripts_made_and_tests_the_response() {
        let runner = scripted_runner();
        let host = FakeHost {
            effects: vec![
                (
                    "global",
                    vec![set_env("token", "new"), set_header("X-A", "1")],
                ),
                (
                    "own test",
                    vec![Effect::Test {
                        name: "status".into(),
                        passed: true,
                    }],
                ),
            ],
            ..FakeHost::default()
        };
        let mut request: Request = SavedRequest::new_in("c".into(), "r".into()).into();
        request.url = "x.io/{{token}}".into();
        request.scripts.test = "own test".into();
        let global = Scripts {
            pre_request: "global".into(),
            test: String::new(),
        };
        let mut env = env();

        let run = block_on(runner.run(&host, &global, request, Some(&mut env)));

        assert_eq!(*runner.transport.sent.borrow(), ["https://x.io/new"]);
        assert_eq!(run.response.map(|r| r.status), Some(200));
        assert_eq!(
            run.tests.tests,
            [TestResult {
                name: "status".into(),
                passed: true
            }]
        );
        assert!(run.pre_request.env_changed);
        assert_eq!(env.variables["token"], "new");
    }

    #[test]
    fn a_failed_pre_request_script_sends_nothing() {
        let runner = scripted_runner();
        let mut request: Request = SavedRequest::new_in("c".into(), "r".into()).into();
        request.scripts.pre_request = "fail".into();
        request.scripts.test = "never".into();

        let run = block_on(runner.run(&FakeHost::default(), &Scripts::default(), request, None));

        assert!(runner.transport.sent.borrow().is_empty());
        assert!(run.response.is_none());
        assert_eq!(
            run.pre_request.error.as_deref(),
            Some("Pre-request script error: line 1: boom")
        );
        assert_eq!(run.tests, ScriptsOutcome::default());
    }
}
