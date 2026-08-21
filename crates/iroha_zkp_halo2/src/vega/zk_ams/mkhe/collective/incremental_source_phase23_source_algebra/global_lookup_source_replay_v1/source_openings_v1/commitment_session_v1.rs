//! Challenge-independent proof-session entropy and commitment inventory.
//!
//! This module allocates the complete current dual-`z` identity inventory before
//! the first `Csrc` blinding is sampled.  Only the already-existing 344 source
//! commitments can be adopted in this slice.  Every later purpose transition is
//! deliberately absent, and production still cannot construct the entropy source.

#![allow(dead_code, reason = "later commitment purposes remain uninhabited")]
#![cfg_attr(
    not(test),
    allow(
        unused_variables,
        reason = "production proof-session entropy is intentionally uninhabited"
    )
)]

use super::super::super::super::super::super::super::MAX_RANDOM_REJECTION_ATTEMPTS_V1;
use super::{
    SOURCE_OPENING_BLINDING_SLOT_BYTES_V1, SOURCE_OPENING_COMMITMENT_DOMAIN_V1,
    SOURCE_OPENING_GROUP_COUNT_V1, SOURCE_OPENING_VERSION_V1,
    ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZkAmsMkheErrorV1,
    exact_source_opening_mapping_digest_v1, map_leaf_error_v1, source_opening_group_coordinate_v1,
};
use crate::vega::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::ZeroizingT256ScalarCopyV1, sponge::Keccak256,
};
use core::{convert::Infallible, marker::PhantomData};
use iroha_confidential_spool::ConfidentialSpoolChunkV1;

