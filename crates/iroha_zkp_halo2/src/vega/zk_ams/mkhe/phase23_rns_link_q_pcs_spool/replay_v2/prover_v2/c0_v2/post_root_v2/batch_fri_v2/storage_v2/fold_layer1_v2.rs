//! Purpose-bound authenticated B0 replay and B1 storage/root construction.

use core::sync::atomic;
use std::path::Path;

use iroha_confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};

use crate::vega::sponge::Keccak256;

use super::*;

#[path = "fold_layer1_v2/fold_layers2_17_v2.rs"]
mod fold_layers2_17_v2;
pub(in super::super) use fold_layers2_17_v2::*;

const FRI1_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.batch-fri.layer1.context\0";
const FRI1_LEAF_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0";
const FRI1_NODE_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0";
const FRI1_TREE_KIND_V2: u8 = 3;
const FRI1_SOURCE_LAYER_V2: u8 = 0;
const FRI1_LAYER_V2: u8 = 1;
const FRI1_COLUMNS_V2: u16 = 380;
const FRI1_PAIR_BLOCKS_V2: u64 = 256;
const FRI1_VALUES_PER_BLOCK_V2: u16 = 1_024;
const FRI1_SLOT_COUNT_V2: u64 = FRI1_PAIR_BLOCKS_V2 * FRI1_COLUMNS_V2 as u64;
const FRI1_LEAVES_V2: usize = 1 << 18;
const FRI1_LEAF_BYTES_V2: usize = 380 * 16;
const FRI1_WINDOW_BYTES_V2: usize = 1_024 * FRI1_LEAF_BYTES_V2;
const FRI1_FRONTIER_NODES_V2: usize = 19;

const _: () = {
    assert!(FRI1_SLOT_COUNT_V2 == 97_280);
    assert!(FRI1_LEAVES_V2 == 262_144);
    assert!(FRI1_LEAF_BYTES_V2 == 6_080);
    assert!(FRI1_WINDOW_BYTES_V2 == 6_225_920);
    assert!(FRI1_FRONTIER_NODES_V2 * 32 == 608);
};

#[derive(Clone, Copy)]
pub(in super::super) struct FriLayer1BindingV2 {
    layer0_binding: FriLayer0BindingV2,
    layer0_root: [u8; 32],
    post_layer0_transcript: [u8; 32],
    layer0_fold_schedule_digest: [u8; 32],
}

fn same_layer0_binding_v2(left: FriLayer0BindingV2, right: FriLayer0BindingV2) -> bool {
    left.parameter_digest == right.parameter_digest
        && left.context.sealed_source_transcript_digest
            == right.context.sealed_source_transcript_digest
        && left.context.source_algebra_binding_digest == right.context.source_algebra_binding_digest
        && left.initial_root == right.initial_root
        && left.quotient_root == right.quotient_root
        && left.pre_layer_transcript == right.pre_layer_transcript
        && left.batch_schedule_digest == right.batch_schedule_digest
}

fn same_layer1_binding_v2(left: FriLayer1BindingV2, right: FriLayer1BindingV2) -> bool {
    same_layer0_binding_v2(left.layer0_binding, right.layer0_binding)
        && left.layer0_root == right.layer0_root
        && left.post_layer0_transcript == right.post_layer0_transcript
        && left.layer0_fold_schedule_digest == right.layer0_fold_schedule_digest
}

impl FriLayer1BindingV2 {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn new_v2(
        layer0_binding: FriLayer0BindingV2,
        layer0_root: [u8; 32],
        pre_layer_transcript: [u8; 32],
        post_layer0_transcript: [u8; 32],
        batch_schedule_digest: [u8; 32],
        layer0_fold_schedule_digest: [u8; 32],
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        layer0_binding.context.validate_v2()?;
        if layer0_binding.parameter_digest != parameter_digest_v2(SpoolGeometryV2::release_v2())?
            || layer0_binding.pre_layer_transcript != pre_layer_transcript
            || layer0_binding.batch_schedule_digest != batch_schedule_digest
            || layer0_root == [0; 32]
            || post_layer0_transcript == [0; 32]
            || layer0_fold_schedule_digest == [0; 32]
        {
            return Err(ProverPrerequisiteErrorV2::InvalidPostRootTranscript);
        }
        Ok(Self {
            layer0_binding,
            layer0_root,
            post_layer0_transcript,
            layer0_fold_schedule_digest,
        })
    }
}

