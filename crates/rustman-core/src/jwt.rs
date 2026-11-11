//! HS256 JSON Web Tokens, the one kind Rustman signs.

use base64::{Engine as _, engine::general_purpose};
use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use sha2::Sha256;

use crate::time::now_millis;

/// How long a signed token is valid.
pub const JWT_LIFETIME_SECS: u64 = 3600;

/// An HS256 JWT for `subject`, signed with `secret`, issued at `issued_at`
/// (seconds since the Unix epoch) and valid for an hour.
pub fn make_jwt_at(subject: &str, secret: &str, issued_at: u64) -> String {
    #[derive(Serialize)]
    struct Claims<'a> {
        sub: &'a str,
        iat: u64,
        exp: u64,
    }

    let claims = Claims {
        sub: subject,
        iat: issued_at,
        exp: issued_at + JWT_LIFETIME_SECS,
    };
    let payload = serde_json::to_vec(&claims).expect("plain data always serializes");
    let signing_input = format!(
        "{}.{}",
        base64url(br#"{"alg":"HS256","typ":"JWT"}"#),
        base64url(&payload)
    );
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes keys of any length");
    mac.update(signing_input.as_bytes());
    let signature = base64url(&mac.finalize().into_bytes());
    format!("{signing_input}.{signature}")
}

/// [`make_jwt_at`], issued now.
pub fn make_jwt(subject: &str, secret: &str) -> String {
    make_jwt_at(
        subject,
        secret,
        u64::try_from(now_millis() / 1000).unwrap_or_default(),
    )
}

fn base64url(data: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_deterministic_for_a_given_time() {
        let token = make_jwt_at("user-42", "secret", 1_700_000_000);
        assert_eq!(token, make_jwt_at("user-42", "secret", 1_700_000_000));
        assert_ne!(token, make_jwt_at("user-42", "other", 1_700_000_000));
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn claims_are_valid_json_for_any_subject() {
        let token = make_jwt_at(r#"a"b\c"#, "secret", 1_700_000_000);
        let payload = token.split('.').nth(1).unwrap();
        let claims: serde_json::Value =
            serde_json::from_slice(&general_purpose::URL_SAFE_NO_PAD.decode(payload).unwrap())
                .unwrap();
        assert_eq!(claims["sub"], r#"a"b\c"#);
        assert_eq!(
            claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(),
            JWT_LIFETIME_SECS
        );
    }
}
