use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use super::collection::SavedRequest;
use crate::time::now_millis;

/// Request bodies and form files larger than this many bytes are left out of
/// history.
pub const HISTORY_BODY_LIMIT: usize = 10 * 1024 * 1024;

/// One sent request and the outcome it got.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp: i64,
    pub method: String,
    pub url: String,
    pub status: i32,
    pub duration_ms: i64,
    pub request: SavedRequest,
    /// What was left out of `request` for its size.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted: Vec<OmittedPart>,
}

/// Request content left out of a history entry, and how large it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmittedPart {
    pub what: OmittedContent,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmittedContent {
    Body,
    /// The file of the form field with this key.
    FormFile(String),
}

impl HistoryEntry {
    /// An entry for `request`, sent just now.
    pub fn new(request: SavedRequest, status: i32, duration_ms: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_millis(),
            method: request.method.as_str().to_owned(),
            url: request.url.clone(),
            status,
            duration_ms,
            request,
            omitted: Vec::new(),
        }
    }

    /// Whether the request carries a body or form file larger than `limit`
    /// bytes, which [`omit_large_parts`](Self::omit_large_parts) would drop.
    pub fn has_large_parts(&self, limit: usize) -> bool {
        self.request.body.len() > limit
            || self.request.form_data_fields.iter().any(|f| {
                f.file_data
                    .as_deref()
                    .is_some_and(|d| decoded_len(d) > limit)
            })
    }

    /// Drops the request body and form files larger than `limit` bytes,
    /// recording each in `omitted`. A form file counts its decoded size, not
    /// its base64 text's.
    pub fn omit_large_parts(&mut self, limit: usize) {
        let body = self.request.body.len();
        if body > limit {
            self.request.body = String::new();
            self.omitted.push(OmittedPart {
                what: OmittedContent::Body,
                bytes: body as u64,
            });
        }
        for field in &mut self.request.form_data_fields {
            if let Some(data) = field.file_data.take_if(|data| decoded_len(data) > limit) {
                self.omitted.push(OmittedPart {
                    what: OmittedContent::FormFile(field.key.clone()),
                    bytes: decoded_len(&data) as u64,
                });
            }
        }
    }
}

/// So that APIs that may change an entry can take it borrowed or owned.
impl<'a> From<&'a HistoryEntry> for Cow<'a, HistoryEntry> {
    fn from(entry: &'a HistoryEntry) -> Self {
        Cow::Borrowed(entry)
    }
}

impl From<HistoryEntry> for Cow<'_, HistoryEntry> {
    fn from(entry: HistoryEntry) -> Self {
        Cow::Owned(entry)
    }
}

/// The size of base64 `data` once decoded.
fn decoded_len(data: &str) -> usize {
    let padding = data.bytes().rev().take_while(|&b| b == b'=').count();
    (data.len() - padding) * 3 / 4
}

/// A history entry without its request, for listing many cheaply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySummary {
    pub id: String,
    pub timestamp: i64,
    pub method: String,
    pub url: String,
    pub status: i32,
    pub duration_ms: i64,
}

impl HistoryEntry {
    pub fn summary(&self) -> HistorySummary {
        HistorySummary {
            id: self.id.clone(),
            timestamp: self.timestamp,
            method: self.method.clone(),
            url: self.url.clone(),
            status: self.status,
            duration_ms: self.duration_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::request::{FormField, FormFieldType};

    fn entry(body: &str, files: &[(&str, &str)]) -> HistoryEntry {
        let mut request = SavedRequest::new_in("c".into(), "r".into());
        request.body = body.to_owned();
        request.form_data_fields = files
            .iter()
            .map(|(key, data)| FormField {
                id: (*key).to_owned(),
                key: (*key).to_owned(),
                value: String::new(),
                field_type: FormFieldType::File,
                enabled: true,
                file_name: Some("f.bin".into()),
                file_data: Some((*data).to_owned()),
                mime_type: None,
            })
            .collect();
        HistoryEntry::new(request, 200, 1)
    }

    #[test]
    fn content_up_to_the_limit_is_kept() {
        let mut entry = entry("12345", &[("f", "QUJD")]);
        assert!(!entry.has_large_parts(5));
        entry.omit_large_parts(5);
        assert!(entry.omitted.is_empty());
        assert_eq!(entry.request.body, "12345");
    }

    #[test]
    fn a_body_over_the_limit_is_left_out_and_recorded() {
        let mut capped = entry("123456", &[]);
        assert!(capped.has_large_parts(5));
        capped.omit_large_parts(5);
        assert!(capped.request.body.is_empty());
        assert_eq!(
            capped.omitted,
            [OmittedPart {
                what: OmittedContent::Body,
                bytes: 6
            }]
        );
    }

    #[test]
    fn each_form_file_is_judged_by_its_decoded_size() {
        // "QUJDREVGRw==" decodes to 7 bytes, "QUI=" to 2.
        let mut capped = entry("", &[("big", "QUJDREVGRw=="), ("small", "QUI=")]);
        assert!(capped.has_large_parts(5));
        capped.omit_large_parts(5);
        let files = &capped.request.form_data_fields;
        assert_eq!(
            (files[0].file_data.as_deref(), files[1].file_data.as_deref()),
            (None, Some("QUI="))
        );
        assert_eq!(
            files[0].file_name.as_deref(),
            Some("f.bin"),
            "the field itself stays"
        );
        assert_eq!(
            capped.omitted,
            [OmittedPart {
                what: OmittedContent::FormFile("big".into()),
                bytes: 7
            }]
        );
    }
}
