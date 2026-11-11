use rustman_core::domain::model::Auth;
use rustman_core::domain::request::{ApiKeyLocation, HttpMethod, graphql_body};
use rustman_core::domain::resolved::{FieldValue, ResolvedBody, ResolvedField, ResolvedRequest};
use rustman_core::jwt::make_jwt;
use rustman_core::url::percent_encode;

/// The request as a multi-line `curl` command, one option per line. Fails
/// only on GraphQL variables that aren't a JSON object.
pub fn export(req: &ResolvedRequest<'_>) -> Result<String, String> {
    let body = body_args(&req.body)?;
    let mut parts = Vec::new();
    // Without `--request`, curl sends any body as a POST.
    if req.method != HttpMethod::Get || !body.is_empty() {
        parts.push(format!("--request {}", req.method));
    }
    parts.push(format!("--url {}", shell_escape(&req.full_url())));
    parts.extend(auth_header(&req.auth));
    parts.extend(
        req.headers
            .iter()
            .map(|(name, value)| header_arg(name, value)),
    );
    if let Auth::Cookie { cookies } = &req.auth {
        parts.extend(cookie_arg(cookies));
    }
    parts.extend(body);
    Ok(format!("curl \\\n  {}", parts.join(" \\\n  ")))
}

/// An empty value is written `Name;`: curl reads `Name:` as "don't send it".
fn header_arg(name: &str, value: &str) -> String {
    let header = if value.trim().is_empty() {
        format!("{name};")
    } else {
        format!("{name}: {value}")
    };
    format!("--header {}", shell_escape(&header))
}

/// The option the auth adds, if any; a query API key is in the URL and a
/// cookie goes with the other cookies.
fn auth_header(auth: &Auth<String>) -> Option<String> {
    match auth {
        Auth::Bearer { token } if !token.is_empty() => {
            Some(header_arg("Authorization", &format!("Bearer {token}")))
        }
        Auth::Basic { user, pass } if !user.is_empty() => Some(format!(
            "--user {}",
            shell_escape(&format!("{user}:{pass}"))
        )),
        Auth::ApiKey {
            name,
            value,
            location: ApiKeyLocation::Header,
        } if !name.is_empty() && !value.is_empty() => Some(header_arg(name, value)),
        Auth::Jwt { secret, subject } if !secret.is_empty() => Some(header_arg(
            "Authorization",
            &format!("Bearer {}", make_jwt(subject, secret)),
        )),
        _ => None,
    }
}

/// A `Cookie` header value as one `--cookie`, `;` and `,` inside values
/// escaped so curl doesn't split there.
fn cookie_arg(cookies: &str) -> Option<String> {
    let mut joined = String::new();
    for pair in cookies.split(';') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() {
            continue;
        }
        if !joined.is_empty() {
            joined.push_str("; ");
        }
        joined.push_str(name);
        joined.push('=');
        for c in value.chars() {
            match c {
                ';' => joined.push_str("%3B"),
                ',' => joined.push_str("%2C"),
                c => joined.push(c),
            }
        }
    }
    (!joined.is_empty()).then(|| format!("--cookie {}", shell_escape(&joined)))
}

fn body_args(body: &ResolvedBody<'_>) -> Result<Vec<String>, String> {
    let raw = |text: &str| {
        (!text.trim().is_empty())
            .then(|| format!("--data-raw {}", shell_escape(text)))
            .into_iter()
            .collect()
    };
    Ok(match body {
        ResolvedBody::None => Vec::new(),
        ResolvedBody::Json(text) | ResolvedBody::Text(text) => raw(text),
        ResolvedBody::GraphQl { query, variables } => {
            if query.trim().is_empty() && variables.trim().is_empty() {
                Vec::new()
            } else {
                raw(&graphql_body(query, variables)?)
            }
        }
        ResolvedBody::Form(fields) => form_args(fields),
    })
}

