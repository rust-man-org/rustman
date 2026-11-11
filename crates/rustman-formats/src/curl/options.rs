//! curl's command-line grammar: which options take a value, and how short
//! options bundle.
//!
//! Short options follow getopt rules, like curl itself: `-sSL` is three
//! flags, and an option that takes a value consumes the rest of its word
//! (`-XPOST`, `-H'Accept: */*'`) or, when nothing is left, the next word
//! (`-sSo out.txt`). Long options take `--name value`, and `--name=value` is
//! accepted too because people write it. Every option comes out under its
//! long name, so the parser has one spelling to match on.

/// One parsed command-line argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Arg {
    /// An option by its long name (without dashes), with its value if it
    /// takes one.
    Opt { name: String, value: Option<String> },
    /// A word that is not an option — a URL, as far as curl is concerned.
    Positional(String),
}

/// Turn shell words into arguments.
pub(super) fn parse_args(words: impl IntoIterator<Item = String>) -> Vec<Arg> {
    let mut words = words.into_iter();
    let mut args = Vec::new();
    while let Some(word) = words.next() {
        if let Some(long) = word.strip_prefix("--").filter(|rest| !rest.is_empty()) {
            let (name, inline) = match long.split_once('=') {
                Some((name, value)) => (name.to_owned(), Some(value.to_owned())),
                None => (long.to_owned(), None),
            };
            let value = match inline {
                Some(value) => Some(value),
                None if long_takes_value(&name) => words.next(),
                None => None,
            };
            args.push(Arg::Opt { name, value });
        } else if let Some(bundle) = word.strip_prefix('-').filter(|rest| !rest.is_empty()) {
            parse_short_bundle(bundle, &mut words, &mut args);
        } else {
            args.push(Arg::Positional(word));
        }
    }
    args
}

fn parse_short_bundle(bundle: &str, words: &mut impl Iterator<Item = String>, args: &mut Vec<Arg>) {
    for (i, c) in bundle.char_indices() {
        match short_option(c) {
            Short::Flag(name) => args.push(Arg::Opt {
                name: name.to_owned(),
                value: None,
            }),
            Short::Value(name) => {
                let rest = &bundle[i + c.len_utf8()..];
                let value = if rest.is_empty() {
                    words.next()
                } else {
                    Some(rest.to_owned())
                };
                args.push(Arg::Opt {
                    name: name.to_owned(),
                    value,
                });
                return;
            }
            Short::Ignored => {}
        }
    }
}

