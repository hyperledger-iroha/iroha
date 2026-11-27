//! Refusal-preserving keyed transition checks for the new anonymous runtime owner.
//!
//! Governance needs schema validity, not a newly allocated schema digest. Native descriptors
//! come from the immutable compiled catalog, so their already-bound IDs need no re-encoding.
//! The installed admission wrapper and its cache remain unchanged.

use super::*;
use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
use iroha_data_model::runtime::{
    carrier::carrier_definition_id_v1,
    definition::{DefinitionDescriptorV1, SchemaV1, StorageShapeV1},
};
use iroha_data_model::sumeragi_finality::keyed_state::StateKeyV1;

type Attempt = Result<(), ExecutionAttemptError<String>>;

pub(crate) fn validate(
    previous: &RootSelectionV1,
    next: &RootSelectionV1,
    published: &impl DefinitionLookup,
) -> Attempt {
    let catalog = super::super::release_catalog().map_err(|error| error.to_string())?;
    let carrier = catalog
        .generic_carriers
        .ok_or_else(|| "carrier profile has no compiled owner".to_owned())?;
    if next.commitment_profile == carrier.commitment {
        // The carrier forward path performs only borrowed shape/owner checks, no ID or schema encoding.
        return super::carrier_transition::validate(previous, next, published).map_err(Into::into);
    }
    if next
        .installed_storage
        .binary_search(&carrier.backing)
        .is_ok()
        || next.kernel == carrier.kernel
    {
        return Err("carrier backing and kernel require their exact commitment"
            .to_owned()
            .into());
    }
    if previous.commitment_profile == carrier.commitment
        && [profile_id(), catalog.baseline.commitment_profile].contains(&next.commitment_profile)
    {
        return Err(
            "carrier commitment cannot discard virtual tables without migration"
                .to_owned()
                .into(),
        );
    }
    if selected(previous) && next.commitment_profile == catalog.baseline.commitment_profile {
        return Err(
            "cannot restore predecessor commitment without governed migration"
                .to_owned()
                .into(),
        );
    }
    if !selected(next) {
        if next.commitment_profile == catalog.baseline.commitment_profile
            && catalog
                .keyed_storage
                .iter()
                .any(|id| next.installed_storage.binary_search(id).is_ok())
        {
            return Err(
                "World-only commitment cannot install State-level keyed storage"
                    .to_owned()
                    .into(),
            );
        }
        return Ok(());
    }
    let adoption = carrier_definition_id_v1(&adoption_kernel(&catalog.keyed_storage))
        .map_err(|original| canonical_decode_attempt_error(original, |error| error.to_string()))?;
    if !selected(previous)
        && (previous.kernel != adoption
            || previous.commitment_profile != catalog.baseline.commitment_profile
            || next.kernel != adoption
            || next.services_profile != previous.services_profile
            || next.bindings != previous.bindings
            || !previous
                .installed_storage
                .iter()
                .all(|id| next.installed_storage.contains(id)))
    {
        return Err(
            "keyed State adoption requires original storage, kernel and application bindings"
                .to_owned()
                .into(),
        );
    }
    validate_native_schema(next, catalog).map_err(Into::into)
}

fn validate_native_schema(
    selection: &RootSelectionV1,
    catalog: &crate::runtime::ReleaseCatalog,
) -> Result<(), String> {
    if selection.commitment_profile != profile_id()
        || selection
            .installed_storage
            .windows(2)
            .any(|ids| ids[0] >= ids[1])
        || !catalog
            .keyed_storage
            .iter()
            .all(|id| selection.installed_storage.binary_search(id).is_ok())
        || u32::try_from(selection.installed_storage.len()).is_err()
    {
        return Err("keyed profile requires its canonical complete State storage owners".into());
    }
    let codec = |schema: &SchemaV1| {
        matches!(schema, SchemaV1::Codec(value)
        if !value.is_empty() && u32::try_from(value.len()).is_ok())
    };
    for (index, id) in selection.installed_storage.iter().enumerate() {
        let Some(DefinitionDescriptorV1::Storage(storage)) = catalog.definitions.get(id) else {
            return Err("native keyed storage has no exact compiled owner".into());
        };
        if !StateKeyV1::valid_table_id(&storage.namespace)
            || u32::try_from(storage.namespace.len()).is_err()
            || !codec(&storage.value)
            || matches!(&storage.shape, StorageShapeV1::Table { key } if !codec(key))
        {
            return Err("native keyed storage has no canonical native codec".into());
        }
        if selection.installed_storage[..index].iter().any(|before| {
            matches!(catalog.definitions.get(before), Some(DefinitionDescriptorV1::Storage(prior)) if prior.namespace == storage.namespace)
        }) { return Err("native keyed storage namespaces overlap".into()); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_native_schema_has_the_installed_acceptance_rules_without_a_digest_allocation() {
        let catalog = crate::runtime::release_catalog().unwrap();
        let mut selected = catalog.baseline.clone();
        selected.commitment_profile = profile_id();
        selected
            .installed_storage
            .extend_from_slice(&catalog.keyed_storage);
        selected.installed_storage.sort_unstable();
        selected.installed_storage.dedup();
        for remove in [false, true] {
            let mut candidate = selected.clone();
            if remove {
                candidate.installed_storage.remove(0);
            }
            assert_eq!(
                validate_native_schema(&candidate, catalog).is_ok(),
                crate::state::keyed_state::schema_for(&candidate).is_ok()
            );
        }
    }
}
