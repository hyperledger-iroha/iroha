//! Immutable runtime-template custody, including its private shared allocation.

use super::{Memory, PrivateMemoryRanges, Registers, TraceMode};
use crate::cache_memory::strong_owner::StrongOwner;
use std::sync::Arc;

/// Immutable shared baseline used to reset a warmed VM to its post-load state.
///
/// Clones share one pristine memory image and its original reservations. The
/// data Arc destroys inline private values in place. Its paired charge owner
/// stays alive until the data Arc and its own backing have been deallocated.
/// Reset copies only dirty memory chunks and preserves decoded program ownership.
pub struct RuntimeTemplate {
    // Field order is a custody invariant: every handle drops data first, so
    // whichever thread destroys the final data Arc still owns backing credit.
    data: Arc<RuntimeTemplateData>,
    backing: StrongOwner<RuntimeTemplateBacking>,
}

pub(super) struct RuntimeTemplateData {
    pub(super) memory: Memory,
    pub(super) registers: Registers,
    pub(super) private_memory_bytes: PrivateMemoryRanges,
    pub(super) code_hash: [u8; 32],
    pub(super) pc: u64,
    pub(super) gas_limit: u64,
    pub(super) max_cycles: u64,
    pub(super) trace_mode: TraceMode,
    pub(super) zk_mode: bool,
    pub(super) zk_trace_enabled: bool,
    pub(super) entrypoint_pc: Option<u64>,
    pub(super) input_bump_next: u64,
}

/// Holds no plaintext and outlives both shared allocations and private payloads.
pub(super) struct RuntimeTemplateBacking {
    // Release aggregate charges after the data Arc and all its payloads are destroyed.
    pub(super) cache_reservation: crate::cache_memory::MemoryReservation,
    // Image and leaf buffers split exact charges from this original admission.
    // The remaining credit covers all other private clone allocations and the
    // exact padded data and backing Arc layouts. Release after both allocations.
    pub(super) _allocation_lease: Option<crate::execution_memory::ExecutionMemoryLease>,
}

impl RuntimeTemplate {
    pub(super) fn owner_allocation_bytes() -> Result<usize, crate::VMError> {
        let overflow = || {
            crate::VMError::AllocationDeferred(mv::allocation::AllocationRefusal::DemandOverflow)
        };
        let data = norito::core::owned_arc_allocation_bytes::<RuntimeTemplateData>()
            .map_err(|_| overflow())?;
        let backing = norito::core::owned_arc_allocation_bytes::<RuntimeTemplateBacking>()
            .map_err(|_| overflow())?;
        data.checked_add(backing).ok_or_else(overflow)
    }

    pub(super) fn new(data: RuntimeTemplateData, backing: RuntimeTemplateBacking) -> Self {
        Self {
            data: Arc::new(data),
            backing: StrongOwner::new(backing),
        }
    }

    pub(super) fn data(&self) -> &RuntimeTemplateData {
        &self.data
    }

    /// Admit this immutable baseline to the shared cache retention budget.
    pub fn try_retain_cache_allocations(&self) -> bool {
        let data = self.data();
        self.backing.cache_reservation.try_retain()
            && data.memory.try_retain()
            && data.registers.try_retain()
            && data.private_memory_bytes.try_retain()
    }
}

impl Clone for RuntimeTemplate {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
            backing: self.backing.clone(),
        }
    }
}

#[cfg(test)]
mod private_disposal_tests;
