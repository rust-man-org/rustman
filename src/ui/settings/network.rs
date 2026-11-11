use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Background, Border, Element, Padding};

use crate::app::AppState;
use crate::message::{Message, SettingsMsg, TlsOption};
use crate::services::http::TlsPin;
use crate::ui::components::tooltip::tooltip;
use crate::ui::icons;
use crate::ui::theme::{CONTROL_SM, RADIUS_XS, SPACE_MD, SPACE_XL, SPACE_XS, TEXT_CAPTION, TEXT_SM, Tokens};

use super::{Page, rows, setting_row, switch};

pub fn view<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let tls = state.tls_options;
    let toggle = |label, hint, on, option| {
        setting_row(t, label, hint, switch(t, on, Message::Settings(SettingsMsg::TlsOptionToggled(option))))
    };
    column![
        rows(
            t,
            vec![
                toggle(
                    "Send HTTP/1.1 only",
                    "Try this first. Some servers close the connection when offered HTTP/2 instead of declining it.",
                    tls.http1_only,
                    TlsOption::Http1Only,
                ),
                toggle(
                    "Ignore certificate errors",
                    "Accept self-signed, internal-CA and mismatched certificates on every request, like curl -k, until you turn this off. The status bar warns while it's on.",
                    tls.accept_invalid_certs,
                    TlsOption::AcceptInvalidCerts,
                ),
                toggle("Force TLS 1.2", "Pin the handshake to TLS 1.2, for servers that fail on 1.3.", tls.pin == TlsPin::Tls12, TlsOption::ForceTls12),
                toggle("Force TLS 1.3", "Pin the handshake to TLS 1.3.", tls.pin == TlsPin::Tls13, TlsOption::ForceTls13),
            ],
        ),
        text("Changes apply to the next request; open connections are dropped.").size(TEXT_SM).color(t.on_raised.text_muted),
    ]
    .spacing(SPACE_XL)
    .into()
}

/// The status-bar warning while certificate checks are off. It holds for
/// every request and survives restarts, so it stays in sight until turned
/// off; a click opens this page.
pub fn tls_chip<'a>(state: &AppState, t: &'a Tokens) -> Option<Element<'a, Message>> {
    if !state.tls_options.accept_invalid_certs {
        return None;
    }
    let content = row![
        icons::shield_alert().size(TEXT_SM).color(t.status.error),
        text("TLS checks off").size(TEXT_CAPTION).color(t.text),
    ]
    .spacing(SPACE_XS + 2.0)
    .align_y(Alignment::Center);
    let pill = button(content)
        .height(CONTROL_SM - SPACE_XS)
        .padding(Padding::from([0.0, SPACE_MD]))
        .on_press(Message::Settings(SettingsMsg::PageSelected(Page::Network)))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(t.error_soft)),
            text_color: t.text,
            border: Border {
                radius: RADIUS_XS.into(),
                ..Border::default()
            },
            ..Default::default()
        });
    let tip = "Certificates aren't checked on any request. Click to change it in Network & TLS.";
    Some(container(tooltip(t, pill, tip, None)).padding(Padding { left: SPACE_XS, ..Padding::ZERO }).into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tls_chip_shows_only_while_certificate_checks_are_off() {
        let dir = crate::test_support::TempDir::new("tls-chip");
        let mut state = crate::app::scratch_state(dir.path().to_path_buf());
        let t = state.tokens();
        assert!(super::tls_chip(&state, t).is_none());
        state.tls_options.accept_invalid_certs = true;
        assert!(super::tls_chip(&state, t).is_some());
    }
}
