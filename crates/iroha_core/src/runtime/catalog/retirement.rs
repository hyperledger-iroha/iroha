//! Immutable service compositions for permanent runtime retirement history.

use super::*;
use iroha_data_model::runtime::retirement::{SERVICE, SERVICE_CLOSURE};

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

    #[test]
    fn retirement_composes_after_signed_child_foundation_in_either_service_order() {
        let catalog = release_catalog().unwrap();
        let (foundation, published) = catalog.child_foundation().unwrap();
        let original = foundation.clone();
        let baseline = catalog.baseline.clone();
        let change = catalog
            .activation(
                additions::RUNTIME_RETIREMENT.module,
                &foundation,
                &published,
                network(),
                50,
                [5; 32],
            )
            .unwrap();
        let after = change.apply(&foundation, &published).unwrap();
        let mut actual = published;
        actual.extend(after.published);
        after.selection.check_closure(&actual).unwrap();

        let lifecycle = select(catalog, &baseline, additions::RUNTIME_LIFECYCLE.module);
        let retirement = select(catalog, &lifecycle, additions::RUNTIME_RETIREMENT.module);
        let reversed = select(catalog, &retirement, additions::CHILD_ADMIN.module);
        assert_eq!(after.selection, reversed);
        assert_eq!(catalog.unsupported(&after.selection), None);
        assert_eq!(catalog.child_foundation().unwrap().0, original);
        assert_eq!(catalog.baseline, baseline);
        assert!(
            after
                .selection
                .installed_storage
                .contains(&catalog.world_storage["world.runtime_child_admin_state"])
        );
        assert!(
            after
                .selection
                .installed_storage
                .contains(&catalog.world_storage["world.runtime_retirements"])
        );
        // Compiling this native composition never advertises the unfinished carrier kernel.
        let generic = catalog.generic_carriers.unwrap();
        for id in [
            generic.kernel,
            generic.host,
            generic.backing,
            generic.commitment,
        ] {
            assert!(!catalog.executes_definition(&id));
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
        self.retirement_compositions = self.compose_service_addition(
            additions::RUNTIME_RETIREMENT.module,
            ServiceEntryV1 {
                name: SERVICE.into(),
                reference_closure: iroha_crypto::Hash::new(SERVICE_CLOSURE).into(),
            },
            None,
            None,
        )?;
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
        self.draft_service_addition(
            composition,
            current,
            published,
            network,
            effective_height,
            qualification,
        )
    }
}
