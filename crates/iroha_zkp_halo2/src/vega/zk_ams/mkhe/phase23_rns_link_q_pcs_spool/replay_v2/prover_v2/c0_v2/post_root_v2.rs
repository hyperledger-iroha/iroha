//! Post-C0 evaluations and authenticated opening-quotient root prerequisite.
//!
//! The shared soundness producer derives all 190 points from C0. Two exact
//! authenticated coefficient passes first bind the 190 `(P~, H~)` evaluations
//! and then synthesize the 380 one-point quotient rows. A column-major spool is
//! read only through the accepted transpose API into the verifier-literal
//! 20-slot Merkle frontier. The combined move-only result retains coefficient,
//! C0, Cq, and S snapshots, but batching, FRI, proof, and release gates stay
//! false and the production authority remains uninhabited.

use core::{convert::Infallible, sync::atomic};
use std::path::Path;

use iroha_confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};

use crate::vega::{
    sponge::Keccak256,
    zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::{
        ProverEvaluationsBoundV2, ProverPostRootPointsV2, ProverQuotientRootBoundV2,
        SoundnessErrorV2,
    },
};

use super::*;

mod global_lookup_s_replay_v1;

const QUOTIENT_LEAF_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0";
const QUOTIENT_NODE_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0";
const QUOTIENT_TREE_KIND_V2: u8 = 2;
const QUOTIENT_TREE_LAYER_V2: u8 = 0;
const QUOTIENT_RELATIONS_V2: usize = 190;
const QUOTIENT_EVALUATION_BYTES_V2: usize = QUOTIENT_RELATIONS_V2 * 16;
const QUOTIENT_FRONTIER_NODES_V2: usize = 20;
const RELEASE_QUOTIENT_LEAVES_V2: usize = 1 << 19;
const CQ_ROW_COUNT_V2: u64 = 380;
const CQ_PRODUCT_MAX_DEGREE_V2: u64 = 262_141;
const CQ_QUOTIENT_MAX_DEGREE_V2: u64 = 131_069;
const CQ_FIXED_COEFFICIENT_WIDTH_V2: u64 = 524_288;
const POST_ROOT_COEFFICIENT_REPLAY_PASSES_V2: u64 = 2;
const POST_ROOT_COEFFICIENT_READ_BYTES_V2: u64 = 1_197_711_360;
const POST_ROOT_CQ_WRITE_BYTES_V2: u64 = 3_190_784_000;
const POST_ROOT_CQ_SEAL_READ_BYTES_V2: u64 = 3_190_784_000;
const POST_ROOT_CQ_ROOT_READ_BYTES_V2: u64 = 3_190_784_000;
const POST_ROOT_TOTAL_IO_BYTES_V2: u64 = 10_770_063_360;
const COMBINED_CQ_AND_S_TOTAL_IO_BYTES_V2: u64 = 11_169_300_480;
const COMBINED_AUTHENTICATED_FILE_BYTES_V2: u64 = 7_180_042_240;
const POST_ROOT_HORNER_STEPS_V2: u64 = 74_711_040;
const POST_ROOT_SYNTHETIC_STEPS_V2: u64 = 74_710_660;
const POST_ROOT_COEFFICIENT_CLEAR_WRITES_V2: u64 = 273_416_192;
const POST_ROOT_FQ2_LOAD_VALUES_V2: u64 = 199_229_440;
const POST_ROOT_NTT_BUTTERFLIES_V2: u64 = 1_892_679_680;
const POST_ROOT_CQ_ENCODE_VALUES_V2: u64 = 199_229_440;
const POST_ROOT_TRANSPOSE_VALUES_V2: u64 = 199_229_440;
const POST_ROOT_COEFFICIENT_BLOCK_READS_V2: u64 = 145_920;
const POST_ROOT_CQ_BLOCK_WRITES_V2: u64 = 194_560;
const POST_ROOT_CQ_SEAL_BLOCK_READS_V2: u64 = 194_560;
const POST_ROOT_CQ_ROOT_BLOCK_READS_V2: u64 = 194_560;
const POST_ROOT_CQ_BLOCK_READS_V2: u64 = 389_120;
const POST_ROOT_QUOTIENT_LEAF_HASHES_V2: u64 = 524_288;
const POST_ROOT_QUOTIENT_NODE_HASHES_V2: u64 = 524_287;
const POST_ROOT_PEAK_EXPLICIT_HEAP_BYTES_V2: usize = 12_599_296;
const POST_ROOT_FIXED_EVALUATION_FRAME_BYTES_V2: usize = 3_040;
const CQ_ROOT_PREPARED_V2: bool = false;
const BATCH_ROWS_WRITTEN_V2: bool = false;
const FRI_PROVER_COMPLETE_V2: bool = false;
const CROSS_FIELD_MASK_PROOF_COMPLETE_V2: bool = false;
const POST_ROOT_ZERO_KNOWLEDGE_BOUND_V2: bool = false;
const POST_ROOT_CANONICAL_PROOF_EMITTED_V2: bool = false;
const POST_ROOT_OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const POST_ROOT_MEASURED_RSS_WITHIN_CAP_V2: bool = false;
const POST_ROOT_RELEASE_READY_V2: bool = false;
const POST_ROOT_RELEASE_COMPLETE_V2: bool = false;

