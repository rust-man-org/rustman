//! The right "Code snippet" panel (mockup `.rightbar`): the active tab as a
//! cURL command. Built from the same input as Share/Export, cached in
//! `AppState::snippet` and refreshed after edits — never per frame, since
//! reading the body editor allocates the whole body.

use crate::{
    app::AppState,
    domain::request::BodyType,
    services::curl::{GenerateCurlInput, generate},
    services::snippet::Lang,
    state::tabs::RequestTabState,
};

/// Bodies past this are cut in the panel; Copy still gets the full command.
const DISPLAY_CAP: usize = 64 * 1024;

/// Shown in place of literal credentials. `{{VARS}}` stay as written.
pub const MASK: &str = "••••••";

/// Panel width bounds (px).
pub(crate) const MIN_W: f32 = 220.0;
pub(crate) const MAX_W: f32 = 720.0;

/// What Send would put on the wire, as `generate` input. `Err` carries a
/// user-facing reason (e.g. invalid GraphQL variables).
pub(crate) fn curl_input(tab: &RequestTabState) -> Result<GenerateCurlInput, String> {
    use crate::domain::request::{FormFieldType, graphql_body};
    use crate::services::curl::{FormFieldInput, KvPair};
    let body_text = tab.body_editor.content();
    let is_graphql = tab.body_type == BodyType::GraphQL;
    let body = if is_graphql {
        // Export what would actually be sent, not the raw query, so the
        // generated command is runnable as-is.
        let variables = tab.graphql_variables_editor.content();
        if body_text.trim().is_empty() && variables.trim().is_empty() {
            None
        } else {
            match graphql_body(&body_text, &variables) {
                Ok(compiled) => Some(compiled),
                Err(err) => {
                    return Err(err);
                }
            }
        }
    } else if body_text.trim().is_empty() {
        None
    } else {
        Some(body_text)
    };
    let mut headers: Vec<KvPair> = tab
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.is_empty())
        .map(|h| KvPair {
            key: h.key.clone(),
            value: h.value.clone(),
        })
        .collect();
    // The GraphQL body is JSON, and unlike the JSON body type nothing
    // else here says so — without this curl would send it as form data.
    if is_graphql
        && !headers
            .iter()
            .any(|h| h.key.eq_ignore_ascii_case("content-type"))
    {
        headers.push(KvPair {
            key: "Content-Type".to_owned(),
            value: "application/json".to_owned(),
        });
    }
    let form_fields: Vec<FormFieldInput> = if tab.body_type == BodyType::FormData {
        tab.form_fields
            .iter()
            .filter(|f| f.enabled && !f.key.is_empty())
            .map(|f| FormFieldInput {
                key: f.key.clone(),
                value: f.value.clone(),
                is_file: f.field_type == FormFieldType::File,
                file_name: f.file_name.clone(),
            })
            .collect()
    } else {
        vec![]
    };
    let api_loc = match tab.api_key_location {
        crate::domain::request::ApiKeyLocation::Header => None,
        crate::domain::request::ApiKeyLocation::Query => Some("query".to_owned()),
    };
    let input = GenerateCurlInput {
        method: tab.method.as_str().to_owned(),
        url: tab.url.clone(),
        headers,
        body,
        form_fields,
        cookies: vec![],
        auth_type: tab.auth_type.as_str().to_owned(),
        bearer_token: if tab.bearer_token.is_empty() {
            None
        } else {
            Some(tab.bearer_token.clone())
        },
        basic_user: if tab.basic_user.is_empty() {
            None
        } else {
            Some(tab.basic_user.clone())
        },
        basic_pass: if tab.basic_pass.is_empty() {
            None
        } else {
            Some(tab.basic_pass.clone())
        },
        api_key_name: if tab.api_key_name.is_empty() {
            None
        } else {
            Some(tab.api_key_name.clone())
        },
        api_key_value: if tab.api_key_value.is_empty() {
            None
        } else {
            Some(tab.api_key_value.clone())
        },
        api_key_location: api_loc,
    };
    Ok(input)
}

/// Literal secrets replaced by [`MASK`]; `{{placeholders}}` are kept, so the
/// panel reads like the mockup without ever painting a real token.
fn masked(mut input: GenerateCurlInput) -> GenerateCurlInput {
    fn hide(v: &mut Option<String>) {
        if let Some(s) = v {
            let t = s.trim();
            if !(t.starts_with("{{") && t.ends_with("}}")) {
                *s = MASK.to_owned();
            }
        }
    }
    hide(&mut input.bearer_token);
    hide(&mut input.basic_pass);
    hide(&mut input.api_key_value);
    input
}

/// Full, unmasked cURL command (Share / Export).
pub(crate) fn command(tab: &RequestTabState) -> Result<String, String> {
    curl_input(tab).map(|i| generate(&i))
}

/// Full, unmasked snippet in `lang` (the panel's Copy button).
pub(crate) fn code(tab: &RequestTabState, lang: Lang) -> Result<String, String> {
    curl_input(tab).map(|i| crate::services::snippet::generate(lang, &i))
}

/// Rebuild the cached panel text for the active tab (no-op while hidden).
pub(crate) fn refresh(state: &mut AppState) {
    if !state.snippet_open {
        return;
    }
    let tab = state.tabs.active_tab();
    state.snippet = if tab.is_websocket() {
        "WebSocket requests have no code snippet.".to_owned()
    } else if tab.url.trim().is_empty() && tab.body_type == BodyType::None {
        String::new()
    } else {
        match curl_input(tab) {
            Ok(input) => {
                let mut s = crate::services::snippet::generate(state.snippet_lang, &masked(input));
                if s.len() > DISPLAY_CAP {
                    let head = crate::services::format::head_capped(&s, DISPLAY_CAP);
                    s = format!("{head}\n… (truncated; Copy has the full snippet)");
                }
                s
            }
            Err(e) => e,
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_secrets_are_masked_but_placeholders_stay() {
        let mut tab = RequestTabState::new();
        tab.url = "https://api.test/x".into();
        tab.auth_type = crate::domain::request::AuthType::Bearer;
        tab.bearer_token = "sk_live_real".into();
        tab.basic_pass = "{{PASS}}".into();
        let shown = generate(&masked(curl_input(&tab).unwrap()));
        assert!(!shown.contains("sk_live_real"), "{shown}");
        let input = masked(curl_input(&tab).unwrap());
        assert_eq!(input.bearer_token.as_deref(), Some(MASK));
        assert_eq!(input.basic_pass.as_deref(), Some("{{PASS}}"));
        assert!(command(&tab).unwrap().contains("https://api.test/x"));
    }
}
