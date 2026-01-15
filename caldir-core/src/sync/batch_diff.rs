//! Batch diff aggregation for multiple calendars.

use crate::sync::{CalendarDiff, DiffKind, EventDiff};

pub struct BatchDiff(pub Vec<CalendarDiff>);

impl BatchDiff {
    fn count_by_kind<'a>(diffs: impl Iterator<Item = &'a EventDiff>) -> (usize, usize, usize) {
        let mut created = 0;
        let mut updated = 0;
        let mut deleted = 0;

        for diff in diffs {
            match diff.kind {
                DiffKind::Create => created += 1,
                DiffKind::Update => updated += 1,
                DiffKind::Delete => deleted += 1,
            }
        }

        (created, updated, deleted)
    }

    pub fn pull_counts(&self) -> (usize, usize, usize) {
        Self::count_by_kind(self.0.iter().flat_map(|d| &d.to_pull))
    }

    pub fn push_counts(&self) -> (usize, usize, usize) {
        Self::count_by_kind(self.0.iter().flat_map(|d| &d.to_push))
    }
}
