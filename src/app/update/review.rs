//! Update handler for workstream 08. Owned by 08; see contract C11.

use iced::Task;

use crate::app::AppState;
use crate::message::{Message, review::ReviewMsg};

pub(super) fn handle(_state: &mut AppState, msg: ReviewMsg) -> Task<Message> {
    match msg {}
}
