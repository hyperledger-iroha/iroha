//! Governed successor for owner-issued restricted data space reads; no implicit read access.

use super::catalog::{
    AdditionDefinitions, CatalogError, ReleaseCatalog, additions::RESTRICTED_DATASPACE_READ,
};
use iroha_data_model::{
    isi::InstructionBox,
    runtime::{
        definition::{DefinitionDescriptorV1, EntrypointV1, RestrictionV1, RootSelectionV1},
        lifecycle::RestrictionKindV1,
    },
};

fn failure(reason: &str) -> CatalogError {
    CatalogError::Addition {
        module: RESTRICTED_DATASPACE_READ.module.into(),
        reason: reason.into(),
    }
}

/// Build a distinct native implementation over a known predecessor bundle. The entire declared
/// storage, event, permission and service closure remains that predecessor's, and every existing
/// interface keeps its identity. There is no schema-based or arbitrary-profile remapping.
fn composition(
    catalog: &ReleaseCatalog,
    selection: &RootSelectionV1,
) -> Result<(AdditionDefinitions, DefinitionDescriptorV1), CatalogError> {
    let base = catalog
        .additions
        .iter()
        .find(|addition| addition.module == RESTRICTED_DATASPACE_READ.module)
        .ok_or_else(|| failure("restricted-read successor is not compiled"))?;
    let mut addition = base.clone();
    let mut predecessor = None;
    for (wire, interface) in &mut addition.rebinds {
        let binding = selection
            .bindings
            .iter()
            .find(|binding| binding.wire_id == *wire)
            .ok_or_else(|| failure("restricted-read predecessor interface is missing"))?;
        if predecessor.is_some_and(|id| id != binding.implementation) {
            return Err(failure(
                "restricted-read predecessor must use one compiled permission bundle",
            ));
        }
        predecessor = Some(binding.implementation);
        *interface = binding.interface;
    }
    let predecessor = predecessor.ok_or_else(|| failure("restricted-read interfaces are empty"))?;
    if catalog.native_module(&predecessor) == Some(RESTRICTED_DATASPACE_READ.module) {
        return Err(failure("restricted-read successor is already selected"));
    }
    let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
        catalog.definitions.get(&predecessor).cloned()
    else {
        return Err(failure(
            "restricted-read predecessor is not a compiled implementation",
        ));
    };
    let Some(DefinitionDescriptorV1::Implementation(base_implementation)) =
        catalog.definitions.get(&base.implementation)
    else {
        return Err(failure("restricted-read implementation is missing"));
    };
    implementation.code = base_implementation.code.clone();
    let mut interfaces = addition.rebinds.values().copied().collect::<Vec<_>>();
    interfaces.sort_unstable();
    implementation.entrypoint = EntrypointV1::Instructions(interfaces);
    let descriptor = DefinitionDescriptorV1::Implementation(implementation);
    addition.implementation = descriptor.id()?;
    Ok((addition, descriptor))
}

/// Compile the exact baseline, lifecycle, Ironwood and Ironwood/lifecycle predecessors supported
/// by this release. Publishing these descriptors does not activate any of them.
pub(super) fn compile(catalog: &mut ReleaseCatalog) -> Result<(), CatalogError> {
    if !catalog
        .additions
        .iter()
        .any(|addition| addition.module == RESTRICTED_DATASPACE_READ.module)
    {
        return Ok(());
    }
    let mut selections = vec![catalog.baseline.clone(), catalog.lifecycle.clone()];
    if let Some(ironwood) = &catalog.ironwood {
        let selection = ironwood.selection.clone();
        selections.push(catalog.lifecycle_successor(&selection)?);
        selections.push(selection);
    }
    for selection in selections {
        let (addition, descriptor) = composition(catalog, &selection)?;
        catalog.publish(descriptor)?;
        let predecessor = selection
            .bindings
            .iter()
            .find(|binding| binding.wire_id == "iroha.grant")
            .ok_or_else(|| failure("compiled grant binding is missing"))?
            .implementation;
        catalog
            .restricted_read_compositions
            .insert(predecessor, addition);
    }
    Ok(())
}

