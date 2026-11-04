//! Local drafts (workstream 01, contract C9).
//!
//! Every synced endpoint has a **shared base** (the `requests` row, mirrored
//! from the repo) and at most one **local draft**: the user's edited copy,
//! stored only on this machine. Sync replaces the base and then calls
//! [`RequestDraft::rebase`], a per-field three-way merge, so local edits
//! survive pulls without ever being committed.
//!
//! Everything here is pure: no I/O, no clock, no app state.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::collection::SavedRequest;

macro_rules! draft_fields {
    ($($variant:ident => $field:ident),* $(,)?) => {
        /// One locally editable field of a request. The merge unit in V1:
        /// list-valued fields (`headers`, `params`, …) compare as a whole.
        ///
        /// Structure (`name`, `group_id`, `sort_order`) is deliberately not
        /// here: it is shared-only and always follows the base.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum DraftField {
            $($variant),*
        }

        impl DraftField {
            pub const ALL: &'static [DraftField] = &[$(DraftField::$variant),*];

            /// Comparable form of this field. JSON, so field types need no
            /// `PartialEq` of their own.
            fn value(self, r: &SavedRequest) -> Value {
                match self {
                    $(DraftField::$variant => serde_json::to_value(&r.$field).unwrap_or(Value::Null)),*
                }
            }

            fn copy(self, from: &SavedRequest, to: &mut SavedRequest) {
                match self {
                    $(DraftField::$variant => to.$field = from.$field.clone()),*
                }
            }
        }
    };
}

draft_fields! {
    Method => method,
    Url => url,
    Params => params,
    Headers => headers,
    Body => body,
    BodyType => body_type,
    GraphqlVariables => graphql_variables,
    AuthType => auth_type,
    BearerToken => bearer_token,
    BasicUser => basic_user,
    BasicPass => basic_pass,
    ApiKeyName => api_key_name,
    ApiKeyValue => api_key_value,
    ApiKeyLocation => api_key_location,
    FormDataFields => form_data_fields,
    CookieString => cookie_string,
    Cookies => cookies,
    JwtSecret => jwt_secret,
    JwtSubject => jwt_subject,
    JwtAlgo => jwt_algo,
    PreRequestScript => pre_request_script,
    TestScript => test_script,
}

impl DraftField {
    fn differs(self, a: &SavedRequest, b: &SavedRequest) -> bool {
        self.value(a) != self.value(b)
    }

    /// Human name for banners and diff tables.
    pub fn label(self) -> &'static str {
        match self {
            Self::Method => "Method",
            Self::Url => "URL",
            Self::Params => "Params",
            Self::Headers => "Headers",
            Self::Body => "Body",
            Self::BodyType => "Body type",
            Self::GraphqlVariables => "GraphQL variables",
            Self::AuthType => "Auth type",
            Self::BearerToken => "Bearer token",
            Self::BasicUser => "Basic user",
            Self::BasicPass => "Basic password",
            Self::ApiKeyName => "API key name",
            Self::ApiKeyValue => "API key value",
            Self::ApiKeyLocation => "API key location",
            Self::FormDataFields => "Form data",
            Self::CookieString => "Cookie string",
            Self::Cookies => "Cookies",
            Self::JwtSecret => "JWT secret",
            Self::JwtSubject => "JWT subject",
            Self::JwtAlgo => "JWT algorithm",
            Self::PreRequestScript => "Pre-request script",
            Self::TestScript => "Test script",
        }
    }

    /// Fields whose values are never shown on screen (design rules: no
    /// secrets in UI, screenshots or errors).
    pub fn is_secret(self) -> bool {
        matches!(
            self,
            Self::BearerToken | Self::BasicPass | Self::ApiKeyValue | Self::JwtSecret
        )
    }

    /// One-line, length-capped rendering of this field of `r` for the UI.
    /// Secrets are masked.
    pub fn preview(self, r: &SavedRequest) -> String {
        const MAX: usize = 80;
        let value = self.value(r);
        if self.is_secret() {
            return match &value {
                Value::String(s) if s.is_empty() => "(empty)".to_owned(),
                _ => "•••••• (hidden)".to_owned(),
            };
        }
        let raw = match &value {
            Value::String(s) if s.is_empty() => return "(empty)".to_owned(),
            Value::String(s) => s.clone(),
            Value::Array(a) if a.is_empty() => return "(none)".to_owned(),
            Value::Array(a) => format!("{} item(s): {}", a.len(), value_compact(&value)),
            other => value_compact(other),
        };
        let one_line: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut chars = one_line.chars();
        let head: String = chars.by_ref().take(MAX).collect();
        if chars.next().is_some() {
            format!("{head}…")
        } else {
            head
        }
    }
}

