//! Exact new-profile cutover rules, without widening native commitment resolution.

use super::*;
use iroha_data_model::runtime::definition::{
    CarrierNodeV1, CarrierSchemaV1, DefinitionId, StorageDescriptorV1,
};
use std::collections::BTreeMap;

fn fixture() -> (
    RootSelectionV1,
    RootSelectionV1,
    BTreeMap<DefinitionId, DefinitionDescriptorV1>,
) {
    let catalog = crate::runtime::release_catalog().unwrap();
    let profile = catalog.generic_carriers.unwrap();
    let mut previous = catalog.baseline.clone();
    previous.kernel = adoption_kernel(&catalog.keyed_storage).id().unwrap();
    previous.commitment_profile = profile_id();
    previous
        .installed_storage
        .extend(catalog.keyed_storage.iter().copied());
    previous.installed_storage.sort_unstable();
    let mut next = previous.clone();
    next.kernel = profile.kernel;
    next.commitment_profile = profile.commitment;
    next.active.push(profile.host);
    next.active.sort_unstable();
    next.installed_storage.extend([
        profile.backing,
        catalog.world_storage["world.runtime_generic_artifacts"],
    ]);
    next.installed_storage.sort_unstable();
    let mut published = catalog.definitions.clone();
    let schema = CarrierSchemaV1 {
        nodes: vec![CarrierNodeV1::U16],
    };
    let table = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
        namespace: "app.carrier_transition".into(),
        shape: StorageShapeV1::Table {
            key: SchemaV1::Carrier(schema.clone()),
        },
        value: SchemaV1::Carrier(schema),
        initial: None,
        invariants: [17; 32],
        projection: ProjectionV1::BareNorito,
    });
    let id = table.id().unwrap();
    published.insert(id, table);
    next.installed_storage.push(id);
    next.installed_storage.sort_unstable();
    (previous, next, published)
}

#[test]
fn exact_carrier_cutover_uses_authenticated_definitions_and_keeps_native_resolver_frozen() {
    let (previous, next, published) = fixture();
    assert!(crate::runtime::keyed_state::schema_digest(&previous).is_ok());
    assert!(crate::runtime::keyed_state::schema_digest(&next).is_err());
    validate(&previous, &next, &published).unwrap();
    // No schema construction/serialization or replacement local budget is needed in governance.
    let limits = norito::core::DecodeLimits::new(0, 0, 0, 0, 0);
    norito::core::with_decode_limits(limits, || Ok(validate(&previous, &next, &published)))
        .unwrap()
        .unwrap();
    validate(&next, &next, &published).unwrap();
}

#[test]
fn carrier_cutover_refuses_incomplete_predecessor_dropped_native_fields_and_owner_aliases() {
    let (previous, next, published) = fixture();
    let catalog = crate::runtime::release_catalog().unwrap();
    assert!(validate(&catalog.baseline, &next, &published).is_err());
    let mut incomplete = previous.clone();
    incomplete
        .installed_storage
        .retain(|id| id != &catalog.keyed_storage[0]);
    assert!(validate(&incomplete, &next, &published).is_err());
    let mut dropped = next.clone();
    dropped
        .installed_storage
        .retain(|id| id != &catalog.baseline.installed_storage[0]);
    assert!(validate(&previous, &dropped, &published).is_err());
    let mut missing = published.clone();
    missing.remove(&catalog.generic_carriers.unwrap().backing);
    assert!(validate(&previous, &next, &missing).is_err());
    assert!(validate(&next, &previous, &published).is_err());
    assert!(validate(&next, &catalog.baseline, &published).is_err());
    let mut alias = published;
    let virtual_id = next
        .installed_storage
        .iter()
        .find(|id| !catalog.definitions.contains_key(id))
        .copied()
        .unwrap();
    let DefinitionDescriptorV1::Storage(table) = alias.get_mut(&virtual_id).unwrap() else {
        panic!("fixture virtual table");
    };
    table.namespace = "world.accounts".into();
    assert!(validate(&previous, &next, &alias).is_err());
}

#[test]
fn backing_installation_requires_its_exact_profile_but_artifact_staging_is_independent() {
    let (keyed, carrier, published) = fixture();
    let catalog = crate::runtime::release_catalog().unwrap();
    let profile = catalog.generic_carriers.unwrap();
    let artifacts = catalog.world_storage["world.runtime_generic_artifacts"];
    for previous in [&catalog.baseline, &keyed] {
        let mut staging = previous.clone();
        staging.installed_storage.push(artifacts);
        staging.installed_storage.sort_unstable();
        staging.check_closure(&published).unwrap();
        crate::runtime::keyed_state::validate_transition_with_definitions(
            previous, &staging, &published,
        )
        .unwrap();

        let mut unsupported = previous.clone();
        unsupported.installed_storage.push(profile.backing);
        unsupported.installed_storage.sort_unstable();
        unsupported.check_closure(&published).unwrap();
        assert!(
            crate::runtime::keyed_state::validate_transition_with_definitions(
                previous,
                &unsupported,
                &published,
            )
            .unwrap_err()
            .contains("installed carrier backing requires its exact kernel and commitment")
        );
    }
    for (kernel, commitment) in [
        (keyed.kernel, profile.commitment),
        (profile.kernel, keyed.commitment_profile),
    ] {
        let mut partial = carrier.clone();
        partial.kernel = kernel;
        partial.commitment_profile = commitment;
        assert!(
            crate::runtime::keyed_state::validate_transition_with_definitions(
                &keyed, &partial, &published,
            )
            .is_err()
        );
    }
    crate::runtime::keyed_state::validate_transition_with_definitions(&keyed, &carrier, &published)
        .unwrap();
}
