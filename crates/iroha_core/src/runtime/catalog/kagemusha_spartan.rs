//! Bind the Spartan reserve guard to the exact existing native KAGEMUSHA ledger owner.
use super::*;
use iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerV1;

fn failure(reason: &str) -> CatalogError {
    CatalogError::Addition {
        module: additions::KAGEMUSHA_SPARTAN.module.into(),
        reason: reason.into(),
    }
}

impl ReleaseCatalog {
    pub(super) fn compile_kagemusha_spartan(&mut self) -> Result<(), CatalogError> {
        let Some(addition) = self
            .additions
            .iter()
            .find(|a| a.module == additions::KAGEMUSHA_SPARTAN.module)
        else {
            return Ok(());
        };
        let original = addition.implementation;
        let ledger = addition.rebinds[KagemushaWalletLedgerV1::WIRE_ID];
        let baseline = self
            .baseline
            .bindings
            .iter()
            .find(|b| b.wire_id == KagemushaWalletLedgerV1::WIRE_ID)
            .ok_or_else(|| failure("missing baseline ledger"))?
            .implementation;
        let Some(DefinitionDescriptorV1::Implementation(template)) =
            self.definitions.get(&original).cloned()
        else {
            return Err(failure("missing compiled Spartan implementation"));
        };
        let owners: Vec<_> = self
            .definitions
            .iter()
            .filter_map(|(id, descriptor)| {
                let DefinitionDescriptorV1::Implementation(implementation) = descriptor else {
                    return None;
                };
                let ImplementationCodeV1::Native { module, .. } = &implementation.code else {
                    return None;
                };
                if !matches!(
                    module.as_str(),
                    BASELINE_MODULE | LIFECYCLE_MODULE | "iroha.ironwood.v1"
                ) {
                    return None;
                }
                let EntrypointV1::Instructions(ref interfaces) = implementation.entrypoint else {
                    return None;
                };
                interfaces
                    .contains(&ledger)
                    .then_some((*id, implementation.clone()))
            })
            .collect();
        for (id, predecessor) in owners {
            let compiled = if id == baseline {
                original
            } else {
                let mut successor = template.clone();
                successor.code = ImplementationCodeV1::Native {
                    module: additions::KAGEMUSHA_SPARTAN.module.into(),
                    reference_closure: addition_digest(
                        additions::KAGEMUSHA_SPARTAN.module,
                        &[b"ledger-predecessor", &id.0],
                    ),
                };
                successor.services_profile = predecessor.services_profile;
                successor.host_profile = predecessor.host_profile;
                successor.limits = predecessor.limits;
                fn include(target: &mut Vec<DefinitionId>, source: Vec<DefinitionId>) {
                    target.extend(source);
                    target.sort_unstable();
                    target.dedup();
                }
                include(&mut successor.reads, predecessor.reads);
                include(&mut successor.writes, predecessor.writes);
                include(&mut successor.scans, predecessor.scans);
                include(&mut successor.permissions, predecessor.permissions);
                include(&mut successor.events, predecessor.events);
                include(&mut successor.parameters, predecessor.parameters);
                include(&mut successor.invokes, predecessor.invokes);
                self.publish(DefinitionDescriptorV1::Implementation(successor))?
            };
            self.spartan_predecessors.insert(id, compiled);
        }
        Ok(())
    }

    pub(super) fn spartan_for_selection(
        &self,
        addition: &AdditionDefinitions,
        current: &RootSelectionV1,
    ) -> Result<AdditionDefinitions, CatalogError> {
        let predecessor = current
            .bindings
            .iter()
            .find(|b| {
                b.wire_id == KagemushaWalletLedgerV1::WIRE_ID
                    && addition.rebinds.get(KagemushaWalletLedgerV1::WIRE_ID) == Some(&b.interface)
            })
            .ok_or_else(|| failure("no currently bound native KAGEMUSHA ledger"))?;
        let mut selected = addition.clone();
        selected.implementation = *self
            .spartan_predecessors
            .get(&predecessor.implementation)
            .ok_or_else(|| {
                failure("KAGEMUSHA ledger predecessor is not supported by this release")
            })?;
        Ok(selected)
    }
}
