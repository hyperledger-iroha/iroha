//! Generic authenticated storage, replay, and rooting for qPCS B1 through B17.
use core::sync::atomic;
use std::path::Path;
use iroha_confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};
use crate::vega::{
    sponge::Keccak256, zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::ProverFriRoundContextV2,
};
use super::*;
const FRI_CONTINUATION_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.batch-fri.continuation.context\0";
const FRI_CONTINUATION_LEAF_BYTES_V2: usize = 380 * 16;
#[derive(Clone, Copy)]
struct FriRoundStorageBindingV2 {
    layer0_binding: FriLayer0BindingV2,
    round: ProverFriRoundContextV2,
}
fn same_round_binding_v2(left: FriRoundStorageBindingV2, right: FriRoundStorageBindingV2) -> bool {
    same_layer0_binding_v2(left.layer0_binding, right.layer0_binding) && left.round == right.round
}
fn continuation_context_digest_v2(
    descriptor: StorageLayoutDescriptorV2,
    binding: FriRoundStorageBindingV2,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    let destination_layer = binding
        .round
        .layer
        .checked_add(1)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    if !(2..=17).contains(&destination_layer)
        || descriptor
            != fri_layer_layout_v2(binding.layer0_binding.parameter_digest, destination_layer)?
    {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
    }
    let mut hash = Keccak256::new();
    hash.update(FRI_CONTINUATION_CONTEXT_DOMAIN_V2);
    hash.update(&[
        Q_PCS_SPOOL_VERSION_V2,
        3,
        binding.round.layer,
        destination_layer,
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
    hash.update(&binding.round.pre_root_transcript);
    hash.update(&binding.round.post_root_transcript);
    hash.update(&binding.layer0_binding.batch_schedule_digest);
    hash.update(&binding.round.prior_fold_schedule_digest);
    hash.update(&binding.round.fold_schedule_digest);
    hash.update(&binding.round.root);
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
    }
    Ok(digest)
}
enum FriLayerLineageV2 {
    Layer1 {
        binding: FriLayer1BindingV2,
        replay_complete: FriLayer0ReplayCompleteV2,
    },
    Continuation {
        binding: FriRoundStorageBindingV2,
        replay_complete: FriLayerReplayCompleteV2,
    },
}
pub(in super::super::super) struct FriLayerRootedV2 {
    snapshot: ConfidentialSpoolSnapshotV1,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    layer0_binding: FriLayer0BindingV2,
    lineage: FriLayerLineageV2,
    pub(in super::super::super) root: [u8; 32],
}
impl FriLayerRootedV2 {
    pub(in super::super::super) fn from_layer1_v2(owner: FriLayer1RootedV2) -> Self {
        Self {
            snapshot: owner.snapshot,
            descriptor: owner.descriptor,
            context_digest: owner.context_digest,
            layer0_binding: owner.binding.layer0_binding,
            lineage: FriLayerLineageV2::Layer1 {
                binding: owner.binding,
                replay_complete: owner.replay_complete,
            },
            root: owner.root,
        }
    }
    fn validate_v2(&mut self) -> Result<(), ProverPrerequisiteErrorV2> {
        let context_matches = match &self.lineage {
            FriLayerLineageV2::Layer1 {
                binding,
                replay_complete,
            } => {
                same_layer1_binding_v2(*binding, replay_complete.binding)
                    && fri1_context_digest_v2(self.descriptor, *binding)? == self.context_digest
            }
            FriLayerLineageV2::Continuation {
                binding,
                replay_complete,
            } => {
                same_round_binding_v2(*binding, replay_complete.binding)
                    && continuation_context_digest_v2(self.descriptor, *binding)?
                        == self.context_digest
            }
        };
        if !context_matches
            || self.root == [0; 32]
            || self.descriptor.role != StorageRoleV2::FriLayer
            || !(1..=17).contains(&self.descriptor.layer)
            || self.descriptor
                != fri_layer_layout_v2(self.layer0_binding.parameter_digest, self.descriptor.layer)?
            || self.snapshot.slot_count_v1() != self.descriptor.slot_count
            || self.snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || self.snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        Ok(())
    }
    pub(in super::super::super) fn begin_fold_replay_v2(
        mut self,
        round: ProverFriRoundContextV2,
    ) -> Result<FriLayerFoldReplayV2, ProverPrerequisiteErrorV2> {
        self.validate_v2()?;
        if round.layer != self.descriptor.layer || round.root != self.root {
            return Err(ProverPrerequisiteErrorV2::InvalidPostRootTranscript);
        }
        let binding = FriRoundStorageBindingV2 {
            layer0_binding: self.layer0_binding,
            round,
        };
        let pair_blocks = if self.descriptor.blocks_per_column >= 2 {
            self.descriptor.blocks_per_column / 2
        } else {
            1
        };
        let values_per_half = if self.descriptor.blocks_per_column >= 2 {
            self.descriptor.values_per_block
        } else {
            self.descriptor.values_per_block / 2
        };
        Ok(FriLayerFoldReplayV2 {
            owner: Some(self),
            binding,
            pair_blocks,
            values_per_half,
            next_pair_block: 0,
            next_column: 0,
        })
    }
}
pub(in super::super::super) struct FriLayerFoldPairV2 {
    lower: AuthenticatedReplayChunkV2,
    upper: Option<AuthenticatedReplayChunkV2>,
    values_per_half: u16,
}
impl FriLayerFoldPairV2 {
    pub(in super::super::super) fn positive_v2(&self) -> &[u8] {
        let bytes = usize::from(self.values_per_half) * 16;
        &self.lower.bytes_v2()[..bytes]
    }
    pub(in super::super::super) fn negative_v2(&self) -> &[u8] {
        let bytes = usize::from(self.values_per_half) * 16;
        match &self.upper {
            Some(upper) => &upper.bytes_v2()[..bytes],
            None => &self.lower.bytes_v2()[bytes..2 * bytes],
        }
    }
    pub(in super::super::super) fn terminal_in_place_v2(
        &mut self,
    ) -> Result<&mut [u8], ProverPrerequisiteErrorV2> {
        if self.upper.is_some() || self.values_per_half != 2 || self.lower.bytes_v2().len() != 64 {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        Ok(self.lower.chunk.as_mut_slice_v1())
    }
}
pub(in super::super::super) struct FriLayerFoldReplayV2 {
    owner: Option<FriLayerRootedV2>,
    binding: FriRoundStorageBindingV2,
    pair_blocks: u64,
    values_per_half: u16,
    next_pair_block: u64,
    next_column: u16,
}
impl FriLayerFoldReplayV2 {
    pub(in super::super::super) const fn layer0_binding_v2(&self) -> FriLayer0BindingV2 {
        self.binding.layer0_binding
    }
    pub(in super::super::super) fn read_next_pair_v2(
        &mut self,
        pair_block: u64,
        column: u16,
    ) -> Result<FriLayerFoldPairV2, ProverPrerequisiteErrorV2> {
        let mut owner = self
            .owner
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if pair_block != self.next_pair_block
            || column != self.next_column
            || pair_block >= self.pair_blocks
            || column >= owner.descriptor.columns
        {
            return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
        }
        let lower_slot = pair_block * u64::from(owner.descriptor.columns) + u64::from(column);
        let lower = AuthenticatedReplayChunkV2 {
            chunk: owner
                .snapshot
                .read_slot_v1(lower_slot, owner.context_digest)?,
        };
        let upper = if owner.descriptor.blocks_per_column >= 2 {
            let upper_slot = (pair_block + self.pair_blocks) * u64::from(owner.descriptor.columns)
                + u64::from(column);
            Some(AuthenticatedReplayChunkV2 {
                chunk: owner
                    .snapshot
                    .read_slot_v1(upper_slot, owner.context_digest)?,
            })
        } else {
            None
        };
        self.next_column += 1;
        if self.next_column == owner.descriptor.columns {
            self.next_column = 0;
            self.next_pair_block += 1;
        }
        self.owner = Some(owner);
        Ok(FriLayerFoldPairV2 {
            lower,
            upper,
            values_per_half: self.values_per_half,
        })
    }
    pub(in super::super::super) fn complete_v2(
        mut self,
    ) -> Result<(FriLayerRootedV2, FriLayerReplayCompleteV2), ProverPrerequisiteErrorV2> {
        let owner = self
            .owner
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_pair_block != self.pair_blocks || self.next_column != 0 {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        Ok((
            owner,
            FriLayerReplayCompleteV2 {
                binding: self.binding,
            },
        ))
    }
}
pub(in super::super::super) struct FriLayerReplayCompleteV2 {
    binding: FriRoundStorageBindingV2,
}
pub(in super::super::super) struct FriLayerWriterV2 {
    writer: Option<ConfidentialSpoolWriterV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    binding: FriRoundStorageBindingV2,
    next_slot: u64,
}
impl FriLayerWriterV2 {
    pub(in super::super::super) fn create_v2(
        directory: &Path,
        round: ProverFriRoundContextV2,
        layer0_binding: FriLayer0BindingV2,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let binding = FriRoundStorageBindingV2 {
            layer0_binding,
            round,
        };
        let destination_layer = round
            .layer
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let descriptor = fri_layer_layout_v2(layer0_binding.parameter_digest, destination_layer)?;
        let context_digest = continuation_context_digest_v2(descriptor, binding)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            descriptor.slot_count,
            descriptor.plaintext_bytes,
            context_digest,
        )?;
        if descriptor.columns != 380
            || descriptor.file_bytes != FRI_RELEASE_FILES_V2[usize::from(destination_layer)]
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
    pub(in super::super::super) fn push_next_v2(
        &mut self,
        pair_block: u64,
        column: u16,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut writer = self
            .writer
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let expected = pair_block * u64::from(self.descriptor.columns) + u64::from(column);
        if expected != self.next_slot
            || pair_block >= self.descriptor.blocks_per_column
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
    pub(in super::super::super) fn seal_v2(
        mut self,
        replay_complete: FriLayerReplayCompleteV2,
    ) -> Result<FriLayerSealedV2, ProverPrerequisiteErrorV2> {
        let writer = self
            .writer
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_slot != self.descriptor.slot_count
            || !same_round_binding_v2(self.binding, replay_complete.binding)
        {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        let snapshot = writer.seal_v1()?;
        if snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        Ok(FriLayerSealedV2 {
            snapshot: Some(snapshot),
            descriptor: self.descriptor,
            context_digest: self.context_digest,
            binding: self.binding,
            replay_complete: Some(replay_complete),
        })
    }
}
pub(in super::super::super) fn continuation_leaf_hash_v2(
    parameter_digest: [u8; 32],
    layer: u8,
    values: &[u8],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if !(2..=17).contains(&layer) || values.len() != FRI_CONTINUATION_LEAF_BYTES_V2 {
        return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
    }
    let length = u32::try_from(REPLAY_DOMAIN_VALUES_V2 >> layer)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0");
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[3, layer]);
    hash.update(&length.to_be_bytes());
    hash.update(&380_u16.to_be_bytes());
    hash.update(values);
    Ok(hash.finalize())
}
pub(in super::super::super) fn continuation_node_hash_v2(
    parameter_digest: [u8; 32],
    layer: u8,
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if !(2..=17).contains(&layer) {
        return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
    }
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0");
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[
        3,
        layer,
        u8::try_from(height).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
    ]);
    hash.update(&left);
    hash.update(&right);
    Ok(hash.finalize())
}
struct FriContinuationFrontierV2 {
    nodes: [[u8; 32]; 18],
    occupied: u32,
    leaves: usize,
    parameter_digest: [u8; 32],
    layer: u8,
}
impl FriContinuationFrontierV2 {
    const fn new_v2(parameter_digest: [u8; 32], layer: u8) -> Self {
        Self {
            nodes: [[0; 32]; 18],
            occupied: 0,
            leaves: 0,
            parameter_digest,
            layer,
        }
    }
    fn push_v2(&mut self, mut digest: [u8; 32]) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut level = 0;
        let mut prior = self.leaves;
        while prior & 1 == 1 {
            digest = continuation_node_hash_v2(
                self.parameter_digest,
                self.layer,
                level + 1,
                self.nodes[level],
                digest,
            )?;
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
    fn finish_v2(self, expected_leaves: usize) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
        let level = expected_leaves.ilog2() as usize;
        if self.leaves != expected_leaves
            || self.occupied != 1_u32 << level
            || self.nodes[level] == [0; 32]
        {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        Ok(self.nodes[level])
    }
}
struct ZeroizingContinuationWindowV2 {
    bytes: Vec<u8>,
}
impl ZeroizingContinuationWindowV2 {
    fn new_v2(values_per_slot: u16) -> Result<Self, ProverPrerequisiteErrorV2> {
        let length = usize::from(values_per_slot)
            .checked_mul(FRI_CONTINUATION_LEAF_BYTES_V2)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if length > 1_024 * FRI_CONTINUATION_LEAF_BYTES_V2 {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Geometry);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if bytes.capacity() != length {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        bytes.resize(length, 0);
        Ok(Self { bytes })
    }
    fn absorb_v2(&mut self, column: u16, chunk: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
        for (lane, value) in chunk.chunks_exact(16).enumerate() {
            let start = lane * FRI_CONTINUATION_LEAF_BYTES_V2 + usize::from(column) * 16;
            self.bytes[start..start + 16].copy_from_slice(value);
        }
        Ok(())
    }
}
impl Drop for ZeroizingContinuationWindowV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
    }
}
pub(in super::super::super) struct FriLayerSealedV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    descriptor: StorageLayoutDescriptorV2,
    context_digest: [u8; 32],
    binding: FriRoundStorageBindingV2,
    replay_complete: Option<FriLayerReplayCompleteV2>,
}
impl FriLayerSealedV2 {
    pub(in super::super::super) fn root_v2(
        mut self,
    ) -> Result<FriLayerRootedV2, ProverPrerequisiteErrorV2> {
        let mut snapshot = self
            .snapshot
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let replay_complete = self
            .replay_complete
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if !same_round_binding_v2(self.binding, replay_complete.binding)
            || continuation_context_digest_v2(self.descriptor, self.binding)? != self.context_digest
        {
            return Err(ProverPrerequisiteErrorV2::InvalidC0Context);
        }
        let mut window = ZeroizingContinuationWindowV2::new_v2(self.descriptor.values_per_block)?;
        let mut frontier = FriContinuationFrontierV2::new_v2(
            self.binding.layer0_binding.parameter_digest,
            self.descriptor.layer,
        );
        for block in 0..self.descriptor.blocks_per_column {
            for column in 0..self.descriptor.columns {
                let slot = block * u64::from(self.descriptor.columns) + u64::from(column);
                let chunk = AuthenticatedReplayChunkV2 {
                    chunk: snapshot.read_slot_v1(slot, self.context_digest)?,
                };
                window.absorb_v2(column, chunk.bytes_v2())?;
            }
            for lane in 0..usize::from(self.descriptor.values_per_block) {
                let start = lane * FRI_CONTINUATION_LEAF_BYTES_V2;
                frontier.push_v2(continuation_leaf_hash_v2(
                    self.binding.layer0_binding.parameter_digest,
                    self.descriptor.layer,
                    &window.bytes[start..start + FRI_CONTINUATION_LEAF_BYTES_V2],
                )?)?;
            }
        }
        let root = frontier.finish_v2(self.descriptor.logical_length as usize)?;
        Ok(FriLayerRootedV2 {
            snapshot,
            descriptor: self.descriptor,
            context_digest: self.context_digest,
            layer0_binding: self.binding.layer0_binding,
            lineage: FriLayerLineageV2::Continuation {
                binding: self.binding,
                replay_complete,
            },
            root,
        })
    }
}
pub(in super::super::super) struct ZeroizingFriTerminalV2 {
    bytes: [u8; 12_160],
}
impl ZeroizingFriTerminalV2 {
    pub(in super::super::super) const fn new_v2() -> Self {
        Self { bytes: [0; 12_160] }
    }
    pub(in super::super::super) fn scatter_v2(
        &mut self,
        column: u16,
        folded: &[u8],
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        if column >= 380 || folded.len() != 32 {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        let offset = usize::from(column) * 16;
        self.bytes[offset..offset + 16].copy_from_slice(&folded[..16]);
        let second = FRI_CONTINUATION_LEAF_BYTES_V2 + offset;
        self.bytes[second..second + 16].copy_from_slice(&folded[16..]);
        Ok(())
    }
    pub(in super::super::super) const fn bytes_v2(&self) -> &[u8; 12_160] {
        &self.bytes
    }
}
impl Drop for ZeroizingFriTerminalV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
    }
}
#[path = "fold_layers2_17_v2/canonical_proof_replay_v2.rs"]
mod canonical_proof_replay_v2;
pub(in super::super::super) use canonical_proof_replay_v2::*;
