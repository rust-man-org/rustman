//! Headers tab (mockup `.hdr-toolbar` + `.kv`): label, auto-generated
//! headers toggle, Bulk Edit, Presets and a ⋯ menu above the table.

use iced::{
    Alignment, Background, Border, Element, Length,
    widget::{Space, button, checkbox, column, container, pick_list, row, scrollable, text},
};

use crate::{
    domain::request::{AuthType, BodyType, KeyValue},
    message::{Message, RequestMsg},
    state::tabs::RequestTabState,
    ui::{
        components::molecules::menu_item,
        icons, styles,
        theme::{MONO, Palette},
        widgets::{kv_table, mini},
    },
};

pub const PRESETS_MENU: &str = "request:headers:presets";
pub const MORE_MENU: &str = "request:headers:more";

const CONTENT_TYPE_SUGGESTIONS: &[&str] = &[
    "application/json",
    "application/xml",
    "text/plain",
    "text/html",
    "text/css",
    "text/javascript",
    "application/javascript",
    "application/x-www-form-urlencoded",
    "multipart/form-data",
    "application/octet-stream",
    "application/pdf",
    "application/zip",
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/svg+xml",
    "audio/mpeg",
    "video/mp4",
    "application/graphql-response+json",
    "application/ld+json",
    "application/merge-patch+json",
    "application/problem+json",
    "application/vnd.api+json",
    "application/yaml",
    "text/csv",
    "text/xml",
    "application/grpc",
    "application/x-protobuf",
    "application/x-ndjson",
];

/// Presets menu: label and the Content-Type it sets ("" removes it).
const PRESETS: &[(&str, &str)] = &[
    ("None", ""),
    ("JSON", "application/json"),
    ("XML", "application/xml"),
    ("Text", "text/plain"),
    ("Form URL Encoded", "application/x-www-form-urlencoded"),
];

/// Shown instead of values that only exist once the request is built.
const CALCULATED: &str = "<calculated when request is sent>";

fn is_content_type_key(key: &str) -> bool {
    key.trim().eq_ignore_ascii_case("content-type")
}

pub fn view<'a>(
    tab: &'a RequestTabState,
    show_auto: bool,
    open_menu: Option<&str>,
) -> Element<'a, Message> {
    let auto = auto_headers(tab);
    let toolbar = toolbar(tab, auto.len(), show_auto, open_menu);
    let body: Element<'a, Message> = match &tab.headers_bulk {
        Some(content) => kv_table::bulk_panel(
            content,
            |a| Message::Request(RequestMsg::HeadersBulkEdited(a)),
            Message::Request(RequestMsg::HeadersBulkToggle),
        ),
        None => scrollable(column![header_table(
            &tab.headers,
            show_auto.then_some(auto)
        )])
        .height(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            crate::ui::theme::grabbable_scrollbar(),
        ))
        .style(crate::ui::theme::thin_scrollbar)
        .into(),
    };
    column![toolbar, body]
        .padding([14, 16])
        .height(Length::Fill)
        .into()
}

