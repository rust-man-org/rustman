//! Imported collections whose ids are already taken. A re-import of a
//! collection that is here may replace it; ids that belong to other
//! collections only ever come in as a copy, so nothing here is overwritten
//! unasked.

use crate::app::AppState;
use crate::app::tree::{insert_collection, root_collection_of};
use crate::domain::group::{ImportedCollection, ImportedGroup};
use crate::message::ClashChoice;
use crate::state::sidebar::{Clash, ImportClash};

/// Adds `collections` in order until one clashes; that one waits for an
/// answer with the rest queued behind it. Once all are in, the status line
/// sums up the batch.
pub(super) fn add(state: &mut AppState, collections: Vec<ImportedCollection>, mut imported: usize, warnings: Vec<String>) {
    let mut rest = collections.into_iter();
    while let Some(collection) = rest.next() {
        match clash(state, &collection) {
            None => {
                imported += collection.all_requests().len();
                insert_collection(state, collection);
            }
            Some(clash) => {
                state.sidebar.import_clash = Some(ImportClash {
                    collection,
                    clash,
                    queued: rest.collect(),
                    imported,
                    warnings,
                });
                return;
            }
        }
    }
    state.status_message = Some(super::import_summary(imported, &warnings));
}

pub(super) fn resolve(state: &mut AppState, choice: ClashChoice) {
    let Some(pending) = state.sidebar.import_clash.take() else { return };
    let requests = pending.collection.all_requests().len();
    let added = match (choice, &pending.clash) {
        (ClashChoice::Replace, Clash::Replaces { .. }) => {
            insert_collection(state, pending.collection);
            requests
        }
        (ClashChoice::Copy, _) => {
            let copy = as_copy(state, pending.collection);
            insert_collection(state, copy);
            requests
        }
        _ => 0,
    };
    add(state, pending.queued, pending.imported + added, pending.warnings);
}

fn clash(state: &AppState, imported: &ImportedCollection) -> Option<Clash> {
    let own = imported.collection.id.as_str();
    let foreign_request = imported
        .all_requests()
        .iter()
        .any(|r| state.requests.iter().any(|(collection, reqs)| collection != own && reqs.iter().any(|x| x.id == r.id)));
    let foreign_group = imported.all_groups().iter().any(|g| {
        state.groups.iter().any(|x| x.id == g.id) && root_collection_of(&state.groups, &g.id).as_deref() != Some(own)
    });
    if foreign_request || foreign_group {
        return Some(Clash::SharesIds);
    }
    let existing = state.collections.iter().find(|c| c.id == own)?;
    Some(Clash::Replaces {
        name: existing.name.clone(),
        requests: state.requests.get(own).map_or(0, Vec::len),
    })
}

/// `imported` with fresh ids throughout, links between them kept, and
/// " copy" after its name when that name is taken.
fn as_copy(state: &AppState, mut imported: ImportedCollection) -> ImportedCollection {
    let id = fresh();
    if state.collections.iter().any(|c| c.name == imported.collection.name) {
        imported.collection.name.push_str(" copy");
    }
    imported.collection.id.clone_from(&id);
    for request in &mut imported.requests {
        request.id = fresh();
        request.collection_id.clone_from(&id);
        request.group_id = None;
    }
    for group in &mut imported.groups {
        remint(group, &id, true, &id);
    }
    imported
}

fn remint(group: &mut ImportedGroup, parent_id: &str, parent_is_collection: bool, collection_id: &str) {
    group.id = fresh();
    parent_id.clone_into(&mut group.parent_id);
    group.parent_is_collection = parent_is_collection;
    for request in &mut group.requests {
        request.id = fresh();
        collection_id.clone_into(&mut request.collection_id);
        request.group_id = Some(group.id.clone());
    }
    let id = group.id.clone();
    for child in &mut group.groups {
        remint(child, &id, false, collection_id);
    }
}

