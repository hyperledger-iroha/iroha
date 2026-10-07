//! Restore asset lookup indexes from the same two authoritative MV images.
//!
//! Domain context comes from the definition in the selected image. Both images
//! are checked before replacing any index; the canonical sources are never
//! mutated while reconstructing their derived replacement history.
//! TODO: admit these restore allocations through the original snapshot resource
//! owner along with the remaining infallible derived-index reconstruction.

use super::*;
use mv::storage::History;

#[derive(Default)]
struct Image {
    domains: BTreeMap<AssetDefinitionId, DomainId>,
    domain_definitions: BTreeMap<DomainId, BTreeSet<AssetDefinitionId>>,
    owners: BTreeMap<AccountId, BTreeSet<AssetDefinitionId>>,
    holders: BTreeMap<AssetDefinitionId, BTreeSet<AccountId>>,
    definition_assets: BTreeMap<AssetDefinitionId, BTreeSet<AssetId>>,
    account_assets: BTreeMap<AccountId, BTreeSet<AssetId>>,
    domain_assets: BTreeMap<DomainId, BTreeSet<AssetId>>,
    nonzero_holders: BTreeMap<AssetDefinitionId, BTreeSet<AccountId>>,
}

fn project<'a>(
    definitions: impl Iterator<Item = (&'a AssetDefinitionId, &'a AssetDefinition)>,
    assets: impl Iterator<Item = (&'a AssetId, &'a AssetValue)>,
    domain_exists: impl Fn(&DomainId) -> bool,
    definition_exists: impl Fn(&AssetDefinitionId) -> bool,
    registry: Option<&iroha_data_model::asset::AssetDefinitionDataspaceRegistryV1>,
    incarnation: impl Fn(&AssetDefinitionId) -> Option<AxtAssetIncarnationV1>,
) -> Result<Image, String> {
    let mut image = Image::default();
    for (id, definition) in definitions {
        let domain = definition.owning_domain().as_ref();
        let incarnation = incarnation(id);
        let direct = asset_definition_dataspace_from_registry(
            registry,
            id,
            Some(definition),
            incarnation.as_ref(),
        )
        .map_err(|error| error.to_string())?;
        iroha_data_model::asset::AssetDefinitionHome::from_definition(definition, direct)
            .map_err(|error| error.to_string())?;
        if let Some(domain) = domain {
            if !domain_exists(domain) {
                return Err(format!(
                    "asset definition {id} references missing owning domain {domain}"
                ));
            }
            image.domains.insert(id.clone(), domain.clone());
            image
                .domain_definitions
                .entry(domain.clone())
                .or_default()
                .insert(id.clone());
        }
        image
            .owners
            .entry(definition.owned_by().clone())
            .or_default()
            .insert(id.clone());
    }
    for (id, value) in assets {
        if !definition_exists(id.definition()) {
            return Err(format!(
                "asset {id} references missing asset definition {}",
                id.definition()
            ));
        }
        image
            .holders
            .entry(id.definition().clone())
            .or_default()
            .insert(id.account().clone());
        image
            .definition_assets
            .entry(id.definition().clone())
            .or_default()
            .insert(id.clone());
        image
            .account_assets
            .entry(id.account().clone())
            .or_default()
            .insert(id.clone());
        if let Some(domain) = image.domains.get(id.definition()) {
            image
                .domain_assets
                .entry(domain.clone())
                .or_default()
                .insert(id.clone());
        }
        if !value.as_ref().is_zero() {
            image
                .nonzero_holders
                .entry(id.definition().clone())
                .or_default()
                .insert(id.account().clone());
        }
    }
    Ok(image)
}

#[derive(Default)]
struct Touches {
    definitions: BTreeSet<AssetDefinitionId>,
    domains: BTreeSet<DomainId>,
    owners: BTreeSet<AccountId>,
    asset_definitions: BTreeSet<AssetDefinitionId>,
    accounts: BTreeSet<AccountId>,
    asset_domains: BTreeSet<DomainId>,
}

impl Touches {
    fn sources(
        definitions: &History<'_, AssetDefinitionId, AssetDefinition>,
        assets: &History<'_, AssetId, AssetValue>,
    ) -> Self {
        let mut touched = Self::default();
        for (id, prior) in definitions.revert_map().iter() {
            touched.definitions.insert(id.clone());
            for definition in [prior.as_ref(), definitions.current().get(id)]
                .into_iter()
                .flatten()
            {
                touched.owners.insert(definition.owned_by().clone());
                if let Some(domain) = definition.owning_domain() {
                    touched.domains.insert(domain.clone());
                    // Untouched balances follow the definition's domain in each
                    // image; a definition change alone can move this bucket.
                    touched.asset_domains.insert(domain.clone());
                }
            }
        }
        for (id, _) in assets.revert_map().iter() {
            touched.asset_definitions.insert(id.definition().clone());
            touched.accounts.insert(id.account().clone());
            for definition in [
                definitions.current().get(id.definition()),
                definitions.get_before_block(id.definition()),
            ]
            .into_iter()
            .flatten()
            {
                if let Some(domain) = definition.owning_domain() {
                    touched.asset_domains.insert(domain.clone());
                }
            }
        }
        touched
    }
}

