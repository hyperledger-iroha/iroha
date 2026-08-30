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
//! The source-only combined owner now lends its recursively owned repeatable
//! snapshot to the same-opening verifier without detaching either owner, and
//! retains the authenticated radix alias needed by that replay. No live
//! production numeric source, prover mask owner, staged entry, composite
//! capability, readiness, receipt, or release authority is made available by
//! this private handoff.

use core::{cell::Cell, fmt};

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
    rns_native_profile::zk_ams_mkhe_rns_native_profile_manifest_v1,
    rns_native_source::{
        ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1, ZkAmsMkheRnsNativeSecretChunkV1,
        ZkAmsMkheRnsNativeSourceArenaV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_source_packing_same_opening::{
        RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_BLOCKS_PER_GROUP_V1,
        RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_GROUP_COUNT_V1,
        RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_SCALARS_PER_BLOCK_V1,
        RNS_NATIVE_SOURCE_PACKING_OWNER_COUNT_V1, RNS_NATIVE_SOURCE_PACKING_RADIX_LOW_DIGITS_V1,
        RNS_NATIVE_SOURCE_PACKING_SIGNED_BLOCKS_PER_PLANE_V1,
        RNS_NATIVE_SOURCE_PACKING_SIGNED_OWNER_COUNT_V1,
        RNS_NATIVE_SOURCE_PACKING_SIGNED_SCALARS_PER_BLOCK_V1,
        RNS_NATIVE_SOURCE_PACKING_VECTOR_COORDINATES_V1, RnsNativeSignedSourceRoleV1,
        RnsNativeSourcePackingAggregateReplayV1, RnsNativeSourcePackingAuthenticatedSourceAxesV1,
        RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1,
        RnsNativeSourcePackingCombinedOuterBindingsV1, RnsNativeSourcePackingOwnerCoordinateV1,
        RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSafeCoreV1,
        RnsNativeSourcePackingSameOpeningContextV1, RnsNativeSourcePackingSameOpeningErrorV1,
        canonical_replay_schedule_digest_v1, difference_scalar_from_be_bytes_v1,
        difference_source_index_v1, owner_coordinate_v1,
        signed_scalar_from_twos_complement_be_i64_v1, signed_source_index_v1,
    },
};
use crate::vega::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::ZeroizingT256ScalarVecV1,
};

const DIGEST_BYTES_V1: usize = 32;

/// This handoff is ownership-only: it inserts no frame between membership and
/// the source/packing same-opening child.
const DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_OWNED_WIRE_BYTES_V1: usize = 0;
const DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_SUCCESSOR_MAX_BYTES_V1: usize =
    RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1;

const VERIFIER_SIDE_DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_IMPLEMENTED_V1: bool = true;
const VERIFIER_SIDE_AUTHENTICATED_REPLAY_HANDOFF_IMPLEMENTED_V1: bool = true;
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
    assert!(VERIFIER_SIDE_AUTHENTICATED_REPLAY_HANDOFF_IMPLEMENTED_V1);
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
    SourceReplayContext,
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

struct ZeroizingReplayScalarV1(Scalar);

impl ZeroizingReplayScalarV1 {
    const fn new_v1(value: Scalar) -> Self {
        Self(value)
    }

    const fn as_ref_v1(&self) -> &Scalar {
        &self.0
    }
}

