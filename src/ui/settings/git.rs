use iced::Element;
use iced::widget::text_input;

use crate::app::AppState;
use crate::message::{Message, SettingsMsg};
use crate::ui::components::{Surface, field};
use crate::ui::theme::{SPACE_MD, SPACE_XS, TEXT_SM, Tokens};

use super::{rows, setting_row};

const FIELD_WIDTH: f32 = 260.0;

/// The fields show what git itself would use as their placeholders, so
/// leaving them empty reads as "this".
pub fn view<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let fallback = state.git.default_identity.as_ref();
    let input = |placeholder: &'a str, value: &'a str, on_input: fn(String) -> SettingsMsg| {
        text_input(placeholder, value)
            .on_input(move |v| Message::Settings(on_input(v)))
            .size(TEXT_SM)
            .padding([SPACE_XS + 1.0, SPACE_MD])
            .width(FIELD_WIDTH)
            .style(move |_, s| field::field_on(t, s, Surface::Raised))
    };
    rows(
        t,
        vec![
            setting_row(
                t,
                "Name",
                "Leave empty to use your git config. If set, every commit Rustman makes uses it.",
                input(fallback.map_or("Your Name", |id| id.name.as_str()), &state.git_user_name, SettingsMsg::GitNameChanged),
            ),
            setting_row(
                t,
                "Email",
                "Goes with the name. Rustman never changes your git config.",
                input(fallback.map_or("you@example.com", |id| id.email.as_str()), &state.git_user_email, SettingsMsg::GitEmailChanged),
            ),
        ],
    )
}
