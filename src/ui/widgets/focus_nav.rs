//! Tab-key focus navigation between focusable inputs — e.g. a header/param
//! row's Key field to its Value field. This iced version has no built-in
//! Tab-to-next-input handling for `text_input` (unlike the code editor
//! widget, which handles its own Tab for indentation), so it's built here
//! from the low-level `Operation` primitives iced does expose.

pub fn focus_next<Message: Send + 'static>() -> iced::Task<Message> {
    iced::advanced::widget::operate(
        iced::advanced::widget::operation::focusable::focus_next::<()>(),
    )
    .discard()
}

pub fn focus_previous<Message: Send + 'static>() -> iced::Task<Message> {
    iced::advanced::widget::operate(
        iced::advanced::widget::operation::focusable::focus_previous::<()>(),
    )
    .discard()
}

/// Tab traversal kept inside the widget with id `scope`: under a modal, the
/// focus cycles through the modal's fields and never reaches the window
/// behind it.
pub fn focus_next_in<Message: Send + 'static>(scope: &'static str) -> iced::Task<Message> {
    use iced::advanced::widget::operation::{focusable, scope as within};
    iced::advanced::widget::operate(within(scope.into(), focusable::focus_next::<()>())).discard()
}

pub fn focus_previous_in<Message: Send + 'static>(scope: &'static str) -> iced::Task<Message> {
    use iced::advanced::widget::operation::{focusable, scope as within};
    iced::advanced::widget::operate(within(scope.into(), focusable::focus_previous::<()>())).discard()
}
