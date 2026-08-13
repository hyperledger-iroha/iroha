//! Authenticated fixed-width retention for the qPCS masking polynomials.
//!
//! Every sampled `S` row is written directly from its sole secret owner before
//! that owner enters the five-row reuse guard.  The stored row has exactly `N`
//! canonical residues and authenticates `S[N - 1] = 0`.  The sealed store is
//! move-only and exposes only a sequential, purpose-bound replay.

use core::sync::atomic;
use std::path::Path;

use iroha_confidential_spool::{
    CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1, CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1,
    CONFIDENTIAL_SPOOL_MAX_SLOTS_V1, ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1,
    ConfidentialSpoolSnapshotV1, ConfidentialSpoolWriterV1,
};

use crate::vega::sponge::Keccak256;

use super::*;

const MASK_SPOOL_MAPPING_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.mask-s-spool.mapping\0";
const MASK_SPOOL_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.mask-s-spool.context\0";
const MASK_SPOOL_FORMULA_V2: &[u8] =
    b"relation=limb*5+repetition;slot=relation*blocks_per_row+block;first_index=block*values_per_block;S[N-1]=0";
const MASK_SPOOL_ENCODING_V2: &[u8] =
    b"canonical big-endian u64 residues;fixed N coefficients;authenticated top zero";
const RELEASE_MASK_S_SLOTS_V2: u64 = 24_320;
const RELEASE_MASK_S_FILE_BYTES_V2: u64 = 199_618_560;
const RELEASE_MASK_S_RELATIONS_V2: u64 = 190;
const RELEASE_MASK_S_SECRET_VALUES_V2: u64 = 24_903_490;
const RELEASE_MASK_S_STORED_VALUES_V2: u64 = 24_903_680;
const RELEASE_MASK_S_WRITE_BYTES_V2: u64 = RELEASE_MASK_S_FILE_BYTES_V2;
const RELEASE_MASK_S_SEAL_READ_BYTES_V2: u64 = RELEASE_MASK_S_FILE_BYTES_V2;
const RELEASE_MASK_S_TOTAL_IO_BYTES_V2: u64 = 399_237_120;
const RELEASE_MASK_S_TRANSIENT_CHUNK_HEAP_BYTES_V2: u64 = 8_192;
const MASK_S_REPLAY_BOUND_V2: bool = true;
const CROSS_FIELD_MASK_PROOF_COMPLETE_V2: bool = false;

#[cfg(test)]
static MASK_REPLAY_CHUNK_ZEROIZED_DROPS_V2: atomic::AtomicUsize = atomic::AtomicUsize::new(0);

const _: () = {
    assert!(
        RELEASE_MASK_S_SLOTS_V2
            == u64::from(RELEASE_LIMB_COUNT_V2)
                * u64::from(OPENING_REPETITIONS_V2)
                * RELEASE_COEFFICIENT_BLOCKS_PER_COMPONENT_V2
    );
    assert!(
        RELEASE_MASK_S_FILE_BYTES_V2
            == RELEASE_MASK_S_SLOTS_V2
                * (RELEASE_COEFFICIENT_BLOCK_BYTES_V2 + AUTHENTICATION_TAG_BYTES_V2)
    );
    assert!(RELEASE_MASK_S_RELATIONS_V2 == 38 * 5);
    assert!(RELEASE_MASK_S_SECRET_VALUES_V2 == RELEASE_MASK_S_RELATIONS_V2 * (131_072 - 1));
    assert!(RELEASE_MASK_S_STORED_VALUES_V2 == RELEASE_MASK_S_RELATIONS_V2 * 131_072);
    assert!(RELEASE_MASK_S_WRITE_BYTES_V2 == RELEASE_MASK_S_FILE_BYTES_V2);
    assert!(RELEASE_MASK_S_SEAL_READ_BYTES_V2 == RELEASE_MASK_S_FILE_BYTES_V2);
    assert!(
        RELEASE_MASK_S_TOTAL_IO_BYTES_V2
            == RELEASE_MASK_S_WRITE_BYTES_V2 + RELEASE_MASK_S_SEAL_READ_BYTES_V2
    );
    assert!(RELEASE_MASK_S_TRANSIENT_CHUNK_HEAP_BYTES_V2 == RELEASE_COEFFICIENT_BLOCK_BYTES_V2);
    assert!(MASK_S_REPLAY_BOUND_V2);
    assert!(!CROSS_FIELD_MASK_PROOF_COMPLETE_V2);
};

