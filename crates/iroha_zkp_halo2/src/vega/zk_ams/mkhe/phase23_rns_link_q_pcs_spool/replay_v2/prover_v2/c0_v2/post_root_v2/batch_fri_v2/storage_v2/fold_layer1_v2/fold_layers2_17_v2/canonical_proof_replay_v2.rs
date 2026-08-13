//! Sequential final-proof replay adapter shared by B1 through B17.

use crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::CanonicalProofSectionV2;

use super::*;

pub(in super::super::super::super) struct FriLayerCanonicalProofReplayV2 {
    owner: Option<FriLayerRootedV2>,
    purpose: Option<CanonicalTreePurposeBoundV2>,
    terminal_replay_complete: Option<FriLayerReplayCompleteV2>,
    shape: CanonicalTreeReplayShapeV2,
    next_column: u64,
    ordinal: u8,
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
}

fn terminal_marker_matches_v2(
    owner: &FriLayerRootedV2,
    marker: &FriLayerReplayCompleteV2,
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
) -> bool {
    marker.binding.round.layer == 17
        && marker.binding.round.root == owner.root
        && marker.binding.round.batch_schedule_digest == batch_schedule_digest
        && marker.binding.round.fold_schedule_digest == fold_schedule_digest
        && marker.binding.layer0_binding.batch_schedule_digest == batch_schedule_digest
        && same_layer0_binding_v2(marker.binding.layer0_binding, owner.layer0_binding)
}

impl FriLayerRootedV2 {
    pub(in super::super::super::super) fn begin_canonical_proof_replay_v2(
        mut self,
        master_binding: [u8; 32],
        section: CanonicalProofSectionV2,
        expected_root: [u8; 32],
        purpose: CanonicalTreeReplayPurposeV2,
        terminal_replay_complete: Option<FriLayerReplayCompleteV2>,
        batch_schedule_digest: [u8; 32],
        fold_schedule_digest: [u8; 32],
    ) -> Result<FriLayerCanonicalProofReplayV2, ProverPrerequisiteErrorV2> {
        self.validate_v2()?;
        let layer = self.descriptor.layer;
        let marker_matches = match (&terminal_replay_complete, layer) {
            (Some(marker), 17) => terminal_marker_matches_v2(
                &self,
                marker,
                batch_schedule_digest,
                fold_schedule_digest,
            ),
            (None, 1..=16) => true,
            _ => false,
        };
        if !marker_matches
            || section.ordinal_v2() != layer + 2
            || section.merkle_layer_v2() != layer
            || section.length_v2() != self.descriptor.logical_length as u32
            || expected_root != self.root
            || self.descriptor.columns != REPLAY_COLUMNS_V2
            || u64::from(self.descriptor.values_per_block) * self.descriptor.blocks_per_column
                != self.descriptor.logical_length
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let purpose = purpose.bind_v2(master_binding, section, expected_root)?;
        Ok(FriLayerCanonicalProofReplayV2 {
            purpose: Some(purpose),
            terminal_replay_complete,
            shape: CanonicalTreeReplayShapeV2 {
                length: section.length_v2(),
                columns: self.descriptor.columns,
                values_per_block: self.descriptor.values_per_block,
            },
            owner: Some(self),
            next_column: 0,
            ordinal: section.ordinal_v2(),
            batch_schedule_digest,
            fold_schedule_digest,
        })
    }
}

impl CanonicalTreeReplayV2 for FriLayerCanonicalProofReplayV2 {
    type Owner = FriLayerRootedV2;

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
        match owner.descriptor.layer {
            17 => {
                let marker = self
                    .terminal_replay_complete
                    .take()
                    .ok_or(ProverPrerequisiteErrorV2::InvalidCanonicalProof)?;
                if !terminal_marker_matches_v2(
                    &owner,
                    &marker,
                    self.batch_schedule_digest,
                    self.fold_schedule_digest,
                ) {
                    return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
                }
            }
            1..=16 if self.terminal_replay_complete.is_none() => {}
            _ => return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof),
        }
        purpose.complete_v2(self.ordinal)?;
        Ok(owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_guards_require_the_b17_terminal_completion_marker() {
        let source = include_str!("canonical_proof_replay_v2.rs");
        assert!(source.contains("(Some(marker), 17)"));
        assert!(source.contains("(None, 1..=16)"));
        assert!(source.contains("terminal_marker_matches_v2("));
        assert!(source.contains("batch_schedule_digest"));
        assert!(source.contains("fold_schedule_digest"));
        assert!(!source.contains("Clone for FriLayerCanonicalProofReplayV2"));
    }
}
