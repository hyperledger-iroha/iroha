//! Public-surface checks for deployment-owned daemon providers and publication factories.
use irohad::{
    IrohaRuntimeDeps, IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderCatalogErrorV1,
    IrohaRuntimeProviderRegistryErrorV1, IrohaRuntimeProviderRegistryV1, MainError,
    RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1, ReportResult, RuntimeProviderBrokerBackendRegistryV1,
    RuntimeProviderBrokerBackendsV1, RuntimeProviderBrokerDeploymentV1,
    RuntimeProviderBrokerExecutableArgsV1, RuntimeProviderBrokerExecutableErrorV1,
    RuntimeProviderBrokerExecutableV1, RuntimeProviderBrokerReadinessErrorV1,
    load_runtime_provider_broker_catalog_file_v1,
    musubi_publication_service::{
        MusubiPublicationPrivateDeploymentV1, MusubiPublicationPrivateIngressFutureV1,
        MusubiPublicationPrivateServiceContextV1, MusubiPublicationPrivateServiceFactoryErrorV1,
        MusubiPublicationPrivateServiceFactoryV1, MusubiPublicationPrivateServiceRunnerV1,
    },
    serve_runtime_provider_broker_with_fallible_readiness_v1,
};
use std::sync::Arc;
struct DeploymentRegistry;
struct ExternalMusubiPublicationFactory;
struct ExternalMusubiPublicationRunner;
type CombinedPublicationLauncher = fn(
    &dyn IrohaRuntimeProviderRegistryV1,
    Box<dyn MusubiPublicationPrivateServiceFactoryV1>,
) -> ReportResult<(), MainError>;
impl MusubiPublicationPrivateServiceRunnerV1 for ExternalMusubiPublicationRunner {
    fn serve(
        self: Box<Self>,
        shutdown: iroha_futures::supervisor::ShutdownSignal,
    ) -> MusubiPublicationPrivateIngressFutureV1 {
        Box::pin(async move {
            shutdown.receive().await;
            Ok(())
        })
    }
}
impl MusubiPublicationPrivateServiceFactoryV1 for ExternalMusubiPublicationFactory {
    fn build(
        self: Box<Self>,
        context: MusubiPublicationPrivateServiceContextV1,
    ) -> Result<MusubiPublicationPrivateDeploymentV1, MusubiPublicationPrivateServiceFactoryErrorV1>
    {
        let _network_id = context.network_id();
        let _state = context.state();
        let _queue = context.queue();
        let _sorafs_node = context.sorafs_node();
        Ok(MusubiPublicationPrivateDeploymentV1::new(Box::new(
            ExternalMusubiPublicationRunner,
        )))
    }
}
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
    let launcher: fn(&dyn IrohaRuntimeProviderRegistryV1) -> ReportResult<(), MainError> =
        irohad::run_with_runtime_provider_registry;
    assert_eq!(Arc::strong_count(&registry), 1);
    let _ = launcher;
}
#[test]
fn external_crate_can_implement_factory_and_name_publication_launchers() {
    let standalone_launcher: fn(
        Box<dyn MusubiPublicationPrivateServiceFactoryV1>,
    ) -> ReportResult<(), MainError> = irohad::run_with_musubi_publication;
    let combined_launcher: CombinedPublicationLauncher =
        irohad::run_with_runtime_provider_registry_and_musubi_publication;
    let factory: Box<dyn MusubiPublicationPrivateServiceFactoryV1> =
        Box::new(ExternalMusubiPublicationFactory);
    let _ = standalone_launcher;
    let _ = combined_launcher;
    drop(factory);
}
#[test]
fn external_crate_can_implement_and_name_broker_backend_launcher() {
    let registry: &dyn RuntimeProviderBrokerBackendRegistryV1 = &DeploymentBrokerBackendRegistry;
    let _ = registry;
    let _ = RuntimeProviderBrokerDeploymentV1::try_new;
    let _ = RuntimeProviderBrokerDeploymentV1::serve;
}
#[test]
fn external_crate_can_name_standard_broker_executable_shell() {
    let load: fn(
        &std::path::Path,
    )
        -> Result<IrohaRuntimeProviderBindingsV1, RuntimeProviderBrokerExecutableErrorV1> =
        load_runtime_provider_broker_catalog_file_v1;
    let assemble = RuntimeProviderBrokerExecutableV1::try_from_args;
    let assemble_file = RuntimeProviderBrokerExecutableV1::try_from_catalog_file;
    let serve = RuntimeProviderBrokerExecutableV1::serve::<fn()>;
    let serve_signalled = RuntimeProviderBrokerExecutableV1::serve_until_shutdown_signal::<fn()>;
    let serve_fallible = RuntimeProviderBrokerDeploymentV1::serve_with_fallible_readiness::<
        fn() -> Result<(), RuntimeProviderBrokerReadinessErrorV1>,
    >;
    let serve_fallible_boundary = serve_runtime_provider_broker_with_fallible_readiness_v1::<
        fn() -> Result<(), RuntimeProviderBrokerReadinessErrorV1>,
    >;
    let serve_systemd =
        RuntimeProviderBrokerExecutableV1::serve_until_shutdown_signal_with_systemd_notify;
    let catalog_path = RuntimeProviderBrokerExecutableArgsV1::catalog_path;
    let _ = (
        load,
        assemble,
        assemble_file,
        serve,
        serve_signalled,
        serve_fallible,
        serve_fallible_boundary,
        serve_systemd,
        catalog_path,
    );
}
#[test]
fn external_crate_can_name_standalone_governance_view_projection() {
    let projection: fn(
        &iroha_data_model::ChainId,
        iroha_data_model::NetworkId,
        &iroha_config::parameters::actual::SorafsGovernanceDagServiceView,
    ) -> Result<
        IrohaRuntimeProviderBindingsV1,
        IrohaRuntimeProviderRegistryErrorV1,
    > = IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service_view;
    let _ = projection;
}
#[test]
fn external_crate_can_name_secret_free_broker_catalog_handoff() {
    let export: fn(
        &IrohaRuntimeProviderBindingsV1,
    ) -> Result<Vec<u8>, IrohaRuntimeProviderCatalogErrorV1> =
        IrohaRuntimeProviderBindingsV1::export_canonical_v1;
    let load: fn(
        &[u8],
    )
        -> Result<IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderCatalogErrorV1> =
        IrohaRuntimeProviderBindingsV1::load_canonical_v1;
    assert_eq!(RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1, 256 * 1024);
    let _ = (export, load);
}
#[test]
fn checked_in_binaries_are_explicitly_adapter_disabled() {
    let source = include_str!("../src/bin/iroha3d.rs");
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(compact.contains("irohad::main_entry("));
    assert!(!compact.contains("run_with_runtime_provider_registry"));
    assert!(!compact.contains("run_with_musubi_publication"));
}
#[test]
fn external_crate_can_construct_runtime_dependencies_with_public_builders() {
    let provider = Arc::new(ExternalProofOutcomeSigner::new());
    let dependencies = IrohaRuntimeDeps::default().with_sorafs_proof_outcome_signer(provider);
    assert!(!dependencies.is_empty());
}
