//! Eight actual frozen balance/reference/index owners and one original relation.
//! Scoped rows do not supply complete State or private settlement authority.
//! TODO: join all remaining owners/cells/history to StatePublication and Kura.
use super::{CanonicalTableLeafSet, CanonicalTablePairedSnapshot, LeafError, LeafLimits};
use crate::state::{
    StateBlock, authority_registry::grouped_ownership::validate_original_assets,
    block_field::AggregatePublication,
};
use iroha_allocation::AllocationBudget;
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinition, AssetDefinitionId, AssetId, AssetValue},
    domain::Domain,
};
use iroha_model_base::domain::DomainId;
use mv::storage::FrozenStorageImages;
use std::collections::BTreeSet;
struct Original<'frozen> {
    rows: FrozenStorageImages<'frozen, AssetId, AssetValue>,
    definitions: FrozenStorageImages<'frozen, AssetDefinitionId, AssetDefinition>,
    domains: FrozenStorageImages<'frozen, DomainId, Domain>,
    by_definition: FrozenStorageImages<'frozen, AssetDefinitionId, BTreeSet<AssetId>>,
    by_account: FrozenStorageImages<'frozen, AccountId, BTreeSet<AssetId>>,
    by_domain: FrozenStorageImages<'frozen, DomainId, BTreeSet<AssetId>>,
    holders: FrozenStorageImages<'frozen, AssetDefinitionId, BTreeSet<AccountId>>,
    nonzero: FrozenStorageImages<'frozen, AssetDefinitionId, BTreeSet<AccountId>>,
    budget: &'frozen AllocationBudget,
}
impl<'frozen> Original<'frozen> {
    fn retain(block: &'frozen StateBlock<'_>) -> Option<Self> {
        let fields = block.fields.as_ref()?;
        if fields.world.publication != AggregatePublication::Frozen {
            return None;
        }
        let rows = fields.world.assets.frozen_images()?;
        let definitions = fields.world.asset_definitions.frozen_images()?;
        let domains = fields.world.domains.frozen_images()?;
        let by_definition = fields.world.asset_definition_assets.frozen_images()?;
        let by_account = fields.world.assets_by_account.frozen_images()?;
        let by_domain = fields.world.assets_by_domain.frozen_images()?;
        let holders = fields.world.asset_definition_holders.frozen_images()?;
        let nonzero = fields
            .world
            .asset_definition_nonzero_holders
            .frozen_images()?;
        let rows_owned = rows.belongs_to(&fields.state_ref.world.assets);
        let definitions_owned = definitions.belongs_to(&fields.state_ref.world.asset_definitions);
        let domains_owned = domains.belongs_to(&fields.state_ref.world.domains);
        let by_definition_owned =
            by_definition.belongs_to(&fields.state_ref.world.asset_definition_assets);
        let by_account_owned = by_account.belongs_to(&fields.state_ref.world.assets_by_account);
        let by_domain_owned = by_domain.belongs_to(&fields.state_ref.world.assets_by_domain);
        let holders_owned = holders.belongs_to(&fields.state_ref.world.asset_definition_holders);
        let nonzero_owned =
            nonzero.belongs_to(&fields.state_ref.world.asset_definition_nonzero_holders);
        if !rows_owned
            || !definitions_owned
            || !domains_owned
            || !by_definition_owned
            || !by_account_owned
            || !by_domain_owned
            || !holders_owned
            || !nonzero_owned
            || rows.mode() != definitions.mode()
            || rows.mode() != domains.mode()
            || rows.mode() != by_definition.mode()
            || rows.mode() != by_account.mode()
            || rows.mode() != by_domain.mode()
            || rows.mode() != holders.mode()
            || rows.mode() != nonzero.mode()
        {
            return None;
        }
        Some(Self {
            rows,
            definitions,
            domains,
            by_definition,
            by_account,
            by_domain,
            holders,
            nonzero,
            budget: &fields.state_ref.ivm_execution_budget,
        })
    }
}
/// Validate both original balance images and encode the caller's actual assets.
/// All eight targets/modes and the original State pool stay borrowed through encoding.
pub(in crate::state) fn capture(
    block: &StateBlock<'_>,
    limits: LeafLimits,
    max_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let Some(original) = Original::retain(block) else {
        return Ok(None);
    };
    validate_original_assets(
        &original.rows,
        &original.definitions,
        &original.domains,
        &original.by_definition,
        &original.by_account,
        &original.by_domain,
        &original.holders,
        &original.nonzero,
        max_work,
    )?;
    CanonicalTableLeafSet::paired_table_from_rows(
        "world.assets",
        limits,
        original.budget,
        original.rows.current_entries(),
    )
    .map(Some)
}
#[cfg(test)]
#[path = "frozen_assets/tests.rs"]
mod tests;
