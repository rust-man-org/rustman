use super::clone_name;
use crate::app::AppState;
use crate::domain::collection::{Collection, SavedRequest};
use crate::domain::request::HttpMethod;
use crate::message::{ImportMsg, Message, RequestMsg, SaveDialogMsg, SidebarMsg};
use crate::services::storage;
use crate::state::sidebar::{DropPosition, RenameTarget};
use crate::test_support::TempDir;
use crate::ui::sidebar::collections::{TreeCache, model};

/// A scratch workspace with two open collections: a = [r1, r2], b = [r3].
fn app() -> (AppState, TempDir) {
    let dir = TempDir::new("sidebar-tree");
    let mut state = crate::app::boot::load(dir.path().to_path_buf());
    for (cid, ids) in [("a", vec!["r1", "r2"]), ("b", vec!["r3"])] {
        let collection = Collection {
            id: cid.into(),
            name: cid.into(),
            created_at: 0,
        };
        storage::create_collection(&state.db(), &collection).unwrap();
        state.collections.push(collection);
        state.sidebar.expanded.insert(cid.into());
        for (order, rid) in ids.into_iter().enumerate() {
            let mut r = SavedRequest::new_in_group(cid.into(), None, rid.into(), order as i64);
            r.id = rid.into();
            storage::create_request(&state.db(), &r).unwrap();
            state.requests.entry(cid.into()).or_default().push(r);
        }
    }
    (state, dir)
}

fn send(state: &mut AppState, msg: Message) {
    let _ = crate::app::update::update(state, msg);
}

fn side(msg: SidebarMsg) -> Message {
    Message::Sidebar(msg)
}

fn order(state: &AppState, col: &str) -> Vec<String> {
    let mut rs: Vec<&SavedRequest> = state.requests[col].iter().collect();
    rs.sort_by_key(|r| r.sort_order);
    rs.iter().map(|r| r.id.clone()).collect()
}

fn active_request(state: &AppState) -> Option<&str> {
    state.tabs.active_tab().saved_as.as_ref().map(|(_, id)| id.as_str())
}

fn names(state: &AppState) -> Vec<String> {
    model::rows(state).into_iter().map(|r| r.name).collect()
}

#[test]
fn first_clone_of_an_untaken_name_gets_the_copy_suffix() {
    assert_eq!(clone_name("Get user", &["Get user"]), "Get user copy");
}

#[test]
fn repeated_clones_number_upward_ignoring_case() {
    assert_eq!(clone_name("Get user", &["Get user", "Get user copy", "Get user copy 2"]), "Get user copy 3");
    assert_eq!(clone_name("Get user", &["Get user", "GET USER COPY"]), "Get user copy 2");
}

#[test]
fn a_click_opens_the_request_and_leaves_no_drag_behind() {
    let (mut state, _dir) = app();
    send(&mut state, side(SidebarMsg::ItemPressed("r2".into())));
    assert!(state.sidebar.drag.is_some());
    send(&mut state, side(SidebarMsg::ItemDragEnd));
    assert!(state.sidebar.drag.is_none(), "no lingering move mode");
    assert_eq!(active_request(&state), Some("r2"));

    send(
        &mut state,
        side(SidebarMsg::ItemDragHover {
            id: "r1".into(),
            position: DropPosition::Before,
        }),
    );
    assert_eq!(order(&state, "a"), vec!["r1", "r2"]);
}

#[test]
fn a_drag_across_collections_moves_and_persists() {
    let (mut state, dir) = app();
    send(&mut state, side(SidebarMsg::ItemPressed("r1".into())));
    send(
        &mut state,
        side(SidebarMsg::ItemDragHover {
            id: "r3".into(),
            position: DropPosition::Before,
        }),
    );
    assert!(state.sidebar.drag.as_ref().unwrap().target.is_none(), "below the threshold nothing is a drop");

    send(&mut state, side(SidebarMsg::ItemDragStarted("r1".into())));
    send(
        &mut state,
        side(SidebarMsg::ItemDragHover {
            id: "r3".into(),
            position: DropPosition::After,
        }),
    );
    assert!(state.sidebar.drag.as_ref().unwrap().target.is_some());
    send(&mut state, side(SidebarMsg::ItemDragEnd));

    assert!(state.sidebar.drag.is_none());
    assert_eq!(order(&state, "a"), vec!["r2"]);
    assert_eq!(order(&state, "b"), vec!["r3", "r1"]);
    assert_ne!(active_request(&state), Some("r1"), "a drop is not a click");

    drop(state);
    let reloaded = crate::app::boot::load(dir.path().to_path_buf());
    assert_eq!(order(&reloaded, "b"), vec!["r3", "r1"]);
    assert_eq!(order(&reloaded, "a"), vec!["r2"]);
}

