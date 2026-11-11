use iced::widget::{column, container, text, text_editor, text_input};
use iced::{Background, Border, Element, Length};

use crate::app::AppState;
use crate::message::{Message, SettingsMsg, WindowMsg};
use crate::ui::components::{Surface, field};
use crate::ui::theme::{
    CONTROL_MD, HAIRLINE, LINE_SM, MONO, RADIUS_SM, SPACE_2XL, SPACE_MD, SPACE_XL, SPACE_XS,
    TEXT_SM, Tokens, UI_FONT_MEDIUM, line,
};

use super::{rows, section, setting_row, switch};

const TIMEOUT_FIELD: f32 = 120.0;
const SCRIPT_HEIGHT: f32 = 120.0;

pub fn view<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let timeout = text_input("30000", &state.default_timeout_text)
        .on_input(|v| Message::Settings(SettingsMsg::DefaultTimeoutChanged(v)))
        .size(TEXT_SM)
        .padding([SPACE_XS + 1.0, SPACE_MD])
        .width(TIMEOUT_FIELD)
        .style(move |_, s| field::field_on(t, s, Surface::Raised));
    let scripts = column![
        script(t, "Pre-request", text_editor(&state.global_pre_request_editor).on_action(|a| {
            Message::Settings(SettingsMsg::GlobalPreRequestScriptEdited(a))
        })),
        script(t, "Test", text_editor(&state.global_test_editor).on_action(|a| {
            Message::Settings(SettingsMsg::GlobalTestScriptEdited(a))
        })),
    ]
    .spacing(SPACE_XL);
    column![
        rows(
            t,
            vec![
                setting_row(
                    t,
                    "Use system title bar",
                    "Use your desktop's window frame instead of Rustman's title bar.",
                    switch(t, state.window_prefs.system_title_bar, Message::Window(WindowMsg::SystemTitleBarToggled)),
                ),
                setting_row(
                    t,
                    "Request timeout (ms)",
                    if state.timeout_overridden() {
                        "Under 1000 or not a number, so requests use 30000."
                    } else {
                        "Applied to every request. Under 1000 falls back to 30000."
                    },
                    container(timeout).height(CONTROL_MD).center_y(CONTROL_MD),
                ),
            ],
        ),
        section(
            t,
            "Global scripts",
            "Run before every request's own pre-request and test scripts, for setup you'd otherwise repeat.",
            scripts,
        ),
    ]
    .spacing(SPACE_2XL)
    .into()
}

fn script<'a>(
    t: &'static Tokens,
    label: &'static str,
    editor: text_editor::TextEditor<'a, iced::advanced::text::highlighter::PlainText, Message>,
) -> Element<'a, Message> {
    let editor = editor
        .font(MONO)
        .size(TEXT_SM)
        .height(Length::Fill)
        .padding(SPACE_MD)
        .style(move |_, status| text_editor::Style {
            background: Background::Color(t.surface),
            border: Border {
                color: field::field_border(t, Surface::Raised, !matches!(status, text_editor::Status::Active | text_editor::Status::Disabled)),
                width: HAIRLINE,
                radius: RADIUS_SM.into(),
            },
            placeholder: t.text_subtle,
            value: t.text,
            selection: field::selection(t),
        });
    column![
        text(label).size(TEXT_SM).line_height(line(LINE_SM)).font(UI_FONT_MEDIUM).color(t.on_raised.text_muted),
        container(editor).height(SCRIPT_HEIGHT),
    ]
    .spacing(SPACE_XS + 2.0)
    .into()
}
