use std::io;
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder, ZlibDecoder, ZstdDecoder};
use futures_util::TryStreamExt;
use reqwest::header::{CONTENT_ENCODING, HeaderMap};
use rustman_core::domain::headers::Headers;
use rustman_core::run::{BodySize, Response};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::io::StreamReader;

use crate::error::SendError;

/// The `Accept-Encoding` sent unless the request sets its own: every
/// encoding [`read`] decodes.
pub(crate) const ACCEPTED_ENCODINGS: &str = "zstd,gzip,deflate,br";

/// How much more decoded body is read at a time.
const READ_CHUNK: usize = 16 * 1024;

#[derive(Debug, Clone, Copy)]
enum Encoding {
    Gzip,
    /// HTTP's `deflate` is zlib-wrapped.
    Deflate,
    Brotli,
    Zstd,
}

impl Encoding {
    /// The encoding `Content-Encoding` names, when it is a single one Rustman
    /// decodes. Anything else is kept as received.
    fn of(headers: &HeaderMap) -> Option<Self> {
        match headers.get(CONTENT_ENCODING)?.as_bytes() {
            b"gzip" => Some(Self::Gzip),
            b"deflate" => Some(Self::Deflate),
            b"br" => Some(Self::Brotli),
            b"zstd" => Some(Self::Zstd),
            _ => None,
        }
    }
}

/// Reads the response, decoding the body and keeping at most `limit` bytes of
/// it. Reading stops just past the limit, so a small body that decodes to
/// gigabytes is never held whole. The headers are kept as received.
pub(crate) async fn read(response: reqwest::Response, limit: usize) -> Result<Response, SendError> {
    let status = response.status();
    let headers: Headers = response
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str(), String::from_utf8_lossy(value.as_bytes())))
        .collect();
    let (mut body, size) = match Encoding::of(response.headers()) {
        None => read_plain(response, limit).await?,
        Some(encoding) => read_decoded(response, encoding, limit).await?,
    };
    body.truncate(limit);
    body.shrink_to_fit();
    Ok(Response {
        status: status.as_u16(),
        reason: status.canonical_reason().unwrap_or_default().to_owned(),
        headers,
        body,
        truncated: size.decoded > limit,
        size,
        redirects: Vec::new(),
    })
}

async fn read_plain(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<(Vec<u8>, BodySize), SendError> {
    let expected = response
        .content_length()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0);
    let mut body = Vec::with_capacity(expected.min(limit));
    while body.len() <= limit
        && let Some(chunk) = response.chunk().await.map_err(SendError::ReadBody)?
    {
        body.extend_from_slice(&chunk);
    }
    let size = BodySize {
        wire: body.len(),
        decoded: body.len(),
    };
    Ok((body, size))
}

async fn read_decoded(
    response: reqwest::Response,
    encoding: Encoding,
    limit: usize,
) -> Result<(Vec<u8>, BodySize), SendError> {
    let wire = AtomicUsize::new(0);
    let input = StreamReader::new(
        response
            .bytes_stream()
            .inspect_ok(|chunk| {
                wire.fetch_add(chunk.len(), Ordering::Relaxed);
            })
            .map_err(io::Error::other),
    );
    let decoded = match encoding {
        Encoding::Gzip => drain(GzipDecoder::new(input), limit).await,
        Encoding::Deflate => drain(ZlibDecoder::new(input), limit).await,
        Encoding::Brotli => drain(BrotliDecoder::new(input), limit).await,
        Encoding::Zstd => {
            let mut decoder = ZstdDecoder::new(input);
            decoder.multiple_members(true);
            drain(decoder, limit).await
        }
    };
    let wire = wire.into_inner();
    let body = match decoded {
        Ok(body) => body,
        // A HEAD or 304 response is labelled with the encoding but has no
        // body to decode.
        Err(_) if wire == 0 => Vec::new(),
        Err(e) => {
            return Err(match e.downcast::<reqwest::Error>() {
                Ok(e) => SendError::ReadBody(e),
                Err(e) => SendError::Decode(e),
            });
        }
    };
    let size = BodySize {
        wire,
        decoded: body.len(),
    };
    Ok((body, size))
}

