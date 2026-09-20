//! In-app self-update from GitHub releases (https://github.com/rust-man-org/rustman).
//!
//! Uses the app's existing async `reqwest` to fetch the latest release and the
//! matching platform asset, then extracts the binary and atomically swaps the
//! running executable. The asset tokens / binary name must match the artefacts
//! produced by `.github/workflows/release.yml`.

const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/rust-man-org/rustman/releases/latest";

/// Substring matched against the release asset names for the running platform.
#[cfg(target_os = "macos")]
const ASSET_TARGET: &str = "macos-universal";
#[cfg(target_os = "linux")]
const ASSET_TARGET: &str = "linux-x86_64";
#[cfg(target_os = "windows")]
const ASSET_TARGET: &str = "windows-x86_64";

/// Extension the self-update asset must end with. The macOS release uploads
/// *two* assets whose names both contain `ASSET_TARGET` — the installer DMG
/// and the tarball this self-updater actually wants — so the substring match
/// alone isn't enough; without this, picking the `.dmg` by accident feeds a
/// disk image into the gzip decoder and fails with "invalid Gzip header".
#[cfg(not(target_os = "windows"))]
const ASSET_EXTENSION: &str = ".tar.gz";
#[cfg(target_os = "windows")]
const ASSET_EXTENSION: &str = ".zip";

/// Name of the executable inside the downloaded archive.
#[cfg(not(target_os = "windows"))]
const BIN_NAME: &str = "rustman";
#[cfg(target_os = "windows")]
const BIN_NAME: &str = "rustman.exe";

/// The current build's version, baked in at compile time.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// The newer version available, e.g. `"0.4.0"`.
    pub version: String,
    /// The running version.
    pub current: String,
}

#[derive(serde::Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(serde::Deserialize, Debug)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn client() -> Result<reqwest::Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/vnd.github+json"),
    );
    if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")) {
        if let Ok(val) = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")) {
            headers.insert(reqwest::header::AUTHORIZATION, val);
        }
    }
    reqwest::Client::builder()
        .user_agent(concat!("rustman/", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        .build()
        .map_err(|e| e.to_string())
}

async fn fetch_latest() -> Result<GhRelease, String> {
    client()?
        .get(LATEST_RELEASE_API)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<GhRelease>()
        .await
        .map_err(|e| e.to_string())
}

/// Query the latest release; returns `Some` only when it is newer than the
/// running build.
pub async fn check() -> Result<Option<UpdateInfo>, String> {
    let release = fetch_latest().await?;
    let latest = release.tag_name.trim_start_matches('v').to_owned();
    let current = current_version();
    Ok(version_gt(&latest, current).then(|| UpdateInfo {
        version: latest,
        current: current.to_owned(),
    }))
}

/// Picks the self-update asset out of a release's asset list whose name
/// contains `target` and ends with `extension`. Matching on `target` alone
/// isn't enough on macOS, where the DMG installer and the tarball both have
/// it in their name — see `ASSET_EXTENSION`'s doc comment.
fn pick_asset<'a>(
    assets: &'a [GhAsset],
    release: &str,
    target: &str,
    extension: &str,
) -> Result<&'a GhAsset, String> {
    assets
        .iter()
        .find(|a| a.name.contains(target) && a.name.ends_with(extension))
        .ok_or_else(|| format!("release {release} has no '{target}*{extension}' asset"))
}

pub async fn install() -> Result<String, String> {
    let release = fetch_latest().await?;
    let latest = release.tag_name.trim_start_matches('v').to_owned();
    let asset = pick_asset(&release.assets, &latest, ASSET_TARGET, ASSET_EXTENSION)?;
    let bytes = client()?
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?
        .to_vec();
    tokio::task::spawn_blocking(move || extract_and_replace(bytes))
        .await
        .map_err(|e| e.to_string())??;
    Ok(latest)
}

