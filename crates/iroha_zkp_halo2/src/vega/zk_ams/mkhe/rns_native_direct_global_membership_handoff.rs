//! Private verifier-side direct-to-global-membership handoff.
//!
//! The global-membership prerequisite recursively owns the exact claimed
//! successor minted by the direct frame's preflight. This module consumes that
//! chain back to the claimed direct parent, verifies the retained four-core
//! direct frame with the private authoritative-source interface, requires
//! pointer-and-length identity for the sole successor borrow, derives the
//! acyclic membership clean-core root, and consumes the cross, global, and
//! zero claimed-root equality obligations. Only then does it mint one move-only
//! combined predecessor for the source/packing same-opening child.
//!
//! The source/packing successor is the existing membership residual. This
//! handoff adds no wire bytes and does not charge the 32,271-byte direct frame
//! a second time. Its five-field safe core contains only successor-independent
//! candidate axes and the domain-separated digest of the private verified
//! direct-core root. Current inventory, codec, successor, and chain-envelope
//! bindings are admitted only through the source/packing child's post-equation
//! outer bundle.
//!
//! No production numeric source, replay owner, mask owner, staged adapter,
//! composite capability, readiness, receipt, or release authority is made
//! available by this private handoff.

use core::fmt;

use super::{
    rns_native_cross_field_rlwe_direct::{
        RnsNativeCrossFieldRlweAllRootsVerifiedV2, RnsNativeCrossFieldRlweDirectErrorV1,
        RnsNativeCrossFieldRlweSafeCoreProjectionV1,
    },
    rns_native_global_lookup_z_commitment_view::rns_native_global_inverse_product_sumcheck::{
        RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1,
        RnsNativeGlobalMembershipPrerequisiteV1,
        derive_rns_native_verified_global_lookup_core_root_v2,
    },
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
    rns_native_source_packing_same_opening::{
        RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1,
        RnsNativeSourcePackingCombinedOuterBindingsV1, RnsNativeSourcePackingSafeCoreV1,
    },
};

const DIGEST_BYTES_V1: usize = 32;

/// This handoff is ownership-only: it inserts no frame between membership and
/// the source/packing same-opening child.
const DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_OWNED_WIRE_BYTES_V1: usize = 0;
const DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_SUCCESSOR_MAX_BYTES_V1: usize =
    RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1;

const VERIFIER_SIDE_DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_IMPLEMENTED_V1: bool = true;
const PRODUCTION_AUTHORITATIVE_NUMERIC_SOURCE_AVAILABLE_V1: bool = false;
const PRODUCTION_AUTHENTICATED_REPLAY_OWNER_AVAILABLE_V1: bool = false;
const PRODUCTION_DERIVED_MASK_OWNER_AVAILABLE_V1: bool = false;
const PRODUCTION_DIRECT_STAGED_ADAPTER_AVAILABLE_V1: bool = false;
const COMPOSITE_ACCEPTANCE_AVAILABLE_V1: bool = false;
const READINESS_AVAILABLE_V1: bool = false;
const RECEIPT_AVAILABLE_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_OWNED_WIRE_BYTES_V1 == 0);
    assert!(DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_SUCCESSOR_MAX_BYTES_V1 == 108_464);
    assert!(VERIFIER_SIDE_DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_IMPLEMENTED_V1);
    assert!(!PRODUCTION_AUTHORITATIVE_NUMERIC_SOURCE_AVAILABLE_V1);
    assert!(!PRODUCTION_AUTHENTICATED_REPLAY_OWNER_AVAILABLE_V1);
    assert!(!PRODUCTION_DERIVED_MASK_OWNER_AVAILABLE_V1);
    assert!(!PRODUCTION_DIRECT_STAGED_ADAPTER_AVAILABLE_V1);
    assert!(!COMPOSITE_ACCEPTANCE_AVAILABLE_V1);
    assert!(!READINESS_AVAILABLE_V1);
    assert!(!RECEIPT_AVAILABLE_V1);
    assert!(!RELEASE_READY_V1);
};

/// Failure while recovering and verifying the exact direct predecessor owned
/// by a completed global-membership chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeDirectGlobalMembershipHandoffErrorV1 {
    Direct(RnsNativeCrossFieldRlweDirectErrorV1),
    GlobalLookupRoot,
    ZeroPaddingRoot,
}

impl fmt::Display for RnsNativeDirectGlobalMembershipHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeDirectGlobalMembershipHandoffErrorV1 {}

impl From<RnsNativeCrossFieldRlweDirectErrorV1> for RnsNativeDirectGlobalMembershipHandoffErrorV1 {
    fn from(error: RnsNativeCrossFieldRlweDirectErrorV1) -> Self {
        Self::Direct(error)
    }
}

fn source_packing_safe_core_v1(
    projection: RnsNativeCrossFieldRlweSafeCoreProjectionV1,
) -> RnsNativeSourcePackingSafeCoreV1 {
    RnsNativeSourcePackingSafeCoreV1 {
        terminal_predecessor_context_binding_digest: projection
            .terminal_predecessor_context_binding_digest,
        candidate_pre_direct_inventory_context_digest: projection
            .candidate_pre_direct_inventory_context_digest,
        candidate_pre_direct_inventory_root: projection.candidate_pre_direct_inventory_root,
        existing_radix_candidate_root: projection.existing_radix_candidate_root,
        direct_core_safe_digest: projection.direct_core_safe_digest,
    }
}

