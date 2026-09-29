//! Test-only commitment projection from actual retained execution ownership.
//!
//! This borrows a witness and checks its exact sealed wire; it neither supplies
//! finality nor consumes the unfinished State publication owner.

use super::StateBlock;
use crate::{block::ValidBlock, sumeragi::commitment};
use iroha_data_model::sumeragi_finality::ExecutionCommitment;

impl StateBlock<'_> {
    /// Project the execution component of native R from this original retained execution.
    ///
    /// Test support only: the caller supplies no witness, roots, or manifests.
    /// All source, output, and State publication owners remain retained, and a
    /// successful projection grants no permission to publish this State block.
    ///
    /// # Errors
    /// Rejects absent witnesses, foreign proposals, changed sealed
    /// wire or World values, and inconsistent source or witness ownership.
    #[doc(hidden)]
    pub fn execution_commitment_for_testing(
        &self,
        valid: &ValidBlock,
    ) -> Result<ExecutionCommitment, String> {
        let witness = self
            .exec_witness
            .as_ref()
            .ok_or("test projection requires a captured execution witness")?;
        let block = valid.as_ref();
        self.verify_execution_output_seal(block)?;
        let inventory = self.verified_fastpq_source_inventory_for_capture()?;
        self.verify_cached_ordinary_witness_content(&inventory)?;
        self.verify_sumeragi_lane_state_witness(witness)?;
        let transition = self.world_state_transition()?;
        commitment::execution_commitment(witness, block, &transition)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "execution_commitment_test_support_tests.rs"]
mod tests;
