//! Exact alias sources and reverse buckets on the actual frozen StateBlock.
//!
//! Both images use the sole committed alias relation and original State pool.
//! TODO: join every scoped table/cell/history and original publication owner in
//! State/Kura before granting complete State, scope, authorization or finality.

use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock, authority_registry::account_alias_ownership::validate_original_account_aliases,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::account::{AccountAlias, AccountId, AccountValue};
use mv::storage::FrozenStorageImages;
use std::collections::BTreeSet;

struct Original<'frozen> {
    accounts: FrozenStorageImages<'frozen, AccountId, AccountValue>,
    aliases: FrozenStorageImages<'frozen, AccountAlias, AccountId>,
    reverse: FrozenStorageImages<'frozen, AccountId, BTreeSet<AccountAlias>>,
    budget: &'frozen AllocationBudget,
}
impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let accounts = fields.world.accounts.frozen_images()?;
        let aliases = fields.world.account_aliases.frozen_images()?;
        let reverse = fields.world.account_aliases_by_account.frozen_images()?;
        let accounts_owned = accounts.belongs_to(&fields.state_ref.world.accounts);
        let aliases_owned = aliases.belongs_to(&fields.state_ref.world.account_aliases);
        let reverse_owned = reverse.belongs_to(&fields.state_ref.world.account_aliases_by_account);
        if !accounts_owned
            || !aliases_owned
            || !reverse_owned
            || accounts.mode() != aliases.mode()
            || accounts.mode() != reverse.mode()
        {
            return None;
        }
        Some(Self {
            accounts,
            aliases,
            reverse,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}

/// Encode original aliases after both complete original source/inverse passes.
///
/// Incomplete, foreign, released or mixed modes return None. Local work/encoding
/// refusal preserves the same frozen block and pool for retry; no fresh view,
/// target refresh, supplied source or caller pool can replace its originals.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_account_aliases(
        &original.accounts,
        &original.aliases,
        &original.reverse,
        max_work,
    )?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.account_aliases",
        limits,
        original.budget,
        original.aliases.current_entries(),
    )
    .map(Some)
}

#[cfg(test)]
#[path = "frozen_account_aliases/tests.rs"]
mod tests;
