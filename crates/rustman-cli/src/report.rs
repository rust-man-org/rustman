use std::io::{self, Write};

use rustman_core::run::Run;
use rustman_core::text::human_size;
use serde::Serialize;

/// How one request went.
#[derive(Debug, Serialize)]
pub(crate) struct Outcome {
    pub(crate) name: String,
    pub(crate) method: &'static str,
    pub(crate) url: String,
    /// `0` when no response came back.
    pub(crate) status: u16,
    pub(crate) duration_ms: u64,
    pub(crate) size: usize,
    /// Why nothing came back, or why a script didn't run to the end.
    pub(crate) error: Option<String>,
    pub(crate) tests: Vec<TestOutcome>,
    pub(crate) passed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct TestOutcome {
    pub(crate) name: String,
    pub(crate) passed: bool,
}

impl Outcome {
    /// A request fails when nothing came back, a script failed, its status
    /// is 400 or more, or one of its tests failed.
    pub(crate) fn new(name: &str, url: String, run: Run) -> Self {
        let response = run.response.unwrap_or_default();
        let error = response.error.or(run.pre_request.error).or(run.tests.error);
        let tests: Vec<TestOutcome> = run
            .tests
            .tests
            .into_iter()
            .map(|t| TestOutcome {
                name: t.name,
                passed: t.passed,
            })
            .collect();
        Self {
            name: name.to_owned(),
            method: run.request.method.as_str(),
            url,
            status: response.status,
            duration_ms: response.duration_ms,
            size: response.body_size,
            passed: error.is_none() && response.status < 400 && tests.iter().all(|t| t.passed),
            error,
            tests,
        }
    }
}

/// `STATUS  TIME  SIZE  METHOD NAME`, the error after it if any, then a line
/// per failed test.
pub(crate) fn line(out: &mut impl Write, outcome: &Outcome) -> io::Result<()> {
    let (status, time, size) = if outcome.status == 0 {
        ("ERR".to_owned(), "-".to_owned(), "-".to_owned())
    } else {
        (
            outcome.status.to_string(),
            format!("{} ms", outcome.duration_ms),
            human_size(outcome.size),
        )
    };
    write!(
        out,
        "{status:<6} {time:>8}  {size:>9}  {} {}",
        outcome.method, outcome.name
    )?;
    match &outcome.error {
        Some(error) => writeln!(out, "  {error}")?,
        None => writeln!(out)?,
    }
    for test in outcome.tests.iter().filter(|t| !t.passed) {
        writeln!(out, "{:>29}failed: {}", "", test.name)?;
    }
    Ok(())
}

pub(crate) fn summary(out: &mut impl Write, outcomes: &[Outcome]) -> io::Result<()> {
    let failed = failures(outcomes);
    let noun = if outcomes.len() == 1 {
        "request"
    } else {
        "requests"
    };
    writeln!(out, "\n{} {noun}, {failed} failed", outcomes.len())
}

pub(crate) fn json(out: &mut impl Write, outcomes: &[Outcome]) -> io::Result<()> {
    #[derive(Serialize)]
    struct Report<'a> {
        requests: &'a [Outcome],
        total: usize,
        failed: usize,
    }
    let report = Report {
        requests: outcomes,
        total: outcomes.len(),
        failed: failures(outcomes),
    };
    serde_json::to_writer_pretty(&mut *out, &report)?;
    writeln!(out)
}

pub(crate) fn failures(outcomes: &[Outcome]) -> usize {
    outcomes.iter().filter(|o| !o.passed).count()
}

#[cfg(test)]
mod tests {
    use rustman_core::domain::collection::SavedRequest;
    use rustman_core::domain::response::HttpResponse;
    use rustman_core::script::{ScriptsOutcome, TestResult};

    use super::*;

    fn outcome(status: u16, error: Option<&str>, tests: &[(&str, bool)]) -> Outcome {
        let request = SavedRequest::new_in("c".into(), "List items".into()).into();
        let run = Run {
            request,
            pre_request: ScriptsOutcome::default(),
            response: Some(HttpResponse {
                status,
                duration_ms: 12,
                body_size: 1536,
                error: error.map(Into::into),
                ..HttpResponse::default()
            }),
            tests: ScriptsOutcome {
                tests: tests
                    .iter()
                    .map(|(name, passed)| TestResult {
                        name: (*name).to_owned(),
                        passed: *passed,
                    })
                    .collect(),
                ..ScriptsOutcome::default()
            },
        };
        Outcome::new("List items", "https://x.io/items".into(), run)
    }

    fn printed(write: impl Fn(&mut Vec<u8>) -> io::Result<()>) -> String {
        let mut out = Vec::new();
        write(&mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn a_line_shows_status_time_size_name_and_failed_tests() {
        assert_eq!(
            printed(|out| line(out, &outcome(200, None, &[]))),
            "200       12 ms     1.5 KB  GET List items\n"
        );
        assert_eq!(
            printed(|out| line(out, &outcome(0, Some("Connection failed: refused"), &[]))),
            "ERR           -          -  GET List items  Connection failed: refused\n"
        );
        assert_eq!(
            printed(|out| line(out, &outcome(200, None, &[("ok", true), ("has id", false)]))),
            "200       12 ms     1.5 KB  GET List items\n                             failed: has id\n"
        );
    }

    #[test]
    fn failures_are_errors_statuses_from_400_and_failed_tests() {
        let outcomes = [
            outcome(200, None, &[("ok", true)]),
            outcome(399, None, &[]),
            outcome(400, None, &[]),
            outcome(0, Some("x"), &[]),
            outcome(200, None, &[("no", false)]),
        ];
        assert_eq!(failures(&outcomes), 3);
        assert_eq!(
            printed(|out| summary(out, &outcomes)),
            "\n5 requests, 3 failed\n"
        );
    }
}
