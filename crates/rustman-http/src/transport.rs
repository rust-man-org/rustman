use std::time::Instant;

use reqwest::Client;
use rustman_core::domain::resolved::ResolvedRequest;
use rustman_core::domain::response::Redirect;
use rustman_core::run::{HttpTransport, Response, SendOptions, TransportError};

use crate::error::SendError;
use crate::request::Built;
use crate::{redirect, request, response};

/// [`HttpTransport`] over a reqwest [`Client`], whose cookie store, pool and
/// TLS settings every request shares.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: Client,
}

impl ReqwestTransport {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl HttpTransport for ReqwestTransport {
    async fn send(
        &self,
        request: ResolvedRequest<'_>,
        options: &SendOptions,
    ) -> Result<Response, TransportError> {
        exchange(&self.client, request, options)
            .await
            .map_err(|e| TransportError::new(e.to_string()))
    }
}

/// Sends `request`, following redirects by hand (see [`redirect::follow`]),
/// all within one timeout.
async fn exchange(
    client: &Client,
    request: ResolvedRequest<'_>,
    options: &SendOptions,
) -> Result<Response, SendError> {
    let deadline = Instant::now() + options.timeout;
    let Built {
        request: mut next,
        has_payload,
    } = request::build(client, request)?;
    let mut redirects = Vec::new();
    loop {
        // A streamed body (a multipart upload) can't be sent twice, so its
        // redirect is returned rather than followed.
        let again = next.try_clone();
        *next.timeout_mut() = Some(deadline.saturating_duration_since(Instant::now()));
        let response = client.execute(next).await.map_err(SendError::Transport)?;
        let followed = again.zip(redirect::target(&response));
        let Some((again, to)) = followed else {
            let mut out = response::read(response, options.body_limit).await?;
            out.redirects = redirects;
            return Ok(out);
        };
        if redirects.len() == redirect::MAX_REDIRECTS {
            return Err(SendError::TooManyRedirects);
        }
        redirects.push(Redirect {
            status: response.status().as_u16(),
            to: to.to_string(),
        });
        next = redirect::follow(again, response.status(), to, has_payload)?;
    }
}