/// Resolves the path of the freshly-updated executable. On Linux,
/// `self_replace` swaps the running binary out from under the process by
/// renaming the new file onto its own path — an atomic `rename(2)` that
/// unlinks the old directory entry while the process still has the old
/// inode mapped. `std::env::current_exe()` reads `/proc/self/exe`, which
/// keeps resolving to that now-unlinked old inode, with a literal
/// `" (deleted)"` suffix appended to the path string by the kernel. The new
/// binary is a perfectly real file sitting at that same path *without* the
/// suffix, so strip it before checking existence or spawning — otherwise
/// `exe.exists()` is false and any error message names an unrunnable path.
fn resolved_exe_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    return Ok(strip_deleted_suffix(exe));
    #[cfg(not(target_os = "linux"))]
    Ok(exe)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn strip_deleted_suffix(exe: std::path::PathBuf) -> std::path::PathBuf {
    match exe.to_str().and_then(|s| s.strip_suffix(" (deleted)")) {
        Some(stripped) => std::path::PathBuf::from(stripped),
        None => exe,
    }
}

/// Relaunch the freshly-updated executable and exit the current process.
pub fn restart() -> Result<std::convert::Infallible, String> {
    let exe = resolved_exe_path()?;

    if !exe.exists() {
        return Err(format!(
            "Binary not found at {path}. The update replaced the file but the current \
             process may be reading a stale inode. Try running {path} manually.",
            path = exe.display(),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, a downloaded binary is often quarantined.  Try to remove the
        // quarantine attribute so Gatekeeper doesn't block the relaunch.  If this
        // fails we still attempt to spawn — the error is propagated if it fails.
        let _ = std::process::Command::new("xattr")
            .args(["-dr", "com.apple.quarantine"])
            .arg(&exe)
            .output();
    }

    match std::process::Command::new(&exe).spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            let hint = if cfg!(target_os = "macos") {
                "The binary may need code-signing or the quarantine flag removed. \
                 Try: xattr -dr com.apple.quarantine "
            } else if cfg!(target_os = "linux") {
                "If you are running via cargo run, the binary was swapped but the \
                 old inode may still be cached. Try running the binary directly: "
            } else {
                ""
            };
            Err(format!(
                "Failed to restart from {path}: {e}. {hint}{path}",
                path = exe.display(),
                hint = hint,
            ))
        }
    }
}

/// Compare two `MAJOR.MINOR.PATCH` strings (pre-release suffixes are ignored).
fn version_gt(a: &str, b: &str) -> bool {
    parse_ver(a) > parse_ver(b)
}

