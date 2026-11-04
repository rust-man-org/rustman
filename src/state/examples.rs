use crate::domain::{example::RequestExample, request::KeyValue};
use iced_code_editor::CodeEditor;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct ExamplesState {
    pub items: HashMap<String, Vec<RequestExample>>,
    pub expanded: HashSet<String>,
    pub editor: Option<ExampleEditor>,
}

pub struct ExampleEditor {
    pub data: RequestExample,
    pub request_body: CodeEditor,
    pub response_body: CodeEditor,
    pub request_headers: Vec<KeyValue>,
    pub response_headers: Vec<KeyValue>,
    pub status: String,
    pub request_show_headers: bool,
    pub request_show_params: bool,
    pub response_show_headers: bool,
    pub dirty: bool,
    pub confirm_discard: bool,
    pub confirm_delete: bool,
}

impl ExampleEditor {
    pub fn new(data: RequestExample) -> Self {
        let mut response_headers: Vec<_> = data
            .response
            .headers
            .iter()
            .map(|(key, value)| KeyValue {
                id: uuid::Uuid::new_v4().to_string(),
                key: key.clone(),
                value: value.clone(),
                enabled: true,
            })
            .collect();
        response_headers.sort_by(|a, b| a.key.cmp(&b.key));
        Self {
            request_body: crate::state::tabs::make_code_editor(
                &data.request.body,
                crate::state::tabs::body_syntax_for(&data.request.body_type, &data.request.body),
            ),
            response_body: crate::state::tabs::make_code_editor(
                &data.response.body,
                if serde_json::from_str::<serde_json::Value>(&data.response.body).is_ok() {
                    "json"
                } else {
                    "txt"
                },
            ),
            request_headers: data.request.headers.clone(),
            response_headers,
            status: data.response.status.to_string(),
            data,
            request_show_headers: false,
            request_show_params: false,
            response_show_headers: false,
            dirty: false,
            confirm_discard: false,
            confirm_delete: false,
        }
    }

    pub fn snapshot(&self) -> Result<RequestExample, String> {
        let name = self.data.name.trim();
        if name.is_empty() {
            return Err("Enter an example name.".into());
        }
        let status: u16 = self
            .status
            .parse()
            .ok()
            .filter(|s| (100..=599).contains(s))
            .ok_or("Response status must be between 100 and 599.")?;
        let mut data = self.data.clone();
        data.name = name.into();
        data.request.body = self.request_body.content();
        data.request.headers = self.request_headers.clone();
        data.request.params =
            crate::domain::request::sync_params_from_url(&data.request.url, &data.request.params);
        data.response.status = status;
        data.response.body = self.response_body.content();
        data.response.body_size = data.response.body.len();
        data.response.headers = self
            .response_headers
            .iter()
            .filter(|h| h.enabled && !h.key.trim().is_empty())
            .map(|h| (h.key.clone(), h.value.clone()))
            .collect();
        data.response.binary_data = None;
        data.response.is_binary = false;
        data.response.body_stored = false;
        data.response.error = None;
        Ok(data)
    }
}
