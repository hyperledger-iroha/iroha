use core::convert::Infallible;
use std::path::PathBuf;

use iroha_confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};

use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, sponge::Keccak256};

use super::super::super::super::super::{
    ZkAmsMkheErrorV1, global_lookup_statement_v1::global_lookup_topology_digest_v1,
};
use super::super::{
    PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1, PHASE23_MAIN_BLOCK_BYTES_V1, PHASE23_RECORD_COUNT_V1,
    PHASE23_SIGNED_BLOCKS_PER_WITNESS_V1, phase23_record_position_v1,
};
use super::{Phase23SourceAlgebraPrerequisiteV2, validate_prerequisite_record_v2};

#[path = "global_lookup_source_replay_v1/source_openings_v1.rs"]
mod source_openings_v1;

pub(in super::super) use source_openings_v1::GlobalLookupSourceOpeningEntropySealV1;
pub(in crate::vega::zk_ams::mkhe) use source_openings_v1::{
    GlobalLookupCanonicalReopenSealV1, Phase23GlobalLookupSourceReopenedV1,
};
use source_openings_v1::{GlobalLookupSourceOpeningMaterialV1, SourceOpeningAssemblyV1};

const SOURCE_REPLAY_VERSION_V1: u8 = 1;
const SIGNED_SOURCE_ROLES_V1: usize = 3;
const SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1: usize = 1_024;
const COMPACT_PLANE_COEFFICIENTS_V1: usize = 16_384;
const SOURCE_BLOCKS_PER_COMPACT_PLANE_V1: usize =
    COMPACT_PLANE_COEFFICIENTS_V1 / SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1;
const COMPACT_PLANES_PER_ROLE_V1: usize =
    PHASE23_SIGNED_BLOCKS_PER_WITNESS_V1 / SOURCE_BLOCKS_PER_COMPACT_PLANE_V1;
const COMPACT_PLANES_PER_RECORD_V1: usize = SIGNED_SOURCE_ROLES_V1 * COMPACT_PLANES_PER_ROLE_V1;
const COMPACT_PLANE_COUNT_V1: usize = PHASE23_RECORD_COUNT_V1 * COMPACT_PLANES_PER_RECORD_V1;
const COMPACT_PLANE_BYTES_V1: u64 = COMPACT_PLANE_COEFFICIENTS_V1 as u64;
const AUTHENTICATION_TAG_BYTES_V1: u64 = 16;
const COMPACT_SPOOL_FILE_BYTES_V1: u64 =
    COMPACT_PLANE_COUNT_V1 as u64 * (COMPACT_PLANE_BYTES_V1 + AUTHENTICATION_TAG_BYTES_V1);
const COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1: u64 = 2 * COMPACT_SPOOL_FILE_BYTES_V1;
const CANONICAL_SOURCE_READ_BLOCKS_V1: usize =
    PHASE23_RECORD_COUNT_V1 * PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1;
const SIGNED_SOURCE_READ_BLOCKS_V1: usize =
    PHASE23_RECORD_COUNT_V1 * SIGNED_SOURCE_ROLES_V1 * PHASE23_SIGNED_BLOCKS_PER_WITNESS_V1;
const TOTAL_SOURCE_READ_BLOCKS_V1: usize =
    CANONICAL_SOURCE_READ_BLOCKS_V1 + SIGNED_SOURCE_READ_BLOCKS_V1;
const SOURCE_PLAINTEXT_READ_BYTES_V1: u64 =
    TOTAL_SOURCE_READ_BLOCKS_V1 as u64 * PHASE23_MAIN_BLOCK_BYTES_V1 as u64;
const SOURCE_AUTHENTICATED_READ_BYTES_V1: u64 = TOTAL_SOURCE_READ_BLOCKS_V1 as u64
    * (PHASE23_MAIN_BLOCK_BYTES_V1 as u64 + AUTHENTICATION_TAG_BYTES_V1);
const COMPACT_SPOOL_PLAINTEXT_BYTES_V1: u64 =
    COMPACT_PLANE_COUNT_V1 as u64 * COMPACT_PLANE_BYTES_V1;
