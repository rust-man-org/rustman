//! Following redirects by hand, so what the user supplied never travels to a
//! host they didn't name.

use reqwest::header::{
    ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue, LOCATION,
    TRANSFER_ENCODING,
};
use reqwest::{Method, StatusCode, Url};

use crate::error::SendError;
use crate::response::ACCEPTED_ENCODINGS;

/// Redirects followed before giving up, as browsers and curl do.
pub(crate) const MAX_REDIRECTS: usize = 10;

/// Where `response` redirects to, if it is a redirect with a usable
/// `Location`.
pub(crate) fn target(response: &reqwest::Response) -> Option<Url> {
    let redirects = matches!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    );
    if !redirects {
        return None;
    }
    let location = response.headers().get(LOCATION)?.to_str().ok()?;
    let to = response.url().join(location).ok()?;
    matches!(to.scheme(), "http" | "https").then_some(to)
}

/// `request`, which got `status` from `from`, rewritten to go to `to`.
///
/// 307 and 308 resend the method and body; the others become a `GET`
/// without one, as browsers do. Leaving the origin drops every header the
/// request carried (its own `Content-Type` excepted when the body goes
/// along), so a token in any header stays with the host it was meant for.
/// HTTPS to plain HTTP is refused outright when there is anything beyond
/// the URL to expose.
pub(crate) fn follow(
    mut request: reqwest::Request,
    status: StatusCode,
    to: Url,
    has_payload: bool,
) -> Result<reqwest::Request, SendError> {
    let from = request.url();
    if from.scheme() == "https" && to.scheme() == "http" && has_payload {
        return Err(SendError::InsecureRedirect);
    }
    let same_origin = from.origin() == to.origin();
    let resends_body = matches!(
        status,
        StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT
    );
    if !resends_body {
        *request.body_mut() = None;
        if request.method() != Method::HEAD {
            *request.method_mut() = Method::GET;
        }
        let headers = request.headers_mut();
        for name in [
            CONTENT_TYPE,
            CONTENT_LENGTH,
            CONTENT_ENCODING,
            TRANSFER_ENCODING,
        ] {
            headers.remove(name);
        }
    }
    if !same_origin {
        let headers = request.headers_mut();
        let content_type = headers.remove(CONTENT_TYPE).filter(|_| resends_body);
        headers.clear();
        headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static(ACCEPTED_ENCODINGS),
        );
        if let Some(content_type) = content_type {
            headers.insert(CONTENT_TYPE, content_type);
        }
    }
    *request.url_mut() = to;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use reqwest::header::{AUTHORIZATION, HeaderName};

    use super::*;

    fn request(method: Method, url: &str) -> reqwest::Request {
        let mut request = reqwest::Request::new(method, Url::parse(url).unwrap());
        let headers = request.headers_mut();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        headers.insert(
            HeaderName::from_static("x-api-key"),
            HeaderValue::from_static("k"),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        *request.body_mut() = Some("{}".into());
        request
    }

    fn url(url: &str) -> Url {
        Url::parse(url).unwrap()
    }

    fn header_names(request: &reqwest::Request) -> Vec<&str> {
        let mut names: Vec<&str> = request.headers().keys().map(HeaderName::as_str).collect();
        names.sort_unstable();
        names
    }

    #[test]
    fn leaving_the_origin_drops_every_supplied_header() {
        let next = follow(
            request(Method::GET, "https://api.test/a"),
            StatusCode::FOUND,
            url("https://elsewhere.test/b"),
            true,
        )
        .unwrap();
        assert_eq!(header_names(&next), ["accept-encoding"]);
        assert_eq!(next.url().as_str(), "https://elsewhere.test/b");
    }

    #[test]
    fn another_port_is_another_origin() {
        let next = follow(
            request(Method::GET, "http://127.0.0.1:1/a"),
            StatusCode::FOUND,
            url("http://127.0.0.1:2/a"),
            true,
        )
        .unwrap();
        assert!(next.headers().get(AUTHORIZATION).is_none());
    }

    #[test]
    fn the_same_origin_keeps_its_headers() {
        let next = follow(
            request(Method::GET, "https://api.test/a"),
            StatusCode::MOVED_PERMANENTLY,
            url("https://api.test/b"),
            true,
        )
        .unwrap();
        assert_eq!(header_names(&next), ["authorization", "x-api-key"]);
    }

    #[test]
    fn see_other_turns_a_post_into_a_get_without_a_body() {
        let next = follow(
            request(Method::POST, "https://api.test/a"),
            StatusCode::SEE_OTHER,
            url("https://api.test/b"),
            true,
        )
        .unwrap();
        assert_eq!(next.method(), Method::GET);
        assert!(next.body().is_none());
        assert!(next.headers().get(CONTENT_TYPE).is_none());
    }

    #[test]
    fn a_temporary_redirect_resends_the_method_body_and_content_type() {
        let next = follow(
            request(Method::POST, "https://api.test/a"),
            StatusCode::TEMPORARY_REDIRECT,
            url("https://other.test/b"),
            true,
        )
        .unwrap();
        assert_eq!(next.method(), Method::POST);
        assert!(next.body().is_some());
        assert_eq!(header_names(&next), ["accept-encoding", "content-type"]);
    }

    #[test]
    fn https_to_http_is_refused_when_there_is_something_to_expose() {
        let downgrade = |has_payload| {
            follow(
                request(Method::GET, "https://api.test/a"),
                StatusCode::FOUND,
                url("http://api.test/a"),
                has_payload,
            )
        };
        assert!(matches!(downgrade(true), Err(SendError::InsecureRedirect)));
        assert!(downgrade(false).is_ok());
    }
}
