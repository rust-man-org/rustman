//! Update handler for workstream 03. Owned by 03; see contract C11.

use iced::Task;

use crate::app::AppState;
use crate::message::{Message, meta::MetaMsg};

pub(super) fn handle(_state: &mut AppState, msg: MetaMsg) -> Task<Message> {
    match msg {}
}
