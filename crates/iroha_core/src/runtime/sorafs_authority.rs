//! Selected Parliament proposal hosting and nondelegable SoraFS authority services.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    NetworkId,
    runtime::{
        definition::{
            BindingV1, DefinitionDescriptorV1, DefinitionId, DefinitionLookup, EntrypointV1,
            ImplementationCodeV1, ImplementationDescriptorV1, RestrictionEntryV1, RestrictionV1,
            RootSelectionV1, ServiceEntryV1,
        },
        lifecycle::{ChangeSetV1, RestrictionKindV1, RootScopeV1, SelectionTransitionV1},
    },
};

use super::catalog::{
    AdditionDefinitions, CatalogError, ReleaseCatalog, additions::SORAFS_AUTHORITY,
};

pub(crate) const SERVICE: &str = "sorafs_authority_governance";
pub(crate) const SERVICE_CLOSURE: &[u8] =
    b"iroha:sorafs:authority:paired-terms-64-epochs:cutoff-1-epoch:17-units-per-role:deadline-closure:controller-bound-http:signed-operating-effects:v1";

fn failure(reason: impl Into<String>) -> CatalogError {
    CatalogError::Addition {
        module: SORAFS_AUTHORITY.module.into(),
        reason: reason.into(),
    }
}

fn restriction_kind(restriction: RestrictionV1) -> RestrictionKindV1 {
    match restriction {
        RestrictionV1::Deprecated(_) => RestrictionKindV1::Deprecated,
        RestrictionV1::AdmissionClosed(_) => RestrictionKindV1::AdmissionClosed,
        RestrictionV1::CompletionOnly(_) => RestrictionKindV1::CompletionOnly,
        RestrictionV1::Disabled(_) => RestrictionKindV1::Disabled,
    }
}

fn replacement(
    mut implementation: ImplementationDescriptorV1,
    profile: DefinitionId,
    storage: &[DefinitionId],
) -> Result<DefinitionDescriptorV1, CatalogError> {
    let ImplementationCodeV1::Native {
        reference_closure, ..
    } = &mut implementation.code
    else {
        return Err(failure(
            "authority services require a compiled native predecessor",
        ));
    };
    *reference_closure =
        iroha_crypto::Hash::new_from_chunks(&[SERVICE_CLOSURE, reference_closure]).into();
    implementation.services_profile = profile;
    for access in [
        &mut implementation.reads,
        &mut implementation.writes,
        &mut implementation.scans,
    ] {
        access.extend_from_slice(storage);
        access.sort_unstable();
        access.dedup();
    }
    Ok(DefinitionDescriptorV1::Implementation(implementation))
}

fn representatives(catalog: &ReleaseCatalog) -> Result<Vec<RootSelectionV1>, CatalogError> {
    Ok(vec![catalog.baseline.clone()])
}

fn selected_storage(
    catalog: &ReleaseCatalog,
    selection: &RootSelectionV1,
) -> Result<BTreeMap<String, DefinitionId>, CatalogError> {
    let mut namespaces = BTreeMap::new();
    for id in &selection.installed_storage {
        let Some(DefinitionDescriptorV1::Storage(storage)) = catalog.definitions.get(id) else {
            return Err(failure(
                "selected authority dependency is not compiled storage",
            ));
        };
        if namespaces.insert(storage.namespace.clone(), *id).is_some() {
            return Err(failure("ambiguous selected authority storage namespace"));
        }
    }
    Ok(namespaces)
}

fn selected_interface(
    selection: &RootSelectionV1,
    wire: &str,
) -> Result<DefinitionId, CatalogError> {
    selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire)
        .map(|binding| binding.interface)
        .ok_or_else(|| {
            failure(format!(
                "selected authority dependency has no interface for {wire}"
            ))
        })
}