impl Drop for ZeroizingReplayScalarV1 {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

struct RnsNativeDirectGlobalMembershipReplayStateV1 {
    difference_low_reads: Cell<usize>,
    difference_top_reads: Cell<usize>,
    signed_reads: Cell<usize>,
    replayed: bool,
}

impl RnsNativeDirectGlobalMembershipReplayStateV1 {
    const fn new_v1() -> Self {
        Self {
            difference_low_reads: Cell::new(0),
            difference_top_reads: Cell::new(0),
            signed_reads: Cell::new(0),
            replayed: false,
        }
    }
}

fn authenticated_source_axes_v1<S>(
    direct: &RnsNativeCrossFieldRlweAllRootsVerifiedV2<'_, '_, S>,
) -> Result<
    RnsNativeSourcePackingAuthenticatedSourceAxesV1,
    RnsNativeDirectGlobalMembershipHandoffErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let source = direct.inventory().linked().source();
    let snapshot = source.snapshot();
    let layout = snapshot.layout();
    let receipt = snapshot
        .structural_receipt()
        .map_err(|_| RnsNativeDirectGlobalMembershipHandoffErrorV1::SourceReplayContext)?;
    receipt
        .validate(layout)
        .map_err(|_| RnsNativeDirectGlobalMembershipHandoffErrorV1::SourceReplayContext)?;
    let profile_manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
        .map_err(|_| RnsNativeDirectGlobalMembershipHandoffErrorV1::SourceReplayContext)?;
    Ok(RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
        profile_manifest_digest: profile_manifest.manifest_digest,
        source_binding_digest: layout.source_binding_digest(),
        main_snapshot_digest: receipt.main_snapshot_digest,
        nonce_snapshot_digest: receipt.nonce_snapshot_digest,
        source_receipt_digest: receipt.receipt_digest,
        source_formula_digest: source.formula_digest(),
        source_mapping_digest: source.mapping_digest(),
    })
}

