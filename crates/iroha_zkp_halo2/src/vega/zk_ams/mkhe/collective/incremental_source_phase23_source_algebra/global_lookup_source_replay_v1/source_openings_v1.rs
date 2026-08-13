//! Purpose-bound source openings for the Phase-23 global lookup. The first
//! authenticated pass creates 344 `Csrc` commitments in `j=256*b+i` order and
//! binds packing coordinate `k=64*i+b`, retaining only a confidential blinding
//! spool, public points, and the upstream owners. A private coordinate-free second
//! pass can later form two weighted columns. No plaintext mirror or `Cpack`
//! exists, and every proof, authority, receipt, RSS, and release gate is false.

#![allow(
    dead_code,
    reason = "the production entropy and reopen seals are uninhabited"
)]

use core::convert::Infallible;
use std::path::PathBuf;

use iroha_confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};

use crate::{
    generalized_bulletproof::{ProofSuite, SecretMultiexpBuilder},
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, ZeroizingT256ScalarCopyV1,
            ZeroizingT256ScalarVecV1, ZkAmsT256BulletproofSuiteV1,
            zk_ams_t256_bulletproof_generator_basis_digest_v1,
        },
        sponge::Keccak256,
    },
};

use super::super::super::super::super::super::{
    MAX_RANDOM_REJECTION_ATTEMPTS_V1, ZkAmsMkheErrorV1,
    global_lookup_statement_v1::global_lookup_topology_digest_v1,
};
use super::super::super::{
    MaskedRelaxedRandomSourceV1, PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1,
    PHASE23_MAIN_BLOCK_BYTES_V1, PHASE23_RECORD_COUNT_V1,
};
use super::{
    AUTHENTICATION_TAG_BYTES_V1, GLOBAL_LOOKUP_TOPOLOGY_KAT_V1, Phase23GlobalLookupSourceReplayV1,
    TOTAL_REPLAY_IO_BYTES_V1, map_leaf_error_v1, validate_canonical_source_block_v1,
    validate_replay_record_v1,
};

const SOURCE_OPENING_VERSION_V1: u8 = 1;
const SOURCE_OPENING_GROUPS_PER_RECORD_V1: usize = 8;
const SOURCE_OPENING_BLOCKS_PER_GROUP_V1: usize = 64;
const SOURCE_OPENING_SCALARS_PER_BLOCK_V1: usize = 256;
const SOURCE_OPENING_SCALARS_PER_GROUP_V1: usize = 16_384;
const SOURCE_OPENING_GROUP_COUNT_V1: usize =
    PHASE23_RECORD_COUNT_V1 * SOURCE_OPENING_GROUPS_PER_RECORD_V1;
const SOURCE_OPENING_PEDERSEN_TERMS_PER_GROUP_V1: usize = SOURCE_OPENING_SCALARS_PER_GROUP_V1 + 1;
const SOURCE_OPENING_SCALAR_COUNT_V1: u64 =
    SOURCE_OPENING_GROUP_COUNT_V1 as u64 * SOURCE_OPENING_SCALARS_PER_GROUP_V1 as u64;
const SOURCE_OPENING_RETAINED_BLINDING_BYTES_V1: u64 = SOURCE_OPENING_GROUP_COUNT_V1 as u64 * 32;
const SOURCE_OPENING_PUBLIC_POINT_WIRE_BYTES_V1: u64 = SOURCE_OPENING_GROUP_COUNT_V1 as u64 * 33;
const SOURCE_OPENING_BLINDING_SLOT_BYTES_V1: u64 = 32;
const SOURCE_OPENING_BLINDING_FILE_BYTES_V1: u64 = SOURCE_OPENING_GROUP_COUNT_V1 as u64
    * (SOURCE_OPENING_BLINDING_SLOT_BYTES_V1 + AUTHENTICATION_TAG_BYTES_V1);
const SOURCE_OPENING_BLINDING_WRITE_AND_SEAL_READ_BYTES_V1: u64 =
    2 * SOURCE_OPENING_BLINDING_FILE_BYTES_V1;
const SOURCE_OPENING_CURRENT_REPLAY_IO_BYTES_V1: u64 =
    TOTAL_REPLAY_IO_BYTES_V1 + SOURCE_OPENING_BLINDING_WRITE_AND_SEAL_READ_BYTES_V1;

const CANONICAL_REOPEN_BLOCK_COUNT_V1: usize =
    PHASE23_RECORD_COUNT_V1 * PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1;
const CANONICAL_REOPEN_PLAINTEXT_BYTES_V1: u64 =
    CANONICAL_REOPEN_BLOCK_COUNT_V1 as u64 * PHASE23_MAIN_BLOCK_BYTES_V1 as u64;
const CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1: u64 = CANONICAL_REOPEN_BLOCK_COUNT_V1 as u64
    * (PHASE23_MAIN_BLOCK_BYTES_V1 as u64 + AUTHENTICATION_TAG_BYTES_V1);
const SOURCE_OPENING_LIFECYCLE_IO_BYTES_V1: u64 =
    SOURCE_OPENING_CURRENT_REPLAY_IO_BYTES_V1 + CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1;
const SOURCE_OPENING_NEW_SCALAR_MIRROR_FILE_BYTES_V1: u64 = 0;

const WEIGHTED_COLUMN_COUNT_V1: usize = 2;
const WEIGHTED_COLUMN_SCALAR_BYTES_V1: usize =
    WEIGHTED_COLUMN_COUNT_V1 * SOURCE_OPENING_SCALARS_PER_GROUP_V1 * core::mem::size_of::<Scalar>();
const WEIGHTED_COLUMN_GROUP_WEIGHT_BYTES_V1: usize =
    SOURCE_OPENING_GROUP_COUNT_V1 * core::mem::size_of::<Scalar>();
const WEIGHTED_COLUMN_SOURCE_CHUNK_BYTES_V1: usize = PHASE23_MAIN_BLOCK_BYTES_V1;
const WEIGHTED_COLUMN_NAMED_HEAP_BYTES_V1: usize = WEIGHTED_COLUMN_SCALAR_BYTES_V1
    + WEIGHTED_COLUMN_GROUP_WEIGHT_BYTES_V1
    + WEIGHTED_COLUMN_SOURCE_CHUNK_BYTES_V1;
const WEIGHTED_COLUMN_NAMED_HEAP_CEILING_BYTES_V1: usize = 2_700_000;