/// Select only a composition already compiled into this release; unknown predecessors fail closed.
pub(super) fn for_selection(
    catalog: &ReleaseCatalog,
    selection: &RootSelectionV1,
) -> Result<AdditionDefinitions, CatalogError> {
    let predecessor = selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == "iroha.grant")
        .ok_or_else(|| failure("restricted-read predecessor grant binding is missing"))?
        .implementation;
    let addition = catalog
        .restricted_read_compositions
        .get(&predecessor)
        .ok_or_else(|| failure("restricted-read predecessor composition is not compiled"))?;
    for (wire, interface) in &addition.rebinds {
        if !selection.bindings.iter().any(|binding| {
            binding.wire_id == *wire
                && binding.interface == *interface
                && binding.implementation == predecessor
        }) {
            return Err(failure(
                "restricted-read predecessor bindings do not match the compiled composition",
            ));
        }
    }
    Ok(addition.clone())
}

/// Carry the predecessor bundle's containment onto its replacement. A future restriction cannot
/// be silently moved or removed by this activation; its owner must choose a compatible height.
pub(super) fn inherited_restriction(
    selection: &RootSelectionV1,
    effective_height: u64,
) -> Result<Option<RestrictionKindV1>, CatalogError> {
    let implementation = selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == "iroha.grant")
        .ok_or_else(|| failure("restricted-read predecessor grant binding is missing"))?
        .implementation;
    let Some(entry) = selection
        .restrictions
        .iter()
        .find(|entry| entry.definition == implementation)
    else {
        return Ok(None);
    };
    let (kind, height) = match entry.restriction {
        RestrictionV1::Deprecated(height) => (RestrictionKindV1::Deprecated, height),
        RestrictionV1::AdmissionClosed(height) => (RestrictionKindV1::AdmissionClosed, height),
        RestrictionV1::CompletionOnly(height) => (RestrictionKindV1::CompletionOnly, height),
        RestrictionV1::Disabled(height) => (RestrictionKindV1::Disabled, height),
    };
    if height > effective_height {
        return Err(failure(
            "restricted-read activation precedes an inherited restriction",
        ));
    }
    Ok(Some(kind))
}

