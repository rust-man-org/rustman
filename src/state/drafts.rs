//! UI/session state for workstream 01 (local drafts, C9). Owned by 01.

use std::collections::HashMap;

use crate::domain::collection::SavedRequest;
use crate::domain::draft::RequestDraft;

/// Every local draft, keyed by request id (D3; later `endpoint_id`).
#[derive(Debug, Default)]
pub struct DraftsState {
    pub by_request: HashMap<String, RequestDraft>,
}

impl DraftsState {
    /// Build from stored drafts, marking those whose base row is gone as
    /// orphaned. Durable conflicts are kept as stored.
    pub fn from_loaded(
        drafts: Vec<RequestDraft>,
        requests: &HashMap<String, Vec<SavedRequest>>,
    ) -> Self {
        let mut by_request = HashMap::new();
        for mut d in drafts {
            d.orphaned = !base_exists(requests, &d.request_id);
            by_request.insert(d.request_id.clone(), d);
        }
        Self { by_request }
    }

    pub fn get(&self, request_id: &str) -> Option<&RequestDraft> {
        self.by_request.get(request_id)
    }

    pub fn count(&self) -> usize {
        self.by_request.len()
    }
}

pub(crate) fn base_exists(requests: &HashMap<String, Vec<SavedRequest>>, request_id: &str) -> bool {
    requests
        .values()
        .any(|reqs| reqs.iter().any(|r| r.id == request_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: &str) -> SavedRequest {
        let mut r = SavedRequest::new_in("col".into(), id.into());
        r.id = id.into();
        r
    }

    #[test]
    fn missing_base_marks_draft_orphaned_on_load() {
        let present = req("a");
        let gone = req("b");
        let drafts = vec![
            RequestDraft::new(&present, present.clone(), 1),
            RequestDraft::new(&gone, gone.clone(), 1),
        ];
        let mut requests = HashMap::new();
        requests.insert("col".to_owned(), vec![present]);

        let state = DraftsState::from_loaded(drafts, &requests);
        assert!(!state.get("a").unwrap().orphaned);
        assert!(state.get("b").unwrap().orphaned);
        assert_eq!(state.count(), 2);
    }
}
