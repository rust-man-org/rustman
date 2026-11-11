use std::path::PathBuf;
use std::time::Duration;

pub(crate) const USAGE: &str = "\
Usage: rustman-cli run <SOURCE> [OPTIONS]

Sends every request in a collection, in the order the app lists them, and
prints one line per request.

SOURCE is a collection file (Rustman or Postman), or the name or id of a
collection saved in the app.

Options:
  --request <NAME>   Send only the requests with this name or id
  --env <NAME>       Expand {{variables}} from this saved environment (name or id)
  --var <KEY=VALUE>  Set a variable, over --env; repeatable
  --timeout <MS>     Timeout per request, in milliseconds [default: 30000]
  --insecure         Accept invalid TLS certificates
  --no-scripts       Don't run pre-request and test scripts
  --json             Print the results as JSON
  --data-dir <DIR>   Where the app keeps its data [default: the app's]
  -h, --help         Print this help
  -V, --version      Print the version

Variables that scripts set carry over to the requests after them. A request
fails when no response came back, its status is 400 or more, or a test
failed. Exit status: 0 when no request failed, 1 when one did, 2 when
nothing could be run.";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Run(RunOptions),
    Help,
    Version,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RunOptions {
    pub(crate) source: String,
    pub(crate) request: Option<String>,
    pub(crate) env: Option<String>,
    pub(crate) vars: Vec<(String, String)>,
    pub(crate) timeout: Duration,
    pub(crate) insecure: bool,
    pub(crate) scripts: bool,
    pub(crate) json: bool,
    pub(crate) data_dir: Option<PathBuf>,
}

pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("run") => parse_run(args).map(Command::Run),
        Some("-h" | "--help" | "help") => Ok(Command::Help),
        Some("-V" | "--version") => Ok(Command::Version),
        Some(other) => Err(format!("unknown command '{other}'")),
        None => Err("missing command".to_owned()),
    }
}

fn parse_run(mut args: impl Iterator<Item = String>) -> Result<RunOptions, String> {
    let mut source = None;
    let mut options = RunOptions {
        source: String::new(),
        request: None,
        env: None,
        vars: Vec::new(),
        timeout: Duration::from_secs(30),
        insecure: false,
        scripts: true,
        json: false,
        data_dir: None,
    };
    while let Some(arg) = args.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => {
                (flag.to_owned(), Some(value.to_owned()))
            }
            _ => (arg, None),
        };
        let mut value = || {
            inline
                .clone()
                .or_else(|| args.next())
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--request" => options.request = Some(value()?),
            "--env" => options.env = Some(value()?),
            "--var" => {
                let var = value()?;
                let (key, value) = var
                    .split_once('=')
                    .ok_or_else(|| format!("--var takes KEY=VALUE, not '{var}'"))?;
                options.vars.push((key.to_owned(), value.to_owned()));
            }
            "--timeout" => {
                let ms = value()?;
                let ms: u64 = ms
                    .parse()
                    .map_err(|_| format!("--timeout takes milliseconds, not '{ms}'"))?;
                options.timeout = Duration::from_millis(ms);
            }
            "--data-dir" => options.data_dir = Some(PathBuf::from(value()?)),
            "--insecure" => options.insecure = true,
            "--no-scripts" => options.scripts = false,
            "--json" => options.json = true,
            flag if flag.starts_with('-') => return Err(format!("unknown option '{flag}'")),
            _ if source.is_none() => source = Some(flag),
            extra => return Err(format!("unexpected argument '{extra}'")),
        }
    }
    options.source = source.ok_or("missing SOURCE")?;
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<RunOptions, String> {
        let args = std::iter::once("run")
            .chain(args.iter().copied())
            .map(str::to_owned);
        match parse(args)? {
            Command::Run(options) => Ok(options),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_option_is_read_in_either_spelling() {
        let options = run(&[
            "api.json",
            "--env",
            "Staging",
            "--var=host=x.io",
            "--var",
            "token=a=b",
            "--timeout=1500",
            "--request",
            "List",
            "--insecure",
            "--no-scripts",
            "--json",
            "--data-dir",
            "/tmp/rm",
        ])
        .unwrap();
        assert_eq!(options.source, "api.json");
        assert_eq!(options.env.as_deref(), Some("Staging"));
        assert_eq!(
            options.vars,
            [
                ("host".into(), "x.io".into()),
                ("token".into(), "a=b".into())
            ]
        );
        assert_eq!(options.timeout, Duration::from_millis(1500));
        assert_eq!(options.request.as_deref(), Some("List"));
        assert!(options.insecure && options.json && !options.scripts);
        assert_eq!(options.data_dir, Some(PathBuf::from("/tmp/rm")));
    }

    #[test]
    fn mistakes_are_named() {
        assert_eq!(run(&[]).unwrap_err(), "missing SOURCE");
        assert_eq!(run(&["a", "b"]).unwrap_err(), "unexpected argument 'b'");
        assert_eq!(
            run(&["a", "--nope"]).unwrap_err(),
            "unknown option '--nope'"
        );
        assert_eq!(run(&["a", "--env"]).unwrap_err(), "--env needs a value");
        assert_eq!(
            run(&["a", "--var", "x"]).unwrap_err(),
            "--var takes KEY=VALUE, not 'x'"
        );
        assert_eq!(
            run(&["a", "--timeout", "soon"]).unwrap_err(),
            "--timeout takes milliseconds, not 'soon'"
        );
        assert_eq!(parse(Vec::new()).unwrap_err(), "missing command");
        assert_eq!(parse(["--help".to_owned()]).unwrap(), Command::Help);
    }
}
