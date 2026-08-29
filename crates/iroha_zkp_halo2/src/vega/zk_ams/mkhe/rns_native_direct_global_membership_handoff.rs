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
//! The private handoff now lends its retained point inventory and repeatable
//! authenticated source to the same-opening verifier without detached parts.
//! No live production entry, mask owner, composite capability, readiness,
//! receipt, or release authority is made available by this join.

use core::fmt;

use super::{
    collective::RnsNativeQpcsCompositeAuthorityV2,
    rns_native_composite_verifier::RnsNativeCrossFieldRlweCompositeInputV2,
    rns_native_cross_field_rlwe_direct::{
        RnsNativeCrossFieldRlweAllRootsVerifiedV2, RnsNativeCrossFieldRlweDirectErrorV1,
        RnsNativeCrossFieldRlweSafeCoreProjectionV1,
    },
    rns_native_global_lookup_z_commitment_view::rns_native_global_inverse_product_sumcheck::{
        RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1,
        RnsNativeGlobalMembershipPrerequisiteV1,
        derive_rns_native_verified_global_lookup_core_root_v2,
    },
    rns_native_source::{
        ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1, ZkAmsMkheRnsNativeSecretChunkV1,
        ZkAmsMkheRnsNativeSourceArenaV1, ZkAmsMkheRnsNativeSourceErrorV1,
        ZkAmsMkheRnsNativeSourceLayoutV1, ZkAmsMkheRnsNativeSourceReceiptV1,
        ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_source_packing_same_opening::{
        DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1, DIFFERENCE_GROUPS_V1, DIFFERENCE_SCALAR_BYTES_V1,
        DIFFERENCE_SCALARS_PER_BLOCK_V1, MAIN_SOURCE_BLOCK_BYTES_V1, OWNERS_V1,
        RnsNativeSignedSourceRoleV1, RnsNativeSourcePackingAggregateReplayV1,
        RnsNativeSourcePackingAuthenticatedSourceAxesV1,
        RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1,
        RnsNativeSourcePackingCombinedOuterBindingsV1, RnsNativeSourcePackingCompositeAuthorityV2,
        RnsNativeSourcePackingCompositeTransitionV2,
        RnsNativeSourcePackingOwnedReplayPredecessorV2, RnsNativeSourcePackingReplayReceiptV1,
        RnsNativeSourcePackingSafeCoreV1, RnsNativeSourcePackingSameOpeningContextV1,
        RnsNativeSourcePackingSameOpeningErrorV1, SIGNED_BLOCKS_PER_PLANE_V1, SIGNED_OWNERS_V1,
        SIGNED_SCALAR_BYTES_V1, SIGNED_SCALARS_PER_BLOCK_V1, VECTOR_COORDINATES_V1,
        canonical_profile_manifest_digest_v1, canonical_replay_schedule_digest_v1,
        difference_scalar_from_be_bytes_v1, difference_source_index_v1,
        signed_scalar_from_twos_complement_be_i64_v1, signed_source_index_v1,
    },
    rns_native_wire::ZkAmsMkheRnsNativeProofEnvelopeV1,
};

use crate::vega::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::{ZeroizingT256ScalarCopyV1, ZeroizingT256ScalarVecV1},
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

/// Failure while consuming the completed handoff into the final composite
/// context.  Earlier root-recovery failures cannot inhabit this transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeDirectGlobalMembershipCompositeTransitionErrorV2 {
    SourcePacking(RnsNativeSourcePackingSameOpeningErrorV1),
    CompositeContext(RnsNativeCrossFieldRlweDirectErrorV1),
}

impl fmt::Display for RnsNativeDirectGlobalMembershipCompositeTransitionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeDirectGlobalMembershipCompositeTransitionErrorV2 {}

impl From<RnsNativeSourcePackingSameOpeningErrorV1>
    for RnsNativeDirectGlobalMembershipCompositeTransitionErrorV2
{
    fn from(error: RnsNativeSourcePackingSameOpeningErrorV1) -> Self {
        Self::SourcePacking(error)
    }
}

