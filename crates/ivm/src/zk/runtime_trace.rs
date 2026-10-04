//! Immutable runtime observations with original-pool backing and shared custody.
//!
//! Captures are diagnostic records, not proofs.
//! A capture retains complete public backing capacities, while exposing only
//! initialized logical rows. Combining captures reserves a complete new owner
//! before copying; a checkpoint clone shares its original immutable storage.

use super::{
    delta_rows::{DeltaEntry, DeltaTraceLog, Row},
    trace_storage::{PcTraceLog, TraceRows, check_scope, overflow},
};
use crate::{
    VMError,
    cache_memory::MemoryReservation,
    error::ExecutionDeferral,
    execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan},
};
use iroha_allocation::{
    AllocationBudget, AllocationCharge,
    shared::{Reserved, Shared},
};

type Change = (usize, u64, bool);
type Charge = (Option<AllocationCharge>, MemoryReservation);
type Owner = Shared<Data, Charge>;

struct Data {
    pcs: TraceRows<u64>,
    rows: TraceRows<Row>,
    changes: TraceRows<Change>,
    original: Option<AllocationBudget>,
}

#[derive(Clone, Copy)]
struct Source<'a> {
    pcs: &'a TraceRows<u64>,
    rows: &'a TraceRows<Row>,
    changes: &'a TraceRows<Change>,
    original: Option<&'a AllocationBudget>,
}

impl Data {
    fn source(&self) -> Source<'_> {
        Source {
            pcs: &self.pcs,
            rows: &self.rows,
            changes: &self.changes,
            original: self.original.as_ref(),
        }
    }
}

#[derive(Clone, Copy)]
struct Geometry {
    pcs: usize,
    rows: usize,
    changes: usize,
}

impl Geometry {
    fn of(source: Source<'_>) -> Self {
        Self {
            pcs: source.pcs.capacity(),
            rows: source.rows.capacity(),
            changes: source.changes.capacity(),
        }
    }

    fn with_next(self, next: Source<'_>) -> Result<Self, VMError> {
        Ok(Self {
            pcs: self
                .pcs
                .checked_add(next.pcs.capacity())
                .ok_or_else(overflow)?,
            rows: self
                .rows
                .checked_add(next.rows.capacity())
                .ok_or_else(overflow)?,
            changes: self
                .changes
                .checked_add(next.changes.capacity())
                .ok_or_else(overflow)?,
        })
    }

    fn plan(self) -> Result<ExecutionMemoryPlan, VMError> {
        let mut plan = ExecutionMemoryPlan::default();
        plan.include(Owner::layout())
            .map_err(VMError::AllocationDeferred)?;
        for child in [
            ExecutionMemoryPlan::array::<u64>(self.pcs),
            ExecutionMemoryPlan::array::<Row>(self.rows),
            ExecutionMemoryPlan::array::<Change>(self.changes),
        ] {
            plan.include_child(child.map_err(VMError::AllocationDeferred)?)
                .map_err(VMError::AllocationDeferred)?;
        }
        Ok(plan)
    }
}

/// Immutable runtime PC/delta capture whose original charges follow its final clone.
///
/// Cloning allocates nothing and never copies private values. There is no raw
/// ownership extraction, mutable row access, or independent admission pool.
/// A capture may outlive its source VM and host checkpoint. Its initialized
/// backing is erased by the canonical trace owners before credit is refunded.
pub struct RuntimeTraceCapture(Owner);

impl Clone for RuntimeTraceCapture {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for RuntimeTraceCapture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeTraceCapture")
            .field("pc_rows", &self.pcs().len())
            .field("delta_rows", &self.delta_len())
            .finish_non_exhaustive()
    }
}

impl RuntimeTraceCapture {
    /// Copy a VM's canonical owners under their complete original admission.
    /// The caller cannot supply rows, capacities, or a replacement budget.
    pub(crate) fn try_capture(pcs: &PcTraceLog, delta: &DeltaTraceLog) -> Result<Self, VMError> {
        let (pcs, pc_original) = pcs.capture_storage();
        let (rows, changes, original) = delta.capture_storage();
        if !same_original(pc_original, original) {
            return Err(owner_unavailable());
        }
        Self::build(
            Source {
                pcs,
                rows,
                changes,
                original,
            },
            None,
        )
    }

    /// Create a new immutable capture in left-to-right observation order.
    ///
    /// Both inputs and any checkpoint clones remain unchanged on refusal or
    /// unwind. Exact original pool identity is required, including empty inputs.
    /// `None` standalone owners can combine only with other standalone owners.
    /// All public capacities, including spare slots, are reserved together
    /// before private rows are copied; logical change counts never size backing.
    ///
    /// # Errors
    /// Preserves the original pool refusal, or reports local owner/allocator
    /// unavailability. Combining never falls back to uncharged storage.
    pub fn try_combine(&self, following: &Self) -> Result<Self, VMError> {
        Self::build(self.0.source(), Some(following.0.source()))
    }