fn value_compact(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The review branch/PR a draft was sent to with **Save** (C12). The draft
/// stays local until the PR merges and `main` is pulled; then it collapses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRef {
    pub branch: String,
    /// `None` while the branch is pushed but the PR is not open yet.
    #[serde(default)]
    pub pr_number: Option<u64>,
}

/// Which side wins when the user resolves a conflicting field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Keep the local value; acknowledges the current upstream value.
    Mine,
    /// Replace the local value with the current base value.
    Upstream,
}

/// Derived status, for badges and banners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftState {
    /// Base unchanged since the last rebase.
    Clean,
    /// The last rebase moved the base and merged it without conflicts.
    UpstreamChanged,
    /// Fields edited both locally and upstream; resolve before Save.
    Conflict(Vec<DraftField>),
    /// Sent for review and waiting for the PR to merge (C12).
    InReview(ReviewRef),
    /// The endpoint no longer exists upstream.
    Orphaned,
}

/// A user's local working copy of one synced request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestDraft {
    /// Request `id` for now; becomes `endpoint_id` once C1 lands (D3).
    pub request_id: String,
    pub collection_id: String,
    /// The user's edited copy: what the tab shows and Send uses.
    pub data: SavedRequest,
    /// The base this draft was last rebased onto.
    pub base: SavedRequest,
    pub updated_at: i64,
    /// Unresolved conflicting fields. Durable: survives repeated syncs and
    /// restarts until the user resolves each one explicitly.
    #[serde(default)]
    pub conflicts: Vec<DraftField>,
    /// Review branch/PR this draft was sent to (C12). Kept across edits, so
    /// saving again updates the same branch and PR.
    #[serde(default)]
    pub review: Option<ReviewRef>,
    /// Set by the last [`rebase`](Self::rebase) when the base moved.
    /// Transient: not persisted, cleared on restart.
    #[serde(skip)]
    pub upstream_changed: bool,
    /// The base row disappeared upstream. Derived on load and on sync.
    #[serde(skip)]
    pub orphaned: bool,
}

impl RequestDraft {
    /// Start a draft from `base` with the user's edited copy `data`.
    pub fn new(base: &SavedRequest, data: SavedRequest, now: i64) -> Self {
        let mut draft = Self {
            request_id: base.id.clone(),
            collection_id: base.collection_id.clone(),
            data,
            base: base.clone(),
            updated_at: now,
            conflicts: Vec::new(),
            review: None,
            upstream_changed: false,
            orphaned: false,
        };
        draft.pin_structure();
        draft
    }

    /// Replace the edited copy (keeps base, conflicts and structure).
    pub fn set_data(&mut self, data: SavedRequest, now: i64) {
        self.data = data;
        self.updated_at = now;
        self.pin_structure();
    }

    pub fn state(&self) -> DraftState {
        if self.orphaned {
            DraftState::Orphaned
        } else if !self.conflicts.is_empty() {
            DraftState::Conflict(self.conflicts.clone())
        } else if let Some(review) = &self.review {
            DraftState::InReview(review.clone())
        } else if self.upstream_changed {
            DraftState::UpstreamChanged
        } else {
            DraftState::Clean
        }
    }

    /// Fields where the draft differs from its base (for "View diff").
    pub fn changed_fields(&self) -> Vec<DraftField> {
        DraftField::ALL
            .iter()
            .copied()
            .filter(|f| f.differs(&self.data, &self.base))
            .collect()
    }

