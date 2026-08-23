//! Prover-side owner for the original-radix pre-`z` commitments.
//!
//! This child advances the proof-session inventory through the already-reserved
//! 11,696 `D`/`S` low-digit slots.  Its public serialization is byte-for-byte the
//! candidate section consumed by `rns_native_existing_radix_commitment_view`:
//! group-major, `D[0..17)` then `S[0..17)`, with no top, inverse, residual, or
//! predecessor material mixed into the candidate root.
//!
//! The owner deliberately stops at that source-only boundary.  Production
//! entropy, the live Phase-23 correspondence, direct quotient openings,
//! resource evidence, readiness, and release remain unavailable.

use super::*;
use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, bulletproof_t256::ZeroizingT256ScalarVecV1};
use core::sync::atomic::{AtomicU64, Ordering};

const EXISTING_RADIX_CANDIDATE_VERSION_V1: u8 = 1;
const EXISTING_RADIX_CANDIDATE_GROUPS_V1: usize = 344;
const EXISTING_RADIX_CANDIDATE_ROLES_V1: usize = 2;
const EXISTING_RADIX_CANDIDATE_LOW_DIGITS_V1: usize = 17;
const EXISTING_RADIX_CANDIDATE_POINTS_PER_GROUP_V1: usize =
    EXISTING_RADIX_CANDIDATE_ROLES_V1 * EXISTING_RADIX_CANDIDATE_LOW_DIGITS_V1;
const EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1: usize =
    EXISTING_RADIX_CANDIDATE_GROUPS_V1 * EXISTING_RADIX_CANDIDATE_POINTS_PER_GROUP_V1;
const EXISTING_RADIX_CANDIDATE_POINT_BYTES_V1: usize = 33;
const EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1: usize =
    EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 * EXISTING_RADIX_CANDIDATE_POINT_BYTES_V1;
const EXISTING_RADIX_CANDIDATE_FIRST_INVENTORY_ORDINAL_V1: u32 = 344;
const EXISTING_RADIX_DIFFERENCE_FIRST_INVENTORY_ORDINAL_V1: u32 = 344;
const EXISTING_RADIX_SLACK_FIRST_INVENTORY_ORDINAL_V1: u32 = 6_192;
const EXISTING_RADIX_CANDIDATE_AFTER_INVENTORY_ORDINAL_V1: u32 = 12_040;
const EXISTING_RADIX_CANDIDATE_RETAINED_BLINDING_BYTES_V1: u64 =
    EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u64 * 32;
const EXISTING_RADIX_CANDIDATE_PUBLIC_WIRE_BYTES_V1: u64 =
    EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1 as u64;
const EXISTING_RADIX_CANDIDATE_SEMANTIC_BYTES_V1: u64 =
    EXISTING_RADIX_CANDIDATE_RETAINED_BLINDING_BYTES_V1
        + EXISTING_RADIX_CANDIDATE_PUBLIC_WIRE_BYTES_V1;
const EXISTING_RADIX_CANDIDATE_NEW_FILE_BYTES_V1: u64 = 0;
const EXISTING_RADIX_CANDIDATE_NEW_IO_BYTES_V1: u64 = 0;

// Process-local ownership identity. It is deliberately excluded from every
// transcript/root digest: its sole job is preventing a sampled move-only token
// from being adopted by a different in-memory assembly with identical semantic
// inputs. Values are never reused; exhaustion fails closed.
static NEXT_EXISTING_RADIX_ASSEMBLY_INSTANCE_V1: AtomicU64 = AtomicU64::new(1);

const EXISTING_RADIX_PRE_Z_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-existing-radix.pre-z-manifest";
const EXISTING_RADIX_PRE_Z_CANDIDATE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-existing-radix.pre-z-candidate-root";
const EXISTING_RADIX_BLINDING_TOKEN_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.existing-radix-candidate.blinding-token\0";
const EXISTING_RADIX_BLINDING_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.existing-radix-candidate.blinding-root\0";
const EXISTING_RADIX_OWNER_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.existing-radix-candidate.owner\0";
const EXISTING_RADIX_POINT_ORDER_LANGUAGE_V1: &[u8] =
    b"ordinal=((group*2+role-index)*17+column);group=0..343;role-index=(0:D-low/tag1,1:S-low/tag2);column=0..16;top-commitments-are-aliased-from-original-inventory-and-never-encoded-here";
