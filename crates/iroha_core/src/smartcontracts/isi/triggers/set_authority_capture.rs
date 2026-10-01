//! Scoped canonical trigger readers owned by the actual trigger Set.
//!
//! These readers use the same borrowed semantic preimages as the World
//! projection. Their paired roots remain table-local and non-finalized.

use super::*;
use crate::state::authority_registry::leaf::{
    CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits,
};

impl Set {
    /// Capture the data action table from its borrowed semantic projection.
    pub(crate) fn capture_data_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> core::result::Result<CanonicalTablePairedSnapshot, LeafError> {
        let rows = self.data_triggers.view();
        CanonicalTableLeafSet::paired_semantic_table_from_rows(
            "triggers.data",
            "iroha:state:trigger-data-action:v1",
            limits,
            budget,
            rows.iter(),
            BorrowedWorldAction::new,
        )
    }

    /// Capture the pipeline action table from its borrowed semantic projection.
    pub(crate) fn capture_pipeline_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> core::result::Result<CanonicalTablePairedSnapshot, LeafError> {
        let rows = self.pipeline_triggers.view();
        CanonicalTableLeafSet::paired_semantic_table_from_rows(
            "triggers.pipeline",
            "iroha:state:trigger-pipeline-action:v1",
            limits,
            budget,
            rows.iter(),
            BorrowedWorldAction::new,
        )
    }

    /// Capture the time action table, including retry policy and retry state.
    pub(crate) fn capture_time_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> core::result::Result<CanonicalTablePairedSnapshot, LeafError> {
        let rows = self.time_triggers.view();
        CanonicalTableLeafSet::paired_semantic_table_from_rows(
            "triggers.time",
            "iroha:state:trigger-time-action:v1",
            limits,
            budget,
            rows.iter(),
            BorrowedWorldAction::new,
        )
    }

    /// Capture the explicit-call action table from its semantic preimages.
    pub(crate) fn capture_by_call_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> core::result::Result<CanonicalTablePairedSnapshot, LeafError> {
        let rows = self.by_call_triggers.view();
        CanonicalTableLeafSet::paired_semantic_table_from_rows(
            "triggers.by_call",
            "iroha:state:trigger-by-call-action:v1",
            limits,
            budget,
            rows.iter(),
            BorrowedWorldAction::new,
        )
    }

    /// Capture original contract bytecode after validating its derived indexes.
    pub(crate) fn capture_contracts_authority_table(
        &self,
        limits: LeafLimits,
        budget: &iroha_allocation::AllocationBudget,
    ) -> core::result::Result<CanonicalTablePairedSnapshot, LeafError> {
        let view = self.view();
        view.validate_world_contract_rows()
            .map_err(LeafError::SourceValidation)?;
        let rows = view.contracts();
        CanonicalTableLeafSet::paired_semantic_table_from_rows(
            "triggers.contracts",
            "iroha:state:trigger-contract-bytecode:v1",
            limits,
            budget,
            rows.iter(),
            BorrowedWorldContract::from,
        )
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
