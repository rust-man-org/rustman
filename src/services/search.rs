//! Local sidebar search. Results contain safe labels and locations, never body/script values.
use crate::domain::{
    collection::{Collection, SavedRequest},
    group::Group,
};
use crate::services::format::head_capped;
use std::collections::{HashMap, HashSet};

pub const MAX_RESULTS: usize = 30;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Endpoint,
    Folder,
    BodyScript,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentField {
    Body,
    PreRequest,
    Tests,
}
#[derive(Debug, Clone)]
pub struct Hit {
    pub id: String,
    pub collection_id: String,
    pub kind: Kind,
    pub name: String,
    pub path: String,
    pub method: String,
    pub detail: String,
    pub field: Option<ContentField>,
    pub score: u32,
}

/// All words must match; higher scores favor names over paths and content.
/// Query/result caps bound work and UI size. Content is inspected but not copied
/// into hits. Credential-specific fields are deliberately excluded.
pub fn find(
    collections: &[Collection],
    groups: &[Group],
    requests: &[&SavedRequest],
    query: &str,
) -> Vec<Hit> {
    let query: String = query.chars().take(128).collect();
    let words: Vec<_> = query
        .split_whitespace()
        .take(12)
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let by_id: HashMap<_, _> = groups.iter().map(|g| (g.id.as_str(), g)).collect();
    let col_names: HashMap<_, _> = collections
        .iter()
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();
    let mut descendants: HashMap<&str, Vec<&SavedRequest>> = HashMap::new();
    let mut hits = Vec::new();
    for req in requests {
        let ancestors = ancestors(&by_id, req.group_id.as_deref());
        for g in &ancestors {
            descendants.entry(g.id.as_str()).or_default().push(req);
        }
        let col_name = col_names
            .get(req.collection_id.as_str())
            .copied()
            .unwrap_or("Collection");
        let path = std::iter::once(col_name)
            .chain(ancestors.iter().rev().map(|g| g.name.as_str()))
            .collect::<Vec<_>>()
            .join(" / ");
        let direct = [
            req.name.to_lowercase(),
            req.url.to_lowercase(),
            path.to_lowercase(),
            req.method.as_str().to_lowercase(),
        ];
        let content = std::cell::OnceCell::new();
        let mut score = 0;
        let mut field = None;
        let matched = words.iter().all(|word| {
            let best = direct
                .iter()
                .zip([100, 40, 20, 10])
                .filter_map(|(s, w)| fuzzy_score(s, word).map(|v| v + w))
                .max();
            if let Some(v) = best {
                score += v;
                return true;
            }
            let best = content
                .get_or_init(|| {
                    [
                        req.body.to_lowercase(),
                        req.pre_request_script.to_lowercase(),
                        req.test_script.to_lowercase(),
                    ]
                })
                .iter()
                .enumerate()
                .filter_map(|(i, s)| fuzzy_score(s, word).map(|v| (v, i)))
                .max();
            if let Some((v, i)) = best {
                score += v;
                field = Some(
                    [
                        ContentField::Body,
                        ContentField::PreRequest,
                        ContentField::Tests,
                    ][i],
                );
                true
            } else {
                false
            }
        });
        if !matched {
            continue;
        }
        let detail = match field {
            None => format!("{} {}", req.method.as_str(), head_capped(&req.url, 180)),
            Some(ContentField::Body) => "Matched in request body".into(),
            Some(ContentField::PreRequest) => "Matched in pre-request script".into(),
            Some(ContentField::Tests) => "Matched in test script".into(),
        };
        hits.push(Hit {
            id: req.id.clone(),
            collection_id: req.collection_id.clone(),
            kind: if field.is_some() {
                Kind::BodyScript
            } else {
                Kind::Endpoint
            },
            name: head_capped(&req.name, 160).into(),
            path: head_capped(&path, 200).into(),
            method: req.method.as_str().into(),
            detail,
            field,
            score,
        });
    }
    for group in groups {
        let ancestors = ancestors(&by_id, Some(&group.id));
        let Some(root) = ancestors.last().filter(|g| g.parent_is_collection) else {
            continue;
        };
        let col_name = col_names
            .get(root.parent_id.as_str())
            .copied()
            .unwrap_or("Collection");
        let path = std::iter::once(col_name)
            .chain(ancestors.iter().rev().skip(1).map(|g| g.name.as_str()))
            .collect::<Vec<_>>()
            .join(" / ");
        let children = descendants
            .get(group.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let name = group.name.to_lowercase();
        let context = path.to_lowercase();
        let names: Vec<_> = children.iter().map(|r| r.name.to_lowercase()).collect();
        let mut score = 0;
        if !words.iter().all(|word| {
            let best = fuzzy_score(&name, word)
                .map(|v| v + 100)
                .or_else(|| fuzzy_score(&context, word))
                .or_else(|| names.iter().filter_map(|n| fuzzy_score(n, word)).max());
            if let Some(v) = best {
                score += v;
                true
            } else {
                false
            }
        }) {
            continue;
        }
        let contained = children
            .iter()
            .take(3)
            .map(|r| head_capped(&r.name, 45))
            .collect::<Vec<_>>()
            .join(", ");
        hits.push(Hit {
            id: group.id.clone(),
            collection_id: root.parent_id.clone(),
            kind: Kind::Folder,
            name: head_capped(&group.name, 160).into(),
            path: head_capped(&path, 200).into(),
            method: String::new(),
            detail: format!(
                "{} requests{}{}",
                children.len(),
                if contained.is_empty() {
                    ""
                } else {
                    " · contains "
                },
                contained
            ),
            field: None,
            score,
        });
    }
    hits.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.id.cmp(&b.id))
    });
    // Each group gets a fair share even when many endpoints match.
    let mut per_kind = [0; 3];
    hits.retain(|h| {
        let n = &mut per_kind[h.kind as usize];
        *n += 1;
        *n <= 10
    });
    hits.truncate(MAX_RESULTS);
    hits
}

