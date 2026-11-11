use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::domain::collection::SavedRequest;
use crate::domain::environment::AppEnvironment;
use crate::domain::group::ImportedCollection;
use crate::message::SidebarPanel;
use crate::ui::sidebar::collections::TreeCache;
use crate::ui::sidebar::history::HistoryCache;

/// How many history entries the History panel lists, as many as are kept.
pub const HISTORY_LISTED: usize = 100;

/// Widget id of the inline rename field; the update handler focuses it.
pub fn rename_input_id() -> iced::widget::Id {
    iced::widget::Id::new("sidebar-rename")
}

/// Widget id of the "New folder" dialog's name field, focused on open.
pub fn folder_name_input_id() -> iced::widget::Id {
    iced::widget::Id::new("folder-dialog-name")
}

/// The tree row a rename applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RenameTarget {
    Collection { id: String },
    Group { id: String },
    Request { id: String, collection_id: String },
    Environment { id: String },
}

impl RenameTarget {
    pub fn id(&self) -> &str {
        match self {
            RenameTarget::Collection { id }
            | RenameTarget::Group { id }
            | RenameTarget::Request { id, .. }
            | RenameTarget::Environment { id } => id,
        }
    }
}

/// An open rename field. Renames apply per keystroke, so the name it
/// started from is kept for Escape to restore.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rename {
    pub target: RenameTarget,
    pub original: String,
}

/// Where a dragged row lands relative to the row under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropPosition {
    /// Sibling of the target, placed just above it.
    Before,
    /// Child of the target (folders and collections only), appended last.
    Inside,
    /// Sibling of the target, placed just below it.
    After,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DropTarget {
    pub id: String,
    pub position: DropPosition,
}

/// One press on a tree row, from button-down to button-up.
///
/// A press stays a click until the pointer travels past the drag threshold,
/// so hand jitter still opens the request. The gesture always ends: on
/// release anywhere, on Escape, or when the window loses focus.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TreeDrag {
    pub item: String,
    /// The pointer moved past the threshold: this is a drag, not a click.
    pub active: bool,
    /// Valid drop slot currently under the pointer.
    pub target: Option<DropTarget>,
}

impl TreeDrag {
    pub fn pressed(item: String) -> Self {
        Self {
            item,
            active: false,
            target: None,
        }
    }
}

/// A collection or folder, whose deletion takes everything inside with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    Collection(String),
    Folder(String),
}

/// A deletion waiting for confirmation, with what it takes along.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDelete {
    pub target: DeleteTarget,
    pub name: String,
    pub requests: usize,
}

impl PendingDelete {
    /// "Delete 'Acme API' and its 24 requests?"
    pub fn question(&self) -> String {
        let name = &self.name;
        match self.requests {
            0 => format!("Delete '{name}'?"),
            1 => format!("Delete '{name}' and its request?"),
            n => format!("Delete '{name}' and its {n} requests?"),
        }
    }
}

/// The last single deletion, restorable until its toast expires.
#[derive(Debug, Clone)]
pub struct Undo {
    /// Which deletion this is, so the timer of an earlier one can't end it.
    pub seq: u64,
    pub item: Deleted,
}

#[derive(Debug, Clone)]
pub enum Deleted {
    /// A request, where it was in its collection, and the tabs it was open
    /// in with their modified flags.
    Request {
        request: Box<SavedRequest>,
        index: usize,
        tabs: Vec<(String, bool)>,
    },
    Environment {
        env: AppEnvironment,
        index: usize,
    },
}

impl Deleted {
    pub fn name(&self) -> &str {
        match self {
            Self::Request { request, .. } => &request.name,
            Self::Environment { env, .. } => &env.name,
        }
    }
}

/// Why an imported collection can't simply be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clash {
    /// It is collection `name` already here, with `requests` requests: it
    /// can replace it or come in as a copy.
    Replaces { name: String, requests: usize },
    /// Some of its folder or request ids belong to other collections, so
    /// only a copy with fresh ids leaves those intact.
    SharesIds,
}