const TOTAL_REPLAY_IO_BYTES_V1: u64 =
    SOURCE_AUTHENTICATED_READ_BYTES_V1 + COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1;

const GLOBAL_LOOKUP_TOPOLOGY_KAT_V1: [u8; 32] = [
    0x3a, 0xf9, 0xa6, 0xad, 0x67, 0x38, 0x3c, 0x32, 0xb0, 0x6b, 0xb5, 0xd9, 0x5a, 0x05, 0x86, 0x3b,
    0x8c, 0xb0, 0xb3, 0x33, 0x86, 0x60, 0x17, 0x7b, 0xc2, 0xa9, 0x2e, 0x1b, 0xbf, 0x40, 0xb4, 0xab,
];
const MAPPING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.source-replay.mapping\0";
const CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-replay.spool-context\0";
const AUTHENTICATED_READ_SCHEDULE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.source-replay.authenticated-read-schedule\0";
const RECEIPT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.source-replay.receipt\0";
const COMPACT_ENCODING_V1: &[u8] =
    b"twos-complement-i8;source-i64-be-high-seven-bytes-exact-sign-extension";
const PLANE_ORDER_V1: &[u8] =
    b"slot=((record*3+role-index)*8+plane);role=r,e0,e1;16-source-blocks-per-plane";

const AUTHENTICATED_SOURCE_REPLAY_COMPLETE_V1: bool = true;
const SOURCE_SAME_OPENING_PROVED_V1: bool = false;
const GLOBAL_LOOKUP_PROOF_VERIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false;
const RELEASE_READY_V1: bool = false;
const RELEASE_COMPLETE_V1: bool = false;

const _: () = {
    assert!(SOURCE_BLOCKS_PER_COMPACT_PLANE_V1 == 16);
    assert!(COMPACT_PLANES_PER_ROLE_V1 == 8);
    assert!(COMPACT_PLANES_PER_RECORD_V1 == 24);
    assert!(COMPACT_PLANE_COUNT_V1 == 1_032);
    assert!(CANONICAL_SOURCE_READ_BLOCKS_V1 == 22_016);
    assert!(SIGNED_SOURCE_READ_BLOCKS_V1 == 16_512);
    assert!(TOTAL_SOURCE_READ_BLOCKS_V1 == 38_528);
    assert!(SOURCE_PLAINTEXT_READ_BYTES_V1 == 315_621_376);
    assert!(SOURCE_AUTHENTICATED_READ_BYTES_V1 == 316_237_824);
    assert!(COMPACT_SPOOL_PLAINTEXT_BYTES_V1 == 16_908_288);
    assert!(COMPACT_SPOOL_FILE_BYTES_V1 == 16_924_800);
    assert!(COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1 == 33_849_600);
    assert!(TOTAL_REPLAY_IO_BYTES_V1 == 350_087_424);
    assert!(AUTHENTICATED_SOURCE_REPLAY_COMPLETE_V1);
    assert!(!SOURCE_SAME_OPENING_PROVED_V1);
    assert!(!GLOBAL_LOOKUP_PROOF_VERIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!RELEASE_READY_V1);
    assert!(!RELEASE_COMPLETE_V1);
};

#[repr(u8)]
enum CompactSourceRoleV1 {
    Ephemeral = 1,
    ErrorZero = 2,
    ErrorOne = 3,
}

impl CompactSourceRoleV1 {
    const ALL: [Self; SIGNED_SOURCE_ROLES_V1] = [Self::Ephemeral, Self::ErrorZero, Self::ErrorOne];

    const fn index_v1(&self) -> usize {
        self.tag_v1() as usize - 1
    }

    const fn tag_v1(&self) -> u8 {
        match self {
            Self::Ephemeral => 1,
            Self::ErrorZero => 2,
            Self::ErrorOne => 3,
        }
    }

    const fn bound_v1(&self) -> i8 {
        match self {
            Self::Ephemeral => 1,
            Self::ErrorZero | Self::ErrorOne => 2,
        }
    }
}

struct CompactPlaneCoordinateV1 {
    slot: u16,
    record: u16,
    role: CompactSourceRoleV1,
    plane: u8,
    first_source_block: u16,
}