fn ancestors<'a>(groups: &HashMap<&str, &'a Group>, mut id: Option<&'a str>) -> Vec<&'a Group> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    while let Some(g) = id.and_then(|id| groups.get(id)).copied() {
        if !seen.insert(g.id.as_str()) {
            break;
        }
        result.push(g);
        if g.parent_is_collection {
            break;
        }
        id = Some(&g.parent_id);
    }
    result
}

fn fuzzy_score(text: &str, needle: &str) -> Option<u32> {
    if text == needle {
        return Some(240);
    }
    if text.starts_with(needle) {
        return Some(210);
    }
    if text.contains(needle) {
        return Some(180);
    }
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter_map(|word| {
            let chars: Vec<_> = word.chars().take(128).collect();
            let query: Vec<_> = needle.chars().collect();
            let mut pos = 0;
            for c in &chars {
                if query.get(pos) == Some(c) {
                    pos += 1;
                }
            }
            if query.len() >= 2 && pos == query.len() {
                return Some(110);
            }
            (query.len() >= 4 && one_edit(&chars, &query)).then_some(80)
        })
        .max()
}

/// One insertion, deletion, replacement or adjacent transposition.
fn one_edit(a: &[char], b: &[char]) -> bool {
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let first = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()));
    if first == a.len().min(b.len()) {
        return true;
    }
    match a.len().cmp(&b.len()) {
        std::cmp::Ordering::Greater => a[first + 1..] == b[first..],
        std::cmp::Ordering::Less => a[first..] == b[first + 1..],
        std::cmp::Ordering::Equal => {
            a[first + 1..] == b[first + 1..]
                || (first + 1 < a.len()
                    && a[first] == b[first + 1]
                    && a[first + 1] == b[first]
                    && a[first + 2..] == b[first + 2..])
        }
    }
}

#[cfg(test)]
mod tests;