#[cfg(test)]
static POST_ROOT_EVALUATION_DROPS_V2: atomic::AtomicUsize = atomic::AtomicUsize::new(0);
#[cfg(test)]
static POST_ROOT_COEFFICIENT_DROPS_V2: atomic::AtomicUsize = atomic::AtomicUsize::new(0);
#[cfg(test)]
static POST_ROOT_CQ_WINDOW_DROPS_V2: atomic::AtomicUsize = atomic::AtomicUsize::new(0);

const _: () = {
    assert!(QUOTIENT_RELATIONS_V2 == 38 * 5);
    assert!(QUOTIENT_EVALUATION_BYTES_V2 == 3_040);
    assert!(QUOTIENT_FRONTIER_NODES_V2 == 1 + RELEASE_DOMAIN_LOG_V2 as usize);
    assert!(CQ_ROW_COUNT_V2 == 38 * 5 * 2);
    assert!(CQ_PRODUCT_MAX_DEGREE_V2 == 2 * 131_072 - 3);
    assert!(CQ_QUOTIENT_MAX_DEGREE_V2 == 131_072 - 3);
    assert!(CQ_FIXED_COEFFICIENT_WIDTH_V2 == RELEASE_QUOTIENT_LEAVES_V2 as u64);
    assert!(
        POST_ROOT_COEFFICIENT_REPLAY_PASSES_V2 * RELEASE_COEFFICIENT_FILE_BYTES_V2
            == POST_ROOT_COEFFICIENT_READ_BYTES_V2
    );
    assert!(POST_ROOT_CQ_WRITE_BYTES_V2 == CQ_COLUMN_FILE_BYTES_V2);
    assert!(POST_ROOT_CQ_SEAL_READ_BYTES_V2 == CQ_COLUMN_FILE_BYTES_V2);
    assert!(POST_ROOT_CQ_ROOT_READ_BYTES_V2 == CQ_COLUMN_FILE_BYTES_V2);
    assert!(
        POST_ROOT_TOTAL_IO_BYTES_V2
            == POST_ROOT_COEFFICIENT_READ_BYTES_V2
                + POST_ROOT_CQ_WRITE_BYTES_V2
                + POST_ROOT_CQ_SEAL_READ_BYTES_V2
                + POST_ROOT_CQ_ROOT_READ_BYTES_V2
    );
    assert!(
        COMBINED_CQ_AND_S_TOTAL_IO_BYTES_V2
            == POST_ROOT_TOTAL_IO_BYTES_V2 + RELEASE_MASK_S_TOTAL_IO_BYTES_V2
    );
    assert!(
        COMBINED_AUTHENTICATED_FILE_BYTES_V2
            == RELEASE_COEFFICIENT_FILE_BYTES_V2
                + RELEASE_LDE_FILE_BYTES_V2
                + CQ_COLUMN_FILE_BYTES_V2
                + 199_618_560
    );
    assert!(POST_ROOT_HORNER_STEPS_V2 == 190 * 3 * 131_072);
    assert!(POST_ROOT_SYNTHETIC_STEPS_V2 == 190 * (3 * 131_072 - 2));
    assert!(POST_ROOT_COEFFICIENT_CLEAR_WRITES_V2 == 570 * 131_072 + 379 * 524_288);
    assert!(POST_ROOT_FQ2_LOAD_VALUES_V2 == 380 * 524_288);
    assert!(POST_ROOT_NTT_BUTTERFLIES_V2 == 380 * (524_288 / 2) * 19);
    assert!(POST_ROOT_CQ_ENCODE_VALUES_V2 == 380 * 524_288);
    assert!(POST_ROOT_TRANSPOSE_VALUES_V2 == 380 * 524_288);
    assert!(POST_ROOT_COEFFICIENT_BLOCK_READS_V2 == 2 * RELEASE_COEFFICIENT_SLOTS_V2);
    assert!(POST_ROOT_CQ_BLOCK_WRITES_V2 == RELEASE_LDE_SLOTS_V2);
    assert!(POST_ROOT_CQ_SEAL_BLOCK_READS_V2 == RELEASE_LDE_SLOTS_V2);
    assert!(POST_ROOT_CQ_ROOT_BLOCK_READS_V2 == RELEASE_LDE_SLOTS_V2);
    assert!(
        POST_ROOT_CQ_BLOCK_READS_V2
            == POST_ROOT_CQ_SEAL_BLOCK_READS_V2 + POST_ROOT_CQ_ROOT_BLOCK_READS_V2
    );
    assert!(POST_ROOT_QUOTIENT_LEAF_HASHES_V2 == RELEASE_QUOTIENT_LEAVES_V2 as u64);
    assert!(POST_ROOT_QUOTIENT_NODE_HASHES_V2 + 1 == POST_ROOT_QUOTIENT_LEAF_HASHES_V2);
    assert!(POST_ROOT_PEAK_EXPLICIT_HEAP_BYTES_V2 == 4_194_304 + 8_388_608 + 16_384);
    assert!(POST_ROOT_FIXED_EVALUATION_FRAME_BYTES_V2 == QUOTIENT_EVALUATION_BYTES_V2);
    assert!(!CQ_ROOT_PREPARED_V2);
    assert!(!BATCH_ROWS_WRITTEN_V2);
    assert!(!FRI_PROVER_COMPLETE_V2);
    assert!(!CROSS_FIELD_MASK_PROOF_COMPLETE_V2);
    assert!(!POST_ROOT_ZERO_KNOWLEDGE_BOUND_V2);
    assert!(!POST_ROOT_CANONICAL_PROOF_EMITTED_V2);
    assert!(!POST_ROOT_OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!POST_ROOT_MEASURED_RSS_WITHIN_CAP_V2);
    assert!(!POST_ROOT_RELEASE_READY_V2);
    assert!(!POST_ROOT_RELEASE_COMPLETE_V2);
};

