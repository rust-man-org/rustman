//! The Git panel's working state: the question it is waiting on, what the
//! last refresh had to leave out, and the queue of background jobs.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::services::repos;
use crate::services::vcs::GitIdentity;

#[derive(Debug, Default)]
pub struct GitPanel {
    pub prompt: Option<GitPrompt>,
    /// What the last refresh had to leave out of the working tree.
    pub warnings: Vec<String>,
    /// Tabs with unsaved edits to a request that git changed underneath
    /// them, until the user reloads or keeps their edits.
    pub changed_upstream: Vec<String>,
    /// Collections kept out of every repository, Local included.
    pub local_only: Vec<String>,
    /// Who plain `git commit` would commit as in the active repository,
    /// read on every refresh so git config edits show up.
    pub repo_identity: Option<GitIdentity>,
    /// The global git identity, for the Settings placeholder.
    pub default_identity: Option<GitIdentity>,
    /// Bumped whenever another repository becomes active, so a result read
    /// for the one before is recognised and dropped.
    pub generation: u64,
    pub jobs: JobQueue,
}

impl GitPanel {
    pub fn load(data_dir: &Path) -> Self {
        Self {
            local_only: repos::load_local_only(data_dir),
            ..Self::default()
        }
    }
}

/// Who a commit is made as, and who the panel says it will be made as: the
/// name and email in Settings → Git when both are set, an override for
/// every repository; otherwise `from_git`, what plain `git commit` would use
/// in the repository.
pub fn commit_identity(
    name: &str,
    email: &str,
    from_git: Option<&GitIdentity>,
) -> Option<GitIdentity> {
    let (name, email) = (name.trim(), email.trim());
    if name.is_empty() || email.is_empty() {
        return from_git.cloned();
    }
    Some(GitIdentity {
        name: name.to_owned(),
        email: email.to_owned(),
    })
}

/// A question the panel asks before doing anything it can't undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitPrompt {
    /// "Open folder" picked a folder that is not a repository.
    Init {
        path: PathBuf,
        entries: usize,
        /// The repository the folder sits inside, which could be opened
        /// instead.
        enclosing: Option<PathBuf>,
    },
    /// A branch switch was refused: these uncommitted files would be lost.
    Switch {
        branch: String,
        changes: Vec<String>,
    },
    /// Removing a repository: what becomes of its collections.
    RemoveRepo {
        id: String,
        name: String,
        collections: usize,
    },
}

/// Git work for the blocking pool, run one job at a time in order: two at
/// once would race on the same files and on git's `index.lock`.
#[derive(Debug, Default)]
pub struct JobQueue {
    running: bool,
    waiting: VecDeque<GitJob>,
}

impl JobQueue {
    pub fn is_busy(&self) -> bool {
        self.running || !self.waiting.is_empty()
    }

    /// Queues `job`. A refresh already waiting covers another one: it reads
    /// the state of when it starts, not of when it was asked for.
    pub fn push(&mut self, job: GitJob) {
        if job == GitJob::Refresh && self.waiting.contains(&job) {
            return;
        }
        self.waiting.push_back(job);
    }

    /// The next job, marked running; `None` while one still runs.
    pub fn start_next(&mut self) -> Option<GitJob> {
        if self.running {
            return None;
        }
        let job = self.waiting.pop_front()?;
        self.running = true;
        Some(job)
    }

    pub fn finished(&mut self) {
        self.running = false;
    }
}

/// One unit of background git work. Jobs name their repository when queued,
/// so switching repositories meanwhile can't redirect them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitJob {
    /// Write the active repository's working tree, then read every
    /// repository's state for the panel.
    Refresh,
    Diff {
        repo: String,
    },
    Commit {
        repo: String,
        message: String,
        identity: GitIdentity,
    },
    SetRemote {
        repo: String,
        url: String,
    },
    CreateBranch {
        repo: String,
        name: String,
    },
    Fetch {
        repo: String,
    },
    Push {
        repo: String,
    },
    Pull {
        repo: String,
    },
    Switch {
        repo: String,
        branch: String,
        discard: bool,
    },
    Restore {
        repo: String,
        commit: String,
    },
    /// Add the folder at `path`, making it a repository first if `init`.
    AddFolder {
        path: PathBuf,
        init: bool,
    },
    Clone {
        url: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(name: &str, email: &str) -> GitIdentity {
        GitIdentity {
            name: name.into(),
            email: email.into(),
        }
    }

    #[test]
    fn a_complete_override_wins_in_every_repository() {
        let work = identity("Ada", "ada@work.invalid");
        assert_eq!(
            commit_identity(" Ada L ", "ada@example.invalid", Some(&work)),
            Some(identity("Ada L", "ada@example.invalid"))
        );
        assert_eq!(
            commit_identity("Ada L", "ada@example.invalid", None),
            Some(identity("Ada L", "ada@example.invalid"))
        );
    }

    #[test]
    fn without_a_complete_override_git_decides() {
        let work = identity("Ada", "ada@work.invalid");
        assert_eq!(commit_identity("", "", Some(&work)), Some(work.clone()));
        assert_eq!(
            commit_identity("Ada L", "  ", Some(&work)),
            Some(work),
            "half an override is none"
        );
        assert_eq!(commit_identity("", "", None), None);
    }

    fn fetch(repo: &str) -> GitJob {
        GitJob::Fetch { repo: repo.into() }
    }

    #[test]
    fn jobs_run_one_at_a_time_in_order() {
        let mut queue = JobQueue::default();
        assert!(!queue.is_busy());
        queue.push(fetch("a"));
        queue.push(fetch("b"));
        assert_eq!(queue.start_next(), Some(fetch("a")));
        assert_eq!(queue.start_next(), None, "a is still running");
        assert!(queue.is_busy());
        queue.finished();
        assert_eq!(queue.start_next(), Some(fetch("b")));
        queue.finished();
        assert_eq!(queue.start_next(), None);
        assert!(!queue.is_busy());
    }

    #[test]
    fn a_waiting_refresh_absorbs_another() {
        let mut queue = JobQueue::default();
        queue.push(GitJob::Refresh);
        assert_eq!(queue.start_next(), Some(GitJob::Refresh));
        queue.push(GitJob::Refresh);
        queue.push(fetch("a"));
        queue.push(GitJob::Refresh);
        queue.finished();
        assert_eq!(
            queue.start_next(),
            Some(GitJob::Refresh),
            "one waits behind the running one"
        );
        queue.finished();
        assert_eq!(queue.start_next(), Some(fetch("a")));
        queue.finished();
        assert_eq!(queue.start_next(), None);
    }
}
