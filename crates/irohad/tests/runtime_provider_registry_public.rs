//! Public-surface checks for deployment-owned daemon provider registries.

use std::sync::Arc;

use irohad::{
    BuildLine, IrohaRuntimeDeps, IrohaRuntimeProviderBindingsV1,
    IrohaRuntimeProviderRegistryErrorV1, IrohaRuntimeProviderRegistryV1, MainError, ReportResult,
    RuntimeProviderBrokerBackendRegistryV1, RuntimeProviderBrokerBackendsV1,
    RuntimeProviderBrokerDeploymentV1,
};

struct DeploymentRegistry;

struct DeploymentBrokerBackendRegistry;

impl RuntimeProviderBrokerBackendRegistryV1 for DeploymentBrokerBackendRegistry {
    fn resolve(
        &self,
        _bindings: &IrohaRuntimeProviderBindingsV1,
    ) -> Result<RuntimeProviderBrokerBackendsV1, IrohaRuntimeProviderRegistryErrorV1> {
        Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
    }
}

impl IrohaRuntimeProviderRegistryV1 for DeploymentRegistry {
    fn resolve(
        &self,
        bindings: &IrohaRuntimeProviderBindingsV1,
    ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
        for binding in bindings.iter() {
            if let Some(native) = binding.native_signer_binding() {
                assert_eq!(binding.handle(), native.handle());
                assert_eq!(
                    binding.native_signer_algorithm(),
                    native.public_key().try_algorithm().ok()
                );
            }
        }
        Ok(IrohaRuntimeDeps::default())
    }
}

struct ExternalProofOutcomeSigner {
    public_key: iroha_crypto::PublicKey,
}

impl ExternalProofOutcomeSigner {
    const HANDLE: &'static str = "hsm://external-launcher/proof-outcome/primary";
    const QUALIFICATION: iroha_torii::SorafsNativeTransactionSignerQualificationV1 =
        iroha_torii::SorafsNativeTransactionSignerQualificationV1::new(1, [0x41; 32]);

    fn new() -> Self {
        let keypair =
            iroha_crypto::KeyPair::try_from_seed(vec![0x41; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("derive external runtime-signer fixture");
        Self {
            public_key: keypair.public_key().clone(),
        }
    }

    fn account_id(&self) -> iroha_data_model::account::AccountId {
        iroha_data_model::account::AccountId::new(self.public_key.clone())
    }
}

impl iroha_torii::SorafsNativeTransactionSignerProviderV1 for ExternalProofOutcomeSigner {
    fn role(&self) -> iroha_torii::SorafsNativeTransactionSignerRoleV1 {
        iroha_torii::SorafsNativeTransactionSignerRoleV1::ProofOutcome
    }

    fn handle(&self) -> &str {
        Self::HANDLE
    }

    fn authority(&self) -> iroha_data_model::account::AccountId {
        self.account_id()
    }

    fn public_key(
        &self,
    ) -> Result<iroha_crypto::PublicKey, iroha_torii::SorafsNativeTransactionSignerProbeErrorV1>
    {
        Ok(self.public_key.clone())
    }

    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::SorafsNativeTransactionSignerQualificationV1,
        iroha_torii::SorafsNativeTransactionSignerProbeErrorV1,
    > {
        Ok(Self::QUALIFICATION)
    }
}

impl iroha_torii::SoraFsProofOutcomeTransactionSigner for ExternalProofOutcomeSigner {
    fn sign(
        &self,
        _payload: iroha_data_model::transaction::TransactionPayload,
    ) -> Result<
        iroha_data_model::transaction::SignedTransaction,
        iroha_torii::SoraFsProofOutcomeSigningError,
    > {
        Err(iroha_torii::SoraFsProofOutcomeSigningError::Refused)
    }
}

#[test]
fn external_crate_can_implement_registry_and_name_standard_launcher() {
    let registry: Arc<dyn IrohaRuntimeProviderRegistryV1> = Arc::new(DeploymentRegistry);
    let launcher: fn(
        BuildLine,
        &dyn IrohaRuntimeProviderRegistryV1,
    ) -> ReportResult<(), MainError> = irohad::run_with_runtime_provider_registry;

    assert_eq!(Arc::strong_count(&registry), 1);
    let _ = launcher;
}

#[test]
fn external_crate_can_implement_and_name_broker_backend_launcher() {
    let registry: &dyn RuntimeProviderBrokerBackendRegistryV1 = &DeploymentBrokerBackendRegistry;
    let _ = registry;
    let _ = RuntimeProviderBrokerDeploymentV1::try_new;
    let _ = RuntimeProviderBrokerDeploymentV1::serve;
}

#[test]
fn external_crate_can_name_standalone_governance_view_projection() {
    let projection: fn(
        &iroha_data_model::ChainId,
        &iroha_config::parameters::actual::SorafsGovernanceDagServiceView,
    ) -> Result<
        IrohaRuntimeProviderBindingsV1,
        IrohaRuntimeProviderRegistryErrorV1,
    > = IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service_view;

    let _ = projection;
}

#[test]
fn checked_in_binaries_are_explicitly_adapter_disabled() {
    let source = include_str!("../src/bin/irohad.rs");
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(compact.contains("irohad::main_entry("));
    assert!(!compact.contains("run_with_runtime_provider_registry"));
}

#[test]
fn external_crate_can_construct_runtime_dependencies_with_public_builders() {
    let provider = Arc::new(ExternalProofOutcomeSigner::new());
    let dependencies = IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(provider);

    assert!(!dependencies.is_empty());
}
