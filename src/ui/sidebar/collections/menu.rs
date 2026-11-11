use iced::Element;

use crate::message::{FolderDialogMsg, ImportMsg, Message, RequestMsg, SidebarMsg};
use crate::state::sidebar::{DeleteTarget, RenameTarget};
use crate::ui::components::menu::{self, Entry, Item};
use crate::ui::components::tooltip::shortcut;
use crate::ui::icons;
use crate::ui::theme::{Tokens, UI_FONT};

use super::model::{Row, RowKind};

const ROW_MENU_WIDTH: f32 = 200.0;
const NEW_MENU_WIDTH: f32 = 320.0;

/// The actions of one tree row, shown under its "…" button.
pub fn row_menu(t: &'static Tokens, row: &Row) -> Element<'static, Message> {
    let side = Message::Sidebar;
    let id = row.id.clone();
    let mut entries: Vec<Entry<'static, Message>> = Vec::new();
    let delete = |msg: SidebarMsg| -> Entry<'static, Message> {
        Item::new("Delete", side(msg))
            .icon(icons::trash())
            .styled(t.on_raised.status.error, UI_FONT)
            .into()
    };
    match &row.kind {
        RowKind::Collection { .. } | RowKind::Folder { .. } => {
            let parent_is_collection = matches!(row.kind, RowKind::Collection { .. });
            entries.push(new_request(id.clone(), parent_is_collection));
            entries.push(new_folder(id.clone(), parent_is_collection));
            entries.push(Entry::Separator);
            let target = if parent_is_collection {
                RenameTarget::Collection { id: id.clone() }
            } else {
                RenameTarget::Group { id: id.clone() }
            };
            entries.push(rename(target));
            if parent_is_collection {
                entries.push(
                    Item::new("Export…", Message::Import(ImportMsg::OpenExportDialog(id.clone())))
                        .icon(icons::export())
                        .into(),
                );
            }
            entries.push(Entry::Separator);
            entries.push(delete(SidebarMsg::DeleteAsked(if parent_is_collection {
                DeleteTarget::Collection(id)
            } else {
                DeleteTarget::Folder(id)
            })));
        }
        RowKind::Request { collection_id, .. } => {
            entries.push(
                Item::new(
                    "Duplicate",
                    side(SidebarMsg::CloneRequest {
                        id: id.clone(),
                        collection_id: collection_id.clone(),
                    }),
                )
                .icon(icons::copy())
                .into(),
            );
            entries.push(rename(RenameTarget::Request {
                id: id.clone(),
                collection_id: collection_id.clone(),
            }));
            entries.push(Entry::Separator);
            entries.push(delete(SidebarMsg::DeleteRequest {
                id,
                collection_id: collection_id.clone(),
            }));
        }
        RowKind::Empty { .. } => {}
    }
    menu::panel(t, entries, ROW_MENU_WIDTH)
}

/// The sidebar header's "New" menu. "New folder" asks where the folder goes.
pub fn new_menu(t: &'static Tokens) -> Element<'static, Message> {
    let entries = vec![
        Item::new("New request", Message::Request(RequestMsg::NewTab))
            .icon(icons::file_plus())
            .shortcut(shortcut("T"))
            .into(),
        Item::new("New collection", Message::Sidebar(SidebarMsg::NewCollection))
            .icon(icons::folder_plus())
            .into(),
        Item::new("New folder", Message::Sidebar(SidebarMsg::FolderDialog(FolderDialogMsg::Opened)))
            .icon(icons::folder())
            .into(),
        Entry::Separator,
        Item::new(
            "Import cURL / Postman / OpenAPI…",
            Message::Request(RequestMsg::OpenImportDialog),
        )
        .icon(icons::import())
        .shortcut(shortcut("I"))
        .into(),
        Item::new("Export…", Message::Import(ImportMsg::ExportAllCollections))
            .icon(icons::export())
            .into(),
    ];
    menu::panel(t, entries, NEW_MENU_WIDTH)
}

fn new_request(parent_id: String, parent_is_collection: bool) -> Entry<'static, Message> {
    Item::new(
        "New request",
        Message::Sidebar(SidebarMsg::NewRequestIn {
            parent_id,
            parent_is_collection,
        }),
    )
    .icon(icons::file_plus())
    .into()
}

fn new_folder(parent_id: String, parent_is_collection: bool) -> Entry<'static, Message> {
    Item::new(
        "New folder",
        Message::Sidebar(SidebarMsg::NewGroupIn {
            parent_id,
            parent_is_collection,
        }),
    )
    .icon(icons::folder())
    .into()
}

fn rename(target: RenameTarget) -> Entry<'static, Message> {
    Item::new("Rename", Message::Sidebar(SidebarMsg::RenameStarted(target)))
        .icon(icons::edit())
        .into()
}
