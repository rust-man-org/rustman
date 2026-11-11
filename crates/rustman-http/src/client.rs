use reqwest::redirect::Policy;
use reqwest::tls::Version;
use reqwest::{Client, NoProxy, Proxy};

const USER_AGENT: &str = concat!("rustman/", env!("CARGO_PKG_VERSION"));

/// Hosts never sent through a proxy, whatever the environment says: a
/// proxy can't reach the user's own machine, and browsers bypass it too.
const LOOPBACK: &str = "localhost,127.0.0.0/8,::1";

/// Connection-level TLS behaviour, for endpoints a browser or curl reaches but
/// a default rustls client does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TlsOptions {
    /// Accept self-signed, untrusted or mismatched certificates, like `curl -k`.
    pub accept_invalid_certs: bool,
    /// Offer only HTTP/1.1 in ALPN. Some servers and middleboxes reset the
    /// connection when offered `h2`, and unlike a browser a plain client
    /// doesn't retry without it.
    pub http1_only: bool,
    pub pin: TlsPin,
}

/// The TLS version the handshake is restricted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TlsPin {
    #[default]
    Any,
    /// For servers that break on 1.3.
    Tls12,
    /// For servers that require it.
    Tls13,
}

/// A client with cookie storage and the given TLS behaviour, or why reqwest
/// rejected the combination.
///
/// TLS settings live on the client because it owns the connection pool, so
/// changing them means building a new one.
pub fn try_build_client(options: TlsOptions) -> Result<Client, String> {
    // Bodies are decoded in `response::read`, behind the size cap.
    // Redirects are followed in `transport`, which strips what they mustn't
    // carry to another host.
    let mut builder = Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .user_agent(USER_AGENT)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd();
    if options.accept_invalid_certs {
        builder = builder
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);
    }
    if options.http1_only {
        builder = builder.http1_only();
    }
    let pinned = match options.pin {
        TlsPin::Any => None,
        TlsPin::Tls12 => Some(Version::TLS_1_2),
        TlsPin::Tls13 => Some(Version::TLS_1_3),
    };
    if let Some(version) = pinned {
        builder = builder.min_tls_version(version).max_tls_version(version);
    }
    for proxy in env_proxies() {
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|e| e.to_string())
}

/// The proxies `HTTP_PROXY`, `HTTPS_PROXY` and `ALL_PROXY` (or their
/// lower-case forms) name, read as reqwest reads them, but never used for
/// [`LOOPBACK`] hosts or those `NO_PROXY` lists. The first that applies to
/// a URL wins, so a scheme's own proxy goes before `ALL_PROXY`.
fn env_proxies() -> Vec<Proxy> {
    let bypass = match first_env(&["NO_PROXY", "no_proxy"]) {
        Some(listed) => format!("{LOOPBACK},{listed}"),
        None => LOOPBACK.to_owned(),
    };
    let proxies = [
        first_env(&["HTTP_PROXY", "http_proxy"]).and_then(|url| Proxy::http(url).ok()),
        first_env(&["HTTPS_PROXY", "https_proxy"]).and_then(|url| Proxy::https(url).ok()),
        first_env(&["ALL_PROXY", "all_proxy"]).and_then(|url| Proxy::all(url).ok()),
    ];
    proxies
        .into_iter()
        .flatten()
        .map(|proxy| proxy.no_proxy(NoProxy::from_string(&bypass)))
        .collect()
}

fn first_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| std::env::var(name).ok())
}

/// [`try_build_client`], falling back to default TLS settings (and logging
/// why) when `options` are rejected. Fails only when even those can't be
/// set up, as with a broken system certificate store.
pub fn build_client_with(options: TlsOptions) -> Result<Client, String> {
    try_build_client(options).or_else(|err| {
        eprintln!("HTTP client: {err}; falling back to default TLS settings");
        try_build_client(TlsOptions::default())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_combination_builds_without_falling_back() {
        for accept_invalid_certs in [false, true] {
            for http1_only in [false, true] {
                for pin in [TlsPin::Any, TlsPin::Tls12, TlsPin::Tls13] {
                    let options = TlsOptions {
                        accept_invalid_certs,
                        http1_only,
                        pin,
                    };
                    assert!(try_build_client(options).is_ok(), "{options:?}");
                }
            }
        }
    }

    #[test]
    fn default_options_are_all_off() {
        assert_eq!(
            TlsOptions::default(),
            TlsOptions {
                accept_invalid_certs: false,
                http1_only: false,
                pin: TlsPin::Any,
            }
        );
    }
}
