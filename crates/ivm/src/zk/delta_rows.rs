//! Flat public-span register deltas, retaining their original physical owners.
//!
//! Logical changes retain the existing ascending-index trace order. Every row
//! advances its physical change span by a public bound, including unchanged
//! values, so private contents never choose an allocation size or growth point.

use super::{
    RegisterState,
    trace_storage::{TraceCell, TraceRows, check_scope, next_capacity, overflow},
};
use crate::{
    VMError,
    execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan},
};
use iroha_allocation::{AllocationBudget, AllocationScope};
use iroha_crypto::zeroize_value_for_confidential_discard as erase;

/// One borrowed compact observation. Its changes cannot outlive the original log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeltaEntry<'a> {
    /// Original observed program counter.
    pub pc: u64,
    /// Initialized logical changes in canonical ascending-register order.
    pub changes: &'a [(usize, u64, bool)],
}
#[derive(Clone)]
pub(super) struct Row {
    pub(super) pc: u64,
    pub(super) start: usize,
    pub(super) count: usize,
}
impl TraceCell for Row {
    fn scrub(&mut self) {
        erase(&mut self.pc);
        erase(&mut self.start);
        erase(&mut self.count);
    }
}
impl TraceCell for (usize, u64, bool) {
    fn scrub(&mut self) {
        erase(&mut self.0);
        erase(&mut self.1);
        erase(&mut self.2);
    }
}
#[derive(Clone, Copy)]
struct Pending {
    rows: usize,
    next: usize,
    repeated: usize,
}