const SOURCE_OPENING_MAPPING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.mapping\0";
const SOURCE_OPENING_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.context\0";
const SOURCE_OPENING_BLINDING_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.blinding-spool-context\0";
const SOURCE_OPENING_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.commitments\0";
const SOURCE_OPENING_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.record\0";
const SOURCE_OPENING_TEST_ENTROPY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.test-entropy\0";
const SOURCE_OPENING_BLINDING_ORDER_V1: &[u8] =
    b"slot=commitment-ordinal=record*8+group;scalar=canonical-T256-big-endian";
const CANONICAL_REOPEN_SCHEDULE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.canonical-reopen-schedule\0";
const CANONICAL_REOPEN_RECORD_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-opening.canonical-reopen-record\0";
const SOURCE_GROUP_ORDER_V1: &[u8] = b"record-major:43-records*8-groups";
const SOURCE_TO_PACKING_MAP_V1: &[u8] = b"group-local:b=0..64;i=0..256;j=256*b+i;k=64*i+b";
const SOURCE_SNAPSHOT_BINDING_RULE_V1: &[u8] =
    b"source-publication-receipt-transitively-binds-provider,snapshot-identity,main-snapshot-digest,nonce-snapshot-digest";
const SOURCE_OPENING_BASIS_V1: &[u8] = b"ZkAmsT256BulletproofSuiteV1:G_bold[0..16384)+h";

const SOURCE_OPENING_MATERIALIZED_V1: bool = true;
const SOURCE_SAME_OPENING_PROVED_V1: bool = false;
const PACKING_SAME_OPENING_PROVED_V1: bool = false;
const GLOBAL_LOOKUP_PROOF_VERIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const AUTHORITY_ACCEPTED_V1: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false;
const RSS_GATE_ACCEPTED_V1: bool = false;
const RELEASE_READY_V1: bool = false;
const RELEASE_COMPLETE_V1: bool = false;

const _: () = {
    assert!(PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 == 512);
    assert!(SOURCE_OPENING_GROUPS_PER_RECORD_V1 * SOURCE_OPENING_BLOCKS_PER_GROUP_V1 == 512);
    assert!(
        SOURCE_OPENING_BLOCKS_PER_GROUP_V1 * SOURCE_OPENING_SCALARS_PER_BLOCK_V1
            == SOURCE_OPENING_SCALARS_PER_GROUP_V1
    );
    assert!(SOURCE_OPENING_GROUP_COUNT_V1 == 344);
    assert!(SOURCE_OPENING_PEDERSEN_TERMS_PER_GROUP_V1 == 16_385);
    assert!(SOURCE_OPENING_SCALAR_COUNT_V1 == 5_636_096);
    assert!(SOURCE_OPENING_RETAINED_BLINDING_BYTES_V1 == 11_008);
    assert!(SOURCE_OPENING_PUBLIC_POINT_WIRE_BYTES_V1 == 11_352);
    assert!(SOURCE_OPENING_BLINDING_FILE_BYTES_V1 == 16_512);
    assert!(SOURCE_OPENING_BLINDING_WRITE_AND_SEAL_READ_BYTES_V1 == 33_024);
    assert!(SOURCE_OPENING_CURRENT_REPLAY_IO_BYTES_V1 == 350_120_448);
    assert!(CANONICAL_REOPEN_BLOCK_COUNT_V1 == 22_016);
    assert!(CANONICAL_REOPEN_PLAINTEXT_BYTES_V1 == 180_355_072);
    assert!(CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1 == 180_707_328);
    assert!(SOURCE_OPENING_LIFECYCLE_IO_BYTES_V1 == 530_827_776);
    assert!(SOURCE_OPENING_NEW_SCALAR_MIRROR_FILE_BYTES_V1 == 0);
    assert!(WEIGHTED_COLUMN_NAMED_HEAP_BYTES_V1 == 1_067_776);
    assert!(WEIGHTED_COLUMN_NAMED_HEAP_BYTES_V1 < WEIGHTED_COLUMN_NAMED_HEAP_CEILING_BYTES_V1);
    assert!(SOURCE_OPENING_MATERIALIZED_V1);
    assert!(!SOURCE_SAME_OPENING_PROVED_V1);
    assert!(!PACKING_SAME_OPENING_PROVED_V1);
    assert!(!GLOBAL_LOOKUP_PROOF_VERIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!AUTHORITY_ACCEPTED_V1);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!RSS_GATE_ACCEPTED_V1);
    assert!(!RELEASE_READY_V1);
    assert!(!RELEASE_COMPLETE_V1);
};

struct SourceOpeningGroupCoordinateV1 {
    ordinal: u16,
    record: u16,
    group: u8,
}

