//! Exact original subject/lifecycle/inverse custody from the frozen StateBlock.
//!
//! Four actual native originals share the existing committed/startup relation.
//! No artifact permission, execution effect, AXT or complete State authority is
//! inferred. TODO: consume every checked output, cell/frontier and authenticated
//! history together in the sole StatePublication and durable Kura owner.

use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::{
    smartcontracts::code::ContractSubjectBinding,
    state::{
        StateBlock, authority_registry::grouped_ownership::validate_original_contract_subjects,
        block_field::AggregatePublication,
    },
};
use iroha_allocation::AllocationBudget;
use iroha_crypto::Hash;
use iroha_data_model::{
    account::{AccountId, AccountValue},
    smart_contract::ContractAddress,
};
use mv::storage::FrozenStorageImages;

struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, ContractAddress, ContractSubjectBinding>,
    reverse: FrozenStorageImages<'frozen, AccountId, ContractAddress>,
    accounts: FrozenStorageImages<'frozen, AccountId, AccountValue>,
    instances: FrozenStorageImages<'frozen, ContractAddress, Hash>,
    budget: &'frozen AllocationBudget,
}
impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.contract_subject_bindings.frozen_images()?;
        let reverse = fields.world.contract_subject_addresses.frozen_images()?;
        let accounts = fields.world.accounts.frozen_images()?;
        let instances = fields.world.contract_instances.frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.contract_subject_bindings);
        let reverse_owned = reverse.belongs_to(&fields.state_ref.world.contract_subject_addresses);
        let accounts_owned = accounts.belongs_to(&fields.state_ref.world.accounts);
        let instances_owned = instances.belongs_to(&fields.state_ref.world.contract_instances);
        if !rows_owned
            || !reverse_owned
            || !accounts_owned
            || !instances_owned
            || rows.mode() != reverse.mode()
            || rows.mode() != accounts.mode()
            || rows.mode() != instances.mode()
        {
            return None;
        }
        Some(Self {
            rows,
            reverse,
            accounts,
            instances,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}

/// Encode original current bindings after both source images and inverse images pass.
///
/// All four original borrows, exact targets/modes and the original State pool
/// survive validation and encoding. Incomplete/foreign/mixed/released sources
/// return None; local refusal preserves the same block for retry. No fresh view,
/// source reconstruction, alternate pool or finalized authority is acquired.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_contract_subjects(
        &original.rows,
        &original.reverse,
        &original.accounts,
        &original.instances,
        max_work,
    )?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.contract_subject_bindings",
        limits,
        original.budget,
        original.rows.current_entries(),
    )
    .map(Some)
}

#[cfg(test)]
#[path = "frozen_contract_subjects/tests.rs"]
mod tests;
