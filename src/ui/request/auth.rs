use iced::{
    Alignment, Element, Length,
    widget::{Space, button, column, container, pick_list, row, text, text_input},
};

use crate::{
    domain::request::AuthType,
    message::{Message, RequestMsg},
    state::tabs::RequestTabState,
    ui::{
        icons::{self, eye, eye_closed},
        styles::{self, tip},
        theme::{ICON_SM, Palette, TEXT_CAPTION, TEXT_MD, TEXT_SM},
    },
};

/// Width of the label column, so every field lines up.
const LABEL_WIDTH: f32 = 96.0;

/// A pick-list entry: the stored id (`AuthType::as_str`, what
/// `AuthTypeChanged` parses) plus the label people read. The list used to
/// show the raw ids — "apikey", "jwt-user" — straight from the enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Choice {
    id: &'static str,
    label: &'static str,
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label)
    }
}

const AUTH_TYPES: &[Choice] = &[
    Choice {
        id: "none",
        label: "No Auth",
    },
    Choice {
        id: "bearer",
        label: "Bearer Token",
    },
    Choice {
        id: "basic",
        label: "Basic Auth",
    },
    Choice {
        id: "apikey",
        label: "API Key",
    },
    Choice {
        id: "jwt-user",
        label: "JWT (HS256)",
    },
    Choice {
        id: "cookie",
        label: "Cookie",
    },
];

const KEY_LOCATIONS: &[Choice] = &[
    Choice {
        id: "header",
        label: "Header",
    },
    Choice {
        id: "query",
        label: "Query Params",
    },
];

fn choice(list: &'static [Choice], id: &str) -> Option<Choice> {
    list.iter().copied().find(|c| c.id == id)
}

