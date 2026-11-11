use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::redirect::Policy;
use reqwest::{Client, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::archive;
use crate::error::UpdateError;
use crate::{Asset, UpdateInfo};

const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/rust-man-org/rustman/releases/latest";

/// The archive type the updater installs. The macOS release also uploads a
/// DMG whose name holds the same platform token, so the name alone can't
/// pick the right asset.
#[cfg(not(target_os = "windows"))]
const ASSET_EXTENSION: &str = ".tar.gz";
#[cfg(target_os = "windows")]
const ASSET_EXTENSION: &str = ".zip";

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize, Debug)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// The checksum list the release workflow uploads, in `sha256sum` format.
const CHECKSUMS_ASSET: &str = "SHA256SUMS";

/// Where an update may come from: GitHub, and the storage hosts its release
/// downloads redirect to. Always over HTTPS.
const DOWNLOAD_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// Larger than any release archive; a download past it is refused.
const MAX_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const MAX_CHECKSUMS_BYTES: usize = 64 * 1024;
const MAX_REDIRECTS: usize = 10;
const HEX: &[u8; 16] = b"0123456789abcdef";

/// An API token, sent to the GitHub API only, and only when set here rather
/// than picked up from a CI-style `GITHUB_TOKEN`.
const TOKEN_VAR: &str = "RUSTMAN_GITHUB_TOKEN";

pub(crate) async fn check(current: &str) -> Result<Option<UpdateInfo>, UpdateError> {
    let target = asset_target()?;
    let release = fetch_latest(current).await?;
    let latest = latest_version(&release);
    if !version_gt(&latest, current) {
        return Ok(None);
    }
    let archive = pick_asset(&release.assets, &latest, target, ASSET_EXTENSION)?;
    let checksums = release
        .assets
        .iter()
        .find(|a| a.name == CHECKSUMS_ASSET)
        .map(|a| trusted(&a.browser_download_url))
        .transpose()?;
    Ok(Some(UpdateInfo {
        version: latest,
        current: current.to_owned(),
        asset: Asset {
            name: archive.name.clone(),
            url: trusted(&archive.browser_download_url)?,
            checksums,
        },
    }))
}

/// Installs exactly the release `update` describes, which [`check`] found
/// and the user accepted, after making sure it is still newer than
/// `current` and, when the release lists checksums, that the archive
/// matches its own.
pub(crate) async fn install(update: &UpdateInfo, current: &str) -> Result<String, UpdateError> {
    if !version_gt(&update.version, current) {
        return Err(UpdateError::NotNewer {
            version: update.version.clone(),
            current: current.to_owned(),
        });
    }
    let client = download_client(current)?;
    let asset = &update.asset;
    let archive = download(&client, &asset.url, MAX_ARCHIVE_BYTES).await?;
    if let Some(checksums) = &asset.checksums {
        let checksums = download(&client, checksums, MAX_CHECKSUMS_BYTES).await?;
        verify(&archive, &asset.name, &checksums)?;
    }
    tokio::task::spawn_blocking(move || archive::replace_running_binary(&archive)).await??;
    Ok(update.version.clone())
}

fn latest_version(release: &GhRelease) -> String {
    release.tag_name.trim_start_matches('v').to_owned()
}

/// The asset token for this platform and CPU, or why there is none.
fn asset_target() -> Result<&'static str, UpdateError> {
    #[cfg(target_os = "macos")]
    {
        Ok("macos-universal")
    }
    #[cfg(target_os = "linux")]
    {
        match std::env::consts::ARCH {
            "x86_64" => Ok("linux-x86_64"),
            "aarch64" => Ok("linux-aarch64"),
            arch => Err(UpdateError::Unsupported(format!(
                "no Linux release builds for the '{arch}' architecture — build from source instead"
            ))),
        }
    }
    #[cfg(target_os = "windows")]
    {
        match std::env::consts::ARCH {
            "x86_64" => Ok("windows-x86_64"),
            "aarch64" => Ok("windows-aarch64"),
            arch => Err(UpdateError::Unsupported(format!(
                "no Windows release builds for the '{arch}' architecture — build from source instead"
            ))),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(UpdateError::Unsupported(
            "self-update is not supported on this platform".to_owned(),
        ))
    }
}

