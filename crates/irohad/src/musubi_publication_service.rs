//! Supervised deployment boundary for the private Musubi publication service.
//!
//! An explicit installation selects original signed-genesis transport intent and ordinary
//! account signers after State exists. Startup reopens the original publication journal,
//! seed custody, native pin session and clock before binding its dedicated private TLS listener.
//! Standard profiles without installation remain inert; ambiguous injected/configured factories
//! refuse. Fresh native finality and account-read verification remain mandatory for every
//! provider observation, independently of TLS intent or listener construction.
mod finality;
mod local_custody;
mod local_factory;
mod native_pin;
mod native_storage;
pub(crate) mod stock_installation;
pub use native_storage::{NativeMusubiStorageBuilderV1, NativeMusubiStorageLimitsV1};
mod pin_registration;
mod pin_signer;
mod private_tls_ingress;
mod provider_readback;
mod shared_seed_staging;
mod storage_coordination;
pub use finality::{
    MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    MusubiPublicationFinalizedArchiveRegistrationReadErrorV1,
    MusubiPublicationFinalizedArchiveRegistrationReaderV1,
};
pub use local_custody::{
    MusubiPublicationPrivateLocalCustodyErrorV1, MusubiPublicationPrivateLocalCustodyV1,
};
pub use local_factory::{
    MusubiPublicationPrivateIngressBuilderV1, MusubiPublicationPrivateLocalFactorySettingsV1,
    MusubiPublicationPrivateLocalFactoryV1, MusubiPublicationPrivateStorageBuilderV1,
};
pub use native_pin::{
    NativeMusubiFinalizedPinV1, NativeMusubiPinCheckRefusalV1, NativeMusubiPinCoordinatorV1,
    NativeMusubiPinOriginalV1, NativeMusubiPinPhaseV1, NativeMusubiPinProgressV1,
    NativePinAuthorizationV1,
};
pub use pin_registration::{
    MusubiPublicationFinalizedPinRegistrationQueryV1,
    MusubiPublicationFinalizedPinRegistrationReadErrorV1,
    MusubiPublicationFinalizedPinRegistrationReaderV1,
};
pub use pin_signer::{MusubiPublicationPinSigningErrorV1, MusubiPublicationPinTransactionSignerV1};
pub use private_tls_ingress::{
    MusubiPublicationPrivateTlsIngressBuilderV1, MusubiPublicationPrivateTlsSettingsV1,
};
pub use provider_readback::MusubiPublicationAuthenticatedProviderReadbackV1;
pub use shared_seed_staging::{
    MusubiPublicationFinalizedSeedReadCapabilityV1, MusubiPublicationFinalizedSeedReadErrorV1,
    MusubiPublicationFinalizedSeedReadLeaseV1,
};
// TODO: Qualify the complete generated multi-provider publication, independent replica
// readback, crash/resource behavior and supported platforms end to end. The stock assembly
// and genuine component controls do not establish release readiness. Never substitute cached
// advertisements, publisher claims or result-only genesis for current certified native state.
use iroha_core::{queue::Queue, state::State};
use iroha_data_model::NetworkId;
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
/// Live daemon-owned dependencies made available only after trusted startup replay.
///
/// The context carries handles rather than snapshots so a long-running publication backend can
/// observe later finalized blocks and submit its signed registry transactions through the same
/// queue as Torii. The mandatory genesis-derived network identity remains available even while a
/// fresh node is waiting for Sumeragi to commit its staged genesis.
pub struct MusubiPublicationPrivateServiceContextV1 {
    network_id: NetworkId,
    state: Arc<State>,
    queue: Arc<Queue>,
    sorafs_node: sorafs_node::NodeHandle,
    provider_attestations: Option<Arc<dyn sorafs_node::MusubiProviderAttestationInventoryReaderV1>>,
}
impl core::fmt::Debug for MusubiPublicationPrivateServiceContextV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MusubiPublicationPrivateServiceContextV1")
            .field("network_id", &self.network_id)
            .finish_non_exhaustive()
    }
}
impl MusubiPublicationPrivateServiceContextV1 {
    /// Capture the exact handles owned by a successfully assembled daemon.
    pub(crate) fn new(
        network_id: NetworkId,
        state: Arc<State>,
        queue: Arc<Queue>,
        sorafs_node: sorafs_node::NodeHandle,
    ) -> Self {
        Self {
            network_id,
            state,
            queue,
            sorafs_node,
            provider_attestations: None,
        }
    }
    pub(crate) fn with_native_provider_attestation_inventory(
        mut self,
        inventory: Option<Arc<dyn sorafs_node::MusubiProviderAttestationInventoryReaderV1>>,
    ) -> Self {
        self.provider_attestations = inventory;
        self
    }
    /// Borrow the exact native provider's locally retained signed-attestation inventory.
    ///
    /// This is the same owner used by the supervised capture journal. It grants read access only;
    /// it proves neither native registry inclusion nor current eligibility. The existing publication
    /// coordinator must retain its original manager-signed Register and verify native readback.
    pub fn provider_attestation_inventory(
        &self,
    ) -> Option<Arc<dyn sorafs_node::MusubiProviderAttestationInventoryReaderV1>> {
        self.provider_attestations.clone()
    }
    /// Exact genesis-derived network identity already validated by daemon startup.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Clone the live finalized-state handle.
    #[must_use]
    pub fn state(&self) -> Arc<State> {
        Arc::clone(&self.state)
    }
    /// Bind a read-only finalized archive-registration reader to these exact daemon handles.
    #[must_use]
    pub fn finalized_archive_registration_reader(
        &self,
    ) -> MusubiPublicationFinalizedArchiveRegistrationReaderV1 {
        MusubiPublicationFinalizedArchiveRegistrationReaderV1::from_validated_context(
            self.network_id,
            Arc::clone(&self.state),
        )
    }
    /// Clone the node's transaction-admission queue handle.
    #[must_use]
    pub fn queue(&self) -> Arc<Queue> {
        Arc::clone(&self.queue)
    }
    /// Clone the embedded `SoraFS` node handle.
    #[must_use]
    pub fn sorafs_node(&self) -> sorafs_node::NodeHandle {
        self.sorafs_node.clone()
    }
}
/// Redacted failure while assembling an injected private publication deployment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationPrivateServiceFactoryErrorV1 {
    /// A deployment-owned signer, journal, listener, or backend is unavailable.
    Unavailable,
    /// A supplied dependency failed its deployment qualification or identity binding.
    Unqualified,
}
impl core::fmt::Display for MusubiPublicationPrivateServiceFactoryErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "private Musubi publication factory is unavailable",
            Self::Unqualified => "private Musubi publication factory is unqualified",
        })
    }
}
impl std::error::Error for MusubiPublicationPrivateServiceFactoryErrorV1 {}
/// One-shot deployment-owned factory invoked after daemon handles are ready.
///
/// The factory may assemble the private HTTPS runner and its protocol core from the exact live
/// daemon handles. It must keep credentials and signing material inside deployment-owned adapters;
/// neither the context nor the returned error may contain secrets.
pub trait MusubiPublicationPrivateServiceFactoryV1: Send + 'static {
    /// Build one qualified private deployment from the exact daemon context.
    ///
    /// # Errors
    ///
    /// Returns a redacted failure before any publication child is added to the supervisor.
    fn build(
        self: Box<Self>,
        context: MusubiPublicationPrivateServiceContextV1,
    ) -> Result<MusubiPublicationPrivateDeploymentV1, MusubiPublicationPrivateServiceFactoryErrorV1>;
}
/// Redacted terminal failure from a deployment-owned private HTTPS ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationPrivateIngressErrorV1 {
    /// The listener, TLS identity, durable journal, signer, or backend became unavailable.
    Unavailable,
    /// The injected ingress failed its own deployment qualification or identity checks.
    Unqualified,
}
/// Boxed lifetime-independent ingress future accepted by the daemon supervisor.
pub type MusubiPublicationPrivateIngressFutureV1 = Pin<
    Box<dyn Future<Output = Result<(), MusubiPublicationPrivateIngressErrorV1>> + Send + 'static>,
