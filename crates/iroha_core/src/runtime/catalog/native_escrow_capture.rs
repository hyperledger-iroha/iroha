//! Immutable services-profile compositions for native escrow custody.

use super::*;
use iroha_data_model::runtime::native_escrow::{SERVICE, SERVICE_CLOSURE};

#[derive(Clone, Debug)]
pub(super) struct Composition {
    pub(super) services: DefinitionId,
    pub(super) replacements: BTreeMap<DefinitionId, DefinitionId>,
    pub(super) addition: AdditionDefinitions,
}

fn failure(reason: impl Into<String>) -> CatalogError {
    CatalogError::Addition {
        module: additions::NATIVE_ESCROW_CAPTURE.module.into(),
        reason: reason.into(),
    }
}

impl ReleaseCatalog {
    /// Compile every currently supported native service family before publishing migrations.
    pub(super) fn compile_native_escrow_capture(&mut self) -> Result<(), CatalogError> {
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == additions::NATIVE_ESCROW_CAPTURE.module)
            .cloned()
        else {
            return Ok(());
        };
        let original = self.definitions.clone();
        let storage = base.storage["world.runtime_escrow_captures"];
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
                    return Err(failure("native-escrow interface is not compiled"));
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
                    .copied().ok_or_else(|| failure("native escrow family lacks its opening interface"))?;
                Ok((*wire, interface))
            }).collect::<Result<_, CatalogError>>()?;
            let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                original.get(&base.implementation).cloned()
            else {
                return Err(failure("native-escrow implementation is not compiled"));
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
            self.native_escrow_capture_compositions.insert(
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

    /// Draft an ordinary governed transition to the compiled custody service, preserving the
    /// current root's other selections. Existing live custody must be drained at activation.
    pub(super) fn draft_native_escrow_capture(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        current.check_closure(&self.definitions)?;
        let composition = self
            .native_escrow_capture_compositions
            .get(&current.services_profile)
            .ok_or_else(|| failure("no compiled native-escrow successor for this profile"))?;
        let addition = &composition.addition;
        let mut successor = current.clone();
        successor.services_profile = composition.services;
        let mut transitions = Vec::new();
        for storage in addition.storage.values() {
            if current.installed_storage.binary_search(storage).is_ok() {
                return Err(failure("native-escrow storage is already installed"));
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
    /// Kernel lifecycle State composes with either order of the two independent custody services.
    pub(super) fn compile_runtime_lifecycle_after_native_escrow(
        &mut self,
    ) -> Result<(), CatalogError> {
        if self.native_escrow_capture_compositions.is_empty() {
            return Ok(());
        }
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == additions::RUNTIME_LIFECYCLE.module)
            .cloned()
        else {
            return Ok(());
        };
        let mut families = vec![self.baseline.clone(), self.lifecycle.clone()];
        if let Some(ironwood) = &self.ironwood {
            let family = ironwood.selection.clone();
            families.push(self.lifecycle_successor(&family)?);
            families.push(family);
        }
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0; 32]),
        ));
        for family in families {
            let native = self
                .draft_native_escrow_capture(&family, &self.definitions, network, 1, [0; 32])?
                .apply(&family, &self.definitions)
                .map_err(|error| failure(error.to_string()))?
                .selection;
            let mut selected = vec![native.clone()];
            if !self.deferred_consent_compositions.is_empty() {
                selected.push(
                    self.draft_deferred_consent(&native, &self.definitions, network, 1, [0; 32])?
                        .apply(&native, &self.definitions)
                        .map_err(|error| failure(error.to_string()))?
                        .selection,
                );
            }
            for selected in selected {
                let composition = self.compose_runtime_lifecycle(&base, &selected)?;
                self.runtime_lifecycle_compositions
                    .insert(selected.services_profile, composition);
            }
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
    fn native_escrow_and_consent_services_compose_in_both_orders_without_changing_baseline() {
        let catalog = release_catalog().unwrap();
        for family in [&catalog.baseline, &catalog.lifecycle] {
            let native = select(catalog, family, additions::NATIVE_ESCROW_CAPTURE.module);
            let consent_after = select(catalog, &native, additions::DEFERRED_CONSENT.module);
            let consent = select(catalog, family, additions::DEFERRED_CONSENT.module);
            let native_after = select(catalog, &consent, additions::NATIVE_ESCROW_CAPTURE.module);
            assert_eq!(consent_after, native_after);
            assert_eq!(catalog.unsupported(&native_after), None);
            assert!(!super::super::super::native_escrow_capture::enabled(family).unwrap());
            assert!(super::super::super::native_escrow_capture::enabled(&native_after).unwrap());
        }
    }
    #[test]
    fn native_escrow_and_kernel_lifecycle_compose_in_both_orders() {
        let catalog = release_catalog().unwrap();
        for family in [&catalog.baseline, &catalog.lifecycle] {
            let native = select(catalog, family, additions::NATIVE_ESCROW_CAPTURE.module);
            let lifecycle_after = select(catalog, &native, additions::RUNTIME_LIFECYCLE.module);
            let lifecycle = select(catalog, family, additions::RUNTIME_LIFECYCLE.module);
            let native_after = select(catalog, &lifecycle, additions::NATIVE_ESCROW_CAPTURE.module);
            assert_eq!(lifecycle_after, native_after);
            assert_eq!(catalog.unsupported(&native_after), None);
        }
    }
}
