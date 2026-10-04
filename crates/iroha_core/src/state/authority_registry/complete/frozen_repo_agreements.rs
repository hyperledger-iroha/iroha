//! Four actual original frozen repo agreement owners and one exact both-image relation.
//! Scoped rows do not establish instruction validity, complete State or AXT authority.
//! TODO: join all remaining originals/cells/history to sole StatePublication and Kura.
use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock, authority_registry::grouped_ownership::validate_original_repo_agreements,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::{
    account::AccountId,
    repo::{RepoAgreement, RepoAgreementId},
};
use mv::storage::FrozenStorageImages;
use std::collections::BTreeSet;
struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, RepoAgreementId, RepoAgreement>,
    initiators: FrozenStorageImages<'frozen, AccountId, BTreeSet<RepoAgreementId>>,
    counterparties: FrozenStorageImages<'frozen, AccountId, BTreeSet<RepoAgreementId>>,
    custodians: FrozenStorageImages<'frozen, AccountId, BTreeSet<RepoAgreementId>>,
    budget: &'frozen AllocationBudget,
}
impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.repo_agreements.frozen_images()?;
        let initiators = fields.world.repo_agreements_by_initiator.frozen_images()?;
        let counterparties = fields
            .world
            .repo_agreements_by_counterparty
            .frozen_images()?;
        let custodians = fields.world.repo_agreements_by_custodian.frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.repo_agreements);
        let initiators_owned =
            initiators.belongs_to(&fields.state_ref.world.repo_agreements_by_initiator);
        let counterparties_owned =
            counterparties.belongs_to(&fields.state_ref.world.repo_agreements_by_counterparty);
        let custodians_owned =
            custodians.belongs_to(&fields.state_ref.world.repo_agreements_by_custodian);
        if !rows_owned
            || !initiators_owned
            || !counterparties_owned
            || !custodians_owned
            || rows.mode() != initiators.mode()
            || rows.mode() != counterparties.mode()
            || rows.mode() != custodians.mode()
        {
            return None;
        }
        Some(Self {
            rows,
            initiators,
            counterparties,
            custodians,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}
/// Validate both original repo agreement images and encode the actual original current rows.
/// The four targets/modes and original State pool remain borrowed through encoding.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_repo_agreements(
        &original.rows,
        &original.initiators,
        &original.counterparties,
        &original.custodians,
        max_work,
    )?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.repo_agreements",
        limits,
        original.budget,
        original.rows.current_entries(),
    )
    .map(Some)
}
#[cfg(test)]
#[path = "frozen_repo_agreements/tests.rs"]
mod tests;