fn parse_ver(v: &str) -> (u64, u64, u64) {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let mut it = core.split('.').map(|x| x.trim().parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// Unpack the platform binary from the downloaded archive and swap it in for the
/// running executable. Blocking (filesystem + decompression).
fn extract_and_replace(bytes: Vec<u8>) -> Result<(), String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = current.parent().ok_or("current exe has no parent dir")?;
    let tmp = dir.join(".rustman-update.tmp");

    extract_binary(&bytes, &tmp)?;

    // Sanity-check: the extracted binary should look like a valid executable.
    verify_binary(&tmp)?;

    self_replace::self_replace(&tmp).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

/// Quick validation that a file at `path` is plausibly a native binary.
/// Checks the ELF / Mach-O / PE magic bytes.
fn verify_binary(path: &std::path::Path) -> Result<(), String> {
    let magic = std::fs::read(path)
        .map_err(|e| e.to_string())?;
    if magic.len() < 4 {
        return Err("extracted binary is too small".into());
    }
    match &magic[..4] {
        // ELF  (Linux)
        [0x7f, b'E', b'L', b'F'] => Ok(()),
        // Mach-O 64-bit  (macOS, fat binary)
        [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe] | [0xbe, 0xba, 0xfe, 0xca] => {
            Ok(())
        }
        // PE  (Windows)
        [b'M', b'Z', _, _] => Ok(()),
        h => Err(format!(
            "extracted binary has unknown header {h:02x?} — refusing to replace"
        )),
    }
}

#[cfg(not(target_os = "windows"))]
fn extract_binary(bytes: &[u8], out: &std::path::Path) -> Result<(), String> {
    use std::io::Read;
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let is_bin = entry
            .path()
            .map(|p| p.file_name() == Some(std::ffi::OsStr::new(BIN_NAME)))
            .unwrap_or(false);
        if is_bin {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            std::fs::write(out, &buf).map_err(|e| e.to_string())?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err(format!("'{BIN_NAME}' not found in archive"))
}

#[cfg(target_os = "windows")]
fn extract_binary(bytes: &[u8], out: &std::path::Path) -> Result<(), String> {
    use std::io::Read;
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        if f.name().rsplit(['/', '\\']).next() == Some(BIN_NAME) {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            std::fs::write(out, &buf).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err(format!("'{BIN_NAME}' not found in archive"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(version_gt("0.4.0", "0.3.2"));
        assert!(version_gt("1.0.0", "0.9.9"));
        assert!(version_gt("0.3.10", "0.3.2"));
        assert!(!version_gt("0.3.2", "0.3.2"));
        assert!(!version_gt("0.3.1", "0.3.2"));
        // pre-release suffix is ignored, so a dev tag of the current version
        // is not treated as an upgrade.
        assert!(!version_gt("0.3.2-dev.5", "0.3.2"));
    }

    fn asset(name: &str) -> GhAsset {
        GhAsset { name: name.to_owned(), browser_download_url: format!("https://example.test/{name}") }
    }

    #[test]
    fn picks_the_tarball_not_the_dmg_when_both_contain_the_target_substring() {
        // Regression test for the real "invalid Gzip header" self-update crash
        // on macOS: `Rustman-macos-universal.dmg` and
        // `Rustman-macos-universal.tar.gz` both contain "macos-universal", so
        // a plain substring match could grab whichever the API happens to
        // list first — including the DMG, whose raw bytes aren't gzip at all.
        let assets = vec![
            asset("Rustman-macos-universal.dmg"),
            asset("Rustman-macos-universal.tar.gz"),
        ];
        let picked = pick_asset(&assets, "v1.2.3", "macos-universal", ".tar.gz")
            .expect("should find the tarball");
        assert_eq!(picked.name, "Rustman-macos-universal.tar.gz");
    }

    #[test]
    fn picks_the_tarball_even_when_it_is_listed_before_the_dmg() {
        let assets = vec![
            asset("Rustman-macos-universal.tar.gz"),
            asset("Rustman-macos-universal.dmg"),
        ];
        let picked = pick_asset(&assets, "v1.2.3", "macos-universal", ".tar.gz")
            .expect("should find the tarball");
        assert_eq!(picked.name, "Rustman-macos-universal.tar.gz");
    }

    #[test]
    fn errors_clearly_when_no_matching_asset_exists() {
        let assets = vec![asset("Rustman-macos-universal.dmg")];
        let err = pick_asset(&assets, "v1.2.3", "macos-universal", ".tar.gz").unwrap_err();
        assert!(err.contains("v1.2.3"), "error should name the release: {err}");
    }

    #[test]
    fn strips_deleted_suffix_left_by_proc_self_exe_after_self_replace() {
        // Regression test for the real "Binary not found ... (deleted)" crash:
        // after self_replace renames the new binary onto the running
        // process's own path, /proc/self/exe keeps resolving to the old,
        // now-unlinked inode with this literal suffix appended.
        let deleted = std::path::PathBuf::from("/home/anime/work/rustman/target/debug/rustman (deleted)");
        let real = strip_deleted_suffix(deleted);
        assert_eq!(real, std::path::PathBuf::from("/home/anime/work/rustman/target/debug/rustman"));
    }

    #[test]
    fn leaves_a_normal_path_untouched() {
        let normal = std::path::PathBuf::from("/usr/local/bin/rustman");
        assert_eq!(strip_deleted_suffix(normal.clone()), normal);
    }
}