// Only these compiled predecessor compositions authorize remapping. A matching
// namespace or service name in an arbitrary descriptor is never a fallback.
fn representative(
    catalog: &ReleaseCatalog,
    current: &RootSelectionV1,
) -> Result<RootSelectionV1, CatalogError> {
    let mut matches = representatives(catalog)?
        .into_iter()
        .filter(|candidate| candidate.services_profile == current.services_profile);
    let selected = matches
        .next()
        .ok_or(CatalogError::UnsupportedServices(current.services_profile))?;
    if matches.next().is_some() {
        return Err(failure("ambiguous compiled authority predecessor"));
    }
    let namespaces = selected_storage(catalog, current)?;
    for (namespace, id) in selected_storage(catalog, &selected)? {
        if namespaces.get(&namespace) != Some(&id) {
            return Err(failure(
                "authority predecessor storage differs from its compiled composition",
            ));
        }
    }
    for binding in &selected.bindings {
        if selected_interface(current, &binding.wire_id)? != binding.interface {
            return Err(failure(
                "authority predecessor interface differs from its compiled composition",
            ));
        }
    }
    Ok(selected)
}

struct Composition {
    interfaces: BTreeMap<&'static str, DefinitionDescriptorV1>,
    implementation: DefinitionDescriptorV1,
}