/// Keep changed buckets and redundant source touches, with complete old members.
fn restore<K: mv::Key, V: mv::Value + PartialEq>(
    current: BTreeMap<K, V>,
    mut previous: BTreeMap<K, V>,
    touched: &BTreeSet<K>,
) -> Storage<K, V> {
    let mut keys = touched.clone();
    for key in current.keys().chain(previous.keys()) {
        if current.get(key) != previous.get(key) {
            keys.insert(key.clone());
        }
    }
    let undo = keys
        .into_iter()
        .map(|key| {
            let prior = previous.remove(&key);
            (key, prior)
        })
        .collect();
    Storage::from_snapshot_parts(current, undo)
}

/// Check both source images before replacing all eight derived lookup indexes.
pub(super) fn assets(world: &mut World) -> Result<(), String> {
    // Borrow the backing once so all three histories retain disjoint source fields.
    let world: &mut WorldData = world;
    let definitions = world.asset_definitions.history();
    let domains = world.domains.history();
    let balances = world.assets.history();
    let incarnations = world.axt_asset_incarnations.history();
    let parameters = world
        .parameters
        .try_committed_view()
        .map_err(|error| format!("cannot retain direct-home parameters: {error:?}"))?;
    let current_registry = asset_definition_registry_from_parameters(parameters.current())
        .map_err(|error| error.to_string())?;
    let previous_registry = asset_definition_registry_from_parameters(
        parameters.undo().as_ref().unwrap_or(parameters.current()),
    )
    .map_err(|error| error.to_string())?;
    validate_asset_definition_registry_transition(
        previous_registry.as_ref(),
        current_registry.as_ref(),
    )
    .map_err(|error| error.to_string())?;
    if let Some(next) = &current_registry {
        for (id, binding) in &next.bindings {
            if binding.active
                && definitions.get_before_block(id).is_some()
                && incarnations.get_before_block(id) == Some(&binding.incarnation)
                && previous_registry
                    .as_ref()
                    .and_then(|registry| registry.bindings.get(id))
                    != Some(binding)
            {
                return Err(
                    "direct home changed for an existing snapshot-predecessor incarnation".into(),
                );
            }
        }
    }
    // Check every binding, including orphans and tombstones absent from the definition iterator.
    for (previous, registry) in [
        (false, current_registry.as_ref()),
        (true, previous_registry.as_ref()),
    ] {
        if let Some(registry) = registry {
            for id in registry.bindings.keys() {
                let definition = if previous {
                    definitions.get_before_block(id)
                } else {
                    definitions.current().get(id)
                };
                let incarnation = if previous {
                    incarnations.get_before_block(id)
                } else {
                    incarnations.current().get(id)
                };
                asset_definition_dataspace_from_registry(
                    Some(registry),
                    id,
                    definition,
                    incarnation,
                )
                .map_err(|error| error.to_string())?;
            }
        }
    }
    let current = project(
        definitions.current().iter(),
        balances.current().iter(),
        |id| domains.current().get(id).is_some(),
        |id| definitions.current().get(id).is_some(),
        current_registry.as_ref(),
        |id| incarnations.current().get(id).copied(),
    )?;
    let previous = project(
        definitions.iter_before_block(),
        balances.iter_before_block(),
        |id| domains.get_before_block(id).is_some(),
        |id| definitions.get_before_block(id).is_some(),
        previous_registry.as_ref(),
        |id| incarnations.get_before_block(id).copied(),
    )?;
    let touched = Touches::sources(&definitions, &balances);
    drop((definitions, domains, balances));
    world.asset_definition_domains =
        restore(current.domains, previous.domains, &touched.definitions);
    world.domain_asset_definitions = restore(
        current.domain_definitions,
        previous.domain_definitions,
        &touched.domains,
    );
    world.asset_definitions_by_owner = restore(current.owners, previous.owners, &touched.owners);
    world.asset_definition_holders = restore(
        current.holders,
        previous.holders,
        &touched.asset_definitions,
    );
    world.asset_definition_assets = restore(
        current.definition_assets,
        previous.definition_assets,
        &touched.asset_definitions,
    );
    world.assets_by_account = restore(
        current.account_assets,
        previous.account_assets,
        &touched.accounts,
    );
    world.assets_by_domain = restore(
        current.domain_assets,
        previous.domain_assets,
        &touched.asset_domains,
    );
    world.asset_definition_nonzero_holders = restore(
        current.nonzero_holders,
        previous.nonzero_holders,
        &touched.asset_definitions,
    );
    Ok(())
}

#[cfg(test)]
mod tests;