fn toolbar<'a>(
    tab: &'a RequestTabState,
    hidden: usize,
    show_auto: bool,
    open_menu: Option<&str>,
) -> Element<'a, Message> {
    let bulk = tab.headers_bulk.is_some();
    let presets = PRESETS
        .iter()
        .map(|(label, value)| {
            menu_item(
                None,
                *label,
                Some(Message::Request(RequestMsg::HeaderPresetApplied(
                    (*value).to_owned(),
                ))),
                false,
            )
        })
        .collect();
    let more_open = open_menu == Some(MORE_MENU);
    let more = mini::anchored_menu(
        mini::icon_button(
            icons::more(),
            "More",
            Some(Message::Sidebar(
                crate::message::SidebarMsg::ContextMenuOpened(MORE_MENU.to_owned()),
            )),
            more_open,
        ),
        more_open,
        vec![
            menu_item(
                Some(if show_auto {
                    icons::eye_closed()
                } else {
                    icons::eye()
                }),
                if show_auto {
                    "Hide auto-generated headers"
                } else {
                    "Show auto-generated headers"
                },
                Some(Message::Request(RequestMsg::AutoHeadersToggled)),
                false,
            ),
            menu_item(
                Some(icons::eraser()),
                "Remove all headers",
                (!tab.headers.is_empty() && !bulk)
                    .then_some(Message::Request(RequestMsg::HeadersCleared)),
                true,
            ),
        ],
        220.0,
    );
    container(
        row![
            text("Headers").size(12).color(Palette::text_muted()),
            hidden_toggle(hidden, show_auto),
            Space::new().width(Length::Fill),
            mini::text_button(
                None,
                if bulk { "Key-Value Edit" } else { "Bulk Edit" },
                Some(Message::Request(RequestMsg::HeadersBulkToggle)),
            ),
            mini::dropdown(
                "Presets",
                PRESETS_MENU,
                open_menu == Some(PRESETS_MENU),
                26.0,
                presets,
                200.0,
            ),
            more,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(iced::Padding {
        bottom: 10.0,
        ..Default::default()
    })
    .width(Length::Fill)
    .into()
}

/// `.hidden-toggle`: "N hidden" pill; accent while the auto-generated
/// headers are listed.
fn hidden_toggle<'a>(hidden: usize, on: bool) -> Element<'a, Message> {
    let label = if on {
        "Hide auto-generated headers".to_owned()
    } else {
        format!("{hidden} hidden")
    };
    let fg = if on {
        Palette::accent()
    } else {
        Palette::text_muted()
    };
    button(
        container(
            row![
                (if on {
                    icons::eye_closed()
                } else {
                    icons::eye()
                })
                .size(12)
                .color(fg),
                text(label).size(11).color(fg),
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        )
        .height(24)
        .center_y(24),
    )
    .on_press(Message::Request(RequestMsg::AutoHeadersToggled))
    .padding([0, 9])
    .style(move |_, status| {
        let hot = matches!(status, button::Status::Hovered);
        button::Style {
            background: if on {
                Some(Background::Color(Palette::accent_soft()))
            } else if hot {
                Some(Background::Color(Palette::surface_high()))
            } else {
                None
            },
            text_color: fg,
            border: Border {
                color: if on {
                    Palette::accent()
                } else {
                    Palette::border()
                },
                width: 1.0,
                radius: 24.0.into(),
            },
            ..Default::default()
        }
    })
    .into()
}

/// Headers the client adds on send that the table does not set itself
/// (Postman's "hidden" headers). Secrets never show: auth values are a
/// pointer to the Authorization tab.
pub fn auto_headers(tab: &RequestTabState) -> Vec<(String, String)> {
    let user_sets = |name: &str| {
        tab.headers
            .iter()
            .any(|h| h.enabled && h.key.trim().eq_ignore_ascii_case(name))
    };
    let mut out: Vec<(String, String)> = Vec::new();
    let mut add = |name: &str, value: &str| {
        if !user_sets(name) {
            out.push((name.to_owned(), value.to_owned()));
        }
    };
    let from_auth = "<from Authorization tab>";
    match tab.auth_type {
        AuthType::Bearer if !tab.bearer_token.is_empty() => add("Authorization", from_auth),
        AuthType::Basic if !tab.basic_user.is_empty() => add("Authorization", from_auth),
        AuthType::JwtUser if !tab.jwt_secret.is_empty() => add("Authorization", from_auth),
        AuthType::Cookie if !tab.cookie_string.is_empty() => add("Cookie", from_auth),
        AuthType::ApiKey
            if !tab.api_key_name.is_empty()
                && tab.api_key_location == crate::domain::request::ApiKeyLocation::Header =>
        {
            add(&tab.api_key_name, from_auth)
        }
        _ => {}
    }
    match tab.body_type {
        BodyType::Json | BodyType::GraphQL => add("Content-Type", "application/json"),
        BodyType::FormData => add("Content-Type", CALCULATED),
        BodyType::UrlEncoded => add("Content-Type", "application/x-www-form-urlencoded"),
        _ => {}
    }
    if tab.body_type != BodyType::None {
        add("Content-Length", CALCULATED);
    }
    add("Host", CALCULATED);
    add("User-Agent", concat!("rustman/", env!("CARGO_PKG_VERSION")));
    add("Accept", "*/*");
    out
}

fn header_table(items: &[KeyValue], auto: Option<Vec<(String, String)>>) -> Element<'_, Message> {
    let mut rows: Vec<Element<Message>> = vec![kv_table::head("Key", "Value")];

    for (name, value) in auto.unwrap_or_default() {
        let cell = |s: String, color| {
            container(text(s).size(12).font(MONO).color(color))
                .padding([6, 7])
                .into()
        };
        rows.push(kv_table::kv_row(
            icons::check().size(12).color(Palette::text_subtle()).into(),
            cell(name, Palette::text_muted()),
            cell(value, Palette::text_subtle()),
            Space::new().into(),
        ));
    }

    for (i, item) in items.iter().enumerate() {
        let value_widget: Element<Message> = if is_content_type_key(&item.key) {
            let suggestions: Vec<&str> = CONTENT_TYPE_SUGGESTIONS
                .iter()
                .filter(|s| item.value.is_empty() || s.starts_with(&item.value))
                .copied()
                .collect();
            if suggestions.is_empty() {
                value_cell(i, &item.value)
            } else {
                let current = CONTENT_TYPE_SUGGESTIONS
                    .iter()
                    .find(|s| **s == item.value)
                    .copied()
                    .unwrap_or("");
                pick_list(suggestions, Some(current), move |s| {
                    Message::Request(RequestMsg::HeaderValueChanged(i, s.to_owned()))
                })
                .text_size(12)
                .font(MONO)
                .padding([5, 7])
                .width(Length::Fill)
                .style(styles::pick_list)
                .into()
            }
        } else {
            value_cell(i, &item.value)
        };

        rows.push(kv_table::kv_row(
            checkbox(item.enabled)
                .on_toggle(move |_| Message::Request(RequestMsg::HeaderToggled(i)))
                .size(13)
                .spacing(0)
                .style(styles::checkbox)
                .into(),
            super::cell::text_cell("Key", &item.key, 12.0, [5, 7], Length::Fill, move |s| {
                Message::Request(RequestMsg::HeaderKeyChanged(i, s))
            }),
            value_widget,
            kv_table::remove_button(Message::Request(RequestMsg::HeaderRemoved(i))),
        ));
    }

    rows.push(kv_table::add_row(Message::Request(RequestMsg::HeaderAdded)));
    kv_table::frame(rows)
}

fn value_cell(i: usize, value: &str) -> Element<'_, Message> {
    super::cell::text_cell("Value", value, 12.0, [5, 7], Length::Fill, move |s| {
        Message::Request(RequestMsg::HeaderValueChanged(i, s))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_headers_skip_what_the_table_sets() {
        let mut tab = RequestTabState::new();
        tab.body_type = BodyType::Json;
        let names = |t: &RequestTabState| {
            auto_headers(t)
                .into_iter()
                .map(|(k, _)| k)
                .collect::<Vec<_>>()
        };
        assert!(names(&tab).contains(&"Content-Type".to_owned()));

        let mut ct = KeyValue::new_empty();
        ct.key = "content-type".into();
        ct.value = "text/plain".into();
        tab.headers.push(ct);
        assert!(!names(&tab).contains(&"Content-Type".to_owned()));
        assert!(names(&tab).contains(&"User-Agent".to_owned()));
    }

    #[test]
    fn auth_values_never_show() {
        let mut tab = RequestTabState::new();
        tab.auth_type = AuthType::Bearer;
        tab.bearer_token = "s3cret".into();
        let auth = auto_headers(&tab)
            .into_iter()
            .find(|(k, _)| k == "Authorization")
            .expect("bearer adds Authorization");
        assert!(!auth.1.contains("s3cret"));
    }
}