#[derive(Clone, Copy, PartialEq, Eq)]
struct MaskSpoolDescriptorV2 {
    relations: u16,
    blocks_per_row: u64,
    slot_count: u64,
    plaintext_bytes: u64,
    file_bytes: u64,
    mapping_digest: [u8; 32],
}

fn mask_spool_descriptor_v2(
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
) -> Result<MaskSpoolDescriptorV2, ProverPrerequisiteErrorV2> {
    geometry.validate_v2()?;
    if parameter_digest_v2(geometry)? != parameter_digest {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    let relations = u16::from(geometry.limb_count_v2()?)
        .checked_mul(u16::from(OPENING_REPETITIONS_V2))
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let blocks_per_row = geometry.coefficient_blocks_per_component_v2()?;
    let slot_count = u64::from(relations)
        .checked_mul(blocks_per_row)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let plaintext_bytes = geometry.coefficient_block_bytes_v2()?;
    let file_bytes = slot_count
        .checked_mul(
            plaintext_bytes
                .checked_add(AUTHENTICATION_TAG_BYTES_V2)
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    if slot_count > CONFIDENTIAL_SPOOL_MAX_SLOTS_V1
        || plaintext_bytes > CONFIDENTIAL_SPOOL_MAX_PLAINTEXT_BYTES_V1
        || file_bytes > CONFIDENTIAL_SPOOL_MAX_FILE_BYTES_V1
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let release = SpoolGeometryV2::release_v2();
    if geometry.ring_degree == release.ring_degree
        && geometry.domain_log == release.domain_log
        && geometry.query_count == release.query_count
        && geometry.coefficient_values_per_block == release.coefficient_values_per_block
        && geometry.moduli == release.moduli
        && (slot_count != RELEASE_MASK_S_SLOTS_V2 || file_bytes != RELEASE_MASK_S_FILE_BYTES_V2)
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let mut hash = Keccak256::new();
    hash.update(MASK_SPOOL_MAPPING_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&geometry.ring_degree.to_be_bytes());
    hash.update(&[geometry.limb_count_v2()?, OPENING_REPETITIONS_V2]);
    hash.update(&geometry.coefficient_values_per_block.to_be_bytes());
    hash.update(&blocks_per_row.to_be_bytes());
    hash.update(&slot_count.to_be_bytes());
    hash.update(&plaintext_bytes.to_be_bytes());
    hash.update(&file_bytes.to_be_bytes());
    hash.update(MASK_SPOOL_FORMULA_V2);
    hash.update(MASK_SPOOL_ENCODING_V2);
    hash.update(&slot_count.to_be_bytes());
    for slot in 0..slot_count {
        let relation = slot / blocks_per_row;
        let block = slot % blocks_per_row;
        hash.update(&slot.to_be_bytes());
        hash.update(&[
            u8::try_from(relation / u64::from(OPENING_REPETITIONS_V2))
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            u8::try_from(relation % u64::from(OPENING_REPETITIONS_V2))
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
        ]);
        hash.update(&block.to_be_bytes());
        hash.update(&(block * u64::from(geometry.coefficient_values_per_block)).to_be_bytes());
    }
    let mapping_digest = hash.finalize();
    if mapping_digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    Ok(MaskSpoolDescriptorV2 {
        relations,
        blocks_per_row,
        slot_count,
        plaintext_bytes,
        file_bytes,
        mapping_digest,
    })
}

fn mask_spool_context_v2(
    parameter_digest: [u8; 32],
    descriptor: MaskSpoolDescriptorV2,
    context: PublicSpoolContextV2,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    context.validate_v2()?;
    let mut hash = Keccak256::new();
    hash.update(MASK_SPOOL_CONTEXT_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&descriptor.mapping_digest);
    hash.update(&context.sealed_source_transcript_digest);
    hash.update(&context.source_algebra_binding_digest);
    hash.update(MASK_SAMPLE_DOMAIN_V2);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    Ok(digest)
}

struct LiveMaskSpoolWriterV2 {
    writer: ConfidentialSpoolWriterV1,
    next_relation: u16,
}

pub(super) struct MaskSpoolWriterV2 {
    live: Option<LiveMaskSpoolWriterV2>,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    descriptor: MaskSpoolDescriptorV2,
    context: PublicSpoolContextV2,
    context_digest: [u8; 32],
}

impl MaskSpoolWriterV2 {
    pub(super) fn create_v2(
        directory: &Path,
        geometry: SpoolGeometryV2,
        parameter_digest: [u8; 32],
        context: PublicSpoolContextV2,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let descriptor = mask_spool_descriptor_v2(geometry, parameter_digest)?;
        let context_digest = mask_spool_context_v2(parameter_digest, descriptor, context)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            descriptor.slot_count,
            descriptor.plaintext_bytes,
            context_digest,
        )?;
        if layout.file_len_v1() != descriptor.file_bytes {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
        }
        let writer = ConfidentialSpoolWriterV1::create_in_v1(directory, layout)?;
        Ok(Self {
            live: Some(LiveMaskSpoolWriterV2 {
                writer,
                next_relation: 0,
            }),
            geometry,
            parameter_digest,
            descriptor,
            context,
            context_digest,
        })
    }

    pub(super) fn push_next_mask_v2(
        &mut self,
        limb: u8,
        repetition: u8,
        mask: &SecretResiduesV2,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if live.next_relation >= self.descriptor.relations {
            return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
        }
        let expected_limb = u8::try_from(live.next_relation / u16::from(OPENING_REPETITIONS_V2))
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let expected_repetition =
            u8::try_from(live.next_relation % u16::from(OPENING_REPETITIONS_V2))
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let ring_degree = usize::try_from(self.geometry.ring_degree)
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if limb != expected_limb
            || repetition != expected_repetition
            || mask.as_slice_v2().len() + 1 != ring_degree
        {
            return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
        }
        let modulus = self.geometry.moduli[usize::from(limb)];
        if mask.as_slice_v2().iter().any(|value| *value >= modulus) {
            return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
        }
        let values_per_block = usize::from(self.geometry.coefficient_values_per_block);
        for block in 0..self.descriptor.blocks_per_row {
            let first = usize::try_from(block)
                .ok()
                .and_then(|value| value.checked_mul(values_per_block))
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
            let mut chunk =
                ConfidentialSpoolChunkV1::new_zeroed_v1(self.descriptor.plaintext_bytes)?;
            for (offset, encoded) in chunk.as_mut_slice_v1().chunks_exact_mut(8).enumerate() {
                let index = first
                    .checked_add(offset)
                    .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
                let value = if index + 1 == ring_degree {
                    0
                } else {
                    *mask
                        .as_slice_v2()
                        .get(index)
                        .ok_or(ProverPrerequisiteErrorV2::InvalidSourceShape)?
                };
                encoded.copy_from_slice(&value.to_be_bytes());
            }
            let slot = u64::from(live.next_relation)
                .checked_mul(self.descriptor.blocks_per_row)
                .and_then(|value| value.checked_add(block))
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
            live.writer.write_slot_v1(slot, chunk)?;
        }
        live.next_relation = live
            .next_relation
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        self.live = Some(live);
        Ok(())
    }

    pub(super) fn seal_v2(mut self) -> Result<MaskSpoolSealedV2, ProverPrerequisiteErrorV2> {
        let live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if live.next_relation != self.descriptor.relations {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        Ok(MaskSpoolSealedV2 {
            snapshot: Some(live.writer.seal_v1()?),
            geometry: self.geometry,
            parameter_digest: self.parameter_digest,
            descriptor: self.descriptor,
            context: self.context,
            context_digest: self.context_digest,
        })
    }
}

/// Move-only authenticated S store. Replay remains internal and sequential.
pub(super) struct MaskSpoolSealedV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
    descriptor: MaskSpoolDescriptorV2,
    context: PublicSpoolContextV2,
    context_digest: [u8; 32],
}

impl MaskSpoolSealedV2 {
    /// Return the revalidated encrypted-snapshot identity for transcript
    /// binding. This digest is not an authentication authority or a plaintext
    /// commitment; the move-only snapshot must still be replayed completely.
    pub(super) fn snapshot_digest_v2(&self) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
        let descriptor = mask_spool_descriptor_v2(self.geometry, self.parameter_digest)?;
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if descriptor != self.descriptor
            || mask_spool_context_v2(self.parameter_digest, descriptor, self.context)?
                != self.context_digest
            || snapshot.slot_count_v1() != descriptor.slot_count
            || snapshot.plaintext_len_v1() != descriptor.plaintext_bytes
            || snapshot.file_len_v1() != descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        let digest = *snapshot.snapshot_digest_v1();
        (digest != [0; 32])
            .then_some(digest)
            .ok_or(ProverPrerequisiteErrorV2::InvalidC0Context)
    }

    pub(super) fn begin_replay_v2(
        mut self,
    ) -> Result<MaskReplayReaderV2, ProverPrerequisiteErrorV2> {
        let snapshot = self
            .snapshot
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let descriptor = mask_spool_descriptor_v2(self.geometry, self.parameter_digest)?;
        if descriptor != self.descriptor
            || mask_spool_context_v2(self.parameter_digest, descriptor, self.context)?
                != self.context_digest
            || snapshot.slot_count_v1() != descriptor.slot_count
            || snapshot.plaintext_len_v1() != descriptor.plaintext_bytes
            || snapshot.file_len_v1() != descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        self.snapshot = Some(snapshot);
        Ok(MaskReplayReaderV2 {
            owner: Some(self),
            next_slot: 0,
        })
    }
}