fn fresh() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{ImportMsg, Message};
    use crate::test_support::TempDir;

    fn app() -> (AppState, TempDir) {
        let dir = TempDir::new("import-clash");
        (crate::app::boot::load(dir.path().to_path_buf()), dir)
    }

    fn import(state: &mut AppState, collections: Vec<ImportedCollection>) {
        let _ = crate::app::update::update(state, Message::Import(ImportMsg::CollectionsImported(collections.into())));
    }

    fn answer(state: &mut AppState, choice: ClashChoice) {
        let _ = crate::app::update::update(state, Message::Import(ImportMsg::ClashResolved(choice)));
    }

    fn ids(state: &AppState, collection: &str) -> Vec<String> {
        let mut ids: Vec<String> = state.requests[collection].iter().map(|r| r.id.clone()).collect();
        ids.sort();
        ids
    }

    #[test]
    fn a_re_import_asks_before_replacing_the_collection_here() {
        let (mut state, _dir) = app();
        let original = rustman_testkit::sample_collection();
        let id = original.collection.id.clone();
        import(&mut state, vec![original.clone()]);
        let mut changed = original.clone();
        changed.requests.truncate(1);
        changed.groups.clear();
        import(&mut state, vec![changed]);
        let pending = state.sidebar.import_clash.as_ref().expect("asks first");
        assert_eq!(pending.clash, Clash::Replaces { name: "Storefront API".into(), requests: original.all_requests().len() });
        assert_eq!(state.requests[&id].len(), original.all_requests().len(), "nothing replaced yet");
        answer(&mut state, ClashChoice::Replace);
        assert_eq!(state.requests[&id].len(), 1);
        assert!(state.sidebar.import_clash.is_none());
    }

    #[test]
    fn a_copy_gets_fresh_ids_and_keeps_its_folders() {
        let (mut state, _dir) = app();
        let original = rustman_testkit::sample_collection();
        let id = original.collection.id.clone();
        import(&mut state, vec![original.clone()]);
        let before = ids(&state, &id);
        import(&mut state, vec![original.clone()]);
        answer(&mut state, ClashChoice::Copy);
        assert_eq!(ids(&state, &id), before, "the collection here is untouched");
        let copy = state.collections.iter().find(|c| c.id != id).expect("a second collection");
        assert_eq!(copy.name, "Storefront API copy");
        let copied = &state.requests[&copy.id];
        assert_eq!(copied.len(), before.len());
        assert!(copied.iter().all(|r| !before.contains(&r.id)));
        for request in copied {
            if let Some(group) = &request.group_id {
                assert_eq!(root_collection_of(&state.groups, group).as_deref(), Some(copy.id.as_str()), "folder links follow the copy");
            }
        }
    }

    #[test]
    fn request_ids_of_another_collection_only_come_in_as_a_copy() {
        let (mut state, _dir) = app();
        let original = rustman_testkit::sample_collection();
        let id = original.collection.id.clone();
        import(&mut state, vec![original.clone()]);
        let mut other = original.clone();
        other.collection.id = "other".into();
        other.collection.name = "Other".into();
        import(&mut state, vec![other]);
        assert_eq!(state.sidebar.import_clash.as_ref().unwrap().clash, Clash::SharesIds);
        answer(&mut state, ClashChoice::Replace);
        assert!(!state.requests.contains_key("other"), "replace isn't offered, so it skips");
        assert_eq!(ids(&state, &id).len(), original.all_requests().len());
    }

    #[test]
    fn collections_after_a_clash_wait_for_its_answer() {
        let (mut state, _dir) = app();
        let original = rustman_testkit::sample_collection();
        import(&mut state, vec![original.clone()]);
        let mut fresh_one = as_copy(&state, original.clone());
        fresh_one.collection.name = "Fresh".into();
        let fresh_id = fresh_one.collection.id.clone();
        import(&mut state, vec![original, fresh_one]);
        assert!(!state.collections.iter().any(|c| c.id == fresh_id));
        answer(&mut state, ClashChoice::Skip);
        assert!(state.collections.iter().any(|c| c.id == fresh_id));
        let fresh_requests = state.requests[&fresh_id].len();
        assert_eq!(state.status_message, Some(format!("Imported {fresh_requests} requests")), "the skipped one isn't counted");
    }
}
