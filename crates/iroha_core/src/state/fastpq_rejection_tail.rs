//! Read-only source-owner checks before signed rejection settlement is possible.

use super::{Hash, StateTransaction, output_capacity::ExecutionOutputPlanState};
use iroha_data_model::parameter::FastpqSourceLimitsV1;

impl StateTransaction<'_, '_> {
    /// Check the retained empty logical entry before any fee/body meter is opened.
    /// This reads existing ownership and the block-start intrinsic profile only.
    pub(crate) fn fastpq_rejection_tail_context(
        &self,
        hash: Hash,
    ) -> Result<FastpqSourceLimitsV1, String> {
        if matches!(
            self.block_execution_output_plan,
            Some(
                ExecutionOutputPlanState::Sealed(_)
                    | ExecutionOutputPlanState::Authorized(_)
                    | ExecutionOutputPlanState::Finalized(_)
                    | ExecutionOutputPlanState::Captured
                    | ExecutionOutputPlanState::Poisoned
            )
        ) {
            return Err("FASTPQ rejection-tail carrier is not executing".into());
        }
        if self.tx_call_hash != Some(hash) || self.execution_fee_meter.is_some() {
            return Err("FASTPQ rejection-tail context is not a fresh signed body".into());
        }
        if self
            .fastpq_transcripts
            .get(&hash)
            .is_some_and(|bundle| !bundle.is_empty())
            || self
                .pending_transfer_transcripts
                .iter()
                .any(|entry| entry.batch_hash == hash)
        {
            return Err("FASTPQ rejection-tail entry already has source occurrences".into());
        }
        self.fastpq_source_quota
            .require_empty_ordinary_entry(hash)?;
        Ok(self.fastpq_source_policy.0.intrinsic)
    }

    /// Retain an invariant/codec fault so Network cannot publish it as a rejected row.
    pub(crate) fn fail_fastpq_rejection_tail(&mut self, error: String) {
        self.fastpq_source_quota.fail_preparation(error);
        *self.block_execution_output_plan = Some(ExecutionOutputPlanState::Poisoned);
    }
}