    fn build(first: Source<'_>, following: Option<Source<'_>>) -> Result<Self, VMError> {
        if following.is_some_and(|next| !same_original(first.original, next.original)) {
            return Err(owner_unavailable());
        }
        let geometry = match following {
            Some(next) => Geometry::of(first).with_next(next)?,
            None => Geometry::of(first),
        };
        let plan = geometry.plan()?;
        match first.original {
            Some(original) => original.with_deferred_refund_notifications(|scope| {
                check_scope(first.original, Some(scope))?;
                let lease = ExecutionMemoryLease::reserve(original, plan)
                    .map_err(VMError::AllocationDeferred)?;
                Self::copy_admitted(first, following, geometry, Some(lease))
            }),
            None => Self::copy_admitted(first, following, geometry, None),
        }
    }

    fn copy_admitted(
        first: Source<'_>,
        following: Option<Source<'_>>,
        geometry: Geometry,
        mut lease: Option<ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        let charge = lease
            .as_mut()
            .map(|lease| {
                lease
                    .split_allocation(Owner::layout())
                    .map_err(|_| unavailable())
            })
            .transpose()?;
        let charge = (charge, MemoryReservation::active(Owner::layout().size()));
        #[cfg(test)]
        if REFUSE_SHELL.replace(false) {
            return Err(unavailable());
        }
        let shell = Reserved::<Data, Charge>::try_new(charge).map_err(|(charge, _)| {
            drop(charge);
            unavailable()
        })?;
        // All three allocations precede the first private copy. The shell and
        // earlier empty backing retire under the same deferred refund scope if
        // any later allocation refuses. No source ownership has changed.
        let mut copied = Data {
            pcs: TraceRows::allocate(geometry.pcs, lease.as_mut())?,
            rows: TraceRows::allocate(geometry.rows, lease.as_mut())?,
            changes: TraceRows::allocate(geometry.changes, lease.as_mut())?,
            original: first.original.cloned(),
        };
        for source in [Some(first), following].into_iter().flatten() {
            let offset = copied.changes.as_slice().len();
            copied.changes.copy_from(source.changes.as_slice());
            for row in source.rows.as_slice() {
                copied.rows.push_reserved(Row {
                    pc: row.pc,
                    start: offset.checked_add(row.start).ok_or_else(overflow)?,
                    count: row.count,
                });
            }
            copied.pcs.copy_from(source.pcs.as_slice());
        }
        debug_assert!(
            lease
                .as_ref()
                .is_none_or(|lease| lease.remaining_bytes() == 0)
        );
        Ok(Self(shell.initialize(copied)))
    }

    /// Borrow initialized runtime PCs in original observation order.
    #[must_use]
    pub fn pcs(&self) -> &[u64] {
        self.0.pcs.as_slice()
    }

    /// Whether neither runtime PC nor delta observations are initialized.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pcs().is_empty() && self.delta_len() == 0
    }

    /// Number of initialized delta rows, independent of retained spare capacity.
    #[must_use]
    pub fn delta_len(&self) -> usize {
        self.0.rows.as_slice().len()
    }

    /// Borrow one logical delta; no spare private slots escape the owner.
    #[must_use]
    pub fn delta(&self, index: usize) -> Option<DeltaEntry<'_>> {
        self.0.rows.as_slice().get(index).map(|row| DeltaEntry {
            pc: row.pc,
            changes: &self.0.changes.as_slice()[row.start..row.start + row.count],
        })
    }

    /// Borrow logical deltas in original order without a temporary collection.
    pub fn deltas(
        &self,
    ) -> impl ExactSizeIterator<Item = DeltaEntry<'_>> + DoubleEndedIterator + std::iter::FusedIterator
    {
        (0..self.delta_len()).map(|index| self.delta(index).expect("captured delta descriptor"))
    }

    /// Check exact original funding identity, never equality of pool limits.
    #[must_use]
    pub fn belongs_to(&self, original: &AllocationBudget) -> bool {
        self.0
            .original
            .as_ref()
            .is_some_and(|owner| owner.same_pool(original))
    }
}

fn same_original(left: Option<&AllocationBudget>, right: Option<&AllocationBudget>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.same_pool(right),
        (None, None) => true,
        _ => false,
    }
}

fn owner_unavailable() -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::TraceOwnerUnavailable)
}

fn unavailable() -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
}

#[cfg(test)]
thread_local! { static REFUSE_SHELL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

#[cfg(test)]
mod tests;
