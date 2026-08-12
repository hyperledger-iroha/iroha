//! Authenticated, single-pass replay of the qPCS `S` spool into global lookup.
//!
//! This module deliberately has no inhabited production authority yet.  It owns
//! the exact replay schedule and binding now, while the later lookup-plane seal
//! remains the only place that may make the production entry point reachable.

use core::{convert::Infallible, mem::size_of};

use super::*;

const GLOBAL_LOOKUP_S_REPLAY_PURPOSE_V1: &[u8] = b"iroha.vega.mkhe.global_lookup.q_pcs_s_replay.v1";
const GLOBAL_LOOKUP_S_REPLAY_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.vega.mkhe.global_lookup.q_pcs_s_replay.binding.v1";
const GLOBAL_LOOKUP_S_REPLAY_MAPPING_DOMAIN_V1: &[u8] =
    b"iroha.vega.mkhe.global_lookup.q_pcs_s_replay.mapping.v1";

const GLOBAL_LOOKUP_S_REPETITIONS_V1: usize = 5;
const GLOBAL_LOOKUP_S_BLOCKS_PER_GROUP_V1: usize = 16;
const GLOBAL_LOOKUP_S_DIGITS_V1: usize = 4;
const GLOBAL_LOOKUP_S_PLANES_PER_TUPLE_V1: usize = 8;
const GLOBAL_LOOKUP_S_RADIX_BITS_V1: u32 = 15;
const GLOBAL_LOOKUP_S_RADIX_MASK_V1: u64 = (1_u64 << GLOBAL_LOOKUP_S_RADIX_BITS_V1) - 1;
const GLOBAL_LOOKUP_S_MAX_VALUE_EXCLUSIVE_V1: u64 = 1_u64 << 60;
const GLOBAL_LOOKUP_TOPOLOGY_DIGEST_V1: [u8; 32] = [
    0x2d, 0x1d, 0xcc, 0x86, 0xa7, 0xc5, 0x8d, 0x99, 0xa7, 0x29, 0xdf, 0x30, 0xb5, 0xc4, 0x8d, 0x30,
    0x82, 0xce, 0xa1, 0xe4, 0x70, 0x60, 0x68, 0xee, 0xdf, 0x6c, 0x6e, 0xa5, 0xae, 0xa5, 0x67, 0xa6,
];

const GLOBAL_LOOKUP_S_RELEASE_RELATIONS_V1: u64 =
    RELEASE_LIMB_COUNT_V2 as u64 * GLOBAL_LOOKUP_S_REPETITIONS_V1 as u64;
const GLOBAL_LOOKUP_S_RELEASE_GROUPS_PER_RELATION_V1: u64 =
    RELEASE_COEFFICIENT_BLOCKS_PER_COMPONENT_V2 / GLOBAL_LOOKUP_S_BLOCKS_PER_GROUP_V1 as u64;
const GLOBAL_LOOKUP_S_RELEASE_Q_MASK_BLOCKS_V1: u64 =
    GLOBAL_LOOKUP_S_RELEASE_RELATIONS_V1 * GLOBAL_LOOKUP_S_RELEASE_GROUPS_PER_RELATION_V1;
const GLOBAL_LOOKUP_S_RELEASE_SLOTS_V1: u64 =
    GLOBAL_LOOKUP_S_RELEASE_RELATIONS_V1 * RELEASE_COEFFICIENT_BLOCKS_PER_COMPONENT_V2;
const GLOBAL_LOOKUP_S_RELEASE_TUPLES_V1: u64 =
    GLOBAL_LOOKUP_S_RELEASE_SLOTS_V1 * RELEASE_COEFFICIENT_VALUES_PER_BLOCK_V2 as u64;
const GLOBAL_LOOKUP_S_RELEASE_PLANE_VALUES_V1: u64 =
    GLOBAL_LOOKUP_S_RELEASE_TUPLES_V1 * GLOBAL_LOOKUP_S_PLANES_PER_TUPLE_V1 as u64;
const GLOBAL_LOOKUP_S_RELEASE_PLAINTEXT_BYTES_V1: u64 =
    GLOBAL_LOOKUP_S_RELEASE_TUPLES_V1 * size_of::<u64>() as u64;
const GLOBAL_LOOKUP_S_RELEASE_REPLAY_FILE_BYTES_V1: u64 = GLOBAL_LOOKUP_S_RELEASE_SLOTS_V1
    * (RELEASE_COEFFICIENT_BLOCK_BYTES_V2 + AUTHENTICATION_TAG_BYTES_V2);