/// Move-only delta storage. No vector adoption or uncharged clone is provided.
pub struct DeltaTraceLog {
    rows: TraceRows<Row>,
    changes: TraceRows<(usize, u64, bool)>,
    last: Option<RegisterState>,
    pending: Option<Pending>,
    original: Option<AllocationBudget>,
}
impl DeltaTraceLog {
    /// Bind empty trace storage to the existing original pool.
    pub fn new(original: Option<&AllocationBudget>) -> Self {
        Self {
            rows: TraceRows::empty(original.is_some()),
            changes: TraceRows::empty(original.is_some()),
            last: None,
            pending: None,
            original: original.cloned(),
        }
    }
    /// Validate original refund custody before a compound trace operation.
    pub(crate) fn validate_scope(
        &self,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<(), VMError> {
        check_scope(self.original.as_ref(), scope)
    }
    /// Reserve an entire public observation batch before its semantic effects.
    ///
    /// The first observation always needs all 256 registers. Subsequent bounds
    /// come from the preceding instruction's public destination geometry. A
    /// repeated cycle/padding batch uses zero after that instruction's first
    /// state has been recorded. Overlapping batches cannot manufacture credit.
    pub(crate) fn prepare_batch(
        &mut self,
        rows: usize,
        first_changes: usize,
        repeated_changes: usize,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<(), VMError> {
        self.validate_scope(scope)?;
        if self.pending.is_some() || rows == 0 || first_changes > 256 || repeated_changes > 256 {
            return Err(VMError::ExecutionDeferred(
                crate::error::ExecutionDeferral::TraceOwnerUnavailable,
            ));
        }
        let first = if self.last.is_none() {
            256
        } else {
            first_changes
        };
        let slots = (rows - 1)
            .checked_mul(repeated_changes)
            .and_then(|slots| slots.checked_add(first))
            .ok_or_else(overflow)?;
        let row_count = self.len().checked_add(rows).ok_or_else(overflow)?;
        let change_count = self
            .changes
            .as_slice()
            .len()
            .checked_add(slots)
            .ok_or_else(overflow)?;
        self.grow(row_count, change_count)?;
        self.pending = Some(Pending {
            rows,
            next: first,
            repeated: repeated_changes,
        });
        Ok(())
    }
    fn grow(&mut self, rows: usize, changes: usize) -> Result<(), VMError> {
        let row_capacity = next_capacity(self.rows.capacity(), rows)?;
        let change_capacity = next_capacity(self.changes.capacity(), changes)?;
        let grow_rows = row_capacity != self.rows.capacity();
        let grow_changes = change_capacity != self.changes.capacity();
        let mut plan = ExecutionMemoryPlan::default();
        if grow_rows {
            plan.include_child(
                ExecutionMemoryPlan::array::<Row>(row_capacity)
                    .map_err(VMError::AllocationDeferred)?,
            )
            .map_err(VMError::AllocationDeferred)?;
        }
        if grow_changes {
            plan.include_child(
                ExecutionMemoryPlan::array::<(usize, u64, bool)>(change_capacity)
                    .map_err(VMError::AllocationDeferred)?,
            )
            .map_err(VMError::AllocationDeferred)?;
        }
        if !grow_rows && !grow_changes {
            return Ok(());
        }
        let mut lease = self
            .original
            .as_ref()
            .map(|original| ExecutionMemoryLease::reserve(original, plan))
            .transpose()
            .map_err(VMError::AllocationDeferred)?;
        // Allocate/copy both replacements before changing either live owner.
        let mut new_rows = if grow_rows {
            Some(TraceRows::allocate(row_capacity, lease.as_mut())?)
        } else {
            None
        };
        let mut new_changes = if grow_changes {
            Some(TraceRows::allocate(change_capacity, lease.as_mut())?)
        } else {
            None
        };
        if let Some(rows) = &mut new_rows {
            rows.copy_from(self.rows.as_slice());
        }
        if let Some(changes) = &mut new_changes {
            changes.copy_from(self.changes.as_slice());
        }
        let retired_rows = new_rows.map(|rows| std::mem::replace(&mut self.rows, rows));
        let retired_changes =
            new_changes.map(|changes| std::mem::replace(&mut self.changes, changes));
        drop((retired_rows, retired_changes));
        Ok(())
    }
    /// Record from the ordinary state without allocating or changing span demand.
    pub(crate) fn record_reserved(&mut self, pc: u64, gpr: [u64; 256], tags: [bool; 256]) {
        let pending = self.pending.expect("admitted delta observation batch");
        let changed = |index: usize| {
            self.last
                .as_ref()
                .is_none_or(|last| last.gpr[index] != gpr[index] || last.tags[index] != tags[index])
        };
        let count = (0..256).filter(|&index| changed(index)).count();
        assert!(
            count <= pending.next,
            "public delta destination bound underestimated"
        );
        let start = self.changes.as_slice().len();
        for index in 0..256 {
            if self
                .last
                .as_ref()
                .is_none_or(|last| last.gpr[index] != gpr[index] || last.tags[index] != tags[index])
            {
                self.changes.push_reserved((index, gpr[index], tags[index]));
            }
        }
        for _ in count..pending.next {
            self.changes.push_reserved((0, 0, false));
        }
        self.rows.push_reserved(Row { pc, start, count });
        self.clear_last();
        self.last = Some(RegisterState { pc, gpr, tags });
        self.pending = (pending.rows > 1).then_some(Pending {
            rows: pending.rows - 1,
            next: pending.repeated,
            repeated: pending.repeated,
        });
    }
    /// Internal immutable capture inputs, including the full public capacities.
    pub(super) fn capture_storage(
        &self,
    ) -> (
        &TraceRows<Row>,
        &TraceRows<(usize, u64, bool)>,
        Option<&AllocationBudget>,
    ) {
        (&self.rows, &self.changes, self.original.as_ref())
    }
    /// Number of initialized logical observations.
    pub fn len(&self) -> usize {
        self.rows.as_slice().len()
    }
    /// Whether no observation is initialized.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Borrow one observation without exposing private spare slots.
    pub fn entry(&self, index: usize) -> Option<DeltaEntry<'_>> {
        self.rows.as_slice().get(index).map(|row| DeltaEntry {
            pc: row.pc,
            changes: &self.changes.as_slice()[row.start..row.start + row.count],
        })
    }
    /// Borrow logical observations in unchanged trace-hash order.
    pub fn entries(
        &self,
    ) -> impl ExactSizeIterator<Item = DeltaEntry<'_>> + DoubleEndedIterator + std::iter::FusedIterator
    {
        (0..self.len()).map(|index| self.entry(index).expect("initialized delta descriptor"))
    }
    /// Exact retained physical capacities independent of logical change counts.
    pub(crate) fn allocated_bytes(&self) -> Result<usize, VMError> {
        let mut plan = ExecutionMemoryPlan::array::<Row>(self.rows.capacity())
            .map_err(VMError::AllocationDeferred)?;
        plan.include_child(
            ExecutionMemoryPlan::array::<(usize, u64, bool)>(self.changes.capacity())
                .map_err(VMError::AllocationDeferred)?,
        )
        .map_err(VMError::AllocationDeferred)?;
        Ok(plan.requested_bytes())
    }
    fn clear_last(&mut self) {
        if let Some(last) = &mut self.last {
            erase(&mut last.pc);
            erase(&mut last.gpr);
            erase(&mut last.tags);
        }
        self.last = None;
    }
    /// Finish a refused or terminal batch without erasing prior observations.
    ///
    /// Only its enclosing instruction/invocation owner may discard this credit;
    /// nested callbacks must leave the parent's pending observation intact.
    /// Unobserved spans have no initialized contents, and their backing remains
    /// charged to the original owner for a subsequent before-effects admission.
    pub(crate) fn discard_unobserved(&mut self) {
        self.pending = None;
    }
    /// Scrub private contents, retaining original backing for subsequent batches.
    pub(crate) fn scrub(&mut self) {
        self.rows.clear();
        self.changes.clear();
        self.clear_last();
        self.pending = None;
    }
    /// Retire storage while preserving the original pool identity.
    pub(crate) fn reset(&mut self, scope: Option<&AllocationScope<'_>>) -> Result<(), VMError> {
        self.validate_scope(scope)?;
        self.scrub();
        let rows = std::mem::replace(&mut self.rows, TraceRows::empty(self.original.is_some()));
        let changes =
            std::mem::replace(&mut self.changes, TraceRows::empty(self.original.is_some()));
        drop((rows, changes));
        Ok(())
    }
    /// Independent snapshot backing, including public spans and pending credit.
    #[cfg(test)]
    pub(crate) fn try_clone_allocation(
        &self,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<Self, VMError> {
        self.validate_scope(scope)?;
        let mut copy = Self::new(self.original.as_ref());
        // Preserve the full public capacity so copying never shrinks according
        // to private logical counts or loses already admitted future rows.
        copy.grow(self.rows.capacity(), self.changes.capacity())?;
        copy.rows.copy_from(self.rows.as_slice());
        copy.changes.copy_from(self.changes.as_slice());
        copy.last = self.last.clone();
        copy.pending = self.pending;
        Ok(copy)
    }
}
impl Drop for DeltaTraceLog {
    fn drop(&mut self) {
        self.clear_last();
    }
}
#[cfg(test)]
mod tests;