pub fn view(tab: &RequestTabState) -> Element<'_, Message> {
    let type_picker = row![
        field_label("Type"),
        pick_list(
            AUTH_TYPES,
            choice(AUTH_TYPES, tab.auth_type.as_str()),
            |c: Choice| Message::Request(RequestMsg::AuthTypeChanged(c.id.to_owned()))
        )
        .text_size(TEXT_SM)
        .padding([5, 10])
        .style(styles::pick_list)
        .width(180),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let fields: Element<Message> = match &tab.auth_type {
        AuthType::Bearer => column![
            labeled_input("Token", "Paste a token", &tab.bearer_token, |s| {
                Message::Request(RequestMsg::BearerTokenChanged(s))
            }),
            hint("Sent as  Authorization: Bearer <token>"),
        ]
        .spacing(6)
        .into(),

        AuthType::Basic => {
            let (eye_icon, eye_tip) = if tab.show_basic_pass {
                (eye_closed(), "Hide password")
            } else {
                (eye(), "Show password")
            };

            column![
                labeled_input("Username", "Username", &tab.basic_user, |s| {
                    Message::Request(RequestMsg::BasicUserChanged(s))
                }),
                row![
                    labeled_input_secure(
                        "Password",
                        "Password",
                        &tab.basic_pass,
                        !tab.show_basic_pass,
                        |s| { Message::Request(RequestMsg::BasicPassChanged(s)) }
                    ),
                    tip(
                        button(eye_icon.size(ICON_SM).color(Palette::text_muted()))
                            .on_press_maybe((!tab.basic_pass.is_empty()).then_some(
                                Message::Request(RequestMsg::ToggleBasicPassVisibility)
                            ))
                            .style(styles::ghost_button)
                            .padding(6),
                        eye_tip,
                    ),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
                hint("Sent as  Authorization: Basic base64(username:password)"),
            ]
            .spacing(6)
            .into()
        }

        AuthType::ApiKey => column![
            labeled_input("Key", "e.g. X-API-Key", &tab.api_key_name, |s| {
                Message::Request(RequestMsg::ApiKeyNameChanged(s))
            }),
            labeled_input("Value", "Paste the key", &tab.api_key_value, |s| {
                Message::Request(RequestMsg::ApiKeyValueChanged(s))
            }),
            row![
                field_label("Add to"),
                pick_list(
                    KEY_LOCATIONS,
                    choice(KEY_LOCATIONS, tab.api_key_location.as_str()),
                    |c: Choice| Message::Request(RequestMsg::ApiKeyLocationChanged(
                        c.id.to_owned()
                    ))
                )
                .text_size(TEXT_SM)
                .padding([5, 10])
                .style(styles::pick_list)
                .width(180),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(6)
        .into(),

        AuthType::Cookie => column![
            labeled_input(
                "Cookie",
                "name=value; other=value",
                &tab.cookie_string,
                |s| { Message::Request(RequestMsg::CookieStringChanged(s)) }
            ),
            hint("Sent as the Cookie header on every request from this tab"),
        ]
        .spacing(6)
        .into(),

        AuthType::JwtUser => column![
            labeled_input("Subject", "sub claim, e.g. user-42", &tab.jwt_subject, |s| {
                Message::Request(RequestMsg::JwtSubjectChanged(s))
            }),
            labeled_input("Secret", "HMAC signing secret", &tab.jwt_secret, |s| {
                Message::Request(RequestMsg::JwtSecretChanged(s))
            }),
            row![
                field_label("Algorithm"),
                pick_list(&["HS256"][..], Some(tab.jwt_algo.as_str()), |s| {
                    Message::Request(RequestMsg::JwtAlgoChanged(s.to_owned()))
                })
                .text_size(TEXT_SM)
                .padding([5, 10])
                .style(styles::pick_list)
                .width(120),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            hint("A fresh token is signed for every request (exp = now + 1h)"),
        ]
        .spacing(6)
        .into(),

        AuthType::None => no_auth(),
    };

    let col = column![type_picker, fields].spacing(10);

    container(col).padding([12, 14]).width(Length::Fill).into()
}

/// The "No Auth" state: a short explanation instead of a blank panel.
fn no_auth() -> Element<'static, Message> {
    container(
        column![
            icons::lock().size(20).color(Palette::text_subtle()),
            Space::new().height(crate::ui::theme::SPACE_XS),
            text("This request does not use authorization")
                .size(TEXT_MD)
                .color(Palette::text_muted()),
            text("Pick a type above to add a bearer token, basic auth, an API key, a signed JWT or a cookie.")
                .size(TEXT_CAPTION)
                .color(Palette::text_subtle())
                .align_x(Alignment::Center),
        ]
        .spacing(4)
        .max_width(420)
        .align_x(Alignment::Center),
    )
    .padding([24, 8])
    .center_x(Length::Fill)
    .into()
}

fn field_label(label: &'static str) -> Element<'static, Message> {
    text(label)
        .size(TEXT_SM)
        .color(Palette::text_muted())
        .width(LABEL_WIDTH)
        .into()
}

fn hint(message: &'static str) -> Element<'static, Message> {
    row![
        Space::new().width(LABEL_WIDTH),
        text(message)
            .size(TEXT_CAPTION)
            .color(Palette::text_subtle()),
    ]
    .spacing(8)
    .into()
}

fn labeled_input<'a>(
    label: &'static str,
    placeholder: &'static str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    labeled_input_secure(label, placeholder, value, false, on_input)
}

fn labeled_input_secure<'a>(
    label: &'static str,
    placeholder: &'static str,
    value: &'a str,
    secure: bool,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        field_label(label),
        text_input(placeholder, value)
            .on_input(on_input)
            .secure(secure)
            .size(TEXT_SM)
            .padding([6, 10])
            .width(Length::Fill)
            .style(styles::field_input),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::{AUTH_TYPES, KEY_LOCATIONS};

    /// The pick list hands `Choice::id` to the parsers; every id must round-
    /// trip, or picking an entry would silently select something else.
    #[test]
    fn every_choice_id_round_trips_through_the_parsers() {
        for c in AUTH_TYPES {
            let parsed: crate::domain::request::AuthType = c.id.parse().unwrap();
            assert_eq!(parsed.as_str(), c.id, "{}", c.label);
        }
        for c in KEY_LOCATIONS {
            let parsed: crate::domain::request::ApiKeyLocation = c.id.parse().unwrap();
            assert_eq!(parsed.as_str(), c.id, "{}", c.label);
        }
    }
}
