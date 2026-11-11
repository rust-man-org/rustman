//! Credentials stay in the local database: collection files in git hold
//! them blanked by [`rustman_core::secrets`], and loading a file back must
//! never let those blanks replace the values kept locally.

use std::collections::HashMap;

use rustman_core::domain::collection::SavedRequest;
use rustman_core::domain::group::{ImportedCollection, ImportedGroup};
use rustman_core::domain::request::KeyValue;
use rustman_core::secrets::{is_secret_value, strip_secrets};

/// Fills the secrets an incoming (pulled, checked out or restored)
/// collection lacks from `local`, the same collection's requests as stored
/// now.
///
/// A blank incoming secret never replaces a local one. A non-blank one, from
/// a commit made before secrets were stripped, fills only a blank local
/// field; a changed `{{variable}}` reference follows git. Header and cookie rows are
/// matched by id and merged only where stripping would have blanked them,
/// so other edits to them still come from git.
pub fn keep_local_secrets(incoming: &mut ImportedCollection, local: &[SavedRequest]) {
    let local: HashMap<&str, &SavedRequest> = local.iter().map(|r| (r.id.as_str(), r)).collect();
    for_each_request(incoming, |theirs| {
        if let Some(mine) = local.get(theirs.id.as_str()) {
            merge(theirs, mine);
        }
    });
}

fn merge(theirs: &mut SavedRequest, mine: &SavedRequest) {
    let mut stripped = mine.clone();
    strip_secrets(&mut stripped);
    keep(&mut theirs.bearer_token, &mine.bearer_token);
    keep(&mut theirs.basic_pass, &mine.basic_pass);
    keep(&mut theirs.api_key_value, &mine.api_key_value);
    keep(&mut theirs.jwt_secret, &mine.jwt_secret);
    keep(&mut theirs.cookie_string, &mine.cookie_string);
    keep_rows(&mut theirs.headers, &mine.headers, &stripped.headers);
    keep_rows(&mut theirs.cookies, &mine.cookies, &stripped.cookies);
}

/// The local value wins, unless both are `{{variable}}` references: then
/// git's choice of variable does. A real local secret is never replaced.
fn keep(theirs: &mut String, mine: &str) {
    let reference_from_git =
        !theirs.trim().is_empty() && !is_secret_value(theirs) && !is_secret_value(mine);
    if !mine.is_empty() && !reference_from_git {
        mine.clone_into(theirs);
    }
}

/// [`keep`] for the rows whose local value stripping blanks.
fn keep_rows(theirs: &mut [KeyValue], mine: &[KeyValue], stripped: &[KeyValue]) {
    for row in theirs {
        let local = mine.iter().zip(stripped).find(|(m, _)| m.id == row.id);
        if let Some((mine, stripped)) = local
            && stripped.value != mine.value
        {
            keep(&mut row.value, &mine.value);
        }
    }
}

fn for_each_request(collection: &mut ImportedCollection, mut f: impl FnMut(&mut SavedRequest)) {
    fn walk(group: &mut ImportedGroup, f: &mut impl FnMut(&mut SavedRequest)) {
        group.requests.iter_mut().for_each(&mut *f);
        for child in &mut group.groups {
            walk(child, f);
        }
    }
    collection.requests.iter_mut().for_each(&mut f);
    for group in &mut collection.groups {
        walk(group, &mut f);
    }
}
