//! `rustman-cli`: sends the requests of a Rustman collection from the
//! command line, for scripts and CI.

mod args;
mod report;
mod source;

use std::io::{self, Write};
use std::process::ExitCode;

use rustman_core::domain::collection::SavedRequest;
use rustman_core::domain::group::ImportedCollection;
use rustman_core::domain::model::Request;
use rustman_core::run::{Runner, SendOptions};
use rustman_core::script::{Effect, ScriptError, ScriptHost, ScriptInput};
use rustman_http::{ReqwestTransport, TlsOptions, try_build_client};
use rustman_script::Engine;

use crate::args::{Command, RunOptions, USAGE};
use crate::report::Outcome;

fn main() -> ExitCode {
    match args::parse(std::env::args().skip(1)) {
        Ok(Command::Run(options)) => match run(&options) {
            Ok(0) => ExitCode::SUCCESS,
            Ok(_) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("rustman-cli: {e}");
                ExitCode::from(2)
            }
        },
        Ok(Command::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("rustman-cli {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rustman-cli: {e}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// The scripting engine, or nothing with `--no-scripts`.
struct Scripting {
    enabled: bool,
}

impl ScriptHost for Scripting {
    fn run(&self, source: &str, input: ScriptInput) -> Result<Vec<Effect>, ScriptError> {
        if self.enabled {
            Engine.run(source, input)
        } else {
            Ok(Vec::new())
        }
    }
}

/// Sends the requests in order, reporting each as it finishes, and returns
/// how many failed.
fn run(options: &RunOptions) -> Result<usize, String> {
    let loaded = source::load(options)?;
    let requests: Vec<&SavedRequest> = loaded
        .collections
        .iter()
        .flat_map(ImportedCollection::all_requests)
        .filter(|r| {
            options
                .request
                .as_ref()
                .is_none_or(|wanted| r.id == *wanted || r.name == *wanted)
        })
        .collect();
    if requests.is_empty() {
        return Err(match &options.request {
            Some(wanted) => format!("no request is called '{wanted}'"),
            None => "the collection has no requests".to_owned(),
        });
    }

    let client = try_build_client(TlsOptions {
        accept_invalid_certs: options.insecure,
        ..TlsOptions::default()
    })?;
    let runner = Runner::new(
        ReqwestTransport::new(client),
        SendOptions {
            timeout: options.timeout,
            ..SendOptions::default()
        },
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("cannot start the async runtime: {e}"))?;

    let output_error = |e: io::Error| format!("cannot write the results: {e}");
    let host = Scripting {
        enabled: options.scripts,
    };
    let mut env = loaded.env;
    let mut out = io::stdout().lock();
    let mut outcomes = Vec::with_capacity(requests.len());
    for saved in requests {
        let request = Request::from(saved.clone());
        let run = runtime.block_on(runner.run(&host, &loaded.global, request, Some(&mut env)));
        let url = run.request.resolve(Some(&env)).full_url();
        let outcome = Outcome::new(&saved.name, url, run);
        if !options.json {
            report::line(&mut out, &outcome).map_err(output_error)?;
        }
        outcomes.push(outcome);
    }
    if options.json {
        report::json(&mut out, &outcomes)
    } else {
        report::summary(&mut out, &outcomes)
    }
    .map_err(output_error)?;
    out.flush().map_err(output_error)?;
    Ok(report::failures(&outcomes))
}
