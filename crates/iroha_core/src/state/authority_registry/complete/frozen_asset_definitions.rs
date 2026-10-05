//! Seven actual frozen definition/reference/group/policy owners and one relation.
//!
//! This encodes a scoped original table, not complete State or private settlement.
//! TODO: join all other relations/cells/history to sole StatePublication and Kura.

use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock, authority_registry::grouped_ownership::validate_original_asset_definitions,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinition, AssetDefinitionId},
    domain::Domain,
};
use iroha_model_base::domain::DomainId;
use mv::storage::FrozenStorageImages;
use std::collections::BTreeSet;

struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, AssetDefinitionId, AssetDefinition>,
    domains: FrozenStorageImages<'frozen, DomainId, Domain>,
    contexts: FrozenStorageImages<'frozen, AssetDefinitionId, DomainId>,
    by_domain: FrozenStorageImages<'frozen, DomainId, BTreeSet<AssetDefinitionId>>,
    by_owner: FrozenStorageImages<'frozen, AccountId, BTreeSet<AssetDefinitionId>>,
    transitions: FrozenStorageImages<'frozen, (u64, AssetDefinitionId), ()>,
    counts: FrozenStorageImages<'frozen, u64, u32>,
    budget: &'frozen AllocationBudget,
}
impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.asset_definitions.frozen_images()?;
        let domains = fields.world.domains.frozen_images()?;
        let contexts = fields.world.asset_definition_domains.frozen_images()?;
        let by_domain = fields.world.domain_asset_definitions.frozen_images()?;
        let by_owner = fields.world.asset_definitions_by_owner.frozen_images()?;
        let transitions = fields
            .world
            .confidential_policy_transition_index
            .frozen_images()?;
        let counts = fields
            .world
            .confidential_policy_transition_counts
            .frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.asset_definitions);
        let domains_owned = domains.belongs_to(&fields.state_ref.world.domains);
        let contexts_owned = contexts.belongs_to(&fields.state_ref.world.asset_definition_domains);
        let by_domain_owned =
            by_domain.belongs_to(&fields.state_ref.world.domain_asset_definitions);
        let by_owner_owned =
            by_owner.belongs_to(&fields.state_ref.world.asset_definitions_by_owner);
        let transitions_owned =
            transitions.belongs_to(&fields.state_ref.world.confidential_policy_transition_index);
        let counts_owned =
            counts.belongs_to(&fields.state_ref.world.confidential_policy_transition_counts);
        if !rows_owned
            || !domains_owned
            || !contexts_owned
            || !by_domain_owned
            || !by_owner_owned
            || !transitions_owned
            || !counts_owned
            || rows.mode() != domains.mode()
            || rows.mode() != contexts.mode()
            || rows.mode() != by_domain.mode()
            || rows.mode() != by_owner.mode()
            || rows.mode() != transitions.mode()
            || rows.mode() != counts.mode()
        {
            return None;
        }
        Some(Self {
            rows,
            domains,
            contexts,
            by_domain,
            by_owner,
            transitions,
            counts,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}

/// Encode original definitions after both images of all seven relations pass.
/// Exact source targets/modes and original State pool remain borrowed through
/// encoding. Incomplete/foreign/mixed/released originals refuse without refresh;
/// every local error leaves the caller's same immutable block available for retry.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_asset_definitions(
        &original.rows,
        &original.domains,
        &original.contexts,
        &original.by_domain,
        &original.by_owner,
        &original.transitions,
        &original.counts,
        max_work,
    )?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.asset_definitions",
        limits,
        original.budget,
        original.rows.current_entries(),
    )
    .map(Some)
}

#[cfg(test)]
#[path = "frozen_asset_definitions/tests.rs"]
mod tests;