fn replay_context_v1(
    source: RnsNativeSourcePackingAuthenticatedSourceAxesV1,
    safe_core: RnsNativeSourcePackingSafeCoreV1,
) -> RnsNativeSourcePackingSameOpeningContextV1 {
    RnsNativeSourcePackingSameOpeningContextV1 {
        profile_manifest_digest: source.profile_manifest_digest,
        source_binding_digest: source.source_binding_digest,
        main_snapshot_digest: source.main_snapshot_digest,
        nonce_snapshot_digest: source.nonce_snapshot_digest,
        source_receipt_digest: source.source_receipt_digest,
        source_formula_digest: source.source_formula_digest,
        source_mapping_digest: source.source_mapping_digest,
        safe_core,
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
    authenticated_source_axes: RnsNativeSourcePackingAuthenticatedSourceAxesV1,
    canonical_replay_schedule_digest: [u8; DIGEST_BYTES_V1],
    replay_state: RnsNativeDirectGlobalMembershipReplayStateV1,
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

fn advance_replay_counter_v1(
    counter: &Cell<usize>,
) -> Result<(), RnsNativeSourcePackingSameOpeningErrorV1> {
    counter.set(
        counter
            .get()
            .checked_add(1)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?,
    );
    Ok(())
}

impl<'owner, 'source, 'proof, S> RnsNativeSourcePackingAggregateReplayV1
    for &'owner mut RnsNativeDirectGlobalMembershipHandoffV1<'source, 'proof, S>
where
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    fn authenticated_source_axes_v1(&self) -> RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
        self.authenticated_source_axes
    }

    fn canonical_replay_schedule_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.canonical_replay_schedule_digest
    }

    fn difference_low_commitment_v1(
        &self,
        group: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        if group >= RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_GROUP_COUNT_V1
            || digit >= RNS_NATIVE_SOURCE_PACKING_RADIX_LOW_DIGITS_V1
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        let point = self
            ._atomic_direct
            .existing_radix()
            .difference_low_commitment_v1(group, digit)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)?;
        advance_replay_counter_v1(&self.replay_state.difference_low_reads)?;
        Ok(point)
    }

    fn difference_top_commitment_v1(
        &self,
        group: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        if group >= RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_GROUP_COUNT_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        let point = self
            ._atomic_direct
            .inventory()
            .comparator_top_commitments(group)
            .map(|(difference_top, _)| difference_top)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)?;
        advance_replay_counter_v1(&self.replay_state.difference_top_reads)?;
        Ok(point)
    }

    fn signed_commitment_v1(
        &self,
        record: usize,
        role: RnsNativeSignedSourceRoleV1,
        plane: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        if record >= RNS_NATIVE_SOURCE_PACKING_SIGNED_OWNER_COUNT_V1 / (3 * 8) || plane >= 8 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        let signed_unit = record
            .checked_mul(3)
            .and_then(|value| value.checked_add(role as usize))
            .and_then(|value| value.checked_mul(8))
            .and_then(|value| value.checked_add(plane))
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
        if signed_unit >= RNS_NATIVE_SOURCE_PACKING_SIGNED_OWNER_COUNT_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        let point = self
            ._atomic_direct
            .inventory()
            .small_source_product_commitments(signed_unit)
            .map(|commitments| commitments.signed)
            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)?;
        advance_replay_counter_v1(&self.replay_state.signed_reads)?;
        Ok(point)
    }

    fn replay_tau_aggregate_v1(
        &mut self,
        tau: Scalar,
        destination: &mut ZeroizingT256ScalarVecV1,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>
    {
        if self.replay_state.replayed
            || tau.is_zero()
            || destination.len() != RNS_NATIVE_SOURCE_PACKING_VECTOR_COORDINATES_V1
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        self.replay_state.replayed = true;
        for value in destination.as_mut_slice() {
            value.clear_secret();
        }

        let snapshot = self
            ._atomic_direct
            .inventory_mut()
            .linked_mut()
            .source_mut()
            .snapshot_mut();
        let arena = ZkAmsMkheRnsNativeSourceArenaV1::Main;
        let mut power = Scalar::one();
        for ordinal in 0..RNS_NATIVE_SOURCE_PACKING_OWNER_COUNT_V1 {
            match owner_coordinate_v1(ordinal)? {
                RnsNativeSourcePackingOwnerCoordinateV1::Difference { group } => {
                    let group = usize::from(group);
                    for block in 0..RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_BLOCKS_PER_GROUP_V1 {
                        let first = difference_source_index_v1(group, block)?;
                        let chunk = snapshot
                            .read_slot(arena, u64::from(first.source_slot))
                            .map_err(|_| {
                                RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable
                            })?;
                        if chunk.arena() != arena
                            || u64::try_from(chunk.as_slice().len()).ok()
                                != Some(arena.plaintext_bytes())
                        {
                            return Err(
                                RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable,
                            );
                        }
                        for scalar_in_block in
                            0..RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_SCALARS_PER_BLOCK_V1
                        {
                            let coordinate = scalar_in_block
                                .checked_mul(
                                    RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_BLOCKS_PER_GROUP_V1,
                                )
                                .and_then(|value| value.checked_add(block))
                                .ok_or(
                                    RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow,
                                )?;
                            let index = difference_source_index_v1(group, coordinate)?;
                            if usize::from(index.owner_ordinal) != ordinal
                                || index.source_slot != first.source_slot
                            {
                                return Err(
                                    RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry,
                                );
                            }
                            let start = usize::from(index.byte_offset);
                            let end = start.checked_add(32).ok_or(
                                RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow,
                            )?;
                            let mut encoded: [u8; 32] = chunk
                                .as_slice()
                                .get(start..end)
                                .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)?
                                .try_into()
                                .map_err(|_| {
                                    RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable
                                })?;
                            let decoded = difference_scalar_from_be_bytes_v1(encoded);
                            encoded.fill(0);
                            let value = ZeroizingReplayScalarV1::new_v1(decoded?);
                            destination.as_mut_slice()[coordinate] += *value.as_ref_v1() * power;
                        }
                    }
                }
                RnsNativeSourcePackingOwnerCoordinateV1::Signed {
                    record,
                    role,
                    plane,
                } => {
                    let signed_unit = usize::from(record)
                        .checked_mul(3)
                        .and_then(|value| value.checked_add(role as usize))
                        .and_then(|value| value.checked_mul(8))
                        .and_then(|value| value.checked_add(usize::from(plane)))
                        .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
                    for local_block in 0..RNS_NATIVE_SOURCE_PACKING_SIGNED_BLOCKS_PER_PLANE_V1 {
                        let first_coordinate = local_block
                            .checked_mul(RNS_NATIVE_SOURCE_PACKING_SIGNED_SCALARS_PER_BLOCK_V1)
                            .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow)?;
                        let first = signed_source_index_v1(signed_unit, first_coordinate)?;
                        let chunk = snapshot
                            .read_slot(arena, u64::from(first.source_slot))
                            .map_err(|_| {
                                RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable
                            })?;
                        if chunk.arena() != arena
                            || u64::try_from(chunk.as_slice().len()).ok()
                                != Some(arena.plaintext_bytes())
                        {
                            return Err(
                                RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable,
                            );
                        }
                        for coefficient in 0..RNS_NATIVE_SOURCE_PACKING_SIGNED_SCALARS_PER_BLOCK_V1
                        {
                            let coordinate = first_coordinate.checked_add(coefficient).ok_or(
                                RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow,
                            )?;
                            let index = signed_source_index_v1(signed_unit, coordinate)?;
                            if usize::from(index.owner_ordinal) != ordinal
                                || index.source_slot != first.source_slot
                            {
                                return Err(
                                    RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry,
                                );
                            }
                            let start = usize::from(index.byte_offset);
                            let end = start.checked_add(8).ok_or(
                                RnsNativeSourcePackingSameOpeningErrorV1::ArithmeticOverflow,
                            )?;
                            let mut encoded: [u8; 8] = chunk
                                .as_slice()
                                .get(start..end)
                                .ok_or(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)?
                                .try_into()
                                .map_err(|_| {
                                    RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable
                                })?;
                            let value = ZeroizingReplayScalarV1::new_v1(
                                signed_scalar_from_twos_complement_be_i64_v1(encoded),
                            );
                            encoded.fill(0);
                            destination.as_mut_slice()[coordinate] += *value.as_ref_v1() * power;
                        }
                    }
                }
            }
            power *= tau;
        }
        Ok(RnsNativeSourcePackingReplayReceiptV1 {
            source_binding_digest: self.authenticated_source_axes.source_binding_digest,
            canonical_replay_schedule_digest: self.canonical_replay_schedule_digest,
            owner_count: u16::try_from(RNS_NATIVE_SOURCE_PACKING_OWNER_COUNT_V1)
                .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
            coordinates: u16::try_from(RNS_NATIVE_SOURCE_PACKING_VECTOR_COORDINATES_V1)
                .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
        })
    }

    fn finish_v1(
        self,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>
    {
        let refreshed_source_axes = authenticated_source_axes_v1(&self._atomic_direct)
            .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)?;
        let refreshed_schedule = canonical_replay_schedule_digest_v1(replay_context_v1(
            refreshed_source_axes,
            self.safe_core,
        ))?;
        if !self.replay_state.replayed
            || refreshed_source_axes != self.authenticated_source_axes
            || refreshed_schedule != self.canonical_replay_schedule_digest
            || self.replay_state.difference_low_reads.get()
                != RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_GROUP_COUNT_V1
                    * RNS_NATIVE_SOURCE_PACKING_RADIX_LOW_DIGITS_V1
            || self.replay_state.difference_top_reads.get()
                != RNS_NATIVE_SOURCE_PACKING_DIFFERENCE_GROUP_COUNT_V1
            || self.replay_state.signed_reads.get()
                != RNS_NATIVE_SOURCE_PACKING_SIGNED_OWNER_COUNT_V1
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable);
        }
        Ok(RnsNativeSourcePackingReplayReceiptV1 {
            source_binding_digest: self.authenticated_source_axes.source_binding_digest,
            canonical_replay_schedule_digest: self.canonical_replay_schedule_digest,
            owner_count: u16::try_from(RNS_NATIVE_SOURCE_PACKING_OWNER_COUNT_V1)
                .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
            coordinates: u16::try_from(RNS_NATIVE_SOURCE_PACKING_VECTOR_COORDINATES_V1)
                .map_err(|_| RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry)?,
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
    let authenticated_source_axes = authenticated_source_axes_v1(&atomic_direct)?;
    let canonical_replay_schedule_digest = canonical_replay_schedule_digest_v1(replay_context_v1(
        authenticated_source_axes,
        safe_core,
    ))
    .map_err(|_| RnsNativeDirectGlobalMembershipHandoffErrorV1::SourceReplayContext)?;
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
        authenticated_source_axes,
        canonical_replay_schedule_digest,
        replay_state: RnsNativeDirectGlobalMembershipReplayStateV1::new_v1(),
        outer_bindings,
    })
}

#[cfg(test)]
#[path = "rns_native_direct_global_membership_handoff_tests.rs"]
mod tests;
