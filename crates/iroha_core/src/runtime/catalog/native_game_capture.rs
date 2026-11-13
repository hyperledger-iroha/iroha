//! Immutable service compositions for certified native game-transfer completion custody.

use super::*;
use iroha_data_model::runtime::native_game::{SERVICE, SERVICE_CLOSURE};

#[derive(Clone, Debug)]
pub(super) struct Composition {
    pub(super) services: DefinitionId,
    pub(super) replacements: BTreeMap<DefinitionId, DefinitionId>,
    pub(super) addition: AdditionDefinitions,
    prerequisites: Vec<AdditionDefinitions>,
}

fn failure(reason: impl Into<String>) -> CatalogError {
    CatalogError::Addition {
        module: additions::NATIVE_GAME_CAPTURE.module.into(),
        reason: reason.into(),
    }
}

impl ReleaseCatalog {
    /// Compile every currently supported native service family before publishing migrations.
    pub(super) fn compile_native_game_capture(&mut self) -> Result<(), CatalogError> {
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == additions::NATIVE_GAME_CAPTURE.module)
            .cloned()
        else {
            return Ok(());
        };
        let prerequisites: Vec<_> = self
            .additions
            .iter()
            .take_while(|addition| addition.module != additions::GAME_PARTITIONS.module)
            .chain(
                self.additions
                    .iter()
                    .filter(|addition| addition.module == additions::GAME_PARTITIONS.module),
            )
            .cloned()
            .collect();
        let original = self.definitions.clone();
        let storage = base.storage["world.runtime_game_transfers"];
        for (profile, descriptor) in &original {
            let DefinitionDescriptorV1::ServicesProfile(mut services) = descriptor.clone() else {
                continue;
            };
            if services.services.iter().any(|entry| entry.name == SERVICE) {
                continue;
            }
            let Some(bundle) = original.values().find_map(|descriptor| {
                let DefinitionDescriptorV1::Implementation(implementation) = descriptor else {
                    return None;
                };
                (implementation.services_profile == *profile
                    && matches!(&implementation.code, ImplementationCodeV1::Native { module, .. }
                        if matches!(module.as_str(), BASELINE_MODULE | LIFECYCLE_MODULE | "iroha.ironwood.v1")))
                    .then(|| implementation.clone())
            }) else {
                continue;
            };
            services.services.push(ServiceEntryV1 {
                name: SERVICE.into(),
                reference_closure: iroha_crypto::Hash::new(SERVICE_CLOSURE).into(),
            });
            services.services.sort_by(|a, b| a.name.cmp(&b.name));
            let services = self.publish(DefinitionDescriptorV1::ServicesProfile(services))?;
            let mut replacements = BTreeMap::new();
            for (id, descriptor) in &original {
                let DefinitionDescriptorV1::Implementation(mut implementation) = descriptor.clone()
                else {
                    continue;
                };
                if implementation.services_profile != *profile {
                    continue;
                }
                if !matches!(implementation.code, ImplementationCodeV1::Native { .. }) {
                    continue;
                }
                implementation.services_profile = services;
                for touched in [
                    &mut implementation.reads,
                    &mut implementation.writes,
                    &mut implementation.scans,
                ] {
                    touched.push(storage);
                    *touched = sorted(core::mem::take(touched));
                }
                replacements.insert(
                    *id,
                    self.publish(DefinitionDescriptorV1::Implementation(implementation))?,
                );
            }
            let mut selected_prerequisites = Vec::new();
            for prerequisite in &prerequisites {
                let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                    original.get(&prerequisite.implementation).cloned()
                else {
                    return Err(failure("game prerequisite implementation is not compiled"));
                };
                implementation.services_profile = services;
                for (touched, inherited) in [
                    (&mut implementation.reads, &bundle.reads),
                    (&mut implementation.writes, &bundle.writes),
                    (&mut implementation.scans, &bundle.scans),
                ] {
                    touched.extend(inherited.iter().copied().chain([storage]));
                    *touched = sorted(core::mem::take(touched));
                }
                let mut prerequisite = prerequisite.clone();
                prerequisite.implementation =
                    self.publish(DefinitionDescriptorV1::Implementation(implementation))?;
                selected_prerequisites.push(prerequisite);
            }
            let mut interfaces = BTreeMap::new();
            for (wire, id) in &base.interfaces {
                let Some(DefinitionDescriptorV1::Interface(mut interface)) =
                    original.get(id).cloned()
                else {
                    return Err(failure("native-game interface is not compiled"));
                };
                interface.events.clone_from(&bundle.events);
                interfaces.insert(
                    *wire,
                    self.publish(DefinitionDescriptorV1::Interface(interface))?,
                );
            }
            let rebinds = base.rebinds.clone();
            let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                original.get(&base.implementation).cloned()
            else {
                return Err(failure("native-game implementation is not compiled"));
            };
            implementation.entrypoint = EntrypointV1::Instructions(sorted(
                interfaces
                    .values()
                    .chain(rebinds.values())
                    .copied()
                    .collect(),
            ));
            implementation.services_profile = services;
            for (touched, inherited) in [
                (&mut implementation.reads, &bundle.reads),
                (&mut implementation.writes, &bundle.writes),
                (&mut implementation.scans, &bundle.scans),
            ] {
                touched.extend(inherited.iter().copied().chain([storage]));
                *touched = sorted(core::mem::take(touched));
            }
            implementation.events = bundle.events;
            implementation.permissions = bundle.permissions;
            implementation.parameters = bundle.parameters;
            implementation
                .invokes
                .retain(|id| !base.interfaces.values().any(|old| old == id));
            implementation.invokes.extend(bundle.invokes);
            implementation.invokes.extend(interfaces.values().copied());
            implementation.invokes = sorted(implementation.invokes);
            let implementation =
                self.publish(DefinitionDescriptorV1::Implementation(implementation))?;
            self.native_game_capture_compositions.insert(
                *profile,
                Composition {
                    services,
                    replacements,
                    prerequisites: selected_prerequisites,
                    addition: AdditionDefinitions {
                        module: base.module,
                        implementation,
                        interfaces,
                        rebinds,
                        storage: base.storage.clone(),
                        kernel: None,
                    },
                },
            );
        }
        Ok(())
    }

    /// Draft an ordinary governed transition to the compiled game service, preserving the
    /// current root's other selections. Game prerequisites enter atomically with captured transfer semantics.
    pub(super) fn draft_native_game_capture(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        current.check_closure(&self.definitions)?;
        let composition = self
            .native_game_capture_compositions
            .get(&current.services_profile)
            .ok_or_else(|| failure("no compiled native-game successor for this profile"))?;
        let addition = &composition.addition;
        let mut successor = current.clone();
        successor.services_profile = composition.services;
        let mut transitions = Vec::new();
        for storage in addition.storage.values() {
            if current.installed_storage.binary_search(storage).is_ok() {
                return Err(failure("native-game storage is already installed"));
            }
            successor.installed_storage.push(*storage);
            transitions.push(SelectionTransitionV1::InstallStorage(*storage));
        }
        successor.installed_storage = sorted(successor.installed_storage);
        for id in &mut successor.active {
            if matches!(
                self.definitions.get(id),
                Some(DefinitionDescriptorV1::Implementation(_))
            ) {
                if current
                    .restrictions
                    .iter()
                    .any(|entry| entry.definition == *id)
                {
                    return Err(failure(
                        "resolve restrictions on replaced implementations before the custody transition",
                    ));
                }
                let replacement = composition.replacements.get(id).ok_or_else(|| {
                    failure("selected implementation has no compiled custody successor")
                })?;
                transitions.push(SelectionTransitionV1::Restrict {
                    definition: *id,
                    restriction: RestrictionKindV1::Disabled,
                });
                transitions.push(SelectionTransitionV1::Deactivate(*id));
                transitions.push(SelectionTransitionV1::Activate(*replacement));
                *id = *replacement;
            }
        }
        for prerequisite in &composition.prerequisites {
            for storage in prerequisite.storage.values() {
                if successor.installed_storage.binary_search(storage).is_err() {
                    successor.installed_storage.push(*storage);
                    successor.installed_storage.sort_unstable();
                    transitions.push(SelectionTransitionV1::InstallStorage(*storage));
                }
            }
            for id in prerequisite
                .interfaces
                .values()
                .chain([&prerequisite.implementation])
            {
                if !successor.active.contains(id) {
                    successor.active.push(*id);
                    transitions.push(SelectionTransitionV1::Activate(*id));
                }
            }
        }
        successor.active.extend(
            addition
                .interfaces
                .values()
                .chain([&addition.implementation])
                .copied(),
        );
        successor.active = sorted(successor.active);
        for id in addition
            .interfaces
            .values()
            .chain([&addition.implementation])
        {
            transitions.push(SelectionTransitionV1::Activate(*id));
        }
        for binding in &mut successor.bindings {
            binding.implementation = *composition
                .replacements
                .get(&binding.implementation)
                .ok_or_else(|| failure("selected binding has no compiled custody successor"))?;
        }
        for prerequisite in &composition.prerequisites {
            for (wire, interface) in &prerequisite.interfaces {
                if let Some(binding) = successor
                    .bindings
                    .iter()
                    .find(|binding| binding.wire_id == *wire)
                {
                    if binding.interface != *interface {
                        return Err(failure(
                            "selected game prerequisite has a different immutable interface",
                        ));
                    }
                } else {
                    successor.bindings.push(BindingV1 {
                        wire_id: (*wire).into(),
                        interface: *interface,
                        implementation: prerequisite.implementation,
                    });
                }
            }
        }
        for (wire, interface) in addition.interfaces.iter().chain(&addition.rebinds) {
            successor
                .bindings
                .retain(|binding| binding.wire_id != *wire);
            successor.bindings.push(BindingV1 {
                wire_id: (*wire).into(),
                interface: *interface,
                implementation: addition.implementation,
            });
        }
        successor.bindings.sort_by(|a, b| a.wire_id.cmp(&b.wire_id));
        transitions.extend(
            successor
                .bindings
                .iter()
                .cloned()
                .map(SelectionTransitionV1::Bind),
        );
        transitions.push(SelectionTransitionV1::SetServicesProfile(
            composition.services,
        ));
        successor.check_closure(&self.definitions)?;
        let mut pending: Vec<_> = successor
            .active
            .iter()
            .chain(&successor.installed_storage)
            .copied()
            .chain([
                successor.kernel,
                successor.commitment_profile,
                successor.services_profile,
            ])
            .collect();
        let mut needed = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !needed.insert(id) {
                continue;
            }
            let descriptor = self
                .definitions
                .get(&id)
                .ok_or_else(|| failure("game completion dependency is absent"))?;
            pending.extend(
                descriptor
                    .typed_dependencies()
                    .into_iter()
                    .map(|(_, dependency)| dependency),
            );
        }
        let publish = needed
            .into_iter()
            .filter(|id| published.definition(id).is_none())
            .map(|id| self.definitions[&id].clone())
            .collect();
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
}

