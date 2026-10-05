//! Exact account identity inverses over the original frozen StateBlock.
//!
//! Both original images share the committed identity relation. Implicit accounts
//! remain valid; no AccountId hash, controller validity, alias/metadata or
//! execution predicate is added. TODO: join all scoped encoders, canonical cells,
//! relations and authenticated history in the sole StatePublication owner before
//! claiming complete predecessor coherence, State authority or finality.

use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock,
    authority_registry::account_identity_ownership::validate_original_account_identities,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::{
    account::{AccountId, AccountValue, OpaqueAccountId},
    nexus::UniversalAccountId,
};
use mv::storage::FrozenStorageImages;

struct Original<'frozen> {
    accounts: FrozenStorageImages<'frozen, AccountId, AccountValue>,
    uaids: FrozenStorageImages<'frozen, UniversalAccountId, AccountId>,
    opaques: FrozenStorageImages<'frozen, OpaqueAccountId, UniversalAccountId>,
    budget: &'frozen AllocationBudget,
}
impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let accounts = fields.world.accounts.frozen_images()?;
        let uaids = fields.world.uaid_accounts.frozen_images()?;
        let opaques = fields.world.opaque_uaids.frozen_images()?;
        let accounts_owned = accounts.belongs_to(&fields.state_ref.world.accounts);
        let uaids_owned = uaids.belongs_to(&fields.state_ref.world.uaid_accounts);
        let opaques_owned = opaques.belongs_to(&fields.state_ref.world.opaque_uaids);
        if !accounts_owned
            || !uaids_owned
            || !opaques_owned
            || accounts.mode() != uaids.mode()
            || accounts.mode() != opaques.mode()
        {
            return None;
        }
        Some(Self {
            accounts,
            uaids,
            opaques,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}

/// Encode the original accounts after complete original identity inverse checks.
///
/// `None` refuses incomplete, foreign, released or mixed acquisition sources.
/// All three original borrows and their State pool survive validation and paired
/// encoding. Local refusal leaves the same StateBlock available for retry; no
/// State view, target refresh, caller pool or replacement row source is acquired.
/// Success grants no complete State root, execution or private-row authority.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_account_identities(
        &original.accounts,
        &original.uaids,
        &original.opaques,
        max_work,
    )?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.accounts",
        limits,
        original.budget,
        original.accounts.current_entries(),
    )
    .map(Some)
}

#[cfg(test)]
#[path = "frozen_account_identity/tests.rs"]
mod tests;