pub(super) struct MaskReplayReaderV2 {
    owner: Option<MaskSpoolSealedV2>,
    next_slot: u64,
}

impl MaskReplayReaderV2 {
    pub(super) fn read_next_block_v2(
        &mut self,
    ) -> Result<MaskReplayChunkV2, ProverPrerequisiteErrorV2> {
        let mut owner = self
            .owner
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let mut snapshot = owner
            .snapshot
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_slot >= owner.descriptor.slot_count {
            return Err(ProverPrerequisiteErrorV2::Spool(
                QPcsSpoolErrorV2::ExtraCoefficientBlock,
            ));
        }
        let relation = self.next_slot / owner.descriptor.blocks_per_row;
        let block = self.next_slot % owner.descriptor.blocks_per_row;
        let limb = usize::try_from(relation / u64::from(OPENING_REPETITIONS_V2))
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let modulus = owner.geometry.moduli[limb];
        let chunk = snapshot.read_slot_v1(self.next_slot, owner.context_digest)?;
        for (index, encoded) in chunk.as_slice_v1().chunks_exact(8).enumerate() {
            let value = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
            );
            if value >= modulus
                || (block + 1 == owner.descriptor.blocks_per_row
                    && index + 1 == usize::from(owner.geometry.coefficient_values_per_block)
                    && value != 0)
            {
                return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
            }
        }
        self.next_slot += 1;
        owner.snapshot = Some(snapshot);
        self.owner = Some(owner);
        Ok(MaskReplayChunkV2 { chunk })
    }

    pub(super) fn complete_v2(mut self) -> Result<MaskSpoolSealedV2, ProverPrerequisiteErrorV2> {
        let owner = self
            .owner
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_slot != owner.descriptor.slot_count {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        Ok(owner)
    }
}

