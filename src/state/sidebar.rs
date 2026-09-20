use crate::message::SidebarPanel;

/// Widget id of a request's rename `text_input`. Shared by the sidebar view
/// (which puts it on the input) and the update handler (which focuses it right
/// after a clone), so the two cannot drift apart.
pub fn rename_input_id(request_id: &str) -> iced::widget::Id {
    format!("request-rename-{request_id}").into()
}

#[derive(Debug, Clone)]
pub struct SidebarState {
    pub panel: SidebarPanel,
    pub expanded: std::collections::HashSet<String>,
    pub selected_request: Option<String>,
    pub col_renaming: Option<String>,
    pub req_renaming: Option<String>,
    pub env_editing: Option<String>,
    pub env_edit_rows: Vec<(String, String)>,
    pub collapsed: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            panel: SidebarPanel::Collections,
            expanded: std::collections::HashSet::new(),
            selected_request: None,
            col_renaming: None,
            req_renaming: None,
            env_editing: None,
            env_edit_rows: Vec::new(),
            collapsed: false,
        }
    }
}
