//! Independently selected contract kernel bridge and its exact named ABI V1 host profile.

use super::*;

#[derive(Clone, Debug)]
pub(super) struct Composition {
    pub(super) host: DefinitionId,
    pub(super) registry: DefinitionId,
    pub(super) addition: AdditionDefinitions,
}
fn failure(reason: impl Into<String>) -> CatalogError {
    CatalogError::Addition {
        module: additions::CONTRACT_LIFECYCLE.module.into(),
        reason: reason.into(),
    }
}
impl ReleaseCatalog {
    pub(super) fn compile_contract_lifecycle(&mut self) -> Result<(), CatalogError> {
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == additions::CONTRACT_LIFECYCLE.module)
            .cloned()
        else {
            return Ok(());
        };
        let host = self.publish(DefinitionDescriptorV1::HostProfile(
            HostProfileDescriptorV1 {
                abi_version: 1,
                abi_hash: ivm::syscalls::compute_abi_hash(
                    ivm::SyscallPolicy::AbiV1RuntimeLifecycle,
                ),
                metering: ivm::gas::schedule_hash().into(),
            },
        ))?;
        for (services, lifecycle) in self.runtime_lifecycle_compositions.clone() {
            let registry = lifecycle.storage["world.runtime_obligations"];
            let Some(DefinitionDescriptorV1::Implementation(owner)) =
                self.definitions.get(&lifecycle.implementation).cloned()
            else {
                return Err(failure("compiled lifecycle implementation is absent"));
            };
            let mut interfaces = BTreeMap::new();
            for (wire, id) in &base.interfaces {
                let Some(DefinitionDescriptorV1::Interface(mut interface)) =
                    self.definitions.get(id).cloned()
                else {
                    return Err(failure("contract registration interface is absent"));
                };
                interface.events.clone_from(&owner.events);
                interfaces.insert(
                    *wire,
                    self.publish(DefinitionDescriptorV1::Interface(interface))?,
                );
            }
            let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                self.definitions.get(&base.implementation).cloned()
            else {
                return Err(failure("contract registration implementation is absent"));
            };
            implementation.entrypoint =
                EntrypointV1::Instructions(sorted(interfaces.values().copied().collect()));
            // This is the native kernel handler. The independently selected IVM host profile
            // authenticates its contract caller; it is not the execution engine of this handler.
            implementation.host_profile = None;
            implementation.services_profile = services;
            implementation.reads = owner.reads;
            implementation.writes = owner.writes;
            implementation.scans = owner.scans;
            implementation.events = owner.events;
            implementation.permissions = owner.permissions;
            implementation.parameters = owner.parameters;
            implementation.invokes = sorted(
                owner
                    .invokes
                    .into_iter()
                    .chain(interfaces.values().copied())
                    .collect(),
            );
            let implementation =
                self.publish(DefinitionDescriptorV1::Implementation(implementation))?;
            self.contract_lifecycle_compositions.insert(
                services,
                Composition {
                    host,
                    registry,
                    addition: AdditionDefinitions {
                        module: base.module,
                        implementation,
                        interfaces,
                        rebinds: BTreeMap::new(),
                        storage: BTreeMap::new(),
                        kernel: None,
                    },
                },
            );
        }
        Ok(())
    }
    pub(super) fn draft_contract_lifecycle(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        current.check_closure(&self.definitions)?;
        let composition = self
            .contract_lifecycle_compositions
            .get(&current.services_profile)
            .ok_or_else(|| {
                failure("no compiled contract lifecycle composition for this services profile")
            })?;
        if current
            .installed_storage
            .binary_search(&composition.registry)
            .is_err()
        {
            return Err(failure(
                "activate kernel lifecycle storage before the contract host profile",
            ));
        }
        let addition = &composition.addition;
        if current
            .bindings
            .iter()
            .any(|binding| addition.interfaces.contains_key(binding.wire_id.as_str()))
        {
            return Err(failure(
                "contract lifecycle registration is already selected",
            ));
        }
        let mut successor = current.clone();
        let active = sorted(
            addition
                .interfaces
                .values()
                .copied()
                .chain([composition.host, addition.implementation])
                .collect(),
        );
        let mut transitions: Vec<_> = active
            .iter()
            .copied()
            .filter(|id| current.active.binary_search(id).is_err())
            .map(SelectionTransitionV1::Activate)
            .collect();
        successor.active.extend(active.iter().copied());
        successor.active = sorted(successor.active);
        for (wire, interface) in &addition.interfaces {
            let binding = BindingV1 {
                wire_id: (*wire).into(),
                interface: *interface,
                implementation: addition.implementation,
            };
            transitions.push(SelectionTransitionV1::Bind(binding.clone()));
            successor.bindings.push(binding);
        }
        successor.bindings.sort_by(|a, b| a.wire_id.cmp(&b.wire_id));
        successor.check_closure(&self.definitions)?;
        let mut pending = active;
        let mut needed = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !needed.insert(id) {
                continue;
            }
            let descriptor = self
                .definitions
                .get(&id)
                .ok_or_else(|| failure("compiled dependency is absent"))?;
            pending.extend(
                descriptor
                    .typed_dependencies()
                    .into_iter()
                    .map(|(_, id)| id),
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

#[cfg(test)]
mod tests;
