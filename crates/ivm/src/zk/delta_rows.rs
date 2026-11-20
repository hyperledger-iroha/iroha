//! Flat public-span register deltas, retaining their original physical owners.
//!
//! Logical changes retain the existing ascending-index trace order. Every row
//! advances its physical change span by a public bound, including unchanged
//! values, so private contents never choose an allocation size or growth point.

use super::{
    RegisterState,
    trace_storage::{TraceCell, TraceRows, check_scope, next_capacity, overflow},
};
use crate::error::VmDeferral;
use crate::{
    VMError,
    execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan},
    parallel::REGISTER_COUNT,
    registers::BASELINE_REGISTER_COUNT,
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
/// The baseline engine's complete register image at one observation.
#[derive(Clone)]
struct BaselineRegisterState {
    pc: u64,
    gpr: [u64; BASELINE_REGISTER_COUNT],
    tags: [bool; BASELINE_REGISTER_COUNT],
}
/// The preceding observation's complete image, in the register geometry of the engine that
/// recorded it: C10's 32 registers or the baseline's 256.
#[derive(Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "inline like the image it replaces: boxing would allocate outside the funded trace budget"
)]
enum LastImage {
    C10(RegisterState),
    Baseline(BaselineRegisterState),
}
#[derive(Clone, Copy)]
struct Pending {
    rows: usize,
    next: usize,
    repeated: usize,
}