impl From<SoundnessErrorV2> for ProverPrerequisiteErrorV2 {
    fn from(_: SoundnessErrorV2) -> Self {
        Self::InvalidPostRootTranscript
    }
}

/// The post-root producer remains production-uninhabited independently of C0.
pub(super) enum PostRootAuthorityV2 {
    Production {
        point_schedule: Infallible,
        quotient_rows: Infallible,
        quotient_root: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

struct ZeroizingEvaluationFrameV2 {
    bytes: [u8; QUOTIENT_EVALUATION_BYTES_V2],
}

impl ZeroizingEvaluationFrameV2 {
    const fn new_v2() -> Self {
        Self {
            bytes: [0; QUOTIENT_EVALUATION_BYTES_V2],
        }
    }
}

impl Drop for ZeroizingEvaluationFrameV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        #[cfg(test)]
        POST_ROOT_EVALUATION_DROPS_V2.fetch_add(1, atomic::Ordering::SeqCst);
    }
}

struct ZeroizingCoefficientBufferV2 {
    values: Vec<u64>,
}

impl ZeroizingCoefficientBufferV2 {
    fn new_v2(capacity: usize) -> Result<Self, ProverPrerequisiteErrorV2> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if values.capacity() != capacity {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        Ok(Self { values })
    }
}

impl Drop for ZeroizingCoefficientBufferV2 {
    fn drop(&mut self) {
        self.values.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        #[cfg(test)]
        POST_ROOT_COEFFICIENT_DROPS_V2.fetch_add(1, atomic::Ordering::SeqCst);
    }
}

