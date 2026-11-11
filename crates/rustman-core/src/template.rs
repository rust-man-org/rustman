//! `{{variable}}` expansion and the URL a request is actually sent to.

use std::net::IpAddr;
use std::ops::Range;

use crate::domain::environment::AppEnvironment;

/// `text` with every `{{name}}` that `env` defines replaced by its value.
/// Whitespace inside the braces is ignored; unknown names and unclosed
/// braces stay literal. As in Postman, a name holds no braces, so a stray
/// `{{` can't swallow the placeholder after it, and `{{{a}}}` is `{`, `a`'s
/// value, then `}`.
pub fn substitute(text: &str, env: Option<&AppEnvironment>) -> String {
    let Some(env) = env else {
        return text.to_owned();
    };
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((span, name)) = next_placeholder(rest) {
        out.push_str(&rest[..span.start]);
        match env.variables.get(name) {
            Some(value) => out.push_str(value),
            None => out.push_str(&rest[span.clone()]),
        }
        rest = &rest[span.end..];
    }
    out.push_str(rest);
    out
}

/// Whether `text` is only `{{name}}` placeholders, at least one, and
/// whitespace: a reference to values kept elsewhere rather than a value.
pub fn is_only_placeholders(text: &str) -> bool {
    let mut rest = text;
    let mut any = false;
    while let Some((span, name)) = next_placeholder(rest) {
        if name.is_empty() || !rest[..span.start].trim().is_empty() {
            return false;
        }
        any = true;
        rest = &rest[span.end..];
    }
    any && rest.trim().is_empty()
}

/// Where the first `{{name}}` in `text` is, and its name, trimmed.
fn next_placeholder(text: &str) -> Option<(Range<usize>, &str)> {
    let mut from = 0;
    while let Some(i) = text[from..].find("{{") {
        let start = from + i;
        let after = &text[start + 2..];
        let name_len = after.find(['{', '}']).unwrap_or(after.len());
        if after[name_len..].starts_with("}}") {
            return Some((start..start + name_len + 4, after[..name_len].trim()));
        }
        from = start + 1;
    }
    None
}

/// The URL as sent: variables expanded first, then `http://` (localhost,
/// loopback and private addresses) or `https://` added when no scheme is
/// left.
///
/// Expansion has to come first because a variable may carry the scheme
/// itself (`{{base}}/x` with `base = https://api.example.com`).
pub fn resolve_url(raw_url: &str, env: Option<&AppEnvironment>) -> String {
    let url = substitute(raw_url, env);
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }
    for scheme in ["http://", "https://", "ws://", "wss://"] {
        if let Some(prefix) = url.get(..scheme.len())
            && prefix.eq_ignore_ascii_case(scheme)
        {
            return format!("{scheme}{}", &url[scheme.len()..]);
        }
    }
    let scheme = if is_local(host(url)) { "http" } else { "https" };
    format!("{scheme}://{url}")
}

/// The host of a URL that has no scheme, without its user, port or the
/// brackets around an IPv6 address.
fn host(url: &str) -> &str {
    let authority = url.split(['/', '?', '#']).next().unwrap_or(url);
    let host_and_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    match host_and_port.strip_prefix('[') {
        Some(bracketed) => bracketed.split_once(']').map_or(bracketed, |(ip, _)| ip),
        None => host_and_port.split(':').next().unwrap_or(host_and_port),
    }
}

/// `localhost` and its subdomains, loopback addresses, and the private
/// ranges `10/8`, `172.16/12`, `192.168/16` and `fc00::/7`.
fn is_local(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private(),
        Ok(IpAddr::V6(ip)) => ip.is_loopback() || ip.is_unique_local(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(vars: &[(&str, &str)]) -> AppEnvironment {
        AppEnvironment {
            id: "test".to_owned(),
            name: "test".to_owned(),
            variables: vars
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            is_active: true,
        }
    }

    #[test]
    fn only_placeholders_make_a_reference() {
        for reference in ["{{token}}", " {{ a }} {{b}} ", "{{a}}{{b}}"] {
            assert!(is_only_placeholders(reference), "{reference:?}");
        }
        for value in [
            "",
            "  ",
            "s3cret",
            "Bearer {{token}}",
            "{{a}}x",
            "{{}}",
            "{{a}",
            "{{{a}}}",
        ] {
            assert!(!is_only_placeholders(value), "{value:?}");
        }
    }

    #[test]
    fn variable_supplied_host_gets_one_scheme() {
        let e = env(&[
            ("API_BACKEND", "api.www.visitdenmark.com"),
            ("GRAPHQL", "e9e32dbc-0bae-4278-b349-1bcdecb53f7d"),
        ]);
        assert_eq!(
            resolve_url("{{API_BACKEND}}/graphql/{{GRAPHQL}}", Some(&e)),
            "https://api.www.visitdenmark.com/graphql/e9e32dbc-0bae-4278-b349-1bcdecb53f7d"
        );
    }

    #[test]
    fn scheme_inside_the_variable_is_honoured() {
        let e = env(&[
            ("API_BACKEND", "https://api.www.visitdenmark.com"),
            ("GRAPHQL", "abc"),
        ]);
        assert_eq!(
            resolve_url("{{API_BACKEND}}/graphql/{{GRAPHQL}}", Some(&e)),
            "https://api.www.visitdenmark.com/graphql/abc"
        );
    }

    #[test]
    fn schemeless_urls_get_a_scheme_by_host() {
        assert_eq!(
            resolve_url("api.example.com/x", None),
            "https://api.example.com/x"
        );
        assert_eq!(
            resolve_url("localhost:8899/x", None),
            "http://localhost:8899/x"
        );
        assert_eq!(resolve_url("10.0.0.7/x", None), "http://10.0.0.7/x");
        assert_eq!(resolve_url("  wss://x.io  ", None), "wss://x.io");
    }

    #[test]
    fn a_typed_scheme_is_recognised_in_any_case() {
        for (typed, sent) in [
            ("HTTPS://x.io", "https://x.io"),
            ("Http://localhost", "http://localhost"),
            ("WSS://x.io/ws", "wss://x.io/ws"),
            ("https://x.io/A", "https://x.io/A"),
        ] {
            assert_eq!(resolve_url(typed, None), sent);
        }
        assert_eq!(resolve_url("дом.рф", None), "https://дом.рф");
    }

    #[test]
    fn only_local_hosts_get_plain_http() {
        for local in [
            "LOCALHOST:3000",
            "api.localhost/x",
            "127.1.2.3",
            "192.168.1.20:80/x",
            "172.31.255.255",
            "me:pw@10.1.1.1/x",
            "[::1]/x",
            "[fd12::7]:8080",
        ] {
            assert!(resolve_url(local, None).starts_with("http://"), "{local}");
        }
        for public in [
            "172.32.0.1",
            "192.169.0.1",
            "11.0.0.1",
            "localhost.example.com",
            "127.example.com",
            "[2001:db8::1]",
            "10.0.0.1@evil.example",
        ] {
            assert!(
                resolve_url(public, None).starts_with("https://"),
                "{public}"
            );
        }
    }
}