impl From<RnsNativeCrossFieldRlweDirectErrorV1>
    for RnsNativeDirectGlobalMembershipCompositeTransitionErrorV2
{
    fn from(error: RnsNativeCrossFieldRlweDirectErrorV1) -> Self {
        Self::CompositeContext(error)
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

impl<'source, 'proof, 'envelope, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeSourcePackingCompositeTransitionV2<'proof, 'envelope>
    for RnsNativeDirectGlobalMembershipHandoffV1<'source, 'proof, S>
{
    type CompositeInput = RnsNativeCrossFieldRlweCompositeInputV2<'source, 'proof, 'envelope, S>;
    type Error = RnsNativeDirectGlobalMembershipCompositeTransitionErrorV2;

    fn consume_source_packing_for_composite_v2(
        self,
        source_packing_authority: RnsNativeSourcePackingCompositeAuthorityV2<'envelope>,
        qpcs_authority: RnsNativeQpcsCompositeAuthorityV2<'envelope>,
        envelope: &'envelope ZkAmsMkheRnsNativeProofEnvelopeV1,
    ) -> Result<Self::CompositeInput, Self::Error> {
        source_packing_authority
            .validate_predecessor_v2(self.safe_core, self.outer_bindings)
            .map_err(RnsNativeDirectGlobalMembershipCompositeTransitionErrorV2::from)?;
        let Self {
            _atomic_direct,
            membership_residual: _,
            safe_core: _,
            outer_bindings: _,
        } = self;
        _atomic_direct
            .into_composite_context_v2(envelope, source_packing_authority, qpcs_authority)
            .map_err(RnsNativeDirectGlobalMembershipCompositeTransitionErrorV2::from)
    }
}

fn map_source_replay_error_v2(
    error: ZkAmsMkheRnsNativeSourceErrorV1,
) -> RnsNativeSourcePackingSameOpeningErrorV1 {
    match error {
        ZkAmsMkheRnsNativeSourceErrorV1::Allocation
        | ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded => {
            RnsNativeSourcePackingSameOpeningErrorV1::ResourceExhausted
        }
        _ => RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable,
    }
}

fn source_packing_context_v2<'source, 'proof, S>(
    atomic: &RnsNativeCrossFieldRlweAllRootsVerifiedV2<'source, 'proof, S>,
    safe_core: RnsNativeSourcePackingSafeCoreV1,
) -> Result<
    (
        RnsNativeSourcePackingSameOpeningContextV1,
        ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceReceiptV1,
    ),
    RnsNativeSourcePackingSameOpeningErrorV1,
>
where
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    let source = atomic.inventory().linked().source();
    let snapshot = source.snapshot();
    let layout = snapshot.layout();
    layout.validate().map_err(map_source_replay_error_v2)?;
    let receipt = snapshot
        .structural_receipt()
        .map_err(map_source_replay_error_v2)?;
    receipt
        .validate(layout)
        .map_err(map_source_replay_error_v2)?;
    let context = RnsNativeSourcePackingSameOpeningContextV1 {
        profile_manifest_digest: canonical_profile_manifest_digest_v1()?,
        source_binding_digest: receipt.source_binding_digest,
        main_snapshot_digest: receipt.main_snapshot_digest,
        nonce_snapshot_digest: receipt.nonce_snapshot_digest,
        source_receipt_digest: receipt.receipt_digest,
        source_formula_digest: source.formula_digest(),
        source_mapping_digest: source.mapping_digest(),
        safe_core,
    };
    // This validates the complete context and freezes the sole owner order
    // before any mutable source borrow can begin.
    canonical_replay_schedule_digest_v1(context)?;
    Ok((context, layout, receipt))
}

/// Borrow-backed replay minted only from the exact all-roots direct owner.
/// It can neither outlive nor be substituted for that owner.
pub(super) struct RnsNativeDirectGlobalMembershipReplayV2<
    'owner,
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
> {
    atomic: &'owner mut RnsNativeCrossFieldRlweAllRootsVerifiedV2<'source, 'proof, S>,
    context: RnsNativeSourcePackingSameOpeningContextV1,
    expected_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    expected_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    schedule_digest: [u8; DIGEST_BYTES_V1],
    replayed: bool,
}

impl<S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1>
    RnsNativeDirectGlobalMembershipReplayV2<'_, '_, '_, S>
{
    fn expected_replay_receipt_v2(&self) -> RnsNativeSourcePackingReplayReceiptV1 {
        RnsNativeSourcePackingReplayReceiptV1 {
            source_binding_digest: self.context.source_binding_digest,
            canonical_replay_schedule_digest: self.schedule_digest,
            owner_count: OWNERS_V1 as u16,
            coordinates: VECTOR_COORDINATES_V1 as u16,
        }
    }

    fn validate_snapshot_stability_v2(
        &mut self,
    ) -> Result<(), RnsNativeSourcePackingSameOpeningErrorV1> {
        let snapshot = self.atomic.source_packing_snapshot_mut_v2();
        if snapshot.layout() != self.expected_layout
            || snapshot
                .structural_receipt()
                .map_err(map_source_replay_error_v2)?
                != self.expected_receipt
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable);
        }
        Ok(())
    }
}