fn compact_plane_coordinate_v1(slot: usize) -> Result<CompactPlaneCoordinateV1, ZkAmsMkheErrorV1> {
    if slot >= COMPACT_PLANE_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let record = slot / COMPACT_PLANES_PER_RECORD_V1;
    let local = slot % COMPACT_PLANES_PER_RECORD_V1;
    let role = match local / COMPACT_PLANES_PER_ROLE_V1 {
        0 => CompactSourceRoleV1::Ephemeral,
        1 => CompactSourceRoleV1::ErrorZero,
        2 => CompactSourceRoleV1::ErrorOne,
        _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
    };
    let plane = local % COMPACT_PLANES_PER_ROLE_V1;
    Ok(CompactPlaneCoordinateV1 {
        slot: u16::try_from(slot).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        record: u16::try_from(record).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        role,
        plane: u8::try_from(plane).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        first_source_block: u16::try_from(plane * SOURCE_BLOCKS_PER_COMPACT_PLANE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    })
}

fn mapping_digest_for_plane_order_v1(order: &[u16]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if order.len() != COMPACT_PLANE_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let topology_digest = global_lookup_topology_digest_v1();
    if topology_digest != GLOBAL_LOOKUP_TOPOLOGY_KAT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(MAPPING_DOMAIN_V1);
    hash.update(&[SOURCE_REPLAY_VERSION_V1]);
    hash.update(&topology_digest);
    for value in [
        PHASE23_RECORD_COUNT_V1 as u32,
        PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 as u32,
        PHASE23_SIGNED_BLOCKS_PER_WITNESS_V1 as u32,
        SIGNED_SOURCE_ROLES_V1 as u32,
        SOURCE_BLOCKS_PER_COMPACT_PLANE_V1 as u32,
        COMPACT_PLANE_COEFFICIENTS_V1 as u32,
        COMPACT_PLANE_COUNT_V1 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&(COMPACT_ENCODING_V1.len() as u16).to_be_bytes());
    hash.update(COMPACT_ENCODING_V1);
    hash.update(&(PLANE_ORDER_V1.len() as u16).to_be_bytes());
    hash.update(PLANE_ORDER_V1);
    for record in 0..PHASE23_RECORD_COUNT_V1 {
        let position = phase23_record_position_v1(
            u16::try_from(record).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        hash.update(&position.ordinal.to_be_bytes());
        hash.update(&[position.family as u8]);
        hash.update(&position.chunk_index.to_be_bytes());
        hash.update(&position.family_chunk_count.to_be_bytes());
        hash.update(&position.logical_value_count.to_be_bytes());
        for block in 0..PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 {
            hash.update(&[0]);
            hash.update(&position.ordinal.to_be_bytes());
            hash.update(&(block as u16).to_be_bytes());
        }
    }
    let mut seen = [false; COMPACT_PLANE_COUNT_V1];
    for (stream_slot, requested_slot) in order.iter().copied().enumerate() {
        let requested_slot = usize::from(requested_slot);
        if requested_slot >= COMPACT_PLANE_COUNT_V1 || seen[requested_slot] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        seen[requested_slot] = true;
        let coordinate = compact_plane_coordinate_v1(requested_slot)?;
        hash.update(&(stream_slot as u16).to_be_bytes());
        hash.update(&coordinate.slot.to_be_bytes());
        hash.update(&coordinate.record.to_be_bytes());
        hash.update(&[coordinate.role.tag_v1(), coordinate.plane]);
        for local_block in 0..SOURCE_BLOCKS_PER_COMPACT_PLANE_V1 {
            let source_block = usize::from(coordinate.first_source_block) + local_block;
            hash.update(&(source_block as u16).to_be_bytes());
            hash.update(&(local_block as u16).to_be_bytes());
            hash.update(
                &u32::try_from(local_block * SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                    .to_be_bytes(),
            );
        }
    }
    if seen.contains(&false) {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    require_nonzero_v1(hash.finalize())
}

fn exact_mapping_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let order: [u16; COMPACT_PLANE_COUNT_V1] = core::array::from_fn(|index| index as u16);
    mapping_digest_for_plane_order_v1(&order)
}

struct SourceReplayContextAxesV1 {
    source_receipt_digest: [u8; 32],
    prerequisite_record_digest: [u8; 32],
    source_formula_digest: [u8; 32],
    source_mapping_digest: [u8; 32],
    ordered_bundle_root: [u8; 32],
    source_lineage_root: [u8; 32],
    output_lineage_root: [u8; 32],
    preflight_digest: [u8; 32],
    aggregate_schedule_digest: [u8; 32],
}

fn spool_context_digest_v1(
    axes: SourceReplayContextAxesV1,
    plane_mapping_digest: [u8; 32],
    topology_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if topology_digest != GLOBAL_LOOKUP_TOPOLOGY_KAT_V1 || plane_mapping_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut hash = Keccak256::new();
    hash.update(CONTEXT_DOMAIN_V1);
    hash.update(&[SOURCE_REPLAY_VERSION_V1]);
    hash.update(&topology_digest);
    hash.update(&plane_mapping_digest);
    for digest in [
        axes.source_receipt_digest,
        axes.prerequisite_record_digest,
        axes.source_formula_digest,
        axes.source_mapping_digest,
        axes.ordered_bundle_root,
        axes.source_lineage_root,
        axes.output_lineage_root,
        axes.preflight_digest,
        axes.aggregate_schedule_digest,
    ] {
        hash.update(&require_nonzero_v1(digest)?);
    }
    require_nonzero_v1(hash.finalize())
}

pub(in super::super) enum GlobalLookupSourceReplaySinkSealV1 {
    Production {
        confidential_spool_directory: Infallible,
    },
    #[cfg(test)]
    TestOnly(PathBuf),
}

impl GlobalLookupSourceReplaySinkSealV1 {
    fn into_directory_v1(self) -> PathBuf {
        match self {
            Self::Production {
                confidential_spool_directory,
            } => match confidential_spool_directory {},
            #[cfg(test)]
            Self::TestOnly(directory) => directory,
        }
    }
}

struct SourceReplayLiveV1<K, P> {
    prerequisite: Phase23SourceAlgebraPrerequisiteV2<K, P>,
    writer: ConfidentialSpoolWriterV1,
    openings: SourceOpeningAssemblyV1,
    context_digest: [u8; 32],
    topology_digest: [u8; 32],
    plane_mapping_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    authenticated_read_schedule_hash: Keccak256,
    next_record: u16,
    next_canonical_block: u16,
    canonical_complete: bool,
    next_role: u8,
    next_plane: u8,
    next_output_slot: u16,
}

struct SourceReplayAssemblyV1<K, P> {
    live: Option<SourceReplayLiveV1<K, P>>,
}

struct SourceReplayIngressV1<K, P> {
    prerequisite: Option<Phase23SourceAlgebraPrerequisiteV2<K, P>>,
    sink: Option<GlobalLookupSourceReplaySinkSealV1>,
    opening_entropy: Option<GlobalLookupSourceOpeningEntropySealV1>,
}

impl<K, P> SourceReplayIngressV1<K, P> {
    fn begin_v1(mut self) -> Result<SourceReplayAssemblyV1<K, P>, ZkAmsMkheErrorV1> {
        let prerequisite = self
            .prerequisite
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let sink = self
            .sink
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let opening_entropy = self
            .opening_entropy
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_prerequisite_record_v2(&prerequisite.record)?;
        let owner = prerequisite
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        owner.owner.validate_v1()?;
        let source_receipt_digest = owner.owner.source.receipt_v1().receipt_digest_v1();
        let topology_digest = global_lookup_topology_digest_v1();
        let plane_mapping_digest = exact_mapping_digest_v1()?;
        let axes = SourceReplayContextAxesV1 {
            source_receipt_digest,
            prerequisite_record_digest: prerequisite.record.record_digest,
            source_formula_digest: prerequisite.record.formula_digest,
            source_mapping_digest: prerequisite.record.mapping_digest,
            ordered_bundle_root: prerequisite.record.ordered_bundle_root,
            source_lineage_root: prerequisite.record.source_lineage_root,
            output_lineage_root: prerequisite.record.output_lineage_root,
            preflight_digest: prerequisite.record.preflight_digest,
            aggregate_schedule_digest: prerequisite.record.aggregate_schedule_digest,
        };
        let context_digest = spool_context_digest_v1(axes, plane_mapping_digest, topology_digest)?;
        let directory = sink.into_directory_v1();
        let openings = SourceOpeningAssemblyV1::begin_v1(
            source_receipt_digest,
            prerequisite.record.record_digest,
            context_digest,
            opening_entropy,
            &directory,
        )?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            COMPACT_PLANE_COUNT_V1 as u64,
            COMPACT_PLANE_BYTES_V1,
            context_digest,
        )
        .map_err(map_leaf_error_v1)?;
        if layout.slot_count_v1() != COMPACT_PLANE_COUNT_V1 as u64
            || layout.plaintext_len_v1() != COMPACT_PLANE_BYTES_V1
            || layout.file_len_v1() != COMPACT_SPOOL_FILE_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let writer = ConfidentialSpoolWriterV1::create_in_v1(&directory, layout)
            .map_err(map_leaf_error_v1)?;
        let mut authenticated_read_schedule_hash = Keccak256::new();
        authenticated_read_schedule_hash.update(AUTHENTICATED_READ_SCHEDULE_DOMAIN_V1);
        authenticated_read_schedule_hash.update(&[SOURCE_REPLAY_VERSION_V1]);
        authenticated_read_schedule_hash.update(&source_receipt_digest);
        authenticated_read_schedule_hash.update(&topology_digest);
        authenticated_read_schedule_hash.update(&plane_mapping_digest);
        authenticated_read_schedule_hash.update(&context_digest);
        authenticated_read_schedule_hash
            .update(&(TOTAL_SOURCE_READ_BLOCKS_V1 as u32).to_be_bytes());
        Ok(SourceReplayAssemblyV1 {
            live: Some(SourceReplayLiveV1 {
                prerequisite,
                writer,
                openings,
                context_digest,
                topology_digest,
                plane_mapping_digest,
                source_receipt_digest,
                authenticated_read_schedule_hash,
                next_record: 0,
                next_canonical_block: 0,
                canonical_complete: false,
                next_role: 0,
                next_plane: 0,
                next_output_slot: 0,
            }),
        })
    }
}

impl<K, P> SourceReplayAssemblyV1<K, P> {
    fn authenticate_next_canonical_block_v1(
        &mut self,
        record: usize,
        block: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let record =
            u16::try_from(record).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let block = u16::try_from(block).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if record != live.next_record
            || block != live.next_canonical_block
            || live.canonical_complete
            || live.next_role != 0
            || live.next_plane != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let owner = live
            .prerequisite
            .live
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let mut chunk = owner
            .owner
            .source
            .read_canonical_plaintext_block_v1(record, block)?;
        let bytes = chunk.as_mut_bytes_v1();
        validate_canonical_source_block_v1(bytes)?;
        live.openings
            .absorb_next_canonical_block_v1(record, block, bytes)?;
        live.authenticated_read_schedule_hash.update(&[0]);
        live.authenticated_read_schedule_hash
            .update(&record.to_be_bytes());
        live.authenticated_read_schedule_hash
            .update(&block.to_be_bytes());
        live.next_canonical_block = live
            .next_canonical_block
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if usize::from(live.next_canonical_block) == PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 {
            live.canonical_complete = true;
        }
        self.live = Some(live);
        Ok(())
    }

    fn replay_next_signed_plane_v1(&mut self, slot: usize) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let coordinate = compact_plane_coordinate_v1(slot)?;
        if coordinate.slot != live.next_output_slot
            || coordinate.record != live.next_record
            || coordinate.role.index_v1() != usize::from(live.next_role)
            || coordinate.plane != live.next_plane
            || !live.canonical_complete
            || usize::from(live.next_canonical_block) != PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut output = ConfidentialSpoolChunkV1::new_zeroed_v1(COMPACT_PLANE_BYTES_V1)
            .map_err(map_leaf_error_v1)?;
        for local_block in 0..SOURCE_BLOCKS_PER_COMPACT_PLANE_V1 {
            let source_block = usize::from(coordinate.first_source_block) + local_block;
            let source_block = u16::try_from(source_block)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let owner = live
                .prerequisite
                .live
                .as_mut()
                .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            let mut source = match &coordinate.role {
                CompactSourceRoleV1::Ephemeral => owner
                    .owner
                    .source
                    .read_ephemeral_block_v1(coordinate.record, source_block)?,
                CompactSourceRoleV1::ErrorZero => owner
                    .owner
                    .source
                    .read_error_zero_block_v1(coordinate.record, source_block)?,
                CompactSourceRoleV1::ErrorOne => owner
                    .owner
                    .source
                    .read_error_one_block_v1(coordinate.record, source_block)?,
            };
            let source_bytes = source.as_mut_bytes_v1();
            let output_start = local_block * SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1;
            let output_end = output_start + SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1;
            narrow_signed_source_block_v1(
                &coordinate.role,
                source_bytes,
                &mut output.as_mut_slice_v1()[output_start..output_end],
            )?;
            live.authenticated_read_schedule_hash
                .update(&[coordinate.role.tag_v1()]);
            live.authenticated_read_schedule_hash
                .update(&coordinate.record.to_be_bytes());
            live.authenticated_read_schedule_hash
                .update(&source_block.to_be_bytes());
        }
        live.writer
            .write_slot_v1(u64::from(coordinate.slot), output)
            .map_err(map_leaf_error_v1)?;
        live.next_output_slot = live
            .next_output_slot
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        live.next_plane += 1;
        if usize::from(live.next_plane) == COMPACT_PLANES_PER_ROLE_V1 {
            live.next_plane = 0;
            live.next_role += 1;
        }
        if usize::from(live.next_role) == SIGNED_SOURCE_ROLES_V1 {
            live.next_role = 0;
            live.next_record += 1;
            live.next_canonical_block = 0;
            live.canonical_complete = false;
        }
        self.live = Some(live);
        Ok(())
    }

    fn finish_v1(mut self) -> Result<Phase23GlobalLookupSourceReplayV1<K, P>, ZkAmsMkheErrorV1> {
        let live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if usize::from(live.next_record) != PHASE23_RECORD_COUNT_V1
            || live.next_canonical_block != 0
            || live.canonical_complete
            || live.next_role != 0
            || live.next_plane != 0
            || usize::from(live.next_output_slot) != COMPACT_PLANE_COUNT_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        validate_prerequisite_record_v2(&live.prerequisite.record)?;
        let owner = live
            .prerequisite
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        owner.owner.validate_v1()?;
        if owner.owner.source.receipt_v1().receipt_digest_v1() != live.source_receipt_digest
            || global_lookup_topology_digest_v1() != live.topology_digest
            || exact_mapping_digest_v1()? != live.plane_mapping_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let authenticated_read_schedule_root =
            require_nonzero_v1(live.authenticated_read_schedule_hash.finalize())?;
        let openings = live.openings.finish_v1()?;
        let snapshot = live.writer.seal_v1().map_err(map_leaf_error_v1)?;
        if snapshot.slot_count_v1() != COMPACT_PLANE_COUNT_V1 as u64
            || snapshot.plaintext_len_v1() != COMPACT_PLANE_BYTES_V1
            || snapshot.file_len_v1() != COMPACT_SPOOL_FILE_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let snapshot_root = require_nonzero_v1(*snapshot.snapshot_digest_v1())?;
        let mut record = GlobalLookupSourceReplayRecordV1 {
            source_receipt_digest: live.source_receipt_digest,
            prerequisite_record_digest: live.prerequisite.record.record_digest,
            topology_digest: live.topology_digest,
            plane_mapping_digest: live.plane_mapping_digest,
            spool_context_digest: live.context_digest,
            authenticated_read_schedule_root,
            snapshot_root,
            source_read_blocks: TOTAL_SOURCE_READ_BLOCKS_V1 as u32,
            source_plaintext_read_bytes: SOURCE_PLAINTEXT_READ_BYTES_V1,
            source_authenticated_read_bytes: SOURCE_AUTHENTICATED_READ_BYTES_V1,
            output_plane_count: COMPACT_PLANE_COUNT_V1 as u16,
            output_plaintext_bytes: COMPACT_SPOOL_PLAINTEXT_BYTES_V1,
            output_file_bytes: COMPACT_SPOOL_FILE_BYTES_V1,
            output_write_and_seal_read_bytes: COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1,
            total_replay_io_bytes: TOTAL_REPLAY_IO_BYTES_V1,
            authenticated_source_replay_complete: AUTHENTICATED_SOURCE_REPLAY_COMPLETE_V1,
            source_same_opening_proved: SOURCE_SAME_OPENING_PROVED_V1,
            global_lookup_proof_verified: GLOBAL_LOOKUP_PROOF_VERIFIED_V1,
            zero_knowledge_accepted: ZERO_KNOWLEDGE_ACCEPTED_V1,
            operational_receipt_accepted: OPERATIONAL_RECEIPT_ACCEPTED_V1,
            release_ready: RELEASE_READY_V1,
            release_complete: RELEASE_COMPLETE_V1,
            record_digest: [0; 32],
        };
        record.record_digest = replay_record_digest_v1(&record)?;
        validate_replay_record_v1(&record)?;
        Ok(Phase23GlobalLookupSourceReplayV1 {
            prerequisite: live.prerequisite,
            snapshot,
            openings,
            record,
        })
    }

    #[cfg(test)]
    fn panic_after_take_for_test_v1(&mut self) {
        let _live = self.live.take().expect("live source replay assembly");
        panic!("intentional source replay unwind test");
    }
}

struct GlobalLookupSourceReplayRecordV1 {
    source_receipt_digest: [u8; 32],
    prerequisite_record_digest: [u8; 32],
    topology_digest: [u8; 32],
    plane_mapping_digest: [u8; 32],
    spool_context_digest: [u8; 32],
    authenticated_read_schedule_root: [u8; 32],
    snapshot_root: [u8; 32],
    source_read_blocks: u32,
    source_plaintext_read_bytes: u64,
    source_authenticated_read_bytes: u64,
    output_plane_count: u16,
    output_plaintext_bytes: u64,
    output_file_bytes: u64,
    output_write_and_seal_read_bytes: u64,
    total_replay_io_bytes: u64,
    authenticated_source_replay_complete: bool,
    source_same_opening_proved: bool,
    global_lookup_proof_verified: bool,
    zero_knowledge_accepted: bool,
    operational_receipt_accepted: bool,
    release_ready: bool,
    release_complete: bool,
    record_digest: [u8; 32],
}

#[must_use = "dropping this owner closes the compact source and original prerequisite"]
pub(in crate::vega::zk_ams::mkhe) struct Phase23GlobalLookupSourceReplayV1<K, P> {
    prerequisite: Phase23SourceAlgebraPrerequisiteV2<K, P>,
    snapshot: ConfidentialSpoolSnapshotV1,
    openings: GlobalLookupSourceOpeningMaterialV1,
    record: GlobalLookupSourceReplayRecordV1,
}

fn replay_record_digest_v1(
    record: &GlobalLookupSourceReplayRecordV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RECEIPT_DOMAIN_V1);
    hash.update(&[SOURCE_REPLAY_VERSION_V1]);
    for digest in [
        record.source_receipt_digest,
        record.prerequisite_record_digest,
        record.topology_digest,
        record.plane_mapping_digest,
        record.spool_context_digest,
        record.authenticated_read_schedule_root,
        record.snapshot_root,
    ] {
        hash.update(&require_nonzero_v1(digest)?);
    }
    hash.update(&record.source_read_blocks.to_be_bytes());
    hash.update(&record.source_plaintext_read_bytes.to_be_bytes());
    hash.update(&record.source_authenticated_read_bytes.to_be_bytes());
    hash.update(&record.output_plane_count.to_be_bytes());
    hash.update(&record.output_plaintext_bytes.to_be_bytes());
    hash.update(&record.output_file_bytes.to_be_bytes());
    hash.update(&record.output_write_and_seal_read_bytes.to_be_bytes());
    hash.update(&record.total_replay_io_bytes.to_be_bytes());
    hash.update(&[
        record.authenticated_source_replay_complete as u8,
        record.source_same_opening_proved as u8,
        record.global_lookup_proof_verified as u8,
        record.zero_knowledge_accepted as u8,
        record.operational_receipt_accepted as u8,
        record.release_ready as u8,
        record.release_complete as u8,
    ]);
    require_nonzero_v1(hash.finalize())
}