enum Short {
    /// Takes a value; the long name it stands for.
    Value(&'static str),
    /// A flag that matters to the request.
    Flag(&'static str),
    /// Output, verbosity, IP version… nothing that changes the request.
    Ignored,
}

fn short_option(c: char) -> Short {
    let value = match c {
        'A' => "user-agent",
        'b' => "cookie",
        'c' => "cookie-jar",
        'C' => "continue-at",
        'd' => "data",
        'D' => "dump-header",
        'e' => "referer",
        'E' => "cert",
        'F' => "form",
        'H' => "header",
        'K' => "config",
        'm' => "max-time",
        'o' => "output",
        'P' => "ftp-port",
        'Q' => "quote",
        'r' => "range",
        't' => "telnet-option",
        'T' => "upload-file",
        'u' => "user",
        'U' => "proxy-user",
        'w' => "write-out",
        'x' => "proxy",
        'X' => "request",
        'y' => "speed-time",
        'Y' => "speed-limit",
        'z' => "time-cond",
        'G' => return Short::Flag("get"),
        'I' => return Short::Flag("head"),
        'k' => return Short::Flag("insecure"),
        ':' => return Short::Flag("next"),
        _ => return Short::Ignored,
    };
    Short::Value(value)
}

/// Long options that take a value. Unknown options are assumed to be flags;
/// if one did take a value, the parser's URL heuristic still keeps that
/// value from being mistaken for the URL.
fn long_takes_value(name: &str) -> bool {
    LONG_WITH_VALUE.binary_search(&name).is_ok()
}

/// Sorted for binary search (checked by a test).
const LONG_WITH_VALUE: &[&str] = &[
    "abstract-unix-socket",
    "alt-svc",
    "aws-sigv4",
    "cacert",
    "capath",
    "cert",
    "cert-type",
    "ciphers",
    "config",
    "connect-timeout",
    "connect-to",
    "continue-at",
    "cookie",
    "cookie-jar",
    "create-file-mode",
    "crlfile",
    "curves",
    "data",
    "data-ascii",
    "data-binary",
    "data-raw",
    "data-urlencode",
    "delegation",
    "dns-interface",
    "dns-ipv4-addr",
    "dns-ipv6-addr",
    "dns-servers",
    "doh-url",
    "dump-header",
    "ech",
    "egd-file",
    "engine",
    "etag-compare",
    "etag-save",
    "expect100-timeout",
    "form",
    "form-string",
    "ftp-account",
    "ftp-alternative-to-user",
    "ftp-method",
    "ftp-port",
    "ftp-ssl-ccc-mode",
    "happy-eyeballs-timeout-ms",
    "haproxy-clientip",
    "header",
    "hostpubmd5",
    "hostpubsha256",
    "hsts",
    "interface",
    "ip-tos",
    "ipfs-gateway",
    "json",
    "keepalive-cnt",
    "keepalive-time",
    "key",
    "key-type",
    "krb",
    "libcurl",
    "limit-rate",
    "local-port",
    "login-options",
    "mail-auth",
    "mail-from",
    "mail-rcpt",
    "max-filesize",
    "max-redirs",
    "max-time",
    "netrc-file",
    "noproxy",
    "oauth2-bearer",
    "output",
    "output-dir",
    "parallel-max",
    "pass",
    "pinnedpubkey",
    "preproxy",
    "proto",
    "proto-default",
    "proto-redir",
    "proxy",
    "proxy-cacert",
    "proxy-capath",
    "proxy-cert",
    "proxy-cert-type",
    "proxy-ciphers",
    "proxy-crlfile",
    "proxy-header",
    "proxy-key",
    "proxy-key-type",
    "proxy-pass",
    "proxy-pinnedpubkey",
    "proxy-service-name",
    "proxy-tls13-ciphers",
    "proxy-tlsauthtype",
    "proxy-tlspassword",
    "proxy-tlsuser",
    "proxy-user",
    "proxy1.0",
    "pubkey",
    "quote",
    "random-file",
    "range",
    "rate",
    "referer",
    "request",
    "request-target",
    "resolve",
    "retry",
    "retry-delay",
    "retry-max-time",
    "sasl-authzid",
    "service-name",
    "socks4",
    "socks4a",
    "socks5",
    "socks5-gssapi-service",
    "socks5-hostname",
    "speed-limit",
    "speed-time",
    "stderr",
    "telnet-option",
    "tftp-blksize",
    "time-cond",
    "tls-max",
    "tls13-ciphers",
    "tlsauthtype",
    "tlspassword",
    "tlsuser",
    "trace",
    "trace-ascii",
    "trace-config",
    "unix-socket",
    "upload-file",
    "url",
    "url-query",
    "user",
    "user-agent",
    "variable",
    "vlan-priority",
    "write-out",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<Arg> {
        parse_args(words.iter().map(std::string::ToString::to_string))
    }

    fn opt(name: &str, value: Option<&str>) -> Arg {
        Arg::Opt {
            name: name.to_owned(),
            value: value.map(str::to_owned),
        }
    }

    #[test]
    fn long_value_table_is_sorted() {
        assert!(LONG_WITH_VALUE.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn short_bundles_follow_getopt() {
        assert_eq!(
            args(&["-sSo", "out.txt", "-XPOST", "-HAccept: */*", "url"]),
            [
                opt("output", Some("out.txt")),
                opt("request", Some("POST")),
                opt("header", Some("Accept: */*")),
                Arg::Positional("url".into()),
            ]
        );
        assert_eq!(args(&["-uadmin:pw"]), [opt("user", Some("admin:pw"))]);
        assert_eq!(
            args(&["-kLm", "10"]),
            [opt("insecure", None), opt("max-time", Some("10"))]
        );
    }

    #[test]
    fn long_options() {
        assert_eq!(
            args(&["--max-redirs", "5", "--compressed", "--request=PATCH", "u"]),
            [
                opt("max-redirs", Some("5")),
                opt("compressed", None),
                opt("request", Some("PATCH")),
                Arg::Positional("u".into()),
            ]
        );
    }

    #[test]
    fn a_lone_dash_is_positional() {
        assert_eq!(args(&["-"]), [Arg::Positional("-".into())]);
    }
}
