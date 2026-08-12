//! Supervised deployment boundary for the private Musubi publication service.
//!
//! The stock daemon injects nothing and therefore opens no publication listener. A deployment
//! may construct the transport-independent service from `iroha::musubi_runtime`, retain its TLS
//! and signing material outside argv and repository configuration, and inject an authenticated
//! HTTPS ingress here. This module never routes through Torii or the daemon-private runtime
//! provider broker.

mod finality;

pub use finality::{
    MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    MusubiPublicationFinalizedArchiveRegistrationReadErrorV1,
    MusubiPublicationFinalizedArchiveRegistrationReaderV1,
};

// TODO: Supply a deployment-qualified runner only after the production boundaries below exist.
// The stock tree deliberately cannot assemble one from the current SoraFS/Torii primitives:
//
// 1. provider ingest durably binds a V5 network/archive authorization context, accepts only
//    monotonic finalized observations over the retained admission cursor, and keeps generic and
//    Musubi receipt shapes disjoint. The finalized reader can seal the local provider's exact
//    opaque completed-row claim, and a fresh verifier result can derive an externally inert
//    approval request. A runtime driver must still perform that fresh verifier pass and submit the
//    request only to an approval-only HSM/KMS or threshold provider;
// 2. the approved provider attestation has a bounded journal and an inert, root-fenced local
//    two-slot CAS adapter with a fixed 128 MiB checkpoint/payload ceiling on Linux/macOS. Its bound
//    cross-process composite operation lease authenticates the committed initialization-lock
//    identity plus separate checkpoint-head and immutable-blob namespaces. It binds the exact
//    network/provider and rejects online substitution, torn writes, and divergent lineage, but is
//    not daemon-wired; external rollback-resistant provider/session/singleton deployment and
//    fault/platform qualification remain;
// 3. the authenticated provider-attestation inventory/coordinator handoff needs production SoraFS
//    pin/replication mutation APIs and must consume the implemented daemon-owned finalized archive
//    registration reader before submitting or reconciling those mutations;
// 4. readback needs admitted-provider authentication, redirect and DNS-rebinding defenses, and
//    full plan/CAR/bundle verification; and
// 5. deployment assembly needs non-secret public configuration, runtime credential/signer
//    resolution, and a private TLS listener constructed by this injected factory around the
//    daemon-owned finalized-state/SoraFS handles.
//
// The publication protocol core, publication-service durable clock and replay journal, typed
// supervisor dependency, provider-attestation journal, inert local two-slot store with its bound
// composite operation lease, and read-only authoritative archive-registration reader are complete.
// The finalized-completion capture/reconciliation driver, qualified replay-stable HSM signer,
// authenticated inventory adapter, daemon wiring, external rollback-resistant
// provider/session/singleton deployment, and production fault/platform qualification are not.
// Until every boundary above is implemented and deployment-qualified, stock `irohad` must keep the
// routes absent. In particular, do not
// substitute an in-memory backend, treat the local two-slot store as protection from privileged
// offline rollback, treat a public query response or publisher-supplied bytes as finality evidence,
// or revive the retired public Torii upload path.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use iroha_core::{queue::Queue, state::State};
use iroha_data_model::NetworkId;
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};

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
        }
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

    /// Clone the embedded SoraFS node handle.
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
    /// body before allocation, strip only their configured private mount prefix, and pass the
    /// exact uppercase method plus path/header/body values to
    /// `iroha::musubi_runtime::MusubiPublicationPrivateServiceV1`.
    /// The runner owns that core together with its injected durable journal, HSM/signer, and
    /// SoraFS backends; `irohad` never receives those secrets or dependency objects.
    fn serve(self: Box<Self>, shutdown: ShutdownSignal) -> MusubiPublicationPrivateIngressFutureV1;
}

/// Complete injected private-service deployment assembled outside stock `irohad` configuration.
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
    match deployment {
        Some(deployment) => (
            MusubiPublicationPrivateServiceAvailabilityV1::Supervised,
            Some(deployment.start(shutdown)),
        ),
        None => (
            MusubiPublicationPrivateServiceAvailabilityV1::Unavailable,
            None,
        ),
    }
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
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;
    use iroha_config::parameters::actual::Queue as QueueConfig;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_futures::supervisor::Supervisor;
    use sorafs_node::config::StorageConfig;

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

    fn factory_context() -> MusubiPublicationPrivateServiceContextV1 {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::new(), kura, query));
        let (events, _) = tokio::sync::broadcast::channel(1);
        let queue = Arc::new(Queue::from_config(QueueConfig::default(), events));
        let sorafs_node = sorafs_node::NodeHandle::new(StorageConfig::default());
        MusubiPublicationPrivateServiceContextV1::new(
            *state.network_id_ref(),
            state,
            queue,
            sorafs_node,
        )
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
    fn absent_factory_is_fail_closed_and_starts_no_child() {
        let (availability, child) = build_and_start_injected_musubi_publication_private_service_v1(
            None,
            factory_context(),
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
        let result = build_and_start_injected_musubi_publication_private_service_v1(
            Some(Box::new(FailingFactory)),
            factory_context(),
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
        let context = factory_context();
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