fn validate_replay_record_v1(
    record: &GlobalLookupSourceReplayRecordV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if !record.authenticated_source_replay_complete
        || record.source_same_opening_proved
        || record.global_lookup_proof_verified
        || record.zero_knowledge_accepted
        || record.operational_receipt_accepted
        || record.release_ready
        || record.release_complete
        || record.source_read_blocks != TOTAL_SOURCE_READ_BLOCKS_V1 as u32
        || record.source_plaintext_read_bytes != SOURCE_PLAINTEXT_READ_BYTES_V1
        || record.source_authenticated_read_bytes != SOURCE_AUTHENTICATED_READ_BYTES_V1
        || usize::from(record.output_plane_count) != COMPACT_PLANE_COUNT_V1
        || record.output_plaintext_bytes != COMPACT_SPOOL_PLAINTEXT_BYTES_V1
        || record.output_file_bytes != COMPACT_SPOOL_FILE_BYTES_V1
        || record.output_write_and_seal_read_bytes != COMPACT_SPOOL_WRITE_AND_SEAL_READ_BYTES_V1
        || record.total_replay_io_bytes != TOTAL_REPLAY_IO_BYTES_V1
        || record.record_digest != replay_record_digest_v1(record)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn validate_canonical_source_block_v1(bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    if bytes.len() != PHASE23_MAIN_BLOCK_BYTES_V1
        || !bytes
            .len()
            .is_multiple_of(VEGA_T256_SCALAR_MODULUS_BE_V1.len())
        || bytes
            .chunks_exact(VEGA_T256_SCALAR_MODULUS_BE_V1.len())
            .any(|encoded| encoded >= VEGA_T256_SCALAR_MODULUS_BE_V1.as_slice())
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn narrow_signed_source_block_v1(
    role: &CompactSourceRoleV1,
    source: &[u8],
    output: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    if source.len() != PHASE23_MAIN_BLOCK_BYTES_V1
        || output.len() != SIGNED_SOURCE_COEFFICIENTS_PER_BLOCK_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for (encoded, compact) in source.chunks_exact(8).zip(output) {
        let low = encoded[7];
        let sign_extension = if low & 0x80 == 0 { 0x00 } else { 0xff };
        if encoded[..7].iter().any(|byte| *byte != sign_extension) {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let value = low as i8;
        let bound = role.bound_v1();
        if value < -bound || value > bound {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        *compact = low;
    }
    Ok(())
}

fn map_leaf_error_v1(_: iroha_confidential_spool::ConfidentialSpoolErrorV1) -> ZkAmsMkheErrorV1 {
    ZkAmsMkheErrorV1::InvalidPhase23Fold
}

fn require_nonzero_v1(digest: [u8; 32]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

pub(super) fn replay_global_lookup_source_v1<K, P>(
    prerequisite: Phase23SourceAlgebraPrerequisiteV2<K, P>,
    sink: GlobalLookupSourceReplaySinkSealV1,
    opening_entropy: GlobalLookupSourceOpeningEntropySealV1,
) -> Result<Phase23GlobalLookupSourceReplayV1<K, P>, ZkAmsMkheErrorV1> {
    let mut assembly = SourceReplayIngressV1 {
        prerequisite: Some(prerequisite),
        sink: Some(sink),
        opening_entropy: Some(opening_entropy),
    }
    .begin_v1()?;
    for record in 0..PHASE23_RECORD_COUNT_V1 {
        for block in 0..PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 {
            assembly.authenticate_next_canonical_block_v1(record, block)?;
        }
        for role in 0..SIGNED_SOURCE_ROLES_V1 {
            for plane in 0..COMPACT_PLANES_PER_ROLE_V1 {
                let slot = record * COMPACT_PLANES_PER_RECORD_V1
                    + role * COMPACT_PLANES_PER_ROLE_V1
                    + plane;
                assembly.replay_next_signed_plane_v1(slot)?;
            }
        }
    }
    assembly.finish_v1()
}

#[cfg(test)]
#[path = "global_lookup_source_replay_v1_tests.rs"]
mod tests;
