//! Separately identified, dormant generic-carrier kernel and restricted host contracts.
//!
//! These definitions are reviewable catalog data, never an implicit extension of an installed
//! kernel or ABI. The release refuses their selection and omits the host from its handshake
//! until the actual execution/storage owners implement the complete contract.

use super::*;

/// Metering class of interfaces admitted by the restricted generic host.
pub(crate) const METERING_CLASS: &str = "ivm.generic_carriers.v1";
/// Separate physical owner; ordinary native contract storage never contains these values.
pub(crate) const STORAGE_OWNER: &str = iroha_data_model::runtime::carrier::keyed::STORAGE_OWNER;
const KERNEL_NAME: &str = "iroha.kernel.generic_carriers.v1";
const KERNEL_CONTRACT: &[u8] = b"iroha:kernel:generic-carriers:v1:compact-schema-bound-record:strict-byte-key-order:descriptor-ivm:sole-original-external-instructions:public-methods-no-caller-identity:original-signed-fee-and-actual-gas-including-semantic-rejection:no-captured-deferred-reveal-or-contract-relay:capability-storage:existing-corehost-ordered-rollback:keyed-state";

/// Fixed maximum gas of one initial generic instruction invocation.
pub(crate) const MAX_GAS: u64 = 10_000_000;
/// Fixed maximum ordered writes of one initial generic instruction invocation.
pub(crate) const MAX_EFFECTS: u32 = 64;
/// Fixed maximum declared generic storage dependencies per access class.
pub(crate) const MAX_STORAGE_DEPENDENCIES: usize = 16;

/// Exact identities of the dormant contract pair; carrying them grants no activation authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProfileDefinitions {
    /// Kernel which opts into descriptor-selected canonical carrier interpretation.
    pub(crate) kernel: DefinitionId,
    /// Distinct C10-backed ABI V1 restricted host, not the installed C10 ABI V1 host.
    pub(crate) host: DefinitionId,
    /// Installed physical snapshot owner; virtual selected tables own the keyed leaves.
    pub(crate) backing: DefinitionId,
    /// Separate virtual-table keyed commitment, never a widening of the native profile.
    pub(crate) commitment: DefinitionId,
}

impl ReleaseCatalog {
    pub(super) fn compile_generic_carriers(&mut self) -> Result<(), CatalogError> {
        let kernel = self.publish(DefinitionDescriptorV1::Kernel(KernelDescriptorV1 {
            name: KERNEL_NAME.into(),
            reference_closure: iroha_crypto::Hash::new(KERNEL_CONTRACT).into(),
        }))?;
        let host = self.publish(DefinitionDescriptorV1::HostProfile(
            HostProfileDescriptorV1 {
                abi_version: 1,
                abi_hash: ivm::IvmProfileId::C10_GENERIC_CARRIER.abi_hash(),
                metering: ivm::gas::schedule_hash_for(ivm::IvmEngine::C10).into(),
            },
        ))?;
        let field = WORLD_FIELDS
            .iter()
            .find(|field| field.id == STORAGE_OWNER)
            .ok_or(CatalogError::UnresolvedSchema(STORAGE_OWNER))?;
        let physical =
            world_storage(field)?.ok_or(CatalogError::UnresolvedSchema(STORAGE_OWNER))?;
        let backing = iroha_data_model::runtime::carrier::keyed::backing_descriptor();
        let DefinitionDescriptorV1::Storage(expected) = &backing else {
            unreachable!()
        };
        if physical.namespace != expected.namespace
            || physical.shape != expected.shape
            || physical.value != expected.value
            || physical.projection != expected.projection
        {
            return Err(CatalogError::UnresolvedSchema(STORAGE_OWNER));
        }
        let backing = self.publish(backing)?;
        let commitment =
            self.publish(iroha_data_model::runtime::carrier::keyed::profile_descriptor())?;
        self.world_storage.insert(STORAGE_OWNER, backing);
        self.generic_carriers = Some(ProfileDefinitions {
            kernel,
            host,
            backing,
            commitment,
        });
        Ok(())
    }

