//! Opaque move-only carrier for one exact-preflighted borrowed successor.
//!
//! A carrier can be minted only from the direct frame's opaque successor
//! claim. The generic parent remains owned, so every later proof stage can
//! retain the complete chronology without copying or reconstructing the
//! borrowed continuation.

use super::rns_native_cross_field_rlwe_direct::RnsNativeCrossFieldRlweClaimedSuccessorSliceV1;

/// Move-only claimed successor paired with its exact owning predecessor.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the predecessor and borrowed successor must advance exactly once"
)]
#[must_use = "a claimed successor must remain paired with its exact predecessor"]
pub(super) struct RnsNativeClaimedSuccessorV1<'proof, Parent> {
    parent: Parent,
    successor: &'proof [u8],
}

impl<'proof, Parent> RnsNativeClaimedSuccessorV1<'proof, Parent> {
    /// Mint a carrier only from the opaque claim produced by the exact-decoded
    /// direct frame. There is deliberately no raw-slice constructor.
    pub(super) fn from_direct_claim_v1(
        parent: Parent,
        claim: RnsNativeCrossFieldRlweClaimedSuccessorSliceV1<'proof>,
    ) -> Self {
        Self {
            parent,
            successor: claim.into_borrowed_successor_v1(),
        }
    }

    /// Borrow the retained parent without splitting ownership.
    pub(super) const fn parent(&self) -> &Parent {
        &self.parent
    }

    /// Borrow the exact successor claimed by the preflighted direct frame.
    pub(super) const fn successor(&self) -> &'proof [u8] {
        self.successor
    }

    /// Consume the carrier and recover its exact owning predecessor together
    /// with the sole successor borrow minted by the direct-frame preflight.
    /// Neither component can be recovered without consuming this carrier.
    pub(super) fn into_parts_v1(self) -> (Parent, &'proof [u8]) {
        (self.parent, self.successor)
    }
}

#[cfg(test)]
#[path = "rns_native_claimed_successor_tests.rs"]
mod tests;