/// The client for the GitHub API, the only one that may carry a token.
fn api_client(current: &str) -> Result<Client, UpdateError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    if let Ok(token) = std::env::var(TOKEN_VAR) {
        match HeaderValue::from_str(&format!("Bearer {token}")) {
            Ok(value) => {
                headers.insert(AUTHORIZATION, value);
            }
            Err(_) => eprintln!("rustman: ignoring a {TOKEN_VAR} that isn't a valid header"),
        }
    }
    Ok(Client::builder()
        .user_agent(format!("rustman/{current}"))
        .default_headers(headers)
        .build()?)
}

/// The client for release downloads: no credentials, and redirects only to
/// the [`DOWNLOAD_HOSTS`].
fn download_client(current: &str) -> Result<Client, UpdateError> {
    let policy = Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            attempt.error("too many redirects")
        } else if is_trusted(attempt.url()) {
            attempt.follow()
        } else {
            let to = attempt.url().to_string();
            attempt.error(format!("refusing a redirect to {to}"))
        }
    });
    Ok(Client::builder()
        .user_agent(format!("rustman/{current}"))
        .redirect(policy)
        .build()?)
}

async fn fetch_latest(current: &str) -> Result<GhRelease, UpdateError> {
    Ok(api_client(current)?
        .get(LATEST_RELEASE_API)
        .send()
        .await?
        .error_for_status()?
        .json::<GhRelease>()
        .await?)
}

/// `url` when it is HTTPS on one of the [`DOWNLOAD_HOSTS`].
fn trusted(url: &str) -> Result<String, UpdateError> {
    match Url::parse(url) {
        Ok(parsed) if is_trusted(&parsed) => Ok(url.to_owned()),
        _ => Err(UpdateError::Untrusted(url.to_owned())),
    }
}

fn is_trusted(url: &Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| DOWNLOAD_HOSTS.contains(&host))
}

/// The body at `url`, read as it streams in, and refused once it passes
/// `limit` bytes.
async fn download(client: &Client, url: &str, limit: usize) -> Result<Vec<u8>, UpdateError> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    let expected = response
        .content_length()
        .map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX));
    if expected > limit {
        return Err(UpdateError::TooLarge(limit));
    }
    let mut body = Vec::with_capacity(expected);
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > limit {
            return Err(UpdateError::TooLarge(limit));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Checks `archive` against its line in `checksums` (`<sha256>  <name>`, as
/// `sha256sum` writes it).
fn verify(archive: &[u8], name: &str, checksums: &[u8]) -> Result<(), UpdateError> {
    let checksums = std::str::from_utf8(checksums).map_err(|_| UpdateError::BadChecksums)?;
    let expected = checksums
        .lines()
        .find_map(|line| {
            let (hash, file) = line.split_once(char::is_whitespace)?;
            let file = file.trim_start().trim_start_matches('*');
            (file == name).then_some(hash)
        })
        .ok_or_else(|| UpdateError::NoChecksum(name.to_owned()))?;
    let actual: String = Sha256::digest(archive)
        .iter()
        .flat_map(|byte| [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0xf)]])
        .map(char::from)
        .collect();
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(UpdateError::ChecksumMismatch(name.to_owned()))
    }
}

/// The asset for `target` with `extension`. Assets with `nokit` in their
/// name are a variant older releases also published; the standard build is
/// the one to install.
fn pick_asset<'a>(
    assets: &'a [GhAsset],
    release: &str,
    target: &'static str,
    extension: &'static str,
) -> Result<&'a GhAsset, UpdateError> {
    assets
        .iter()
        .find(|a| {
            !a.name.contains("nokit") && a.name.contains(target) && a.name.ends_with(extension)
        })
        .ok_or_else(|| UpdateError::NoAsset {
            release: release.to_owned(),
            target,
            extension,
        })
}

/// Whether `MAJOR.MINOR.PATCH` version `a` is newer than `b`. Pre-release
/// and build suffixes are ignored, so a dev build of the current version is
/// not an update.
fn version_gt(a: &str, b: &str) -> bool {
    parse_version(a) > parse_version(b)
}