const COMMITMENT_SESSION_VERSION_V1: u8 = 1;
const TEST_ENTROPY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.test-entropy\0";
const COMMITMENT_BLINDING_BYTES_V1: u64 = 32;
const COMMITMENT_POINT_WIRE_BYTES_V1: u64 = 33;
const COMMITMENT_AUTHENTICATION_TAG_BYTES_V1: u64 = 16;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlobalLookupCommitmentPhaseV1 {
    ChallengeIndependent = 1,
    RadixPostZ = 2,
    GlobalLookupPostZ = 3,
    PostDeltaResidual = 4,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlobalLookupCommitmentPurposeV1 {
    Source = 1,
    ExistingDifferenceLow = 2,
    ExistingSumLow = 3,
    ComparatorDifferenceTop = 4,
    ComparatorSumTop = 5,
    ComparatorDifferenceDigit = 6,
    ComparatorBorrow = 7,
    ComparatorMixedTop = 8,
    SmallSigned = 9,
    SmallNegativeMagnitude = 10,
    QMaskDigit = 11,
    QMaskComplementDigit = 12,
    Multiplicity = 13,
    SumcheckMask = 14,
    RadixDifferenceInverse = 15,
    RadixSumInverse = 16,
    GlobalDifferenceInverse = 17,
    GlobalSumInverse = 18,
    ComparatorDifferenceInverse = 19,
    SmallSignedInverse = 20,
    SmallNegativeInverse = 21,
    QMaskDigitInverse = 22,
    QMaskComplementInverse = 23,
    ResidualQ3 = 24,
    ResidualQ5 = 25,
    ResidualQ8 = 26,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GlobalLookupCommitmentRoleV1 {
    phase: GlobalLookupCommitmentPhaseV1,
    purpose: GlobalLookupCommitmentPurposeV1,
    first_ordinal: u32,
    count: u32,
}

const fn role_v1(
    phase: GlobalLookupCommitmentPhaseV1,
    purpose: GlobalLookupCommitmentPurposeV1,
    first_ordinal: u32,
    count: u32,
) -> GlobalLookupCommitmentRoleV1 {
    GlobalLookupCommitmentRoleV1 {
        phase,
        purpose,
        first_ordinal,
        count,
    }
}

const CHALLENGE_INDEPENDENT_COMMITMENTS_V1: u32 = 39_338;
const RADIX_POST_Z_COMMITMENTS_V1: u32 = 11_696;
const GLOBAL_LOOKUP_POST_Z_COMMITMENTS_V1: u32 = 31_768;
const POST_DELTA_RESIDUAL_COMMITMENTS_V1: u32 = 3;
const GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1: u32 = 82_805;
const CONDITIONAL_UNIFIED_Z_COMMITMENTS_V1: u32 = 71_109;
const VECTOR_ARITHMETIC_PRE_Z_ALIASES_V1: u32 = 9_288;
const VECTOR_ARITHMETIC_POST_DELTA_ALIASES_V1: u32 = 3;
const VECTOR_ARITHMETIC_ALIASES_V1: u32 = 9_291;

#[rustfmt::skip]
const GLOBAL_LOOKUP_COMMITMENT_ROLES_V1: [GlobalLookupCommitmentRoleV1; 26] = [
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::Source, 0, 344),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow, 344, 5_848),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::ExistingSumLow, 6_192, 5_848),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop, 12_040, 344),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::ComparatorSumTop, 12_384, 344),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit, 12_728, 5_848),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::ComparatorBorrow, 18_576, 6_192),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::ComparatorMixedTop, 24_768, 344),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::SmallSigned, 25_112, 1_032),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude, 26_144, 1_032),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::QMaskDigit, 27_176, 6_080),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::QMaskComplementDigit, 33_256, 6_080),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::Multiplicity, 39_336, 1),
    role_v1(GlobalLookupCommitmentPhaseV1::ChallengeIndependent, GlobalLookupCommitmentPurposeV1::SumcheckMask, 39_337, 1),
    role_v1(GlobalLookupCommitmentPhaseV1::RadixPostZ, GlobalLookupCommitmentPurposeV1::RadixDifferenceInverse, 39_338, 5_848),
    role_v1(GlobalLookupCommitmentPhaseV1::RadixPostZ, GlobalLookupCommitmentPurposeV1::RadixSumInverse, 45_186, 5_848),
    role_v1(GlobalLookupCommitmentPhaseV1::GlobalLookupPostZ, GlobalLookupCommitmentPurposeV1::GlobalDifferenceInverse, 51_034, 5_848),
    role_v1(GlobalLookupCommitmentPhaseV1::GlobalLookupPostZ, GlobalLookupCommitmentPurposeV1::GlobalSumInverse, 56_882, 5_848),
    role_v1(GlobalLookupCommitmentPhaseV1::GlobalLookupPostZ, GlobalLookupCommitmentPurposeV1::ComparatorDifferenceInverse, 62_730, 5_848),
    role_v1(GlobalLookupCommitmentPhaseV1::GlobalLookupPostZ, GlobalLookupCommitmentPurposeV1::SmallSignedInverse, 68_578, 1_032),
    role_v1(GlobalLookupCommitmentPhaseV1::GlobalLookupPostZ, GlobalLookupCommitmentPurposeV1::SmallNegativeInverse, 69_610, 1_032),
    role_v1(GlobalLookupCommitmentPhaseV1::GlobalLookupPostZ, GlobalLookupCommitmentPurposeV1::QMaskDigitInverse, 70_642, 6_080),
    role_v1(GlobalLookupCommitmentPhaseV1::GlobalLookupPostZ, GlobalLookupCommitmentPurposeV1::QMaskComplementInverse, 76_722, 6_080),
    role_v1(GlobalLookupCommitmentPhaseV1::PostDeltaResidual, GlobalLookupCommitmentPurposeV1::ResidualQ3, 82_802, 1),
    role_v1(GlobalLookupCommitmentPhaseV1::PostDeltaResidual, GlobalLookupCommitmentPurposeV1::ResidualQ5, 82_803, 1),
    role_v1(GlobalLookupCommitmentPhaseV1::PostDeltaResidual, GlobalLookupCommitmentPurposeV1::ResidualQ8, 82_804, 1),
];

const INVENTORY_BLINDING_BYTES_V1: u64 =
    GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 as u64 * COMMITMENT_BLINDING_BYTES_V1;