/// Accept the old game's dependencies only inside the complete immutable capture composition.
pub(super) fn selected_capture(
    selection: &RootSelectionV1,
    definitions: &impl DefinitionLookup,
) -> bool {
    if selection.check_closure(definitions).is_err() {
        return false;
    }
    let Some(DefinitionDescriptorV1::ServicesProfile(services)) =
        definitions.definition(&selection.services_profile)
    else {
        return false;
    };
    if !services.services.iter().any(|entry| {
        entry.name == SERVICE
            && entry.reference_closure == iroha_crypto::Hash::new(SERVICE_CLOSURE).into()
    }) {
        return false;
    }
    for (module, namespace) in [
        (additions::GAME_PARTITIONS.module, "world.game_partitions"),
        (
            additions::NATIVE_GAME_CAPTURE.module,
            "world.runtime_game_transfers",
        ),
    ] {
        if !selection.installed_storage.iter().any(|id| matches!(definitions.definition(id), Some(DefinitionDescriptorV1::Storage(storage)) if storage.namespace == namespace && storage.invariants == addition_digest(module, &[b"storage", namespace.as_bytes()]))) {
            return false;
        }
    }
    let expected_code = ImplementationCodeV1::Native {
        module: additions::NATIVE_GAME_CAPTURE.module.into(),
        reference_closure: addition_digest(
            additions::NATIVE_GAME_CAPTURE.module,
            &[b"implementation"],
        ),
    };
    let mut owner = None;
    for wire in additions::NATIVE_GAME_CAPTURE
        .instructions
        .iter()
        .chain(additions::NATIVE_GAME_CAPTURE.rebinds)
    {
        let Some(binding) = selection
            .bindings
            .iter()
            .find(|binding| binding.wire_id == *wire)
        else {
            return false;
        };
        if owner.is_some_and(|id| id != binding.implementation) {
            return false;
        }
        owner = Some(binding.implementation);
        if !matches!(definitions.definition(&binding.implementation), Some(DefinitionDescriptorV1::Implementation(implementation)) if implementation.code == expected_code && implementation.services_profile == selection.services_profile)
        {
            return false;
        }
    }
    // Partition installation, seeding and stepping keep the original native owner under the
    // selected service, which enforces the transfer lock. No foreign handler qualifies here.
    additions::GAME_PARTITIONS.instructions.iter()
        .filter(|wire| !additions::NATIVE_GAME_CAPTURE.rebinds.contains(wire))
        .all(|wire| selection.bindings.iter().find(|binding| binding.wire_id == *wire).is_some_and(|binding| {
            matches!(definitions.definition(&binding.implementation), Some(DefinitionDescriptorV1::Implementation(implementation))
                if implementation.code == ImplementationCodeV1::Native {
                    module: additions::GAME_PARTITIONS.module.into(),
                    reference_closure: addition_digest(additions::GAME_PARTITIONS.module, &[b"implementation"]),
                } && implementation.services_profile == selection.services_profile)
        }))
}

impl ReleaseCatalog {
    /// Kernel lifecycle tables compose after game exactly as their already compiled predecessors.
    pub(super) fn compile_runtime_lifecycle_after_native_game(
        &mut self,
    ) -> Result<(), CatalogError> {
        let previous = self.runtime_lifecycle_compositions.clone();
        for (profile, composition) in &self.native_game_capture_compositions {
            let Some(mut lifecycle) = previous.get(profile).cloned() else {
                continue;
            };
            lifecycle.implementation = *composition
                .replacements
                .get(&lifecycle.implementation)
                .ok_or_else(|| failure("kernel lifecycle has no compiled native-game successor"))?;
            self.runtime_lifecycle_compositions
                .insert(composition.services, lifecycle);
        }
        Ok(())
    }
}
