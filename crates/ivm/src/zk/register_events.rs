//! Original-pool register event rows admitted before an enclosing semantic batch.
//!
//! Live rows contain their canonical eight-sibling paths inline. This owner has
//! no infallible growth or detached payload vector. Funded growth and reset require
//! the original refund scope; its caller must acquire and release logger mutexes
//! and TLS borrows entirely inside that scope. Standalone storage remains an
//! explicit local owner and never creates a replacement State pool.

use super::RegEvent;
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

/// Fixed event rows retain their original allocation until their final owner drops.
///
/// There is deliberately no `Default`, infallible clone, or caller-owned Vec
/// adoption. The shared logger selects its pool exactly once at construction.
pub struct RegLog {
    rows: Rows,
    original: Option<AllocationBudget>,
}

enum Rows {
    Funded(Option<ExecutionBuffer<RegEvent>>),
    Standalone(OwnedVec<RegEvent>),
}
impl Rows {
    fn empty(funded: bool) -> Self {
        if funded {
            Self::Funded(None)
        } else {
            Self::Standalone(OwnedVec::default())
        }
    }
    fn as_slice(&self) -> &[RegEvent] {
        match self {
            Self::Funded(rows) => rows.as_ref().map_or(&[], ExecutionBuffer::as_slice),
            Self::Standalone(rows) => rows,
        }
    }
    fn as_mut_slice(&mut self) -> &mut [RegEvent] {
        match self {
            Self::Funded(rows) => rows.as_mut().map_or(&mut [], ExecutionBuffer::as_mut_slice),
            Self::Standalone(rows) => rows,
        }
    }
    fn capacity(&self) -> usize {
        match self {
            Self::Funded(rows) => rows.as_ref().map_or(0, ExecutionBuffer::capacity),
            Self::Standalone(rows) => rows.capacity(),
        }
    }
    fn push_reserved(&mut self, event: RegEvent) {
        match self {
            Self::Funded(rows) => rows
                .as_mut()
                .expect("admitted register rows")
                .push_reserved(event),
            Self::Standalone(rows) => rows.insert_reserved(rows.len(), event),
        }
    }
    fn scrub(&mut self) {
        for event in self.as_mut_slice() {
            let (index, value, tag, path, root) = match event {
                RegEvent::Read {
                    index,
                    value,
                    tag,
                    path,
                    root,
                }
                | RegEvent::Write {
                    index,
                    value,
                    tag,
                    path,
                    root,
                } => (index, value, tag, path, root),
            };
            erase(index);
            erase(value);
            erase(tag);
            for sibling in path {
                erase(sibling);
            }
            // Erase only initialized fields, never enum padding/discriminants.
            erase(&mut **root);
        }
    }
    fn clear(&mut self) {
        self.scrub();
        match self {
            Self::Funded(rows) => {
                if let Some(rows) = rows {
                    rows.truncate(0);
                }
            }
            Self::Standalone(rows) => rows.clear(),
        }
    }
}
impl Drop for Rows {
    fn drop(&mut self) {
        self.scrub();
    }
}

impl RegLog {
    #[cfg(test)]
    pub(crate) fn matches_budget(&self, budget: Option<&AllocationBudget>) -> bool {
        match (&self.original, budget) {
            (Some(original), Some(budget)) => original.same_pool(budget),
            (None, None) => true,
            _ => false,
        }
    }

    /// Bind an empty row owner without allocating event backing.
    pub(crate) fn new(original: Option<&AllocationBudget>) -> Self {
        Self {
            rows: Rows::empty(original.is_some()),
            original: original.cloned(),
        }
    }

    fn check_scope(&self, scope: Option<&AllocationScope<'_>>) -> Result<(), VMError> {
        match (&self.original, scope) {
            (Some(original), Some(scope)) if scope.belongs_to(original) => Ok(()),
            (None, None) => Ok(()),
            _ => Err(VMError::HostUnavailable),
        }
    }

