//! Exact contract-alias inverse and leases over the original frozen StateBlock.
//!
//! The committed and frozen owners share one bounded both-image relation. Undeployed and
//! expired bindings remain representable; deployment, current authority, time-based resolution
//! and cleanup are separate owners. No literal re-decode or fresh State view is needed.
//! TODO: consume this scoped encoder with every canonical table/cell and authenticated history
//! in the sole StatePublication owner. Whole predecessor coherence, custody and finality remain open.

use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    ContractAliasBindingRecord, StateBlock,
    authority_registry::grouped_ownership::validate_original_contract_aliases,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::smart_contract::{ContractAddress, ContractAlias};
use mv::storage::FrozenStorageImages;

// Only one actual execution owner can supply the rows, index and encoding pool.
struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, ContractAddress, ContractAliasBindingRecord>,
    index: FrozenStorageImages<'frozen, ContractAlias, ContractAddress>,
    budget: &'frozen AllocationBudget,
}

impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.contract_alias_bindings.frozen_images()?;
        let index = fields.world.contract_aliases.frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.contract_alias_bindings);
        let index_owned = index.belongs_to(&fields.state_ref.world.contract_aliases);
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

/// Encode the original canonical binding rows whose exact inverse and leases were checked.
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
    validate_original_contract_aliases(&original.rows, &original.index, max_work)?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.contract_alias_bindings",
        limits,
        original.budget,
        original.rows.current_entries(),
    )
    .map(Some)
}

#[cfg(test)]
#[path = "frozen_contract_aliases/tests.rs"]
mod tests;
