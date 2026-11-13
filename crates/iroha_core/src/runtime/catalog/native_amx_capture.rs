//! Immutable services-profile compositions for native AMX decision and settlement custody.

use super::*;
use iroha_data_model::runtime::native_amx::{SERVICE, SERVICE_CLOSURE};

#[derive(Clone, Debug)]
pub(super) struct Composition {
    pub(super) services: DefinitionId,
    pub(super) replacements: BTreeMap<DefinitionId, DefinitionId>,
    pub(super) addition: AdditionDefinitions,
}

fn failure(reason: impl Into<String>) -> CatalogError {
    CatalogError::Addition {
        module: additions::NATIVE_AMX_CAPTURE.module.into(),
        reason: reason.into(),
    }
}

impl ReleaseCatalog {
    /// Compile every currently supported native service family before publishing migrations.
    pub(super) fn compile_native_amx_capture(&mut self) -> Result<(), CatalogError> {
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == additions::NATIVE_AMX_CAPTURE.module)
            .cloned()
        else {
            return Ok(());
        };
        let original = self.definitions.clone();
        let storage = base.storage["world.runtime_amx_captures"];
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
            let mut interfaces = BTreeMap::new();
            for (wire, id) in &base.interfaces {
                let Some(DefinitionDescriptorV1::Interface(mut interface)) =
                    original.get(id).cloned()
                else {
                    return Err(failure("native-AMX interface is not compiled"));
                };
                interface.events.clone_from(&bundle.events);
                interfaces.insert(
                    *wire,
                    self.publish(DefinitionDescriptorV1::Interface(interface))?,
                );
            }
            let rebinds: BTreeMap<_, _> = base.rebinds.keys().map(|wire| {
                let interface = bundle.invokes.iter().find(|id| matches!(original.get(id),
                    Some(DefinitionDescriptorV1::Interface(interface)) if interface.wire_id == *wire))
                    .copied().ok_or_else(|| failure("native AMX family lacks its opening interface"))?;
                Ok((*wire, interface))
            }).collect::<Result<_, CatalogError>>()?;
            let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                original.get(&base.implementation).cloned()
            else {
                return Err(failure("native-AMX implementation is not compiled"));
            };
            implementation.entrypoint = EntrypointV1::Instructions(sorted(
                interfaces
                    .values()
                    .chain(rebinds.values())
                    .copied()
                    .collect(),
            ));
            implementation.services_profile = services;
            implementation.reads = sorted(bundle.reads.iter().copied().chain([storage]).collect());
            implementation.writes =
                sorted(bundle.writes.iter().copied().chain([storage]).collect());
            implementation.scans = sorted(bundle.scans.iter().copied().chain([storage]).collect());
            implementation.events = bundle.events;
            implementation.permissions = bundle.permissions;
            implementation.parameters = bundle.parameters;
            implementation.invokes = sorted(
                bundle
                    .invokes
                    .into_iter()
                    .chain(interfaces.values().copied())
                    .collect(),
            );
            let implementation =
                self.publish(DefinitionDescriptorV1::Implementation(implementation))?;
            self.native_amx_capture_compositions.insert(
                *profile,
                Composition {
                    services,
                    replacements,
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

    /// Draft an ordinary governed transition to the compiled AMX service, preserving the
    /// current root's other selections. Existing unresolved decisions and Yes custody must be drained at activation.
    pub(super) fn draft_native_amx_capture(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        current.check_closure(&self.definitions)?;
        let composition = self
            .native_amx_capture_compositions
            .get(&current.services_profile)
            .ok_or_else(|| failure("no compiled native-AMX successor for this profile"))?;
        let addition = &composition.addition;
        let mut successor = current.clone();
        successor.services_profile = composition.services;
        let mut transitions = Vec::new();
        for storage in addition.storage.values() {
            if current.installed_storage.binary_search(storage).is_ok() {
                return Err(failure("native-AMX storage is already installed"));
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
        let needed: BTreeSet<_> = successor
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

impl ReleaseCatalog {
    /// Kernel lifecycle tables compose after AMX exactly as their already compiled predecessors.
    pub(super) fn compile_runtime_lifecycle_after_native_amx(
        &mut self,
    ) -> Result<(), CatalogError> {
        let previous = self.runtime_lifecycle_compositions.clone();
        for (profile, composition) in &self.native_amx_capture_compositions {
            let Some(mut lifecycle) = previous.get(profile).cloned() else {
                continue;
            };
            lifecycle.implementation = *composition
                .replacements
                .get(&lifecycle.implementation)
                .ok_or_else(|| failure("kernel lifecycle has no compiled native-AMX successor"))?;
            self.runtime_lifecycle_compositions
                .insert(composition.services, lifecycle);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn select(catalog: &ReleaseCatalog, before: &RootSelectionV1, module: &str) -> RootSelectionV1 {
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([9; 32]),
        ));
        catalog
            .activation(module, before, &catalog.definitions, network, 50, [1; 32])
            .unwrap()
            .apply(before, &catalog.definitions)
            .unwrap()
            .selection
    }
    #[test]
    fn native_amx_composes_after_other_services_and_kernel_lifecycle_in_either_order() {
        let catalog = release_catalog().unwrap();
        for family in [&catalog.baseline, &catalog.lifecycle] {
            let escrow = select(catalog, family, additions::NATIVE_ESCROW_CAPTURE.module);
            let consent = select(catalog, &escrow, additions::DEFERRED_CONSENT.module);
            for prior in [family, &escrow, &consent] {
                let amx = select(catalog, prior, additions::NATIVE_AMX_CAPTURE.module);
                assert_eq!(catalog.unsupported(&amx), None);
                let lifecycle_after = select(catalog, &amx, additions::RUNTIME_LIFECYCLE.module);
                let lifecycle = select(catalog, prior, additions::RUNTIME_LIFECYCLE.module);
                let amx_after = select(catalog, &lifecycle, additions::NATIVE_AMX_CAPTURE.module);
                assert_eq!(lifecycle_after, amx_after);
            }
        }
    }
    #[test]
    fn later_independent_services_without_compiled_amx_compositions_refuse_explicitly() {
        let catalog = release_catalog().unwrap();
        let selected = select(
            catalog,
            &catalog.baseline,
            additions::NATIVE_AMX_CAPTURE.module,
        );
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([9; 32]),
        ));
        for module in [
            additions::DEFERRED_CONSENT.module,
            additions::NATIVE_ESCROW_CAPTURE.module,
        ] {
            let error = catalog
                .activation(
                    module,
                    &selected,
                    &catalog.definitions,
                    network,
                    50,
                    [1; 32],
                )
                .unwrap_err();
            assert!(error.to_string().contains("no compiled"), "{error}");
        }
    }
}
