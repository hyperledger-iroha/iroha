//! Sequential final-proof replay adapter for B0.
use crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::CanonicalProofSectionV2;
use super::*;
pub(in super::super) struct FriLayer0CanonicalProofReplayV2 {
    owner: Option<FriLayer0RootedV2>,
    purpose: Option<CanonicalTreePurposeBoundV2>,
    shape: CanonicalTreeReplayShapeV2,
    next_column: u64,
    ordinal: u8,
}
impl FriLayer0RootedV2 {
    pub(in super::super) fn begin_canonical_proof_replay_v2(
        mut self,
        master_binding: [u8; 32],
        section: CanonicalProofSectionV2,
        expected_root: [u8; 32],
        purpose: CanonicalTreeReplayPurposeV2,
    ) -> Result<FriLayer0CanonicalProofReplayV2, ProverPrerequisiteErrorV2> {
        if section.ordinal_v2() != 2
            || section.merkle_layer_v2() != 0
            || section.length_v2() != self.descriptor.logical_length as u32
            || expected_root != self.root
            || self.root == [0; 32]
            || self.descriptor != fri_layer_layout_v2(self.binding.parameter_digest, 0)?
            || fri0_context_digest_v2(self.descriptor, self.binding)? != self.context_digest
            || self.snapshot.slot_count_v1() != self.descriptor.slot_count
            || self.snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || self.snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let purpose = purpose.bind_v2(master_binding, section, expected_root)?;
        let shape = CanonicalTreeReplayShapeV2 {
            length: section.length_v2(),
            columns: self.descriptor.columns,
            values_per_block: self.descriptor.values_per_block,
        };
        self.validate_canonical_shape_v2(shape)?;
        Ok(FriLayer0CanonicalProofReplayV2 {
            owner: Some(self),
            purpose: Some(purpose),
            shape,
            next_column: 0,
            ordinal: section.ordinal_v2(),
        })
    }
    fn validate_canonical_shape_v2(
        &self,
        shape: CanonicalTreeReplayShapeV2,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        if shape.columns != REPLAY_COLUMNS_V2
            || shape.values_per_block != REPLAY_BLOCK_VALUES_V2
            || u64::from(shape.length) / u64::from(shape.values_per_block)
                * u64::from(shape.columns)
                != self.descriptor.slot_count
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        Ok(())
    }
}
impl CanonicalTreeReplayV2 for FriLayer0CanonicalProofReplayV2 {
    type Owner = FriLayer0RootedV2;
    fn shape_v2(&self) -> Result<CanonicalTreeReplayShapeV2, ProverPrerequisiteErrorV2> {
        if self.owner.is_none() || self.purpose.is_none() {
            return Err(ProverPrerequisiteErrorV2::Poisoned);
        }
        Ok(self.shape)
    }
    fn read_next_column_v2(
        &mut self,
    ) -> Result<AuthenticatedReplayChunkV2, ProverPrerequisiteErrorV2> {
        let mut owner = self
            .owner
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_column >= owner.descriptor.slot_count {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let chunk = AuthenticatedReplayChunkV2 {
            chunk: owner
                .snapshot
                .read_slot_v1(self.next_column, owner.context_digest)?,
        };
        self.next_column += 1;
        self.owner = Some(owner);
        Ok(chunk)
    }
    fn complete_v2(mut self) -> Result<Self::Owner, ProverPrerequisiteErrorV2> {
        let owner = self
            .owner
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let purpose = self
            .purpose
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.next_column != owner.descriptor.slot_count {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        purpose.complete_v2(self.ordinal)?;
        Ok(owner)
    }
}
