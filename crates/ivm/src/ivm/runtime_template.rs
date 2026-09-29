//! Immutable runtime-template custody, including its private shared allocation.

use super::{Memory, PrivateMemoryRanges, Registers, TraceMode};
use crate::cache_memory::strong_owner::StrongOwner;

/// Immutable shared baseline used to reset a warmed VM to its post-load state.
///
/// Clones share one pristine memory image and its original reservations. The
/// private owner releases Arc backing before payloads and their charges; hosts
/// keep this handle directly without allocating a second sharing wrapper.
/// Reset copies only dirty memory chunks and preserves decoded program ownership.
pub struct RuntimeTemplate {
    inner: StrongOwner<RuntimeTemplateData>,
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
    // Release aggregate charges after every owned allocation above is destroyed.
    pub(super) cache_reservation: crate::cache_memory::MemoryReservation,
    // Image and leaf buffers split exact charges from this original admission.
    // The remaining credit covers all other private clone allocations and the
    // known private shared owner's padded Arc layout. Release after payloads.
    pub(super) _allocation_lease: Option<crate::execution_memory::ExecutionMemoryLease>,
}

impl RuntimeTemplate {
    pub(super) fn new(data: RuntimeTemplateData) -> Self {
        Self {
            inner: StrongOwner::new(data),
        }
    }

    pub(super) fn data(&self) -> &RuntimeTemplateData {
        &self.inner
    }

    /// Admit this immutable baseline to the shared cache retention budget.
    pub fn try_retain_cache_allocations(&self) -> bool {
        let data = self.data();
        data.cache_reservation.try_retain()
            && data.memory.try_retain()
            && data.registers.try_retain()
            && data.private_memory_bytes.try_retain()
    }
}

impl Clone for RuntimeTemplate {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
