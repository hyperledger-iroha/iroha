//! Immutable services-profile compositions for deferred consent.

use super::*;
use iroha_data_model::runtime::deferred::{SERVICE, SERVICE_CLOSURE};

#[derive(Clone, Debug)]
pub(super) struct Composition {
    pub(super) services: DefinitionId,
    pub(super) replacements: BTreeMap<DefinitionId, DefinitionId>,
    pub(super) addition: AdditionDefinitions,
}

fn failure(reason: impl Into<String>) -> CatalogError {
    CatalogError::Addition {
        module: additions::DEFERRED_CONSENT.module.into(),
        reason: reason.into(),
    }
}

impl ReleaseCatalog {
    /// Compile every currently supported native service family before publishing migrations.
    pub(super) fn compile_deferred_consent(&mut self) -> Result<(), CatalogError> {
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == additions::DEFERRED_CONSENT.module)
            .cloned()
        else {
            return Ok(());
        };
        let original = self.definitions.clone();
        let storage = base.storage["world.runtime_deferred_consents"];
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
                    return Err(failure("deferred-consent interface is not compiled"));
                };
                interface.events.clone_from(&bundle.events);
                interfaces.insert(
                    *wire,
                    self.publish(DefinitionDescriptorV1::Interface(interface))?,
                );
            }
            let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                original.get(&base.implementation).cloned()
            else {
                return Err(failure("deferred-consent implementation is not compiled"));
            };
            implementation.entrypoint =
                EntrypointV1::Instructions(sorted(interfaces.values().copied().collect()));
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
            self.deferred_consent_compositions.insert(
                *profile,
                Composition {
                    services,
                    replacements,
                    addition: AdditionDefinitions {
                        module: base.module,
                        implementation,
                        interfaces,
                        rebinds: BTreeMap::new(),
                        storage: base.storage.clone(),
                        kernel: None,
                    },
                },
            );
        }
        Ok(())
    }

    /// Draft an ordinary governed transition to the compiled consent service, preserving the
    /// current root's other selections. Existing deferred work must be drained at activation.
    pub(super) fn draft_deferred_consent(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        current.check_closure(&self.definitions)?;
        let composition = self
            .deferred_consent_compositions
            .get(&current.services_profile)
            .ok_or_else(|| failure("no compiled deferred-consent successor for this profile"))?;
        let addition = &composition.addition;
        let mut successor = current.clone();
        successor.services_profile = composition.services;
        let mut transitions = Vec::new();
        for storage in addition.storage.values() {
            if current.installed_storage.binary_search(storage).is_ok() {
                return Err(failure("deferred-consent storage is already installed"));
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
                        "resolve restrictions on replaced implementations before the consent transition",
                    ));
                }
                let replacement = composition.replacements.get(id).ok_or_else(|| {
                    failure("selected implementation has no compiled consent successor")
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
                .ok_or_else(|| failure("selected binding has no compiled consent successor"))?;
        }
        for (wire, interface) in &addition.interfaces {
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
    fn deferred_consent_and_lifecycle_addition_compose_in_either_order() {
        let catalog = release_catalog().unwrap();
        for family in [&catalog.baseline, &catalog.lifecycle] {
            let lifecycle_first = select(catalog, family, additions::RUNTIME_LIFECYCLE.module);
            let consent_last = select(
                catalog,
                &lifecycle_first,
                additions::DEFERRED_CONSENT.module,
            );
            let consent_first = select(catalog, family, additions::DEFERRED_CONSENT.module);
            let lifecycle_last =
                select(catalog, &consent_first, additions::RUNTIME_LIFECYCLE.module);
            assert_eq!(consent_last, lifecycle_last);
            assert_eq!(catalog.unsupported(&lifecycle_last), None);
        }
    }

    #[test]
    fn deferred_consent_and_time_services_compose_in_either_order() {
        let catalog = release_catalog().unwrap();
        let mut families = vec![catalog.baseline.clone()];
        if let Some(ironwood) = &catalog.ironwood {
            families.push(ironwood.selection.clone());
        }
        for family in families {
            let consent_first = select(catalog, &family, additions::DEFERRED_CONSENT.module);
            let time_last = catalog.clone().lifecycle_successor(&consent_first).unwrap();
            let time_first = catalog.clone().lifecycle_successor(&family).unwrap();
            let consent_last = select(catalog, &time_first, additions::DEFERRED_CONSENT.module);
            assert_eq!(time_last, consent_last);
            assert_eq!(catalog.unsupported(&time_last), None);
        }
    }

    #[test]
    fn deferred_consent_is_inert_until_governance_selects_its_compiled_service() {
        let catalog = release_catalog().unwrap();
        assert!(!super::super::super::deferred_consent::enabled(&catalog.baseline).unwrap());
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([9; 32]),
        ));
        let mut families = vec![catalog.baseline.clone(), catalog.lifecycle.clone()];
        if let Some(ironwood) = &catalog.ironwood {
            families.push(ironwood.selection.clone());
            families.push(
                catalog
                    .clone()
                    .lifecycle_successor(&ironwood.selection)
                    .unwrap(),
            );
        }
        for before in families {
            let change = catalog
                .activation(
                    additions::DEFERRED_CONSENT.module,
                    &before,
                    &catalog.definitions,
                    network,
                    50,
                    [1; 32],
                )
                .unwrap();
            let after = change
                .apply(&before, &catalog.definitions)
                .unwrap()
                .selection;
            assert!(super::super::super::deferred_consent::enabled(&after).unwrap());
            assert_eq!(after.kernel, before.kernel);
            assert_eq!(after.commitment_profile, before.commitment_profile);
            assert_eq!(catalog.unsupported(&after), None);
            assert_eq!(
                after.installed_storage.len(),
                before.installed_storage.len() + 1
            );
            for previous in &before.bindings {
                let selected = after
                    .bindings
                    .iter()
                    .find(|binding| binding.wire_id == previous.wire_id)
                    .unwrap();
                assert_eq!(selected.interface, previous.interface);
                assert_ne!(selected.implementation, previous.implementation);
                assert_eq!(
                    catalog.native_module(&selected.implementation),
                    catalog.native_module(&previous.implementation)
                );
            }
        }
    }
}