const INVENTORY_POINT_WIRE_BYTES_V1: u64 =
    GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 as u64 * COMMITMENT_POINT_WIRE_BYTES_V1;
const INVENTORY_SEMANTIC_BYTES_V1: u64 =
    INVENTORY_BLINDING_BYTES_V1 + INVENTORY_POINT_WIRE_BYTES_V1;
const INVENTORY_AUTHENTICATION_TAG_BYTES_V1: u64 =
    GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 as u64 * COMMITMENT_AUTHENTICATION_TAG_BYTES_V1;
const PROJECTED_INVENTORY_FILE_BYTES_V1: u64 =
    INVENTORY_SEMANTIC_BYTES_V1 + INVENTORY_AUTHENTICATION_TAG_BYTES_V1;
const PROJECTED_INVENTORY_WRITE_AND_SEAL_READ_BYTES_V1: u64 = 2 * PROJECTED_INVENTORY_FILE_BYTES_V1;
const INVENTORY_SKELETON_NEW_FILE_BYTES_V1: u64 = 0;
const INVENTORY_SKELETON_NEW_IO_BYTES_V1: u64 = 0;
const INVENTORY_SKELETON_NAMED_HEAP_BYTES_V1: usize = GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1
    as usize
    * core::mem::size_of::<Option<GlobalLookupCommitmentTicketV1>>();
const DUAL_Z_PROOF_INVENTORY_CAP_ADMISSIBLE_V1: bool = false;
const UNIFIED_Z_INVENTORY_INHABITED_V1: bool = false;
const TRANSCRIPT_Z_ALIAS_INSTANTIATED_V1: bool = false;
const PROOF_ACCOUNTING_QUALIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const AUTHORITY_ACCEPTED_V1: bool = false;
const RSS_QUALIFIED_V1: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false;
const RELEASE_READY_V1: bool = false;
const RELEASE_COMPLETE_V1: bool = false;