const EXISTING_RADIX_SOLE_Z_SEPARATION_LANGUAGE_V1: &[u8] =
    b"pre-z-candidate-root=fixed-manifest||role-group-column-points-only;exclude-full-added-inventory-root,S3/S5/S8/S10-11-roots,residuals,bindings,codec,and-all-inverse-roots;transport-header-binds-predecessors;post-verification-token-binds-predecessor-residual-and-binding";

const EXISTING_RADIX_CANDIDATE_OWNER_MATERIALIZED_V1: bool = true;
const LIVE_PHASE23_EXISTING_RADIX_SOURCE_INTEGRATED_V1: bool = false;
const DIRECT_QUOTIENT_OPENING_OWNERS_INTEGRATED_V1: bool = false;
const RESOURCE_EVIDENCE_ACCEPTED_V1: bool = false;
const READINESS_ACCEPTED_V1: bool = false;
const RELEASE_READY_V1: bool = false;
const RELEASE_COMPLETE_V1: bool = false;

const _: () = {
    assert!(EXISTING_RADIX_CANDIDATE_GROUPS_V1 == 43 * 8);
    assert!(EXISTING_RADIX_CANDIDATE_ROLES_V1 == 2);
    assert!(EXISTING_RADIX_CANDIDATE_LOW_DIGITS_V1 == 17);
    assert!(EXISTING_RADIX_CANDIDATE_POINTS_PER_GROUP_V1 == 34);
    assert!(EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 == 11_696);
    assert!(EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1 == 385_968);
    assert!(EXISTING_RADIX_CANDIDATE_RETAINED_BLINDING_BYTES_V1 == 374_272);
    assert!(EXISTING_RADIX_CANDIDATE_SEMANTIC_BYTES_V1 == 760_240);
    assert!(EXISTING_RADIX_CANDIDATE_NEW_FILE_BYTES_V1 == 0);
    assert!(EXISTING_RADIX_CANDIDATE_NEW_IO_BYTES_V1 == 0);
    assert!(
        EXISTING_RADIX_CANDIDATE_AFTER_INVENTORY_ORDINAL_V1
            - EXISTING_RADIX_CANDIDATE_FIRST_INVENTORY_ORDINAL_V1
            == EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32
    );
    assert!(EXISTING_RADIX_CANDIDATE_OWNER_MATERIALIZED_V1);
    assert!(!LIVE_PHASE23_EXISTING_RADIX_SOURCE_INTEGRATED_V1);
    assert!(!DIRECT_QUOTIENT_OPENING_OWNERS_INTEGRATED_V1);
    assert!(!RESOURCE_EVIDENCE_ACCEPTED_V1);
    assert!(!READINESS_ACCEPTED_V1);
    assert!(!RELEASE_READY_V1);
    assert!(!RELEASE_COMPLETE_V1);
};

/// Typed role accepted by the canonical candidate assembly.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::vega::zk_ams::mkhe) enum RnsNativeExistingRadixCandidateRoleV1 {
    DifferenceLow = 1,
    SlackLow = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExistingRadixCandidateCoordinateV1 {
    wire_ordinal: u32,
    group: u16,
    role: RnsNativeExistingRadixCandidateRoleV1,
    column: u8,
    purpose_ordinal: u32,
    inventory_ordinal: u32,
}

