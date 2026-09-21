//! Challenge-independent proof-session entropy and commitment inventory.
//!
//! This module allocates the complete current native40 identity inventory before
//! the first `Csrc` blinding is sampled. The root session adopts the 344 source
//! commitments; private child typestates may advance specifically reviewed
//! challenge-independent ranges. The actual source materializer's original
//! fallible entropy is consumed once at replay ingress; upstream correspondence
//! and source/proof qualification remain separate unavailable authorities.

#![allow(dead_code, reason = "later commitment purposes remain uninhabited")]
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
#[cfg(test)]
use core::convert::Infallible;
use core::marker::PhantomData;
use iroha_crypto::confidential_spool::ConfidentialSpoolChunkV1;

const COMMITMENT_SESSION_VERSION_V1: u8 = 1;
#[cfg(test)]
const TEST_ENTROPY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.test-entropy\0";
const COMMITMENT_BLINDING_BYTES_V1: u64 = 32;
const COMMITMENT_POINT_WIRE_BYTES_V1: u64 = 33;
const COMMITMENT_AUTHENTICATION_TAG_BYTES_V1: u64 = 16;

#[cfg(test)]
use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::{
    ALL_POINT_PURPOSES_V1, COMPARATOR_SIGNED_POINT_PURPOSES_V1, comparator_signed_coordinate_v1,
};
use crate::vega::zk_ams::mkhe::global_lookup_statement_v1::{
    GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1, GlobalLookupCommitmentCoordinateV1,
    GlobalLookupCommitmentPhaseV1, GlobalLookupCommitmentPurposeV1, commitment_coordinate_v1,
};

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
const COMPLETE_INVENTORY_MATERIALIZED_V1: bool = false;
const PROOF_ACCOUNTING_QUALIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const AUTHORITY_ACCEPTED_V1: bool = false;
const RSS_QUALIFIED_V1: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false;
const RELEASE_READY_V1: bool = false;
const RELEASE_COMPLETE_V1: bool = false;

const _: () = {
    assert!(GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 == 72_386);
    assert!(INVENTORY_BLINDING_BYTES_V1 == 2_316_352);
    assert!(INVENTORY_POINT_WIRE_BYTES_V1 == 2_388_738);
    assert!(INVENTORY_SEMANTIC_BYTES_V1 == 4_705_090);
    assert!(INVENTORY_AUTHENTICATION_TAG_BYTES_V1 == 1_158_176);
    assert!(PROJECTED_INVENTORY_FILE_BYTES_V1 == 5_863_266);
    assert!(PROJECTED_INVENTORY_WRITE_AND_SEAL_READ_BYTES_V1 == 11_726_532);
    assert!(INVENTORY_SKELETON_NEW_FILE_BYTES_V1 == 0);
    assert!(INVENTORY_SKELETON_NEW_IO_BYTES_V1 == 0);
    assert!(INVENTORY_SKELETON_NAMED_HEAP_BYTES_V1 > 0);
    assert!(!COMPLETE_INVENTORY_MATERIALIZED_V1);
    assert!(!PROOF_ACCOUNTING_QUALIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!AUTHORITY_ACCEPTED_V1);
    assert!(!RSS_QUALIFIED_V1);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!RELEASE_READY_V1);
    assert!(!RELEASE_COMPLETE_V1);
};

struct GlobalLookupCommitmentTicketV1 {
    coordinate: GlobalLookupCommitmentCoordinateV1,
    point_wire: [u8; 33],
}

// Minted only by the exact completed-source transition. No constructor,
// mutation or rebind operation is exposed to later commitment stages.
struct CompletedSourcePrefixV1 {
    source_record_digest: [u8; 32],
    proof_session_context_digest: [u8; 32],
    source_opening_context_digest: [u8; 32],
    commitments_root: [u8; 32],
    blinding_snapshot_root: [u8; 32],
}

struct GlobalLookupCommitmentInventorySkeletonV1 {
    slots: Vec<Option<GlobalLookupCommitmentTicketV1>>,
    source_prefix: Option<CompletedSourcePrefixV1>,
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
            source_prefix: None,
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