/// Whether this instruction selects the owner-delegation successor. Merely publishing or
/// activating its implementation never changes an instruction still bound to its predecessor.
pub(crate) fn selected_for(selection: &RootSelectionV1, instruction: &InstructionBox) -> bool {
    let Some(wire) = iroha_data_model::isi::instruction_wire_id(instruction) else {
        return false;
    };
    let Ok(catalog) = super::release_catalog() else {
        return false;
    };
    selection.bindings.iter().any(|binding| {
        binding.wire_id == wire
            && catalog.native_module(&binding.implementation)
                == Some(super::catalog::additions::RESTRICTED_DATASPACE_READ.module)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{NetworkId, runtime::definition::RestrictionEntryV1};

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([7; 32])))
    }

    #[test]
    fn restricted_read_catalog_preserves_each_compiled_predecessor_and_only_rebinds_permission_paths()
     {
        let mut catalog = super::super::release_catalog().unwrap().clone();
        let mut predecessors = vec![catalog.baseline.clone(), catalog.lifecycle.clone()];
        if let Some(ironwood) = &catalog.ironwood {
            let selection = ironwood.selection.clone();
            predecessors.push(catalog.lifecycle_successor(&selection).unwrap());
            predecessors.push(selection);
        }
        for before in predecessors {
            let change = catalog
                .activation(
                    RESTRICTED_DATASPACE_READ.module,
                    &before,
                    &catalog.definitions,
                    network(),
                    50,
                    [9; 32],
                )
                .unwrap();
            let after = change
                .apply(&before, &catalog.definitions)
                .unwrap()
                .selection;
            assert_eq!(after.installed_storage, before.installed_storage);
            assert_eq!(after.services_profile, before.services_profile);
            assert_eq!(after.kernel, before.kernel);
            assert_eq!(after.commitment_profile, before.commitment_profile);
            assert_eq!(catalog.unsupported(&after), None);
            for binding in &before.bindings {
                let next = after
                    .bindings
                    .iter()
                    .find(|entry| entry.wire_id == binding.wire_id)
                    .unwrap();
                assert_eq!(binding.interface, next.interface);
                if RESTRICTED_DATASPACE_READ
                    .rebinds
                    .contains(&binding.wire_id.as_str())
                {
                    assert_ne!(binding.implementation, next.implementation);
                    assert_eq!(
                        catalog.native_module(&next.implementation),
                        Some(RESTRICTED_DATASPACE_READ.module)
                    );
                } else {
                    assert_eq!(binding, next);
                }
            }
            assert!(
                catalog
                    .activation(
                        RESTRICTED_DATASPACE_READ.module,
                        &after,
                        &catalog.definitions,
                        network(),
                        60,
                        [9; 32]
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn restricted_read_catalog_preserves_containment_and_refuses_uncompiled_compositions() {
        let mut catalog = super::super::release_catalog().unwrap().clone();
        let mut before = catalog.baseline.clone();
        let source = before
            .bindings
            .iter()
            .find(|binding| binding.wire_id == "iroha.grant")
            .unwrap()
            .implementation;
        before.restrictions.push(RestrictionEntryV1 {
            definition: source,
            restriction: RestrictionV1::AdmissionClosed(20),
        });
        let change = catalog
            .activation(
                RESTRICTED_DATASPACE_READ.module,
                &before,
                &catalog.definitions,
                network(),
                50,
                [9; 32],
            )
            .unwrap();
        let after = change
            .apply(&before, &catalog.definitions)
            .unwrap()
            .selection;
        let successor = after
            .bindings
            .iter()
            .find(|binding| binding.wire_id == "iroha.grant")
            .unwrap()
            .implementation;
        assert!(after.restrictions.contains(&RestrictionEntryV1 {
            definition: successor,
            restriction: RestrictionV1::AdmissionClosed(50)
        }));
        before.restrictions[0].restriction = RestrictionV1::AdmissionClosed(60);
        assert!(
            catalog
                .activation(
                    RESTRICTED_DATASPACE_READ.module,
                    &before,
                    &catalog.definitions,
                    network(),
                    50,
                    [9; 32]
                )
                .is_err()
        );
        before.restrictions.clear();
        let DefinitionDescriptorV1::Implementation(mut foreign) =
            catalog.definitions[&source].clone()
        else {
            panic!("native bundle")
        };
        // The old composition operation replaced code and entrypoint, so changing only code
        // must still fail the explicit predecessor inventory even if its replacement would match.
        foreign.code = iroha_data_model::runtime::definition::ImplementationCodeV1::Native {
            module: "iroha.unqualified.permission_bundle".into(),
            reference_closure: [77; 32],
        };
        let descriptor = DefinitionDescriptorV1::Implementation(foreign);
        let foreign_id = descriptor.id().unwrap();
        catalog.definitions.insert(foreign_id, descriptor);
        for binding in &mut before.bindings {
            if binding.implementation == source {
                binding.implementation = foreign_id;
            }
        }
        assert!(
            for_selection(&catalog, &before).is_err(),
            "even a superficially matching native bundle needs an exact compiled composition"
        );
    }

    #[test]
    fn restricted_read_catalog_retains_all_containment_kinds_and_interface_restrictions() {
        let catalog = super::super::release_catalog().unwrap();
        for kind in [
            RestrictionKindV1::Deprecated,
            RestrictionKindV1::AdmissionClosed,
            RestrictionKindV1::CompletionOnly,
            RestrictionKindV1::Disabled,
        ] {
            let mut before = catalog.baseline.clone();
            let binding = before
                .bindings
                .iter()
                .find(|binding| binding.wire_id == "iroha.grant")
                .unwrap()
                .clone();
            let interface_restriction = RestrictionEntryV1 {
                definition: binding.interface,
                restriction: RestrictionV1::AdmissionClosed(18),
            };
            before.restrictions = vec![
                interface_restriction.clone(),
                RestrictionEntryV1 {
                    definition: binding.implementation,
                    restriction: kind.at(20),
                },
            ];
            before.restrictions.sort_by_key(|entry| entry.definition);
            let change = catalog
                .activation(
                    RESTRICTED_DATASPACE_READ.module,
                    &before,
                    &catalog.definitions,
                    network(),
                    50,
                    [9; 32],
                )
                .unwrap();
            let after = change
                .apply(&before, &catalog.definitions)
                .unwrap()
                .selection;
            let successor = after
                .bindings
                .iter()
                .find(|entry| entry.wire_id == "iroha.grant")
                .unwrap()
                .implementation;
            assert!(after.restrictions.contains(&interface_restriction));
            assert!(after.restrictions.contains(&RestrictionEntryV1 {
                definition: successor,
                restriction: kind.at(50)
            }));
        }
    }
}