const GLOBAL_LOOKUP_S_RELEASE_TOTAL_IO_BYTES_V1: u64 =
    RELEASE_MASK_S_TOTAL_IO_BYTES_V2 + GLOBAL_LOOKUP_S_RELEASE_REPLAY_FILE_BYTES_V1;
const GLOBAL_LOOKUP_S_RELEASE_TRANSIENT_HEAP_BYTES_V1: u64 = RELEASE_COEFFICIENT_BLOCK_BYTES_V2;
const GLOBAL_LOOKUP_S_PROOF_COMPLETE_V1: bool = false;
const GLOBAL_LOOKUP_S_ZERO_KNOWLEDGE_BOUND_V1: bool = false;
const GLOBAL_LOOKUP_S_OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false;
const GLOBAL_LOOKUP_S_MEASURED_RSS_WITHIN_CAP_V1: bool = false;
const GLOBAL_LOOKUP_S_RELEASE_READY_V1: bool = false;
const GLOBAL_LOOKUP_S_RELEASE_COMPLETE_V1: bool = false;

const _: () = {
    assert!(!GLOBAL_LOOKUP_S_PROOF_COMPLETE_V1);
    assert!(!GLOBAL_LOOKUP_S_ZERO_KNOWLEDGE_BOUND_V1);
    assert!(!GLOBAL_LOOKUP_S_OPERATIONAL_RECEIPT_ACCEPTED_V1);
    assert!(!GLOBAL_LOOKUP_S_MEASURED_RSS_WITHIN_CAP_V1);
    assert!(!GLOBAL_LOOKUP_S_RELEASE_READY_V1);
    assert!(!GLOBAL_LOOKUP_S_RELEASE_COMPLETE_V1);
};

struct GlobalLookupSReplayAuthorityV1 {
    _upstream_lookup_plane_seal: Infallible,
}

struct GlobalLookupSReplayBindingV1 {
    digest: [u8; 32],
    mapping_digest: [u8; 32],
    snapshot_digest: [u8; 32],
    slot_count: u64,
    tuple_count: u64,
}

trait GlobalLookupSTupleSinkV1 {
    fn begin_v1(
        &mut self,
        binding: &GlobalLookupSReplayBindingV1,
    ) -> Result<(), ProverPrerequisiteErrorV2>;

    fn absorb_next_v1(
        &mut self,
        digits: [u16; GLOBAL_LOOKUP_S_DIGITS_V1],
        complement_digits: [u16; GLOBAL_LOOKUP_S_DIGITS_V1],
    ) -> Result<(), ProverPrerequisiteErrorV2>;

    fn finish_v1(
        &mut self,
        binding: &GlobalLookupSReplayBindingV1,
    ) -> Result<(), ProverPrerequisiteErrorV2>;
}

struct GlobalLookupSReplayAxesV1 {
    parameter_digest: [u8; 32],
    sealed_source_transcript_digest: [u8; 32],
    source_algebra_binding_digest: [u8; 32],
    initial_root: [u8; 32],
    quotient_root: [u8; 32],
    topology_digest: [u8; 32],
}

struct GlobalLookupSCoordinateV1 {
    slot: u64,
    limb: u32,
    repetition: u32,
    group: u32,
    block_in_group: u32,
    first_coefficient: u64,
}

impl QuotientRootPreparedV2 {
    #[allow(dead_code)]
    fn replay_global_lookup_s_v1<S: GlobalLookupSTupleSinkV1>(
        mut self,
        authority: GlobalLookupSReplayAuthorityV1,
        sink: &mut S,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        // Move the sole S owner out first.  Every later failure or unwind drops
        // either this value or the replay reader and can never return `self`.
        let masks = self
            .masks
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let GlobalLookupSReplayAuthorityV1 {
            _upstream_lookup_plane_seal,
        } = authority;
        match _upstream_lookup_plane_seal {}

        #[allow(unreachable_code)]
        {
            self.require_global_lookup_s_owner_v1()?;
            let geometry = SpoolGeometryV2::release_v2();
            let axes = GlobalLookupSReplayAxesV1 {
                parameter_digest: self.parameter_digest,
                sealed_source_transcript_digest: self.context.sealed_source_transcript_digest,
                source_algebra_binding_digest: self.context.source_algebra_binding_digest,
                initial_root: self.initial_root,
                quotient_root: self.quotient_root,
                topology_digest: crate::vega::zk_ams::mkhe::global_lookup_statement_v1::global_lookup_topology_digest_v1(),
            };
            let masks = replay_mask_v1(masks, geometry, axes, sink, true)?;
            self.masks = Some(masks);
            Ok(self)
        }
    }