fn source_opening_group_coordinate_v1(
    ordinal: usize,
) -> Result<SourceOpeningGroupCoordinateV1, ZkAmsMkheErrorV1> {
    if ordinal >= SOURCE_OPENING_GROUP_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(SourceOpeningGroupCoordinateV1 {
        ordinal: u16::try_from(ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        record: u16::try_from(ordinal / SOURCE_OPENING_GROUPS_PER_RECORD_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        group: u8::try_from(ordinal % SOURCE_OPENING_GROUPS_PER_RECORD_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    })
}

fn source_to_packing_coordinate_v1(source_j: usize) -> Result<usize, ZkAmsMkheErrorV1> {
    if source_j >= SOURCE_OPENING_SCALARS_PER_GROUP_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let block = source_j / SOURCE_OPENING_SCALARS_PER_BLOCK_V1;
    let coefficient = source_j % SOURCE_OPENING_SCALARS_PER_BLOCK_V1;
    Ok(SOURCE_OPENING_BLOCKS_PER_GROUP_V1 * coefficient + block)
}

fn source_opening_mapping_digest_for_orders_v1(
    group_order: &[u16],
    source_order: &[u16],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if group_order.len() != SOURCE_OPENING_GROUP_COUNT_V1
        || source_order.len() != SOURCE_OPENING_SCALARS_PER_GROUP_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let topology_digest = global_lookup_topology_digest_v1();
    if topology_digest != GLOBAL_LOOKUP_TOPOLOGY_KAT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(SOURCE_OPENING_MAPPING_DOMAIN_V1);
    hash.update(&[SOURCE_OPENING_VERSION_V1]);
    hash.update(&topology_digest);
    for value in [
        PHASE23_RECORD_COUNT_V1 as u32,
        SOURCE_OPENING_GROUPS_PER_RECORD_V1 as u32,
        SOURCE_OPENING_GROUP_COUNT_V1 as u32,
        SOURCE_OPENING_BLOCKS_PER_GROUP_V1 as u32,
        SOURCE_OPENING_SCALARS_PER_BLOCK_V1 as u32,
        SOURCE_OPENING_SCALARS_PER_GROUP_V1 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&(SOURCE_GROUP_ORDER_V1.len() as u16).to_be_bytes());
    hash.update(SOURCE_GROUP_ORDER_V1);
    hash.update(&(SOURCE_TO_PACKING_MAP_V1.len() as u16).to_be_bytes());
    hash.update(SOURCE_TO_PACKING_MAP_V1);

    let mut seen_groups = [false; SOURCE_OPENING_GROUP_COUNT_V1];
    for (stream_ordinal, requested_ordinal) in group_order.iter().copied().enumerate() {
        let requested_ordinal = usize::from(requested_ordinal);
        if requested_ordinal >= seen_groups.len() || seen_groups[requested_ordinal] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        seen_groups[requested_ordinal] = true;
        let coordinate = source_opening_group_coordinate_v1(requested_ordinal)?;
        hash.update(&(stream_ordinal as u16).to_be_bytes());
        hash.update(&coordinate.ordinal.to_be_bytes());
        hash.update(&coordinate.record.to_be_bytes());
        hash.update(&[coordinate.group]);
    }
    if seen_groups.contains(&false) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }

    let mut seen_source = [false; SOURCE_OPENING_SCALARS_PER_GROUP_V1];
    let mut seen_packing = [false; SOURCE_OPENING_SCALARS_PER_GROUP_V1];
    for (stream_j, requested_j) in source_order.iter().copied().enumerate() {
        let requested_j = usize::from(requested_j);
        if requested_j >= seen_source.len() || seen_source[requested_j] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let packing_k = source_to_packing_coordinate_v1(requested_j)?;
        if seen_packing[packing_k] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        seen_source[requested_j] = true;
        seen_packing[packing_k] = true;
        let block = requested_j / SOURCE_OPENING_SCALARS_PER_BLOCK_V1;
        let coefficient = requested_j % SOURCE_OPENING_SCALARS_PER_BLOCK_V1;
        hash.update(&(stream_j as u16).to_be_bytes());
        hash.update(&(requested_j as u16).to_be_bytes());
        hash.update(&(block as u16).to_be_bytes());
        hash.update(&(coefficient as u16).to_be_bytes());
        hash.update(&(packing_k as u16).to_be_bytes());
    }
    if seen_source.contains(&false) || seen_packing.contains(&false) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    require_nonzero_opening_digest_v1(hash.finalize())
}

fn exact_source_opening_mapping_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let group_order: [u16; SOURCE_OPENING_GROUP_COUNT_V1] =
        core::array::from_fn(|index| index as u16);
    let source_order: [u16; SOURCE_OPENING_SCALARS_PER_GROUP_V1] =
        core::array::from_fn(|index| index as u16);
    source_opening_mapping_digest_for_orders_v1(&group_order, &source_order)
}

struct SourceOpeningContextAxesV1 {
    source_receipt_digest: [u8; 32],
    prerequisite_record_digest: [u8; 32],
    replay_spool_context_digest: [u8; 32],
}

fn source_opening_context_digest_v1(
    axes: &SourceOpeningContextAxesV1,
    topology_digest: [u8; 32],
    mapping_digest: [u8; 32],
    basis_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if topology_digest != GLOBAL_LOOKUP_TOPOLOGY_KAT_V1
        || basis_digest != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(SOURCE_OPENING_CONTEXT_DOMAIN_V1);
    hash.update(&[SOURCE_OPENING_VERSION_V1]);
    hash.update(&topology_digest);
    hash.update(&require_nonzero_opening_digest_v1(mapping_digest)?);
    hash.update(&basis_digest);
    hash.update(&(SOURCE_OPENING_BASIS_V1.len() as u16).to_be_bytes());
    hash.update(SOURCE_OPENING_BASIS_V1);
    hash.update(&(SOURCE_SNAPSHOT_BINDING_RULE_V1.len() as u16).to_be_bytes());
    hash.update(SOURCE_SNAPSHOT_BINDING_RULE_V1);
    for digest in [
        axes.source_receipt_digest,
        axes.prerequisite_record_digest,
        axes.replay_spool_context_digest,
    ] {
        hash.update(&require_nonzero_opening_digest_v1(digest)?);
    }
    require_nonzero_opening_digest_v1(hash.finalize())
}

fn source_opening_blinding_context_digest_v1(
    opening_context_digest: [u8; 32],
    mapping_digest: [u8; 32],
    basis_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if basis_digest != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(SOURCE_OPENING_BLINDING_CONTEXT_DOMAIN_V1);
    hash.update(&[SOURCE_OPENING_VERSION_V1]);
    hash.update(&require_nonzero_opening_digest_v1(opening_context_digest)?);
    hash.update(&require_nonzero_opening_digest_v1(mapping_digest)?);
    hash.update(&basis_digest);
    hash.update(&(SOURCE_OPENING_GROUP_COUNT_V1 as u16).to_be_bytes());
    hash.update(&SOURCE_OPENING_BLINDING_SLOT_BYTES_V1.to_be_bytes());
    hash.update(&(SOURCE_OPENING_BLINDING_ORDER_V1.len() as u16).to_be_bytes());
    hash.update(SOURCE_OPENING_BLINDING_ORDER_V1);
    require_nonzero_opening_digest_v1(hash.finalize())
}

/// Production cannot currently mint the proof-session entropy capability.
/// The deterministic fixture exists only for independent unit tests.
pub(in crate::vega::zk_ams::mkhe) enum GlobalLookupSourceOpeningEntropySealV1 {
    Production {
        proof_session_entropy: Infallible,
    },
    #[cfg(test)]
    TestOnly(DeterministicSourceOpeningEntropyV1),
}

#[cfg(test)]
enum TestEntropyFaultV1 {
    None,
    ErrorAt(u16),
    ZeroAt(u16),
    PanicAt(u16),
}

#[cfg(test)]
pub(in crate::vega::zk_ams::mkhe) struct DeterministicSourceOpeningEntropyV1 {
    seed: [u8; 32],
    next_group: u16,
    active_group: Option<u16>,
    attempt: u16,
    fault: TestEntropyFaultV1,
}

#[cfg(test)]
impl DeterministicSourceOpeningEntropyV1 {
    const fn new_v1(seed: [u8; 32]) -> Self {
        Self {
            seed,
            next_group: 0,
            active_group: None,
            attempt: 0,
            fault: TestEntropyFaultV1::None,
        }
    }

    const fn with_fault_v1(seed: [u8; 32], fault: TestEntropyFaultV1) -> Self {
        Self {
            seed,
            next_group: 0,
            active_group: None,
            attempt: 0,
            fault,
        }
    }

    fn begin_group_v1(&mut self, group: u16) -> Result<(), ZkAmsMkheErrorV1> {
        if group != self.next_group || self.active_group.is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        self.active_group = Some(group);
        self.attempt = 0;
        Ok(())
    }

    fn finish_group_v1(&mut self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.active_group != Some(self.next_group) {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        self.active_group = None;
        self.attempt = 0;
        self.next_group += 1;
        Ok(())
    }
}

#[cfg(test)]
impl MaskedRelaxedRandomSourceV1 for DeterministicSourceOpeningEntropyV1 {
    fn fill_bytes(
        &mut self,
        destination: &mut [u8],
    ) -> Result<(), crate::vega::MaskedRelaxedRandomErrorV1> {
        let group = self
            .active_group
            .ok_or(crate::vega::MaskedRelaxedRandomErrorV1::Unavailable)?;
        if destination.len() != SOURCE_OPENING_BLINDING_SLOT_BYTES_V1 as usize {
            return Err(crate::vega::MaskedRelaxedRandomErrorV1::Unavailable);
        }
        match &self.fault {
            TestEntropyFaultV1::ErrorAt(at) if *at == group => {
                return Err(crate::vega::MaskedRelaxedRandomErrorV1::Unavailable);
            }
            TestEntropyFaultV1::ZeroAt(at) if *at == group => destination.fill(0),
            TestEntropyFaultV1::PanicAt(at) if *at == group => {
                panic!("intentional source-opening entropy unwind");
            }
            TestEntropyFaultV1::None
            | TestEntropyFaultV1::ErrorAt(_)
            | TestEntropyFaultV1::ZeroAt(_)
            | TestEntropyFaultV1::PanicAt(_) => {
                let destination: &mut [u8; 32] = destination
                    .try_into()
                    .map_err(|_| crate::vega::MaskedRelaxedRandomErrorV1::Unavailable)?;
                let mut hash = Keccak256::new();
                hash.update(SOURCE_OPENING_TEST_ENTROPY_DOMAIN_V1);
                hash.update(&self.seed);
                hash.update(&group.to_be_bytes());
                hash.update(&self.attempt.to_be_bytes());
                hash.finalize_into(destination);
            }
        }
        self.attempt = self.attempt.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
impl Drop for DeterministicSourceOpeningEntropyV1 {
    fn drop(&mut self) {
        self.seed.fill(0);
        self.next_group = 0;
        self.active_group = None;
        self.attempt = 0;
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

impl GlobalLookupSourceOpeningEntropySealV1 {
    fn sample_blinding_chunk_v1(
        &mut self,
        group: u16,
    ) -> Result<(ConfidentialSpoolChunkV1, ZeroizingT256ScalarCopyV1), ZkAmsMkheErrorV1> {
        match self {
            Self::Production {
                proof_session_entropy,
            } => match *proof_session_entropy {},
            #[cfg(test)]
            Self::TestOnly(entropy) => {
                entropy.begin_group_v1(group)?;
                let result = sample_blinding_chunk_from_random_v1(entropy)?;
                entropy.finish_group_v1()?;
                Ok(result)
            }
        }
    }

    #[cfg(test)]
    pub(super) const fn test_only_v1(seed: [u8; 32]) -> Self {
        Self::TestOnly(DeterministicSourceOpeningEntropyV1::new_v1(seed))
    }
}

fn sample_blinding_chunk_from_random_v1(
    random: &mut impl MaskedRelaxedRandomSourceV1,
) -> Result<(ConfidentialSpoolChunkV1, ZeroizingT256ScalarCopyV1), ZkAmsMkheErrorV1> {
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut chunk =
            ConfidentialSpoolChunkV1::new_zeroed_v1(SOURCE_OPENING_BLINDING_SLOT_BYTES_V1)
                .map_err(map_leaf_error_v1)?;
        random
            .fill_bytes(chunk.as_mut_slice_v1())
            .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
        let encoded: &[u8; 32] = chunk
            .as_mut_slice_v1()
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if let Ok(mut blinding) = Scalar::from_be_bytes_exact_ref(encoded) {
            let blinding = ZeroizingT256ScalarCopyV1::take(&mut blinding);
            if blinding.get().is_zero() {
                continue;
            }
            return Ok((chunk, blinding));
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

struct SourceOpeningLiveV1 {
    entropy: GlobalLookupSourceOpeningEntropySealV1,
    group_scalars: ZeroizingT256ScalarVecV1,
    blinding_writer: ConfidentialSpoolWriterV1,
    commitments: Vec<Point>,
    topology_digest: [u8; 32],
    mapping_digest: [u8; 32],
    basis_digest: [u8; 32],
    context_digest: [u8; 32],
    blinding_context_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    prerequisite_record_digest: [u8; 32],
    commitment_hash: Keccak256,
    next_record: u16,
    next_block: u16,
    next_group: u16,
}

pub(super) struct SourceOpeningAssemblyV1 {
    live: Option<SourceOpeningLiveV1>,
}

impl SourceOpeningAssemblyV1 {
    pub(super) fn begin_v1(
        source_receipt_digest: [u8; 32],
        prerequisite_record_digest: [u8; 32],
        replay_spool_context_digest: [u8; 32],
        entropy: GlobalLookupSourceOpeningEntropySealV1,
        directory: &PathBuf,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // Establish every local allocation and the pinned basis before any
        // source scalar or fresh blinding enters this owner.
        ZkAmsT256BulletproofSuiteV1::generators()
            .reduce(SOURCE_OPENING_SCALARS_PER_GROUP_V1)
            .map_err(map_bulletproof_error_v1)?;
        let basis_digest = zk_ams_t256_bulletproof_generator_basis_digest_v1();
        if basis_digest != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let topology_digest = global_lookup_topology_digest_v1();
        let mapping_digest = exact_source_opening_mapping_digest_v1()?;
        let axes = SourceOpeningContextAxesV1 {
            source_receipt_digest,
            prerequisite_record_digest,
            replay_spool_context_digest,
        };
        let context_digest =
            source_opening_context_digest_v1(&axes, topology_digest, mapping_digest, basis_digest)?;
        let blinding_context_digest = source_opening_blinding_context_digest_v1(
            context_digest,
            mapping_digest,
            basis_digest,
        )?;
        let blinding_layout = ConfidentialSpoolLayoutV1::new_v1(
            SOURCE_OPENING_GROUP_COUNT_V1 as u64,
            SOURCE_OPENING_BLINDING_SLOT_BYTES_V1,
            blinding_context_digest,
        )
        .map_err(map_leaf_error_v1)?;
        if blinding_layout.slot_count_v1() != SOURCE_OPENING_GROUP_COUNT_V1 as u64
            || blinding_layout.plaintext_len_v1() != SOURCE_OPENING_BLINDING_SLOT_BYTES_V1
            || blinding_layout.file_len_v1() != SOURCE_OPENING_BLINDING_FILE_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let blinding_writer = ConfidentialSpoolWriterV1::create_in_v1(directory, blinding_layout)
            .map_err(map_leaf_error_v1)?;
        let mut commitments = Vec::new();
        commitments
            .try_reserve_exact(SOURCE_OPENING_GROUP_COUNT_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut commitment_hash = Keccak256::new();
        commitment_hash.update(SOURCE_OPENING_COMMITMENT_DOMAIN_V1);
        commitment_hash.update(&[SOURCE_OPENING_VERSION_V1]);
        commitment_hash.update(&context_digest);
        commitment_hash.update(&basis_digest);
        commitment_hash.update(&mapping_digest);
        commitment_hash.update(&(SOURCE_OPENING_GROUP_COUNT_V1 as u16).to_be_bytes());
        Ok(Self {
            live: Some(SourceOpeningLiveV1 {
                entropy,
                group_scalars: ZeroizingT256ScalarVecV1::with_capacity(
                    SOURCE_OPENING_SCALARS_PER_GROUP_V1,
                ),
                blinding_writer,
                commitments,
                topology_digest,
                mapping_digest,
                basis_digest,
                context_digest,
                blinding_context_digest,
                source_receipt_digest: axes.source_receipt_digest,
                prerequisite_record_digest: axes.prerequisite_record_digest,
                commitment_hash,
                next_record: 0,
                next_block: 0,
                next_group: 0,
            }),
        })
    }

    pub(super) fn absorb_next_canonical_block_v1(
        &mut self,
        record: u16,
        block: u16,
        bytes: &[u8],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        // Poison this assembly before coordinate validation, scalar parsing,
        // entropy, or MSM work. Any error or unwind drops all accumulated
        // blindings and the group-local plaintext scalars.
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if record != live.next_record
            || block != live.next_block
            || usize::from(record) >= PHASE23_RECORD_COUNT_V1
            || usize::from(block) >= PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1
            || live.commitments.len() != usize::from(live.next_group)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        validate_canonical_source_block_v1(bytes)?;
        let block_in_group = usize::from(block) % SOURCE_OPENING_BLOCKS_PER_GROUP_V1;
        let expected_group_scalars = block_in_group * SOURCE_OPENING_SCALARS_PER_BLOCK_V1;
        if live.group_scalars.len() != expected_group_scalars {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        append_canonical_block_scalars_v1(&mut live.group_scalars, bytes)?;
        if block_in_group + 1 == SOURCE_OPENING_BLOCKS_PER_GROUP_V1 {
            let coordinate = source_opening_group_coordinate_v1(usize::from(live.next_group))?;
            if coordinate.record != record
                || usize::from(coordinate.group)
                    != usize::from(block) / SOURCE_OPENING_BLOCKS_PER_GROUP_V1
                || live.group_scalars.len() != SOURCE_OPENING_SCALARS_PER_GROUP_V1
            {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            let (blinding_chunk, blinding) =
                live.entropy.sample_blinding_chunk_v1(coordinate.ordinal)?;
            let commitment = source_opening_commitment_for_suite_v1::<ZkAmsT256BulletproofSuiteV1>(
                live.group_scalars.as_slice(),
                blinding.as_ref(),
                SOURCE_OPENING_SCALARS_PER_GROUP_V1,
            )?;
            live.commitment_hash
                .update(&coordinate.ordinal.to_be_bytes());
            live.commitment_hash
                .update(&coordinate.record.to_be_bytes());
            live.commitment_hash.update(&[coordinate.group]);
            live.commitment_hash.update(
                &commitment
                    .to_non_identity_wire_bytes()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
            );
            live.blinding_writer
                .write_slot_v1(u64::from(coordinate.ordinal), blinding_chunk)
                .map_err(map_leaf_error_v1)?;
            live.commitments.push(commitment);
            live.group_scalars.clear_and_truncate(0);
            live.next_group = live
                .next_group
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
        live.next_block = live
            .next_block
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if usize::from(live.next_block) == PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 {
            live.next_block = 0;
            live.next_record = live
                .next_record
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
        self.live = Some(live);
        Ok(())
    }

    pub(super) fn finish_v1(
        mut self,
    ) -> Result<GlobalLookupSourceOpeningMaterialV1, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if usize::from(live.next_record) != PHASE23_RECORD_COUNT_V1
            || live.next_block != 0
            || usize::from(live.next_group) != SOURCE_OPENING_GROUP_COUNT_V1
            || !live.group_scalars.as_slice().is_empty()
            || live.commitments.len() != SOURCE_OPENING_GROUP_COUNT_V1
            || global_lookup_topology_digest_v1() != live.topology_digest
            || exact_source_opening_mapping_digest_v1()? != live.mapping_digest
            || zk_ams_t256_bulletproof_generator_basis_digest_v1() != live.basis_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let commitments_root = require_nonzero_opening_digest_v1(live.commitment_hash.finalize())?;
        let blinding_snapshot = live.blinding_writer.seal_v1().map_err(map_leaf_error_v1)?;
        if blinding_snapshot.slot_count_v1() != SOURCE_OPENING_GROUP_COUNT_V1 as u64
            || blinding_snapshot.plaintext_len_v1() != SOURCE_OPENING_BLINDING_SLOT_BYTES_V1
            || blinding_snapshot.file_len_v1() != SOURCE_OPENING_BLINDING_FILE_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let blinding_snapshot_root =
            require_nonzero_opening_digest_v1(*blinding_snapshot.snapshot_digest_v1())?;
        let mut record = SourceOpeningRecordV1 {
            source_receipt_digest: live.source_receipt_digest,
            prerequisite_record_digest: live.prerequisite_record_digest,
            topology_digest: live.topology_digest,
            mapping_digest: live.mapping_digest,
            basis_digest: live.basis_digest,
            context_digest: live.context_digest,
            blinding_context_digest: live.blinding_context_digest,
            commitments_root,
            blinding_snapshot_root,
            group_count: SOURCE_OPENING_GROUP_COUNT_V1 as u16,
            scalars_per_group: SOURCE_OPENING_SCALARS_PER_GROUP_V1 as u32,
            total_source_scalars: SOURCE_OPENING_SCALAR_COUNT_V1,
            pedersen_terms_per_group: SOURCE_OPENING_PEDERSEN_TERMS_PER_GROUP_V1 as u32,
            retained_blinding_bytes: SOURCE_OPENING_RETAINED_BLINDING_BYTES_V1,
            public_point_wire_bytes: SOURCE_OPENING_PUBLIC_POINT_WIRE_BYTES_V1,
            first_pass_replay_io_bytes: TOTAL_REPLAY_IO_BYTES_V1,
            blinding_file_bytes: SOURCE_OPENING_BLINDING_FILE_BYTES_V1,
            blinding_write_and_seal_read_bytes:
                SOURCE_OPENING_BLINDING_WRITE_AND_SEAL_READ_BYTES_V1,
            current_replay_io_bytes: SOURCE_OPENING_CURRENT_REPLAY_IO_BYTES_V1,
            later_canonical_plaintext_bytes: CANONICAL_REOPEN_PLAINTEXT_BYTES_V1,
            later_canonical_authenticated_read_bytes: CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1,
            total_lifecycle_io_bytes: SOURCE_OPENING_LIFECYCLE_IO_BYTES_V1,
            new_scalar_mirror_file_bytes: SOURCE_OPENING_NEW_SCALAR_MIRROR_FILE_BYTES_V1,
            source_opening_materialized: SOURCE_OPENING_MATERIALIZED_V1,
            source_same_opening_proved: SOURCE_SAME_OPENING_PROVED_V1,
            packing_same_opening_proved: PACKING_SAME_OPENING_PROVED_V1,
            global_lookup_proof_verified: GLOBAL_LOOKUP_PROOF_VERIFIED_V1,
            zero_knowledge_accepted: ZERO_KNOWLEDGE_ACCEPTED_V1,
            authority_accepted: AUTHORITY_ACCEPTED_V1,
            operational_receipt_accepted: OPERATIONAL_RECEIPT_ACCEPTED_V1,
            rss_gate_accepted: RSS_GATE_ACCEPTED_V1,
            release_ready: RELEASE_READY_V1,
            release_complete: RELEASE_COMPLETE_V1,
            record_digest: [0; 32],
        };
        record.record_digest = source_opening_record_digest_v1(&record)?;
        validate_source_opening_record_v1(&record)?;
        let material = GlobalLookupSourceOpeningMaterialV1 {
            blinding_snapshot,
            commitments: live.commitments,
            record,
        };
        material.validate_v1()?;
        Ok(material)
    }

    #[cfg(test)]
    fn panic_after_take_for_test_v1(&mut self) {
        let _live = self.live.take().expect("live source opening assembly");
        panic!("intentional source-opening assembly unwind");
    }
}

fn append_canonical_block_scalars_v1(
    destination: &mut ZeroizingT256ScalarVecV1,
    bytes: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    if bytes.len() != PHASE23_MAIN_BLOCK_BYTES_V1
        || bytes.len() / 32 != SOURCE_OPENING_SCALARS_PER_BLOCK_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for encoded in bytes.chunks_exact(32) {
        let encoded: &[u8; 32] = encoded
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        destination.push(
            Scalar::from_be_bytes_exact_ref(encoded)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    Ok(())
}

fn source_opening_commitment_for_suite_v1<S>(
    values: &[Scalar],
    blinding: &Scalar,
    exact_values: usize,
) -> Result<Point, ZkAmsMkheErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    let blinding = ZeroizingT256ScalarCopyV1::new(*blinding);
    if values.len() != exact_values
        || exact_values == 0
        || !exact_values.is_power_of_two()
        || blinding.get().is_zero()
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let generators = S::generators()
        .reduce(exact_values)
        .map_err(map_bulletproof_error_v1)?;
    let mut terms =
        SecretMultiexpBuilder::<S>::new(exact_values + 1).map_err(map_bulletproof_error_v1)?;
    for (value, generator) in values.iter().zip(generators.g_bold) {
        terms
            .push(value, generator)
            .map_err(map_bulletproof_error_v1)?;
    }
    terms
        .push(blinding.as_ref(), &generators.h)
        .map_err(map_bulletproof_error_v1)?;
    let commitment = terms.evaluate().map_err(map_bulletproof_error_v1)?;
    if commitment.is_identity() {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(commitment)
}

struct SourceOpeningRecordV1 {
    source_receipt_digest: [u8; 32],
    prerequisite_record_digest: [u8; 32],
    topology_digest: [u8; 32],
    mapping_digest: [u8; 32],
    basis_digest: [u8; 32],
    context_digest: [u8; 32],
    blinding_context_digest: [u8; 32],
    commitments_root: [u8; 32],
    blinding_snapshot_root: [u8; 32],
    group_count: u16,
    scalars_per_group: u32,
    total_source_scalars: u64,
    pedersen_terms_per_group: u32,
    retained_blinding_bytes: u64,
    public_point_wire_bytes: u64,
    first_pass_replay_io_bytes: u64,
    blinding_file_bytes: u64,
    blinding_write_and_seal_read_bytes: u64,
    current_replay_io_bytes: u64,
    later_canonical_plaintext_bytes: u64,
    later_canonical_authenticated_read_bytes: u64,
    total_lifecycle_io_bytes: u64,
    new_scalar_mirror_file_bytes: u64,
    source_opening_materialized: bool,
    source_same_opening_proved: bool,
    packing_same_opening_proved: bool,
    global_lookup_proof_verified: bool,
    zero_knowledge_accepted: bool,
    authority_accepted: bool,
    operational_receipt_accepted: bool,
    rss_gate_accepted: bool,
    release_ready: bool,
    release_complete: bool,
    record_digest: [u8; 32],
}

fn source_opening_record_digest_v1(
    record: &SourceOpeningRecordV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_OPENING_RECORD_DOMAIN_V1);
    hash.update(&[SOURCE_OPENING_VERSION_V1]);
    for digest in [
        record.source_receipt_digest,
        record.prerequisite_record_digest,
        record.topology_digest,
        record.mapping_digest,
        record.basis_digest,
        record.context_digest,
        record.blinding_context_digest,
        record.commitments_root,
        record.blinding_snapshot_root,
    ] {
        hash.update(&require_nonzero_opening_digest_v1(digest)?);
    }
    hash.update(&record.group_count.to_be_bytes());
    hash.update(&record.scalars_per_group.to_be_bytes());
    hash.update(&record.total_source_scalars.to_be_bytes());
    hash.update(&record.pedersen_terms_per_group.to_be_bytes());
    hash.update(&record.retained_blinding_bytes.to_be_bytes());
    hash.update(&record.public_point_wire_bytes.to_be_bytes());
    hash.update(&record.first_pass_replay_io_bytes.to_be_bytes());
    hash.update(&record.blinding_file_bytes.to_be_bytes());
    hash.update(&record.blinding_write_and_seal_read_bytes.to_be_bytes());
    hash.update(&record.current_replay_io_bytes.to_be_bytes());
    hash.update(&record.later_canonical_plaintext_bytes.to_be_bytes());
    hash.update(
        &record
            .later_canonical_authenticated_read_bytes
            .to_be_bytes(),
    );
    hash.update(&record.total_lifecycle_io_bytes.to_be_bytes());
    hash.update(&record.new_scalar_mirror_file_bytes.to_be_bytes());
    hash.update(&[
        record.source_opening_materialized as u8,
        record.source_same_opening_proved as u8,
        record.packing_same_opening_proved as u8,
        record.global_lookup_proof_verified as u8,
        record.zero_knowledge_accepted as u8,
        record.authority_accepted as u8,
        record.operational_receipt_accepted as u8,
        record.rss_gate_accepted as u8,
        record.release_ready as u8,
        record.release_complete as u8,
    ]);
    require_nonzero_opening_digest_v1(hash.finalize())
}

fn validate_source_opening_record_v1(
    record: &SourceOpeningRecordV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if record.source_receipt_digest == [0; 32]
        || record.prerequisite_record_digest == [0; 32]
        || record.topology_digest != GLOBAL_LOOKUP_TOPOLOGY_KAT_V1
        || record.mapping_digest != exact_source_opening_mapping_digest_v1()?
        || record.basis_digest != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1
        || record.context_digest == [0; 32]
        || record.blinding_context_digest
            != source_opening_blinding_context_digest_v1(
                record.context_digest,
                record.mapping_digest,
                record.basis_digest,
            )?
        || record.commitments_root == [0; 32]
        || record.blinding_snapshot_root == [0; 32]
        || usize::from(record.group_count) != SOURCE_OPENING_GROUP_COUNT_V1
        || record.scalars_per_group != SOURCE_OPENING_SCALARS_PER_GROUP_V1 as u32
        || record.total_source_scalars != SOURCE_OPENING_SCALAR_COUNT_V1
        || record.pedersen_terms_per_group != SOURCE_OPENING_PEDERSEN_TERMS_PER_GROUP_V1 as u32
        || record.retained_blinding_bytes != SOURCE_OPENING_RETAINED_BLINDING_BYTES_V1
        || record.public_point_wire_bytes != SOURCE_OPENING_PUBLIC_POINT_WIRE_BYTES_V1
        || record.first_pass_replay_io_bytes != TOTAL_REPLAY_IO_BYTES_V1
        || record.blinding_file_bytes != SOURCE_OPENING_BLINDING_FILE_BYTES_V1
        || record.blinding_write_and_seal_read_bytes
            != SOURCE_OPENING_BLINDING_WRITE_AND_SEAL_READ_BYTES_V1
        || record.current_replay_io_bytes != SOURCE_OPENING_CURRENT_REPLAY_IO_BYTES_V1
        || record.later_canonical_plaintext_bytes != CANONICAL_REOPEN_PLAINTEXT_BYTES_V1
        || record.later_canonical_authenticated_read_bytes
            != CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1
        || record.total_lifecycle_io_bytes != SOURCE_OPENING_LIFECYCLE_IO_BYTES_V1
        || record.new_scalar_mirror_file_bytes != SOURCE_OPENING_NEW_SCALAR_MIRROR_FILE_BYTES_V1
        || !record.source_opening_materialized
        || record.source_same_opening_proved
        || record.packing_same_opening_proved
        || record.global_lookup_proof_verified
        || record.zero_knowledge_accepted
        || record.authority_accepted
        || record.operational_receipt_accepted
        || record.rss_gate_accepted
        || record.release_ready
        || record.release_complete
        || record.record_digest != source_opening_record_digest_v1(record)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

/// Opaque move-only source-opening material.
pub(in crate::vega::zk_ams::mkhe) struct GlobalLookupSourceOpeningMaterialV1 {
    blinding_snapshot: ConfidentialSpoolSnapshotV1,
    commitments: Vec<Point>,
    record: SourceOpeningRecordV1,
}

impl GlobalLookupSourceOpeningMaterialV1 {
    fn validate_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        validate_source_opening_record_v1(&self.record)?;
        if self.blinding_snapshot.slot_count_v1() != SOURCE_OPENING_GROUP_COUNT_V1 as u64
            || self.blinding_snapshot.plaintext_len_v1() != SOURCE_OPENING_BLINDING_SLOT_BYTES_V1
            || self.blinding_snapshot.file_len_v1() != SOURCE_OPENING_BLINDING_FILE_BYTES_V1
            || *self.blinding_snapshot.snapshot_digest_v1() != self.record.blinding_snapshot_root
            || self.commitments.len() != SOURCE_OPENING_GROUP_COUNT_V1
            || self.commitments.iter().copied().any(Point::is_identity)
            || commitments_root_v1(self.record.context_digest, &self.commitments)?
                != self.record.commitments_root
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

fn commitments_root_v1(
    context_digest: [u8; 32],
    commitments: &[Point],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if commitments.len() != SOURCE_OPENING_GROUP_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(SOURCE_OPENING_COMMITMENT_DOMAIN_V1);
    hash.update(&[SOURCE_OPENING_VERSION_V1]);
    hash.update(&require_nonzero_opening_digest_v1(context_digest)?);
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    hash.update(&exact_source_opening_mapping_digest_v1()?);
    hash.update(&(SOURCE_OPENING_GROUP_COUNT_V1 as u16).to_be_bytes());
    for (ordinal, commitment) in commitments.iter().copied().enumerate() {
        let coordinate = source_opening_group_coordinate_v1(ordinal)?;
        hash.update(&coordinate.ordinal.to_be_bytes());
        hash.update(&coordinate.record.to_be_bytes());
        hash.update(&[coordinate.group]);
        hash.update(
            &commitment
                .to_non_identity_wire_bytes()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
        );
    }
    require_nonzero_opening_digest_v1(hash.finalize())
}

/// Future post-rho authority. Production cannot yet provide verifier weights.
pub(in crate::vega::zk_ams::mkhe) enum GlobalLookupCanonicalReopenSealV1 {
    Production {
        post_rho_verifier_weights: Infallible,
    },
    #[cfg(test)]
    TestOnly(ZeroizingT256ScalarVecV1),
}

#[cfg(test)]
impl GlobalLookupCanonicalReopenSealV1 {
    fn deterministic_test_v1() -> Self {
        let mut weights = ZeroizingT256ScalarVecV1::with_capacity(SOURCE_OPENING_GROUP_COUNT_V1);
        for ordinal in 0..SOURCE_OPENING_GROUP_COUNT_V1 {
            weights.push(Scalar::from_u64(ordinal as u64 + 1));
        }
        Self::TestOnly(weights)
    }
}

trait PurposeBoundCanonicalOpeningSinkV1: Sized {
    type Output;

    fn absorb_next_scalar_v1(
        &mut self,
        scalar: &ZeroizingT256ScalarCopyV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;

    fn finish_v1(self) -> Result<Self::Output, ZkAmsMkheErrorV1>;
}

struct WeightedOpeningColumnsSinkV1 {
    group_weights: ZeroizingT256ScalarVecV1,
    source_column: ZeroizingT256ScalarVecV1,
    packing_column: ZeroizingT256ScalarVecV1,
    next_scalar: u64,
}

impl WeightedOpeningColumnsSinkV1 {
    fn from_seal_v1(seal: GlobalLookupCanonicalReopenSealV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let group_weights = match seal {
            GlobalLookupCanonicalReopenSealV1::Production {
                post_rho_verifier_weights,
            } => match post_rho_verifier_weights {},
            #[cfg(test)]
            GlobalLookupCanonicalReopenSealV1::TestOnly(weights) => weights,
        };
        // Equality-polynomial weights may legitimately be zero when a
        // verifier challenge is one. This mechanical owner checks only the
        // exact vector shape; the future transcript owner must supply the
        // verifier-derived weights without strengthening their predicate.
        if group_weights.len() != SOURCE_OPENING_GROUP_COUNT_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut source_column =
            ZeroizingT256ScalarVecV1::with_capacity(SOURCE_OPENING_SCALARS_PER_GROUP_V1);
        let mut packing_column =
            ZeroizingT256ScalarVecV1::with_capacity(SOURCE_OPENING_SCALARS_PER_GROUP_V1);
        for _ in 0..SOURCE_OPENING_SCALARS_PER_GROUP_V1 {
            source_column.push(Scalar::zero());
            packing_column.push(Scalar::zero());
        }
        Ok(Self {
            group_weights,
            source_column,
            packing_column,
            next_scalar: 0,
        })
    }
}

impl PurposeBoundCanonicalOpeningSinkV1 for WeightedOpeningColumnsSinkV1 {
    type Output = WeightedOpeningColumnsV1;

    fn absorb_next_scalar_v1(
        &mut self,
        scalar: &ZeroizingT256ScalarCopyV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.next_scalar >= SOURCE_OPENING_SCALAR_COUNT_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let group = usize::try_from(self.next_scalar / SOURCE_OPENING_SCALARS_PER_GROUP_V1 as u64)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let source_j =
            usize::try_from(self.next_scalar % SOURCE_OPENING_SCALARS_PER_GROUP_V1 as u64)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let packing_k = source_to_packing_coordinate_v1(source_j)?;
        let weighted =
            ZeroizingT256ScalarCopyV1::new(scalar.get() * self.group_weights.as_slice()[group]);
        self.source_column.as_mut_slice()[source_j] += weighted.get();
        self.packing_column.as_mut_slice()[packing_k] += weighted.get();
        self.next_scalar += 1;
        Ok(())
    }

    fn finish_v1(self) -> Result<Self::Output, ZkAmsMkheErrorV1> {
        if self.next_scalar != SOURCE_OPENING_SCALAR_COUNT_V1
            || self.group_weights.len() != SOURCE_OPENING_GROUP_COUNT_V1
            || self.source_column.len() != SOURCE_OPENING_SCALARS_PER_GROUP_V1
            || self.packing_column.len() != SOURCE_OPENING_SCALARS_PER_GROUP_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(WeightedOpeningColumnsV1 {
            group_weights: self.group_weights,
            source_column: self.source_column,
            packing_column: self.packing_column,
        })
    }
}

struct WeightedOpeningColumnsV1 {
    group_weights: ZeroizingT256ScalarVecV1,
    source_column: ZeroizingT256ScalarVecV1,
    packing_column: ZeroizingT256ScalarVecV1,
}

#[path = "source_openings_v1/canonical_reopen_v1.rs"]
mod canonical_reopen_v1;

pub(in crate::vega::zk_ams::mkhe) use canonical_reopen_v1::Phase23GlobalLookupSourceReopenedV1;

fn map_bulletproof_error_v1(
    _: crate::generalized_bulletproof::GeneralizedBulletproofErrorV1,
) -> ZkAmsMkheErrorV1 {
    ZkAmsMkheErrorV1::InvalidPhase23Fold
}

fn require_nonzero_opening_digest_v1(digest: [u8; 32]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

#[cfg(test)]
#[path = "source_openings_v1_tests.rs"]
mod tests;