    /// Whether this release has the execution owner for an exact catalog definition.
    pub(crate) fn executes_definition(&self, id: &DefinitionId) -> bool {
        // TODO(N14): remove this refusal only with descriptor-selected IVM execution, the
        // original CoreHost effects/rollback path, scoped generic storage and keyed commitment.
        // Presence in the review catalog alone must not produce a support attestation.
        self.definitions.contains_key(id)
            && !self.additions.iter().any(|addition| {
                addition.module == additions::GENERIC_CARRIER_ARTIFACTS.module
                    && (addition.implementation == *id
                        || addition
                            .interfaces
                            .values()
                            .any(|candidate| candidate == id)
                        || addition.storage.values().any(|candidate| candidate == id))
            })
            && !self.generic_carriers.is_some_and(|profile| {
                *id == profile.kernel
                    || *id == profile.host
                    || *id == profile.backing
                    || *id == profile.commitment
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent profile family; it is not a native instruction addition.
    const PROFILE_NAME: &str = "iroha.runtime.generic_carriers.v1";

    #[test]
    fn artifact_staging_is_an_independent_real_addition_outside_the_baseline() {
        let catalog = release_catalog().unwrap();
        let addition = catalog
            .additions
            .iter()
            .find(|addition| addition.module == additions::GENERIC_CARRIER_ARTIFACTS.module)
            .unwrap();
        let wire = additions::GENERIC_CARRIER_ARTIFACTS.instructions[0];
        let storage = addition.storage["world.runtime_generic_artifacts"];
        assert!(!catalog.baseline.installed_storage.contains(&storage));
        assert!(
            !catalog
                .baseline
                .bindings
                .iter()
                .any(|binding| binding.wire_id == wire)
        );
        assert_eq!(
            catalog.native_module(&addition.implementation),
            Some(addition.module)
        );
        assert!(
            addition
                .definitions()
                .iter()
                .all(|id| !catalog.executes_definition(id))
        );
        let DefinitionDescriptorV1::Implementation(implementation) =
            &catalog.definitions[&addition.implementation]
        else {
            panic!("native staging implementation");
        };
        assert!(implementation.writes.contains(&storage));
        assert_eq!(
            implementation.entrypoint,
            EntrypointV1::Instructions(vec![addition.interfaces[wire]])
        );
        // A stage-only successor is a real governed change, and leaves the current
        // kernel, host profiles and existing storage identities untouched.
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"carrier-staging-catalog-test"),
        ));
        let published: BTreeMap<_, _> = catalog
            .baseline_definitions
            .iter()
            .map(|id| (*id, catalog.definitions[id].clone()))
            .collect();
        let change = catalog
            .activation(
                addition.module,
                &catalog.baseline,
                &published,
                network,
                10,
                [7; 32],
            )
            .unwrap();
        let applied = change.apply(&catalog.baseline, &published).unwrap();
        assert_eq!(applied.selection.kernel, catalog.baseline.kernel);
        assert_eq!(
            applied.selection.commitment_profile,
            catalog.baseline.commitment_profile
        );
        assert!(applied.selection.installed_storage.contains(&storage));
        assert!(catalog.unsupported(&applied.selection).is_some());
        for (id, definition) in published {
            assert_eq!(catalog.definitions[&id], definition);
        }
    }

    #[test]
    fn dormant_contracts_never_change_or_advertise_the_installed_baseline() {
        let catalog = release_catalog().unwrap();
        let profile = catalog.generic_carriers.unwrap();
        assert_ne!(profile.kernel, catalog.baseline.kernel);
        assert!(!catalog.baseline_definitions.contains(&profile.kernel));
        assert!(!catalog.baseline_definitions.contains(&profile.host));
        assert!(!catalog.baseline_definitions.contains(&profile.backing));
        assert!(!catalog.baseline_definitions.contains(&profile.commitment));
        assert!(
            !catalog
                .baseline
                .installed_storage
                .contains(&profile.backing)
        );
        assert_eq!(catalog.world_storage[STORAGE_OWNER], profile.backing);
        assert!(!catalog.baseline.active.contains(&profile.host));
        assert_eq!(catalog.unsupported(&catalog.baseline), None);
        let advertised = crate::runtime::attestation::release_host_profiles(catalog);
        assert!(!advertised.contains(&profile.host));
        let mut selected = catalog.baseline.clone();
        selected.kernel = profile.kernel;
        assert_eq!(catalog.unsupported(&selected), Some(profile.kernel));
        selected = catalog.baseline.clone();
        selected.active.push(profile.host);
        selected.active.sort();
        assert_eq!(catalog.unsupported(&selected), Some(profile.host));
        selected = catalog.baseline.clone();
        selected.installed_storage.push(profile.backing);
        selected.installed_storage.sort();
        assert_eq!(catalog.unsupported(&selected), Some(profile.backing));
        selected = catalog.baseline.clone();
        selected.commitment_profile = profile.commitment;
        assert_eq!(catalog.unsupported(&selected), Some(profile.commitment));
        assert!(!catalog.executes_definition(&DefinitionId([0; 32])));
        assert!(
            !catalog
                .additions
                .iter()
                .any(|addition| addition.module == PROFILE_NAME)
        );
    }

    #[test]
    fn carrier_host_is_a_new_abi1_contract_with_exact_c10_metering() {
        let catalog = release_catalog().unwrap();
        let profile = catalog.generic_carriers.unwrap();
        let DefinitionDescriptorV1::HostProfile(host) = &catalog.definitions[&profile.host] else {
            panic!("host descriptor");
        };
        assert_eq!(host.abi_version, 1);
        assert_ne!(host.abi_hash, BASELINE_HOST_PROFILE_ABI_HASH);
        assert_ne!(host.abi_hash, ivm::IvmProfileId::C10_ABI_V1.abi_hash());
        assert_eq!(
            host.metering,
            <[u8; 32]>::from(ivm::gas::schedule_hash_for(ivm::IvmEngine::C10))
        );
        assert_eq!(
            catalog.definitions[&profile.host].id().unwrap(),
            profile.host
        );
        assert_eq!(
            catalog.definitions[&profile.kernel].id().unwrap(),
            profile.kernel
        );
    }
}