pub(super) struct MaskReplayChunkV2 {
    chunk: ConfidentialSpoolChunkV1,
}

impl MaskReplayChunkV2 {
    pub(super) fn bytes_v2(&self) -> &[u8] {
        self.chunk.as_slice_v1()
    }
}

impl Drop for MaskReplayChunkV2 {
    fn drop(&mut self) {
        self.chunk.as_mut_slice_v1().fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        #[cfg(test)]
        MASK_REPLAY_CHUNK_ZEROIZED_DROPS_V2.fetch_add(1, atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
pub(super) fn zero_mask_spool_for_test_v2(
    directory: &Path,
    geometry: SpoolGeometryV2,
    context: PublicSpoolContextV2,
) -> Result<MaskSpoolSealedV2, ProverPrerequisiteErrorV2> {
    let mut writer =
        MaskSpoolWriterV2::create_v2(directory, geometry, parameter_digest_v2(geometry)?, context)?;
    let mask_len = usize::try_from(geometry.ring_degree)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?
        .checked_sub(1)
        .ok_or(ProverPrerequisiteErrorV2::InvalidSourceShape)?;
    for limb in 0..geometry.limb_count_v2()? {
        for repetition in 0..OPENING_REPETITIONS_V2 {
            let mask = SecretResiduesV2::new_zeroed_exact_v2(mask_len)?;
            writer.push_next_mask_v2(limb, repetition, &mask)?;
        }
    }
    writer.seal_v2()
}

#[cfg(test)]
#[path = "s_spool_v2_tests.rs"]
mod tests;
