use std::path::Path;

use git2::{
    BranchType, Commit, DiffFormat, DiffOptions, ErrorCode, IndexAddOption, Repository, Signature,
    Sort, Status, StatusOptions, build::CheckoutBuilder,
};

use super::error::GitError;
use super::{GitIdentity, open};

/// The working diff stops here; beyond it the panel shows a truncation note.
const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;
const DIFF_TRUNCATED: &str = "\n… diff truncated at 2 MB\n";

/// One commit as listed in the history panel.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    /// Seconds since the Unix epoch.
    pub timestamp: i64,
}

/// One changed path and how it changed: `new`, `deleted`, `renamed`,
/// `modified` or `changed`.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub state: String,
}

/// A local branch.
#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
}

/// The current branch, pending changes and position against `origin`.
#[derive(Debug, Clone, Default)]
pub struct RepoStatus {
    pub branch: String,
    pub changes: Vec<FileChange>,
    pub remote_url: Option<String>,
    pub ahead: usize,
    pub behind: usize,
}

/// The current branch, pending changes and position against `origin`.
pub fn status(data_dir: &Path) -> Result<RepoStatus, GitError> {
    let repo = open(data_dir)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let changes = repo
        .statuses(Some(&mut opts))?
        .iter()
        .filter_map(|entry| {
            Some(FileChange {
                path: entry.path().ok()?.to_owned(),
                state: change_kind(entry.status()).to_owned(),
            })
        })
        .collect();

    let branch = branch_name(&repo);
    let remote_url = super::remote::origin_url(&repo);
    let (ahead, behind) = ahead_behind(&repo, &branch).unwrap_or_default();
    Ok(RepoStatus {
        branch,
        changes,
        remote_url,
        ahead,
        behind,
    })
}

/// The checked-out branch, read from HEAD alone; `None` if `data_dir` is
/// not a repository.
pub fn current_branch(data_dir: &Path) -> Option<String> {
    Repository::open(data_dir)
        .ok()
        .map(|repo| branch_name(&repo))
}

/// The checked-out branch's short name; `main` before the first commit.
fn branch_name(repo: &Repository) -> String {
    repo.head()
        .ok()
        .and_then(|h| h.shorthand().ok().map(str::to_owned))
        .unwrap_or_else(|| "main".to_owned())
}

fn change_kind(s: Status) -> &'static str {
    if s.intersects(Status::WT_NEW | Status::INDEX_NEW) {
        "new"
    } else if s.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
        "deleted"
    } else if s.intersects(Status::WT_RENAMED | Status::INDEX_RENAMED) {
        "renamed"
    } else if s.intersects(Status::WT_MODIFIED | Status::INDEX_MODIFIED) {
        "modified"
    } else {
        "changed"
    }
}

/// Commits ahead of and behind `origin/<branch>`; `None` without upstream.
fn ahead_behind(repo: &Repository, branch: &str) -> Option<(usize, usize)> {
    let local = repo.refname_to_id(&format!("refs/heads/{branch}")).ok()?;
    let upstream = repo
        .refname_to_id(&format!("refs/remotes/origin/{branch}"))
        .ok()?;
    repo.graph_ahead_behind(local, upstream).ok()
}

/// Stages everything and commits it as `identity`; returns the commit id.
pub fn commit(data_dir: &Path, identity: &GitIdentity, message: &str) -> Result<String, GitError> {
    let repo = open(data_dir)?;
    let mut index = repo.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree = repo.find_tree(index.write_tree()?)?;
    let signature = Signature::now(&identity.name, &identity.email)?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&Commit<'_>> = parent.iter().collect();
    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;
    Ok(oid.to_string())
}

/// Up to `limit` commits from HEAD, newest first. Empty without commits, or
/// when the log can't be read.
pub fn full_log(data_dir: &Path, limit: usize) -> Vec<CommitInfo> {
    log(data_dir, limit).unwrap_or_else(|e| {
        eprintln!("rustman: could not read the git log: {e}");
        Vec::new()
    })
}

fn log(data_dir: &Path, limit: usize) -> Result<Vec<CommitInfo>, GitError> {
    let repo = open(data_dir)?;
    if repo
        .head()
        .is_err_and(|e| e.code() == ErrorCode::UnbornBranch)
    {
        return Ok(Vec::new());
    }
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TIME)?;
    let mut out = Vec::new();
    for oid in revwalk.take(limit) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        out.push(CommitInfo {
            id: oid.to_string(),
            message: commit.message().unwrap_or_default().to_owned(),
            timestamp: commit.time().seconds(),
        });
    }
    Ok(out)
}

/// Uncommitted changes as a patch, untracked files included with their
/// content, so a never-committed collection shows up as well. Capped at
/// [`MAX_DIFF_BYTES`].
pub fn working_diff(data_dir: &Path) -> Result<String, GitError> {
    let repo = open(data_dir)?;
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let mut opts = DiffOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true);
    let diff = repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?;

    let mut out = String::new();
    let printed = diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let text = String::from_utf8_lossy(line.content());
        if out.len() + 1 + text.len() > MAX_DIFF_BYTES {
            out.push_str(DIFF_TRUNCATED);
            return false;
        }
        if matches!(line.origin(), '+' | '-' | ' ') {
            out.push(line.origin());
        }
        out.push_str(&text);
        true
    });
    match printed {
        Err(e) if e.code() == ErrorCode::User && out.ends_with(DIFF_TRUNCATED) => Ok(out),
        other => other.map(|()| out).map_err(GitError::from),
    }
}

/// The local branches.
pub fn branches(data_dir: &Path) -> Result<Vec<BranchInfo>, GitError> {
    let repo = open(data_dir)?;
    let mut out = Vec::new();
    for item in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = item?;
        if let Ok(Some(name)) = branch.name() {
            out.push(BranchInfo {
                name: name.to_owned(),
                is_head: branch.is_head(),
            });
        }
    }
    Ok(out)
}

/// Branches off HEAD; an existing branch of that name is an error.
pub fn create_branch(data_dir: &Path, name: &str) -> Result<(), GitError> {
    let repo = open(data_dir)?;
    let head = repo.head()?.peel_to_commit()?;
    repo.branch(name, &head, false)?;
    Ok(())
}

/// What a branch switch may do to uncommitted changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checkout {
    /// Refuse with [`GitError::Dirty`] while anything is uncommitted.
    Safe,
    /// Overwrite uncommitted edits to tracked files. Files never committed
    /// stay.
    DiscardChanges,
}

/// Switches to the local branch `name`.
pub fn checkout_branch(data_dir: &Path, name: &str, mode: Checkout) -> Result<(), GitError> {
    let repo = open(data_dir)?;
    let refname = format!("refs/heads/{name}");
    let target = repo.revparse_single(&refname)?;
    let mut opts = CheckoutBuilder::new();
    match mode {
        Checkout::Safe => {
            let dirty = uncommitted(&repo)?;
            if !dirty.is_empty() {
                return Err(GitError::Dirty(dirty));
            }
            opts.safe();
        }
        Checkout::DiscardChanges => {
            opts.force();
        }
    }
    repo.checkout_tree(&target, Some(&mut opts))?;
    repo.set_head(&refname)?;
    Ok(())
}

/// Every path with uncommitted changes, untracked files included.
fn uncommitted(repo: &Repository) -> Result<Vec<String>, GitError> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    Ok(repo
        .statuses(Some(&mut opts))?
        .iter()
        .filter_map(|entry| entry.path().ok().map(str::to_owned))
        .collect())
}