    /// Nothing left to keep: no local difference and no open conflict.
    /// The caller deletes such a draft.
    pub fn is_collapsed(&self) -> bool {
        self.conflicts.is_empty() && !self.orphaned && self.changed_fields().is_empty()
    }

    /// Three-way merge onto a new base (`old` = `self.base`):
    ///
    /// | condition            | result                         |
    /// |----------------------|--------------------------------|
    /// | already in conflicts | keep mine, keep the marker     |
    /// | mine == old          | take new (user did not touch)  |
    /// | new == old           | keep mine (upstream untouched) |
    /// | mine == new          | same value, no conflict        |
    /// | otherwise            | keep mine, add a conflict      |
    ///
    /// Structure always follows the new base.
    pub fn rebase(mut self, new_base: &SavedRequest) -> Self {
        let old = &self.base;
        let mut upstream_changed = false;
        for &field in DraftField::ALL {
            let (mine, before, after) = (
                field.value(&self.data),
                field.value(old),
                field.value(new_base),
            );
            if before != after {
                upstream_changed = true;
            }
            if self.conflicts.contains(&field) || mine == after {
                continue;
            }
            if mine == before {
                field.copy(new_base, &mut self.data);
            } else if after != before {
                self.conflicts.push(field);
            }
        }
        self.base = new_base.clone();
        self.collection_id = new_base.collection_id.clone();
        self.upstream_changed = upstream_changed;
        self.orphaned = false;
        self.pin_structure();
        self
    }

    /// Resolve one conflicting field. No-op if it is not in conflict.
    pub fn resolve(&mut self, field: DraftField, side: Side) {
        if !self.conflicts.contains(&field) {
            return;
        }
        if side == Side::Upstream {
            field.copy(&self.base, &mut self.data);
        }
        self.conflicts.retain(|f| *f != field);
    }

    /// **Save** (send for review, C12) is allowed only with every conflict
    /// resolved and the endpoint still present upstream. Permission (C10) is
    /// checked by the caller.
    pub fn can_publish(&self) -> bool {
        self.conflicts.is_empty() && !self.orphaned
    }

    /// The request committed to the review branch on Save: the draft's data
    /// with the base's identity and structure.
    #[allow(dead_code)] // first caller: 08 Save for review (C12)
    pub fn published_request(&self) -> SavedRequest {
        let mut out = self.data.clone();
        Self::copy_structure(&self.base, &mut out);
        out
    }

    fn pin_structure(&mut self) {
        Self::copy_structure(&self.base, &mut self.data);
    }

