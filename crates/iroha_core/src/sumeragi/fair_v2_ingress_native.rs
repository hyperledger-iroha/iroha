//! Process-lived Native physical custody across global-height ingress rollover.
//!
//! Native uses the same bounded fair queue and authenticated-source pool. Its
//! separate source identity has no global-roster or global-lifecycle authority.

use super::FairV2IngressState;

impl FairV2IngressState {
    /// Whether a global-height physical owner still prevents successor binding.
    pub(super) fn has_global_ingress(&self) -> bool {
        self.lanes.iter().any(|(source, lane)| {
            !source.is_native()
                && (!lane.entries.is_empty() || !lane.pending_wire.is_empty() || lane.bytes != 0)
        }) || self
            .pending_wire_owners
            .values()
            .any(|source| !source.is_native())
            || self.ready.iter().any(|source| !source.is_native())
    }

    /// Drop only the retiring global queue, preserving exact Native allocations,
    /// occurrence IDs, coalescing indexes and source rotation. Called under the
    /// original service/state fences after durable global carrier parking.
    pub(super) fn retain_native_ingress(&mut self) {
        self.lanes.retain(|source, _| source.is_native());
        self.pending_wire_owners
            .retain(|_, source| source.is_native());
        self.ready.retain(|source| source.is_native());
        self.len = self.lanes.values().map(|lane| lane.entries.len()).sum();
        self.bytes = self.lanes.values().map(|lane| lane.bytes).sum();
        self.nonempty_since = self
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter().map(|entry| entry.enqueued_at))
            .min();
        if self.len == 0 {
            self.last_service_attempt_at = None;
        }
    }
}
