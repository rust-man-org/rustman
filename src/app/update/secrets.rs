//! Update handler for workstream 06. Owned by 06; see contract C11.

use iced::Task;

use crate::app::AppState;
use crate::message::{Message, secrets::SecretsMsg};

pub(super) fn handle(_state: &mut AppState, msg: SecretsMsg) -> Task<Message> {
    match msg {}
}
