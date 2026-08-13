//! Sequential final-proof replay adapters for C0 and Cq.
use super::*;
use crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::CanonicalProofSectionV2;
pub(in super::super) struct C0CanonicalProofReplayV2 {
    replay: Option<C0BatchReplayV2>,
    purpose: Option<CanonicalTreePurposeBoundV2>,
    shape: CanonicalTreeReplayShapeV2,
    next_column: u64,
    ordinal: u8,
}
pub(in super::super) struct CqCanonicalProofReplayV2 {
    owner: Option<QPcsCqStoredV2>,
    purpose: Option<CanonicalTreePurposeBoundV2>,
    shape: CanonicalTreeReplayShapeV2,
    next_column: u64,
    ordinal: u8,
}
impl QPcsC0StoredV2 {
    pub(in super::super) fn begin_canonical_proof_replay_v2(
        self,
        context: PublicSpoolContextV2,
        master_binding: [u8; 32],
        section: CanonicalProofSectionV2,
        expected_root: [u8; 32],
        purpose: CanonicalTreeReplayPurposeV2,
    ) -> Result<C0CanonicalProofReplayV2, ProverPrerequisiteErrorV2> {
        let shape = CanonicalTreeReplayShapeV2 {
            length: u32::try_from(self.geometry.domain_size_v2()?)
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            columns: u16::try_from(self.geometry.lde_column_count_v2()?)
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            values_per_block: self.geometry.lde_values_per_block,
        };
        if section.ordinal_v2() != 0
            || shape.length != section.length_v2()
            || shape.columns != REPLAY_COLUMNS_V2
            || shape.values_per_block != REPLAY_BLOCK_VALUES_V2
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let purpose = purpose.bind_v2(master_binding, section, expected_root)?;
        let replay = self.begin_c0_batch_replay_v2(context)?;
        Ok(C0CanonicalProofReplayV2 {
            replay: Some(replay),
            purpose: Some(purpose),
            shape,
            next_column: 0,
            ordinal: section.ordinal_v2(),
        })
    }
}
impl CanonicalTreeReplayV2 for C0CanonicalProofReplayV2 {
    type Owner = QPcsC0StoredV2;
    fn shape_v2(&self) -> Result<CanonicalTreeReplayShapeV2, ProverPrerequisiteErrorV2> {
        if self.replay.is_none() || self.purpose.is_none() {
            return Err(ProverPrerequisiteErrorV2::Poisoned);
        }
        Ok(self.shape)
    }
    fn read_next_column_v2(
        &mut self,
    ) -> Result<AuthenticatedReplayChunkV2, ProverPrerequisiteErrorV2> {
        let mut replay = self
            .replay
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let blocks = u64::from(self.shape.length) / u64::from(self.shape.values_per_block);
        let total = blocks * u64::from(self.shape.columns);
        if self.next_column >= total {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let block = self.next_column / u64::from(self.shape.columns);
        let column = (self.next_column % u64::from(self.shape.columns)) as u16;
        let chunk = replay.read_next_v2(block, column)?;
        self.next_column += 1;
        self.replay = Some(replay);
        Ok(chunk)
    }
    fn complete_v2(mut self) -> Result<Self::Owner, ProverPrerequisiteErrorV2> {
        let replay = self
            .replay
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let purpose = self
            .purpose
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let expected = u64::from(self.shape.length) / u64::from(self.shape.values_per_block)
            * u64::from(self.shape.columns);
        if self.next_column != expected {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let owner = replay.complete_v2()?;
        purpose.complete_v2(self.ordinal)?;
        Ok(owner)
    }
}
impl QPcsCqStoredV2 {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn begin_canonical_proof_replay_v2(
        self,
        context: PublicSpoolContextV2,
        parameter_digest: [u8; 32],
        initial_root: [u8; 32],
        master_binding: [u8; 32],
        section: CanonicalProofSectionV2,
        expected_root: [u8; 32],
        purpose: CanonicalTreeReplayPurposeV2,
    ) -> Result<CqCanonicalProofReplayV2, ProverPrerequisiteErrorV2> {
        let exact = cq_bound_layout_v2(
            parameter_digest,
            REPLAY_DOMAIN_VALUES_V2,
            REPLAY_COLUMNS_V2,
            REPLAY_BLOCK_VALUES_V2,
        )?;
        if section.ordinal_v2() != 1
            || self.descriptor != exact
            || self.snapshot_digest != *self.snapshot.snapshot_digest_v1()
            || cq_post_root_context_digest_v2(
                self.descriptor,
                context,
                parameter_digest,
                initial_root,
                self.pre_quotient_transcript,
            )? != self.context_digest
            || self.snapshot.slot_count_v1() != self.descriptor.slot_count
            || self.snapshot.plaintext_len_v1() != self.descriptor.plaintext_bytes
            || self.snapshot.file_len_v1() != self.descriptor.file_bytes
            || section.length_v2() != self.descriptor.logical_length as u32
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let purpose = purpose.bind_v2(master_binding, section, expected_root)?;
        Ok(CqCanonicalProofReplayV2 {
            owner: Some(self),
            purpose: Some(purpose),
            shape: CanonicalTreeReplayShapeV2 {
                length: section.length_v2(),
                columns: exact.columns,
                values_per_block: exact.values_per_block,
            },
            next_column: 0,
            ordinal: section.ordinal_v2(),
        })
    }
}
impl CanonicalTreeReplayV2 for CqCanonicalProofReplayV2 {
    type Owner = QPcsCqStoredV2;
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
        let blocks = owner.descriptor.blocks_per_column;
        let total = blocks * u64::from(owner.descriptor.columns);
        if self.next_column >= total {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let block = self.next_column / u64::from(owner.descriptor.columns);
        let column = (self.next_column % u64::from(owner.descriptor.columns)) as u16;
        let slot = u64::from(column) * blocks + block;
        let chunk = AuthenticatedReplayChunkV2 {
            chunk: owner.snapshot.read_slot_v1(slot, owner.context_digest)?,
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
