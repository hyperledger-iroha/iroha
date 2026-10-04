//! Exact original-funded fixed trace backing and public PC rows.
//!
//! An absent original budget retains the existing fallible standalone owner;
//! neither construction nor reset creates a substitute State allocation pool.

use crate::{
    VMError,
    cache_memory::{OwnedVec, OwnedVecGrowthError},
    error::ExecutionDeferral,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, AllocationScope, ChargedBufferError, PrepaidBufferError,
};
use iroha_crypto::zeroize_value_for_confidential_discard as erase;

pub(super) trait TraceCell: Clone {
    fn scrub(&mut self);
}
impl TraceCell for u64 {
    fn scrub(&mut self) {
        erase(self);
    }
}

pub(super) enum TraceRows<T: TraceCell> {
    Funded(Option<ExecutionBuffer<T>>),
    Local(OwnedVec<T>),
}
impl<T: TraceCell> TraceRows<T> {
    pub(super) fn empty(funded: bool) -> Self {
        if funded {
            Self::Funded(None)
        } else {
            Self::Local(OwnedVec::default())
        }
    }
    pub(super) fn allocate(
        capacity: usize,
        lease: Option<&mut ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        #[cfg(test)]
        if REFUSE_ALLOCATION.with(|count| {
            let next = count.get().saturating_sub(1);
            let refuse = count.get() == 1;
            count.set(next);
            refuse
        }) {
            return Err(unavailable());
        }
        match lease {
            Some(lease) => ExecutionBuffer::new(capacity, lease)
                .map(|rows| Self::Funded(Some(rows)))
                .map_err(prepaid_error),
            None => OwnedVec::<T>::default()
                .try_copy_capacity(capacity)
                .map(Self::Local)
                .map_err(local_error),
        }
    }
    pub(super) fn as_slice(&self) -> &[T] {
        match self {
            Self::Funded(rows) => rows.as_ref().map_or(&[], ExecutionBuffer::as_slice),
            Self::Local(rows) => rows,
        }
    }
    pub(super) fn as_mut_slice(&mut self) -> &mut [T] {
        match self {
            Self::Funded(rows) => rows.as_mut().map_or(&mut [], ExecutionBuffer::as_mut_slice),
            Self::Local(rows) => rows,
        }
    }
    pub(super) fn capacity(&self) -> usize {
        match self {
            Self::Funded(rows) => rows.as_ref().map_or(0, ExecutionBuffer::capacity),
            Self::Local(rows) => rows.capacity(),
        }
    }
    pub(super) fn push_reserved(&mut self, row: T) {
        match self {
            Self::Funded(rows) => rows
                .as_mut()
                .expect("prepaid trace backing")
                .push_reserved(row),
            Self::Local(rows) => rows.insert_reserved(rows.len(), row),
        }
    }
    pub(super) fn copy_from(&mut self, rows: &[T]) {
        for row in rows {
            self.push_reserved(row.clone());
            #[cfg(test)]
            if PANIC_AFTER_COPY.replace(false) {
                panic!("private trace copy unwind fixture");
            }
        }
    }
    fn scrub(&mut self) {
        for row in self.as_mut_slice() {
            row.scrub();
        }
    }
    pub(super) fn clear(&mut self) {
        self.scrub();
        match self {
            Self::Funded(rows) => {
                if let Some(rows) = rows {
                    rows.truncate(0);
                }
            }
            Self::Local(rows) => rows.clear(),
        }
    }
}
impl<T: TraceCell> Drop for TraceRows<T> {
    fn drop(&mut self) {
        self.scrub();
    }
}

