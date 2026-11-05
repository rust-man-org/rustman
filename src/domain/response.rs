use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_size: usize,
    pub body_stored: bool,
    #[serde(default)]
    pub is_binary: bool,
    /// Raw bytes of a binary response, kept only for the live response so the
    /// UI can render a spreadsheet preview. Never persisted (history and
    /// collections would otherwise balloon with embedded binary blobs) —
    /// reloading a past response from history just falls back to the plain
    /// "Binary response" message instead of a rendered preview.
    #[serde(skip)]
    pub binary_data: Option<Vec<u8>>,
    pub duration_ms: u64,
    pub error: Option<String>,
    /// Phase timings and byte counts for the status-bar hover cards. Absent
    /// (zero) for responses loaded from older history.
    #[serde(default)]
    pub timing: ResponseTiming,
    #[serde(default)]
    pub sizes: ResponseSizes,
    #[serde(default)]
    pub network: ResponseNetwork,
}

/// Connection facts for the network hover card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseNetwork {
    /// `HTTP/1.1`, `HTTP/2.0`, … (empty for older history).
    pub http_version: String,
    /// Server IP and port the response came from, when known.
    pub remote_addr: Option<String>,
    /// Sent over HTTPS (certificate verified by the client).
    pub tls: bool,
}

/// Where the time went, in microseconds. `wait` runs from handing the request
/// to the client until the response headers arrive, so it includes DNS,
/// connect and TLS (the HTTP client does not report those separately).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseTiming {
    pub prepare_us: u64,
    pub wait_us: u64,
    pub download_us: u64,
    pub process_us: u64,
}

/// Bytes on the wire, as far as they are known before transfer encoding:
/// header blocks counted as `Name: value\r\n` lines plus the start line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseSizes {
    pub response_headers: usize,
    pub request_headers: usize,
    pub request_body: usize,
}

/// What kind of rich preview (if any) a binary response's content-type calls for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryPreviewKind {
    Spreadsheet,
    /// A recognised binary type without a richer preview (image, zip, etc.) —
    /// falls back to the plain "Binary response" message.
    Other,
}

impl HttpResponse {
    /// Looks at the response's Content-Type header to decide which preview
    /// (if any) `binary_data` should be rendered with.
    pub fn binary_preview_kind(&self) -> BinaryPreviewKind {
        let content_type = self
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value.to_ascii_lowercase())
            .unwrap_or_default();

        if content_type.contains("spreadsheet")
            || content_type.contains("ms-excel")
            || content_type.contains("opendocument.spreadsheet")
        {
            BinaryPreviewKind::Spreadsheet
        } else {
            BinaryPreviewKind::Other
        }
    }

    /// Whether this (non-binary) response's Content-Type is HTML.
    pub fn is_html(&self) -> bool {
        self.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("content-type")
                && value.to_ascii_lowercase().contains("text/html")
        })
    }
}
