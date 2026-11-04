//! Collection tree index: the sidebar search filter. Built once per view
//! from plain slices, so it is testable without an `AppState`.

use std::collections::{HashMap, HashSet};

use crate::domain::{
    collection::{Collection, SavedRequest},
    group::Group,
};

pub struct TreeIndex {
    /// `None` while the search box is empty: everything shows.
    visible: Option<HashSet<String>>,
}

impl TreeIndex {
    pub fn build(
        collections: &[Collection],
        groups: &[Group],
        requests: &[&SavedRequest],
        query: &str,
    ) -> Self {
        let by_id: HashMap<&str, &Group> = groups.iter().map(|g| (g.id.as_str(), g)).collect();

        let q = query.trim().to_lowercase();
        let visible = (!q.is_empty()).then(|| {
            let hit = |s: &str| s.to_lowercase().contains(&q);
            // A named container that matches shows its whole subtree.
            let named: HashSet<&str> = groups
                .iter()
                .filter(|g| hit(&g.name))
                .map(|g| g.id.as_str())
                .chain(
                    collections
                        .iter()
                        .filter(|c| hit(&c.name))
                        .map(|c| c.id.as_str()),
                )
                .collect();
            let mut vis = HashSet::new();
            for r in requests {
                let (folders, _) = chain(&by_id, r.group_id.as_deref());
                let shown = hit(&r.name)
                    || hit(&r.url)
                    || named.contains(r.collection_id.as_str())
                    || folders.iter().any(|g| named.contains(g));
                if shown {
                    vis.insert(r.id.clone());
                    vis.insert(r.collection_id.clone());
                    vis.extend(folders.iter().map(|g| (*g).to_owned()));
                }
            }
            for g in groups {
                let (folders, col) = chain(&by_id, Some(&g.id));
                let col_named = col.is_some_and(|c| named.contains(c));
                if col_named || folders.iter().any(|f| named.contains(f)) {
                    vis.extend(folders.iter().map(|f| (*f).to_owned()));
                    vis.extend(col.map(str::to_owned));
                }
            }
            vis.extend(named.iter().map(|id| (*id).to_owned()));
            vis
        });

        Self { visible }
    }

    pub fn filtering(&self) -> bool {
        self.visible.is_some()
    }

    pub fn shows(&self, id: &str) -> bool {
        self.visible.as_ref().is_none_or(|v| v.contains(id))
    }
}

/// Folder ids from `start` up to its root folder, plus the collection the
/// root folder sits in. Guarded against cyclic parent links.
fn chain<'a>(
    by_id: &HashMap<&str, &'a Group>,
    start: Option<&str>,
) -> (Vec<&'a str>, Option<&'a str>) {
    let mut out = Vec::new();
    let mut cur = start;
    while let Some(id) = cur {
        let Some(g) = by_id.get(id) else { break };
        if out.contains(&g.id.as_str()) {
            break;
        }
        out.push(g.id.as_str());
        if g.parent_is_collection {
            return (out, Some(g.parent_id.as_str()));
        }
        cur = Some(g.parent_id.as_str());
    }
    (out, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(id: &str, name: &str) -> Collection {
        Collection {
            id: id.into(),
            name: name.into(),
            created_at: 0,
        }
    }

    fn group(id: &str, parent: &str, top: bool, name: &str) -> Group {
        Group {
            id: id.into(),
            parent_id: parent.into(),
            parent_is_collection: top,
            name: name.into(),
            sort_order: 0,
        }
    }

    fn req(id: &str, col: &str, group: Option<&str>, name: &str, url: &str) -> SavedRequest {
        let mut r = SavedRequest::new_in_group(col.into(), group.map(Into::into), name.into(), 0);
        r.id = id.into();
        r.url = url.into();
        r
    }

    /// Merchant API ─ auth ─ Login, Logout
    ///              └ merchant ─ orders ─ Orders
    /// Billing ─ Invoices
    fn fixture() -> (Vec<Collection>, Vec<Group>, Vec<SavedRequest>) {
        (
            vec![col("c1", "Merchant API"), col("c2", "Billing")],
            vec![
                group("auth", "c1", true, "auth"),
                group("merchant", "c1", true, "merchant"),
                group("orders", "merchant", false, "orders"),
            ],
            vec![
                req("login", "c1", Some("auth"), "Login", "{{base}}/login"),
                req("logout", "c1", Some("auth"), "Logout", "{{base}}/logout"),
                req("ord", "c1", Some("orders"), "Orders", "{{base}}/v2/orders"),
                req("inv", "c2", None, "Invoices", "{{base}}/invoices"),
            ],
        )
    }

    fn index(query: &str) -> TreeIndex {
        let (c, g, r) = fixture();
        let refs: Vec<&SavedRequest> = r.iter().collect();
        TreeIndex::build(&c, &g, &refs, query)
    }

    #[test]
    fn empty_query_shows_everything() {
        let ix = index("  ");
        assert!(!ix.filtering());
        assert!(ix.shows("anything"));
    }

    #[test]
    fn request_match_keeps_its_ancestors_only() {
        let ix = index("ORDERS");
        for id in ["ord", "orders", "merchant", "c1"] {
            assert!(ix.shows(id), "{id} should show");
        }
        for id in ["login", "auth", "c2", "inv"] {
            assert!(!ix.shows(id), "{id} should hide");
        }
    }

    #[test]
    fn url_matches_too() {
        let ix = index("/logout");
        assert!(ix.shows("logout") && ix.shows("auth") && ix.shows("c1"));
        assert!(!ix.shows("login"));
    }

    #[test]
    fn folder_match_shows_its_whole_subtree() {
        let ix = index("merch");
        // "merch" also hits the collection name, so everything in c1 shows.
        assert!(ix.shows("login") && ix.shows("orders") && ix.shows("ord"));
        let ix = index("auth");
        assert!(ix.shows("login") && ix.shows("logout") && ix.shows("c1"));
        assert!(!ix.shows("merchant") && !ix.shows("ord"));
    }

    #[test]
    fn collection_match_shows_its_requests() {
        let ix = index("billing");
        assert!(ix.shows("c2") && ix.shows("inv"));
        assert!(!ix.shows("c1"));
    }

    #[test]
    fn cyclic_parents_do_not_hang() {
        let groups = vec![group("a", "b", false, "a"), group("b", "a", false, "b")];
        let r = req("r", "c1", Some("a"), "R", "");
        let ix = TreeIndex::build(&[], &groups, &[&r], "a");
        assert!(ix.shows("a") && ix.shows("b") && ix.shows("r"));
    }
}