    fn require_global_lookup_s_owner_v1(&self) -> Result<(), ProverPrerequisiteErrorV2> {
        if self.accepted_c0.is_none()
            || self.accepted_cq.is_none()
            || self.transcript.is_none()
            || self.masks.is_some()
            || self.parameter_digest == [0_u8; 32]
            || self.initial_root == [0_u8; 32]
            || self.quotient_root == [0_u8; 32]
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        self.context.validate_v2()?;
        Ok(())
    }
}

fn replay_mask_v1<S: GlobalLookupSTupleSinkV1>(
    masks: MaskSpoolSealedV2,
    geometry: SpoolGeometryV2,
    axes: GlobalLookupSReplayAxesV1,
    sink: &mut S,
    require_release: bool,
) -> Result<MaskSpoolSealedV2, ProverPrerequisiteErrorV2> {
    geometry.validate_v2()?;
    let expected_parameter_digest = parameter_digest_v2(geometry)?;
    let slot_count = slot_count_v1(geometry)?;
    let tuple_count = tuple_count_v1(geometry, slot_count)?;
    if expected_parameter_digest != axes.parameter_digest
        || axes.sealed_source_transcript_digest == [0_u8; 32]
        || axes.source_algebra_binding_digest == [0_u8; 32]
        || axes.initial_root == [0_u8; 32]
        || axes.quotient_root == [0_u8; 32]
        || axes.topology_digest == [0_u8; 32]
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    if require_release {
        require_release_accounting_v1(geometry, slot_count, tuple_count)?;
    }

    let snapshot_digest = masks.snapshot_digest_v2()?;
    let mapping_digest = mapping_digest_v1(geometry, slot_count)?;
    let binding = binding_v1(
        &axes,
        snapshot_digest,
        mapping_digest,
        slot_count,
        tuple_count,
    );
    sink.begin_v1(&binding)?;

    let mut reader = masks.begin_replay_v2()?;
    let mut seen_slots = 0_u64;
    let mut seen_tuples = 0_u64;
    for slot in 0..slot_count {
        let coordinate = coordinate_v1(geometry, slot_count, slot)?;
        require_coordinate_v1(geometry, slot, &coordinate)?;
        let chunk = reader.read_next_block_v2()?;
        if chunk.bytes_v2().len()
            != usize::try_from(geometry.coefficient_block_bytes_v2()?)
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?
        {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        let modulus = geometry.moduli[coordinate.limb as usize];
        let mut words = chunk.bytes_v2().chunks_exact(size_of::<u64>());
        for (coefficient, encoded) in (&mut words).enumerate() {
            let value = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
            );
            if coordinate
                .first_coefficient
                .checked_add(coefficient as u64)
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?
                + 1
                == u64::from(geometry.ring_degree)
                && value != 0
            {
                return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
            }
            let (digits, complement_digits) = radix_tuple_v1(value, modulus)?;
            sink.absorb_next_v1(digits, complement_digits)?;
            seen_tuples = seen_tuples
                .checked_add(1)
                .ok_or(ProverPrerequisiteErrorV2::InvalidSourceShape)?;
        }
        if !words.remainder().is_empty() {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        seen_slots = seen_slots
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::InvalidSourceShape)?;
    }
    require_completion_v1(slot_count, tuple_count, seen_slots, seen_tuples)?;
    let masks = reader.complete_v2()?;
    sink.finish_v1(&binding)?;
    Ok(masks)
}

fn slot_count_v1(geometry: SpoolGeometryV2) -> Result<u64, ProverPrerequisiteErrorV2> {
    (geometry.moduli.len() as u64)
        .checked_mul(GLOBAL_LOOKUP_S_REPETITIONS_V1 as u64)
        .and_then(|count| count.checked_mul(geometry.coefficient_blocks_per_component_v2().ok()?))
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)
}

fn tuple_count_v1(
    geometry: SpoolGeometryV2,
    slot_count: u64,
) -> Result<u64, ProverPrerequisiteErrorV2> {
    slot_count
        .checked_mul(u64::from(geometry.coefficient_values_per_block))
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)
}