>;
/// Deployment-owned runner for the three fixed private publication routes.
pub trait MusubiPublicationPrivateServiceRunnerV1: Send + 'static {
    /// Serve until shutdown while forwarding bounded requests to the publication service core.
    ///
    /// Implementations must enforce TLS, reject duplicate security-sensitive headers, bound the
    /// body before allocation, accept only the three exact route paths, and pass the
    /// exact uppercase method plus path/header/body values to
    /// `iroha_musubi_service::MusubiPublicationPrivateServiceV1`.
    /// The runner owns that core together with its injected durable journal, signer, and
    /// `SoraFS` backends; `irohad` never receives those secrets or dependency objects.
    fn serve(self: Box<Self>, shutdown: ShutdownSignal) -> MusubiPublicationPrivateIngressFutureV1;
}
/// Complete private-service deployment assembled from explicit runtime installation or injection.
pub struct MusubiPublicationPrivateDeploymentV1 {
    runner: Box<dyn MusubiPublicationPrivateServiceRunnerV1>,
}
impl core::fmt::Debug for MusubiPublicationPrivateDeploymentV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MusubiPublicationPrivateDeploymentV1")
            .finish_non_exhaustive()
    }
}
impl MusubiPublicationPrivateDeploymentV1 {
    /// Assemble a deployment from an already qualified core and private HTTPS ingress.
    #[must_use]
    pub fn new(runner: Box<dyn MusubiPublicationPrivateServiceRunnerV1>) -> Self {
        Self { runner }
    }
    /// Start the private service as one supervisor child.
    ///
    /// A return before shutdown, including a nominal `Ok(())`, is fatal and deliberately causes
    /// the parent supervisor to stop the deployment rather than silently losing publication.
    #[must_use]
    pub fn start(self, shutdown: ShutdownSignal) -> Child {
        let ingress_shutdown = shutdown.clone();
        let task = tokio::spawn(async move {
            let result = self.runner.serve(ingress_shutdown).await;
            if !shutdown.is_sent() {
                match result {
                    Ok(()) => panic!(
                        "private Musubi publication ingress exited before supervisor shutdown"
                    ),
                    Err(error) => panic!(
                        "private Musubi publication ingress failed before supervisor shutdown: {error:?}"
                    ),
                }
            }
        });
        Child::new(task, OnShutdown::Wait(Duration::from_secs(2)))
    }
}
/// Stock-launcher state for a service that cannot start without deployment injection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MusubiPublicationPrivateServiceAvailabilityV1 {
    /// No listener, signer, journal, or backend was injected; all private routes are absent.
    #[default]
    Unavailable,
    /// A deployment-owned private service was assembled and handed to the supervisor.
    Supervised,
}
/// Start an optional deployment, leaving the stock daemon explicitly unavailable by default.
///
/// The returned child must be monitored by the caller's `Supervisor` when present.
#[must_use]
pub fn start_injected_musubi_publication_private_service_v1(
    deployment: Option<MusubiPublicationPrivateDeploymentV1>,
    shutdown: ShutdownSignal,
) -> (MusubiPublicationPrivateServiceAvailabilityV1, Option<Child>) {
    deployment.map_or_else(
        || {
            (
                MusubiPublicationPrivateServiceAvailabilityV1::Unavailable,
                None,
            )
        },
        |deployment| {
            (
                MusubiPublicationPrivateServiceAvailabilityV1::Supervised,
                Some(deployment.start(shutdown)),
            )
        },
    )
}
/// Build and start an optional late-bound deployment.
///
/// `None` remains the stock fail-closed state. A factory failure is returned before a child can be
/// monitored or any private route can become available.
///
/// # Errors
///
/// Returns a redacted deployment-factory failure.
pub fn build_and_start_injected_musubi_publication_private_service_v1(
    factory: Option<Box<dyn MusubiPublicationPrivateServiceFactoryV1>>,
    context: MusubiPublicationPrivateServiceContextV1,
    shutdown: ShutdownSignal,
) -> Result<
    (MusubiPublicationPrivateServiceAvailabilityV1, Option<Child>),
    MusubiPublicationPrivateServiceFactoryErrorV1,
