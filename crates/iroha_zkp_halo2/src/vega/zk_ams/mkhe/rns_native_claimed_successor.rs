//! Sealed compatibility carriers for the retired cross-field prototype.
//!
//! These private move-only types preserve the narrow seams still named by the
//! retained transcript and comparator stages. No production constructor exists,
//! so the retired verifier cannot grant proof, receipt, or release authority.

use super::{
    rns_native_cross_field_inventory::RnsNativeCrossFieldInventoryPrerequisiteV1,
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
    rns_native_transcript::ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1,
};

/// Exact maximum encoded successor retained by the comparator wire contract.
pub(super) const RNS_NATIVE_CROSS_FIELD_RLWE_DIRECT_SUCCESSOR_MAX_BYTES_V1: usize = 6_747_974;

/// Opaque root equality evidence with no production construction path.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the sealed equality obligation consumes this value exactly once"
)]
#[must_use = "root evidence must remain paired with its claimed-root obligation"]
pub(super) struct RnsNativeCrossFieldRlweVerifiedCoreRootV1([u8; 32]);

impl RnsNativeCrossFieldRlweVerifiedCoreRootV1 {
    /// Compare the sealed root with the transcript claim without exposing it.
    pub(super) fn matches_claimed_cross_field_root_v1(
        self,
        claimed_root: [u8; 32],
        qpcs_bound_transcript_state: [u8; 32],
    ) -> bool {
        let recomputed_root = self.0;
        recomputed_root != [0; 32]
            && recomputed_root != qpcs_bound_transcript_state
            && recomputed_root == claimed_root
    }

    #[cfg(test)]
    pub(super) fn test_fixture_v1(root: [u8; 32]) -> Option<Self> {
        (root != [0; 32]).then_some(Self(root))
    }
}

/// Exact unconstructible parent shape retained by later comparator stages.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the private facade preserves the move-only consumer seam"
)]
pub(super) struct RnsNativeCrossFieldRlweClaimedInventoryParentV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    pre_global_lookup_capability: ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1,
    inventory: RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S>,
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeCrossFieldRlweClaimedInventoryParentV1<'source, 'proof, S>
{
    /// Borrow the opaque pre-global transcript snapshot.
    pub(super) const fn pre_global_lookup_capability_v1(
        &self,
    ) -> &ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1 {
        &self.pre_global_lookup_capability
    }

    /// Borrow the exact retained inventory.
    pub(super) const fn inventory(
        &self,
    ) -> &RnsNativeCrossFieldInventoryPrerequisiteV1<'source, 'proof, S> {
        &self.inventory
    }
}

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
    /// Borrow the retained parent without splitting ownership.
    pub(super) const fn parent(&self) -> &Parent {
        &self.parent
    }

    /// Borrow the opaque successor retained by this sealed carrier.
    pub(super) const fn successor(&self) -> &'proof [u8] {
        self.successor
    }
}