pub(in super::super) fn fri1_context_digest_v2(
    descriptor: StorageLayoutDescriptorV2,
    binding: FriLayer1BindingV2,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if descriptor != fri_layer_layout_v2(binding.layer0_binding.parameter_digest, FRI1_LAYER_V2)? {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let mut hash = Keccak256::new();
    hash.update(FRI1_CONTEXT_DOMAIN_V2);
    hash.update(&[
        Q_PCS_SPOOL_VERSION_V2,
        FRI1_TREE_KIND_V2,
        FRI1_SOURCE_LAYER_V2,
        FRI1_LAYER_V2,
    ]);
    hash.update(&binding.layer0_binding.parameter_digest);
    hash.update(&descriptor.mapping_digest);
    hash.update(
        &binding
            .layer0_binding
            .context
            .sealed_source_transcript_digest,
    );
    hash.update(&binding.layer0_binding.context.source_algebra_binding_digest);
    hash.update(&binding.layer0_binding.initial_root);
    hash.update(&binding.layer0_binding.quotient_root);
    hash.update(&binding.layer0_binding.pre_layer_transcript);
    hash.update(&binding.layer0_binding.batch_schedule_digest);
    hash.update(&binding.layer0_root);
    hash.update(&binding.post_layer0_transcript);
    hash.update(&binding.layer0_fold_schedule_digest);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    Ok(digest)
}

pub(in super::super) struct FriLayer0FoldPairV2 {
    pub(in super::super) lower: AuthenticatedReplayChunkV2,
    pub(in super::super) upper: AuthenticatedReplayChunkV2,
}

pub(in super::super) struct FriLayer0FoldReplayV2 {
    owner: Option<FriLayer0RootedV2>,
    binding: FriLayer1BindingV2,
    next_pair_block: u64,
    next_column: u16,
}

pub(in super::super) struct FriLayer0ReplayCompleteV2 {
    binding: FriLayer1BindingV2,
}

impl FriLayer0RootedV2 {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn begin_layer1_fold_replay_v2(
        self,
        pre_layer_transcript: [u8; 32],
        post_layer0_transcript: [u8; 32],
        batch_schedule_digest: [u8; 32],
        layer0_fold_schedule_digest: [u8; 32],
        layer0_root: [u8; 32],
    ) -> Result<(FriLayer0FoldReplayV2, FriLayer1BindingV2), ProverPrerequisiteErrorV2> {
        let binding = FriLayer1BindingV2::new_v2(
            self.binding,
            layer0_root,
            pre_layer_transcript,
            post_layer0_transcript,
            batch_schedule_digest,
            layer0_fold_schedule_digest,
        )?;
        if self.root != layer0_root
            || self.root == [0; 32]
            || self.descriptor
                != fri_layer_layout_v2(self.binding.parameter_digest, FRI1_SOURCE_LAYER_V2)?
            || fri0_context_digest_v2(self.descriptor, self.binding)? != self.context_digest
            || self.snapshot.slot_count_v1() != self.descriptor.slot_count
            || self.snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || self.snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        Ok((
            FriLayer0FoldReplayV2 {
                owner: Some(self),
                binding,
                next_pair_block: 0,
                next_column: 0,
            },
            binding,
        ))
    }
}

impl FriLayer0FoldReplayV2 {
    pub(in super::super) fn read_next_pair_v2(
        &mut self,
        pair_block: u64,
        column: u16,
    ) -> Result<FriLayer0FoldPairV2, ProverPrerequisiteErrorV2> {
        let mut owner = self
            .owner
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if pair_block != self.next_pair_block
            || column != self.next_column
            || pair_block >= FRI1_PAIR_BLOCKS_V2
            || column >= FRI1_COLUMNS_V2
        {
            return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
        }
        let lower_slot = pair_block
            .checked_mul(u64::from(FRI1_COLUMNS_V2))
            .and_then(|value| value.checked_add(u64::from(column)))
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let upper_slot = pair_block
            .checked_add(FRI1_PAIR_BLOCKS_V2)
            .and_then(|value| value.checked_mul(u64::from(FRI1_COLUMNS_V2)))
            .and_then(|value| value.checked_add(u64::from(column)))
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let lower = AuthenticatedReplayChunkV2 {
            chunk: owner
                .snapshot
                .read_slot_v1(lower_slot, owner.context_digest)?,
        };
        let upper = AuthenticatedReplayChunkV2 {
            chunk: owner
                .snapshot
                .read_slot_v1(upper_slot, owner.context_digest)?,
        };
        self.next_column += 1;
        if self.next_column == FRI1_COLUMNS_V2 {
            self.next_column = 0;
            self.next_pair_block += 1;
        }
        self.owner = Some(owner);
        Ok(FriLayer0FoldPairV2 { lower, upper })
    }

    pub(in super::super) fn complete_v2(
        mut self,
    ) -> Result<(FriLayer0RootedV2, FriLayer0ReplayCompleteV2), ProverPrerequisiteErrorV2> {
        let owner = self
            .owner
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_pair_block != FRI1_PAIR_BLOCKS_V2 || self.next_column != 0 {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        Ok((
            owner,
            FriLayer0ReplayCompleteV2 {
                binding: self.binding,
            },
        ))
    }
}

pub(in super::super) struct FriLayer1WriterV2 {
    writer: Option<ConfidentialSpoolWriterV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    binding: FriLayer1BindingV2,
    next_slot: u64,
}

impl FriLayer1WriterV2 {
    pub(in super::super) fn create_v2(
        directory: &Path,
        binding: FriLayer1BindingV2,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let descriptor = fri_layer_layout_v2(binding.layer0_binding.parameter_digest, 1)?;
        let context_digest = fri1_context_digest_v2(descriptor, binding)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            descriptor.slot_count,
            descriptor.plaintext_bytes,
            context_digest,
        )?;
        if descriptor.logical_length != FRI1_LEAVES_V2 as u64
            || descriptor.columns != FRI1_COLUMNS_V2
            || descriptor.values_per_block != FRI1_VALUES_PER_BLOCK_V2
            || descriptor.blocks_per_column != FRI1_PAIR_BLOCKS_V2
            || descriptor.slot_count != FRI1_SLOT_COUNT_V2
            || descriptor.file_bytes != FRI_RELEASE_FILES_V2[1]
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

    pub(in super::super) fn push_next_v2(
        &mut self,
        pair_block: u64,
        column: u16,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut writer = self
            .writer
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let expected = pair_block
            .checked_mul(u64::from(FRI1_COLUMNS_V2))
            .and_then(|value| value.checked_add(u64::from(column)))
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if expected != self.next_slot
            || pair_block >= FRI1_PAIR_BLOCKS_V2
            || column >= FRI1_COLUMNS_V2
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

    pub(in super::super) fn seal_v2(
        mut self,
        replay_complete: FriLayer0ReplayCompleteV2,
    ) -> Result<FriLayer1SealedV2, ProverPrerequisiteErrorV2> {
        let writer = self
            .writer
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_slot != FRI1_SLOT_COUNT_V2
            || !same_layer1_binding_v2(self.binding, replay_complete.binding)
        {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        let snapshot = writer.seal_v1()?;
        if snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || snapshot.file_len_v1() != self.descriptor.file_bytes
            || fri1_context_digest_v2(self.descriptor, self.binding)? != self.context_digest
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        Ok(FriLayer1SealedV2 {
            snapshot: Some(snapshot),
            descriptor: self.descriptor,
            context_digest: self.context_digest,
            binding: self.binding,
            replay_complete: Some(replay_complete),
        })
    }
}

pub(in super::super) fn fri1_leaf_hash_v2(
    parameter_digest: [u8; 32],
    values: &[u8],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if values.len() != FRI1_LEAF_BYTES_V2 {
        return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
    }
    let mut hash = Keccak256::new();
    hash.update(FRI1_LEAF_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[FRI1_TREE_KIND_V2, FRI1_LAYER_V2]);
    hash.update(&(FRI1_LEAVES_V2 as u32).to_be_bytes());
    hash.update(&FRI1_COLUMNS_V2.to_be_bytes());
    hash.update(values);
    Ok(hash.finalize())
}

pub(in super::super) fn fri1_node_hash_v2(
    parameter_digest: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(FRI1_NODE_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[
        FRI1_TREE_KIND_V2,
        FRI1_LAYER_V2,
        u8::try_from(height).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
    ]);
    hash.update(&left);
    hash.update(&right);
    Ok(hash.finalize())
}

struct FriLayer1FrontierV2 {
    nodes: [[u8; 32]; FRI1_FRONTIER_NODES_V2],
    occupied: u32,
    leaves: usize,
    parameter_digest: [u8; 32],
}

impl FriLayer1FrontierV2 {
    const fn new_v2(parameter_digest: [u8; 32]) -> Self {
        Self {
            nodes: [[0; 32]; FRI1_FRONTIER_NODES_V2],
            occupied: 0,
            leaves: 0,
            parameter_digest,
        }
    }

    fn push_v2(&mut self, mut digest: [u8; 32]) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut level = 0;
        let mut prior = self.leaves;
        while prior & 1 == 1 {
            digest =
                fri1_node_hash_v2(self.parameter_digest, level + 1, self.nodes[level], digest)?;
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
        let level = FRI1_LEAVES_V2.ilog2() as usize;
        if self.leaves != FRI1_LEAVES_V2
            || self.occupied != 1_u32 << level
            || self.nodes[level] == [0; 32]
        {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        Ok(self.nodes[level])
    }
}

struct ZeroizingFriLayer1WindowV2 {
    bytes: Vec<u8>,
}

impl ZeroizingFriLayer1WindowV2 {
    fn new_v2() -> Result<Self, ProverPrerequisiteErrorV2> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(FRI1_WINDOW_BYTES_V2)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if bytes.capacity() != FRI1_WINDOW_BYTES_V2 {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        bytes.resize(FRI1_WINDOW_BYTES_V2, 0);
        Ok(Self { bytes })
    }

    fn absorb_v2(&mut self, column: u16, chunk: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
        if chunk.len() != usize::from(FRI1_VALUES_PER_BLOCK_V2) * 16 {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        for (lane, value) in chunk.chunks_exact(16).enumerate() {
            let start = lane * FRI1_LEAF_BYTES_V2 + usize::from(column) * 16;
            self.bytes[start..start + 16].copy_from_slice(value);
        }
        Ok(())
    }
}

impl Drop for ZeroizingFriLayer1WindowV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
    }
}

pub(in super::super) struct FriLayer1SealedV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    binding: FriLayer1BindingV2,
    replay_complete: Option<FriLayer0ReplayCompleteV2>,
}

pub(in super::super) struct FriLayer1RootedV2 {
    snapshot: ConfidentialSpoolSnapshotV1,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    binding: FriLayer1BindingV2,
    replay_complete: FriLayer0ReplayCompleteV2,
    root: [u8; 32],
}

impl FriLayer1RootedV2 {
    pub(in super::super) const fn root_digest_v2(&self) -> [u8; 32] {
        self.root
    }
}

impl FriLayer1SealedV2 {
    pub(in super::super) fn root_v2(
        mut self,
    ) -> Result<FriLayer1RootedV2, ProverPrerequisiteErrorV2> {
        let mut snapshot = self
            .snapshot
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let replay_complete = self
            .replay_complete
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if !same_layer1_binding_v2(self.binding, replay_complete.binding)
            || fri1_context_digest_v2(self.descriptor, self.binding)? != self.context_digest
            || snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        let mut window = ZeroizingFriLayer1WindowV2::new_v2()?;
        let mut frontier =
            FriLayer1FrontierV2::new_v2(self.binding.layer0_binding.parameter_digest);
        for block in 0..self.descriptor.blocks_per_column {
            for column in 0..self.descriptor.columns {
                let slot = block * u64::from(self.descriptor.columns) + u64::from(column);
                let chunk = AuthenticatedReplayChunkV2 {
                    chunk: snapshot.read_slot_v1(slot, self.context_digest)?,
                };
                window.absorb_v2(column, chunk.bytes_v2())?;
            }
            for lane in 0..usize::from(self.descriptor.values_per_block) {
                let start = lane * FRI1_LEAF_BYTES_V2;
                frontier.push_v2(fri1_leaf_hash_v2(
                    self.binding.layer0_binding.parameter_digest,
                    &window.bytes[start..start + FRI1_LEAF_BYTES_V2],
                )?)?;
            }
        }
        let root = frontier.finish_v2()?;
        Ok(FriLayer1RootedV2 {
            snapshot,
            descriptor: self.descriptor,
            context_digest: self.context_digest,
            binding: self.binding,
            replay_complete,
            root,
        })
    }
}
