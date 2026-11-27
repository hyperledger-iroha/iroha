//! Dormant immutable service compositions for independently governed child roots.

use super::*;
use iroha_data_model::runtime::child_admin::CHILD_ADMIN_SERVICE_CLOSURE_V1;

impl ReleaseCatalog {
    /// The fixed fresh-child foundation, independent of the eventual genesis hash.
    ///
    /// This clones the already compiled definitions and selection without encoding or applying
    /// another change set. It grants no authority to replace a
    /// running root; installation is restricted to the original signed-genesis source owner.
    pub(crate) fn child_foundation(
        &self,
    ) -> Result<
        (
            RootSelectionV1,
            BTreeMap<DefinitionId, DefinitionDescriptorV1>,
        ),
        CatalogError,
    > {
        self.child_foundation
            .clone()
            .ok_or_else(|| CatalogError::Addition {
                module: additions::CHILD_ADMIN.module.into(),
                reason: "this catalog has no compiled child foundation".into(),
            })
    }

    /// Compile only during catalog construction, outside original genesis execution attempts.
    fn build_child_foundation(
        &self,
    ) -> Result<
        (
            RootSelectionV1,
            BTreeMap<DefinitionId, DefinitionDescriptorV1>,
        ),
        CatalogError,
    > {
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0; 32]),
        ));
        let mut selection = self.baseline.clone();
        let mut definitions: BTreeMap<_, _> = self
            .baseline_definitions
            .iter()
            .map(|id| (*id, self.definitions[id].clone()))
            .collect();
        for module in [
            additions::RUNTIME_LIFECYCLE.module,
            additions::CHILD_ADMIN.module,
        ] {
            let change = self.activation(module, &selection, &definitions, network, 1, [0; 32])?;
            let applied =
                change
                    .apply(&selection, &definitions)
                    .map_err(|error| CatalogError::Addition {
                        module: additions::CHILD_ADMIN.module.into(),
                        reason: error.to_string(),
                    })?;
            definitions.extend(applied.published);
            selection = applied.selection;
        }
        selection.check_closure(&definitions)?;
        Ok((selection, definitions))
    }

    /// Publish explicit child-authority successors while retaining every predecessor definition.
    pub(super) fn compile_child_admin(&mut self) -> Result<(), CatalogError> {
        self.child_admin_compositions = self.compose_service_addition(
            additions::CHILD_ADMIN.module,
            ServiceEntryV1 {
                name: "child_runtime_admin_v1".into(),
                reference_closure: iroha_crypto::Hash::new(CHILD_ADMIN_SERVICE_CLOSURE_V1).into(),
            },
            None,
            None,
        )?;
        if !self.child_admin_compositions.is_empty() {
            self.child_foundation = Some(self.build_child_foundation()?);
        }
        Ok(())
    }

    /// Draft installation only. A populated administrator policy needs authenticated foundation.
    pub(super) fn draft_child_admin(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        let composition = self
            .child_admin_compositions
            .get(&current.services_profile)
            .ok_or_else(|| CatalogError::Addition {
                module: additions::CHILD_ADMIN.module.into(),
                reason: "no compiled child-administrator successor for the selected services"
                    .into(),
            })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foundation_accessor_uses_cached_recipe_without_active_codec_admission() {
        let catalog = release_catalog().unwrap();
        let original = catalog.child_foundation().unwrap();
        let zero = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            0,
            usize::MAX,
        ));
        assert_eq!(zero.with(|| catalog.child_foundation()).unwrap(), original);
    }

    #[test]
    fn catalog_without_child_addition_has_no_implicit_foundation() {
        let catalog = ReleaseCatalog::build_from(&[]).unwrap();
        assert!(catalog.child_admin_compositions.is_empty());
        assert!(catalog.child_foundation().is_err());
    }

    #[test]
    fn fresh_child_foundation_has_lifecycle_and_explicit_empty_authority_storage() {
        let catalog = release_catalog().unwrap();
        let (selection, definitions) = catalog.child_foundation().unwrap();
        assert_ne!(selection.id().unwrap(), catalog.baseline_id);
        selection.check_closure(&definitions).unwrap();
        for field in [
            "world.runtime_child_admin_state",
            "world.runtime_boundaries",
            "world.runtime_obligations",
        ] {
            assert!(
                selection
                    .installed_storage
                    .contains(&catalog.world_storage[field])
            );
        }
        assert_eq!(catalog.unsupported(&selection), None);
        assert!(
            definitions
                .keys()
                .all(|id| catalog.definitions.contains_key(id))
        );
    }

    #[test]
    fn child_authority_is_an_unselected_addition_and_never_rewrites_the_baseline() {
        let catalog = release_catalog().unwrap();
        let before = catalog.baseline.clone();
        let storage = catalog.world_storage["world.runtime_child_admin_state"];
        assert!(!before.installed_storage.contains(&storage));
        assert!(!before.bindings.iter().any(|binding| {
            additions::CHILD_ADMIN
                .instructions
                .contains(&binding.wire_id.as_str())
        }));
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([74; 32]),
        ));
        let change = catalog
            .activation(
                additions::CHILD_ADMIN.module,
                &before,
                &catalog.definitions,
                network,
                12,
                [4; 32],
            )
            .unwrap();
        let selected = change
            .apply(&before, &catalog.definitions)
            .unwrap()
            .selection;
        assert_eq!(before, catalog.baseline);
        assert_eq!(catalog.unsupported(&selected), None);
        assert!(selected.installed_storage.contains(&storage));
        assert!(
            catalog
                .service_enabled(
                    selected.services_profile,
                    "child_runtime_admin_v1",
                    iroha_crypto::Hash::new(CHILD_ADMIN_SERVICE_CLOSURE_V1).into(),
                )
                .unwrap()
        );
        let world = crate::state::World::default();
        let mut block = world.block();
        let mut tx = block.transaction_without_telemetry(Default::default(), 0);
        let DefinitionDescriptorV1::Storage(descriptor) = &catalog.definitions[&storage] else {
            panic!("child admin storage descriptor");
        };
        // The cell installer authenticates its explicit empty encoding; it cannot manufacture
        // administrators merely because a root selects the new service.
        let initial = descriptor.initial.as_ref().expect("cell initial encoding");
        catalog.cell_installer(&storage).unwrap()(&mut tx, initial).unwrap();
        assert!(tx.runtime_child_admin_state.get().is_none());
    }
}
