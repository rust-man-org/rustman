use std::convert::Infallible;

use iced::Task;

use crate::app::{AppState, UpdateState};
use crate::message::{Message, UpdateMsg};
use crate::services::update;

pub(super) fn handle(state: &mut AppState, msg: UpdateMsg) -> Task<Message> {
    if matches!(msg, UpdateMsg::Checked(Ok(_))) {
        state.chrome.update_check_error = None;
    }
    match msg {
        UpdateMsg::Check => {
            if matches!(
                state.update,
                UpdateState::Checking | UpdateState::Installing | UpdateState::Ready(_)
            ) {
                return Task::none();
            }
            state.update = UpdateState::Checking;
            return Task::perform(update::check(env!("CARGO_PKG_VERSION")), |res| {
                Message::Update(UpdateMsg::Checked(res))
            });
        }
        UpdateMsg::Checked(Ok(Some(info))) => state.update = UpdateState::Available(info),
        UpdateMsg::Checked(Ok(None)) => state.update = UpdateState::UpToDate,
        // A failed check never interrupts: log it, and let Settings show it.
        UpdateMsg::Checked(Err(e)) => {
            eprintln!("update check failed: {e}");
            state.update = UpdateState::Idle;
            state.chrome.update_check_error = Some(e);
        }

        UpdateMsg::Install => {
            // Installs the very release the user was offered, never whatever
            // is latest by the time they click.
            let UpdateState::Available(info) = &state.update else {
                return Task::none();
            };
            let info = info.clone();
            state.update = UpdateState::Installing;
            return Task::perform(update::install(info, env!("CARGO_PKG_VERSION")), |res| {
                Message::Update(UpdateMsg::Installed(res))
            });
        }
        UpdateMsg::Installed(Ok(version)) => state.update = UpdateState::Ready(version),
        UpdateMsg::Installed(Err(e)) => state.update = UpdateState::Failed(e),
        UpdateMsg::Dismiss => state.update = UpdateState::Idle,
        UpdateMsg::Restart => restart(state, update::restart),
    }
    Task::none()
}

/// Writes what autosave hasn't yet, then relaunches. A relaunch that works
/// replaces the process, so only a failure comes back.
fn restart(state: &mut AppState, relaunch: fn() -> Result<Infallible, String>) {
    crate::app::session::save_before_exit(state);
    let Err(e) = relaunch();
    state.update = UpdateState::Failed(e);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::message::RequestMsg;
    use crate::services::storage;
    use crate::state::session::AppSession;

    static DATA_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

    /// Stands in for the relaunch: reports the tab URLs on disk at that moment.
    fn relaunch() -> Result<Infallible, String> {
        let dir = DATA_DIR.lock().unwrap().clone().unwrap();
        let conn = storage::open(&dir).map_err(|e| e.to_string())?;
        let saved: Option<AppSession> = storage::load_session(&conn)?;
        let urls: Vec<String> = saved.into_iter().flat_map(|s| s.tabs).map(|t| t.url).collect();
        Err(urls.join(" "))
    }

    #[test]
    fn a_restart_saves_pending_edits_before_relaunching() {
        let dir = crate::test_support::TempDir::new("restart");
        *DATA_DIR.lock().unwrap() = Some(dir.path().to_path_buf());
        let mut state = crate::app::boot::load(dir.path().to_path_buf());
        let _ = crate::app::update::update(&mut state, Message::Request(RequestMsg::UrlChanged("https://pending.example".into())));
        assert!(state.session_dirty, "not autosaved yet");
        restart(&mut state, relaunch);
        let UpdateState::Failed(on_disk) = &state.update else { panic!("the stand-in relaunch fails") };
        assert!(on_disk.contains("https://pending.example"), "on disk at relaunch: {on_disk:?}");
    }
}
