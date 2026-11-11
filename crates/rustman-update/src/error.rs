/// Why an update couldn't be found, downloaded or installed.
#[derive(Debug, thiserror::Error)]
pub(crate) enum UpdateError {
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Unsupported(String),
    #[error("release {release} has no '{target}*{extension}' asset")]
    NoAsset {
        release: String,
        target: &'static str,
        extension: &'static str,
    },
    #[error("'{0}' not found in archive")]
    NotInArchive(&'static str),
    #[error("extracted binary is too small")]
    TooSmall,
    #[error("extracted binary has unknown header {0:02x?} — refusing to replace")]
    NotABinary([u8; 4]),
    #[error("current exe has no parent dir")]
    NoExeDir,
    #[cfg(target_os = "windows")]
    #[error("{0}")]
    Archive(String),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error("{version} is not newer than the running {current}; refusing to install it")]
    NotNewer { version: String, current: String },
    #[error("refusing to download an update from {0}")]
    Untrusted(String),
    #[error("the download is larger than {} MB; refusing it", .0 / (1024 * 1024))]
    TooLarge(usize),
    #[error("the release's SHA256SUMS isn't text")]
    BadChecksums,
    #[error("the release's SHA256SUMS has no entry for {0}")]
    NoChecksum(String),
    #[error("{0} doesn't match the release's SHA256SUMS; the download is damaged or was altered")]
    ChecksumMismatch(String),
}
