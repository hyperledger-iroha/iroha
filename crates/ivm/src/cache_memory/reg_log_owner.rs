//! Original allocation custody for the shared register-log control block.
//!
//! The exact shared shell and fixed event-row backing keep independent charges
//! from the same original pool. The final shared borrower retains both owners.
//! Existing standalone construction without a State allocation budget remains
//! explicitly untracked by that pool; it never manufactures a replacement pool.

use super::{MemoryBudget, MemoryReservation, global_budget};
use crate::{VMError, error::ExecutionDeferral, zk::RegLog};
use iroha_allocation::{AllocationBudget, AllocationCharge, shared::Shared};
use parking_lot::{Mutex, MutexGuard};
use std::alloc::Layout;

struct LogData {
    log: Mutex<RegLog>,
    retention: MemoryReservation,
    original_budget: Option<AllocationBudget>,
}

impl Drop for LogData {
    fn drop(&mut self) {
        // Final shared release has exclusive custody. Scrub private values
        // before dropping fixed row backing and its original charge, then the
        // shell retention reservation.
        self.log.get_mut().scrub();
    }
}

type Allocation = Shared<LogData, Option<AllocationCharge>>;

/// A nonempty original logger owner. An absent VM logger uses `Option` rather
/// than allocating an empty replacement while severing an invalid host alias.
/// No weak or raw reference can detach the physical shell from its charges.
pub(crate) struct SharedRegLog {
    owner: Allocation,
    cache_held: bool,
}

impl SharedRegLog {
    /// Exact physical control, mutex, inline log, retention and custody layout.
    pub(crate) fn allocation_layout() -> Layout {
        Allocation::layout()
    }

    /// Admit an empty logger before any caller state transition. Funded VMs
    /// supply their original State budget. `None` preserves the existing
    /// untracked standalone-construction boundary, not a funded fallback.
    pub(crate) fn try_new(budget: Option<&AllocationBudget>) -> Result<Self, VMError> {
        Self::try_with_value(RegLog::new(budget), budget)
    }

    /// Retain an independently funded snapshot's original rows and pool.
    #[cfg(test)]
    pub(crate) fn try_from_owned_rows(
        log: RegLog,
        budget: Option<&AllocationBudget>,
    ) -> Result<Self, VMError> {
        if !log.matches_budget(budget) {
            return Err(VMError::HostUnavailable);
        }
        Self::try_with_value(log, budget)
    }

    fn try_with_value(log: RegLog, budget: Option<&AllocationBudget>) -> Result<Self, VMError> {
        Self::try_with_budgets(log, budget, global_budget())
    }

    fn try_with_budgets(
        log: RegLog,
        budget: Option<&AllocationBudget>,
        retention: &MemoryBudget,
    ) -> Result<Self, VMError> {
        let layout = Self::allocation_layout();
        let charge = budget
            .map(|budget| {
                let mut reservation = budget
                    .try_reserve(layout)
                    .map_err(VMError::AllocationDeferred)?;
                // The same exact layout was admitted above; this does not make
                // a second admission and is unaffected by later pool shrink.
                reservation.try_split(layout).map_err(|_| unavailable())
            })
            .transpose()?;
        let data = LogData {
            log: Mutex::new(log),
            retention: retention.reserve(layout.size()),
            original_budget: budget.cloned(),
        };
        #[cfg(test)]
        if super::REFUSE_NEXT_SHARED_ALLOCATION.with(|refuse| refuse.replace(false)) {
            drop(data);
            drop(charge);
            return Err(unavailable());
        }
        let owner = Allocation::try_new(data, charge).map_err(|(data, charge, _)| {
            drop(data);
            drop(charge);
            unavailable()
        })?;
        owner.retention.register_shared_initial();
        Ok(Self {
            owner,
            cache_held: false,
        })
    }

    /// Borrow the log while retaining its complete original shell custody.
    pub(crate) fn lock(&self) -> MutexGuard<'_, RegLog> {
        self.owner.log.lock()
    }

    /// Reserve the complete outer batch before any effect. Original refund
    /// callbacks run only after the physical logger guard has left the scope.
    pub(crate) fn prepare_events(&self, rows: usize) -> Result<(), VMError> {
        match &self.owner.original_budget {
            Some(original) => original.with_deferred_refund_notifications(|scope| {
                self.owner.log.lock().prepare_events(rows, Some(scope))
            }),
            None => self.owner.log.lock().prepare_events(rows, None),
        }
    }

    /// Append one event already owned by the active TLS batch; never allocate.
    pub(crate) fn record_reserved(&self, event: crate::zk::RegEvent) {
        self.owner.log.lock().record_reserved(event);
    }

    /// Original identity, never equality of pool limits or logger contents.
    pub(crate) fn ptr_eq(left: &Self, right: &Self) -> bool {
        Allocation::ptr_eq(&left.owner, &right.owner)
    }

    /// Check the exact original pool, including through final borrowed handles.
    pub(crate) fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.owner
            .original_budget
            .as_ref()
            .is_some_and(|original| original.same_pool(budget))
    }

    /// Admit only an empty log with no retained trace backing. Funded event rows
    /// remain active-only; cache retention currently measures this shell alone.
    pub(crate) fn try_retain(&mut self) -> bool {
        if self.lock().capacity() != 0 || !self.owner.retention.try_retain() {
            return false;
        }
        if !self.cache_held {
            self.owner.retention.promote_shared_cache_handle();
            self.cache_held = true;
        }
        true
    }

    /// The same shell becomes an active borrower, preserving retained credit
    /// until final release even when the VM is the last cache reference.
    pub(crate) fn activate(&mut self) {
        if self.cache_held {
            let reservation = &self.owner.retention;
            let mut stats = reservation.budget.0.lock().expect("memory accounting lock");
            assert!(
                reservation
                    .cache_handles
                    .fetch_sub(1, super::Ordering::Relaxed)
                    > 0
            );
            self.cache_held = false;
            reservation.refresh_retained_class(&mut stats);
        }
    }
}

impl Clone for SharedRegLog {
    fn clone(&self) -> Self {
        self.owner.retention.add_shared_handle(false);
        Self {
            owner: self.owner.clone(),
            cache_held: false,
        }
    }
}

impl Drop for SharedRegLog {
    fn drop(&mut self) {
        self.owner.retention.remove_shared_handle(self.cache_held);
        // Shared frees its exact control allocation, then the log payload and
        // aggregate reservation, then its original execution charge. Callers
        // must retire displaced owners outside enclosing locks/TLS borrows.
    }
}

fn unavailable() -> VMError {
    VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
}

#[cfg(test)]
mod tests;