#[test]
fn invalid_targets_are_not_offered_and_escape_cancels() {
    let (mut state, _dir) = app();
    send(&mut state, side(SidebarMsg::ItemPressed("r1".into())));
    send(&mut state, side(SidebarMsg::ItemDragStarted("r1".into())));
    send(
        &mut state,
        side(SidebarMsg::ItemDragHover {
            id: "r2".into(),
            position: DropPosition::Inside,
        }),
    );
    assert!(state.sidebar.drag.as_ref().unwrap().target.is_none(), "requests have no inside");

    send(
        &mut state,
        side(SidebarMsg::ItemDragHover {
            id: "b".into(),
            position: DropPosition::Inside,
        }),
    );
    send(&mut state, side(SidebarMsg::ItemDragCancel));
    send(&mut state, side(SidebarMsg::ItemDragEnd));
    assert_eq!(order(&state, "a"), vec!["r1", "r2"], "cancelled: nothing moved");
    assert_ne!(active_request(&state), Some("r1"), "and no click either");
}

#[test]
fn the_backstop_only_touches_active_drags() {
    let (mut state, _dir) = app();
    send(&mut state, side(SidebarMsg::ItemPressed("r1".into())));
    send(&mut state, side(SidebarMsg::ItemDragStray { pressed: false }));
    assert!(state.sidebar.drag.is_some(), "a pending press is the row's business");

    send(&mut state, side(SidebarMsg::ItemDragStarted("r1".into())));
    send(
        &mut state,
        side(SidebarMsg::ItemDragHover {
            id: "b".into(),
            position: DropPosition::Inside,
        }),
    );
    send(&mut state, side(SidebarMsg::ItemDragStray { pressed: false }));
    assert!(state.sidebar.drag.is_none());
    assert_eq!(order(&state, "b"), vec!["r3", "r1"]);

    send(&mut state, side(SidebarMsg::ItemPressed("r2".into())));
    send(&mut state, side(SidebarMsg::ItemDragStarted("r2".into())));
    send(&mut state, side(SidebarMsg::ItemDragStray { pressed: true }));
    assert!(state.sidebar.drag.is_none());
}

#[test]
fn escape_restores_the_name_a_rename_started_from() {
    let (mut state, _dir) = app();
    send(&mut state, side(SidebarMsg::RenameStarted(RenameTarget::Collection { id: "a".into() })));
    send(&mut state, side(SidebarMsg::RenameEdited("Typed".into())));
    assert_eq!(state.collections[0].name, "Typed", "renames apply as you type");
    send(&mut state, side(SidebarMsg::CancelRename));
    assert_eq!(state.collections[0].name, "a");
    assert!(state.sidebar.renaming.is_none());
}

#[test]
fn renaming_a_request_retitles_its_open_tab() {
    let (mut state, _dir) = app();
    send(&mut state, side(SidebarMsg::RequestOpened("r1".into())));
    send(
        &mut state,
        side(SidebarMsg::RenameStarted(RenameTarget::Request {
            id: "r1".into(),
            collection_id: "a".into(),
        })),
    );
    send(&mut state, side(SidebarMsg::RenameEdited("List users".into())));
    send(&mut state, side(SidebarMsg::RenameFinished));
    assert_eq!(state.tabs.active_tab().title, "List users");
    assert!(state.sidebar.renaming.is_none());
}

fn stored_name(state: &AppState, collection: &str, id: &str) -> String {
    storage::get_requests(&state.db(), collection).unwrap().into_iter().find(|r| r.id == id).unwrap().name
}

