//! Composition of an explicitly selected native service with existing immutable definitions.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct Composition {
    services: DefinitionId,
    replacements: BTreeMap<DefinitionId, DefinitionId>,
    active: Vec<DefinitionId>,
    addition: AdditionDefinitions,
}

impl Composition {
    /// Whether the selection includes this service's exact auxiliary interfaces.
    pub(super) fn selects_auxiliary_definitions(&self, selection: &RootSelectionV1) -> bool {
        self.active
            .iter()
            .all(|id| selection.active.binary_search(id).is_ok())
    }

    #[cfg(test)]
    /// All definitions this composition contributes to the compiled catalog.
    pub(super) fn definitions(&self) -> impl Iterator<Item = DefinitionId> + '_ {
        core::iter::once(self.services)
            .chain(self.replacements.values().copied())
            .chain(self.active.iter().copied())
            .chain(self.addition.definitions())
    }
}

impl ReleaseCatalog {
    /// Compile every currently supported native service family before publishing migrations.
    pub(super) fn compose_service_addition(
        &mut self,
        module: &str,
        service: ServiceEntryV1,
        host: Option<DefinitionId>,
        event: Option<DefinitionId>,
    ) -> Result<BTreeMap<DefinitionId, Composition>, CatalogError> {
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == module)
            .cloned()
        else {
            return Ok(BTreeMap::new());
        };
        let original = self.definitions.clone();
        // Preserve catalog order within each source profile without repeatedly copying
        // the unrelated (and often large) native implementation descriptors.
        let mut native = BTreeMap::<_, Vec<_>>::new();
        for (id, descriptor) in &original {
            if let DefinitionDescriptorV1::Implementation(implementation) = descriptor
                && matches!(implementation.code, ImplementationCodeV1::Native { .. })
            {
                native
                    .entry(implementation.services_profile)
                    .or_default()
                    .push((*id, implementation));
            }
        }
        let mut compositions = BTreeMap::new();
        let failure = |reason: &str| CatalogError::Addition {
            module: module.into(),
            reason: reason.into(),
        };
        for (profile, descriptor) in &original {
            let DefinitionDescriptorV1::ServicesProfile(services) = descriptor else {
                continue;
            };
            if services
                .services
                .iter()
                .any(|entry| entry.name == service.name)
            {
                continue;
            }
            let Some(implementations) = native.get(profile) else {
                continue;
            };
            let Some(bundle) = implementations.iter().find_map(|&(_, implementation)| {
                matches!(&implementation.code, ImplementationCodeV1::Native { module, .. }
                    if matches!(module.as_str(), BASELINE_MODULE | LIFECYCLE_MODULE | "iroha.ironwood.v1"))
                .then(|| implementation.clone())
            }) else {
                continue;
            };
            let mut services = services.clone();
            services.services.push(service.clone());
            services.services.sort_by(|a, b| a.name.cmp(&b.name));
            let services = self.publish(DefinitionDescriptorV1::ServicesProfile(services))?;
            let mut replacements = BTreeMap::new();
            for &(id, predecessor) in implementations {
                let mut implementation = predecessor.clone();
                implementation.services_profile = services;
                for touched in [
                    &mut implementation.reads,
                    &mut implementation.writes,
                    &mut implementation.scans,
                ] {
                    touched.extend(base.storage.values().copied());
                    *touched = sorted(core::mem::take(touched));
                }
                replacements.insert(
                    id,
                    self.publish(DefinitionDescriptorV1::Implementation(implementation))?,
                );
            }
            let mut interfaces = BTreeMap::new();
            for (wire, id) in &base.interfaces {
                let Some(DefinitionDescriptorV1::Interface(mut interface)) =
                    original.get(id).cloned()
                else {
                    return Err(failure("service addition interface is not compiled"));
                };
                interface.events = sorted(bundle.events.iter().copied().chain(event).collect());
                interfaces.insert(
                    *wire,
                    self.publish(DefinitionDescriptorV1::Interface(interface))?,
                );
            }
            let rebinds: BTreeMap<_, _> = base.rebinds.keys().map(|wire| {
                let interface = bundle.invokes.iter().find(|id| matches!(original.get(id),
                    Some(DefinitionDescriptorV1::Interface(interface)) if interface.wire_id == *wire))
                    .copied().ok_or_else(|| failure("service family lacks its opening interface"))?;
                Ok((*wire, interface))
            }).collect::<Result<_, CatalogError>>()?;
            let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                original.get(&base.implementation).cloned()
            else {
                return Err(failure("service addition implementation is not compiled"));
            };
            implementation.entrypoint = EntrypointV1::Instructions(sorted(
                interfaces
                    .values()
                    .chain(rebinds.values())
                    .copied()
                    .collect(),
            ));
            implementation.services_profile = services;
            implementation.reads = sorted(
                bundle
                    .reads
                    .iter()
                    .copied()
                    .chain(base.storage.values().copied())
                    .collect(),
            );
            implementation.writes = sorted(
                bundle
                    .writes
                    .iter()
                    .copied()
                    .chain(base.storage.values().copied())
                    .collect(),
            );
            implementation.scans = sorted(
                bundle
                    .scans
                    .iter()
                    .copied()
                    .chain(base.storage.values().copied())
                    .collect(),
            );
            implementation.events = sorted(bundle.events.into_iter().chain(event).collect());
            // This registration handler is native. The root selects the VM host separately.
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
            compositions.insert(
                *profile,
                Composition {
                    services,
                    replacements,
                    active: host.into_iter().chain(event).collect(),
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
        Ok(compositions)
    }

    /// Draft only the selected predecessor's bindings, storage and service closure.
    pub(super) fn draft_service_addition(
        &self,
        composition: &Composition,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        current.check_closure(&self.definitions)?;
        let failure = |reason: &str| CatalogError::Addition {
            module: composition.addition.module.into(),
            reason: reason.into(),
        };
        let addition = &composition.addition;
        let mut successor = current.clone();
        successor.services_profile = composition.services;
        let mut transitions = Vec::new();
        for storage in addition.storage.values() {
            if current.installed_storage.binary_search(storage).is_ok() {
                return Err(failure("service addition storage is already installed"));
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
                        "resolve restrictions on replaced implementations before the services transition",
                    ));
                }
                let replacement = composition.replacements.get(id).ok_or_else(|| {
                    failure("selected implementation has no compiled service successor")
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
                .chain(&composition.active)
                .copied(),
        );
        successor.active = sorted(successor.active);
        for id in addition
            .interfaces
            .values()
            .chain([&addition.implementation])
            .chain(&composition.active)
        {
            transitions.push(SelectionTransitionV1::Activate(*id));
        }
        for binding in &mut successor.bindings {
            binding.implementation = *composition
                .replacements
                .get(&binding.implementation)
                .ok_or_else(|| failure("selected binding has no compiled service successor"))?;
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
            .map_err(|error| failure(&error.to_string()))?;
        Ok(change)
    }
}
