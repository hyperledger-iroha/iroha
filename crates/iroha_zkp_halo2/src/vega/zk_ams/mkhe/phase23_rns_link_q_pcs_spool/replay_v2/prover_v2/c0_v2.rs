//! Bounded authenticated initial-C0-root prerequisite for qPCS V2.
//!
//! This child consumes the accepted masked-coefficient typestate, performs one exact-width Fq2 NTT
//! row at a time, writes a purpose-bound column-major staging spool, transposes it into the already
//! accepted block-major LDE spool, and streams that authenticated replay into the verifier-literal
//! initial Merkle tree.  The only production authority is uninhabited.  The
//! returned value is therefore a non-authorizing prerequisite and retains the
//! accepted replay owner without exposing a file, path, key, slot, or snapshot.
use super::super::super::super::{Fq2ParametersV1, Fq2V1};
use super::*;
use crate::vega::sponge::Keccak256;
use core::{convert::Infallible, sync::atomic};
use iroha_crypto::confidential_spool::ConfidentialSpoolChunkV1;
use std::path::Path;
const MERKLE_LEAF_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0";
const MERKLE_NODE_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0";
const INITIAL_TREE_KIND_V2: u8 = 1;
const INITIAL_TREE_LAYER_V2: u8 = 0;
const RELEASE_INITIAL_LEAVES_V2: usize = 1 << 19;
const INITIAL_MERKLE_FRONTIER_NODES_V2: usize = 20;
#[path = "c0_v2/storage_v2.rs"]
mod storage_v2;
use storage_v2::*;
const _: () = {
    assert!(RELEASE_DOMAIN_LOG_V2 == 19);
    assert!(REPLAY_COLUMNS_V2 == 380);
    assert!(RELEASE_INITIAL_LEAVES_V2 == REPLAY_DOMAIN_VALUES_V2 as usize);
    assert!(INITIAL_MERKLE_FRONTIER_NODES_V2 == 1 + RELEASE_DOMAIN_LOG_V2 as usize);
    assert!(!INITIAL_C0_ROOT_PREPARED_V2);
    assert!(!INITIAL_C0_ROOT_FROZEN_V2);
    assert!(!POST_ROOT_POINTS_DERIVED_V2);
    assert!(!CQ_ROWS_WRITTEN_V2);
    assert!(!FRI_FIRST_PASS_COMPLETE_V2);
    assert!(!FRI_SECOND_PASS_COMPLETE_V2);
    assert!(!CANONICAL_PROOF_EMITTED_V2);
    assert!(!PROVER_RELEASE_READY_V2);
};
/// Three independent future authorities are required to make this transition.
pub(super) enum InitialC0AuthorityV2 {
    Production {
        ntt: Infallible,
        transpose: Infallible,
        initial_root: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
struct ZeroizingNttBufferV2 {
    values: Vec<Fq2V1>,
}
impl ZeroizingNttBufferV2 {
    fn new_v2(len: usize) -> Result<Self, ProverPrerequisiteErrorV2> {
        if len == 0 || !len.is_power_of_two() {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if values.capacity() != len {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        values.resize(len, Fq2V1::ZERO);
        Ok(Self { values })
    }
    fn clear_v2(&mut self) {
        self.values.fill(Fq2V1::ZERO);
    }
}
impl Drop for ZeroizingNttBufferV2 {
    fn drop(&mut self) {
        self.values.fill(Fq2V1::ZERO);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        #[cfg(test)]
        {
            debug_assert!(self.values.iter().all(|value| *value == Fq2V1::ZERO));
            ZEROIZING_NTT_BUFFER_DROPS_V2.fetch_add(1, atomic::Ordering::SeqCst);
        }
    }
}
fn ntt_in_place_v2(
    values: &mut [Fq2V1],
    field: Fq2ParametersV1,
) -> Result<(), ProverPrerequisiteErrorV2> {
    if values.len() != 1_usize << field.domain_log || !values.len().is_power_of_two() {
        return Err(ProverPrerequisiteErrorV2::InvalidNtt);
    }
    for index in 1..values.len() {
        let reversed = index.reverse_bits() >> (usize::BITS - u32::from(field.domain_log));
        if index < reversed {
            values.swap(index, reversed);
        }
    }
    let mut length = 2_usize;
    let mut stages = 0_u8;
    while length <= values.len() {
        let step = field.pow(field.domain_root, (values.len() / length) as u128);
        for start in (0..values.len()).step_by(length) {
            let mut twiddle = Fq2V1::ONE;
            for offset in 0..length / 2 {
                let even = values[start + offset];
                let odd = field.mul(values[start + offset + length / 2], twiddle);
                values[start + offset] = field.add(even, odd);
                values[start + offset + length / 2] = field.sub(even, odd);
                twiddle = field.mul(twiddle, step);
            }
        }
        stages = stages
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        length = length
            .checked_mul(2)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    }
    if stages != field.domain_log {
        return Err(ProverPrerequisiteErrorV2::InvalidNtt);
    }
    Ok(())
}
fn replay_component_v2(
    stage: QPcsCoefficientReplayStageV2,
    buffer: &mut ZeroizingNttBufferV2,
    destination_start: usize,
    modulus: u64,
) -> Result<QPcsCoefficientReplayStageV2, ProverPrerequisiteErrorV2> {
    let ring_degree = usize::try_from(stage.geometry.ring_degree)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let mut reader = stage.begin_next_coefficient_row_v2()?;
    let mut written = 0_usize;
    for _ in 0..reader
        .stage
        .as_ref()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?
        .geometry
        .coefficient_blocks_per_component_v2()?
    {
        let chunk = reader.read_next_block_v2()?;
        for encoded in chunk.bytes_v2().chunks_exact(8) {
            let value = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
            );
            if value >= modulus || written >= ring_degree {
                return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
            }
            let destination = destination_start
                .checked_add(written)
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
            *buffer
                .values
                .get_mut(destination)
                .ok_or(ProverPrerequisiteErrorV2::InvalidSourceShape)? = Fq2V1::base(value);
            written += 1;
        }
    }
    if written != ring_degree {
        return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
    }
    Ok(reader.complete_v2()?)
}
fn write_ntt_column_v2(
    writer: &mut InitialColumnWriterV2,
    buffer: &ZeroizingNttBufferV2,
) -> Result<(), ProverPrerequisiteErrorV2> {
    let values_per_block = usize::from(writer.descriptor.values_per_block);
    for values in buffer.values.chunks_exact(values_per_block) {
        let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(writer.descriptor.plaintext_bytes)?;
        for (encoded, value) in chunk.as_mut_slice_v1().chunks_exact_mut(16).zip(values) {
            encoded[..8].copy_from_slice(&value.c0.to_be_bytes());
            encoded[8..].copy_from_slice(&value.c1.to_be_bytes());
        }
        writer.push_next_block_v2(chunk)?;
    }
    Ok(())
}
fn initial_leaf_hash_v2(
    parameter_digest: [u8; 32],
    length: usize,
    coordinate_count: u16,
    values: &[u8],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if values.len() != usize::from(coordinate_count) * 16 || !length.is_power_of_two() {
        return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
    }
    let mut hash = Keccak256::new();
    hash.update(MERKLE_LEAF_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[INITIAL_TREE_KIND_V2, INITIAL_TREE_LAYER_V2]);
    hash.update(
        &u32::try_from(length)
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(&coordinate_count.to_be_bytes());
    hash.update(values);
    Ok(hash.finalize())
}
fn initial_node_hash_v2(
    parameter_digest: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(MERKLE_NODE_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[
        INITIAL_TREE_KIND_V2,
        INITIAL_TREE_LAYER_V2,
        u8::try_from(height).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
    ]);
    hash.update(&left);
    hash.update(&right);
    Ok(hash.finalize())
}
struct MerkleFrontierV2 {
    nodes: [[u8; 32]; INITIAL_MERKLE_FRONTIER_NODES_V2],
    occupied: u32,
    leaves: usize,
    parameter_digest: [u8; 32],
}
impl MerkleFrontierV2 {
    const fn new_v2(parameter_digest: [u8; 32]) -> Self {
        Self {
            nodes: [[0; 32]; INITIAL_MERKLE_FRONTIER_NODES_V2],
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
            digest = initial_node_hash_v2(self.parameter_digest, level + 1, left, digest)?;
            self.occupied &= !(1_u32 << level);
            self.nodes[level] = [0; 32];
            prior >>= 1;
            level += 1;
        }
        *self
            .nodes
            .get_mut(level)
            .ok_or(ProverPrerequisiteErrorV2::InvalidMerkleRoot)? = digest;
        self.occupied |= 1_u32 << level;
        self.leaves = self
            .leaves
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        Ok(())
    }
    fn finish_v2(self, expected: usize) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
        if self.leaves != expected || !expected.is_power_of_two() {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        let level = expected.ilog2() as usize;
        if level >= INITIAL_MERKLE_FRONTIER_NODES_V2 || self.occupied != 1_u32 << level {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        let root = self.nodes[level];
        if root == [0; 32] {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        Ok(root)
    }
}
struct ZeroizingLeafWindowV2 {
    bytes: Vec<u8>,
    values_per_block: usize,
    leaf_bytes: usize,
}
impl ZeroizingLeafWindowV2 {
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
    fn absorb_column_v2(
        &mut self,
        column: usize,
        chunk: &AuthenticatedReplayChunkV2,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        if chunk.bytes_v2().len() != self.values_per_block * 16 {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        for (index, value) in chunk.bytes_v2().chunks_exact(16).enumerate() {
            let start = index
                .checked_mul(self.leaf_bytes)
                .and_then(|offset| offset.checked_add(column.checked_mul(16)?))
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
            self.bytes[start..start + 16].copy_from_slice(value);
        }
        Ok(())
    }
    fn leaf_v2(&self, index: usize) -> Result<&[u8], ProverPrerequisiteErrorV2> {
        let start = index
            .checked_mul(self.leaf_bytes)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        self.bytes
            .get(start..start + self.leaf_bytes)
            .ok_or(ProverPrerequisiteErrorV2::InvalidMerkleRoot)
    }
}
impl Drop for ZeroizingLeafWindowV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        #[cfg(test)]
        {
            debug_assert!(self.bytes.iter().all(|byte| *byte == 0));
            ZEROIZING_LEAF_WINDOW_DROPS_V2.fetch_add(1, atomic::Ordering::SeqCst);
        }
    }
}
fn build_initial_root_v2(
    snapshot: QPcsSpoolSnapshotV2,
    geometry: SpoolGeometryV2,
    parameter_digest: [u8; 32],
) -> Result<([u8; 32], QPcsC0CompleteV2), ProverPrerequisiteErrorV2> {
    let domain_size = usize::try_from(geometry.domain_size_v2()?)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let columns = usize::try_from(geometry.lde_column_count_v2()?)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let coordinate_count =
        u16::try_from(columns).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let values_per_block = usize::from(geometry.lde_values_per_block);
    let blocks = usize::try_from(geometry.lde_blocks_per_column_v2()?)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let mut reader = snapshot.begin_c0_replay_v2()?;
    let mut window = ZeroizingLeafWindowV2::new_v2(values_per_block, columns)?;
    let mut frontier = MerkleFrontierV2::new_v2(parameter_digest);
    for _ in 0..blocks {
        for column in 0..columns {
            let chunk = reader.read_next_block_column_v2()?;
            window.absorb_column_v2(column, &chunk)?;
        }
        for index in 0..values_per_block {
            let digest = initial_leaf_hash_v2(
                parameter_digest,
                domain_size,
                coordinate_count,
                window.leaf_v2(index)?,
            )?;
            frontier.push_v2(digest)?;
        }
    }
    let complete = reader.complete_v2()?;
    Ok((frontier.finish_v2(domain_size)?, complete))
}
/// Move-only, non-authorizing C0-root prerequisite retaining accepted replay.
pub(super) struct InitialC0RootPreparedV2 {
    accepted_c0: Option<QPcsC0CompleteV2>,
    masks: Option<MaskSpoolSealedV2>,
    context: PublicSpoolContextV2,
    parameter_digest: [u8; 32],
    initial_root: [u8; 32],
}
impl InitialC0RootPreparedV2 {
    pub(super) const fn parameter_digest_v2(&self) -> [u8; 32] {
        self.parameter_digest
    }
    pub(super) const fn initial_root_v2(&self) -> [u8; 32] {
        self.initial_root
    }
}
impl CoefficientsSealedV2 {
    pub(super) fn prepare_initial_c0_root_v2(
        self,
        directory: &Path,
        authority: InitialC0AuthorityV2,
    ) -> Result<InitialC0RootPreparedV2, ProverPrerequisiteErrorV2> {
        match authority {
            InitialC0AuthorityV2::Production {
                ntt,
                transpose: _transpose,
                initial_root: _initial_root,
            } => match ntt {},
            #[cfg(test)]
            InitialC0AuthorityV2::TestOnly => {}
        }
        let geometry = self
            .stage
            .as_ref()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?
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
        prepare_initial_c0_operation_v2(self, directory)
    }
    #[cfg(test)]
    fn prepare_test_geometry_v2(
        self,
        directory: &Path,
    ) -> Result<InitialC0RootPreparedV2, ProverPrerequisiteErrorV2> {
        prepare_initial_c0_operation_v2(self, directory)
    }
}
fn prepare_initial_c0_operation_v2(
    mut sealed: CoefficientsSealedV2,
    directory: &Path,
) -> Result<InitialC0RootPreparedV2, ProverPrerequisiteErrorV2> {
    let mut stage = sealed
        .stage
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let masks = sealed
        .masks
        .take()
        .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
    let geometry = stage.geometry;
    let parameter_digest = stage.parameter_digest;
    let context = sealed.context;
    let domain_size = usize::try_from(geometry.domain_size_v2()?)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let ring_degree = usize::try_from(geometry.ring_degree)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    if domain_size
        != ring_degree
            .checked_mul(4)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let mut column_writer =
        InitialColumnWriterV2::create_v2(directory, geometry, parameter_digest, context)?;
    let mut buffer = ZeroizingNttBufferV2::new_v2(domain_size)?;
    for limb in 0..geometry.limb_count_v2()? {
        let modulus = geometry.moduli[usize::from(limb)];
        let field = Fq2ParametersV1::derive(modulus, usize::from(geometry.domain_log))
            .map_err(|_| ProverPrerequisiteErrorV2::InvalidNtt)?;
        for repetition in 0..OPENING_REPETITIONS_V2 {
            buffer.clear_v2();
            stage = replay_component_v2(stage, &mut buffer, 0, modulus)?;
            stage = replay_component_v2(stage, &mut buffer, ring_degree, modulus)?;
            ntt_in_place_v2(&mut buffer.values, field)?;
            column_writer.expect_next_column_v2(fixed_row_column_v2(
                limb,
                repetition,
                LdeRowRoleV2::Product,
            )?)?;
            write_ntt_column_v2(&mut column_writer, &buffer)?;
            buffer.clear_v2();
            stage = replay_component_v2(stage, &mut buffer, 0, modulus)?;
            ntt_in_place_v2(&mut buffer.values, field)?;
            column_writer.expect_next_column_v2(fixed_row_column_v2(
                limb,
                repetition,
                LdeRowRoleV2::Quotient,
            )?)?;
            write_ntt_column_v2(&mut column_writer, &buffer)?;
        }
    }
    drop(buffer);
    let column_snapshot = column_writer.seal_v2()?;
    let mut transpose = column_snapshot.begin_transpose_v2(stage, context)?;
    for _ in 0..transpose.descriptor.slot_count {
        transpose.copy_next_block_v2()?;
    }
    let stage = transpose.complete_v2()?;
    let snapshot = stage.seal_lde_v2()?;
    let (initial_root, accepted_c0) = build_initial_root_v2(snapshot, geometry, parameter_digest)?;
    Ok(InitialC0RootPreparedV2 {
        accepted_c0: Some(accepted_c0),
        masks: Some(masks),
        context,
        parameter_digest,
        initial_root,
    })
}
#[cfg(test)]
static ZEROIZING_NTT_BUFFER_DROPS_V2: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static ZEROIZING_LEAF_WINDOW_DROPS_V2: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[path = "c0_v2/post_root_v2.rs"]
mod post_root_v2;
#[cfg(test)]
#[path = "c0_v2_tests.rs"]
mod tests;
