//! Test-only commitment projection from actual retained execution ownership.
//!
//! This borrows a witness and checks its exact sealed wire; it neither supplies
//! finality nor consumes the unfinished State publication owner.

use super::StateBlock;
use crate::{block::ValidBlock, sumeragi::exec};
use iroha_data_model::block::consensus_v2::ExecutionCommitment;

impl StateBlock<'_> {
    /// Project the consensus commitment of this block's actual retained execution.
    ///
    /// Test support only: the caller supplies no witness, roots, or manifests.
    /// All source, output, and State publication owners remain retained, and a
    /// successful projection grants no permission to publish this State block.
    ///
    /// # Errors
    /// Rejects absent or replay-owned witnesses, foreign proposals, changed sealed
    /// wire or World values, and inconsistent source or witness ownership.
    #[doc(hidden)]
    pub fn execution_commitment_for_testing(
        &self,
        valid: &ValidBlock,
    ) -> Result<ExecutionCommitment, String> {
        if self.authenticated_replay_commit {
            return Err("test projection requires a locally retained execution witness".into());
        }
        let witness = self
            .exec_witness
            .as_ref()
            .ok_or("test projection requires a captured execution witness")?;
        let block = valid.as_ref();
        self.verify_execution_output_seal(block)?;
        let inventory = self.verified_fastpq_source_inventory_for_capture()?;
        self.verify_cached_ordinary_witness_content(&inventory)?;
        let native_manifest =
            exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
                block,
                self.staged_merge_entry(),
            )?;
        let lane_manifest = exec::LaneFinalityManifestV1::from_result_bearing_block(block)?;
        exec::execution_commitment_from_validated_block(
            witness,
            &native_manifest,
            &lane_manifest,
            block,
        )
        .map_err(str::to_owned)
    }
}

#[cfg(test)]
#[path = "execution_commitment_test_support_tests.rs"]
mod tests;
