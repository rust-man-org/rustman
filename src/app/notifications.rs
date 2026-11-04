use crate::{
    message::{AppMsg, Message},
    state::notifications::{Toast, ToastPlacement},
};
use iced::Task;

pub(crate) fn success(
    state: &mut super::AppState,
    title: &str,
    detail: Option<String>,
    placement: ToastPlacement,
) -> Task<Message> {
    let id = uuid::Uuid::new_v4().to_string();
    state.notification = Some(Toast {
        id: id.clone(),
        title: title.to_owned(),
        detail,
        placement,
    });
    Task::perform(
        async move {
            tokio::time::sleep(std::time::Duration::from_secs(8)).await;
            id
        },
        |id| Message::App(AppMsg::ToastDismissed(id)),
    )
}