fn read_component_v2(
    replay: PostC0CoefficientReplayV2,
    modulus: u64,
    coefficients: &mut ZeroizingCoefficientBufferV2,
    clear: bool,
) -> Result<PostC0CoefficientReplayV2, ProverPrerequisiteErrorV2> {
    if clear {
        coefficients.values.fill(0);
        coefficients.values.clear();
    }
    let mut row = replay.begin_next_row_v2()?;
    let blocks = row.geometry_v2()?.coefficient_blocks_per_component_v2()?;
    for _ in 0..blocks {
        let chunk = row.read_next_block_v2()?;
        for encoded in chunk.bytes_v2().chunks_exact(8) {
            let value = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
            );
            if value >= modulus {
                return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
            }
            if coefficients.values.len() == coefficients.values.capacity() {
                return Err(ProverPrerequisiteErrorV2::Allocation);
            }
            coefficients.values.push(value);
        }
    }
    Ok(row.complete_v2()?)
}

fn evaluate_coefficients_v2(coefficients: &[u64], point: u64, modulus: u64) -> u64 {
    let mut evaluation = 0_u64;
    for coefficient in coefficients.iter().rev().copied() {
        evaluation = add_mod_v2(
            ((u128::from(evaluation) * u128::from(point)) % u128::from(modulus)) as u64,
            coefficient,
            modulus,
        );
    }
    evaluation
}

fn evaluate_component_v2(
    replay: PostC0CoefficientReplayV2,
    point: u64,
    modulus: u64,
    coefficients: &mut ZeroizingCoefficientBufferV2,
) -> Result<(PostC0CoefficientReplayV2, u64), ProverPrerequisiteErrorV2> {
    let replay = read_component_v2(replay, modulus, coefficients, true)?;
    let evaluation = evaluate_coefficients_v2(&coefficients.values, point, modulus);
    Ok((replay, evaluation))
}

fn pow_mod_v2(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = ((u128::from(result) * u128::from(base)) % u128::from(modulus)) as u64;
        }
        base = ((u128::from(base) * u128::from(base)) % u128::from(modulus)) as u64;
        exponent >>= 1;
    }
    result
}

fn synthesize_quotient_v2(
    coefficients: &mut ZeroizingCoefficientBufferV2,
    point: u64,
    evaluation: u64,
    modulus: u64,
    domain_size: usize,
) -> Result<(), ProverPrerequisiteErrorV2> {
    if modulus <= 2 || point == 0 || point >= modulus || evaluation >= modulus {
        return Err(ProverPrerequisiteErrorV2::InvalidPostRootTranscript);
    }
    if coefficients.values.len() < 2
        || coefficients.values.last() != Some(&0)
        || domain_size < coefficients.values.len()
        || coefficients.values.capacity() != domain_size
    {
        return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
    }
    let inverse = pow_mod_v2(point, modulus - 2, modulus);
    if inverse == 0 {
        return Err(ProverPrerequisiteErrorV2::InvalidPostRootTranscript);
    }
    let mut quotient = ((u128::from(add_mod_v2(
        evaluation,
        modulus - coefficients.values[0],
        modulus,
    )) * u128::from(inverse))
        % u128::from(modulus)) as u64;
    coefficients.values[0] = quotient;
    for index in 1..coefficients.values.len() - 1 {
        quotient = ((u128::from(add_mod_v2(
            quotient,
            modulus - coefficients.values[index],
            modulus,
        )) * u128::from(inverse))
            % u128::from(modulus)) as u64;
        coefficients.values[index] = quotient;
    }
    if quotient != 0 {
        return Err(ProverPrerequisiteErrorV2::InvalidOpeningQuotient);
    }
    let last = coefficients.values.len() - 1;
    coefficients.values[last] = 0;
    coefficients.values.resize(domain_size, 0);
    Ok(())
}