    fn copy_structure(from: &SavedRequest, to: &mut SavedRequest) {
        to.id = from.id.clone();
        to.collection_id = from.collection_id.clone();
        to.group_id = from.group_id.clone();
        to.sort_order = from.sort_order;
        to.name = from.name.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::request::{HttpMethod, KeyValue};

    fn base() -> SavedRequest {
        let mut r = SavedRequest::new_in("col".into(), "Login".into());
        r.id = "req-1".into();
        r.method = HttpMethod::Post;
        r.url = "https://a.test/v1/login".into();
        r.body = r#"{"user":"a"}"#.into();
        r
    }

    fn draft_with(edit: impl FnOnce(&mut SavedRequest)) -> RequestDraft {
        let b = base();
        let mut data = b.clone();
        edit(&mut data);
        RequestDraft::new(&b, data, 1)
    }

    fn upstream(edit: impl FnOnce(&mut SavedRequest)) -> SavedRequest {
        let mut r = base();
        edit(&mut r);
        r
    }

    #[test]
    fn untouched_field_takes_upstream() {
        let d = draft_with(|r| r.body = "mine".into());
        let d = d.rebase(&upstream(|r| r.url = "https://a.test/v2/login".into()));
        assert_eq!(d.data.url, "https://a.test/v2/login");
        assert_eq!(d.data.body, "mine");
        assert!(d.conflicts.is_empty());
        assert_eq!(d.state(), DraftState::UpstreamChanged);
    }

    #[test]
    fn upstream_untouched_keeps_mine() {
        let d = draft_with(|r| r.url = "https://mine.test".into());
        let d = d.rebase(&base());
        assert_eq!(d.data.url, "https://mine.test");
        assert_eq!(d.state(), DraftState::Clean);
    }

    #[test]
    fn same_change_on_both_sides_is_not_a_conflict() {
        let d = draft_with(|r| r.url = "https://same.test".into());
        let d = d.rebase(&upstream(|r| r.url = "https://same.test".into()));
        assert!(d.conflicts.is_empty());
        assert!(d.is_collapsed(), "draft equals new base, caller deletes it");
    }

    #[test]
    fn different_changes_conflict_and_keep_mine() {
        let d = draft_with(|r| r.url = "https://mine.test".into());
        let d = d.rebase(&upstream(|r| r.url = "https://theirs.test".into()));
        assert_eq!(d.data.url, "https://mine.test");
        assert_eq!(d.conflicts, vec![DraftField::Url]);
        assert_eq!(d.state(), DraftState::Conflict(vec![DraftField::Url]));
        assert!(!d.can_publish());
    }

    #[test]
    fn list_fields_merge_as_a_whole() {
        let kv = |k: &str| KeyValue {
            id: k.into(),
            key: k.into(),
            value: "1".into(),
            enabled: true,
        };
        let d = draft_with(|r| r.headers = vec![kv("X-Mine")]);
        let d = d.rebase(&upstream(|r| r.headers = vec![kv("X-Theirs")]));
        assert_eq!(d.conflicts, vec![DraftField::Headers]);
    }

    #[test]
    fn conflict_survives_an_unchanged_second_sync() {
        let d = draft_with(|r| r.url = "https://mine.test".into());
        let theirs = upstream(|r| r.url = "https://theirs.test".into());
        let d = d.rebase(&theirs).rebase(&theirs);
        assert_eq!(d.conflicts, vec![DraftField::Url]);
    }

    #[test]
    fn conflict_marker_holds_even_when_values_later_match() {
        let d = draft_with(|r| r.url = "https://mine.test".into());
        let d = d.rebase(&upstream(|r| r.url = "https://theirs.test".into()));
        // Upstream later changes to exactly my value: still needs resolution.
        let d = d.rebase(&upstream(|r| r.url = "https://mine.test".into()));
        assert_eq!(d.conflicts, vec![DraftField::Url]);
        assert!(!d.can_publish());
    }

    #[test]
    fn conflict_list_has_no_duplicates() {
        let d = draft_with(|r| r.url = "https://mine.test".into());
        let d = d
            .rebase(&upstream(|r| r.url = "https://b.test".into()))
            .rebase(&upstream(|r| r.url = "https://c.test".into()));
        assert_eq!(d.conflicts, vec![DraftField::Url]);
    }

    #[test]
    fn resolve_mine_keeps_value_and_clears_only_that_field() {
        let d = draft_with(|r| {
            r.url = "https://mine.test".into();
            r.body = "mine".into();
        });
        let mut d = d.rebase(&upstream(|r| {
            r.url = "https://theirs.test".into();
            r.body = "theirs".into();
        }));
        d.resolve(DraftField::Url, Side::Mine);
        assert_eq!(d.data.url, "https://mine.test");
        assert_eq!(d.conflicts, vec![DraftField::Body]);
        assert!(!d.can_publish());
        d.resolve(DraftField::Body, Side::Mine);
        assert!(d.can_publish());
    }

    #[test]
    fn resolve_upstream_copies_base_value() {
        let d = draft_with(|r| r.url = "https://mine.test".into());
        let mut d = d.rebase(&upstream(|r| r.url = "https://theirs.test".into()));
        d.resolve(DraftField::Url, Side::Upstream);
        assert_eq!(d.data.url, "https://theirs.test");
        assert!(d.conflicts.is_empty());
        assert!(d.is_collapsed());
    }

    #[test]
    fn structure_always_follows_base() {
        let d = draft_with(|r| {
            r.name = "Renamed locally".into();
            r.sort_order = 99;
        });
        assert_eq!(d.data.name, "Login", "structure is pinned at creation");
        let d = d.rebase(&upstream(|r| {
            r.name = "Sign in".into();
            r.group_id = Some("auth".into());
        }));
        assert_eq!(d.data.name, "Sign in");
        assert_eq!(d.data.group_id.as_deref(), Some("auth"));
        assert!(d.conflicts.is_empty());
    }

    #[test]
    fn published_request_carries_data_with_base_identity() {
        let mut d = draft_with(|r| r.body = "mine".into());
        d.data.id = "tampered".into();
        let out = d.published_request();
        assert_eq!(out.id, "req-1");
        assert_eq!(out.body, "mine");
    }

    #[test]
    fn orphaned_draft_cannot_publish_and_keeps_conflicts() {
        let d = draft_with(|r| r.url = "https://mine.test".into());
        let mut d = d.rebase(&upstream(|r| r.url = "https://theirs.test".into()));
        d.orphaned = true;
        assert_eq!(d.state(), DraftState::Orphaned);
        assert!(!d.can_publish());
        assert_eq!(d.conflicts, vec![DraftField::Url]);
        assert!(!d.is_collapsed());
    }

    fn review() -> ReviewRef {
        ReviewRef {
            branch: "review/yosua/login".into(),
            pr_number: Some(12),
        }
    }

    #[test]
    fn review_survives_edits_and_rebase() {
        let mut d = draft_with(|r| r.body = "mine".into());
        d.review = Some(review());
        d.set_data(d.data.clone(), 2);
        let d = d.rebase(&upstream(|r| r.url = "https://a.test/v2".into()));
        assert_eq!(d.review, Some(review()));
        assert_eq!(d.state(), DraftState::InReview(review()));
    }

    #[test]
    fn merged_pr_collapses_the_draft_on_pull() {
        let mut d = draft_with(|r| r.body = "mine".into());
        d.review = Some(review());
        // PR merged: `main` now carries exactly the draft's change.
        let merged = upstream(|r| r.body = "mine".into());
        assert!(d.rebase(&merged).is_collapsed());
    }

    #[test]
    fn conflict_and_orphan_outrank_review_in_state() {
        let mut d = draft_with(|r| r.url = "https://mine.test".into());
        d.review = Some(review());
        let mut d = d.rebase(&upstream(|r| r.url = "https://theirs.test".into()));
        assert_eq!(d.state(), DraftState::Conflict(vec![DraftField::Url]));
        d.orphaned = true;
        assert_eq!(d.state(), DraftState::Orphaned);
    }

    #[test]
    fn previews_mask_secrets_and_cap_length() {
        let mut r = base();
        r.bearer_token = "super-secret".into();
        assert!(!DraftField::BearerToken.preview(&r).contains("secret"));
        r.bearer_token.clear();
        assert_eq!(DraftField::BearerToken.preview(&r), "(empty)");

        r.body = "{\n  \"a\": 1\n}".into();
        assert_eq!(DraftField::Body.preview(&r), "{ \"a\": 1 }");
        r.body = "x".repeat(500);
        assert!(DraftField::Body.preview(&r).chars().count() <= 81);
        assert!(DraftField::ALL.iter().all(|f| !f.label().is_empty()));
    }

    #[test]
    fn serde_round_trip_keeps_conflicts() {
        let d = draft_with(|r| r.url = "https://mine.test".into());
        let d = d.rebase(&upstream(|r| r.url = "https://theirs.test".into()));
        let json = serde_json::to_string(&d).unwrap();
        let back: RequestDraft = serde_json::from_str(&json).unwrap();
        assert_eq!(back.conflicts, vec![DraftField::Url]);
        assert_eq!(back.data.url, "https://mine.test");
        assert_eq!(back.state(), DraftState::Conflict(vec![DraftField::Url]));
    }
}
