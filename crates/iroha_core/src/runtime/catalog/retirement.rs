//! Immutable service compositions for permanent runtime retirement history.

use super::*;
use iroha_data_model::runtime::retirement::{SERVICE, SERVICE_CLOSURE};

#[derive(Clone, Debug)]
pub(super) struct Composition {
    pub(super) services: DefinitionId,
    pub(super) replacements: BTreeMap<DefinitionId, DefinitionId>,
    pub(super) addition: AdditionDefinitions,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([73; 32]),
        ))
    }

    fn select(catalog: &ReleaseCatalog, before: &RootSelectionV1, module: &str) -> RootSelectionV1 {
        catalog
            .activation(module, before, &catalog.definitions, network(), 50, [3; 32])
            .unwrap_or_else(|error| panic!("{module}: {error}"))
            .apply(before, &catalog.definitions)
            .unwrap()
            .selection
    }

    #[test]
    fn retirement_requires_floor_storage_and_preserves_immutable_source_aliases() {
        let catalog = release_catalog().unwrap();
        let baseline = catalog.baseline.clone();
        assert!(
            catalog
                .activation(
                    additions::RUNTIME_RETIREMENT.module,
                    &baseline,
                    &catalog.definitions,
                    network(),
                    50,
                    [3; 32],
                )
                .is_err()
        );
        let lifecycle = select(catalog, &baseline, additions::RUNTIME_LIFECYCLE.module);
        let selected = select(catalog, &lifecycle, additions::RUNTIME_RETIREMENT.module);
        assert_eq!(catalog.baseline, baseline);
        assert_eq!(catalog.unsupported(&selected), None);
        assert!(
            catalog
                .service_enabled(
                    selected.services_profile,
                    SERVICE,
                    iroha_crypto::Hash::new(SERVICE_CLOSURE).into(),
                )
                .unwrap()
        );
        for old in &lifecycle.bindings {
            let new = selected
                .bindings
                .iter()
                .find(|binding| binding.wire_id == old.wire_id)
                .unwrap();
            assert_ne!(old.implementation, new.implementation);
            let (
                DefinitionDescriptorV1::Implementation(old_descriptor),
                DefinitionDescriptorV1::Implementation(new_descriptor),
            ) = (
                &catalog.definitions[&old.implementation],
                &catalog.definitions[&new.implementation],
            )
            else {
                panic!("compiled native implementations");
            };
            assert_eq!(old_descriptor.code, new_descriptor.code);
            assert!(!selected.active.contains(&old.implementation));
            assert!(catalog.definitions.contains_key(&old.implementation));
        }
    }

    #[test]
    fn retirement_composes_after_selected_capture_and_contract_services() {
        let catalog = release_catalog().unwrap();
        let mut selected = catalog.baseline.clone();
        for module in [
            additions::NATIVE_ESCROW_CAPTURE.module,
            additions::DEFERRED_CONSENT.module,
            additions::NATIVE_AMX_CAPTURE.module,
            additions::NATIVE_GAME_CAPTURE.module,
            additions::RUNTIME_LIFECYCLE.module,
            additions::CONTRACT_LIFECYCLE.module,
        ] {
            selected = select(catalog, &selected, module);
        }
        let original = catalog.definitions[&selected.services_profile].clone();
        let after = select(catalog, &selected, additions::RUNTIME_RETIREMENT.module);
        assert_eq!(catalog.unsupported(&after), None);
        assert_eq!(catalog.definitions[&selected.services_profile], original);
        let DefinitionDescriptorV1::ServicesProfile(before) = original else {
            panic!("services");
        };
        let DefinitionDescriptorV1::ServicesProfile(after) =
            &catalog.definitions[&after.services_profile]
        else {
            panic!("services");
        };
        for service in before.services {
            assert!(after.services.contains(&service));
        }
    }
}

fn failure(reason: impl Into<String>) -> CatalogError {
    CatalogError::Addition {
        module: additions::RUNTIME_RETIREMENT.module.into(),
        reason: reason.into(),
    }
}

impl ReleaseCatalog {
    /// Compile every currently supported native service family before publishing migrations.
    pub(super) fn compile_retirement(&mut self) -> Result<(), CatalogError> {
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == additions::RUNTIME_RETIREMENT.module)
            .cloned()
        else {
            return Ok(());
        };
        let original = self.definitions.clone();
        let storage = base.storage["world.runtime_retirements"];
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
                    return Err(failure("runtime-retirement interface is not compiled"));
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
                    .copied().ok_or_else(|| failure("runtime retirement family lacks its opening interface"))?;
                Ok((*wire, interface))
            }).collect::<Result<_, CatalogError>>()?;
            let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                original.get(&base.implementation).cloned()
            else {
                return Err(failure("runtime-retirement implementation is not compiled"));
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
            self.retirement_compositions.insert(
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

    /// Draft the permanent history service after the root has installed governed floor policy.
    pub(super) fn draft_retirement(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        current.check_closure(&self.definitions)?;
        if !current.installed_storage.iter().any(|id| matches!(self.definitions.get(id),
            Some(DefinitionDescriptorV1::Storage(storage)) if storage.namespace == "world.runtime_boundaries"))
        {
            return Err(failure("runtime retirement requires the selected governed startup-floor storage"));
        }
        let composition = self
            .retirement_compositions
            .get(&current.services_profile)
            .ok_or_else(|| failure("no compiled runtime-retirement successor for this profile"))?;
        let addition = &composition.addition;
        let mut successor = current.clone();
        successor.services_profile = composition.services;
        let mut transitions = Vec::new();
        for storage in addition.storage.values() {
            if current.installed_storage.binary_search(storage).is_ok() {
                return Err(failure("runtime-retirement storage is already installed"));
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
                        "resolve restrictions on replaced implementations before the retirement transition",
                    ));
                }
                let replacement = composition.replacements.get(id).ok_or_else(|| {
                    failure("selected implementation has no compiled retirement successor")
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
                .ok_or_else(|| failure("selected binding has no compiled retirement successor"))?;
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