    fn seal_source_prefix_v1(
        &mut self,
        source_record_digest: [u8; 32],
        proof_session_context_digest: [u8; 32],
        source_opening_context_digest: [u8; 32],
        commitments_root: [u8; 32],
        blinding_snapshot_root: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.source_prefix.is_some()
            || source_record_digest == [0; 32]
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
        self.source_prefix = Some(CompletedSourcePrefixV1 {
            source_record_digest,
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
        #[cfg(test)]
        SOURCE_PREFIX_ROOT_VALIDATIONS_V1.with(|count| count.set(count.get() + 1));
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

enum GlobalLookupProofSessionEntropySourceV1<R> {
    Production {
        original_random: R,
        commitment_entropy_bytes: u64,
        q_mask_entropy_bytes: u64,
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

struct GlobalLookupCommitmentSessionLiveV1<R> {
    // Created once at the original entropy handoff and moved through every
    // consuming phase. Existing earlier allocations/work are not certified by
    // adding this ledger; later owners may not replace it with fresh counters.
    proof_resources:
        crate::vega::zk_ams::mkhe::rns_native_resource_budget::RnsNativeProofResourceBudgetV1,
    entropy: GlobalLookupProofSessionEntropySourceV1<R>,
    inventory: GlobalLookupCommitmentInventorySkeletonV1,
    proof_session_context_digest: [u8; 32],
    source_opening_context_digest: Option<[u8; 32]>,
    next_global_ordinal: u32,
    next_purpose: GlobalLookupCommitmentPurposeV1,
    next_purpose_ordinal: u32,
    pending_source: Option<GlobalLookupCommitmentCoordinateV1>,
}

pub(in crate::vega::zk_ams::mkhe) struct SourceOpeningEntropyStageV1;
pub(in crate::vega::zk_ams::mkhe) struct SourceOpeningCompleteStageV1;

/// Move-only typestated session. Taking `live` before every operation poisons
/// the owner on error and unwind.
pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupCommitmentSessionV1<R, State> {
    live: Option<GlobalLookupCommitmentSessionLiveV1<R>>,
    state: PhantomData<State>,
}

pub(in crate::vega::zk_ams::mkhe) type GlobalLookupProofSessionEntropySealV1<R> =
    GlobalLookupCommitmentSessionV1<R, SourceOpeningEntropyStageV1>;

#[cfg(test)]
impl GlobalLookupCommitmentSessionV1<Infallible, SourceOpeningEntropyStageV1> {
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
                proof_resources: Default::default(),
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
}

impl<R: crate::vega::MaskedRelaxedRandomSourceV1>
    GlobalLookupCommitmentSessionV1<R, SourceOpeningEntropyStageV1>
{
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
        source_record_digest: [u8; 32],
        source_opening_context_digest: [u8; 32],
        commitments_root: [u8; 32],
        blinding_snapshot_root: [u8; 32],
    ) -> Result<GlobalLookupCommitmentSessionV1<R, SourceOpeningCompleteStageV1>, ZkAmsMkheErrorV1>
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
        live.inventory.seal_source_prefix_v1(
            source_record_digest,
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

impl<R: crate::vega::MaskedRelaxedRandomSourceV1, State> GlobalLookupCommitmentSessionV1<R, State> {
    // Source authentication is immutable after the complete-source seal. Each
    // advancing stage separately enforces its exact cursor and pending state.
    pub(super) fn validate_completed_source_prefix_v1(
        &self,
        source_record_digest: [u8; 32],
        source_opening_context_digest: [u8; 32],
        commitments_root: [u8; 32],
        blinding_snapshot_root: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let live = self
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_completed_source_prefix_v1(
            live,
            source_record_digest,
            source_opening_context_digest,
            commitments_root,
            blinding_snapshot_root,
        )
    }
}

fn validate_completed_source_prefix_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    live: &GlobalLookupCommitmentSessionLiveV1<R>,
    source_record_digest: [u8; 32],
    source_opening_context_digest: [u8; 32],
    commitments_root: [u8; 32],
    blinding_snapshot_root: [u8; 32],
) -> Result<(), ZkAmsMkheErrorV1> {
    let prefix = live
        .inventory
        .source_prefix
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    if [
        source_record_digest,
        source_opening_context_digest,
        commitments_root,
        blinding_snapshot_root,
        live.proof_session_context_digest,
    ]
    .contains(&[0; 32])
        || prefix.source_record_digest != source_record_digest
        || prefix.proof_session_context_digest != live.proof_session_context_digest
        || prefix.source_opening_context_digest != source_opening_context_digest
        || live.source_opening_context_digest != Some(source_opening_context_digest)
        || prefix.commitments_root != commitments_root
        || prefix.blinding_snapshot_root != blinding_snapshot_root
        || live
            .inventory
            .adopted_source_commitments_root_v1(source_opening_context_digest)?
            != commitments_root
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn sample_blinding_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    entropy: &mut GlobalLookupProofSessionEntropySourceV1<R>,
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

fn fill_entropy_v1<R: crate::vega::MaskedRelaxedRandomSourceV1>(
    entropy: &mut GlobalLookupProofSessionEntropySourceV1<R>,
    purpose_ordinal: u32,
    attempt: u16,
    destination: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    if purpose_ordinal >= GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1
        || usize::from(attempt) >= MAX_RANDOM_REJECTION_ATTEMPTS_V1
        || destination.len() != SOURCE_OPENING_BLINDING_SLOT_BYTES_V1 as usize
    {
        destination.fill(0);
        return Err(ZkAmsMkheErrorV1::RandomUnavailable);
    }
    match entropy {
        GlobalLookupProofSessionEntropySourceV1::Production {
            original_random,
            commitment_entropy_bytes,
            ..
        } => {
            let Some(next) = commitment_entropy_bytes
                .checked_add(SOURCE_OPENING_BLINDING_SLOT_BYTES_V1)
                .filter(|next| *next <= MAX_COMMITMENT_ENTROPY_BYTES_V1)
            else {
                destination.fill(0);
                return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
            };
            // Charge the exact requested bytes before a partial failure/unwind.
            // The enclosing consuming session owns poisoning and destruction.
            *commitment_entropy_bytes = next;
            original_random.fill_bytes(destination).map_err(|_| {
                destination.fill(0);
                ZkAmsMkheErrorV1::RandomUnavailable
            })
        }
        #[cfg(test)]
        GlobalLookupProofSessionEntropySourceV1::TestOnly(test) => {
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

#[path = "commitment_session_v1/prepared_opening_tail_v1.rs"]
mod prepared_opening_tail_v1;
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) use prepared_opening_tail_v1::PreparedPlaneOpeningTailV1;

#[path = "commitment_session_v1/retained_source_session_v1.rs"]
mod retained_source_session_v1;
pub(super) use retained_source_session_v1::RetainedSourceSessionV1;

#[path = "commitment_session_v1/existing_radix_candidate_v1.rs"]
mod existing_radix_candidate_v1;
pub(in crate::vega::zk_ams::mkhe) use existing_radix_candidate_v1::{
    RnsNativeExistingRadixCandidateAppendReceiptV1, RnsNativeExistingRadixCandidateAssemblyV1,
    RnsNativeExistingRadixCandidateBlindingV1, RnsNativeExistingRadixCandidateOwnerV1,
    RnsNativeExistingRadixCandidateRoleV1,
};

#[cfg(test)]
#[path = "commitment_session_v1_tests.rs"]
mod tests;

#[cfg(test)]
impl crate::vega::MaskedRelaxedRandomSourceV1 for Infallible {
    fn fill_bytes(
        &mut self,
        _destination: &mut [u8],
    ) -> Result<(), crate::vega::MaskedRelaxedRandomErrorV1> {
        match *self {}
    }
}

const MAX_COMMITMENT_ENTROPY_BYTES_V1: u64 = GLOBAL_LOOKUP_COMMITMENT_INVENTORY_CAPACITY_V1 as u64
    * MAX_RANDOM_REJECTION_ATTEMPTS_V1 as u64
    * SOURCE_OPENING_BLINDING_SLOT_BYTES_V1;
const _: () = assert!(MAX_COMMITMENT_ENTROPY_BYTES_V1 == 296_493_056);

#[path = "commitment_session_v1/original_entropy_handoff_v1.rs"]
mod original_entropy_handoff_v1;

// Count actual source-prefix root traversals, only for replay-work regression tests.
#[cfg(test)]
thread_local! {
    static SOURCE_PREFIX_ROOT_VALIDATIONS_V1: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[path = "commitment_session_v1/q_mask_first_block_v1.rs"]
mod q_mask_first_block_v1;
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) use q_mask_first_block_v1::{
    QMaskComplementOpeningsV1,
    QMaskSBlockAdmissionV1, CompleteQMaskSOpeningsV1, QMaskSOpeningStreamV1, QMaskSErrorV1, QMaskFirstBlockMemoryV1, SampledQMaskSBlockV1,
};
