use crate::app::AppState;
use crate::ui::theme::TEXT_MD;
use crate::ui::tree_label::Measured;

use super::model::{self, Row};
use super::row;

/// The tree's rows with their labels measured, rebuilt only when the
/// workspace fingerprint changes and re-fitted only when the width does.
#[derive(Debug, Clone, Default)]
pub struct TreeCache {
    fingerprint: Option<u64>,
    width_bits: Option<u32>,
    lines: Vec<Line>,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub row: Row,
    pub label: Measured,
    /// The label cut to the current width, or `None` when it fits whole.
    pub fitted: Option<String>,
}

impl TreeCache {
    pub fn sync(&mut self, state: &AppState) -> &[Line] {
        let fingerprint = model::fingerprint(state);
        if self.fingerprint != Some(fingerprint) {
            self.lines = model::rows(state)
                .into_iter()
                .map(|row| Line {
                    label: Measured::new(row.name.clone(), TEXT_MD, row::face(&row)),
                    row,
                    fitted: None,
                })
                .collect();
            self.fingerprint = Some(fingerprint);
            self.width_bits = None;
        }
        let width = state.sidebar_width;
        if self.width_bits != Some(width.to_bits()) {
            for line in &mut self.lines {
                line.fitted = line.label.fitted(row::label_budget(&line.row, width));
            }
            self.width_bits = Some(width.to_bits());
        }
        &self.lines
    }
}