impl<S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1> RnsNativeSourcePackingAggregateReplayV1
    for RnsNativeDirectGlobalMembershipReplayV2<'_, '_, '_, S>
{
    fn authenticated_source_axes_v1(&self) -> RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
        self.context.authenticated_source_axes_v1()
    }

    fn canonical_replay_schedule_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.schedule_digest
    }

    fn difference_low_commitment_v1(
        &self,
        group: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        self.atomic
            .source_packing_difference_low_commitment_v2(group, digit)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)
    }

    fn difference_top_commitment_v1(
        &self,
        group: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        self.atomic
            .inventory()
            .comparator_top_commitments(group)
            .map(|(difference, _)| difference)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)
    }

    fn signed_commitment_v1(
        &self,
        record: usize,
        role: RnsNativeSignedSourceRoleV1,
        plane: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        let owner = record
            .checked_mul(3)
            .and_then(|value| value.checked_add(role as usize))
            .and_then(|value| value.checked_mul(8))
            .and_then(|value| value.checked_add(plane))
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
        self.atomic
            .inventory()
            .small_source_product_commitments(owner)
            .map(|commitments| commitments.signed)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)
    }

    fn replay_tau_aggregate_v1(
        &mut self,
        tau: Scalar,
        destination: &mut ZeroizingT256ScalarVecV1,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>
    {
        if self.replayed || destination.len() != VECTOR_COORDINATES_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        destination.as_mut_slice().fill(Scalar::zero());
        let snapshot = self.atomic.source_packing_snapshot_mut_v2();
        let mut power = Scalar::one();
        let mut reads = 0_usize;

        for group in 0..DIFFERENCE_GROUPS_V1 {
            for block in 0..DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1 {
                let first = difference_source_index_v1(group, block)?;
                let chunk = snapshot
                    .read_slot(
                        ZkAmsMkheRnsNativeSourceArenaV1::Main,
                        u64::from(first.source_slot),
                    )
                    .map_err(map_source_replay_error_v2)?;
                if chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Main
                    || chunk.as_slice().len() != MAIN_SOURCE_BLOCK_BYTES_V1
                {
                    return Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable);
                }
                reads = reads
                    .checked_add(1)
                    .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
                for scalar_in_block in 0..DIFFERENCE_SCALARS_PER_BLOCK_V1 {
                    let coordinate = scalar_in_block
                        .checked_mul(DIFFERENCE_BLOCKS_PER_LOCAL_GROUP_V1)
                        .and_then(|value| value.checked_add(block))
                        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
                    let index = difference_source_index_v1(group, coordinate)?;
                    if index.source_slot != first.source_slot {
                        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
                    }
                    let offset = usize::from(index.byte_offset);
                    let end = offset
                        .checked_add(DIFFERENCE_SCALAR_BYTES_V1)
                        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
                    let encoded: &[u8; DIFFERENCE_SCALAR_BYTES_V1] = chunk
                        .as_slice()
                        .get(offset..end)
                        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)?
                        .try_into()
                        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)?;
                    let scalar = ZeroizingT256ScalarCopyV1::new(
                        difference_scalar_from_be_bytes_v1(encoded)?,
                    );
                    destination.as_mut_slice()[coordinate] += power * scalar.get();
                }
            }
            power *= tau;
        }

        for signed_unit in 0..SIGNED_OWNERS_V1 {
            for block in 0..SIGNED_BLOCKS_PER_PLANE_V1 {
                let first =
                    signed_source_index_v1(signed_unit, block * SIGNED_SCALARS_PER_BLOCK_V1)?;
                let chunk = snapshot
                    .read_slot(
                        ZkAmsMkheRnsNativeSourceArenaV1::Main,
                        u64::from(first.source_slot),
                    )
                    .map_err(map_source_replay_error_v2)?;
                if chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Main
                    || chunk.as_slice().len() != MAIN_SOURCE_BLOCK_BYTES_V1
                {
                    return Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable);
                }
                reads = reads
                    .checked_add(1)
                    .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
                for coefficient in 0..SIGNED_SCALARS_PER_BLOCK_V1 {
                    let coordinate = block
                        .checked_mul(SIGNED_SCALARS_PER_BLOCK_V1)
                        .and_then(|value| value.checked_add(coefficient))
                        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
                    let index = signed_source_index_v1(signed_unit, coordinate)?;
                    if index.source_slot != first.source_slot {
                        return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
                    }
                    let offset = usize::from(index.byte_offset);
                    let end = offset
                        .checked_add(SIGNED_SCALAR_BYTES_V1)
                        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
                    let encoded: &[u8; SIGNED_SCALAR_BYTES_V1] = chunk
                        .as_slice()
                        .get(offset..end)
                        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)?
                        .try_into()
                        .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)?;
                    let scalar = ZeroizingT256ScalarCopyV1::new(
                        signed_scalar_from_twos_complement_be_i64_v1(encoded),
                    );
                    destination.as_mut_slice()[coordinate] += power * scalar.get();
                }
            }
            power *= tau;
        }

        if reads
            != usize::try_from(ZkAmsMkheRnsNativeSourceArenaV1::Main.slot_count())
                .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        self.replayed = true;
        self.validate_snapshot_stability_v2()?;
        Ok(self.expected_replay_receipt_v2())
    }

    fn finish_v1(
        mut self,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>
    {
        if !self.replayed {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable);
        }
        self.validate_snapshot_stability_v2()?;
        Ok(self.expected_replay_receipt_v2())
    }
}

