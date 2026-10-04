//! Four actual original frozen account-rekey owners and one exact both-image relation.
//! Scoped rows do not establish complete State, instruction validity or AXT authority.
//! TODO: join all remaining original relations/cells/history to StatePublication and Kura.
use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock, authority_registry::grouped_ownership::validate_original_account_rekeys,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::account::{AccountAlias, AccountId, AccountRekeyRecord, AccountValue};
use mv::storage::FrozenStorageImages;
use std::collections::BTreeSet;
struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, AccountAlias, AccountRekeyRecord>,
    accounts: FrozenStorageImages<'frozen, AccountId, AccountValue>,
    aliases: FrozenStorageImages<'frozen, AccountAlias, AccountId>,
    occurrences: FrozenStorageImages<'frozen, AccountId, BTreeSet<AccountAlias>>,
    budget: &'frozen AllocationBudget,
}
impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.account_rekey_records.frozen_images()?;
        let accounts = fields.world.accounts.frozen_images()?;
        let aliases = fields.world.account_aliases.frozen_images()?;
        let occurrences = fields
            .world
            .account_rekey_records_by_account
            .frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.account_rekey_records);
        let accounts_owned = accounts.belongs_to(&fields.state_ref.world.accounts);
        let aliases_owned = aliases.belongs_to(&fields.state_ref.world.account_aliases);
        let occurrences_owned =
            occurrences.belongs_to(&fields.state_ref.world.account_rekey_records_by_account);
        let rows_mode = rows.mode();
        let accounts_mode = accounts.mode();
        let aliases_mode = aliases.mode();
        let occurrences_mode = occurrences.mode();
        if !rows_owned
            || !accounts_owned
            || !aliases_owned
            || !occurrences_owned
            || rows_mode != accounts_mode
            || rows_mode != aliases_mode
            || rows_mode != occurrences_mode
        {
            return None;
        }
        Some(Self {
            rows,
            accounts,
            aliases,
            occurrences,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}
/// Check both original images and encode actual current rows using the original State pool.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_account_rekeys(
        &original.rows,
        &original.accounts,
        &original.aliases,
        &original.occurrences,
        max_work,
    )?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.account_rekey_records",
        limits,
        original.budget,
        original.rows.current_entries(),
    )
    .map(Some)
}
#[cfg(test)]
#[path = "frozen_account_rekeys/tests.rs"]
mod tests;