fn specialize(
    catalog: &ReleaseCatalog,
    addition: &AdditionDefinitions,
    selected: &RootSelectionV1,
    profile: DefinitionId,
) -> Result<Composition, CatalogError> {
    let namespaces = selected_storage(catalog, selected)?;
    let mut storage_map = BTreeMap::new();
    for (namespace, previous) in selected_storage(catalog, &catalog.baseline)? {
        let next = namespaces
            .get(&namespace)
            .ok_or_else(|| failure("compiled authority predecessor omits baseline storage"))?;
        storage_map.insert(previous, *next);
    }
    for id in addition.storage.values() {
        storage_map.insert(*id, *id);
    }
    let mut interface_map = BTreeMap::new();
    for binding in &catalog.baseline.bindings {
        interface_map.insert(
            binding.interface,
            selected_interface(selected, &binding.wire_id)?,
        );
    }
    let baseline_binding = catalog
        .baseline
        .bindings
        .first()
        .ok_or_else(|| failure("baseline authority dependency bundle missing"))?;
    let selected_binding = selected
        .bindings
        .iter()
        .find(|binding| binding.wire_id == baseline_binding.wire_id)
        .ok_or_else(|| failure("selected authority dependency bundle missing"))?;
    let bundle_events = |id| match catalog.definitions.get(&id) {
        Some(DefinitionDescriptorV1::Implementation(bundle)) if bundle.events.len() == 1 => {
            Ok(bundle.events[0])
        }
        _ => Err(failure(
            "authority composition requires the exact compiled shared event",
        )),
    };
    let event_map = BTreeMap::from([(
        bundle_events(baseline_binding.implementation)?,
        bundle_events(selected_binding.implementation)?,
    )]);
    let remap = |ids: &mut Vec<DefinitionId>,
                 mapping: &BTreeMap<DefinitionId, DefinitionId>|
     -> Result<(), CatalogError> {
        for id in ids.iter_mut() {
            *id = *mapping
                .get(id)
                .ok_or_else(|| failure("unmapped authority composition dependency"))?;
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(())
    };
    let mut interfaces = BTreeMap::new();
    for (wire, previous) in &addition.interfaces {
        let Some(DefinitionDescriptorV1::Interface(mut interface)) =
            catalog.definitions.get(previous).cloned()
        else {
            return Err(failure("authority interface missing"));
        };
        remap(&mut interface.events, &event_map)?;
        let descriptor = DefinitionDescriptorV1::Interface(interface);
        interface_map.insert(*previous, descriptor.id()?);
        interfaces.insert(*wire, descriptor);
    }
    let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
        catalog.definitions.get(&addition.implementation).cloned()
    else {
        return Err(failure("authority implementation missing"));
    };
    for access in [
        &mut implementation.reads,
        &mut implementation.writes,
        &mut implementation.scans,
    ] {
        remap(access, &storage_map)?;
    }
    remap(&mut implementation.invokes, &interface_map)?;
    remap(&mut implementation.events, &event_map)?;
    let EntrypointV1::Instructions(entrypoints) = &mut implementation.entrypoint else {
        return Err(failure(
            "authority implementation has no instruction entrypoints",
        ));
    };
    remap(entrypoints, &interface_map)?;
    let storage: Vec<_> = addition.storage.values().copied().collect();
    Ok(Composition {
        interfaces,
        implementation: replacement(implementation, profile, &storage)?,
    })
}

/// Compile exact service compositions and native implementations before any State can select them.
pub(super) fn compile(catalog: &mut ReleaseCatalog) -> Result<(), CatalogError> {
    let Some(addition) = catalog
        .additions
        .iter()
        .find(|addition| addition.module == SORAFS_AUTHORITY.module)
        .cloned()
    else {
        return Ok(());
    };
    let storage: Vec<_> = addition.storage.values().copied().collect();
    let definitions: Vec<_> = catalog.definitions.values().cloned().collect();
    for selected in representatives(catalog)? {
        let predecessor = selected.services_profile;
        let Some(DefinitionDescriptorV1::ServicesProfile(mut services)) =
            catalog.definitions.get(&predecessor).cloned()
        else {
            return Err(CatalogError::UnsupportedServices(predecessor));
        };
        services.services.push(ServiceEntryV1 {
            name: SERVICE.into(),
            reference_closure: iroha_crypto::Hash::new(SERVICE_CLOSURE).into(),
        });
        services.services.sort_by(|a, b| a.name.cmp(&b.name));
        let profile = catalog.publish(DefinitionDescriptorV1::ServicesProfile(services))?;
        let composition = specialize(catalog, &addition, &selected, profile)?;
        for interface in composition.interfaces.into_values() {
            catalog.publish(interface)?;
        }
        catalog.publish(composition.implementation)?;
        for descriptor in &definitions {
            let DefinitionDescriptorV1::Implementation(implementation) = descriptor else {
                continue;
            };
            // The addition is composed with each supported predecessor services profile.
            // Existing implementations retain their own exact predecessor dependency.
            if implementation.services_profile == predecessor
                && !matches!(&implementation.code, ImplementationCodeV1::Native { module, .. } if module == SORAFS_AUTHORITY.module)
            {
                catalog.publish(replacement(implementation.clone(), profile, &storage)?)?;
            }
        }
    }
    Ok(())
}

/// Draft one atomic services, storage and instruction transition, without unrelated additions.
pub(super) fn draft(
    catalog: &ReleaseCatalog,
    current: &RootSelectionV1,
    published: &impl DefinitionLookup,
    network: NetworkId,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, CatalogError> {
    current.check_closure(&catalog.definitions)?;
    if let Some(id) = catalog.unsupported(current) {
        return Err(CatalogError::UnsupportedServices(id));
    }
    let selected = representative(catalog, current)?;
    let addition = catalog
        .additions
        .iter()
        .find(|addition| addition.module == SORAFS_AUTHORITY.module)
        .ok_or_else(|| failure("authority addition is not compiled"))?;
    let Some(DefinitionDescriptorV1::ServicesProfile(mut services)) =
        catalog.definitions.get(&current.services_profile).cloned()
    else {
        return Err(CatalogError::UnsupportedServices(current.services_profile));
    };
    if services.services.iter().any(|entry| entry.name == SERVICE) {
        return Err(failure("authority services already selected"));
    }
    services.services.push(ServiceEntryV1 {
        name: SERVICE.into(),
        reference_closure: iroha_crypto::Hash::new(SERVICE_CLOSURE).into(),
    });
    services.services.sort_by(|a, b| a.name.cmp(&b.name));
    let profile = DefinitionDescriptorV1::ServicesProfile(services).id()?;
    if !catalog.definitions.contains_key(&profile) {
        return Err(CatalogError::UnsupportedServices(profile));
    }
    let storage: Vec<_> = addition.storage.values().copied().collect();
    let mut replacements = BTreeMap::new();
    for id in &current.active {
        let Some(DefinitionDescriptorV1::Implementation(implementation)) =
            catalog.definitions.get(id)
        else {
            continue;
        };
        let next = replacement(implementation.clone(), profile, &storage)?.id()?;
        if !catalog.definitions.contains_key(&next) {
            return Err(CatalogError::UnsupportedServices(next));
        }
        replacements.insert(*id, next);
    }
    let composition = specialize(catalog, addition, &selected, profile)?;
    let interfaces = composition
        .interfaces
        .into_iter()
        .map(|(wire, descriptor)| Ok((wire, descriptor.id()?)))
        .collect::<Result<BTreeMap<_, _>, CatalogError>>()?;
    let implementation = composition.implementation.id()?;
    if !catalog.definitions.contains_key(&implementation)
        || interfaces
            .values()
            .any(|id| !catalog.definitions.contains_key(id))
    {
        return Err(failure("authority composition is not compiled"));
    }
    let mut transitions: Vec<_> = storage
        .iter()
        .copied()
        .map(SelectionTransitionV1::InstallStorage)
        .collect();
    transitions.extend(
        interfaces
            .values()
            .copied()
            .chain(replacements.values().copied())
            .chain([implementation])
            .map(SelectionTransitionV1::Activate),
    );
    let mut bindings: BTreeMap<_, _> = current
        .bindings
        .iter()
        .cloned()
        .map(|mut binding| {
            if let Some(next) = replacements.get(&binding.implementation) {
                binding.implementation = *next;
            }
            (binding.wire_id.clone(), binding)
        })
        .collect();
    let rebinds = addition
        .rebinds
        .keys()
        .map(|wire| Ok((*wire, selected_interface(current, wire)?)))
        .collect::<Result<BTreeMap<_, _>, CatalogError>>()?;
    // Rebound entrypoints retain their owner's admission restriction. Shared
    // ownership applies the strongest inherited restriction to the new owner.
    let rebound_restriction = rebinds
        .keys()
        .filter_map(|wire| {
            let binding = current
                .bindings
                .iter()
                .find(|binding| binding.wire_id == *wire)?;
            current
                .restrictions
                .iter()
                .find(|entry| entry.definition == binding.implementation)
                .map(|entry| restriction_kind(entry.restriction))
        })
        .max();
    for (wire, interface) in interfaces.iter().chain(&rebinds) {
        bindings.insert(
            (*wire).into(),
            BindingV1 {
                wire_id: (*wire).into(),
                interface: *interface,
                implementation,
            },
        );
    }
    transitions.extend(bindings.values().cloned().map(SelectionTransitionV1::Bind));
    if let Some(restriction) = rebound_restriction {
        transitions.push(SelectionTransitionV1::Restrict {
            definition: implementation,
            restriction,
        });
    }
    for (previous, next) in &replacements {
        if let Some(entry) = current
            .restrictions
            .iter()
            .find(|entry| entry.definition == *previous)
        {
            transitions.push(SelectionTransitionV1::Restrict {
                definition: *next,
                restriction: restriction_kind(entry.restriction),
            });
        }
        transitions.push(SelectionTransitionV1::Restrict {
            definition: *previous,
            restriction: RestrictionKindV1::Disabled,
        });
        transitions.push(SelectionTransitionV1::Deactivate(*previous));
    }
    transitions.push(SelectionTransitionV1::SetServicesProfile(profile));
    let needed: BTreeSet<_> = storage
        .iter()
        .copied()
        .chain(interfaces.values().copied())
        .chain(replacements.values().copied())
        .chain([implementation, profile])
        .collect();
    let publish = needed
        .into_iter()
        .filter(|id| published.definition(id).is_none())
        .map(|id| {
            catalog
                .definitions
                .get(&id)
                .cloned()
                .ok_or(CatalogError::UnsupportedServices(id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut successor = current.clone();
    successor.installed_storage.extend(storage);
    successor.installed_storage.sort_unstable();
    successor.installed_storage.dedup();
    for id in &mut successor.active {
        if let Some(next) = replacements.get(id) {
            *id = *next;
        }
    }
    successor
        .active
        .extend(interfaces.values().copied().chain([implementation]));
    successor.active.sort_unstable();
    successor.active.dedup();
    for entry in &mut successor.restrictions {
        if let Some(next) = replacements.get(&entry.definition) {
            entry.definition = *next;
            entry.restriction = restriction_kind(entry.restriction).at(effective_height);
        }
    }
    if let Some(restriction) = rebound_restriction {
        successor.restrictions.push(RestrictionEntryV1 {
            definition: implementation,
            restriction: restriction.at(effective_height),
        });
    }
    successor.restrictions.sort_by_key(|entry| entry.definition);
    successor.bindings = bindings.into_values().collect();
    successor.services_profile = profile;
    successor.check_closure(&catalog.definitions)?;
    let change = ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: current.id()?,
        successor: successor.id()?,
        effective_height,
        publish,
        transitions,
        qualification,
    };
    change
        .apply(current, published)
        .map_err(|error| failure(error.to_string()))?;
    Ok(change)
}

#[cfg(test)]
mod tests;
