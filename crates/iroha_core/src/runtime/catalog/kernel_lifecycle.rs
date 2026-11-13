//! The kernel's lifecycle addition (`iroha.runtime.lifecycle.v1`) composed over each
//! predecessor family this release compiles (`specs/runtime_upgrades.md`, "Release additions").
//!
//! The addition's tables and instructions are the same on every root, but an implementation
//! commits its services profile and the closure it may touch, and an interface commits the
//! events it may emit. A root that has moved off the baseline (Time, Ironwood, or Ironwood with
//! Time, as Taira does before it ratifies its adoption anchor) selects another services profile,
//! other events and other interfaces, so the baseline composition cannot activate there
//! (`SelectionError::ForeignServices`). The catalog therefore compiles one composition per
//! family: the addition's own storage and wire IDs, with the family's services profile, the
//! events of its native bundle and that bundle's declared closure. The baseline family's
//! composition is the addition as declared, so its identities do not move.
//!
//! A composition is chosen by the services profile of the selection it activates on; a
//! selection whose profile no compiled family uses (for example one that selected the SoraFS
//! authority services) has none, and drafting its activation fails closed. Activating the
//! addition first and Time afterwards reaches the Time family's composition, because the Time
//! successor changes only the services profile of the addition's implementation.

use std::collections::BTreeMap;

use iroha_data_model::runtime::definition::{
    DefinitionDescriptorV1, DefinitionId, EntrypointV1, RootSelectionV1,
};

use super::{
    AdditionDefinitions, CatalogError, ReleaseCatalog, additions::RUNTIME_LIFECYCLE, sorted,
};

/// The baseline wire ID whose binding names a family's native bundle. No independent addition
/// rebinds it, and every family binds it to its main bundle.
const BUNDLE_WIRE_ID: &str = "iroha.set_parameter";

fn failure(reason: impl Into<String>) -> CatalogError {
    CatalogError::Addition {
        module: RUNTIME_LIFECYCLE.module.into(),
        reason: reason.into(),
    }
}

