//! Catalog activation publishes an exact host profile only beside installed kernel lifecycle State.

use super::*;

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0x63; 32]),
    ))
}
fn selected_definitions(
    catalog: &ReleaseCatalog,
    selection: &RootSelectionV1,
) -> BTreeMap<DefinitionId, DefinitionDescriptorV1> {
    selection
        .active
        .iter()
        .chain(&selection.installed_storage)
        .copied()
        .chain([
            selection.kernel,
            selection.commitment_profile,
            selection.services_profile,
        ])
        .map(|id| (id, catalog.definitions[&id].clone()))
        .collect()
}
fn select(catalog: &ReleaseCatalog, original: &RootSelectionV1, module: &str) -> RootSelectionV1 {
    catalog
        .activation(
            module,
            original,
            &catalog.definitions,
            network(),
            40,
            [1; 32],
        )
        .unwrap()
        .apply(original, &catalog.definitions)
        .unwrap()
        .selection
}

#[test]
fn contract_lifecycle_publication_leaves_the_frozen_baseline_and_abi_unchanged() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let wire =
        iroha_data_model::isi::runtime_obligation::RegisterContractRuntimeObligationV1::WIRE_ID;
    assert!(
        !catalog
            .baseline
            .bindings
            .iter()
            .any(|binding| binding.wire_id == wire)
    );
    let hosts: Vec<_> = catalog
        .baseline
        .active
        .iter()
        .filter_map(|id| match &catalog.definitions[id] {
            DefinitionDescriptorV1::HostProfile(host) => Some(host),
            _ => None,
        })
        .collect();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].abi_version, 1);
    assert_eq!(
        hosts[0].abi_hash,
        ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)
    );
    let composition = &catalog.contract_lifecycle_compositions[&catalog.baseline.services_profile];
    assert!(!catalog.baseline.active.contains(&composition.host));
    assert!(
        !catalog
            .baseline
            .installed_storage
            .contains(&composition.registry)
    );
    assert_ne!(
        hosts[0].abi_hash,
        ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1RuntimeLifecycle)
    );
    let error = catalog
        .activation(
            additions::CONTRACT_LIFECYCLE.module,
            &catalog.baseline,
            &selected_definitions(catalog, &catalog.baseline),
            network(),
            40,
            [1; 32],
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("activate kernel lifecycle storage"),
        "{error}"
    );
}

#[test]
fn contract_lifecycle_activates_exact_profile_after_kernel_storage_on_every_compiled_family() {
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut families = vec![catalog.baseline.clone(), catalog.lifecycle.clone()];
    if let Some(ironwood) = &catalog.ironwood {
        families.push(ironwood.selection.clone());
        families.push(catalog.lifecycle_successor(&ironwood.selection).unwrap());
    }
    let original_families = families.clone();
    for family in original_families {
        let escrow = select(catalog, &family, additions::NATIVE_ESCROW_CAPTURE.module);
        let consent = select(catalog, &escrow, additions::DEFERRED_CONSENT.module);
        for prior in [family, escrow, consent] {
            families.push(select(
                catalog,
                &prior,
                additions::NATIVE_AMX_CAPTURE.module,
            ));
        }
    }
    let wire =
        iroha_data_model::isi::runtime_obligation::RegisterContractRuntimeObligationV1::WIRE_ID;
    for family in families {
        let original = select(catalog, &family, additions::RUNTIME_LIFECYCLE.module);
        let published = selected_definitions(catalog, &original);
        let change = catalog
            .activation(
                additions::CONTRACT_LIFECYCLE.module,
                &original,
                &published,
                network(),
                40,
                [1; 32],
            )
            .unwrap();
        let result = change.apply(&original, &published).unwrap();
        let selected = result.selection;
        assert_eq!(selected.services_profile, original.services_profile);
        assert_eq!(selected.kernel, original.kernel);
        assert_eq!(selected.commitment_profile, original.commitment_profile);
        assert_eq!(selected.installed_storage, original.installed_storage);
        for original_binding in &original.bindings {
            assert_eq!(
                selected
                    .bindings
                    .iter()
                    .find(|binding| binding.wire_id == original_binding.wire_id),
                Some(original_binding)
            );
        }
        let composed = &catalog.contract_lifecycle_compositions[&original.services_profile];
        let binding = selected
            .bindings
            .iter()
            .find(|binding| binding.wire_id == wire)
            .unwrap();
        assert_eq!(binding.implementation, composed.addition.implementation);
        let DefinitionDescriptorV1::Implementation(implementation) =
            &catalog.definitions[&binding.implementation]
        else {
            panic!("implementation");
        };
        assert_eq!(implementation.host_profile, Some(composed.host));
        for declared in [
            &implementation.reads,
            &implementation.writes,
            &implementation.scans,
        ] {
            assert!(declared.contains(&composed.registry));
        }
        let DefinitionDescriptorV1::HostProfile(host) = &catalog.definitions[&composed.host] else {
            panic!("host profile");
        };
        assert_eq!(host.abi_version, 1);
        assert_eq!(
            host.abi_hash,
            ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1RuntimeLifecycle)
        );
        let expected_metering: [u8; 32] = ivm::gas::schedule_hash().into();
        assert_eq!(host.metering, expected_metering);
        assert!(selected.active.contains(&composed.host));
        assert_eq!(catalog.unsupported(&selected), None);
        assert!(
            change
                .publish
                .iter()
                .all(|descriptor| !published.contains_key(&descriptor.id().unwrap()))
        );
        assert!(
            catalog
                .activation(
                    additions::CONTRACT_LIFECYCLE.module,
                    &selected,
                    &catalog.definitions,
                    network(),
                    41,
                    [1; 32]
                )
                .is_err()
        );
    }
}
