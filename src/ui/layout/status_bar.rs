use std::time::Duration;

use iced::widget::{container, row, text};
use iced::{Alignment, Background, Element, Length, Padding};

use crate::app::AppState;
use crate::message::{AppMsg, Message};
use crate::ui::components::tooltip::tooltip_at;
use crate::ui::icons;
use crate::ui::theme::{HAIRLINE, SPACE_LG, SPACE_MD, SPACE_XL, SPACE_XS, STATUS_BAR_HEIGHT, TEXT_SM, Tokens};
use crate::ui::widgets::toast_timer::toast_timer;

/// Messages longer than this also get the full text on hover.
const LONG_MESSAGE: usize = 80;

/// How a status message should read. `status_message` is a plain string set
/// from many handlers, so the tone is inferred from its wording — every
/// message used to render in success green, including "Git error: …".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    /// Still working ("Parsing 2 MB command…"): muted, never auto-dismissed.
    Progress,
    Success,
    /// Nothing went wrong, but nothing happened either ("Clipboard is empty.").
    Warning,
    Error,
}

pub(crate) fn tone_of(msg: &str) -> Tone {
    let lower = msg.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    if has(&[
        "error",
        "could not",
        "couldn't",
        "can't",
        "cannot",
        "failed",
        "invalid",
        "not valid",
        "isn't",
        "too large",
        "past the",
        "dropped",
    ]) {
        Tone::Error
    } else if msg.trim_end().ends_with('…') || msg.trim_end().ends_with("...") {
        Tone::Progress
    } else if has(&["nothing", "empty", " first", "only ", "no rustman"]) {
        Tone::Warning
    } else {
        Tone::Success
    }
}

/// How long a toast stays up. Errors linger so they can actually be read;
/// progress lines stay until the work that set them replaces them.
fn ttl(tone: Tone) -> Option<Duration> {
    match tone {
        Tone::Progress => None,
        Tone::Success => Some(Duration::from_secs(4)),
        Tone::Warning => Some(Duration::from_secs(6)),
        Tone::Error => Some(Duration::from_secs(10)),
    }
}

/// A status toast: it dismisses itself after a while, by tone.
fn status_message<'a>(msg: &'a str, t: &'a Tokens) -> Element<'a, Message> {
    let tone = tone_of(msg);
    let content = status_line(msg, tone, t);
    match ttl(tone) {
        Some(ttl) => toast_timer(content, msg, ttl, Message::App(AppMsg::DismissStatus(msg.to_owned()))),
        None => content,
    }
}

fn status_line<'a>(msg: &'a str, tone: Tone, t: &'a Tokens) -> Element<'a, Message> {
    let (color, glyph) = match tone {
        Tone::Progress => (t.text_muted, icons::timer()),
        Tone::Success => (t.status.success, icons::check()),
        Tone::Warning => (t.status.warning, icons::alert()),
        Tone::Error => (t.status.error, icons::alert()),
    };
    // One line only; a multi-line message shows its first line and the
    // whole text on hover.
    let first_line = msg.lines().next().unwrap_or("").trim_end();
    let multi_line = msg.lines().nth(1).is_some();
    let mut line = row![
        glyph.size(TEXT_SM).color(color),
        text(first_line).size(TEXT_SM).color(color).wrapping(iced_core::text::Wrapping::None),
    ]
    .spacing(SPACE_XS + 2.0)
    .align_y(Alignment::Center);
    if multi_line {
        line = line.push(text("…").size(TEXT_SM).color(color));
    }
    if multi_line || msg.len() > LONG_MESSAGE {
        tooltip_at(
            t,
            line,
            crate::services::format::head_capped(msg, crate::consts::HOVER_TIP_BYTES),
            None,
            iced::widget::tooltip::Position::Top,
        )
    } else {
        line.into()
    }
}

/// The bottom bar: the current branch and any status line on the left,
/// the update chip on the right.
pub(super) fn status_bar<'a>(state: &'a AppState, t: &'a Tokens) -> Element<'a, Message> {
    let mut left = row![].spacing(SPACE_XL).align_y(Alignment::Center);
    if let Some(branch) = &state.git_branch {
        left = left.push(
            row![
                icons::git_branch().size(TEXT_SM),
                text(branch.as_str()).size(TEXT_SM),
            ]
            .spacing(SPACE_XS + 2.0)
            .align_y(Alignment::Center),
        );
    }
    if let Some(error) = &state.storage_error {
        left = left.push(status_line(error, Tone::Error, t));
    }
    if let Some(msg) = state.status_message.as_deref().filter(|m| !m.trim().is_empty()) {
        left = left.push(status_message(msg, t));
    }
    let bar = row![container(left).width(Length::Fill).clip(true)]
        .align_y(Alignment::Center)
        .height(STATUS_BAR_HEIGHT - HAIRLINE)
        .padding(Padding {
            left: SPACE_LG,
            right: SPACE_MD,
            ..Padding::ZERO
        });
    #[cfg(feature = "self-update")]
    let bar = bar.push(crate::ui::update_chip::chip(&state.update, t));
    let bar = bar.push(crate::ui::settings::network::tls_chip(state, t));
    container(bar)
        .style(move |_| container::Style {
            background: Some(Background::Color(t.canvas)),
            text_color: Some(t.text_muted),
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::{Tone, tone_of};

    /// Real messages from the update handlers, so wording changes that would
    /// flip a color show up here.
    #[test]
    fn status_messages_get_the_right_tone() {
        for (msg, tone) in [
            ("Saved!", Tone::Success),
            ("Copied!", Tone::Success),
            ("cURL command copied!", Tone::Success),
            ("Imported 12 requests", Tone::Success),
            ("Response saved to /tmp/x.bin", Tone::Success),
            ("Parsing 2.0 MB command…", Tone::Progress),
            ("Reading clipboard…", Tone::Progress),
            ("Clipboard is empty.", Tone::Warning),
            ("Nothing to export — no collections yet", Tone::Warning),
            ("Enter a commit message first", Tone::Warning),
            ("Git error: authentication required", Tone::Error),
            ("Import error: bad json", Tone::Error),
            ("Could not save response: permission denied", Tone::Error),
            ("That isn't a curl or HTTPie command — nothing imported.", Tone::Error),
            ("big.txt is 40 MB — past the 32 MB import limit.", Tone::Error),
        ] {
            assert_eq!(tone_of(msg), tone, "{msg:?}");
        }
    }
}
