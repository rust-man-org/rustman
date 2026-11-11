//! In-app self-update from the project's GitHub releases: find a newer
//! release, download the archive for this platform, swap the running
//! executable for the binary inside it, and relaunch.
//!
//! Asset names and the binary name must match what
//! `.github/workflows/release.yml` publishes. Errors are reported as text at
//! this boundary.

mod archive;
mod error;
mod release;
mod restart;

use error::UpdateError;

pub use restart::restart;

/// A release newer than the running build, and the archive that installs it.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// The newer version, e.g. `"0.4.0"`.
    pub version: String,
    /// The running version.
    pub current: String,
    asset: Asset,
}

/// The release archive for this platform.
#[derive(Debug, Clone)]
struct Asset {
    name: String,
    url: String,
    /// The release's `SHA256SUMS`, when it has one.
    checksums: Option<String>,
}

/// The latest release, when it is newer than `current` (the running build's
/// version). Fails on platforms the release pipeline doesn't build for, so
/// the UI can say why there is no update rather than offer one that could
/// never install.
pub async fn check(current: &str) -> Result<Option<UpdateInfo>, String> {
    reported(release::check(current).await)
}

/// Downloads the release `update` describes, as [`check`] found it, and
/// swaps it in for the running executable; returns the version installed.
/// Refuses a release that isn't newer than `current`, the running build's
/// version, and an archive that doesn't match the release's checksums.
pub async fn install(update: UpdateInfo, current: &str) -> Result<String, String> {
    reported(release::install(&update, current).await)
}

fn reported<T>(result: Result<T, UpdateError>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}