fn coordinate_v1(
    geometry: SpoolGeometryV2,
    slot_count: u64,
    slot: u64,
) -> Result<GlobalLookupSCoordinateV1, ProverPrerequisiteErrorV2> {
    if slot >= slot_count {
        return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
    }
    let blocks_per_relation = geometry.coefficient_blocks_per_component_v2()?;
    if blocks_per_relation == 0 {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let relation = slot / blocks_per_relation;
    let block = slot % blocks_per_relation;
    Ok(GlobalLookupSCoordinateV1 {
        slot,
        limb: u32::try_from(relation / GLOBAL_LOOKUP_S_REPETITIONS_V1 as u64)
            .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
        repetition: u32::try_from(relation % GLOBAL_LOOKUP_S_REPETITIONS_V1 as u64)
            .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
        group: u32::try_from(block / GLOBAL_LOOKUP_S_BLOCKS_PER_GROUP_V1 as u64)
            .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
        block_in_group: u32::try_from(block % GLOBAL_LOOKUP_S_BLOCKS_PER_GROUP_V1 as u64)
            .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
        first_coefficient: block
            .checked_mul(u64::from(geometry.coefficient_values_per_block))
            .ok_or(ProverPrerequisiteErrorV2::InvalidSourceShape)?,
    })
}

fn require_coordinate_v1(
    geometry: SpoolGeometryV2,
    expected_slot: u64,
    coordinate: &GlobalLookupSCoordinateV1,
) -> Result<(), ProverPrerequisiteErrorV2> {
    let blocks_per_relation = geometry.coefficient_blocks_per_component_v2()?;
    let expected_relation = expected_slot / blocks_per_relation;
    let expected_block = expected_slot % blocks_per_relation;
    if coordinate.slot != expected_slot
        || coordinate.limb as u64 != expected_relation / GLOBAL_LOOKUP_S_REPETITIONS_V1 as u64
        || coordinate.repetition as u64 != expected_relation % GLOBAL_LOOKUP_S_REPETITIONS_V1 as u64
        || coordinate.group as u64 != expected_block / GLOBAL_LOOKUP_S_BLOCKS_PER_GROUP_V1 as u64
        || coordinate.block_in_group as u64
            != expected_block % GLOBAL_LOOKUP_S_BLOCKS_PER_GROUP_V1 as u64
        || coordinate.first_coefficient
            != expected_block * u64::from(geometry.coefficient_values_per_block)
    {
        return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
    }
    Ok(())
}

fn radix_tuple_v1(
    value: u64,
    modulus: u64,
) -> Result<([u16; 4], [u16; 4]), ProverPrerequisiteErrorV2> {
    if modulus == 0 || modulus >= GLOBAL_LOOKUP_S_MAX_VALUE_EXCLUSIVE_V1 || value >= modulus {
        return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
    }
    let complement = modulus
        .checked_sub(1)
        .and_then(|top| top.checked_sub(value))
        .ok_or(ProverPrerequisiteErrorV2::NonCanonicalResidue)?;
    Ok((radix_digits_v1(value), radix_digits_v1(complement)))
}

fn radix_digits_v1(value: u64) -> [u16; GLOBAL_LOOKUP_S_DIGITS_V1] {
    let mut digits = [0_u16; GLOBAL_LOOKUP_S_DIGITS_V1];
    let mut remaining = value;
    for digit in &mut digits {
        *digit = (remaining & GLOBAL_LOOKUP_S_RADIX_MASK_V1) as u16;
        remaining >>= GLOBAL_LOOKUP_S_RADIX_BITS_V1;
    }
    digits
}

fn mapping_digest_v1(
    geometry: SpoolGeometryV2,
    slot_count: u64,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(GLOBAL_LOOKUP_S_REPLAY_MAPPING_DOMAIN_V1);
    hash.update(&(GLOBAL_LOOKUP_S_REPETITIONS_V1 as u64).to_be_bytes());
    hash.update(&(GLOBAL_LOOKUP_S_BLOCKS_PER_GROUP_V1 as u64).to_be_bytes());
    hash.update(&(GLOBAL_LOOKUP_S_DIGITS_V1 as u64).to_be_bytes());
    hash.update(&(GLOBAL_LOOKUP_S_RADIX_BITS_V1 as u64).to_be_bytes());
    hash.update(&slot_count.to_be_bytes());
    hash.update(&tuple_count_v1(geometry, slot_count)?.to_be_bytes());
    for slot in 0..slot_count {
        let coordinate = coordinate_v1(geometry, slot_count, slot)?;
        hash.update(&coordinate.slot.to_be_bytes());
        hash.update(&coordinate.limb.to_be_bytes());
        hash.update(&coordinate.repetition.to_be_bytes());
        hash.update(&coordinate.group.to_be_bytes());
        hash.update(&coordinate.block_in_group.to_be_bytes());
        hash.update(&coordinate.first_coefficient.to_be_bytes());
    }
    Ok(hash.finalize().into())
}

fn binding_v1(
    axes: &GlobalLookupSReplayAxesV1,
    snapshot_digest: [u8; 32],
    mapping_digest: [u8; 32],
    slot_count: u64,
    tuple_count: u64,
) -> GlobalLookupSReplayBindingV1 {
    let mut hash = Keccak256::new();
    hash.update(GLOBAL_LOOKUP_S_REPLAY_BINDING_DOMAIN_V1);
    hash.update(GLOBAL_LOOKUP_S_REPLAY_PURPOSE_V1);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&axes.parameter_digest);
    hash.update(&axes.sealed_source_transcript_digest);
    hash.update(&axes.source_algebra_binding_digest);
    hash.update(&snapshot_digest);
    hash.update(&axes.initial_root);
    hash.update(&axes.quotient_root);
    hash.update(&axes.topology_digest);
    hash.update(&mapping_digest);
    hash.update(&slot_count.to_be_bytes());
    hash.update(&tuple_count.to_be_bytes());
    GlobalLookupSReplayBindingV1 {
        digest: hash.finalize().into(),
        mapping_digest,
        snapshot_digest,
        slot_count,
        tuple_count,
    }
}