impl<'source, 'proof, S> RnsNativeSourcePackingOwnedReplayPredecessorV2<'proof>
    for RnsNativeDirectGlobalMembershipHandoffV1<'source, 'proof, S>
where
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    type Replay<'owner>
        = RnsNativeDirectGlobalMembershipReplayV2<'owner, 'source, 'proof, S>
    where
        Self: 'owner;

    fn authenticated_same_opening_context_v2(
        &self,
    ) -> Result<RnsNativeSourcePackingSameOpeningContextV1, RnsNativeSourcePackingSameOpeningErrorV1>
    {
        source_packing_context_v2(&self._atomic_direct, self.safe_core)
            .map(|(context, _, _)| context)
    }

    fn begin_authenticated_replay_v2(
        &mut self,
    ) -> Result<Self::Replay<'_>, RnsNativeSourcePackingSameOpeningErrorV1> {
        let (context, expected_layout, expected_receipt) =
            source_packing_context_v2(&self._atomic_direct, self.safe_core)?;
        let schedule_digest = canonical_replay_schedule_digest_v1(context)?;
        Ok(RnsNativeDirectGlobalMembershipReplayV2 {
            atomic: &mut self._atomic_direct,
            context,
            expected_layout,
            expected_receipt,
            schedule_digest,
            replayed: false,
        })
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