fn existing_radix_candidate_coordinate_v1(
    wire_ordinal: u32,
) -> Result<ExistingRadixCandidateCoordinateV1, ZkAmsMkheErrorV1> {
    let ordinal =
        usize::try_from(wire_ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if ordinal >= EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let group = ordinal / EXISTING_RADIX_CANDIDATE_POINTS_PER_GROUP_V1;
    let local = ordinal % EXISTING_RADIX_CANDIDATE_POINTS_PER_GROUP_V1;
    let (role, column, first_inventory_ordinal) = if local < EXISTING_RADIX_CANDIDATE_LOW_DIGITS_V1
    {
        (
            RnsNativeExistingRadixCandidateRoleV1::DifferenceLow,
            local,
            EXISTING_RADIX_DIFFERENCE_FIRST_INVENTORY_ORDINAL_V1,
        )
    } else {
        (
            RnsNativeExistingRadixCandidateRoleV1::SlackLow,
            local - EXISTING_RADIX_CANDIDATE_LOW_DIGITS_V1,
            EXISTING_RADIX_SLACK_FIRST_INVENTORY_ORDINAL_V1,
        )
    };
    let purpose_ordinal = group
        .checked_mul(EXISTING_RADIX_CANDIDATE_LOW_DIGITS_V1)
        .and_then(|value| value.checked_add(column))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let purpose_ordinal =
        u32::try_from(purpose_ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let inventory_ordinal = first_inventory_ordinal
        .checked_add(purpose_ordinal)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let inventory_coordinate = commitment_coordinate_v1(inventory_ordinal)?;
    let expected_purpose = match role {
        RnsNativeExistingRadixCandidateRoleV1::DifferenceLow => {
            GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow
        }
        RnsNativeExistingRadixCandidateRoleV1::SlackLow => {
            GlobalLookupCommitmentPurposeV1::ExistingSumLow
        }
    };
    if inventory_coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        || inventory_coordinate.purpose != expected_purpose
        || inventory_coordinate.purpose_ordinal != purpose_ordinal
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(ExistingRadixCandidateCoordinateV1 {
        wire_ordinal,
        group: u16::try_from(group).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        role,
        column: u8::try_from(column).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        purpose_ordinal,
        inventory_ordinal,
    })
}

fn next_existing_radix_assembly_instance_v1() -> Result<u64, ZkAmsMkheErrorV1> {
    NEXT_EXISTING_RADIX_ASSEMBLY_INSTANCE_V1
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            current.checked_add(1)
        })
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn existing_radix_pre_z_manifest_digest_v1() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(EXISTING_RADIX_PRE_Z_MANIFEST_DOMAIN_V1);
    hash.update(&[EXISTING_RADIX_CANDIDATE_VERSION_V1]);
    for value in [
        EXISTING_RADIX_CANDIDATE_GROUPS_V1 as u32,
        EXISTING_RADIX_CANDIDATE_ROLES_V1 as u32,
        EXISTING_RADIX_CANDIDATE_LOW_DIGITS_V1 as u32,
        EXISTING_RADIX_CANDIDATE_POINTS_PER_GROUP_V1 as u32,
        EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32,
        EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    for language in [
        EXISTING_RADIX_POINT_ORDER_LANGUAGE_V1,
        EXISTING_RADIX_SOLE_Z_SEPARATION_LANGUAGE_V1,
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.finalize()
}

fn begin_existing_radix_candidate_root_v1() -> Keccak256 {
    let mut hash = Keccak256::new();
    hash.update(EXISTING_RADIX_PRE_Z_CANDIDATE_ROOT_DOMAIN_V1);
    hash.update(&[EXISTING_RADIX_CANDIDATE_VERSION_V1]);
    hash.update(&existing_radix_pre_z_manifest_digest_v1());
    hash
}

fn absorb_existing_radix_candidate_point_v1(
    hash: &mut Keccak256,
    coordinate: ExistingRadixCandidateCoordinateV1,
    point_wire: &[u8; EXISTING_RADIX_CANDIDATE_POINT_BYTES_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    Point::from_non_identity_wire_bytes_exact(point_wire)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    hash.update(&coordinate.wire_ordinal.to_be_bytes());
    hash.update(&coordinate.group.to_be_bytes());
    hash.update(&[coordinate.role as u8, coordinate.column]);
    hash.update(point_wire);
    Ok(())
}

fn existing_radix_candidate_root_from_wire_v1(wire: &[u8]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if wire.len() != EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = begin_existing_radix_candidate_root_v1();
    for wire_ordinal in 0..EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32 {
        let coordinate = existing_radix_candidate_coordinate_v1(wire_ordinal)?;
        let start = usize::try_from(wire_ordinal)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .checked_mul(EXISTING_RADIX_CANDIDATE_POINT_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let point_wire: &[u8; EXISTING_RADIX_CANDIDATE_POINT_BYTES_V1] = wire
            .get(start..start + EXISTING_RADIX_CANDIDATE_POINT_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        absorb_existing_radix_candidate_point_v1(&mut hash, coordinate, point_wire)?;
    }
    require_nonzero_existing_radix_digest_v1(hash.finalize())
}

impl GlobalLookupCommitmentInventorySkeletonV1 {
    fn existing_radix_candidate_ticket_v1(
        &self,
        coordinate: ExistingRadixCandidateCoordinateV1,
    ) -> Result<&GlobalLookupCommitmentTicketV1, ZkAmsMkheErrorV1> {
        let ticket = self
            .slots
            .get(
                usize::try_from(coordinate.inventory_ordinal)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .and_then(Option::as_ref)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let expected = commitment_coordinate_v1(coordinate.inventory_ordinal)?;
        if ticket.coordinate != expected
            || ticket.coordinate.purpose_ordinal != coordinate.purpose_ordinal
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(ticket)
    }

    fn adopt_existing_radix_candidate_v1(
        &mut self,
        coordinate: ExistingRadixCandidateCoordinateV1,
        point: &Point,
    ) -> Result<[u8; EXISTING_RADIX_CANDIDATE_POINT_BYTES_V1], ZkAmsMkheErrorV1> {
        let inventory_coordinate = commitment_coordinate_v1(coordinate.inventory_ordinal)?;
        let slot = self
            .slots
            .get_mut(
                usize::try_from(coordinate.inventory_ordinal)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if slot.is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let point_wire = point
            .to_non_identity_wire_bytes()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        *slot = Some(GlobalLookupCommitmentTicketV1 {
            coordinate: inventory_coordinate,
            point_wire,
        });
        Ok(point_wire)
    }

    fn existing_radix_candidate_root_v1(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let mut hash = begin_existing_radix_candidate_root_v1();
        for wire_ordinal in 0..EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32 {
            let coordinate = existing_radix_candidate_coordinate_v1(wire_ordinal)?;
            let ticket = self.existing_radix_candidate_ticket_v1(coordinate)?;
            absorb_existing_radix_candidate_point_v1(&mut hash, coordinate, &ticket.point_wire)?;
        }
        require_nonzero_existing_radix_digest_v1(hash.finalize())
    }

    fn append_existing_radix_candidate_wire_v1(
        &self,
        destination: &mut Vec<u8>,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        destination
            .try_reserve_exact(EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for wire_ordinal in 0..EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32 {
            let coordinate = existing_radix_candidate_coordinate_v1(wire_ordinal)?;
            destination.extend_from_slice(
                &self
                    .existing_radix_candidate_ticket_v1(coordinate)?
                    .point_wire,
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExistingRadixPendingBlindingV1 {
    coordinate: ExistingRadixCandidateCoordinateV1,
    binding_digest: [u8; 32],
}

struct ExistingRadixSecretScalarWireV1([u8; 32]);

impl ExistingRadixSecretScalarWireV1 {
    fn new_v1(scalar: &Scalar) -> Self {
        Self(scalar.to_be_bytes())
    }

    fn as_ref_v1(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for ExistingRadixSecretScalarWireV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(&mut self.0);
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *bytes);
    }
}

/// Move-only sampled blinding. Production sees an opaque token: scalar access
/// and raw-point adoption are fixture-only until a private prepared-commitment
/// owner can consume the exact value vector and blinding internally.
#[must_use = "dropping this opaque token closes the source-only candidate assembly"]
pub(in crate::vega::zk_ams::mkhe) struct RnsNativeExistingRadixCandidateBlindingV1 {
    assembly_instance: u64,
    coordinate: ExistingRadixCandidateCoordinateV1,
    binding_digest: [u8; 32],
    chunk: ConfidentialSpoolChunkV1,
    scalar: ZeroizingT256ScalarCopyV1,
}

impl RnsNativeExistingRadixCandidateBlindingV1 {
    // The scalar is intentionally visible only to this module's fixtures.
    // `Scalar` is `Copy`, so exposing even `&Scalar` outside this boundary would
    // defeat the move-only zeroizing owner.
    #[cfg(test)]
    fn scalar_v1(&self) -> &Scalar {
        self.scalar.as_ref()
    }
}

struct ExistingRadixCandidateAssemblyLiveV1 {
    assembly_instance: u64,
    session: GlobalLookupCommitmentSessionV1<SourceOpeningCompleteStageV1>,
    blindings: ZeroizingT256ScalarVecV1,
    candidate_root_hash: Keccak256,
    blinding_root_hash: Keccak256,
    next_wire_ordinal: u32,
    pending: Option<ExistingRadixPendingBlindingV1>,
}

/// One-shot group/role/column assembly.  Every operation removes `live` before
/// validation, entropy, or inventory mutation, so no error is retryable.
#[must_use = "dropping this assembly closes the proof-session candidate inventory"]
pub(in crate::vega::zk_ams::mkhe) struct RnsNativeExistingRadixCandidateAssemblyV1 {
    live: Option<ExistingRadixCandidateAssemblyLiveV1>,
}

pub(super) struct ExistingRadixCandidateCompleteStageV1;

impl GlobalLookupCommitmentSessionV1<SourceOpeningCompleteStageV1> {
    pub(in crate::vega::zk_ams::mkhe) fn into_existing_radix_candidate_assembly_v1(
        self,
    ) -> Result<RnsNativeExistingRadixCandidateAssemblyV1, ZkAmsMkheErrorV1> {
        validate_existing_radix_candidate_ingress_v1(&self)?;
        let session_live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let mut blinding_root_hash = begin_existing_radix_blinding_root_v1(session_live)?;
        blinding_root_hash.update(&(EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32).to_be_bytes());
        Ok(RnsNativeExistingRadixCandidateAssemblyV1 {
            live: Some(ExistingRadixCandidateAssemblyLiveV1 {
                assembly_instance: next_existing_radix_assembly_instance_v1()?,
                session: self,
                blindings: ZeroizingT256ScalarVecV1::try_with_exact_capacity(
                    EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1,
                )
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                candidate_root_hash: begin_existing_radix_candidate_root_v1(),
                blinding_root_hash,
                next_wire_ordinal: 0,
                pending: None,
            }),
        })
    }
}

impl RnsNativeExistingRadixCandidateAssemblyV1 {
    pub(in crate::vega::zk_ams::mkhe) fn sample_next_blinding_v1(
        &mut self,
        group: usize,
        role: RnsNativeExistingRadixCandidateRoleV1,
        column: usize,
    ) -> Result<RnsNativeExistingRadixCandidateBlindingV1, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let coordinate = existing_radix_candidate_coordinate_v1(live.next_wire_ordinal)?;
        if usize::from(coordinate.group) != group
            || coordinate.role != role
            || usize::from(coordinate.column) != column
            || live.pending.is_some()
            || live.blindings.len()
                != usize::try_from(live.next_wire_ordinal)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let session_live = live
            .session
            .live
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let (chunk, scalar) =
            sample_blinding_v1(&mut session_live.entropy, coordinate.inventory_ordinal)?;
        let binding_digest =
            existing_radix_blinding_token_digest_v1(session_live, coordinate, scalar.as_ref())?;
        live.pending = Some(ExistingRadixPendingBlindingV1 {
            coordinate,
            binding_digest,
        });
        let assembly_instance = live.assembly_instance;
        self.live = Some(live);
        Ok(RnsNativeExistingRadixCandidateBlindingV1 {
            assembly_instance,
            coordinate,
            binding_digest,
            chunk,
            scalar,
        })
    }

    // Fixture-only until a private prepared-commitment owner can consume the
    // exact value vector and blinding without exposing a copyable scalar.
    #[cfg(test)]
    fn adopt_next_commitment_v1(
        &mut self,
        mut blinding: RnsNativeExistingRadixCandidateBlindingV1,
        point: &Point,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let pending = live
            .pending
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let session_live = live
            .session
            .live
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let scalar_wire = ExistingRadixSecretScalarWireV1::new_v1(blinding.scalar.as_ref());
        if live.assembly_instance != blinding.assembly_instance
            || pending.coordinate != blinding.coordinate
            || pending.binding_digest != blinding.binding_digest
            || blinding.binding_digest
                != existing_radix_blinding_token_digest_v1(
                    session_live,
                    blinding.coordinate,
                    blinding.scalar.as_ref(),
                )?
            || blinding.chunk.as_mut_slice_v1() != scalar_wire.as_ref_v1()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let point_wire = session_live
            .inventory
            .adopt_existing_radix_candidate_v1(blinding.coordinate, point)?;
        absorb_existing_radix_candidate_point_v1(
            &mut live.candidate_root_hash,
            blinding.coordinate,
            &point_wire,
        )?;
        absorb_existing_radix_blinding_v1(
            &mut live.blinding_root_hash,
            blinding.coordinate,
            scalar_wire.as_ref_v1(),
        );
        live.blindings.push(blinding.scalar.get());
        live.next_wire_ordinal = live
            .next_wire_ordinal
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.live = Some(live);
        Ok(())
    }

    pub(in crate::vega::zk_ams::mkhe) fn finish_v1(
        mut self,
    ) -> Result<RnsNativeExistingRadixCandidateOwnerV1, ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.next_wire_ordinal != EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32
            || live.pending.is_some()
            || live.blindings.len() != EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let candidate_root =
            require_nonzero_existing_radix_digest_v1(live.candidate_root_hash.finalize())?;
        let blinding_root =
            require_nonzero_existing_radix_digest_v1(live.blinding_root_hash.finalize())?;
        let mut session_live = live
            .session
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if session_live.inventory.existing_radix_candidate_root_v1()? != candidate_root
            || existing_radix_blinding_root_v1(&session_live, &live.blindings)? != blinding_root
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let next = commitment_coordinate_v1(EXISTING_RADIX_CANDIDATE_AFTER_INVENTORY_ORDINAL_V1)?;
        if next.purpose != GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop
            || next.purpose_ordinal != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        session_live.next_global_ordinal = next.global_ordinal;
        session_live.next_purpose = next.purpose;
        session_live.next_purpose_ordinal = next.purpose_ordinal;
        let session = GlobalLookupCommitmentSessionV1 {
            live: Some(session_live),
            state: PhantomData::<ExistingRadixCandidateCompleteStageV1>,
        };
        let session_live = session
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let owner_binding_digest =
            existing_radix_owner_binding_digest_v1(session_live, candidate_root, blinding_root)?;
        let owner = RnsNativeExistingRadixCandidateOwnerV1 {
            session,
            blindings: live.blindings,
            candidate_root,
            blinding_root,
            owner_binding_digest,
            append_permit: Some(ExistingRadixCandidateAppendPermitV1),
        };
        owner.validate_v1()?;
        Ok(owner)
    }

    #[cfg(test)]
    fn panic_after_take_for_test_v1(&mut self) {
        let _live = self.live.take().expect("live existing-radix assembly");
        panic!("intentional existing-radix candidate unwind");
    }
}

/// Complete move-only owner.  It retains the source-opening session, all
/// candidate blindings, and the physical inventory; it exposes no tuple split,
/// raw root getter, point getter, or blinding getter.
#[must_use = "dropping this owner closes the existing-radix candidate openings"]
pub(in crate::vega::zk_ams::mkhe) struct RnsNativeExistingRadixCandidateOwnerV1 {
    session: GlobalLookupCommitmentSessionV1<ExistingRadixCandidateCompleteStageV1>,
    blindings: ZeroizingT256ScalarVecV1,
    candidate_root: [u8; 32],
    blinding_root: [u8; 32],
    owner_binding_digest: [u8; 32],
    append_permit: Option<ExistingRadixCandidateAppendPermitV1>,
}

struct ExistingRadixCandidateAppendPermitV1;

/// Typed receipt for one exact append into a future `ZER1` transport owner.
#[must_use = "the future transport must retain this exact append receipt"]
pub(in crate::vega::zk_ams::mkhe) struct RnsNativeExistingRadixCandidateAppendReceiptV1 {
    destination_offset: usize,
    destination_len: usize,
    candidate_root: [u8; 32],
    owner_binding_digest: [u8; 32],
}

impl RnsNativeExistingRadixCandidateOwnerV1 {
    /// Append exactly the frozen 385,968-byte point section.  This deliberately
    /// does not fabricate the predecessor-bound `ZER1` header or residual.
    pub(in crate::vega::zk_ams::mkhe) fn append_candidate_section_v1(
        &mut self,
        destination: &mut Vec<u8>,
    ) -> Result<RnsNativeExistingRadixCandidateAppendReceiptV1, ZkAmsMkheErrorV1> {
        let _permit = self
            .append_permit
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        self.validate_v1()?;
        let destination_offset = destination.len();
        let session_live = self
            .session
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        session_live
            .inventory
            .append_existing_radix_candidate_wire_v1(destination)?;
        let end = destination_offset
            .checked_add(EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let emitted = destination
            .get(destination_offset..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if destination.len() != end
            || existing_radix_candidate_root_from_wire_v1(emitted)? != self.candidate_root
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(RnsNativeExistingRadixCandidateAppendReceiptV1 {
            destination_offset,
            destination_len: EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1,
            candidate_root: self.candidate_root,
            owner_binding_digest: self.owner_binding_digest,
        })
    }

    fn validate_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .session
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_existing_radix_candidate_complete_session_v1(live)?;
        if self.blindings.len() != EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1
            || live.inventory.existing_radix_candidate_root_v1()? != self.candidate_root
            || existing_radix_blinding_root_v1(live, &self.blindings)? != self.blinding_root
            || existing_radix_owner_binding_digest_v1(
                live,
                self.candidate_root,
                self.blinding_root,
            )? != self.owner_binding_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

fn validate_existing_radix_candidate_ingress_v1(
    session: &GlobalLookupCommitmentSessionV1<SourceOpeningCompleteStageV1>,
) -> Result<(), ZkAmsMkheErrorV1> {
    let live = session
        .live
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let source = live
        .inventory
        .source_binding
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if live.next_global_ordinal != EXISTING_RADIX_CANDIDATE_FIRST_INVENTORY_ORDINAL_V1
        || live.next_purpose != GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow
        || live.next_purpose_ordinal != 0
        || live.pending_source.is_some()
        || live.inventory.slots[..SOURCE_OPENING_GROUP_COUNT_V1]
            .iter()
            .any(Option::is_none)
        || live.inventory.slots[SOURCE_OPENING_GROUP_COUNT_V1..]
            .iter()
            .any(Option::is_some)
        || live
            .inventory
            .adopted_source_commitments_root_v1(source.source_opening_context_digest)?
            != source.commitments_root
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    validate_existing_radix_source_axes_v1(live)
}

fn validate_existing_radix_candidate_complete_session_v1(
    live: &GlobalLookupCommitmentSessionLiveV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_existing_radix_source_axes_v1(live)?;
    if live.next_global_ordinal != EXISTING_RADIX_CANDIDATE_AFTER_INVENTORY_ORDINAL_V1
        || live.next_purpose != GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop
        || live.next_purpose_ordinal != 0
        || live.pending_source.is_some()
        || live.inventory.slots[..EXISTING_RADIX_CANDIDATE_AFTER_INVENTORY_ORDINAL_V1 as usize]
            .iter()
            .any(Option::is_none)
        || live.inventory.slots[EXISTING_RADIX_CANDIDATE_AFTER_INVENTORY_ORDINAL_V1 as usize..]
            .iter()
            .any(Option::is_some)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn validate_existing_radix_source_axes_v1(
    live: &GlobalLookupCommitmentSessionLiveV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let source = live
        .inventory
        .source_binding
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if live.proof_session_context_digest == [0; 32]
        || source.proof_session_context_digest != live.proof_session_context_digest
        || [
            source.source_opening_context_digest,
            source.commitments_root,
            source.blinding_snapshot_root,
        ]
        .contains(&[0; 32])
        || live
            .inventory
            .adopted_source_commitments_root_v1(source.source_opening_context_digest)?
            != source.commitments_root
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn begin_existing_radix_blinding_root_v1(
    live: &GlobalLookupCommitmentSessionLiveV1,
) -> Result<Keccak256, ZkAmsMkheErrorV1> {
    validate_existing_radix_source_axes_v1(live)?;
    let source = live
        .inventory
        .source_binding
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let mut hash = Keccak256::new();
    hash.update(EXISTING_RADIX_BLINDING_ROOT_DOMAIN_V1);
    hash.update(&[EXISTING_RADIX_CANDIDATE_VERSION_V1]);
    hash.update(&live.proof_session_context_digest);
    hash.update(&source.source_opening_context_digest);
    hash.update(&source.commitments_root);
    hash.update(&source.blinding_snapshot_root);
    hash.update(&existing_radix_pre_z_manifest_digest_v1());
    Ok(hash)
}

fn absorb_existing_radix_blinding_v1(
    hash: &mut Keccak256,
    coordinate: ExistingRadixCandidateCoordinateV1,
    scalar_wire: &[u8; 32],
) {
    hash.update(&coordinate.wire_ordinal.to_be_bytes());
    hash.update(&coordinate.inventory_ordinal.to_be_bytes());
    hash.update(&coordinate.group.to_be_bytes());
    hash.update(&[coordinate.role as u8, coordinate.column]);
    hash.update(scalar_wire);
}

fn existing_radix_blinding_root_v1(
    live: &GlobalLookupCommitmentSessionLiveV1,
    blindings: &ZeroizingT256ScalarVecV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if blindings.len() != EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = begin_existing_radix_blinding_root_v1(live)?;
    hash.update(&(EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32).to_be_bytes());
    for (wire_ordinal, scalar) in blindings.as_slice().iter().enumerate() {
        let coordinate = existing_radix_candidate_coordinate_v1(
            u32::try_from(wire_ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        let scalar_wire = ExistingRadixSecretScalarWireV1::new_v1(scalar);
        absorb_existing_radix_blinding_v1(&mut hash, coordinate, scalar_wire.as_ref_v1());
    }
    require_nonzero_existing_radix_digest_v1(hash.finalize())
}

fn existing_radix_blinding_token_digest_v1(
    live: &GlobalLookupCommitmentSessionLiveV1,
    coordinate: ExistingRadixCandidateCoordinateV1,
    scalar: &Scalar,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_existing_radix_source_axes_v1(live)?;
    let source = live
        .inventory
        .source_binding
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let mut hash = Keccak256::new();
    hash.update(EXISTING_RADIX_BLINDING_TOKEN_DOMAIN_V1);
    hash.update(&[EXISTING_RADIX_CANDIDATE_VERSION_V1]);
    hash.update(&live.proof_session_context_digest);
    hash.update(&source.source_opening_context_digest);
    hash.update(&source.commitments_root);
    hash.update(&source.blinding_snapshot_root);
    hash.update(&coordinate.wire_ordinal.to_be_bytes());
    hash.update(&coordinate.inventory_ordinal.to_be_bytes());
    hash.update(&coordinate.group.to_be_bytes());
    hash.update(&[coordinate.role as u8, coordinate.column]);
    let scalar_wire = ExistingRadixSecretScalarWireV1::new_v1(scalar);
    hash.update(scalar_wire.as_ref_v1());
    require_nonzero_existing_radix_digest_v1(hash.finalize())
}

fn existing_radix_owner_binding_digest_v1(
    live: &GlobalLookupCommitmentSessionLiveV1,
    candidate_root: [u8; 32],
    blinding_root: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_existing_radix_source_axes_v1(live)?;
    let source = live
        .inventory
        .source_binding
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    let mut hash = Keccak256::new();
    hash.update(EXISTING_RADIX_OWNER_BINDING_DOMAIN_V1);
    hash.update(&[EXISTING_RADIX_CANDIDATE_VERSION_V1]);
    for digest in [
        live.proof_session_context_digest,
        source.source_opening_context_digest,
        source.commitments_root,
        source.blinding_snapshot_root,
        existing_radix_pre_z_manifest_digest_v1(),
        require_nonzero_existing_radix_digest_v1(candidate_root)?,
        require_nonzero_existing_radix_digest_v1(blinding_root)?,
    ] {
        hash.update(&digest);
    }
    hash.update(&(EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32).to_be_bytes());
    hash.update(&(EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1 as u32).to_be_bytes());
    hash.update(&EXISTING_RADIX_CANDIDATE_RETAINED_BLINDING_BYTES_V1.to_be_bytes());
    hash.update(&[
        EXISTING_RADIX_CANDIDATE_OWNER_MATERIALIZED_V1 as u8,
        LIVE_PHASE23_EXISTING_RADIX_SOURCE_INTEGRATED_V1 as u8,
        DIRECT_QUOTIENT_OPENING_OWNERS_INTEGRATED_V1 as u8,
        RESOURCE_EVIDENCE_ACCEPTED_V1 as u8,
        READINESS_ACCEPTED_V1 as u8,
        RELEASE_READY_V1 as u8,
        RELEASE_COMPLETE_V1 as u8,
    ]);
    require_nonzero_existing_radix_digest_v1(hash.finalize())
}

fn require_nonzero_existing_radix_digest_v1(
    digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

#[cfg(test)]
#[path = "existing_radix_candidate_v1_tests.rs"]
mod tests;
