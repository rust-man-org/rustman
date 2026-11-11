//! The system `git`, for everything that talks to a remote. It runs without
//! a terminal: prompts fail instead of waiting for input nobody can type,
//! and a stalled transfer is killed after [`TIMEOUT`].

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use git2::Repository;

use super::error::GitError;
use super::open;

/// Generous: a first clone of a big repository can take minutes, and the
/// transfer limits below catch a dead connection much sooner.
const TIMEOUT: Duration = Duration::from_secs(10 * 60);
const POLL: Duration = Duration::from_millis(50);

pub(super) fn origin_url(repo: &Repository) -> Option<String> {
    repo.find_remote("origin")
        .ok()
        .and_then(|r| r.url().ok().map(str::to_owned))
}

/// The `origin` remote's URL.
pub fn remote_url(data_dir: &Path) -> Option<String> {
    origin_url(&open(data_dir).ok()?)
}

/// Points `origin` at `url`, adding it if missing. Nothing is contacted.
pub fn set_remote(data_dir: &Path, url: &str) -> Result<(), GitError> {
    let repo = open(data_dir)?;
    if repo.find_remote("origin").is_ok() {
        repo.remote_set_url("origin", url)?;
    } else {
        repo.remote("origin", url)?;
    }
    Ok(())
}

/// Clones `url` into `dest`, which must not exist yet or be empty.
pub fn clone_repo(url: &str, dest: &Path) -> Result<(), GitError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut command = git();
    command.arg("clone").arg("--").arg(url).arg(dest);
    run(command).map(drop)
}

/// `git fetch --all`.
pub fn fetch(data_dir: &Path) -> Result<(), GitError> {
    run_in(data_dir, &["fetch", "--all"]).map(drop)
}

/// `git push -u origin HEAD`.
pub fn push(data_dir: &Path) -> Result<(), GitError> {
    run_in(data_dir, &["push", "-u", "origin", "HEAD"]).map(drop)
}

/// `git pull --ff-only`, returning git's summary or "Up to date".
pub fn pull(data_dir: &Path) -> Result<String, GitError> {
    let output = run_in(data_dir, &["pull", "--ff-only"])?;
    let summary = output.trim();
    Ok(if summary.is_empty() {
        "Up to date".to_owned()
    } else {
        summary.to_owned()
    })
}

/// `git <args>` in the existing repository at `repo`, returning its stdout.
fn run_in(repo: &Path, args: &[&str]) -> Result<String, GitError> {
    open(repo)?;
    let mut command = git();
    command.arg("-C").arg(repo).args(args);
    run(command)
}

