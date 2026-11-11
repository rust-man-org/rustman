use std::path::PathBuf;

/// A failed repository operation. Library errors keep their own message.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{} does not exist (moved, deleted, or on a drive that isn't mounted)", .0.display())]
    Missing(PathBuf),
    #[error("{} is not a git repository", .0.display())]
    NotARepository(PathBuf),
    #[error("repository has no working directory")]
    Bare,
    /// A checkout was refused: these uncommitted paths would be overwritten.
    #[error("{} uncommitted change(s) would be overwritten", .0.len())]
    Dirty(Vec<String>),
    #[error("could not run git (is it installed?): {0}")]
    Spawn(std::io::Error),
    /// The command ran and failed; holds its trimmed stderr.
    #[error("{0}")]
    Command(String),
}