#[test]
fn a_rename_is_stored_once_it_finishes_not_per_keystroke() {
    let (mut state, _dir) = app();
    let target = RenameTarget::Request {
        id: "r1".into(),
        collection_id: "a".into(),
    };
    send(&mut state, side(SidebarMsg::RenameStarted(target.clone())));
    for typed in ["L", "Li", "List"] {
        send(&mut state, side(SidebarMsg::RenameEdited(typed.into())));
    }
    assert_eq!(state.requests["a"][0].name, "List", "the row shows each keystroke");
    assert_eq!(stored_name(&state, "a", "r1"), "r1", "nothing is written while typing");
    send(&mut state, side(SidebarMsg::RenameFinished));
    assert_eq!(stored_name(&state, "a", "r1"), "List");

    send(&mut state, side(SidebarMsg::RenameStarted(target)));
    send(&mut state, side(SidebarMsg::RenameEdited("Gone".into())));
    send(&mut state, side(SidebarMsg::CancelRename));
    assert_eq!(state.requests["a"][0].name, "List");
    assert_eq!(stored_name(&state, "a", "r1"), "List", "a cancelled rename writes nothing");
}

#[test]
fn starting_another_rename_stores_the_one_in_progress() {
    let (mut state, _dir) = app();
    send(&mut state, side(SidebarMsg::RenameStarted(RenameTarget::Request { id: "r1".into(), collection_id: "a".into() })));
    send(&mut state, side(SidebarMsg::RenameEdited("First".into())));
    send(&mut state, side(SidebarMsg::RenameStarted(RenameTarget::Request { id: "r2".into(), collection_id: "a".into() })));
    assert_eq!(stored_name(&state, "a", "r1"), "First");
}

/// Enter is the only way a rename finishes, so a name typed and left open
/// is still on screen when the window closes.
#[test]
fn closing_the_window_stores_a_rename_left_open() {
    let (mut state, _dir) = app();
    send(&mut state, side(SidebarMsg::RenameStarted(RenameTarget::Request { id: "r1".into(), collection_id: "a".into() })));
    send(&mut state, side(SidebarMsg::RenameEdited("List users".into())));
    crate::app::session::save_before_exit(&mut state);
    assert_eq!(stored_name(&state, "a", "r1"), "List users");
}