/// A `git` command that can't stop to ask for anything.
fn git() -> Command {
    let mut command = Command::new("git");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_HTTP_LOW_SPEED_LIMIT", "1000")
        .env("GIT_HTTP_LOW_SPEED_TIME", "60");
    if uses_default_ssh() {
        command.env(
            "GIT_SSH_COMMAND",
            "ssh -o BatchMode=yes -o ConnectTimeout=30 -o ServerAliveInterval=15",
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Whether the user left git's ssh command alone; a configured one is kept.
fn uses_default_ssh() -> bool {
    let configured = git2::Config::open_default()
        .ok()
        .is_some_and(|config| config.get_string("core.sshCommand").is_ok());
    !configured
        && std::env::var_os("GIT_SSH_COMMAND").is_none()
        && std::env::var_os("GIT_SSH").is_none()
}

/// Runs `command` to completion within [`TIMEOUT`], returning its stdout.
fn run(command: Command) -> Result<String, GitError> {
    let finished = execute(command)?;
    match finished.status {
        Some(status) if status.success() => Ok(finished.stdout),
        Some(status) if finished.stderr.trim().is_empty() => {
            Err(GitError::Command(format!("git failed ({status})")))
        }
        Some(_) => Err(GitError::Command(redact_credentials(
            finished.stderr.trim(),
        ))),
        None => Err(GitError::Command(format!(
            "git took longer than {} minutes and was stopped",
            TIMEOUT.as_secs() / 60
        ))),
    }
}

/// `git config --get <key>` as it resolves in `repo`, or outside any
/// repository; `None` when the key is unset. Asking git itself, rather than
/// reading the files, applies every rule it would, conditional includes
/// included.
pub(super) fn config_value(repo: Option<&Path>, key: &str) -> Result<Option<String>, GitError> {
    let Some(repo) = repo else {
        // The user's config wins over the machine's. Naming a file turns
        // includes off unless asked for.
        return match config_get(&["--global", "--includes"], key)? {
            Some(value) => Ok(Some(value)),
            None => config_get(&["--system", "--includes"], key),
        };
    };
    let mut command = git();
    command.arg("-C").arg(repo).args(["config", "--get", key]);
    config_output(command)
}

/// `git config <scope…> --get <key>`.
fn config_get(scope: &[&str], key: &str) -> Result<Option<String>, GitError> {
    let mut command = git();
    command.arg("config").args(scope).args(["--get", key]);
    config_output(command)
}

fn config_output(command: Command) -> Result<Option<String>, GitError> {
    let finished = execute(command)?;
    match finished.status {
        Some(status) if status.success() => {
            Ok(Some(finished.stdout.trim().to_owned()).filter(|v| !v.is_empty()))
        }
        Some(status) if status.code() == Some(1) => Ok(None),
        _ => Err(GitError::Command(finished.stderr.trim().to_owned())),
    }
}

struct Finished {
    /// `None` when the command ran past [`TIMEOUT`] and was killed.
    status: Option<std::process::ExitStatus>,
    stdout: String,
    stderr: String,
}

fn execute(mut command: Command) -> Result<Finished, GitError> {
    let mut child = command.spawn().map_err(GitError::Spawn)?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = wait(&mut child)?;
    Ok(Finished {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// The exit status, or `None` once [`TIMEOUT`] passed and the child was killed.
fn wait(child: &mut Child) -> Result<Option<std::process::ExitStatus>, GitError> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        std::thread::sleep(POLL);
    }
}

/// Reads a pipe to its end on its own thread, so a chatty child can't fill
/// one pipe and block while the other is waited on.
fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe
            && let Err(e) = pipe.read_to_end(&mut bytes)
        {
            eprintln!("rustman: reading git's output failed: {e}");
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// `text` with the user info of every `scheme://user:token@host` URL
/// masked, so an error can be shown without leaking a token. A token may be
/// the user name alone (`https://TOKEN@host`), so all of it goes.
fn redact_credentials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let authority_end = tail
            .find(|c: char| c == '/' || c.is_whitespace() || c == '\'' || c == '"')
            .unwrap_or(tail.len());
        match tail[..authority_end].rfind('@') {
            Some(userinfo_end) => {
                out.push_str("***");
                rest = &tail[userinfo_end..];
            }
            None => rest = tail,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_in_urls_are_redacted() {
        assert_eq!(
            redact_credentials(
                "fatal: Authentication failed for 'https://ada:ghp_s3cret@github.com/team/api.git/'"
            ),
            "fatal: Authentication failed for 'https://***@github.com/team/api.git/'"
        );
        assert_eq!(
            redact_credentials("https://tok3n@host/a and http://u:p@h"),
            "https://***@host/a and http://***@h"
        );
        let plain = "fatal: repository 'https://github.com/team/api.git/' not found";
        assert_eq!(redact_credentials(plain), plain);
        assert_eq!(
            redact_credentials("git@github.com: denied"),
            "git@github.com: denied"
        );
    }

    #[test]
    fn git_never_waits_for_a_terminal() {
        use std::ffi::OsStr;
        let command = git();
        let prompt = command
            .get_envs()
            .find(|(key, _)| *key == OsStr::new("GIT_TERMINAL_PROMPT"));
        assert_eq!(
            prompt,
            Some((OsStr::new("GIT_TERMINAL_PROMPT"), Some(OsStr::new("0"))))
        );
    }

    #[test]
    fn an_option_shaped_url_is_not_an_option() {
        let dir = rustman_testkit::TempDir::new("clone-option");
        let marker = dir.path().join("pwned");
        let url = format!("--upload-pack=touch {}", marker.display());
        let result = clone_repo(&url, &dir.path().join("dest"));
        assert!(result.is_err());
        assert!(!marker.exists(), "the URL was passed as an option");
    }
}