/// Without any file the form goes as `--data-urlencode`, matching how
/// Rustman sends it.
fn form_args(fields: &[ResolvedField<'_>]) -> Vec<String> {
    let multipart = fields
        .iter()
        .any(|f| matches!(f.value, FieldValue::File { .. }));
    fields
        .iter()
        .filter(|f| !f.name.is_empty())
        .map(|field| {
            let (option, spec) = match (&field.value, multipart) {
                // curl encodes only the content, so the name goes encoded.
                (FieldValue::Text(text), false) => (
                    "--data-urlencode",
                    format!("{}={text}", percent_encode(&field.name)),
                ),
                // `--form` would read a leading `@` or `<`, or a `;type=`,
                // as instructions rather than text.
                (FieldValue::Text(text), true) => {
                    ("--form-string", format!("{}={text}", field.name))
                }
                // File data lives in the model as base64, with no path on
                // disk, so the command names the file for the user to fill
                // in, as Postman and Insomnia do.
                (FieldValue::File { file_name, .. }, _) => (
                    "--form",
                    format!("{}=@{}", field.name, file_name.unwrap_or("file")),
                ),
            };
            format!("{option} {}", shell_escape(&spec))
        })
        .collect()
}

fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }

    let safe = s.bytes().all(|b| {
        matches!(b,
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'/' | b':' | b'@'
            | b',' | b'+' | b'%' | b'='
        )
    });

    if safe {
        return s.to_string();
    }

    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(name: &str, value: &str) -> ResolvedField<'static> {
        ResolvedField {
            name: name.to_owned(),
            value: FieldValue::Text(value.to_owned()),
        }
    }

    fn file(name: &str, file_name: &'static str) -> ResolvedField<'static> {
        ResolvedField {
            name: name.to_owned(),
            value: FieldValue::File {
                file_name: Some(file_name),
                data: None,
                mime_type: None,
            },
        }
    }

    fn form(fields: Vec<ResolvedField<'static>>) -> String {
        export(&resolved(Auth::None, ResolvedBody::Form(fields))).unwrap()
    }

    #[test]
    fn form_data_fields_produce_form_flags() {
        let cmd = form(vec![text("name", "rustman"), file("avatar", "avatar.png")]);

        assert!(cmd.contains("--form-string name=rustman"), "cmd was: {cmd}");
        assert!(cmd.contains("--form avatar=@avatar.png"), "cmd was: {cmd}");
        assert!(!cmd.contains("--data-raw"));
    }

    #[test]
    fn text_fields_next_to_files_are_sent_literally() {
        let cmd = form(vec![
            text("note", "@not-a-file;type=x"),
            file("doc", "a.txt"),
        ]);

        assert!(
            cmd.contains("--form-string 'note=@not-a-file;type=x'"),
            "cmd was: {cmd}"
        );
        let parsed = crate::curl::parse(&cmd);
        assert_eq!(parsed.form[0].value, "@not-a-file;type=x");
        assert!(!parsed.form[0].is_file);
    }

    #[test]
    fn form_fields_without_a_name_are_skipped() {
        let cmd = form(vec![text("", "ignored")]);

        assert!(!cmd.contains("ignored"));
    }

    fn resolved(auth: Auth<String>, body: ResolvedBody<'static>) -> ResolvedRequest<'static> {
        ResolvedRequest {
            method: HttpMethod::Post,
            url: "https://x.io/a".into(),
            query: vec![("q".into(), "a b".into())],
            headers: [("X-Tag", "1")].into_iter().collect(),
            auth,
            body,
        }
    }

    #[test]
    fn a_resolved_request_exports_with_its_query_and_headers() {
        let req = resolved(Auth::None, ResolvedBody::Json("{\"a\":1}".into()));
        assert_eq!(
            export(&req).unwrap(),
            "curl \\\n  --request POST \\\n  --url 'https://x.io/a?q=a%20b' \\\n  \
             --header 'X-Tag: 1' \\\n  --data-raw '{\"a\":1}'"
        );
    }

    #[test]
    fn every_auth_scheme_exports_the_way_it_is_sent() {
        let cases = [
            (
                Auth::ApiKey {
                    name: "X-Key".into(),
                    value: "k".into(),
                    location: ApiKeyLocation::Header,
                },
                "--header 'X-Key: k'",
            ),
            (
                Auth::ApiKey {
                    name: "key".into(),
                    value: "k".into(),
                    location: ApiKeyLocation::Query,
                },
                "--url 'https://x.io/a?q=a%20b&key=k'",
            ),
            (
                Auth::Cookie {
                    cookies: "sid=1; theme=a;b".into(),
                },
                "--cookie 'sid=1; theme=a; b='",
            ),
            (
                Auth::Basic {
                    user: "me".into(),
                    pass: "pw".into(),
                },
                "--user me:pw",
            ),
            (
                Auth::Jwt {
                    secret: "s".into(),
                    subject: "u".into(),
                },
                "--header 'Authorization: Bearer ey",
            ),
        ];
        for (auth, expected) in cases {
            let cmd = export(&resolved(auth, ResolvedBody::None)).unwrap();
            assert!(cmd.contains(expected), "{expected} not in {cmd}");
        }
    }

    #[test]
    fn graphql_exports_the_document_it_sends() {
        let body = ResolvedBody::GraphQl {
            query: "{ me }".into(),
            variables: "{\"id\": 1}".into(),
        };
        let cmd = export(&resolved(Auth::None, body)).unwrap();
        assert!(
            cmd.contains(r#"--data-raw '{"query":"{ me }","variables":{"id":1}}'"#),
            "{cmd}"
        );

        let bad = ResolvedBody::GraphQl {
            query: "{ me }".into(),
            variables: "[1]".into(),
        };
        assert!(export(&resolved(Auth::None, bad)).is_err());
    }

    #[test]
    fn an_empty_form_exports_no_body() {
        let cmd = export(&resolved(Auth::None, ResolvedBody::Form(Vec::new()))).unwrap();
        assert!(!cmd.contains("--data") && !cmd.contains("--form"), "{cmd}");
    }
}