impl ReleaseCatalog {
    /// The same lifecycle State can activate after the consent service as before it.
    pub(super) fn compile_runtime_lifecycle_after_deferred(&mut self) -> Result<(), CatalogError> {
        if self.deferred_consent_compositions.is_empty() {
            return Ok(());
        }
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == RUNTIME_LIFECYCLE.module)
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
        let network = iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0; 32])),
        );
        for family in families {
            let change =
                self.draft_deferred_consent(&family, &self.definitions, network, 1, [0; 32])?;
            let successor = change
                .apply(&family, &self.definitions)
                .map_err(|error| failure(error.to_string()))?
                .selection;
            let composition = self.compose_runtime_lifecycle(&base, &successor)?;
            self.runtime_lifecycle_compositions
                .insert(successor.services_profile, composition);
        }
        Ok(())
    }

    /// Compile the kernel lifecycle addition over the baseline, Time, Ironwood and Ironwood
    /// with Time families. Publishing a composition activates nothing.
    ///
    /// # Errors
    /// A family lacks its native bundle, or a composed descriptor cannot be identified.
    pub(super) fn compile_runtime_lifecycle_compositions(&mut self) -> Result<(), CatalogError> {
        let Some(base) = self
            .additions
            .iter()
            .find(|addition| addition.module == RUNTIME_LIFECYCLE.module)
            .cloned()
        else {
            return Ok(());
        };
        let mut families = vec![self.baseline.clone(), self.lifecycle.clone()];
        if let Some(ironwood) = &self.ironwood {
            let selection = ironwood.selection.clone();
            families.push(self.lifecycle_successor(&selection)?);
            families.push(selection);
        }
        for family in families {
            let composition = self.compose_runtime_lifecycle(&base, &family)?;
            if family.services_profile == self.baseline.services_profile && composition != base {
                return Err(failure(
                    "the baseline family's composition must be the declared addition",
                ));
            }
            self.runtime_lifecycle_compositions
                .insert(family.services_profile, composition);
        }
        Ok(())
    }

    /// The addition's storage and wire IDs with the services profile of `family`, the events of
    /// its native bundle and that bundle's declared closure, besides the addition's own tables
    /// and interfaces.
    pub(super) fn compose_runtime_lifecycle(
        &mut self,
        base: &AdditionDefinitions,
        family: &RootSelectionV1,
    ) -> Result<AdditionDefinitions, CatalogError> {
        let bundle = family
            .bindings
            .iter()
            .find(|binding| binding.wire_id == BUNDLE_WIRE_ID)
            .ok_or_else(|| failure("the family binds no native bundle"))?
            .implementation;
        let Some(DefinitionDescriptorV1::Implementation(bundle)) =
            self.definitions.get(&bundle).cloned()
        else {
            return Err(failure("the family's native bundle is not compiled"));
        };
        let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
            self.definitions.get(&base.implementation).cloned()
        else {
            return Err(failure("the declared implementation is not compiled"));
        };
        let mut events = bundle.events.clone();
        events.sort_unstable();
        let mut interfaces = BTreeMap::new();
        for (wire_id, id) in &base.interfaces {
            let Some(DefinitionDescriptorV1::Interface(mut interface)) =
                self.definitions.get(id).cloned()
            else {
                return Err(failure(format!("the {wire_id} interface is not compiled")));
            };
            interface.events.clone_from(&events);
            interfaces.insert(
                *wire_id,
                self.publish(DefinitionDescriptorV1::Interface(interface))?,
            );
        }
        let own: Vec<DefinitionId> = base.storage.values().copied().collect();
        let with_own =
            |declared: &[DefinitionId]| sorted(declared.iter().chain(&own).copied().collect());
        implementation.entrypoint =
            EntrypointV1::Instructions(sorted(interfaces.values().copied().collect()));
        implementation.reads = with_own(&bundle.reads);
        implementation.writes = with_own(&bundle.writes);
        implementation.scans = with_own(&bundle.scans);
        implementation.events = events;
        implementation.permissions.clone_from(&bundle.permissions);
        implementation.parameters.clone_from(&bundle.parameters);
        implementation.invokes = sorted(
            bundle
                .invokes
                .iter()
                .chain(interfaces.values())
                .copied()
                .collect(),
        );
        implementation.services_profile = family.services_profile;
        let implementation =
            self.publish(DefinitionDescriptorV1::Implementation(implementation))?;
        Ok(AdditionDefinitions {
            module: base.module,
            implementation,
            interfaces,
            rebinds: BTreeMap::new(),
            storage: base.storage.clone(),
            kernel: None,
        })
    }

    /// The composition of the kernel lifecycle addition that activates on `current`: the one
    /// compiled for its services profile.
    ///
    /// # Errors
    /// No compiled family uses the services profile `current` selects.
    pub(super) fn runtime_lifecycle_for(
        &self,
        current: &RootSelectionV1,
    ) -> Result<AdditionDefinitions, CatalogError> {
        self.runtime_lifecycle_compositions
            .get(&current.services_profile)
            .cloned()
            .ok_or_else(|| {
                failure(
                    "no compiled composition uses the services profile this root selects \
                     (baseline, Time, Ironwood and Ironwood with Time are compiled)",
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{NetworkId, runtime::definition::SelectionError};

    use super::*;
    use crate::runtime::release_catalog;

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([0x31; 32])))
    }

    /// Every definition of `catalog` that `selection` relies on, as a root that executes it has
    /// published them.
    fn published_for(
        catalog: &ReleaseCatalog,
        selection: &RootSelectionV1,
    ) -> BTreeMap<DefinitionId, DefinitionDescriptorV1> {
        let mut published: BTreeMap<_, _> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        for id in [
            selection.kernel,
            selection.commitment_profile,
            selection.services_profile,
        ]
        .into_iter()
        .chain(selection.installed_storage.iter().copied())
        .chain(selection.active.iter().copied())
        {
            published.insert(id, catalog.definitions[&id].clone());
        }
        published
    }

    #[test]
    fn the_lifecycle_state_activates_over_every_compiled_family() {
        let catalog = release_catalog().expect("release catalog");
        let mut composed = catalog.clone();
        let mut families = vec![
            ("baseline", catalog.baseline.clone()),
            ("time", catalog.lifecycle.clone()),
        ];
        let ironwood = catalog
            .ironwood
            .as_ref()
            .expect("Ironwood")
            .selection
            .clone();
        families.push((
            "ironwood with time",
            composed
                .lifecycle_successor(&ironwood)
                .expect("Time over Ironwood"),
        ));
        families.push(("ironwood", ironwood));
        let mut implementations = std::collections::BTreeSet::new();
        for (name, family) in families {
            let published = published_for(&composed, &family);
            let change = catalog
                .activation(
                    RUNTIME_LIFECYCLE.module,
                    &family,
                    &published,
                    network(),
                    50,
                    [9; 32],
                )
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let successor = change
                .apply(&family, &published)
                .unwrap_or_else(|error| panic!("{name}: {error}"))
                .selection;
            assert_eq!(
                catalog.unsupported(&successor),
                None,
                "{name}: the release executes the successor"
            );
            assert!(
                crate::runtime::migration::lifecycle_installed(&successor),
                "{name}: the successor installs the lifecycle State"
            );
            let composition = catalog.runtime_lifecycle_for(&family).expect(name);
            assert!(implementations.insert(composition.implementation), "{name}");
            assert_eq!(
                catalog.native_module(&composition.implementation),
                Some(RUNTIME_LIFECYCLE.module)
            );
        }
    }

    #[test]
    fn the_declared_addition_is_the_baseline_composition_and_needs_its_services() {
        let catalog = release_catalog().expect("release catalog");
        let declared = catalog
            .additions
            .iter()
            .find(|addition| addition.module == RUNTIME_LIFECYCLE.module)
            .expect("declared");
        assert_eq!(
            catalog.runtime_lifecycle_for(&catalog.baseline).as_ref(),
            Ok(declared)
        );
        // The declared implementation cannot join a root whose services profile is Time's.
        let mut foreign = catalog.lifecycle.clone();
        foreign.active.push(declared.implementation);
        foreign.active.sort_unstable();
        assert_eq!(
            foreign.check_closure(&catalog.definitions),
            Err(SelectionError::ForeignServices(declared.implementation))
        );
        // A root whose services profile no family uses has no composition.
        let mut unknown = catalog.baseline.clone();
        unknown.services_profile = DefinitionId([0xee; 32]);
        assert!(catalog.runtime_lifecycle_for(&unknown).is_err());
    }
}
