//! The status-bar update chip. Background update checks never interrupt:
//! a failed check is silent here (Settings shows it), and a found update is
//! one small chip.

use iced::widget::{button, row, text};
use iced::{Alignment, Background, Border, Color, Element, Padding};

use crate::app::UpdateState;
use crate::message::{Message, UpdateMsg};
use crate::ui::components::tooltip;
use crate::ui::icons;
use crate::ui::theme::{CONTROL_SM, RADIUS_XS, SPACE_MD, SPACE_XS, TEXT_CAPTION, TEXT_SM, Tokens};

/// Chip height: a step below the compact control height.
const HEIGHT: f32 = CONTROL_SM - SPACE_XS;

/// What the chip says and does in each update state.
#[derive(Debug, Clone)]
pub struct Chip<'a> {
    pub label: &'static str,
    pub detail: Option<&'a str>,
    pub hint: Option<&'a str>,
    pub action: Option<UpdateMsg>,
    pub failed: bool,
}

/// `None` while there is nothing worth showing.
pub fn describe(update: &UpdateState) -> Option<Chip<'_>> {
    let chip = |label, detail, hint, action, failed| Some(Chip { label, detail, hint, action, failed });
    match update {
        UpdateState::Available(info) => chip(
            "Update available",
            Some(info.version.as_str()),
            Some("Download and install it"),
            Some(UpdateMsg::Install),
            false,
        ),
        UpdateState::Installing => chip("Downloading update…", None, None, None, false),
        UpdateState::Ready(version) => chip(
            "Restart to update",
            Some(version.as_str()),
            None,
            Some(UpdateMsg::Restart),
            false,
        ),
        UpdateState::Failed(err) => chip("Update failed", None, Some(err.as_str()), Some(UpdateMsg::Dismiss), true),
        UpdateState::Idle | UpdateState::Checking | UpdateState::UpToDate => None,
    }
}

pub fn chip<'a>(update: &'a UpdateState, t: &'a Tokens) -> Option<Element<'a, Message>> {
    let c = describe(update)?;
    let accent = if c.failed { t.status.error } else { t.accent_text };
    let mut content = row![
        icons::update().size(TEXT_SM).color(accent),
        text(c.label).size(TEXT_CAPTION).color(t.text),
    ]
    .spacing(SPACE_XS + 2.0)
    .align_y(Alignment::Center);
    if let Some(detail) = c.detail {
        content = content.push(text(detail).size(TEXT_CAPTION).color(t.text_muted));
    }
    let tint = if c.failed { t.error_soft } else { t.accent_soft };
    let pill = button(content)
        .height(HEIGHT)
        .padding(Padding::from([0.0, SPACE_MD]))
        .on_press_maybe(c.action.map(Message::Update))
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(tint)),
            text_color: t.text,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: RADIUS_XS.into(),
            },
            ..Default::default()
        });
    Some(match c.hint {
        Some(hint) => tooltip(t, pill, hint, None),
        None => pill.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_states_show_no_chip() {
        for state in [UpdateState::Idle, UpdateState::Checking, UpdateState::UpToDate] {
            assert!(describe(&state).is_none(), "{state:?}");
        }
    }

    #[test]
    fn an_install_failure_is_the_only_error_chip_and_can_be_dismissed() {
        let failed = UpdateState::Failed("disk full".to_owned());
        let c = describe(&failed).unwrap();
        assert!(c.failed);
        assert!(matches!(c.action, Some(UpdateMsg::Dismiss)));
        assert_eq!(c.hint, Some("disk full"));
    }

    #[test]
    fn a_ready_update_restarts_on_click() {
        let ready = UpdateState::Ready("0.3.16".to_owned());
        let c = describe(&ready).unwrap();
        assert!(matches!(c.action, Some(UpdateMsg::Restart)));
        assert_eq!(c.detail, Some("0.3.16"));
    }
}
