//! Update handler for workstream 01. Owned by 01; see contract C11.

use iced::Task;

use crate::app::{AppState, drafts};
use crate::message::{Message, draft::DraftMsg};

pub(super) fn handle(state: &mut AppState, msg: DraftMsg) -> Task<Message> {
    let result = match msg {
        DraftMsg::Discard(id) => {
            drafts::discard_draft(state, &id).map(|()| "Local draft discarded".to_owned())
        }
        DraftMsg::ResolveField(id, field, side) => drafts::resolve_field(state, &id, field, side)
            .map(|left| match left {
                0 => "All conflicts resolved".to_owned(),
                n => format!("{n} conflict(s) left"),
            }),
        DraftMsg::RestoreAsNew(id) => drafts::restore_as_new(state, &id),
    };
    state.status_message = Some(match result {
        Ok(label) => label,
        Err(e) => format!("Draft not changed: {e}"),
    });
    Task::none()
}
