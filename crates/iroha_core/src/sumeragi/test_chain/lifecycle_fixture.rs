//! Lifecycle consumer fixtures, excluded from production builds.
//!
//! This seeds a selected runtime through the existing World setup boundary. The
//! following blocks use real execution and certificates; the setup itself does
//! not qualify governed activation or a multi-peer rollout.

use super::CertifiedTestChain;
use crate::state::StorageReadOnly;

impl CertifiedTestChain {
    /// Select the compact lifecycle runtime before the next fixture block.
    ///
    /// This test setup is not a governed activation transaction.
    pub fn select_lifecycle_for_testing(&self, time_ms: u64) {
        let catalog = crate::runtime::release_catalog().expect("release catalog");
        self.select_runtime_for_testing(time_ms, &catalog.lifecycle);
    }

    /// Install an exact compiled selection through the existing fixture setup boundary.
    ///
    /// This is not governance qualification. Subsequent blocks still execute and certify
    /// normally. Only the selection's actual definition closure and its required Ironwood
    /// migration witness are published, so an unused compiled successor cannot masquerade
    /// as an already-published predecessor.
    pub fn select_runtime_for_testing(
        &self,
        time_ms: u64,
        selection: &iroha_data_model::runtime::definition::RootSelectionV1,
    ) {
        let catalog = crate::runtime::release_catalog().expect("release catalog");
        selection
            .check_closure(&catalog.definitions)
            .expect("closed fixture selection");
        assert!(
            catalog.unsupported(selection).is_none(),
            "compiled fixture selection"
        );
        let mut required: std::collections::BTreeSet<_> = [
            selection.kernel,
            selection.commitment_profile,
            selection.services_profile,
        ]
        .into_iter()
        .chain(selection.installed_storage.iter().copied())
        .chain(selection.active.iter().copied())
        .chain(
            selection
                .bindings
                .iter()
                .flat_map(|binding| [binding.interface, binding.implementation]),
        )
        .collect();
        if crate::runtime::ironwood::enabled(selection) {
            // The first twelve-owner cutover requires publication of this exact
            // descriptor even though migrations are not active selection members.
            required.insert(
                catalog
                    .ironwood
                    .as_ref()
                    .expect("compiled Ironwood catalog")
                    .migration,
            );
        }
        self.setup_world_at(time_ms, |tx| {
            for id in required {
                if tx.world.runtime_definitions.get(&id).is_none() {
                    tx.world
                        .runtime_definitions
                        .insert(id, catalog.definitions[&id].clone());
                }
            }
            *tx.world.runtime_selection.get_mut() = selection.clone();
        });
    }

    /// Restore the predecessor selection through fixture setup, not governance.
    ///
    /// Consumer tests use this to prove refusal of a later certified World cut
    /// with a different runtime selection. It does not qualify live rollback.
    pub fn select_lifecycle_predecessor_for_testing(&self, time_ms: u64) {
        let catalog = crate::runtime::release_catalog().expect("release catalog");
        self.setup_world_at(time_ms, |tx| {
            *tx.world.runtime_selection.get_mut() = catalog.baseline.clone();
        });
    }
}

impl CertifiedTestChain {
    /// Install a compiled, signed contract artifact for lifecycle consumer fixtures.
    ///
    /// Publication and its Run permission use the existing test setup boundary;
    /// contract calls still execute through the native guest host and executor.
    pub fn install_lifecycle_contract_for_testing(
        &self,
        time_ms: u64,
        authority_key: &iroha_crypto::KeyPair,
        nonce: u64,
        program: Vec<u8>,
        manifest: iroha_data_model::smart_contract::manifest::ContractManifest,
    ) -> iroha_data_model::smart_contract::ContractAddress {
        use iroha_data_model::{
            account::AccountId,
            smart_contract::{ContractAddress, ContractArtifactId},
        };
        use iroha_model_base::topology::DataSpaceId;
        let authority = AccountId::new(authority_key.public_key().clone());
        let code_hash = ivm::contract_code_hash(&program);
        let address = ContractAddress::derive(
            &self.network_id(),
            &authority,
            nonce,
            DataSpaceId::UNIVERSAL,
        )
        .expect("fixture contract address");
        let signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
        let manifest = manifest
            .try_signed(signing.context(), signing.max_frame_bytes(), authority_key)
            .expect("sign fixture manifest");
        self.setup_world_at(time_ms, |tx| {
            let artifact = ContractArtifactId::new(DataSpaceId::UNIVERSAL, code_hash);
            tx.world.contract_code.insert(artifact, program);
            tx.world.contract_manifests.insert(artifact, manifest);
            tx.world
                .bind_active_contract_subject_for_testing(address.clone(), code_hash);
            let mut permissions = tx
                .world
                .account_permissions
                .get(&authority)
                .cloned()
                .unwrap_or_default();
            permissions.insert(
                iroha_executor_data_model::permission::smart_contract::CanUseContractPermission {
                    contract: address.clone(),
                    permission: "Run".parse().expect("fixture permission"),
                }
                .into(),
            );
            tx.world.account_permissions.insert(authority, permissions);
        });
        address
    }
}