pub(super) fn check_scope(
    original: Option<&AllocationBudget>,
    scope: Option<&AllocationScope<'_>>,
) -> Result<(), VMError> {
    match (original, scope) {
        (Some(original), Some(scope)) if scope.belongs_to(original) => Ok(()),
        (None, None) => Ok(()),
        _ => Err(VMError::ExecutionDeferred(
            crate::error::ExecutionDeferral::TraceOwnerUnavailable,
        )),
    }
}
pub(super) fn next_capacity(capacity: usize, needed: usize) -> Result<usize, VMError> {
    if needed <= capacity {
        return Ok(capacity);
    }
    Ok(capacity
        .checked_mul(2)
        .ok_or_else(overflow)?
        .max(needed)
        .max(4))
}
pub(super) fn overflow() -> VMError {
    VMError::AllocationDeferred(AllocationRefusal::DemandOverflow)
}
fn unavailable() -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
}
fn local_error(error: OwnedVecGrowthError) -> VMError {
    match error {
        OwnedVecGrowthError::CapacityOverflow => overflow(),
        OwnedVecGrowthError::AllocationUnavailable => unavailable(),
    }
}
fn prepaid_error(error: PrepaidBufferError) -> VMError {
    match error {
        PrepaidBufferError::Allocation(ChargedBufferError::Admission(refusal)) => {
            VMError::AllocationDeferred(refusal)
        }
        PrepaidBufferError::Allocation(ChargedBufferError::Allocator { .. })
        | PrepaidBufferError::Reservation(_) => unavailable(),
    }
}

/// Original-pool public PC observations with only before-effects growth.
pub struct PcTraceLog {
    rows: TraceRows<u64>,
    original: Option<AllocationBudget>,
}
impl PcTraceLog {
    /// Bind the existing pool without allocating backing.
    pub fn new(original: Option<&AllocationBudget>) -> Self {
        Self {
            rows: TraceRows::empty(original.is_some()),
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
    /// Admit public observation count before its enclosing effects.
    pub(crate) fn prepare(
        &mut self,
        additional: usize,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<(), VMError> {
        self.validate_scope(scope)?;
        let needed = self
            .as_slice()
            .len()
            .checked_add(additional)
            .ok_or_else(overflow)?;
        let capacity = next_capacity(self.rows.capacity(), needed)?;
        if capacity == self.rows.capacity() {
            return Ok(());
        }
        let plan =
            ExecutionMemoryPlan::array::<u64>(capacity).map_err(VMError::AllocationDeferred)?;
        let mut lease = self
            .original
            .as_ref()
            .map(|original| ExecutionMemoryLease::reserve(original, plan))
            .transpose()
            .map_err(VMError::AllocationDeferred)?;
        let mut replacement = TraceRows::allocate(capacity, lease.as_mut())?;
        replacement.copy_from(self.as_slice());
        let retired = std::mem::replace(&mut self.rows, replacement);
        drop(retired);
        Ok(())
    }
    /// Publish one observation in already admitted storage.
    pub(crate) fn record_reserved(&mut self, pc: u64) {
        self.rows.push_reserved(pc);
    }
    /// Internal immutable capture input with original identity and public capacity.
    pub(super) fn capture_storage(&self) -> (&TraceRows<u64>, Option<&AllocationBudget>) {
        (&self.rows, self.original.as_ref())
    }
    /// Borrow original initialized PCs.
    pub fn as_slice(&self) -> &[u64] {
        self.rows.as_slice()
    }
    /// Exact charged capacity, including unused admitted slots.
    pub(crate) fn allocated_bytes(&self) -> Result<usize, VMError> {
        ExecutionMemoryPlan::array::<u64>(self.rows.capacity())
            .map(|plan| plan.requested_bytes())
            .map_err(VMError::AllocationDeferred)
    }
    /// Erase observations while retaining the same backing.
    pub(crate) fn clear(&mut self) {
        self.rows.clear();
    }
    /// Retire backing without rebinding the original owner.
    pub(crate) fn reset(&mut self, scope: Option<&AllocationScope<'_>>) -> Result<(), VMError> {
        self.validate_scope(scope)?;
        let retired = std::mem::replace(&mut self.rows, TraceRows::empty(self.original.is_some()));
        drop(retired);
        Ok(())
    }
    /// Independently admit an exact diagnostic copy from the original pool.
    #[cfg(test)]
    pub(crate) fn try_clone_allocation(
        &self,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<Self, VMError> {
        self.validate_scope(scope)?;
        let mut copy = Self::new(self.original.as_ref());
        copy.prepare(self.rows.capacity(), scope)?;
        copy.rows.copy_from(self.as_slice());
        Ok(copy)
    }
}
#[cfg(test)]
thread_local! {
    pub(super) static REFUSE_ALLOCATION: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PANIC_AFTER_COPY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
mod tests;
