//! One original-pool fixed row carried by active and idle runtime owners.

use super::PooledRuntime;
use iroha_allocation::AllocationBudget;
use ivm::execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan};

// Storage drops before its pool identity; ExecutionBuffer deallocates before refunding credit.
pub(super) struct IdleRuntimeBacking {
    storage: ExecutionBuffer<PooledRuntime>,
    original_pool: AllocationBudget,
}

impl IdleRuntimeBacking {
    pub(super) fn try_new(original_pool: &AllocationBudget) -> Result<Self, ivm::VMError> {
        original_pool.with_deferred_refund_notifications(|_| {
            let plan = ExecutionMemoryPlan::array::<PooledRuntime>(1)
                .map_err(ivm::VMError::AllocationDeferred)?;
            let mut lease = ExecutionMemoryLease::reserve(original_pool, plan)
                .map_err(ivm::VMError::AllocationDeferred)?;
            let storage = ExecutionBuffer::new(1, &mut lease).map_err(|_| {
                ivm::VMError::ExecutionDeferred(
                    ivm::error::ExecutionDeferral::AllocationUnavailable,
                )
            })?;
            Ok(Self {
                storage,
                original_pool: original_pool.clone(),
            })
        })
    }

    pub(super) fn belongs_to(&self, original_pool: &AllocationBudget) -> bool {
        self.original_pool.same_pool(original_pool)
    }

    pub(super) fn try_retain(&self) -> bool {
        self.storage.try_retain()
    }
}

pub(super) struct IdleRuntimeSlot {
    backing: Option<IdleRuntimeBacking>,
}

impl IdleRuntimeSlot {
    pub(super) const fn empty() -> Self {
        Self { backing: None }
    }

    pub(super) fn len(&self) -> usize {
        self.backing
            .as_ref()
            .map_or(0, |backing| backing.storage.as_slice().len())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn take(&mut self) -> Option<(PooledRuntime, IdleRuntimeBacking)> {
        let mut backing = self.backing.take()?;
        let runtime = backing.storage.pop()?;
        backing.storage.activate();
        Some((runtime, backing))
    }

    pub(super) fn place(&mut self, mut backing: IdleRuntimeBacking, runtime: PooledRuntime) {
        // All storage is constructor-owned capacity one. Refuse an invalid or occupied
        // internal transfer instead of asking push_reserved to grow or replace an owner.
        if !self.is_empty()
            || !backing.storage.as_slice().is_empty()
            || backing.storage.capacity() != 1
        {
            return;
        }
        backing.storage.push_reserved(runtime);
        self.backing = Some(backing);
    }
}

#[cfg(test)]
#[path = "runtime_slot/tests.rs"]
mod tests;