/// Move-only delta storage. No vector adoption or uncharged clone is provided.
///
/// A log records the register geometry of the engine that runs: C10 observations through
/// [`Self::prepare_batch`] and [`Self::record_reserved`] (32 registers), baseline observations
/// through [`Self::prepare_baseline_batch`] and [`Self::record_baseline_reserved`] (256). The
/// first observation of a geometry records the complete image.
pub struct DeltaTraceLog {
    rows: TraceRows<Row>,
    changes: TraceRows<(usize, u64, bool)>,
    last: Option<LastImage>,
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
    /// The first observation always needs all 32 registers. Subsequent bounds
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
        let continues = matches!(self.last, Some(LastImage::C10(_)));
        self.prepare_geometry(
            REGISTER_COUNT,
            continues,
            rows,
            first_changes,
            repeated_changes,
            scope,
        )
    }
    /// Reserve a batch of baseline observations: [`Self::prepare_batch`] over the baseline
    /// engine's 256 registers. The first baseline observation needs all 256.
    pub(crate) fn prepare_baseline_batch(
        &mut self,
        rows: usize,
        first_changes: usize,
        repeated_changes: usize,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<(), VMError> {
        let continues = matches!(self.last, Some(LastImage::Baseline(_)));
        self.prepare_geometry(
            BASELINE_REGISTER_COUNT,
            continues,
            rows,
            first_changes,
            repeated_changes,
            scope,
        )
    }
    /// Reserve `rows` observations of a `width`-register image. `continues` says whether the
    /// preceding observation has the same geometry; otherwise the first row needs `width`.
    fn prepare_geometry(
        &mut self,
        width: usize,
        continues: bool,
        rows: usize,
        first_changes: usize,
        repeated_changes: usize,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<(), VMError> {
        self.validate_scope(scope)?;
        if self.pending.is_some() || rows == 0 || first_changes > width || repeated_changes > width
        {
            return Err(VMError::Deferred(VmDeferral::ExecutionDeferred(
                crate::error::ExecutionDeferral::TraceOwnerUnavailable,
            )));
        }
        let first = if continues { first_changes } else { width };
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
                    .map_err(|cause| VMError::Deferred(VmDeferral::AllocationDeferred(cause)))?,
            )
            .map_err(|cause| VMError::Deferred(VmDeferral::AllocationDeferred(cause)))?;
        }
        if grow_changes {
            plan.include_child(
                ExecutionMemoryPlan::array::<(usize, u64, bool)>(change_capacity)
                    .map_err(|cause| VMError::Deferred(VmDeferral::AllocationDeferred(cause)))?,
            )
            .map_err(|cause| VMError::Deferred(VmDeferral::AllocationDeferred(cause)))?;
        }
        if !grow_rows && !grow_changes {
            return Ok(());
        }
        let mut lease = self
            .original
            .as_ref()
            .map(|original| ExecutionMemoryLease::reserve(original, plan))
            .transpose()
            .map_err(|cause| VMError::Deferred(VmDeferral::AllocationDeferred(cause)))?;
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
    pub(crate) fn record_reserved(
        &mut self,
        pc: u64,
        gpr: [u64; REGISTER_COUNT],
        tags: [bool; REGISTER_COUNT],
    ) {
        let pending = self.pending.expect("admitted delta observation batch");
        let previous = match &self.last {
            Some(LastImage::C10(last)) => Some((&last.gpr[..], &last.tags[..])),
            _ => None,
        };
        record_changes(
            &mut self.rows,
            &mut self.changes,
            pending,
            pc,
            previous,
            &gpr,
            &tags,
        );
        self.clear_last();
        self.last = Some(LastImage::C10(RegisterState { pc, gpr, tags }));
        self.advance(pending);
    }
    /// Record one baseline observation: [`Self::record_reserved`] over the baseline engine's
    /// 256 registers, against the preceding baseline image.
    pub(crate) fn record_baseline_reserved(
        &mut self,
        pc: u64,
        gpr: &[u64; BASELINE_REGISTER_COUNT],
        tags: &[bool; BASELINE_REGISTER_COUNT],
    ) {
        let pending = self.pending.expect("admitted delta observation batch");
        let previous = match &self.last {
            Some(LastImage::Baseline(last)) => Some((&last.gpr[..], &last.tags[..])),
            _ => None,
        };
        record_changes(
            &mut self.rows,
            &mut self.changes,
            pending,
            pc,
            previous,
            gpr,
            tags,
        );
        match &mut self.last {
            Some(LastImage::Baseline(last)) => {
                last.pc = pc;
                last.gpr = *gpr;
                last.tags = *tags;
            }
            _ => {
                self.clear_last();
                self.last = Some(LastImage::Baseline(BaselineRegisterState {
                    pc,
                    gpr: *gpr,
                    tags: *tags,
                }));
            }
        }
        self.advance(pending);
    }
    /// Move to the next observation of the admitted batch.
    fn advance(&mut self, pending: Pending) {
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
            .map_err(|cause| VMError::Deferred(VmDeferral::AllocationDeferred(cause)))?;
        plan.include_child(
            ExecutionMemoryPlan::array::<(usize, u64, bool)>(self.changes.capacity())
                .map_err(|cause| VMError::Deferred(VmDeferral::AllocationDeferred(cause)))?,
        )
        .map_err(|cause| VMError::Deferred(VmDeferral::AllocationDeferred(cause)))?;
        Ok(plan.requested_bytes())
    }
    fn clear_last(&mut self) {
        match &mut self.last {
            Some(LastImage::C10(last)) => {
                erase(&mut last.pc);
                erase(&mut last.gpr);
                erase(&mut last.tags);
            }
            Some(LastImage::Baseline(last)) => {
                erase(&mut last.pc);
                erase(&mut last.gpr);
                erase(&mut last.tags);
            }
            None => {}
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
/// Push the changes of one admitted observation against `previous` (every register when absent)
/// and its row, padding the row's span to the batch's public bound.
fn record_changes(
    rows: &mut TraceRows<Row>,
    changes: &mut TraceRows<(usize, u64, bool)>,
    pending: Pending,
    pc: u64,
    previous: Option<(&[u64], &[bool])>,
    gpr: &[u64],
    tags: &[bool],
) {
    let changed = |index: usize| {
        previous.is_none_or(|(last_gpr, last_tags)| {
            last_gpr[index] != gpr[index] || last_tags[index] != tags[index]
        })
    };
    let count = (0..gpr.len()).filter(|&index| changed(index)).count();
    assert!(
        count <= pending.next,
        "public delta destination bound underestimated"
    );
    let start = changes.as_slice().len();
    for index in 0..gpr.len() {
        if changed(index) {
            changes.push_reserved((index, gpr[index], tags[index]));
        }
    }
    for _ in count..pending.next {
        changes.push_reserved((0, 0, false));
    }
    rows.push_reserved(Row { pc, start, count });
}
impl Drop for DeltaTraceLog {
    fn drop(&mut self) {
        self.clear_last();
    }
}
#[cfg(test)]
mod tests;
