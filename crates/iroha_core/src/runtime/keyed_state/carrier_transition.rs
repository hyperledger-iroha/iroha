//! Exact carrier commitment cutover shape over authenticated published definitions.
//!
//! Governance checks ownership without constructing a schema or encoding identities. The
//! change-set model already authenticates publication identities. Actual State execution
//! resolves the complete funded schema and captures its original current/predecessor cut.

use super::*;
use iroha_data_model::runtime::definition::{
    DefinitionDescriptorV1, ProjectionV1, SchemaV1, StorageShapeV1,
};

pub(super) fn validate(
    previous: &RootSelectionV1,
    next: &RootSelectionV1,
    published: &impl DefinitionLookup,
) -> Result<(), String> {
    let catalog = crate::runtime::release_catalog().map_err(|error| error.to_string())?;
    let profile = catalog
        .generic_carriers
        .ok_or("carrier commitment has no compiled owner")?;
    // The physical wrapper is excluded from the carrier commitment's virtual leaves. Installing
    // it under another profile would produce State that the exact snapshot owner cannot restore.
    // Artifact staging has a separate native owner and does not require this backing table.
    if next
        .installed_storage
        .binary_search(&profile.backing)
        .is_ok()
        && (next.kernel != profile.kernel || next.commitment_profile != profile.commitment)
    {
        return Err("installed carrier backing requires its exact kernel and commitment".into());
    }
    if next.commitment_profile != profile.commitment {
        if previous.commitment_profile == profile.commitment
            && (next.commitment_profile == profile_id()
                || next.commitment_profile == catalog.baseline.commitment_profile)
        {
            return Err("carrier commitment cannot discard its virtual tables without an explicit migration".into());
        }
        if next.kernel == profile.kernel {
            return Err("carrier kernel requires its exact carrier commitment".into());
        }
        return super::validate_transition(previous, next);
    }
    if ![profile_id(), profile.commitment].contains(&previous.commitment_profile) {
        return Err(
            "carrier adoption requires an original complete native keyed predecessor".into(),
        );
    }
    if next.kernel != profile.kernel || next.active.binary_search(&profile.host).is_err() {
        return Err("carrier commitment requires its exact selected kernel and active host".into());
    }
    let artifacts = *catalog
        .world_storage
        .get("world.runtime_generic_artifacts")
        .ok_or("carrier artifact source is missing")?;
    for id in [
        profile.kernel,
        profile.host,
        profile.commitment,
        profile.backing,
        artifacts,
        next.services_profile,
    ] {
        if published.definition(&id).is_none()
            || published.definition(&id) != catalog.definitions.get(&id)
        {
            return Err("carrier cutover substitutes a compiled profile or backing owner".into());
        }
    }
    if [profile.backing, artifacts]
        .iter()
        .any(|id| next.installed_storage.binary_search(id).is_err())
        || catalog.keyed_storage.iter().any(|id| {
            previous.installed_storage.binary_search(id).is_err()
                || next.installed_storage.binary_search(id).is_err()
        })
    {
        return Err(
            "carrier cutover requires physical owners and every original State-level owner".into(),
        );
    }
    // Opening the new representation cannot silently drop an original native field. Later
    // carrier changes retain the ordinary migration/retirement checks of their exact owners.
    if previous.commitment_profile == profile_id()
        && previous
            .installed_storage
            .iter()
            .any(|id| next.installed_storage.binary_search(id).is_err())
    {
        return Err("carrier adoption must retain every predecessor storage owner".into());
    }
    iroha_data_model::sumeragi_finality::SelectedStateSchemaV1::carrier_selection_allocation_bytes(
        next, published,
    )
    .map_err(|error| error.to_string())?;
    for (index, id) in next.installed_storage.iter().enumerate() {
        let Some(definition @ DefinitionDescriptorV1::Storage(storage)) = published.definition(id)
        else {
            return Err("carrier cutover storage is missing".into());
        };
        for before in &next.installed_storage[..index] {
            if matches!(published.definition(before), Some(DefinitionDescriptorV1::Storage(prior)) if prior.namespace == storage.namespace)
            {
                return Err("carrier cutover namespaces overlap".into());
            }
        }
        if matches!(storage.value, SchemaV1::Carrier(_)) {
            if !matches!(storage.shape, StorageShapeV1::Table { key: SchemaV1::Carrier(_) }) || storage.initial.is_some() || storage.projection != ProjectionV1::BareNorito
                || catalog.world_storage.contains_key(storage.namespace.as_str())
                || catalog.keyed_storage.iter().any(|native| matches!(catalog.definitions.get(native), Some(DefinitionDescriptorV1::Storage(native)) if native.namespace == storage.namespace)) {
                return Err("carrier cutover virtual table has no distinct bare carrier owner".into());
            }
        } else if catalog.definitions.get(id) != Some(definition)
            || !(catalog.world_storage.values().any(|native| native == id)
                || catalog.keyed_storage.contains(id))
        {
            return Err("carrier cutover native field has no exact compiled owner".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
