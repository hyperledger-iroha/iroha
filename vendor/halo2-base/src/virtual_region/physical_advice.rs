//! Exact physical advice coordinates represented by contiguous virtual-cell runs.

use std::collections::BTreeMap;

use crate::{halo2_proofs::circuit::Cell, ContextCell, ContextTag};

#[derive(Clone, Copy, Debug)]
struct CoordinateRun {
    start: u32,
    end: u32,
    first: Cell,
}

impl CoordinateRun {
    fn resolve(self, offset: u32) -> Cell {
        debug_assert!(self.start <= offset && offset <= self.end);
        Cell {
            column: self.first.column,
            row_offset: self
                .first
                .row_offset
                .checked_add((offset - self.start) as usize)
                .expect("physical advice run row overflow"),
        }
    }

    fn joins(self, next: Self) -> bool {
        self.end.checked_add(1) == Some(next.start)
            && self.first.column == next.first.column
            && self
                .first
                .row_offset
                .checked_add((self.end - self.start) as usize)
                .and_then(|last| last.checked_add(1))
                == Some(next.first.row_offset)
    }
}

/// Synthesis-local mapping from every virtual advice identity to its exact physical cell.
///
/// Contiguous virtual offsets with consecutive rows in one physical column share a run.
/// Irregular inserts and overwritten cells split runs; they never fall back to a hidden
/// per-cell hash table. The worst-case run count is the number of mapped cells, so callers
/// must inspect the actual run count before claiming a storage reduction. No iteration of
/// this map determines the ordering of copy constraints.
#[derive(Clone, Default, Debug)]
pub struct PhysicalAdviceMap {
    contexts: BTreeMap<ContextTag, Vec<CoordinateRun>>,
    len: usize,
}

impl PhysicalAdviceMap {
    /// Returns the number of mapped virtual cells, including every cell inside a run.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether no virtual cell is mapped.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of distinct virtual contexts with physical coordinates.
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    /// Returns the actual number of coordinate runs, including irregular singleton runs.
    pub fn run_count(&self) -> usize {
        self.contexts.values().map(Vec::len).sum()
    }

    /// Returns the checked capacity of the run vectors in bytes, excluding map-node overhead.
    pub fn checked_run_capacity_bytes(&self) -> Option<usize> {
        self.contexts.values().try_fold(0_usize, |total, runs| {
            total.checked_add(
                runs.capacity()
                    .checked_mul(std::mem::size_of::<CoordinateRun>())?,
            )
        })
    }

    /// Reconstructs an owned physical cell, or returns `None` for an unmapped identity.
    pub fn resolve(&self, cell: &ContextCell) -> Option<Cell> {
        let runs = self.contexts.get(&(cell.type_id(), cell.context_id()))?;
        let offset = u32::try_from(cell.offset()).expect("ContextCell has a bounded u32 offset");
        let position = runs.partition_point(|run| run.start <= offset);
        let run = *runs.get(position.checked_sub(1)?)?;
        (offset <= run.end).then(|| run.resolve(offset))
    }

    /// Returns whether the exact virtual cell has a physical assignment.
    pub fn contains_key(&self, cell: &ContextCell) -> bool {
        self.resolve(cell).is_some()
    }

    /// Inserts the exact physical coordinate and returns its previous value if present.
    ///
    /// Repeated insertion of identical coordinates leaves the runs unchanged. Overwrites
    /// retain ordinary map semantics, including at an interior run offset; the assignment
    /// caller remains responsible for rejecting an inconsistent repeated synthesis.
    pub fn insert(&mut self, cell: ContextCell, physical: Cell) -> Option<Cell> {
        let offset = u32::try_from(cell.offset()).expect("ContextCell has a bounded u32 offset");
        let runs = self
            .contexts
            .entry((cell.type_id(), cell.context_id()))
            .or_default();
        let position = runs.partition_point(|run| run.start <= offset);
        if position > 0 && offset <= runs[position - 1].end {
            let index = position - 1;
            let original = runs[index];
            let previous = original.resolve(offset);
            if previous.column == physical.column && previous.row_offset == physical.row_offset {
                return Some(previous);
            }
            let mut replacement = Vec::with_capacity(3);
            if original.start < offset {
                replacement.push(CoordinateRun {
                    end: offset - 1,
                    ..original
                });
            }
            replacement.push(CoordinateRun {
                start: offset,
                end: offset,
                first: physical,
            });
            if offset < original.end {
                replacement.push(CoordinateRun {
                    start: offset + 1,
                    end: original.end,
                    first: original.resolve(offset + 1),
                });
            }
            let count = replacement.len();
            runs.splice(index..=index, replacement);
            Self::coalesce(runs, index, count);
            return Some(previous);
        }
        let next_len = self
            .len
            .checked_add(1)
            .expect("physical advice map length overflow");
        runs.insert(
            position,
            CoordinateRun {
                start: offset,
                end: offset,
                first: physical,
            },
        );
        Self::coalesce(runs, position, 1);
        self.len = next_len;
        None
    }

    fn coalesce(runs: &mut Vec<CoordinateRun>, inserted: usize, count: usize) {
        let mut left = inserted.saturating_sub(1);
        let mut last = (inserted + count).min(runs.len() - 1);
        while left < last {
            if runs[left].joins(runs[left + 1]) {
                runs[left].end = runs[left + 1].end;
                runs.remove(left + 1);
                last -= 1;
            } else {
                left += 1;
            }
        }
    }

    /// Drops all physical coordinates between layouter invocations.
    ///
    /// Virtual advice and copy constraints are owned separately and remain available for replay.
    pub fn clear(&mut self) {
        self.contexts.clear();
        self.len = 0;
    }
}

#[cfg(test)]
#[path = "physical_advice_tests.rs"]
mod tests;
