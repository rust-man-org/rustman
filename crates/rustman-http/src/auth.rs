use base64::{Engine as _, engine::general_purpose};
use reqwest::header::{AUTHORIZATION, COOKIE, HeaderName, HeaderValue};
use rustman_core::domain::model::Auth;
use rustman_core::domain::request::ApiKeyLocation;
use rustman_core::jwt::make_jwt;

use crate::error::SendError;

/// The header `auth` adds. Auth whose fields are empty adds no header,
/// rather than an empty `Bearer `.
pub(crate) fn auth_header(
    auth: Auth<String>,
) -> Result<Option<(HeaderName, HeaderValue)>, SendError> {
    let header = match auth {
        Auth::Bearer { token } if !token.is_empty() => (AUTHORIZATION, format!("Bearer {token}")),
        Auth::Basic { user, pass } if !user.is_empty() => {
            let encoded = general_purpose::STANDARD.encode(format!("{user}:{pass}"));
            (AUTHORIZATION, format!("Basic {encoded}"))
        }
        Auth::ApiKey {
            name,
            value,
            location: ApiKeyLocation::Header,
        } if !name.is_empty() && !value.is_empty() => {
            let header = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| SendError::ApiKeyHeader(name))?;
            (header, value)
        }
        Auth::Cookie { cookies } if !cookies.is_empty() => (COOKIE, cookies),
        Auth::Jwt { secret, subject } if !secret.is_empty() => (
            AUTHORIZATION,
            format!("Bearer {}", make_jwt(&subject, &secret)),
        ),
        _ => return Ok(None),
    };
    let (name, value) = header;
    match HeaderValue::try_from(value) {
        Ok(value) => Ok(Some((name, value))),
        Err(_) => Err(SendError::HeaderValue(name.as_str().to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use rustman_core::domain::collection::SavedRequest;
    use rustman_core::domain::environment::AppEnvironment;
    use rustman_core::domain::request::AuthType;

    use super::*;

    fn env_with(vars: &[(&str, &str)]) -> AppEnvironment {
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
    fn auth_fields_take_environment_variables() {
        let env = env_with(&[
            ("token", "abc"),
            ("user", "bob"),
            ("pass", "pw"),
            ("sid", "s1"),
        ]);
        let mut req = SavedRequest::new_in("c".to_owned(), "r".to_owned());
        let header = |req: &SavedRequest| {
            auth_header(req.resolve(Some(&env)).auth)
                .unwrap()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_owned()))
        };

        req.auth_type = AuthType::Bearer;
        req.bearer_token = "{{token}}".to_owned();
        assert_eq!(
            header(&req),
            Some(("authorization".to_owned(), "Bearer abc".to_owned()))
        );

        req.auth_type = AuthType::Basic;
        req.basic_user = "{{user}}".to_owned();
        req.basic_pass = "{{pass}}".to_owned();
        assert_eq!(
            header(&req),
            Some(("authorization".to_owned(), "Basic Ym9iOnB3".to_owned()))
        );

        req.auth_type = AuthType::Cookie;
        req.cookie_string = "sid={{sid}}".to_owned();
        assert_eq!(
            header(&req),
            Some(("cookie".to_owned(), "sid=s1".to_owned()))
        );

        req.auth_type = AuthType::Bearer;
        req.bearer_token = "{{missing}}".to_owned();
        let empty = env_with(&[("missing", "")]);
        assert!(
            auth_header(req.resolve(Some(&empty)).auth)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn an_api_key_without_a_value_sends_no_header() {
        let mut req = SavedRequest::new_in("c".to_owned(), "r".to_owned());
        req.auth_type = AuthType::ApiKey;
        req.api_key_name = "X-Api-Key".to_owned();
        assert!(auth_header(req.resolve(None).auth).unwrap().is_none());
        req.api_key_value = "k".to_owned();
        assert!(auth_header(req.resolve(None).auth).unwrap().is_some());
    }

    #[test]
    fn a_bad_auth_value_names_the_header_not_the_secret() {
        let mut req = SavedRequest::new_in("c".to_owned(), "r".to_owned());
        req.auth_type = AuthType::Bearer;
        req.bearer_token = "secret\nvalue".to_owned();
        let err = auth_header(req.resolve(None).auth).unwrap_err().to_string();
        assert_eq!(err, "Invalid value for header authorization");
    }
}
