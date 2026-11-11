use serde_json::Value;

use crate::curl::{CurlForm, Headers, ParsedCurl, tokenize};

const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

/// Whether `input` starts with an HTTPie (`http`, `https`) or xh command.
/// Only a short prefix is looked at, so huge pastes cost nothing.
pub fn is_httpie_command(input: &str) -> bool {
    let head = input.trim_start();
    ["http", "https", "xh"].iter().any(|program| {
        head.get(..program.len())
            .is_some_and(|word| word.eq_ignore_ascii_case(program))
            && head[program.len()..].starts_with([' ', '\t'])
    })
}

pub fn parse(input: &str) -> ParsedCurl {
    let normalized = input
        .replace("\\\r\n", " ")
        .replace("\\\n", " ")
        .replace('\r', "");
    let trimmed = normalized.trim();
    let (prog, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    let scheme = if prog.eq_ignore_ascii_case("https") {
        "https"
    } else {
        "http"
    };

    let mut method: Option<String> = None;
    let mut url: Option<String> = None;
    let mut headers = Headers::default();
    let mut query: Vec<(String, String)> = Vec::new();
    let mut json_fields = serde_json::Map::new();
    let mut form_fields: Vec<CurlForm> = Vec::new();
    let mut form_mode = false;
    let mut auth: Option<String> = None;
    let mut auth_type = "basic".to_owned();
    let mut raw_body: Option<String> = None;

    let mut tokens = tokenize(rest).into_iter();
    while let Some(token) = tokens.next() {
        match token.as_str() {
            "-f" | "--form" => form_mode = true,
            "-j" | "--json" => form_mode = false,
            "-a" | "--auth" => auth = tokens.next(),
            "-A" | "--auth-type" => {
                if let Some(t) = tokens.next() {
                    auth_type = t.to_lowercase();
                }
            }
            "--raw" => raw_body = tokens.next(),
            _ if token.starts_with("--auth-type=") => {
                auth_type = token["--auth-type=".len()..].to_lowercase();
            }
            _ if token.starts_with("--auth=") => auth = Some(token["--auth=".len()..].to_owned()),
            _ if token.starts_with("--raw=") => raw_body = Some(token["--raw=".len()..].to_owned()),
            _ if token.starts_with('-') => {}
            _ if method.is_none()
                && url.is_none()
                && METHODS.contains(&token.to_uppercase().as_str()) =>
            {
                method = Some(token.to_uppercase());
            }
            _ if url.is_none() => url = Some(token),
            _ => classify_item(
                &token,
                form_mode,
                &mut headers,
                &mut query,
                &mut json_fields,
                &mut form_fields,
            ),
        }
    }

    let url = url.map(|u| qualify_url(u, scheme, &query));

    let body = raw_body.or_else(|| {
        if json_fields.is_empty() || form_mode {
            None
        } else {
            serde_json::to_string_pretty(&Value::Object(json_fields.clone())).ok()
        }
    });

    if let Some(spec) = auth {
        if auth_type == "bearer" {
            headers.insert("Authorization".to_owned(), format!("Bearer {spec}"));
        } else {
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD.encode(spec.as_bytes());
            headers.insert("Authorization".to_owned(), format!("Basic {encoded}"));
        }
    }

    let method = method.or_else(|| {
        if body.is_some() || !form_fields.is_empty() {
            Some("POST".to_owned())
        } else {
            None
        }
    });

    ParsedCurl {
        method,
        url,
        header: headers,
        body,
        form: form_fields,
        ..ParsedCurl::default()
    }
}

fn classify_item(
    token: &str,
    form_mode: bool,
    headers: &mut Headers,
    query: &mut Vec<(String, String)>,
    json_fields: &mut serde_json::Map<String, Value>,
    form_fields: &mut Vec<CurlForm>,
) {
    let Some((key, sep, value)) = split_item(token) else {
        return;
    };
    match sep {
        "==" => query.push((key, value)),
        ":=" => {
            let parsed = serde_json::from_str::<Value>(&value).unwrap_or(Value::String(value));
            json_fields.insert(key, parsed);
        }
        "=" => {
            if form_mode {
                form_fields.push(CurlForm {
                    key,
                    value,
                    is_file: false,
                    content_type: None,
                });
            } else {
                json_fields.insert(key, Value::String(value));
            }
        }
        "@" => form_fields.push(CurlForm {
            key,
            value,
            is_file: true,
            content_type: None,
        }),
        ":" => headers.push(key, value),
        _ => {}
    }
}

fn split_item(token: &str) -> Option<(String, &'static str, String)> {
    let mut best: Option<(usize, &'static str)> = None;
    for sep in ["==", ":=", "=", "@", ":"] {
        if let Some(idx) = token.find(sep) {
            let better = match best {
                None => true,
                Some((best_idx, best_sep)) => {
                    idx < best_idx || (idx == best_idx && sep.len() > best_sep.len())
                }
            };
            if better {
                best = Some((idx, sep));
            }
        }
    }
    best.filter(|(idx, _)| *idx > 0).map(|(idx, sep)| {
        (
            token[..idx].to_owned(),
            sep,
            token[idx + sep.len()..].to_owned(),
        )
    })
}

fn qualify_url(url: String, scheme: &str, query: &[(String, String)]) -> String {
    let mut base = if let Some(rest) = url.strip_prefix(':') {
        format!("{scheme}://localhost:{rest}")
    } else if url.contains("://") {
        url
    } else {
        format!("{scheme}://{url}")
    };
    if base.ends_with(':') {
        base.pop();
    }
    if !query.is_empty() {
        let qs: Vec<String> = query.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let joiner = if base.contains('?') { '&' } else { '?' };
        base = format!("{base}{joiner}{}", qs.join("&"));
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_httpie_commands() {
        assert!(is_httpie_command("http GET example.org"));
        assert!(is_httpie_command("https example.org"));
        assert!(is_httpie_command("xh :3000/api"));
        assert!(!is_httpie_command("http://example.org"));
        assert!(!is_httpie_command("curl example.org"));
    }

    #[test]
    fn parses_method_url_query_and_headers() {
        let parsed = parse("http GET example.org/search q==rust X-API-Key:secret");
        assert_eq!(parsed.method.as_deref(), Some("GET"));
        assert_eq!(
            parsed.url.as_deref(),
            Some("http://example.org/search?q=rust")
        );
        assert_eq!(parsed.header.get("X-API-Key"), Some("secret"));
    }

    #[test]
    fn json_fields_imply_post_and_build_body() {
        let parsed = parse("http example.org name=Alice age:=30 active:=true");
        assert_eq!(parsed.method.as_deref(), Some("POST"));
        let body: Value = serde_json::from_str(parsed.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["name"], "Alice");
        assert_eq!(body["age"], 30);
        assert_eq!(body["active"], true);
    }

    #[test]
    fn form_mode_collects_fields_and_files() {
        let parsed = parse("http -f POST example.org name=Bob avatar@/tmp/pic.png");
        assert_eq!(parsed.method.as_deref(), Some("POST"));
        assert!(parsed.body.is_none());
        assert_eq!(parsed.form.len(), 2);
        assert!(!parsed.form[0].is_file);
        assert!(parsed.form[1].is_file);
        assert_eq!(parsed.form[1].value, "/tmp/pic.png");
    }

    #[test]
    fn localhost_shorthand_and_https_prog() {
        let parsed = parse("http :3000/users");
        assert_eq!(parsed.url.as_deref(), Some("http://localhost:3000/users"));
        let parsed = parse("https example.org");
        assert_eq!(parsed.url.as_deref(), Some("https://example.org"));
    }

    #[test]
    fn bearer_and_basic_auth() {
        let parsed = parse("http -A bearer -a TOKEN123 example.org");
        assert_eq!(parsed.header.get("Authorization"), Some("Bearer TOKEN123"));
        let parsed = parse("http -a user:pass example.org");
        let auth = parsed.header.get("Authorization").unwrap();
        assert!(auth.starts_with("Basic "));
    }
}