> {
    let Some(factory) = factory else {
        return Ok(start_injected_musubi_publication_private_service_v1(
            None, shutdown,
        ));
    };
    let deployment = factory.build(context)?;
    Ok(start_injected_musubi_publication_private_service_v1(
        Some(deployment),
        shutdown,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::parameters::actual::Queue as QueueConfig;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair};
    use iroha_data_model::{account::AccountId, block::BlockHeader, sorafs::capacity::ProviderId};
    use iroha_futures::supervisor::Supervisor;
    use iroha_musubi_service::{
        DurableMusubiPublicationServiceClockV1, DurableMusubiPublicationServiceJournalLimitsV1,
        DurableMusubiPublicationServiceJournalOpenErrorV1,
        DurableMusubiPublicationServiceJournalV1, MusubiPublicationPrivateServiceV1,
        MusubiPublicationServiceBackendErrorV1, MusubiPublicationServiceConfigurationV1,
        MusubiPublicationServiceJournalBindingV1, MusubiSeedIngressBackendV1,
        MusubiStorageCoordinationBackendV1, MusubiStorageCoordinationRequestV1,
        MusubiStorageCoordinationResponseV1, SoftwareMusubiSeedIngressReceiptSignerV1,
    };
    use iroha_musubi_service::{MusubiSeedStagingBackendV1, MusubiSeedStagingErrorV1};
    use sorafs_node::config::StorageConfig;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::{fs, path::Path};
    struct EarlyExitRunner;
    impl MusubiPublicationPrivateServiceRunnerV1 for EarlyExitRunner {
        fn serve(
            self: Box<Self>,
            _shutdown: ShutdownSignal,
        ) -> MusubiPublicationPrivateIngressFutureV1 {
            Box::pin(async { Ok(()) })
        }
    }
    struct ShutdownAwareRunner {
        started: Arc<AtomicBool>,
    }
    impl MusubiPublicationPrivateServiceRunnerV1 for ShutdownAwareRunner {
        fn serve(
            self: Box<Self>,
            shutdown: ShutdownSignal,
        ) -> MusubiPublicationPrivateIngressFutureV1 {
            self.started.store(true, Ordering::SeqCst);
            Box::pin(async move {
                shutdown.receive().await;
                Ok(())
            })
        }
    }
    struct PrivateTestRoot {
        // Retained native handles close before the enclosing temporary directory is removed.
        directory: iroha_fs::PrivateDirectory,
        _parent: tempfile::TempDir,
    }
    impl PrivateTestRoot {
        fn path(&self) -> &Path {
            self.directory.path()
        }
    }
    fn factory_context() -> (MusubiPublicationPrivateServiceContextV1, PrivateTestRoot) {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, query));
        let (events, _) = tokio::sync::broadcast::channel(1);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events));
        let parent = tempfile::tempdir().expect("fixture storage workspace");
        let directory = iroha_fs::PrivateDirectory::open_or_create(parent.path().join("private"))
            .expect("native private fixture storage");
        let temp = PrivateTestRoot {
            directory,
            _parent: parent,
        };
        let sorafs_node = sorafs_node::NodeHandle::new(
            StorageConfig::builder()
                .data_dir(temp.path().join("storage"))
                .build(),
        );
        (
            MusubiPublicationPrivateServiceContextV1::new(
                *state.network_id_ref(),
                state,
                queue,
                sorafs_node,
            ),
            temp,
        )
    }
    #[test]
    fn local_custody_reopens_only_matching_initialized_journal_and_seed_owner() {
        let (context, temporary) = factory_context();
        let journal_root = temporary.path().join("publication-journal");
        let seed_root = temporary.path().join("publication-seeds");
        for root in [&journal_root, &seed_root] {
            iroha_fs::PrivateDirectory::open_or_create(root)
                .expect("native private custody directory");
        }
        let broker_key =
            KeyPair::try_from_seed(vec![0x37; 32], Algorithm::Ed25519).expect("broker fixture key");
        let configuration = MusubiPublicationServiceConfigurationV1 {
            network_id: context.network_id(),
            ingress_broker: AccountId::new(broker_key.public_key().clone()),
            seed_provider: ProviderId::new([0x38; 32]),
            max_future_clock_skew_ms: 0,
            receipt_lifetime_ms: 60_000,
        };
        let limits = DurableMusubiPublicationServiceJournalLimitsV1::new(
            2,
            4,
            16 * 1024 * 1024,
            17 * 1024 * 1024,
        )
        .expect("bounded journal limits");
        assert!(matches!(
            context.open_local_publication_custody(
                &journal_root,
                &seed_root,
                &configuration,
                limits,
                2,
                128 * 1024 * 1024,
            ),
            Err(MusubiPublicationPrivateLocalCustodyErrorV1::Journal(
                DurableMusubiPublicationServiceJournalOpenErrorV1::Uninitialized
            )),
        ));
        let binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&configuration);
        let journal =
            DurableMusubiPublicationServiceJournalV1::initialize(&journal_root, binding, limits)
                .expect("explicit one-time initialization");
        assert_eq!(journal.revision(), 1);
        drop(journal);
        // A missing seed marker refuses after releasing the already acquired journal lease.
        assert!(matches!(
            context.open_local_publication_custody(
                &journal_root,
                &seed_root,
                &configuration,
                limits,
                2,
                128 * 1024 * 1024,
            ),
            Err(MusubiPublicationPrivateLocalCustodyErrorV1::Seed(
                MusubiSeedStagingErrorV1::Invalid
            )),
        ));
        assert_eq!(fs::read_dir(&seed_root).unwrap().count(), 0);
        drop(
            DurableMusubiPublicationServiceJournalV1::open(
                &journal_root,
                MusubiPublicationServiceJournalBindingV1::from_configuration(&configuration),
                limits,
            )
            .expect("failed seed open releases original journal lock"),
        );
        drop(
            MusubiSeedStagingBackendV1::initialize(
                &seed_root,
                configuration.seed_provider,
                2,
                128 * 1024 * 1024,
            )
            .expect("explicit fresh seed ownership"),
        );
        let custody = context
            .open_local_publication_custody(
                &journal_root,
                &seed_root,
                &configuration,
                limits,
                2,
                128 * 1024 * 1024,
            )
            .expect("recovered local custody");
        assert!(matches!(
            context.open_local_publication_custody(
                &journal_root,
                &seed_root,
                &configuration,
                limits,
                2,
                128 * 1024 * 1024,
            ),
            Err(MusubiPublicationPrivateLocalCustodyErrorV1::Journal(
                DurableMusubiPublicationServiceJournalOpenErrorV1::Locked
            )),
        ));
        let (journal, seed, reader) = custody.into_parts();
        assert_eq!(journal.revision(), 1);
        assert_eq!(seed.provider_id(), configuration.seed_provider);
        drop((journal, seed, reader));
        let mut foreign = configuration.clone();
        foreign.network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([0x17; 32])),
        );
        assert!(matches!(
            context.open_local_publication_custody(
                &journal_root,
                &seed_root,
                &foreign,
                limits,
                2,
                128 * 1024 * 1024,
            ),
            Err(MusubiPublicationPrivateLocalCustodyErrorV1::NetworkMismatch),
        ));
        let reopened = context
            .open_local_publication_custody(
                &journal_root,
                &seed_root,
                &configuration,
                limits,
                2,
                128 * 1024 * 1024,
            )
            .expect("restart reopens exact owners");
        assert_eq!(reopened.into_parts().0.revision(), 1);
    }
    struct RetainingStorageBackend {
        _finalized_reader: MusubiPublicationFinalizedArchiveRegistrationReaderV1,
    }
    impl MusubiStorageCoordinationBackendV1 for RetainingStorageBackend {
        fn verify_current_registration(
            &self,
            _request: &MusubiStorageCoordinationRequestV1,
        ) -> Result<(), MusubiPublicationServiceBackendErrorV1> {
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        }

        fn coordinate_storage(
            &mut self,
            _request: &iroha_musubi_service::VerifiedStorageCoordinationRequestV1<'_>,
        ) -> Result<MusubiStorageCoordinationResponseV1, MusubiPublicationServiceBackendErrorV1>
        {
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        }
    }
    struct RetainingStorageBuilder(Arc<AtomicBool>, AccountId);
    impl MusubiPublicationPrivateStorageBuilderV1 for RetainingStorageBuilder {
        fn build(
            self: Box<Self>,
            context: &MusubiPublicationPrivateServiceContextV1,
            finalized_reader: MusubiPublicationFinalizedArchiveRegistrationReaderV1,
            finalized_seed: MusubiPublicationFinalizedSeedReadCapabilityV1,
            paid_pin: iroha_config::parameters::actual::MusubiPublicationPaidPinPolicy,
            mut clock: Box<dyn iroha_musubi_service::MusubiPublicationServiceClockV1>,
        ) -> Result<
            Box<dyn MusubiStorageCoordinationBackendV1>,
            MusubiPublicationPrivateServiceFactoryErrorV1,
        > {
            assert_eq!(context.network_id(), *context.state().network_id_ref());
            assert_eq!(finalized_seed.provider_id(), ProviderId::new([0x38; 32]));
            assert_eq!(paid_pin.transaction_authority, self.1);
            assert!(clock.current_time_ms().unwrap() > 0);
            assert_eq!(
                paid_pin.storage_class,
                iroha_data_model::sorafs::pin_registry::StorageClass::Hot
            );
            assert_eq!(paid_pin.retention_horizon_secs, 30 * 24 * 60 * 60);
            self.0.store(true, Ordering::SeqCst);
            Ok(Box::new(RetainingStorageBackend {
                _finalized_reader: finalized_reader,
            }))
        }
    }
    struct RetainingIngressBuilder(Arc<AtomicBool>);
    struct RetainingServiceRunner {
        service: MusubiPublicationPrivateServiceV1,
    }
    impl MusubiPublicationPrivateServiceRunnerV1 for RetainingServiceRunner {
        fn serve(
            self: Box<Self>,
            shutdown: ShutdownSignal,
        ) -> MusubiPublicationPrivateIngressFutureV1 {
            Box::pin(async move {
                let _service = self.service;
                shutdown.receive().await;
                Ok(())
            })
        }
    }
    impl MusubiPublicationPrivateIngressBuilderV1 for RetainingIngressBuilder {
        fn build(
            self: Box<Self>,
            service: MusubiPublicationPrivateServiceV1,
        ) -> Result<
            MusubiPublicationPrivateDeploymentV1,
            MusubiPublicationPrivateServiceFactoryErrorV1,
        > {
            self.0.store(true, Ordering::SeqCst);
            Ok(MusubiPublicationPrivateDeploymentV1::new(Box::new(
                RetainingServiceRunner { service },
            )))
        }
    }
    fn write_authenticated_readback_fixture(
        root: &Path,
        network_id: NetworkId,
        provider: ProviderId,
    ) -> iroha_storage_client::musubi_archive_fetch::AuthenticatedMusubiArchiveFetchClientV1 {
        let operator =
            KeyPair::try_from_seed(vec![0x39; 32], Algorithm::Ed25519).expect("operator key");
        let directory = iroha_fs::PrivateDirectory::open_or_create(root.join("readback"))
            .expect("native private readback directory");
        directory
            .write_atomic(
                "operator.key",
                format!("{}\n", ExposedPrivateKey(operator.private_key().clone())).as_bytes(),
                iroha_fs::PublishMode::CreateNew,
            )
            .expect("runtime-only private operator key");
        let config_path = directory.path().join("client.toml");
        directory
            .write_atomic(
                "client.toml",
                format!(
                    "[musubi.fetch]\nnetwork_id = \"{network_id}\"\n\n[[musubi.fetch.provider_gateways]]\nprovider_id = \"{}\"\nurl = \"https://8.8.8.8/\"\noperator_public_key = \"{}\"\noperator_private_key_file = \"operator.key\"\n",
                    hex::encode(provider.as_bytes()),
                    operator.public_key(),
                )
                .as_bytes(),
                iroha_fs::PublishMode::CreateNew,
            )
            .expect("private platform configuration");
        iroha_storage_client::musubi_archive_fetch::AuthenticatedMusubiArchiveFetchClientV1::load_platform_file(&config_path)
            .expect("authenticated provider transport")
    }
    #[test]
    fn local_factory_retains_exact_recovered_custody_and_finalized_reader() {
        let (context, temporary) = factory_context();
        let reopening_context = MusubiPublicationPrivateServiceContextV1::new(
            context.network_id(),
            context.state(),
            context.queue(),
            context.sorafs_node(),
        );
        let journal_root = temporary.path().join("journal");
        let seed_root = temporary.path().join("seed");
        let clock_root = temporary.path().join("clock");
        for root in [&journal_root, &seed_root, &clock_root] {
            iroha_fs::PrivateDirectory::open_or_create(root)
                .expect("native private custody directory");
        }
        let broker_key =
            KeyPair::try_from_seed(vec![0x37; 32], Algorithm::Ed25519).expect("broker key");
        let service_configuration = MusubiPublicationServiceConfigurationV1 {
            network_id: context.network_id(),
            ingress_broker: AccountId::new(broker_key.public_key().clone()),
            seed_provider: ProviderId::new([0x38; 32]),
            max_future_clock_skew_ms: 0,
            receipt_lifetime_ms: 60_000,
        };
        let journal_limits = DurableMusubiPublicationServiceJournalLimitsV1::new(
            2,
            4,
            16 * 1024 * 1024,
            17 * 1024 * 1024,
        )
        .expect("bounded journal limits");
        let binding =
            MusubiPublicationServiceJournalBindingV1::from_configuration(&service_configuration);
        drop(
            DurableMusubiPublicationServiceJournalV1::initialize(
                &journal_root,
                binding,
                journal_limits,
            )
            .expect("initialize durable journal"),
        );
        drop(
            DurableMusubiPublicationServiceClockV1::initialize_system(&clock_root)
                .expect("initialize durable time floor"),
        );
        drop(
            MusubiSeedStagingBackendV1::initialize(
                &seed_root,
                service_configuration.seed_provider,
                2,
                128 * 1024 * 1024,
            )
            .expect("explicit seed ownership before factory startup"),
        );
        let readback_client = write_authenticated_readback_fixture(
            temporary.path(),
            context.network_id(),
            service_configuration.seed_provider,
        );
        let signer = SoftwareMusubiSeedIngressReceiptSignerV1::new(
            service_configuration.ingress_broker.clone(),
            broker_key,
        )
        .expect("fixture signer matches broker");
        let storage_called = Arc::new(AtomicBool::new(false));
        let ingress_called = Arc::new(AtomicBool::new(false));
        let settings = MusubiPublicationPrivateLocalFactorySettingsV1::from_config(
            &iroha_config::parameters::actual::MusubiPublication {
                custody_root: temporary.path().to_path_buf(),
                journal_max_operations: journal_limits.max_operations(),
                journal_max_authorizations: journal_limits.max_authorizations(),
                journal_max_total_response_bytes: journal_limits.max_total_response_bytes(),
                journal_max_snapshot_bytes: journal_limits.max_snapshot_bytes(),
                max_seed_records: 2,
                max_seed_bytes: 128 * 1024 * 1024,
                max_future_clock_skew_ms: service_configuration.max_future_clock_skew_ms,
                receipt_lifetime_ms: service_configuration.receipt_lifetime_ms,
                ..Default::default()
            },
            service_configuration.network_id,
            service_configuration.ingress_broker.clone(),
            service_configuration.seed_provider,
        )
        .expect("configured local factory settings");
        let factory = MusubiPublicationPrivateLocalFactoryV1::new(
            settings,
            Box::new(signer),
            Box::new(RetainingStorageBuilder(
                Arc::clone(&storage_called),
                service_configuration.ingress_broker.clone(),
            )),
            readback_client,
            Arc::new(tokio::sync::RwLock::new(
                iroha_torii::sorafs::ProviderAdvertCache::new(
                    [],
                    Arc::new(iroha_torii::sorafs::admission::AdmissionRegistry::empty(
                        *service_configuration.network_id.as_bytes(),
                    )),
                ),
            )),
            Box::new(RetainingIngressBuilder(Arc::clone(&ingress_called))),
        );
        let deployment = Box::new(factory)
            .build(context)
            .expect("daemon-owned core assembly");
        assert!(storage_called.load(Ordering::SeqCst));
        assert!(ingress_called.load(Ordering::SeqCst));
        assert!(matches!(
            reopening_context.open_local_publication_custody(
                &journal_root,
                &seed_root,
                &service_configuration,
                journal_limits,
                2,
                128 * 1024 * 1024,
            ),
            Err(MusubiPublicationPrivateLocalCustodyErrorV1::Journal(
                DurableMusubiPublicationServiceJournalOpenErrorV1::Locked
            )),
        ));
        drop(deployment);
        reopening_context
            .open_local_publication_custody(
                &journal_root,
                &seed_root,
                &service_configuration,
                journal_limits,
                2,
                128 * 1024 * 1024,
            )
            .expect("final owner releases durable custody after runner drop");
    }
    struct RecordingFactory {
        called: Arc<AtomicBool>,
        expected_state: Arc<State>,
        expected_queue: Arc<Queue>,
        expected_capacity: Arc<sorafs_node::capacity::CapacityManager>,
        runner_started: Arc<AtomicBool>,
    }
    impl MusubiPublicationPrivateServiceFactoryV1 for RecordingFactory {
        fn build(
            self: Box<Self>,
            context: MusubiPublicationPrivateServiceContextV1,
        ) -> Result<
            MusubiPublicationPrivateDeploymentV1,
            MusubiPublicationPrivateServiceFactoryErrorV1,
        > {
            assert!(!self.called.swap(true, Ordering::SeqCst));
            assert_eq!(context.network_id(), *self.expected_state.network_id_ref());
            assert!(Arc::ptr_eq(&context.state(), &self.expected_state));
            assert!(Arc::ptr_eq(&context.queue(), &self.expected_queue));
            assert!(Arc::ptr_eq(
                &context.sorafs_node().capacity_manager(),
                &self.expected_capacity,
            ));
            Ok(MusubiPublicationPrivateDeploymentV1::new(Box::new(
                ShutdownAwareRunner {
                    started: Arc::clone(&self.runner_started),
                },
            )))
        }
    }
    struct FailingFactory;
    impl MusubiPublicationPrivateServiceFactoryV1 for FailingFactory {
        fn build(
            self: Box<Self>,
            _context: MusubiPublicationPrivateServiceContextV1,
        ) -> Result<
            MusubiPublicationPrivateDeploymentV1,
            MusubiPublicationPrivateServiceFactoryErrorV1,
        > {
            Err(MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified)
        }
    }
    #[test]
    fn stock_launch_is_fail_closed_and_starts_no_child() {
        let (availability, child) =
            start_injected_musubi_publication_private_service_v1(None, ShutdownSignal::new());
        assert_eq!(
            availability,
            MusubiPublicationPrivateServiceAvailabilityV1::Unavailable
        );
        assert!(child.is_none());
    }
    #[test]
    fn simultaneous_factory_contexts_hold_distinct_storage_owners() {
        let (first, first_storage) = factory_context();
        let (second, second_storage) = factory_context();
        assert_ne!(first_storage.path(), second_storage.path());
        assert!(!Arc::ptr_eq(
            &first.sorafs_node().capacity_manager(),
            &second.sorafs_node().capacity_manager()
        ));
        drop(first);
        assert!(second_storage.path().exists());
        drop(second);
    }
    #[test]
    fn absent_factory_is_fail_closed_and_starts_no_child() {
        let (context, _storage) = factory_context();
        let (availability, child) = build_and_start_injected_musubi_publication_private_service_v1(
            None,
            context,
            ShutdownSignal::new(),
        )
        .expect("absent factory is not an error");
        assert_eq!(
            availability,
            MusubiPublicationPrivateServiceAvailabilityV1::Unavailable
        );
        assert!(child.is_none());
    }
    #[test]
    fn factory_failure_precedes_child_start() {
        let (context, _storage) = factory_context();
        let result = build_and_start_injected_musubi_publication_private_service_v1(
            Some(Box::new(FailingFactory)),
            context,
            ShutdownSignal::new(),
        );
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("unqualified factory must fail closed"),
        };
        assert_eq!(
            error,
            MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified
        );
    }
    #[tokio::test]
    async fn factory_receives_exact_handles_once_and_joins_supervisor() {
        let (context, _storage) = factory_context();
        let called = Arc::new(AtomicBool::new(false));
        let runner_started = Arc::new(AtomicBool::new(false));
        let factory = RecordingFactory {
            called: Arc::clone(&called),
            expected_state: context.state(),
            expected_queue: context.queue(),
            expected_capacity: context.sorafs_node().capacity_manager(),
            runner_started: Arc::clone(&runner_started),
        };
        let mut supervisor = Supervisor::new();
        let shutdown = supervisor.shutdown_signal();
        let (availability, child) = build_and_start_injected_musubi_publication_private_service_v1(
            Some(Box::new(factory)),
            context,
            shutdown.clone(),
        )
        .expect("qualified factory builds");
        assert_eq!(
            availability,
            MusubiPublicationPrivateServiceAvailabilityV1::Supervised
        );
        assert!(called.load(Ordering::SeqCst));
        supervisor.monitor(child.expect("factory-built deployment child"));
        shutdown.send();
        assert!(supervisor.start().await.is_ok());
        assert!(runner_started.load(Ordering::SeqCst));
    }
    #[tokio::test]
    async fn injected_runner_early_return_is_fatal_to_supervision() {
        let mut supervisor = Supervisor::new();
        let (availability, child) = start_injected_musubi_publication_private_service_v1(
            Some(MusubiPublicationPrivateDeploymentV1::new(Box::new(
                EarlyExitRunner,
            ))),
            supervisor.shutdown_signal(),
        );
        assert_eq!(
            availability,
            MusubiPublicationPrivateServiceAvailabilityV1::Supervised
        );
        supervisor.monitor(child.expect("injected deployment child"));
        assert!(supervisor.start().await.is_err());
    }
    #[tokio::test]
    async fn injected_runner_shares_supervisor_shutdown_and_exits_cleanly() {
        let started = Arc::new(AtomicBool::new(false));
        let mut supervisor = Supervisor::new();
        let shutdown = supervisor.shutdown_signal();
        let (availability, child) = start_injected_musubi_publication_private_service_v1(
            Some(MusubiPublicationPrivateDeploymentV1::new(Box::new(
                ShutdownAwareRunner {
                    started: Arc::clone(&started),
                },
            ))),
            shutdown.clone(),
        );
        assert_eq!(
            availability,
            MusubiPublicationPrivateServiceAvailabilityV1::Supervised
        );
        supervisor.monitor(child.expect("injected deployment child"));
        shutdown.send();
        assert!(supervisor.start().await.is_ok());
        assert!(started.load(Ordering::SeqCst));
    }
}
