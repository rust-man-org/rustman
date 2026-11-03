//! Update handler for workstream 01. Owned by 01; see contract C11.

use iced::Task;

use crate::app::AppState;
use crate::message::{Message, draft::DraftMsg};

pub(super) fn handle(_state: &mut AppState, msg: DraftMsg) -> Task<Message> {
    match msg {}
}
