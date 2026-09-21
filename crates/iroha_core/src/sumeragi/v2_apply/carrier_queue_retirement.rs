//! Borrow the immutable Apply service's original State/Queue pairing.

use crate::{
    queue::{Queue, QueueLaneRetirementObserver},
    state::State,
};

/// Original service identity, not a drain assertion or publication permit.
/// Currently constructed only by publication tests pending Apply integration.
pub(crate) struct OriginalCarrierQueue<'service> {
    state: &'service State,
    queue: &'service Queue,
}

impl<'service> OriginalCarrierQueue<'service> {
    #[cfg(test)]
    pub(super) fn new(state: &'service State, queue: &'service Queue) -> Self {
        Self { state, queue }
    }

    /// Reject a substituted State before probing the original Queue owner.
    pub(crate) fn belongs_to(&self, state: &State) -> bool {
        core::ptr::eq(self.state, state)
    }

    /// Acquire only the actual service Queue's transition fence, without waiting.
    /// The caller must authenticate `belongs_to` before this physical probe.
    pub(crate) fn try_observe(
        &self,
    ) -> Result<QueueLaneRetirementObserver<'service>, concread::release::ReleaseWait> {
        self.queue.try_lock_lane_retirement_observer()
    }

    /// A look-alike empty Queue cannot substitute its cut for this service owner.
    pub(crate) fn owns_cut(&self, cut: &crate::queue::QueueLaneRetirementCut<'_>) -> bool {
        cut.belongs_to(self.queue)
    }

    /// Bind real fixture owners without creating a shipping constructor.
    #[cfg(test)]
    pub(crate) fn for_test(state: &'service State, queue: &'service Queue) -> Self {
        Self::new(state, queue)
    }
}