/// Everything `reader` yields, stopping once past `limit` bytes.
async fn drain(reader: impl AsyncRead, limit: usize) -> io::Result<Vec<u8>> {
    let mut reader = pin!(reader);
    let mut body = Vec::new();
    while body.len() <= limit {
        body.reserve(READ_CHUNK);
        if reader.read_buf(&mut body).await? == 0 {
            break;
        }
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use async_compression::tokio::bufread::{BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder};
    use futures_util::stream;
    use reqwest::Body;

    use super::*;

    fn response(encoding: Option<&str>, chunks: Vec<Vec<u8>>) -> reqwest::Response {
        let chunks: Vec<Result<Vec<u8>, io::Error>> = chunks.into_iter().map(Ok).collect();
        let mut builder = http::Response::builder();
        if let Some(encoding) = encoding {
            builder = builder.header("content-encoding", encoding);
        }
        reqwest::Response::from(
            builder
                .body(Body::wrap_stream(stream::iter(chunks)))
                .unwrap(),
        )
    }

    async fn encoded(encoding: &str, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        match encoding {
            "gzip" => GzipEncoder::new(data).read_to_end(&mut out).await,
            "deflate" => ZlibEncoder::new(data).read_to_end(&mut out).await,
            "br" => BrotliEncoder::new(data).read_to_end(&mut out).await,
            "zstd" => ZstdEncoder::new(data).read_to_end(&mut out).await,
            other => unreachable!("{other}"),
        }
        .unwrap();
        out
    }

    #[tokio::test]
    async fn a_small_body_is_read_whole() {
        let out = read(
            response(None, vec![b"hello ".to_vec(), b"world".to_vec()]),
            64,
        )
        .await
        .unwrap();
        assert_eq!(out.body, b"hello world");
        assert_eq!(
            out.size,
            BodySize {
                wire: 11,
                decoded: 11
            }
        );
        assert!(!out.truncated);
    }

    #[tokio::test]
    async fn reading_stops_past_the_limit() {
        let out = read(response(None, vec![vec![b'a'; 100]; 50]), 250)
            .await
            .unwrap();
        assert_eq!(out.body.len(), 250);
        assert!(out.truncated, "the body is marked partial");
        assert!(out.size.wire <= 300, "read stopped early: {:?}", out.size);
    }

    #[tokio::test]
    async fn every_accepted_encoding_is_decoded_and_its_wire_facts_kept() {
        let text = br#"{"hello":"world","pad":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
        for encoding in ACCEPTED_ENCODINGS.split(',') {
            let wire = encoded(encoding, text).await;
            let halves = wire
                .chunks(wire.len() / 2 + 1)
                .map(<[u8]>::to_vec)
                .collect();
            let out = read(response(Some(encoding), halves), 1024).await.unwrap();
            assert_eq!(out.body, text, "{encoding}");
            assert_eq!(
                out.size,
                BodySize {
                    wire: wire.len(),
                    decoded: text.len()
                },
                "{encoding}"
            );
            assert_eq!(out.headers.get("content-encoding"), Some(encoding));
        }
    }

    #[tokio::test]
    async fn a_compression_bomb_is_read_only_to_the_limit() {
        let wire = encoded("gzip", &vec![0; 4_000_000]).await;
        let chunks = wire.chunks(64).map(<[u8]>::to_vec).collect();
        let out = read(response(Some("gzip"), chunks), 1000).await.unwrap();
        assert!(out.truncated);
        assert_eq!(out.body.len(), 1000);
        assert!(
            out.size.wire < wire.len(),
            "read stopped early: {:?}",
            out.size
        );
    }

    #[tokio::test]
    async fn an_encoded_empty_body_is_empty() {
        let out = read(response(Some("gzip"), Vec::new()), 64).await.unwrap();
        assert!(out.body.is_empty() && !out.truncated);
    }

    #[tokio::test]
    async fn a_corrupt_encoding_is_an_error() {
        let err = read(
            response(Some("gzip"), vec![b"not gzip at all".to_vec()]),
            64,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string()
                .starts_with("Read body: error decoding response body: "),
            "{err}"
        );
    }

    #[tokio::test]
    async fn unknown_or_stacked_encodings_are_kept_as_received() {
        for encoding in ["compress", "gzip, br", "GZIP"] {
            let out = read(response(Some(encoding), vec![b"raw".to_vec()]), 64)
                .await
                .unwrap();
            assert_eq!(out.body, b"raw", "{encoding}");
        }
    }

    #[tokio::test]
    async fn header_values_that_are_not_utf8_are_kept() {
        let raw = http::Response::builder()
            .header("x-raw", &b"caf\xe9"[..])
            .header("set-cookie", "a=1")
            .header("set-cookie", "b=2")
            .body(Body::from(Vec::new()))
            .unwrap();
        let out = read(reqwest::Response::from(raw), 64).await.unwrap();
        assert_eq!(out.headers.get("x-raw"), Some("caf\u{fffd}"));
        assert_eq!(
            out.headers.get_all("set-cookie").collect::<Vec<_>>(),
            ["a=1", "b=2"]
        );
    }
}
