//! Import / export collections modal (mockup `.exp-modal`): pick which
//! collections leave or enter the workspace before anything is written.

use std::collections::HashSet;

use crate::services::import::resources::ScanEntry;

/// Widget id of the modal's search box (focused on open).
pub const SEARCH_ID: &str = "collections-modal-search";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalTab {
    Export,
    Import,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// One `<name>.postman_collection.json` per collection.
    Postman,
    /// One Rustman JSON file with every selected collection.
    Rustman,
}

impl ExportFormat {
    pub const ALL: [Self; 2] = [Self::Postman, Self::Rustman];
}

impl std::fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Postman => "Postman Collection v2.1",
            Self::Rustman => "Rustman bundle (.json)",
        })
    }
}

#[derive(Debug)]
pub struct CollectionsModal {
    pub tab: ModalTab,
    /// Search box, shared by both tabs (cleared on tab switch).
    pub filter: String,
    // ── Import ──
    pub folder: Option<String>,
    pub scanning: bool,
    pub entries: Vec<ScanEntry>,
    /// Parallel to `entries`.
    pub selected: Vec<bool>,
    pub error: Option<String>,
    // ── Export ──
    pub export_selected: HashSet<String>,
    pub export_format: ExportFormat,
    /// Also write the active environment next to the collections.
    pub include_env: bool,
}

impl CollectionsModal {
    pub fn new(tab: ModalTab) -> Self {
        Self {
            tab,
            filter: String::new(),
            folder: None,
            scanning: false,
            entries: Vec::new(),
            selected: Vec::new(),
            error: None,
            export_selected: HashSet::new(),
            export_format: ExportFormat::Postman,
            include_env: true,
        }
    }

    /// Replace the scanned rows; importable ones start selected.
    pub fn set_entries(&mut self, entries: Vec<ScanEntry>) {
        self.selected = entries.iter().map(ScanEntry::importable).collect();
        self.entries = entries;
        self.error = None;
    }

    /// Case-insensitive match on name or file name.
    pub fn matches(&self, name: &str, file: &str) -> bool {
        let q = self.filter.trim().to_lowercase();
        q.is_empty() || name.to_lowercase().contains(&q) || file.to_lowercase().contains(&q)
    }

    /// Rows visible under the filter, as indices into `entries`.
    pub fn visible(&self) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&i| self.matches(&self.entries[i].name, &self.entries[i].file))
            .collect()
    }

    /// Select-all over the visible importable rows: on unless all are on.
    pub fn toggle_all(&mut self) {
        let rows: Vec<usize> = self
            .visible()
            .into_iter()
            .filter(|&i| self.entries[i].importable())
            .collect();
        let on = !rows.iter().all(|&i| self.selected[i]);
        for i in rows {
            self.selected[i] = on;
        }
    }

    pub fn toggle(&mut self, i: usize) {
        if self.entries.get(i).is_some_and(ScanEntry::importable) {
            self.selected[i] = !self.selected[i];
        }
    }

    /// Selected entries' resources, in row order.
    pub fn chosen(&self) -> Vec<&ScanEntry> {
        self.entries
            .iter()
            .zip(&self.selected)
            .filter(|(e, on)| **on && e.importable())
            .map(|(e, _)| e)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::import::resources::{ScanKind, ScanStatus, Summary};

    fn entry(name: &str, status: ScanStatus) -> ScanEntry {
        ScanEntry {
            file: format!("{name}.json"),
            name: name.into(),
            kind: ScanKind::Collection,
            summary: Summary::default(),
            status,
            resources: Vec::new(),
        }
    }

    #[test]
    fn errors_start_unselected_and_cannot_be_picked() {
        let mut m = CollectionsModal::new(ModalTab::Import);
        m.set_entries(vec![
            entry("a", ScanStatus::Ready),
            entry("b", ScanStatus::Error("bad".into())),
        ]);
        assert_eq!(m.selected, vec![true, false]);
        m.toggle(1);
        assert!(!m.selected[1]);
        assert_eq!(m.chosen().len(), 1);
    }

    #[test]
    fn select_all_follows_the_filter() {
        let mut m = CollectionsModal::new(ModalTab::Import);
        m.set_entries(vec![
            entry("merchant", ScanStatus::Ready),
            entry("payments", ScanStatus::Review),
        ]);
        m.filter = "pay".into();
        m.toggle_all();
        assert_eq!(m.selected, vec![true, false], "only the visible row flips");
    }
}