/// The tree view rebuilds only when [`model::fingerprint`] changes, so every
/// way of changing the workspace must change it, and the rebuilt rows must
/// show the change.
#[test]
fn every_workspace_mutation_reaches_the_rendered_tree() {
    let (mut state, _dir) = app();
    let mut last = model::fingerprint(&state);
    let mut cache = TreeCache::default();
    let mut step = |state: &mut AppState, what: &str, msg: Message| {
        cache.sync(state);
        send(state, msg);
        let now = model::fingerprint(state);
        assert_ne!(now, last, "{what} left the tree fingerprint unchanged");
        last = now;
        let cached: Vec<&str> = cache.sync(state).iter().map(|l| l.row.name.as_str()).collect();
        assert_eq!(cached, names(state), "{what}: the cached rows are stale");
    };

    step(&mut state, "create collection", side(SidebarMsg::NewCollection));
    assert!(names(&state).contains(&"New Collection".to_owned()));

    step(
        &mut state,
        "create folder",
        side(SidebarMsg::NewGroupIn {
            parent_id: "a".into(),
            parent_is_collection: true,
        }),
    );
    assert!(names(&state).contains(&"New Folder".to_owned()));

    step(
        &mut state,
        "create request",
        side(SidebarMsg::NewRequestIn {
            parent_id: "b".into(),
            parent_is_collection: true,
        }),
    );
    assert!(names(&state).contains(&"New Request".to_owned()));

    send(&mut state, side(SidebarMsg::RenameStarted(RenameTarget::Collection { id: "b".into() })));
    step(&mut state, "rename", side(SidebarMsg::RenameEdited("Billing".into())));
    assert!(names(&state).contains(&"Billing".to_owned()));

    step(
        &mut state,
        "clone",
        side(SidebarMsg::CloneRequest {
            id: "r2".into(),
            collection_id: "a".into(),
        }),
    );
    assert!(names(&state).contains(&"r2 copy".to_owned()));
    send(&mut state, side(SidebarMsg::RenameFinished));

    send(&mut state, side(SidebarMsg::ItemPressed("r1".into())));
    send(&mut state, side(SidebarMsg::ItemDragStarted("r1".into())));
    send(
        &mut state,
        side(SidebarMsg::ItemDragHover {
            id: "r3".into(),
            position: DropPosition::After,
        }),
    );
    step(&mut state, "move", side(SidebarMsg::ItemDragEnd));
    let rows = names(&state);
    let r3 = rows.iter().position(|n| n == "r3").unwrap();
    assert_eq!(rows[r3 + 1], "r1", "the moved request renders after its new sibling");

    step(
        &mut state,
        "delete request",
        side(SidebarMsg::DeleteRequest {
            id: "r2".into(),
            collection_id: "a".into(),
        }),
    );
    assert!(!names(&state).contains(&"r2".to_owned()));

    step(
        &mut state,
        "import",
        Message::Import(ImportMsg::CollectionsImported(vec![crate::test_support::sample_collection()].into())),
    );
    assert!(names(&state).contains(&"Storefront API".to_owned()));

    send(&mut state, side(SidebarMsg::RequestOpened("r3".into())));
    send(&mut state, Message::Request(RequestMsg::MethodChanged(HttpMethod::Delete)));
    step(&mut state, "save with Ctrl+S", Message::Request(RequestMsg::SaveRequest));
    let r3 = model::rows(&state).into_iter().find(|r| r.id == "r3").unwrap();
    assert!(matches!(r3.kind, model::RowKind::Request { method: HttpMethod::Delete, .. }));

    send(&mut state, Message::Request(RequestMsg::NewTab));
    send(&mut state, Message::Request(RequestMsg::SaveRequest));
    send(&mut state, Message::SaveDialog(SaveDialogMsg::NameChanged("Saved here".into())));
    send(&mut state, Message::SaveDialog(SaveDialogMsg::ParentSelected("a".into())));
    step(&mut state, "save dialog", Message::SaveDialog(SaveDialogMsg::Confirm));
    assert!(names(&state).contains(&"Saved here".to_owned()));

    send(&mut state, side(SidebarMsg::ItemPressed("a".into())));
    step(&mut state, "collapse", side(SidebarMsg::ItemDragEnd));
    send(&mut state, side(SidebarMsg::DeleteAsked(crate::state::sidebar::DeleteTarget::Collection("a".into()))));
    step(&mut state, "delete collection", side(SidebarMsg::DeleteConfirmed));
    assert!(!names(&state).contains(&"Saved here".to_owned()));
}

/// `cargo test --release bench_tree_view -- --ignored --nocapture`
#[test]
#[ignore = "bench: tree view build"]
fn bench_tree_view() {
    let dir = TempDir::new("bench-tree");
    let mut state = crate::app::boot::load(dir.path().to_path_buf());
    state.store = None;
    for c in 0..10 {
        let cid = format!("c{c}");
        state.collections.push(Collection {
            id: cid.clone(),
            name: format!("Collection {c}"),
            created_at: 0,
        });
        state.sidebar.expanded.insert(cid.clone());
        for g in 0..5 {
            let group = crate::domain::group::Group::new(cid.clone(), true, format!("Folder {g}"), g);
            state.sidebar.expanded.insert(group.id.clone());
            for r in 0..10 {
                let name = format!("Request number {r} in folder {g}");
                let request = SavedRequest::new_in_group(cid.clone(), Some(group.id.clone()), name, r);
                state.requests.entry(cid.clone()).or_default().push(request);
            }
            state.groups.push(group);
        }
    }
    let time = |label: &str, f: &mut dyn FnMut()| {
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            f();
        }
        eprintln!("BENCH {label}: {:?} per build", start.elapsed() / 1000);
    };
    time("tree view, unchanged workspace", &mut || {
        std::hint::black_box(crate::ui::sidebar::collections::view(&state));
    });
    time("tree fingerprint", &mut || {
        std::hint::black_box(model::fingerprint(&state));
    });
    let mut flip = false;
    time("tree view after a rename", &mut || {
        flip = !flip;
        state.collections[0].name = if flip { "A".into() } else { "B".into() };
        std::hint::black_box(crate::ui::sidebar::collections::view(&state));
    });
}