    /// Admit the whole additional batch before any register, gas or memory effect.
    ///
    /// The original caller enters its refund scope before taking the logger lock.
    /// Old and new exact capacities remain charged throughout allocation/copy;
    /// publication precedes scrubbing/retirement. No borrower can refund a row.
    pub(crate) fn prepare_events(
        &mut self,
        additional: usize,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<(), VMError> {
        self.check_scope(scope)?;
        let needed = self
            .as_slice()
            .len()
            .checked_add(additional)
            .ok_or_else(overflow)?;
        if needed <= self.capacity() {
            return Ok(());
        }
        let capacity = self
            .capacity()
            .checked_mul(2)
            .ok_or_else(overflow)?
            .max(needed)
            .max(4);
        let plan = ExecutionMemoryPlan::array::<RegEvent>(capacity)
            .map_err(VMError::AllocationDeferred)?;
        let mut replacement = if let Some(original) = &self.original {
            let mut lease = ExecutionMemoryLease::reserve(original, plan)
                .map_err(VMError::AllocationDeferred)?;
            #[cfg(test)]
            if REFUSE_NEXT_ALLOCATION.replace(false) {
                return Err(unavailable());
            }
            Rows::Funded(Some(
                ExecutionBuffer::new(capacity, &mut lease).map_err(prepaid_error)?,
            ))
        } else {
            let Rows::Standalone(rows) = &self.rows else {
                unreachable!("original row owner")
            };
            Rows::Standalone(rows.try_copy_capacity(capacity).map_err(local_error)?)
        };
        for event in self.as_slice() {
            replacement.push_reserved(event.clone());
            #[cfg(test)]
            if PANIC_AFTER_COPY.replace(false) {
                panic!("register row copy unwind fixture");
            }
        }
        let retired = std::mem::replace(&mut self.rows, replacement);
        drop(retired);
        Ok(())
    }

    /// Append only against the enclosing invocation's already admitted batch quota.
    pub(crate) fn record_reserved(&mut self, event: RegEvent) {
        self.rows.push_reserved(event);
    }

    /// Borrow initialized events without detaching their original backing.
    pub fn as_slice(&self) -> &[RegEvent] {
        self.rows.as_slice()
    }

    /// Exact physical row capacity, including spare admitted slots.
    pub(crate) fn capacity(&self) -> usize {
        self.rows.capacity()
    }

    /// Exact event backing, independent of the shared logger's control shell.
    #[cfg(test)]
    pub(crate) fn allocated_bytes(&self) -> Result<usize, VMError> {
        ExecutionMemoryPlan::array::<RegEvent>(self.capacity())
            .map(|plan| plan.requested_bytes())
            .map_err(VMError::AllocationDeferred)
    }

    /// Erase every initialized field while keeping the same prepaid backing.
    pub(crate) fn scrub(&mut self) {
        self.rows.clear();
    }

    /// Retire storage under its original refund scope without rebinding its pool.
    #[cfg(test)]
    pub(crate) fn reset(&mut self, scope: Option<&AllocationScope<'_>>) -> Result<(), VMError> {
        self.check_scope(scope)?;
        let retired = std::mem::replace(&mut self.rows, Rows::empty(self.original.is_some()));
        drop(retired);
        Ok(())
    }

    /// Independent diagnostic copy with its own charge from the original pool.
    #[cfg(test)]
    pub(crate) fn try_clone_allocation(
        &self,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<Self, VMError> {
        self.check_scope(scope)?;
        let mut copy = Self::new(self.original.as_ref());
        copy.prepare_events(self.as_slice().len(), scope)?;
        for event in self.as_slice() {
            copy.record_reserved(event.clone());
        }
        Ok(copy)
    }
}

fn overflow() -> VMError {
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
#[cfg(test)]
thread_local! {
    static REFUSE_NEXT_ALLOCATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static PANIC_AFTER_COPY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
mod tests;