const _: () = {
    assert!(CHALLENGE_INDEPENDENT_COMMITMENTS_V1 == 39_338);
    assert!(RADIX_POST_Z_COMMITMENTS_V1 == 2 * 5_848);
    assert!(GLOBAL_LOOKUP_POST_Z_COMMITMENTS_V1 == 31_768);
    assert!(POST_DELTA_RESIDUAL_COMMITMENTS_V1 == 3);
    assert!(GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 == 82_805);
    assert!(
        GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1
            == CHALLENGE_INDEPENDENT_COMMITMENTS_V1
                + RADIX_POST_Z_COMMITMENTS_V1
                + GLOBAL_LOOKUP_POST_Z_COMMITMENTS_V1
                + POST_DELTA_RESIDUAL_COMMITMENTS_V1
    );
    assert!(
        CONDITIONAL_UNIFIED_Z_COMMITMENTS_V1
            == GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 - RADIX_POST_Z_COMMITMENTS_V1
    );
    assert!(VECTOR_ARITHMETIC_ALIASES_V1 == 9_291);
    assert!(
        VECTOR_ARITHMETIC_ALIASES_V1
            == VECTOR_ARITHMETIC_PRE_Z_ALIASES_V1 + VECTOR_ARITHMETIC_POST_DELTA_ALIASES_V1
    );
    assert!(INVENTORY_BLINDING_BYTES_V1 == 2_649_760);
    assert!(INVENTORY_POINT_WIRE_BYTES_V1 == 2_732_565);
    assert!(INVENTORY_SEMANTIC_BYTES_V1 == 5_382_325);
    assert!(INVENTORY_AUTHENTICATION_TAG_BYTES_V1 == 1_324_880);
    assert!(PROJECTED_INVENTORY_FILE_BYTES_V1 == 6_707_205);
    assert!(PROJECTED_INVENTORY_WRITE_AND_SEAL_READ_BYTES_V1 == 13_414_410);
    assert!(INVENTORY_SKELETON_NEW_FILE_BYTES_V1 == 0);
    assert!(INVENTORY_SKELETON_NEW_IO_BYTES_V1 == 0);
    assert!(INVENTORY_SKELETON_NAMED_HEAP_BYTES_V1 > 0);
    assert!(!DUAL_Z_PROOF_INVENTORY_CAP_ADMISSIBLE_V1);
    assert!(!UNIFIED_Z_INVENTORY_INHABITED_V1);
    assert!(!TRANSCRIPT_Z_ALIAS_INSTANTIATED_V1);
    assert!(!PROOF_ACCOUNTING_QUALIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!AUTHORITY_ACCEPTED_V1);
    assert!(!RSS_QUALIFIED_V1);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!RELEASE_READY_V1);
    assert!(!RELEASE_COMPLETE_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GlobalLookupCommitmentCoordinateV1 {
    global_ordinal: u32,
    phase: GlobalLookupCommitmentPhaseV1,
    purpose: GlobalLookupCommitmentPurposeV1,
    purpose_ordinal: u32,
}

fn commitment_coordinate_v1(
    global_ordinal: u32,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    for role in GLOBAL_LOOKUP_COMMITMENT_ROLES_V1 {
        let end = role
            .first_ordinal
            .checked_add(role.count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if global_ordinal >= role.first_ordinal && global_ordinal < end {
            return Ok(GlobalLookupCommitmentCoordinateV1 {
                global_ordinal,
                phase: role.phase,
                purpose: role.purpose,
                purpose_ordinal: global_ordinal - role.first_ordinal,
            });
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn vector_arithmetic_alias_v1(
    vector_ordinal: u32,
) -> Result<GlobalLookupCommitmentCoordinateV1, ZkAmsMkheErrorV1> {
    let inventory_ordinal = match vector_ordinal {
        0..=343 => 12_040 + vector_ordinal,
        344..=687 => 12_384 + vector_ordinal - 344,
        688..=6_879 => 18_576 + vector_ordinal - 688,
        6_880..=7_223 => 24_768 + vector_ordinal - 6_880,
        7_224..=8_255 => 25_112 + vector_ordinal - 7_224,
        8_256..=9_287 => 26_144 + vector_ordinal - 8_256,
        9_288 => 82_802,
        9_289 => 82_803,
        9_290 => 82_804,
        _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
    };
    commitment_coordinate_v1(inventory_ordinal)
}

struct GlobalLookupCommitmentTicketV1 {
    coordinate: GlobalLookupCommitmentCoordinateV1,
    point_wire: [u8; 33],
}

struct SourceOpeningInventoryBindingV1 {
    proof_session_context_digest: [u8; 32],
    source_opening_context_digest: [u8; 32],
    commitments_root: [u8; 32],
    blinding_snapshot_root: [u8; 32],
}

struct GlobalLookupCommitmentInventorySkeletonV1 {
    slots: Vec<Option<GlobalLookupCommitmentTicketV1>>,
    source_binding: Option<SourceOpeningInventoryBindingV1>,
}

impl GlobalLookupCommitmentInventorySkeletonV1 {
    fn new_v1() -> Result<Self, ZkAmsMkheErrorV1> {
        let exact_capacity = usize::try_from(GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(exact_capacity)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        slots.resize_with(exact_capacity, || None);
        if slots.len() != exact_capacity || slots.capacity() != exact_capacity {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(Self {
            slots,
            source_binding: None,
        })
    }

    fn adopt_source_v1(
        &mut self,
        coordinate: GlobalLookupCommitmentCoordinateV1,
        point: &Point,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if coordinate.purpose != GlobalLookupCommitmentPurposeV1::Source
            || coordinate.phase != GlobalLookupCommitmentPhaseV1::ChallengeIndependent
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let slot = self
            .slots
            .get_mut(
                usize::try_from(coordinate.global_ordinal)
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
            coordinate,
            point_wire,
        });
        Ok(())
    }

    fn bind_source_roots_v1(
        &mut self,
        proof_session_context_digest: [u8; 32],
        source_opening_context_digest: [u8; 32],
        commitments_root: [u8; 32],
        blinding_snapshot_root: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.source_binding.is_some()
            || proof_session_context_digest == [0; 32]
            || source_opening_context_digest == [0; 32]
            || commitments_root == [0; 32]
            || blinding_snapshot_root == [0; 32]
            || self.adopted_source_commitments_root_v1(source_opening_context_digest)?
                != commitments_root
            || self.slots[..SOURCE_OPENING_GROUP_COUNT_V1]
                .iter()
                .any(Option::is_none)
            || self.slots[SOURCE_OPENING_GROUP_COUNT_V1..]
                .iter()
                .any(Option::is_some)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        self.source_binding = Some(SourceOpeningInventoryBindingV1 {
            proof_session_context_digest,
            source_opening_context_digest,
            commitments_root,
            blinding_snapshot_root,
        });
        Ok(())
    }

    fn adopted_source_commitments_root_v1(
        &self,
        source_opening_context_digest: [u8; 32],
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if source_opening_context_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut hash = Keccak256::new();
        hash.update(SOURCE_OPENING_COMMITMENT_DOMAIN_V1);
        hash.update(&[SOURCE_OPENING_VERSION_V1]);
        hash.update(&source_opening_context_digest);
        hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
        hash.update(&exact_source_opening_mapping_digest_v1()?);
        hash.update(&(SOURCE_OPENING_GROUP_COUNT_V1 as u16).to_be_bytes());
        for ordinal in 0..SOURCE_OPENING_GROUP_COUNT_V1 {
            let ticket = self.slots[ordinal]
                .as_ref()
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            if ticket.coordinate.global_ordinal != ordinal as u32
                || ticket.coordinate.purpose != GlobalLookupCommitmentPurposeV1::Source
                || ticket.coordinate.purpose_ordinal != ordinal as u32
            {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            let coordinate = source_opening_group_coordinate_v1(ordinal)?;
            hash.update(&coordinate.ordinal.to_be_bytes());
            hash.update(&coordinate.record.to_be_bytes());
            hash.update(&[coordinate.group]);
            hash.update(&ticket.point_wire);
        }
        let digest = hash.finalize();
        (digest != [0; 32])
            .then_some(digest)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    }
}

enum GlobalLookupProofSessionEntropySourceV1 {
    Production {
        proof_session_entropy: Infallible,
    },
    #[cfg(test)]
    TestOnly(DeterministicProofSessionEntropyV1),
}

#[cfg(test)]
pub(super) enum TestEntropyFaultV1 {
    None,
    ErrorAt(u32),
    ZeroAt(u32),
    PanicAt(u32),
}

#[cfg(test)]
struct DeterministicProofSessionEntropyV1 {
    seed: [u8; 32],
    fault: TestEntropyFaultV1,
}

#[cfg(test)]
impl Drop for DeterministicProofSessionEntropyV1 {
    fn drop(&mut self) {
        let seed = core::hint::black_box(&mut self.seed);
        seed.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *seed);
    }
}

struct GlobalLookupCommitmentSessionLiveV1 {
    entropy: GlobalLookupProofSessionEntropySourceV1,
    inventory: GlobalLookupCommitmentInventorySkeletonV1,
    proof_session_context_digest: [u8; 32],
    source_opening_context_digest: Option<[u8; 32]>,
    next_global_ordinal: u32,
    next_purpose: GlobalLookupCommitmentPurposeV1,
    next_purpose_ordinal: u32,
    pending_source: Option<GlobalLookupCommitmentCoordinateV1>,
}

pub(in crate::vega::zk_ams::mkhe) struct SourceOpeningEntropyStageV1;
pub(super) struct SourceOpeningCompleteStageV1;

/// Move-only typestated session. Taking `live` before every operation poisons
/// the owner on error and unwind.
pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupCommitmentSessionV1<State> {
    live: Option<GlobalLookupCommitmentSessionLiveV1>,
    state: PhantomData<State>,
}

pub(in crate::vega::zk_ams::mkhe) type GlobalLookupProofSessionEntropySealV1 =
    GlobalLookupCommitmentSessionV1<SourceOpeningEntropyStageV1>;

impl GlobalLookupCommitmentSessionV1<SourceOpeningEntropyStageV1> {
    #[cfg(test)]
    pub(in crate::vega::zk_ams::mkhe) fn test_only_v1(
        proof_session_context_digest: [u8; 32],
        seed: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::test_only_with_fault_v1(proof_session_context_digest, seed, TestEntropyFaultV1::None)
    }

    #[cfg(test)]
    pub(super) fn test_only_with_fault_v1(
        proof_session_context_digest: [u8; 32],
        seed: [u8; 32],
        fault: TestEntropyFaultV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let entropy = DeterministicProofSessionEntropyV1 { seed, fault };
        if proof_session_context_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let inventory = GlobalLookupCommitmentInventorySkeletonV1::new_v1()?;
        Ok(Self {
            live: Some(GlobalLookupCommitmentSessionLiveV1 {
                entropy: GlobalLookupProofSessionEntropySourceV1::TestOnly(entropy),
                inventory,
                proof_session_context_digest,
                source_opening_context_digest: None,
                next_global_ordinal: 0,
                next_purpose: GlobalLookupCommitmentPurposeV1::Source,
                next_purpose_ordinal: 0,
                pending_source: None,
            }),
            state: PhantomData,
        })
    }

    pub(super) fn bind_source_opening_context_v1(
        &mut self,
        source_opening_context_digest: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if source_opening_context_digest == [0; 32]
            || live.source_opening_context_digest.is_some()
            || live.next_global_ordinal != 0
            || live.pending_source.is_some()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        live.source_opening_context_digest = Some(source_opening_context_digest);
        self.live = Some(live);
        Ok(())
    }

    pub(super) fn sample_source_blinding_v1(
        &mut self,
        purpose_ordinal: u32,
    ) -> Result<(ConfidentialSpoolChunkV1, ZeroizingT256ScalarCopyV1), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let coordinate = commitment_coordinate_v1(live.next_global_ordinal)?;
        if live.source_opening_context_digest.is_none()
            || live.pending_source.is_some()
            || live.next_purpose != GlobalLookupCommitmentPurposeV1::Source
            || live.next_purpose_ordinal != purpose_ordinal
            || coordinate.purpose != GlobalLookupCommitmentPurposeV1::Source
            || coordinate.purpose_ordinal != purpose_ordinal
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let result = sample_blinding_v1(&mut live.entropy, purpose_ordinal)?;
        live.pending_source = Some(coordinate);
        self.live = Some(live);
        Ok(result)
    }

    pub(super) fn adopt_source_commitment_v1(
        &mut self,
        purpose_ordinal: u32,
        point: &Point,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let coordinate = live
            .pending_source
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if coordinate.global_ordinal != live.next_global_ordinal
            || coordinate.purpose_ordinal != purpose_ordinal
            || purpose_ordinal != live.next_purpose_ordinal
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        live.inventory.adopt_source_v1(coordinate, point)?;
        live.next_global_ordinal = live
            .next_global_ordinal
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let next = commitment_coordinate_v1(live.next_global_ordinal)?;
        live.next_purpose = next.purpose;
        live.next_purpose_ordinal = next.purpose_ordinal;
        self.live = Some(live);
        Ok(())
    }

    pub(super) fn complete_source_opening_v1(
        mut self,
        source_opening_context_digest: [u8; 32],
        commitments_root: [u8; 32],
        blinding_snapshot_root: [u8; 32],
    ) -> Result<GlobalLookupCommitmentSessionV1<SourceOpeningCompleteStageV1>, ZkAmsMkheErrorV1>
    {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.source_opening_context_digest != Some(source_opening_context_digest)
            || live.next_global_ordinal != SOURCE_OPENING_GROUP_COUNT_V1 as u32
            || live.next_purpose != GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow
            || live.next_purpose_ordinal != 0
            || live.pending_source.is_some()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        live.inventory.bind_source_roots_v1(
            live.proof_session_context_digest,
            source_opening_context_digest,
            commitments_root,
            blinding_snapshot_root,
        )?;
        Ok(GlobalLookupCommitmentSessionV1 {
            live: Some(live),
            state: PhantomData,
        })
    }
}

impl GlobalLookupCommitmentSessionV1<SourceOpeningCompleteStageV1> {
    pub(super) fn validate_source_opening_v1(
        &self,
        source_opening_context_digest: [u8; 32],
        commitments_root: [u8; 32],
        blinding_snapshot_root: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let binding = live
            .inventory
            .source_binding
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if live.next_global_ordinal != SOURCE_OPENING_GROUP_COUNT_V1 as u32
            || live.next_purpose != GlobalLookupCommitmentPurposeV1::ExistingDifferenceLow
            || live.next_purpose_ordinal != 0
            || live.pending_source.is_some()
            || binding.proof_session_context_digest != live.proof_session_context_digest
            || binding.source_opening_context_digest != source_opening_context_digest
            || binding.commitments_root != commitments_root
            || binding.blinding_snapshot_root != blinding_snapshot_root
            || live
                .inventory
                .adopted_source_commitments_root_v1(source_opening_context_digest)?
                != commitments_root
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

fn sample_blinding_v1(
    entropy: &mut GlobalLookupProofSessionEntropySourceV1,
    purpose_ordinal: u32,
) -> Result<(ConfidentialSpoolChunkV1, ZeroizingT256ScalarCopyV1), ZkAmsMkheErrorV1> {
    for attempt in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut chunk =
            ConfidentialSpoolChunkV1::new_zeroed_v1(SOURCE_OPENING_BLINDING_SLOT_BYTES_V1)
                .map_err(map_leaf_error_v1)?;
        fill_entropy_v1(
            entropy,
            purpose_ordinal,
            attempt as u16,
            chunk.as_mut_slice_v1(),
        )?;
        let encoded: &[u8; 32] = chunk
            .as_slice_v1()
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if let Ok(mut scalar) = Scalar::from_be_bytes_exact_ref(encoded) {
            let scalar = ZeroizingT256ScalarCopyV1::take(&mut scalar);
            if !scalar.get().is_zero() {
                return Ok((chunk, scalar));
            }
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn fill_entropy_v1(
    entropy: &mut GlobalLookupProofSessionEntropySourceV1,
    purpose_ordinal: u32,
    attempt: u16,
    destination: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    match entropy {
        GlobalLookupProofSessionEntropySourceV1::Production {
            proof_session_entropy,
        } => match *proof_session_entropy {},
        #[cfg(test)]
        GlobalLookupProofSessionEntropySourceV1::TestOnly(test) => {
            if destination.len() != SOURCE_OPENING_BLINDING_SLOT_BYTES_V1 as usize {
                return Err(ZkAmsMkheErrorV1::RandomUnavailable);
            }
            match &test.fault {
                TestEntropyFaultV1::ErrorAt(at) if *at == purpose_ordinal => {
                    return Err(ZkAmsMkheErrorV1::RandomUnavailable);
                }
                TestEntropyFaultV1::ZeroAt(at) if *at == purpose_ordinal => destination.fill(0),
                TestEntropyFaultV1::PanicAt(at) if *at == purpose_ordinal => {
                    panic!("intentional proof-session entropy unwind");
                }
                _ => {
                    let destination: &mut [u8; 32] = destination
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
                    let group = u16::try_from(purpose_ordinal)
                        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                    let mut hash = Keccak256::new();
                    hash.update(TEST_ENTROPY_DOMAIN_V1);
                    hash.update(&test.seed);
                    hash.update(&group.to_be_bytes());
                    hash.update(&attempt.to_be_bytes());
                    hash.finalize_into(destination);
                }
            }
            Ok(())
        }
    }
}

#[path = "commitment_session_v1/global_z_rendezvous_v2.rs"]
mod global_z_rendezvous_v2;

#[cfg(test)]
#[path = "commitment_session_v1_tests.rs"]
mod tests;
