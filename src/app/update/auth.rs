//! Update handler for workstream 05. Owned by 05; see contract C11.

use iced::Task;

use crate::app::AppState;
use crate::message::{Message, auth::AuthMsg};

pub(super) fn handle(_state: &mut AppState, msg: AuthMsg) -> Task<Message> {
    match msg {}
}
