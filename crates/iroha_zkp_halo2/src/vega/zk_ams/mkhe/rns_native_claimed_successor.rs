//! Opaque move-only carrier for one exact-preflighted borrowed successor.
//!
//! A carrier can be minted only from the direct frame's opaque successor
//! claim. The generic parent remains owned, so every later proof stage can
//! retain the complete chronology without copying or reconstructing the
//! borrowed continuation.

use super::{
    rns_native_cross_field_rlwe_direct::{
        RnsNativeCrossFieldRlweAtomicVerifiedV2, RnsNativeCrossFieldRlweClaimedInventoryParentV1,
        RnsNativeCrossFieldRlweClaimedSuccessorSliceV1, RnsNativeCrossFieldRlweDirectErrorV1,
        verify_rns_native_cross_field_rlwe_claimed_with_alias_v2,
    },
    rns_native_existing_radix_commitment_view::{
        RnsNativeExistingRadixDirectAliasV1, RnsNativeExistingRadixValidationPermitV1,
    },
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
};

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
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeClaimedSuccessorV1<
        'proof,
        RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>,
    >
{
    pub(super) fn take_existing_radix_validation_permit_v1(
        &mut self,
    ) -> Option<RnsNativeExistingRadixValidationPermitV1> {
        self.parent.take_existing_radix_validation_permit_v1()
    }
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeClaimedSuccessorV1<
        'proof,
        RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>,
    >
{
    /// Purpose-specific consuming bridge from the sole direct-frame claim and
    /// the exact authenticated existing-radix alias into one atomic direct
    /// verification. Neither owner is returned or exposed as raw parts.
    pub(super) fn verify_claimed_direct_with_alias_v2(
        self,
        existing_radix: RnsNativeExistingRadixDirectAliasV1<'proof>,
    ) -> Result<
        RnsNativeCrossFieldRlweAtomicVerifiedV2<'source, 'proof, S>,
        RnsNativeCrossFieldRlweDirectErrorV1,
    > {
        verify_rns_native_cross_field_rlwe_claimed_with_alias_v2(
            self.parent,
            self.successor,
            existing_radix,
        )
    }
}

#[cfg(test)]
#[path = "rns_native_claimed_successor_tests.rs"]
mod tests;