/// Move-only evidence that both the exact direct frame and the recursively
/// owned global-membership chain verified in one chronology.
///
/// The retained direct owner proves cross-root equality was discharged; the
/// terminal equality fact proves the global and zero roots matched concrete
/// verifier evidence, and the retained inventory preserves the authenticated
/// source/terminal lineage.
/// Neither is exposed through the source/packing predecessor trait.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the private same-opening child consumes this combined owner exactly once"
)]
#[must_use = "the combined direct/membership owner must advance to source/packing same-opening"]
pub(super) struct RnsNativeDirectGlobalMembershipHandoffV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    _atomic_direct: RnsNativeCrossFieldRlweAllRootsVerifiedV2<'source, 'proof, S>,
    membership_residual: &'proof [u8],
    safe_core: RnsNativeSourcePackingSafeCoreV1,
    outer_bindings: RnsNativeSourcePackingCombinedOuterBindingsV1,
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'proof>
    for RnsNativeDirectGlobalMembershipHandoffV1<'source, 'proof, S>
{
    fn same_opening_successor_v1(&self) -> &'proof [u8] {
        self.membership_residual
    }

    fn successor_independent_safe_core_v1(&self) -> RnsNativeSourcePackingSafeCoreV1 {
        self.safe_core
    }

    fn combined_outer_bindings_v1(&self) -> RnsNativeSourcePackingCombinedOuterBindingsV1 {
        self.outer_bindings
    }
}

/// Consume the completed membership chain, recover its exact claimed direct
/// carrier, verify the four direct cores, and discharge all three claimed-root
/// equalities.
///
/// The sole argument recursively owns the numeric sidecar and authenticated
/// point aliases. There is no detached source parameter, raw-parts entry, or
/// public generic adapter.
#[allow(
    dead_code,
    reason = "the one-argument private handoff awaits live source correspondence evidence"
)]
pub(super) fn verify_rns_native_direct_global_membership_handoff_v2<'source, 'proof, S>(
    membership: RnsNativeGlobalMembershipPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeDirectGlobalMembershipHandoffV1<'source, 'proof, S>,
    RnsNativeDirectGlobalMembershipHandoffErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let verified_global_lookup_root =
        derive_rns_native_verified_global_lookup_core_root_v2(&membership)
            .map_err(|_| RnsNativeDirectGlobalMembershipHandoffErrorV1::GlobalLookupRoot)?;

    // Retain the verified outer identities before consuming each move-only
    // predecessor. They remain inaccessible to the same-opening child until
    // its Schnorr equation has verified and asks for the outer bundle.
    let membership_residual = membership.residual();
    let global_membership_binding_digest = membership.binding_digest();

    let inverse = membership.into_previous_v1();
    let global_inverse_product_binding_digest = inverse.binding_digest();

    let post_z = inverse.into_previous_v1();
    let global_lookup_pre_z_binding_digest = post_z.pre_z_binding_digest();
    let global_lookup_post_z_binding_digest = post_z.binding_digest();

    let centering = post_z.into_previous_v1();
    let centering_subtraction_binding_digest = centering.binding_digest();

    let radix_complement = centering.into_previous_v1();
    let radix_complement_binding_digest = radix_complement.binding_digest();

    let existing_radix = radix_complement.into_previous_v1();
    let existing_radix_binding_digest = existing_radix.binding_digest();

    let q_mask = existing_radix.previous();
    let q_mask_linear_relations_binding_digest = q_mask.binding_digest();
    let small_sign = q_mask.previous();
    let small_sign_disjointness_binding_digest = small_sign.binding_digest();
    let range_carry = small_sign.previous();
    let comparator_range_carry_binding_digest = range_carry.binding_digest();
    let comparator = range_carry.previous();
    let comparator_binding_digest = comparator.binding_digest();

    let atomic_direct = existing_radix.verify_claimed_direct_v2()?;
    let atomic_direct = atomic_direct
        .discharge_terminal_roots_v2(verified_global_lookup_root)
        .map_err(RnsNativeDirectGlobalMembershipHandoffErrorV1::Direct)?;

    let safe_core = source_packing_safe_core_v1(atomic_direct.direct().safe_core_projection_v1());
    let inventory = atomic_direct.inventory();
    let direct_binding_digest = atomic_direct.direct().binding_digest();
    let linked_source = inventory.linked().source();
    let mut outer_bindings = RnsNativeSourcePackingCombinedOuterBindingsV1 {
        source_statement_anchor_digest: linked_source.statement_anchor_digest(),
        source_final_aggregation_schedule_digest: linked_source.aggregation_schedule_digest(),
        enclosing_packing_binding_digest: inventory.enclosing_packing_binding_digest_v1(),
        inventory_prior_context_digest: inventory.prior_context_digest(),
        inventory_root: inventory.inventory_root(),
        inventory_continuation_digest: inventory.continuation_digest(),
        inventory_binding_digest: inventory.binding_digest(),
        direct_binding_digest,
        comparator_binding_digest,
        comparator_range_carry_binding_digest,
        small_sign_disjointness_binding_digest,
        q_mask_linear_relations_binding_digest,
        existing_radix_binding_digest,
        radix_complement_binding_digest,
        centering_subtraction_binding_digest,
        global_lookup_pre_z_binding_digest,
        global_lookup_post_z_binding_digest,
        global_inverse_product_binding_digest,
        global_membership_binding_digest,
        combined_outer_binding_digest: [0; DIGEST_BYTES_V1],
    };
    outer_bindings.combined_outer_binding_digest =
        outer_bindings.canonical_combined_outer_binding_digest_v1();

    Ok(RnsNativeDirectGlobalMembershipHandoffV1 {
        _atomic_direct: atomic_direct,
        membership_residual,
        safe_core,
        outer_bindings,
    })
}

#[cfg(test)]
#[path = "rns_native_direct_global_membership_handoff_tests.rs"]
mod tests;