fn require_completion_v1(
    expected_slots: u64,
    expected_tuples: u64,
    seen_slots: u64,
    seen_tuples: u64,
) -> Result<(), ProverPrerequisiteErrorV2> {
    if seen_slots != expected_slots || seen_tuples != expected_tuples {
        return Err(ProverPrerequisiteErrorV2::MissingRelations);
    }
    Ok(())
}

fn require_release_accounting_v1(
    geometry: SpoolGeometryV2,
    slot_count: u64,
    tuple_count: u64,
) -> Result<(), ProverPrerequisiteErrorV2> {
    if GLOBAL_LOOKUP_S_RELEASE_RELATIONS_V1 != 190
        || GLOBAL_LOOKUP_S_RELEASE_GROUPS_PER_RELATION_V1 != 8
        || GLOBAL_LOOKUP_S_RELEASE_Q_MASK_BLOCKS_V1 != 1_520
        || GLOBAL_LOOKUP_S_RELEASE_SLOTS_V1 != 24_320
        || GLOBAL_LOOKUP_S_RELEASE_TUPLES_V1 != 24_903_680
        || GLOBAL_LOOKUP_S_RELEASE_PLANE_VALUES_V1 != 199_229_440
        || GLOBAL_LOOKUP_S_RELEASE_PLAINTEXT_BYTES_V1 != 199_229_440
        || GLOBAL_LOOKUP_S_RELEASE_REPLAY_FILE_BYTES_V1 != 199_618_560
        || crate::vega::zk_ams::mkhe::global_lookup_statement_v1::global_lookup_topology_digest_v1()
            != GLOBAL_LOOKUP_TOPOLOGY_DIGEST_V1
        || GLOBAL_LOOKUP_S_REPETITIONS_V1 != usize::from(OPENING_REPETITIONS_V2)
        || geometry.moduli.len() != usize::from(RELEASE_LIMB_COUNT_V2)
        || geometry.coefficient_blocks_per_component_v2()?
            != RELEASE_COEFFICIENT_BLOCKS_PER_COMPONENT_V2
        || geometry.coefficient_values_per_block != RELEASE_COEFFICIENT_VALUES_PER_BLOCK_V2
        || slot_count != GLOBAL_LOOKUP_S_RELEASE_SLOTS_V1
        || tuple_count != GLOBAL_LOOKUP_S_RELEASE_TUPLES_V1
    {
        return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
    }
    Ok(())
}

#[cfg(test)]
#[path = "global_lookup_s_replay_v1_tests.rs"]
mod tests;