fn parse_version(v: &str) -> (u64, u64, u64) {
    let core = v.split(['-', '+']).next().unwrap_or(v);
    let mut parts = core.split('.').map(|x| x.trim().parse().unwrap_or(0));
    let mut next = || parts.next().unwrap_or(0);
    (next(), next(), next())
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
        assert!(!version_gt("0.3.2-dev.5", "0.3.2"));
    }

    fn asset(name: &str) -> GhAsset {
        GhAsset {
            name: name.to_owned(),
            browser_download_url: format!("https://example.test/{name}"),
        }
    }

    #[test]
    fn picks_the_tarball_not_the_dmg_when_both_contain_the_target() {
        for assets in [
            vec![
                asset("Rustman-macos-universal.dmg"),
                asset("Rustman-macos-universal.tar.gz"),
            ],
            vec![
                asset("Rustman-macos-universal.tar.gz"),
                asset("Rustman-macos-universal.dmg"),
            ],
        ] {
            let picked = pick_asset(&assets, "1.2.3", "macos-universal", ".tar.gz").unwrap();
            assert_eq!(picked.name, "Rustman-macos-universal.tar.gz");
        }
    }

    #[test]
    fn errors_clearly_when_no_matching_asset_exists() {
        let assets = vec![asset("Rustman-macos-universal.dmg")];
        let err = pick_asset(&assets, "1.2.3", "macos-universal", ".tar.gz")
            .unwrap_err()
            .to_string();
        assert_eq!(err, "release 1.2.3 has no 'macos-universal*.tar.gz' asset");
    }

    #[test]
    fn never_picks_a_nokit_asset() {
        let assets = vec![
            asset("rustman-linux-x86_64-nokit.tar.gz"),
            asset("rustman-linux-x86_64.tar.gz"),
        ];
        let picked = pick_asset(&assets, "1.2.3", "linux-x86_64", ".tar.gz").unwrap();
        assert_eq!(picked.name, "rustman-linux-x86_64.tar.gz");
    }

    const ARCHIVE: &[u8] = b"archive bytes";

    fn sums(lines: &[String]) -> Vec<u8> {
        lines.join("\n").into_bytes()
    }

    #[test]
    fn an_archive_is_checked_against_its_own_line() {
        let actual: String = Sha256::digest(ARCHIVE)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let good = sums(&[
            format!("{} other.zip", "0".repeat(64)),
            format!("{actual}  rustman-linux-x86_64.tar.gz"),
        ]);
        verify(ARCHIVE, "rustman-linux-x86_64.tar.gz", &good).unwrap();
        let binary_mode = sums(&[format!("{} *a.zip", actual.to_uppercase())]);
        verify(ARCHIVE, "a.zip", &binary_mode).unwrap();

        let altered = sums(&[format!("{}  a.zip", "f".repeat(64))]);
        assert!(matches!(
            verify(ARCHIVE, "a.zip", &altered),
            Err(UpdateError::ChecksumMismatch(_))
        ));
        assert!(matches!(
            verify(ARCHIVE, "missing.zip", &good),
            Err(UpdateError::NoChecksum(_))
        ));
    }

    #[test]
    fn only_https_github_downloads_are_trusted() {
        for url in [
            "https://github.com/rust-man-org/rustman/releases/download/v1/a.zip",
            "https://objects.githubusercontent.com/x",
            "https://release-assets.githubusercontent.com/x",
        ] {
            assert!(trusted(url).is_ok(), "{url}");
        }
        for url in [
            "http://github.com/a.zip",
            "https://github.com.evil.test/a.zip",
            "https://evil.test/github.com/a.zip",
            "not a url",
        ] {
            assert!(trusted(url).is_err(), "{url}");
        }
    }

    fn update(version: &str) -> UpdateInfo {
        UpdateInfo {
            version: version.to_owned(),
            current: "1.0.0".to_owned(),
            asset: Asset {
                name: "a.zip".into(),
                url: "https://github.com/a.zip".into(),
                checksums: None,
            },
        }
    }

    #[tokio::test]
    async fn an_install_that_is_not_newer_is_refused_before_any_download() {
        for version in ["1.0.0", "0.9.9"] {
            let err = install(&update(version), "1.0.0").await.unwrap_err();
            assert!(matches!(err, UpdateError::NotNewer { .. }), "{err}");
        }
    }

    /// Serves one response, whatever is asked.
    fn serve(response: Vec<u8>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/a.zip", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0; 4096];
            assert!(stream.read(&mut buf).unwrap() > 0);
            stream.write_all(&response).unwrap();
        });
        url
    }

    #[tokio::test]
    async fn a_download_past_its_limit_is_refused() {
        let client = Client::new();
        let declared = serve(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n".to_vec());
        assert!(matches!(
            download(&client, &declared, 10).await,
            Err(UpdateError::TooLarge(10))
        ));

        let mut chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
        chunked.extend_from_slice(b"8\r\n12345678\r\n8\r\n12345678\r\n0\r\n\r\n");
        assert!(matches!(
            download(&client, &serve(chunked), 10).await,
            Err(UpdateError::TooLarge(10))
        ));

        let small = serve(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc".to_vec());
        assert_eq!(download(&client, &small, 10).await.unwrap(), b"abc");
    }
}