/// An imported collection waiting for its clash to be resolved, and the
/// ones imported with it, still to be added after it.
#[derive(Debug, Clone)]
pub struct ImportClash {
    pub collection: ImportedCollection,
    pub clash: Clash,
    pub queued: Vec<ImportedCollection>,
    /// Requests the batch has added so far, and its import warnings, for
    /// the summary once the last collection is in.
    pub imported: usize,
    pub warnings: Vec<String>,
}

/// The Save dialog, for the tab it was opened for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveDialog {
    pub tab_id: String,
    pub name: String,
    /// The picked collection or folder; nothing is guessed.
    pub parent: Option<String>,
    /// The name typed into the inline "New collection" row while it is open.
    pub new_collection: Option<String>,
}

/// The header's "New folder" dialog: a name and where the folder goes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderDialog {
    pub name: String,
    /// The picked collection or folder; nothing is guessed.
    pub parent: Option<String>,
    /// The name typed into the inline "New collection" row while it is open.
    pub new_collection: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SidebarState {
    pub panel: SidebarPanel,
    pub expanded: HashSet<String>,
    pub renaming: Option<Rename>,
    pub drag: Option<TreeDrag>,
    /// Tree row whose action menu is open.
    pub context_menu: Option<String>,
    /// Last row pressed and when, for double-click-to-rename.
    pub last_press: Option<(String, Instant)>,
    pub env_editing: Option<String>,
    pub env_edit_rows: Vec<(String, String)>,
    pub collapsed: bool,
    /// Collection names for files a pending commit deletes, from the last commit.
    pub git_removed_names: HashMap<String, String>,
    /// Derived rows of the collections tree. Behind a `RefCell` because the
    /// view refreshes it when the workspace fingerprint changes, wherever
    /// the change came from.
    pub tree_cache: RefCell<TreeCache>,
    pub history_cache: RefCell<HistoryCache>,
    pub folder_dialog: Option<FolderDialog>,
    pub confirm_delete: Option<PendingDelete>,
    pub import_clash: Option<ImportClash>,
    /// The Export dialog's "Include secrets", off each time it opens.
    pub export_include_secrets: bool,
    pub undo: Option<Undo>,
    /// Counts deletions, numbering each [`Undo`].
    pub deletions: u64,
}

impl SidebarState {
    /// Brings the open environment's rows up to date with its stored
    /// variables, which a script's `set_env` may have changed: each key's
    /// first row takes the stored value, keys gone from the environment
    /// lose their rows, and new keys get one. Blank rows and later
    /// duplicates stay as typed.
    pub fn sync_env_rows(&mut self, environments: &[AppEnvironment]) {
        let Some(env) = self.env_editing.as_ref().and_then(|id| environments.iter().find(|e| &e.id == id)) else {
            return;
        };
        let mut seen = HashSet::new();
        self.env_edit_rows.retain_mut(|(key, value)| {
            let key = key.trim();
            if key.is_empty() || !seen.insert(key.to_owned()) {
                return true;
            }
            env.variables.get(key).inspect(|stored| value.clone_from(stored)).is_some()
        });
        let mut added: Vec<(String, String)> = env
            .variables
            .iter()
            .filter(|(key, _)| !seen.contains(key.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        added.sort();
        self.env_edit_rows.extend(added);
    }
}

/// Whether row `index` repeats the key of an earlier row, which wins.
pub fn shadowed(rows: &[(String, String)], index: usize) -> bool {
    let key = rows[index].0.trim();
    !key.is_empty() && rows[..index].iter().any(|(k, _)| k.trim() == key)
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            panel: SidebarPanel::Collections,
            expanded: HashSet::new(),
            renaming: None,
            drag: None,
            context_menu: None,
            last_press: None,
            env_editing: None,
            env_edit_rows: Vec::new(),
            collapsed: false,
            git_removed_names: HashMap::new(),
            tree_cache: RefCell::default(),
            history_cache: RefCell::default(),
            folder_dialog: None,
            confirm_delete: None,
            import_clash: None,
            export_include_secrets: false,
            undo: None,
            deletions: 0,
        }
    }
}
