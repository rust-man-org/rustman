use crate::message::SidebarPanel;

/// Widget id of a request's rename `text_input`. Shared by the sidebar view
/// (which puts it on the input) and the update handler (which focuses it right
/// after a clone), so the two cannot drift apart.
pub fn rename_input_id(request_id: &str) -> iced::widget::Id {
    format!("request-rename-{request_id}").into()
}

/// Widget id of a folder's rename `text_input` (issue #53).
pub fn group_rename_input_id(group_id: &str) -> iced::widget::Id {
    format!("group-rename-{group_id}").into()
}

/// The pre-rename name a `CancelRename` (Escape) restores. Request backups
/// carry their collection because `RenameRequest` needs it to find the row.
#[derive(Debug, Clone)]
pub enum RenameBackup {
    Collection {
        id: String,
        name: String,
    },
    Group {
        id: String,
        name: String,
    },
    Request {
        id: String,
        collection_id: String,
        name: String,
    },
}
pub fn collection_rename_input_id(collection_id: &str) -> iced::widget::Id {
    format!("collection-rename-{collection_id}").into()
}

#[derive(Debug, Clone)]
pub struct SidebarState {
    pub panel: SidebarPanel,
    pub expanded: std::collections::HashSet<String>,
    pub selected_request: Option<String>,
    pub col_renaming: Option<String>,
    pub req_renaming: Option<String>,
    /// Folder currently being renamed in place (issue #53).
    pub group_renaming: Option<String>,
    /// Id of the item being drag-reordered, if any (issue #53).
    pub dragging_item: Option<String>,
    /// Id of the tree row whose right-click menu is open, if any.
    pub context_menu: Option<String>,
    /// Last collection pressed + when, for double-click-to-rename.
    pub last_press: Option<(String, std::time::Instant)>,
    /// Original name captured when a rename field opens. Rename inputs apply
    /// on every keystroke, so Escape-cancel has to write this back.
    pub rename_backup: Option<RenameBackup>,
    pub env_editing: Option<String>,
    pub env_edit_rows: Vec<(String, String)>,
    pub collapsed: bool,
    /// Collection tree search box text (not persisted).
    pub filter: String,
    /// Tree row under the cursor; it shows its inline actions (Postman).
    pub hovered_row: Option<String>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            panel: SidebarPanel::Collections,
            expanded: std::collections::HashSet::new(),
            selected_request: None,
            col_renaming: None,
            req_renaming: None,
            group_renaming: None,
            dragging_item: None,
            context_menu: None,
            last_press: None,
            rename_backup: None,
            env_editing: None,
            env_edit_rows: Vec::new(),
            collapsed: false,
            filter: String::new(),
            hovered_row: None,
        }
    }
}
