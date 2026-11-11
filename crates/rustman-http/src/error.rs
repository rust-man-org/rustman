use std::error::Error;

/// Why a request couldn't be sent or its response read. The messages are
/// what the user sees, so they never echo the (possibly secret-bearing) URL.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SendError {
    #[error("Invalid header: {0}")]
    HeaderName(String),
    /// Holds the header's name: the value is often a credential.
    #[error("Invalid value for header {0}")]
    HeaderValue(String),
    #[error("Invalid API key header name: {0}")]
    ApiKeyHeader(String),
    #[error("{0}")]
    GraphQl(String),
    #[error("File field '{0}' has no data — pick a file first")]
    MissingFile(String),
    #[error("Base64 decode: {0}")]
    FileData(#[from] base64::DecodeError),
    #[error("{0}")]
    Mime(reqwest::Error),
    #[error("{}: {}", category(.0), root_cause(.0))]
    Transport(reqwest::Error),
    #[error("Read body: {0}")]
    ReadBody(reqwest::Error),
    #[error("Read body: error decoding response body: {0}")]
    Decode(std::io::Error),
    #[error("Too many redirects (more than {})", crate::redirect::MAX_REDIRECTS)]
    TooManyRedirects,
    #[error(
        "Refused a redirect from HTTPS to plain HTTP, which would expose the request's headers, auth or body"
    )]
    InsecureRedirect,
}

fn category(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "Request timed out"
    } else if e.is_connect() {
        "Connection failed"
    } else if e.is_redirect() {
        "Too many redirects"
    } else if e.is_body() || e.is_decode() {
        "Failed to read response"
    } else {
        "Request failed"
    }
}

/// The innermost message of `err`'s source chain. reqwest's own message stops
/// at "error sending request for url (…)", which hides the actual cause (TLS
/// trust, DNS, refused connection) and repeats the URL.
fn root_cause(err: &dyn Error) -> String {
    let mut cur = err;
    while let Some(source) = cur.source() {
        cur = source;
    }
    cur.to_string()
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use super::root_cause;

    #[derive(Debug)]
    struct Chain(&'static str, Option<Box<Chain>>);

    impl fmt::Display for Chain {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    impl std::error::Error for Chain {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.1.as_deref().map(|e| e as &dyn std::error::Error)
        }
    }

    #[test]
    fn walks_to_leaf_cause() {
        let chain = Chain(
            "error sending request",
            Some(Box::new(Chain(
                "client error (Connect)",
                Some(Box::new(Chain(
                    "invalid peer certificate: UnknownIssuer",
                    None,
                ))),
            ))),
        );
        assert_eq!(
            root_cause(&chain),
            "invalid peer certificate: UnknownIssuer"
        );
    }

    #[test]
    fn single_error_returns_itself() {
        assert_eq!(root_cause(&Chain("boom", None)), "boom");
    }
}