fn load_fq2_buffer_v2(
    buffer: &mut ZeroizingNttBufferV2,
    coefficients: &ZeroizingCoefficientBufferV2,
) -> Result<(), ProverPrerequisiteErrorV2> {
    if buffer.values.len() != coefficients.values.len() {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    for (destination, coefficient) in buffer.values.iter_mut().zip(&coefficients.values) {
        *destination = Fq2V1::base(*coefficient);
    }
    Ok(())
}

struct CqColumnWriterV2 {
    writer: Option<ConfidentialSpoolWriterV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    next_slot: u64,
}

impl CqColumnWriterV2 {
    fn create_v2(
        directory: &Path,
        parameter_digest: [u8; 32],
        context: PublicSpoolContextV2,
        initial_root: [u8; 32],
        evaluation_transcript: [u8; 32],
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let descriptor = cq_column_layout_v2(parameter_digest)?;
        let context_digest = cq_post_root_context_digest_v2(
            descriptor,
            context,
            parameter_digest,
            initial_root,
            evaluation_transcript,
        )?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            descriptor.slot_count,
            descriptor.plaintext_bytes,
            context_digest,
        )?;
        if layout.file_len_v1() != descriptor.file_bytes {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
        }
        Ok(Self {
            writer: Some(ConfidentialSpoolWriterV1::create_in_v1(directory, layout)?),
            descriptor,
            context_digest,
            next_slot: 0,
        })
    }

    fn write_row_v2(
        &mut self,
        expected_column: u16,
        buffer: &ZeroizingNttBufferV2,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut writer = self
            .writer
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.descriptor.blocks_per_column == 0
            || self.next_slot / self.descriptor.blocks_per_column != u64::from(expected_column)
        {
            return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
        }
        let block_values = usize::from(self.descriptor.values_per_block);
        if block_values == 0
            || buffer.values.len()
                != usize::try_from(self.descriptor.logical_length)
                    .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?
            || !buffer.values.len().is_multiple_of(block_values)
        {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        for values in buffer.values.chunks_exact(block_values) {
            if self.next_slot >= self.descriptor.slot_count {
                return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
            }
            let mut chunk =
                ConfidentialSpoolChunkV1::new_zeroed_v1(self.descriptor.plaintext_bytes)?;
            for (encoded, value) in chunk.as_mut_slice_v1().chunks_exact_mut(16).zip(values) {
                encoded[..8].copy_from_slice(&value.c0.to_be_bytes());
                encoded[8..].copy_from_slice(&value.c1.to_be_bytes());
            }
            writer.write_slot_v1(self.next_slot, chunk)?;
            self.next_slot += 1;
        }
        self.writer = Some(writer);
        Ok(())
    }

    fn seal_v2(mut self) -> Result<CqColumnSnapshotV2, ProverPrerequisiteErrorV2> {
        let writer = self
            .writer
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_slot != self.descriptor.slot_count {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        Ok(CqColumnSnapshotV2 {
            snapshot: Some(writer.seal_v1()?),
            descriptor: self.descriptor,
            context_digest: self.context_digest,
        })
    }
}

struct CqColumnSnapshotV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
}

struct QuotientFrontierV2 {
    nodes: [[u8; 32]; QUOTIENT_FRONTIER_NODES_V2],
    occupied: u32,
    leaves: usize,
    parameter_digest: [u8; 32],
}

fn quotient_leaf_hash_v2(
    parameter_digest: [u8; 32],
    length: usize,
    coordinate_count: u16,
    values: &[u8],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if values.len() != usize::from(coordinate_count) * 16 || !length.is_power_of_two() {
        return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
    }
    let mut hash = Keccak256::new();
    hash.update(QUOTIENT_LEAF_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[QUOTIENT_TREE_KIND_V2, QUOTIENT_TREE_LAYER_V2]);
    hash.update(
        &u32::try_from(length)
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(&coordinate_count.to_be_bytes());
    hash.update(values);
    Ok(hash.finalize())
}

fn quotient_node_hash_v2(
    parameter_digest: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(QUOTIENT_NODE_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[
        QUOTIENT_TREE_KIND_V2,
        QUOTIENT_TREE_LAYER_V2,
        u8::try_from(height).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
    ]);
    hash.update(&left);
    hash.update(&right);
    Ok(hash.finalize())
}

impl QuotientFrontierV2 {
    const fn new_v2(parameter_digest: [u8; 32]) -> Self {
        Self {
            nodes: [[0; 32]; QUOTIENT_FRONTIER_NODES_V2],
            occupied: 0,
            leaves: 0,
            parameter_digest,
        }
    }

    fn push_v2(&mut self, mut digest: [u8; 32]) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut level = 0_usize;
        let mut prior = self.leaves;
        while prior & 1 == 1 {
            let left = *self
                .nodes
                .get(level)
                .ok_or(ProverPrerequisiteErrorV2::InvalidMerkleRoot)?;
            digest = quotient_node_hash_v2(self.parameter_digest, level + 1, left, digest)?;
            self.nodes[level] = [0; 32];
            self.occupied &= !1_u32
                .checked_shl(
                    u32::try_from(level)
                        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
                )
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
            prior >>= 1;
            level += 1;
        }
        if level >= self.nodes.len() {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        self.nodes[level] = digest;
        self.occupied |= 1_u32
            .checked_shl(
                u32::try_from(level).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        self.leaves = self
            .leaves
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if self.leaves > RELEASE_QUOTIENT_LEAVES_V2 {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        Ok(())
    }

    fn finish_v2(self, expected: usize) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
        if expected == 0 || !expected.is_power_of_two() {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        let level = expected.ilog2() as usize;
        if self.leaves != expected
            || level >= QUOTIENT_FRONTIER_NODES_V2
            || self.occupied != 1_u32 << level
            || self.nodes[level] == [0; 32]
        {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        Ok(self.nodes[level])
    }
}

struct ZeroizingCqWindowV2 {
    bytes: Vec<u8>,
    values_per_block: usize,
    leaf_bytes: usize,
}

impl ZeroizingCqWindowV2 {
    fn new_v2(values_per_block: usize, columns: usize) -> Result<Self, ProverPrerequisiteErrorV2> {
        let leaf_bytes = columns
            .checked_mul(16)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let len = values_per_block
            .checked_mul(leaf_bytes)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if bytes.capacity() != len {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        bytes.resize(len, 0);
        Ok(Self {
            bytes,
            values_per_block,
            leaf_bytes,
        })
    }

    fn absorb_v2(&mut self, column: usize, chunk: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
        let column_offset = column
            .checked_mul(16)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if chunk.len() != self.values_per_block * 16
            || column_offset
                .checked_add(16)
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?
                > self.leaf_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        for (index, value) in chunk.chunks_exact(16).enumerate() {
            let start = index
                .checked_mul(self.leaf_bytes)
                .and_then(|value| value.checked_add(column_offset))
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
            self.bytes
                .get_mut(start..start + 16)
                .ok_or(ProverPrerequisiteErrorV2::InvalidSourceShape)?
                .copy_from_slice(value);
        }
        Ok(())
    }
}

impl Drop for ZeroizingCqWindowV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        #[cfg(test)]
        POST_ROOT_CQ_WINDOW_DROPS_V2.fetch_add(1, atomic::Ordering::SeqCst);
    }
}

fn transpose_and_root_v2(
    mut column: CqColumnSnapshotV2,
    parameter_digest: [u8; 32],
    context: PublicSpoolContextV2,
    initial_root: [u8; 32],
    evaluation_transcript: [u8; 32],
    permit: AuthenticatedReplayPermitV2,
) -> Result<([u8; 32], QPcsDerivedReplayV2), ProverPrerequisiteErrorV2> {
    let column_snapshot = column
        .snapshot
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    if cq_post_root_context_digest_v2(
        column.descriptor,
        context,
        parameter_digest,
        initial_root,
        evaluation_transcript,
    )? != column.context_digest
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    if column.descriptor.logical_length != RELEASE_QUOTIENT_LEAVES_V2 as u64
        || column.descriptor.columns != REPLAY_COLUMNS_V2
        || column.descriptor.values_per_block != REPLAY_BLOCK_VALUES_V2
        || column.descriptor.blocks_per_column != REPLAY_BLOCKS_PER_COLUMN_V2
        || column.descriptor.slot_count != RELEASE_LDE_SLOTS_V2
        || column.descriptor.plaintext_bytes != RELEASE_LDE_BLOCK_BYTES_V2
        || column.descriptor.file_bytes != CQ_COLUMN_FILE_BYTES_V2
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let columns = usize::from(column.descriptor.columns);
    let values_per_block = usize::from(column.descriptor.values_per_block);
    let mut window = ZeroizingCqWindowV2::new_v2(values_per_block, columns)?;
    let mut frontier = QuotientFrontierV2::new_v2(parameter_digest);
    let mut replay = bind_cq_post_root_replay_v2(
        column_snapshot,
        column.descriptor,
        context,
        parameter_digest,
        initial_root,
        evaluation_transcript,
        permit,
    )?;
    for _ in 0..column.descriptor.blocks_per_column {
        let mut transpose = replay.begin_next_cq_transpose_window_v2()?;
        for column_index in 0..column.descriptor.columns {
            let chunk = transpose.read_next_column_v2()?;
            window.absorb_v2(usize::from(column_index), chunk.bytes_v2())?;
        }
        for index in 0..values_per_block {
            let start = index * window.leaf_bytes;
            frontier.push_v2(quotient_leaf_hash_v2(
                parameter_digest,
                RELEASE_QUOTIENT_LEAVES_V2,
                column.descriptor.columns,
                &window.bytes[start..start + window.leaf_bytes],
            )?)?;
        }
        replay = transpose.complete_v2()?;
    }
    let root = frontier.finish_v2(RELEASE_QUOTIENT_LEAVES_V2)?;
    Ok((root, replay))
}

/// Combined owner for every authenticated artifact through the Cq root.
pub(super) struct QuotientRootPreparedV2 {
    accepted_c0: Option<QPcsC0StoredV2>,
    masks: Option<MaskSpoolSealedV2>,
    accepted_cq: Option<QPcsDerivedReplayV2>,
    transcript: Option<ProverQuotientRootBoundV2>,
    evaluations: ZeroizingEvaluationFrameV2,
    context: PublicSpoolContextV2,
    parameter_digest: [u8; 32],
    initial_root: [u8; 32],
    quotient_root: [u8; 32],
}

impl InitialC0RootPreparedV2 {
    pub(super) fn prepare_quotient_root_v2(
        self,
        directory: &Path,
        authority: PostRootAuthorityV2,
    ) -> Result<QuotientRootPreparedV2, ProverPrerequisiteErrorV2> {
        match authority {
            PostRootAuthorityV2::Production {
                point_schedule,
                quotient_rows: _quotient_rows,
                quotient_root: _quotient_root,
            } => match point_schedule {},
            #[cfg(test)]
            PostRootAuthorityV2::TestOnly => {}
        }
        prepare_quotient_operation_v2(self, directory)
    }
}

fn prepare_quotient_operation_v2(
    mut prepared: InitialC0RootPreparedV2,
    directory: &Path,
) -> Result<QuotientRootPreparedV2, ProverPrerequisiteErrorV2> {
    let geometry = prepared
        .accepted_c0
        .as_ref()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?
        .snapshot
        .geometry;
    let release = SpoolGeometryV2::release_v2();
    if geometry.ring_degree != release.ring_degree
        || geometry.domain_log != release.domain_log
        || geometry.query_count != release.query_count
        || geometry.coefficient_values_per_block != release.coefficient_values_per_block
        || geometry.lde_values_per_block != release.lde_values_per_block
        || geometry.moduli != release.moduli
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let points = ProverPostRootPointsV2::derive_v2(
        prepared.parameter_digest,
        prepared.context.sealed_source_transcript_digest,
        prepared.context.source_algebra_binding_digest,
        prepared.initial_root,
    )?;
    let accepted_c0 = prepared
        .accepted_c0
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let mut replay = accepted_c0.begin_post_c0_coefficient_replay_v2()?;
    let domain_size = usize::try_from(geometry.domain_size_v2()?)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let ring_degree = usize::try_from(geometry.ring_degree)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let mut coefficients = ZeroizingCoefficientBufferV2::new_v2(domain_size)?;
    let mut evaluations = ZeroizingEvaluationFrameV2::new_v2();
    for limb in 0..geometry.limb_count_v2()? {
        let modulus = geometry.moduli[usize::from(limb)];
        for repetition in 0..OPENING_REPETITIONS_V2 {
            let relation =
                usize::from(limb) * usize::from(OPENING_REPETITIONS_V2) + usize::from(repetition);
            let point = points.point_v2(usize::from(limb), usize::from(repetition))?;
            let (next, product_low) =
                evaluate_component_v2(replay, point, modulus, &mut coefficients)?;
            replay = next;
            if coefficients.values.len() != ring_degree {
                return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
            }
            let (next, product_high) =
                evaluate_component_v2(replay, point, modulus, &mut coefficients)?;
            replay = next;
            if coefficients.values.len() != ring_degree {
                return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
            }
            let point_to_n = pow_mod_v2(point, geometry.ring_degree.into(), modulus);
            let product = add_mod_v2(
                product_low,
                ((u128::from(point_to_n) * u128::from(product_high)) % u128::from(modulus)) as u64,
                modulus,
            );
            let (next, quotient) =
                evaluate_component_v2(replay, point, modulus, &mut coefficients)?;
            replay = next;
            if coefficients.values.len() != ring_degree {
                return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
            }
            if product
                != ((u128::from(add_mod_v2(point_to_n, 1, modulus)) * u128::from(quotient))
                    % u128::from(modulus)) as u64
            {
                return Err(ProverPrerequisiteErrorV2::InvalidRelation);
            }
            evaluations.bytes[relation * 16..relation * 16 + 8]
                .copy_from_slice(&product.to_be_bytes());
            evaluations.bytes[relation * 16 + 8..relation * 16 + 16]
                .copy_from_slice(&quotient.to_be_bytes());
        }
    }
    let first_pass = replay.complete_v2()?;
    let evaluations_bound: ProverEvaluationsBoundV2 =
        points.bind_evaluations_v2(&evaluations.bytes)?;
    let pre_quotient_transcript = evaluations_bound.transcript_v2()?;
    let mut cq_writer = CqColumnWriterV2::create_v2(
        directory,
        prepared.parameter_digest,
        prepared.context,
        prepared.initial_root,
        pre_quotient_transcript,
    )?;
    let mut replay = first_pass.begin_second_replay_v2()?;
    let mut ntt = ZeroizingNttBufferV2::new_v2(domain_size)?;
    for limb in 0..geometry.limb_count_v2()? {
        let modulus = geometry.moduli[usize::from(limb)];
        let field = Fq2ParametersV1::derive(modulus, usize::from(geometry.domain_log))
            .map_err(|_| ProverPrerequisiteErrorV2::InvalidNtt)?;
        for repetition in 0..OPENING_REPETITIONS_V2 {
            let relation =
                usize::from(limb) * usize::from(OPENING_REPETITIONS_V2) + usize::from(repetition);
            let point = evaluations_bound.point_v2(usize::from(limb), usize::from(repetition))?;
            let evaluation_offset = relation * 16;
            let product = u64::from_be_bytes(
                evaluations.bytes[evaluation_offset..evaluation_offset + 8]
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidPostRootTranscript)?,
            );
            let quotient = u64::from_be_bytes(
                evaluations.bytes[evaluation_offset + 8..evaluation_offset + 16]
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidPostRootTranscript)?,
            );

            replay = read_component_v2(replay, modulus, &mut coefficients, true)?;
            if coefficients.values.len() != ring_degree {
                return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
            }
            replay = read_component_v2(replay, modulus, &mut coefficients, false)?;
            if coefficients.values.len() != 2 * ring_degree {
                return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
            }
            synthesize_quotient_v2(&mut coefficients, point, product, modulus, domain_size)?;
            load_fq2_buffer_v2(&mut ntt, &coefficients)?;
            ntt_in_place_v2(&mut ntt.values, field)?;
            cq_writer.write_row_v2(
                fixed_row_column_v2(limb, repetition, LdeRowRoleV2::Product)?,
                &ntt,
            )?;

            replay = read_component_v2(replay, modulus, &mut coefficients, true)?;
            if coefficients.values.len() != ring_degree {
                return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
            }
            synthesize_quotient_v2(&mut coefficients, point, quotient, modulus, domain_size)?;
            load_fq2_buffer_v2(&mut ntt, &coefficients)?;
            ntt_in_place_v2(&mut ntt.values, field)?;
            cq_writer.write_row_v2(
                fixed_row_column_v2(limb, repetition, LdeRowRoleV2::Quotient)?,
                &ntt,
            )?;
        }
    }
    let accepted_c0 = replay.complete_v2()?.finish_v2()?;
    drop(coefficients);
    drop(ntt);
    let cq_column = cq_writer.seal_v2()?;
    let (accepted_c0, permit) = accepted_c0.separate_replay_permit_v2()?;
    let (quotient_root, accepted_cq) = transpose_and_root_v2(
        cq_column,
        prepared.parameter_digest,
        prepared.context,
        prepared.initial_root,
        pre_quotient_transcript,
        permit,
    )?;
    let transcript = evaluations_bound.bind_quotient_root_v2(quotient_root)?;
    Ok(QuotientRootPreparedV2 {
        accepted_c0: Some(accepted_c0),
        masks: prepared.masks.take(),
        accepted_cq: Some(accepted_cq),
        transcript: Some(transcript),
        evaluations,
        context: prepared.context,
        parameter_digest: prepared.parameter_digest,
        initial_root: prepared.initial_root,
        quotient_root,
    })
}

#[cfg(test)]
#[path = "post_root_v2_tests.rs"]
mod tests;
