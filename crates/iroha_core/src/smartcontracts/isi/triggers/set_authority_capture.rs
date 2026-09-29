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
        budget: &mv::allocation::AllocationBudget,
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
        budget: &mv::allocation::AllocationBudget,
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
        budget: &mv::allocation::AllocationBudget,
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
        budget: &mv::allocation::AllocationBudget,
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
        budget: &mv::allocation::AllocationBudget,
    ) -> core::result::Result<CanonicalTablePairedSnapshot, LeafError> {
        let view = self.view();
        view.validate_world_contract_rows()
            .map_err(LeafError::Encoding)?;
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
