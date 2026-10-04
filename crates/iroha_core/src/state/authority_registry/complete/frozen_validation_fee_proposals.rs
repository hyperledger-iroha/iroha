//! Exact fee-proposal projection over the original frozen State execution owner.
//!
//! The committed and frozen captures share one bounded both-image relation. Every
//! fee kind and status is included; proposal admission, operator identity, exact
//! JSON, Parliament execution/history and policy correctness have separate owners.
//! TODO: consume this scoped encoder with every canonical table/cell, original
//! relation and authenticated history in the sole StatePublication owner. Whole
//! predecessor coherence, node custody and finality remain open.

use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    GovernanceProposalRecord, StateBlock,
    authority_registry::grouped_ownership::validate_original_validation_fee_proposals,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use mv::storage::FrozenStorageImages;

// Only one actual execution owner can supply the rows, index and encoding pool.
struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, [u8; 32], GovernanceProposalRecord>,
    index: FrozenStorageImages<'frozen, (u64, [u8; 32]), ()>,
    budget: &'frozen AllocationBudget,
}

impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.governance_proposals.frozen_images()?;
        let index = fields.world.validation_fee_proposal_index.frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.governance_proposals);
        let index_owned = index.belongs_to(&fields.state_ref.world.validation_fee_proposal_index);
        if !rows_owned || !index_owned || rows.mode() != index.mode() {
            return None;
        }
        Some(Self {
            rows,
            index,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}

/// Encode the original canonical proposal rows whose exact fee lookup was checked.
///
/// `None` refuses incomplete, foreign, differently acquired or released sources.
/// Both original borrows and their State allocation pool survive validation and
/// paired encoding; every local refusal leaves the same StateBlock available for
/// retry. No State view, publication reacquisition or replacement pool is opened.
/// Success grants no complete State root, finality or private-row disclosure.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_validation_fee_proposals(&original.rows, &original.index, max_work)?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.governance_proposals",
        limits,
        original.budget,
        original.rows.current_entries(),
    )
    .map(Some)
}

#[cfg(test)]
#[path = "frozen_validation_fee_proposals/tests.rs"]
mod tests;
