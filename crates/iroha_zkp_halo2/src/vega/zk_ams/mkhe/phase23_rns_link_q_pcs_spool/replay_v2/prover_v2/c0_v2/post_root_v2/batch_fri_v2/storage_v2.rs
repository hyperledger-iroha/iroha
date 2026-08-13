//! Purpose-bound authenticated storage and root replay for qPCS B0.
use core::sync::atomic;
use std::path::Path;
use iroha_confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};
use crate::vega::sponge::Keccak256;
use super::*;
#[path = "storage_v2/fold_layer1_v2.rs"]
mod fold_layer1_v2;
pub(super) use fold_layer1_v2::*;
const FRI0_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.batch-fri.layer0.context\0";
const FRI_LEAF_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0";
const FRI_NODE_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0";
const FRI_TREE_KIND_V2: u8 = 3;
const FRI_LAYER0_V2: u8 = 0;
const FRI0_FRONTIER_NODES_V2: usize = 20;
const FRI0_LEAVES_V2: usize = 1 << 19;
const FRI0_LEAF_BYTES_V2: usize = 380 * 16;
const FRI0_WINDOW_BYTES_V2: usize = 1_024 * FRI0_LEAF_BYTES_V2;
#[derive(Clone, Copy)]
pub(super) struct FriLayer0BindingV2 {
    pub(super) parameter_digest: [u8; 32],
    pub(super) context: PublicSpoolContextV2,
    pub(super) initial_root: [u8; 32],
    pub(super) quotient_root: [u8; 32],
    pub(super) pre_layer_transcript: [u8; 32],
    pub(super) batch_schedule_digest: [u8; 32],
}
impl FriLayer0BindingV2 {
    pub(super) fn new_v2(
        parameter_digest: [u8; 32],
        context: PublicSpoolContextV2,
        initial_root: [u8; 32],
        quotient_root: [u8; 32],
        pre_layer_transcript: [u8; 32],
        batch_schedule_digest: [u8; 32],
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        context.validate_v2()?;
        if parameter_digest_v2(SpoolGeometryV2::release_v2())? != parameter_digest
            || initial_root == [0; 32]
            || quotient_root == [0; 32]
            || pre_layer_transcript == [0; 32]
            || batch_schedule_digest == [0; 32]
        {
            return Err(ProverPrerequisiteErrorV2::InvalidPostRootTranscript);
        }
        Ok(Self {
            parameter_digest,
            context,
            initial_root,
            quotient_root,
            pre_layer_transcript,
            batch_schedule_digest,
        })
    }
}
pub(super) fn fri0_context_digest_v2(
    descriptor: StorageLayoutDescriptorV2,
    binding: FriLayer0BindingV2,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if descriptor != fri_layer_layout_v2(binding.parameter_digest, FRI_LAYER0_V2)? {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let mut hash = Keccak256::new();
    hash.update(FRI0_CONTEXT_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2, FRI_TREE_KIND_V2, FRI_LAYER0_V2]);
    hash.update(&binding.parameter_digest);
    hash.update(&descriptor.mapping_digest);
    hash.update(&binding.context.sealed_source_transcript_digest);
    hash.update(&binding.context.source_algebra_binding_digest);
    hash.update(&binding.initial_root);
    hash.update(&binding.quotient_root);
    hash.update(&binding.pre_layer_transcript);
    hash.update(&binding.batch_schedule_digest);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    Ok(digest)
}
pub(super) struct FriLayer0WriterV2 {
    writer: Option<ConfidentialSpoolWriterV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    binding: FriLayer0BindingV2,
    next_slot: u64,
}
impl FriLayer0WriterV2 {
    pub(super) fn create_v2(
        directory: &Path,
        binding: FriLayer0BindingV2,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let descriptor = fri_layer_layout_v2(binding.parameter_digest, FRI_LAYER0_V2)?;
        let context_digest = fri0_context_digest_v2(descriptor, binding)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            descriptor.slot_count,
            descriptor.plaintext_bytes,
            context_digest,
        )?;
        if descriptor.logical_length != FRI0_LEAVES_V2 as u64
            || descriptor.columns != REPLAY_COLUMNS_V2
            || descriptor.values_per_block != REPLAY_BLOCK_VALUES_V2
            || descriptor.blocks_per_column != REPLAY_BLOCKS_PER_COLUMN_V2
            || descriptor.file_bytes != FRI_RELEASE_FILES_V2[0]
            || layout.file_len_v1() != descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
        }
        Ok(Self {
            writer: Some(ConfidentialSpoolWriterV1::create_in_v1(directory, layout)?),
            descriptor,
            context_digest,
            binding,
            next_slot: 0,
        })
    }
    pub(super) fn push_next_v2(
        &mut self,
        block: u64,
        column: u16,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut writer = self
            .writer
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let expected = block
            .checked_mul(u64::from(self.descriptor.columns))
            .and_then(|value| value.checked_add(u64::from(column)))
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if expected != self.next_slot
            || block >= self.descriptor.blocks_per_column
            || column >= self.descriptor.columns
            || chunk.len_v1() != self.descriptor.plaintext_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
        }
        let modulus = RELEASE_MODULI_V1[usize::from(column) / 10];
        for value in chunk.as_slice_v1().chunks_exact(16) {
            let c0 = u64::from_be_bytes(
                value[..8]
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
            );
            let c1 = u64::from_be_bytes(
                value[8..]
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidSourceShape)?,
            );
            if c0 >= modulus || c1 >= modulus {
                return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
            }
        }
        writer.write_slot_v1(self.next_slot, chunk)?;
        self.next_slot += 1;
        self.writer = Some(writer);
        Ok(())
    }
    pub(super) fn seal_v2(
        mut self,
        replay_permit: AuthenticatedReplayPermitV2,
    ) -> Result<FriLayer0SealedV2, ProverPrerequisiteErrorV2> {
        let writer = self
            .writer
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_slot != self.descriptor.slot_count {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        let snapshot = writer.seal_v1()?;
        if snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || snapshot.file_len_v1() != self.descriptor.file_bytes
            || fri0_context_digest_v2(self.descriptor, self.binding)? != self.context_digest
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        Ok(FriLayer0SealedV2 {
            snapshot: Some(snapshot),
            descriptor: self.descriptor,
            context_digest: self.context_digest,
            binding: self.binding,
            replay_permit: Some(replay_permit),
        })
    }
}
struct FriFrontierV2 {
    nodes: [[u8; 32]; FRI0_FRONTIER_NODES_V2],
    occupied: u32,
    leaves: usize,
    parameter_digest: [u8; 32],
}
pub(super) fn fri_leaf_hash_v2(
    parameter_digest: [u8; 32],
    values: &[u8],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if values.len() != FRI0_LEAF_BYTES_V2 {
        return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
    }
    let mut hash = Keccak256::new();
    hash.update(FRI_LEAF_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[FRI_TREE_KIND_V2, FRI_LAYER0_V2]);
    hash.update(&(FRI0_LEAVES_V2 as u32).to_be_bytes());
    hash.update(&REPLAY_COLUMNS_V2.to_be_bytes());
    hash.update(values);
    Ok(hash.finalize())
}
pub(super) fn fri_node_hash_v2(
    parameter_digest: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(FRI_NODE_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[
        FRI_TREE_KIND_V2,
        FRI_LAYER0_V2,
        u8::try_from(height).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
    ]);
    hash.update(&left);
    hash.update(&right);
    Ok(hash.finalize())
}
impl FriFrontierV2 {
    const fn new_v2(parameter_digest: [u8; 32]) -> Self {
        Self {
            nodes: [[0; 32]; FRI0_FRONTIER_NODES_V2],
            occupied: 0,
            leaves: 0,
            parameter_digest,
        }
    }
    fn push_v2(&mut self, mut digest: [u8; 32]) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut level = 0;
        let mut prior = self.leaves;
        while prior & 1 == 1 {
            digest = fri_node_hash_v2(self.parameter_digest, level + 1, self.nodes[level], digest)?;
            self.nodes[level] = [0; 32];
            self.occupied &= !(1_u32 << level);
            prior >>= 1;
            level += 1;
        }
        if level >= self.nodes.len() {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        self.nodes[level] = digest;
        self.occupied |= 1_u32 << level;
        self.leaves += 1;
        Ok(())
    }
    fn finish_v2(self) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
        let level = FRI0_LEAVES_V2.ilog2() as usize;
        if self.leaves != FRI0_LEAVES_V2
            || self.occupied != 1_u32 << level
            || self.nodes[level] == [0; 32]
        {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        Ok(self.nodes[level])
    }
}
struct ZeroizingFriWindowV2 {
    bytes: Vec<u8>,
}
impl ZeroizingFriWindowV2 {
    fn new_v2() -> Result<Self, ProverPrerequisiteErrorV2> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(FRI0_WINDOW_BYTES_V2)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if bytes.capacity() != FRI0_WINDOW_BYTES_V2 {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        bytes.resize(FRI0_WINDOW_BYTES_V2, 0);
        Ok(Self { bytes })
    }
    fn absorb_v2(&mut self, column: u16, chunk: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
        if chunk.len() != usize::from(REPLAY_BLOCK_VALUES_V2) * 16 {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        for (lane, value) in chunk.chunks_exact(16).enumerate() {
            let start = lane * FRI0_LEAF_BYTES_V2 + usize::from(column) * 16;
            self.bytes[start..start + 16].copy_from_slice(value);
        }
        Ok(())
    }
}
impl Drop for ZeroizingFriWindowV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
    }
}
pub(super) struct FriLayer0SealedV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    binding: FriLayer0BindingV2,
    replay_permit: Option<AuthenticatedReplayPermitV2>,
}
pub(super) struct FriLayer0RootedV2 {
    snapshot: ConfidentialSpoolSnapshotV1,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    binding: FriLayer0BindingV2,
    replay_permit: AuthenticatedReplayPermitV2,
    root: [u8; 32],
}
impl FriLayer0RootedV2 {
    pub(super) const fn root_digest_v2(&self) -> [u8; 32] {
        self.root
    }
}
impl FriLayer0SealedV2 {
    pub(super) fn root_v2(mut self) -> Result<FriLayer0RootedV2, ProverPrerequisiteErrorV2> {
        let mut snapshot = self
            .snapshot
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let replay_permit = self
            .replay_permit
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if fri0_context_digest_v2(self.descriptor, self.binding)? != self.context_digest
            || snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        let mut window = ZeroizingFriWindowV2::new_v2()?;
        let mut frontier = FriFrontierV2::new_v2(self.binding.parameter_digest);
        for block in 0..self.descriptor.blocks_per_column {
            for column in 0..self.descriptor.columns {
                let slot = block * u64::from(self.descriptor.columns) + u64::from(column);
                let chunk = AuthenticatedReplayChunkV2 {
                    chunk: snapshot.read_slot_v1(slot, self.context_digest)?,
                };
                window.absorb_v2(column, chunk.bytes_v2())?;
            }
            for lane in 0..usize::from(self.descriptor.values_per_block) {
                let start = lane * FRI0_LEAF_BYTES_V2;
                frontier.push_v2(fri_leaf_hash_v2(
                    self.binding.parameter_digest,
                    &window.bytes[start..start + FRI0_LEAF_BYTES_V2],
                )?)?;
            }
        }
        let root = frontier.finish_v2()?;
        Ok(FriLayer0RootedV2 {
            snapshot,
            descriptor: self.descriptor,
            context_digest: self.context_digest,
            binding: self.binding,
            replay_permit,
            root,
        })
    }
}
#[path = "storage_v2/canonical_proof_replay_v2.rs"]
mod canonical_proof_replay_v2;
pub(super) use canonical_proof_replay_v2::*;
