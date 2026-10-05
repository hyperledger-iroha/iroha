//! Scoped canonical trigger readers owned by the actual trigger Set.
//!
//! These readers use the same borrowed semantic preimages as the World
//! projection. Their paired roots remain table-local and non-finalized.

use super::*;
use crate::state::authority_registry::leaf::{CanonicalTablePairedSnapshot, LeafError, LeafLimits};

impl Set {
    /// Capture data only after the complete original action inverse validates both images.
    pub(crate) fn capture_data_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<CanonicalTablePairedSnapshot, LeafError> {
        self.capture_action_authority_table(ActionTable::Data, limits, budget)
    }
    /// Capture pipeline from the same retained ten-owner action relation.
    pub(crate) fn capture_pipeline_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<CanonicalTablePairedSnapshot, LeafError> {
        self.capture_action_authority_table(ActionTable::Pipeline, limits, budget)
    }
    /// Capture time with unchanged canonical retry policy and retry state bytes.
    pub(crate) fn capture_time_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<CanonicalTablePairedSnapshot, LeafError> {
        self.capture_action_authority_table(ActionTable::Time, limits, budget)
    }
    /// Capture explicit calls from the same checked original action projection.
    pub(crate) fn capture_by_call_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<CanonicalTablePairedSnapshot, LeafError> {
        self.capture_action_authority_table(ActionTable::ByCall, limits, budget)
    }
    fn capture_action_authority_table(
        &self,
        table: ActionTable,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<CanonicalTablePairedSnapshot, LeafError> {
        let mut checked = CheckedActions::capture(self, action_source_work(limits), budget)?;
        let outcome = checked.encode(table, limits);
        let current = checked.matches_current();
        drop(checked);
        if !current? {
            return Err(TriggerContractError::Publication(
                mv::PublicationPreparationError::Changed,
            )
            .into());
        }
        outcome
    }

    /// Capture original contract bytecode after validating its derived indexes.
    pub(crate) fn capture_contracts_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> core::result::Result<CanonicalTablePairedSnapshot, LeafError> {
        let mut checked = CheckedContracts::capture(self, contract_source_work(limits), budget)?;
        let outcome = checked.encode(limits);
        let current = checked.matches_current();
        drop(checked);
        if !current? {
            return Err(TriggerContractError::Publication(
                mv::PublicationPreparationError::Changed,
            )
            .into());
        }
        outcome
    }
}

#[cfg(test)]
mod custody_tests {
    use super::*;

    #[test]
    fn malformed_contract_source_is_not_a_codec_failure_or_a_partial_table() {
        let set = Set::default();
        let blob = IvmBytecode::from_compiled(vec![1, 2, 3]);
        let key = HashOf::new(&blob);
        let mut block = set.block();
        block.contracts.insert(
            key,
            IvmBytecodeEntry {
                original_contract: blob,
                code_hash: Hash::new(b"incorrect derived contract hash"),
                count: NonZeroU64::MIN,
            },
        );
        block.commit();
        let pool = iroha_allocation::AllocationBudget::new(64 * 1024);
        let result = set.capture_contracts_authority_table(
            LeafLimits {
                max_tables: 1,
                max_rows: 1,
                max_payload_bytes: 1024,
                max_ordered_table_bytes: 4096,
                max_streamed_value_bytes: 4096,
            },
            &pool,
        );
        assert!(
            matches!(result, Err(LeafError::SourceValidation(ref message))
            if message == "trigger contract code hash does not match its original bytecode")
        );
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(pool.peak_reserved_bytes(), 0);
    }
}
