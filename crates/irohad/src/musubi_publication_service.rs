//! Supervised deployment boundary for the private Musubi publication service.
//!
//! The stock daemon injects nothing and therefore opens no publication listener. A deployment
//! may construct the transport-independent service from `iroha::musubi_runtime`, retain its TLS
//! and signing material outside argv and repository configuration, and inject an authenticated
//! HTTPS ingress here. This module never routes through Torii or the daemon-private runtime
//! provider broker.

// TODO: Supply a deployment-qualified runner only after the production boundaries below exist.
// The stock tree deliberately cannot assemble one from the current SoraFS/Torii primitives:
//
// 1. provider adapters must invoke `sorafs_car::musubi::MusubiBundleVerifierV1` before a
//    runtime-only completion-authority signer produces
//    `MusubiProviderBundleVerificationAttestationV1`; the signer and those adapters remain to be
//    implemented, and an ordinary SoraFS storage completion is not that attestation;
// 2. storage coordination needs its own crash-safe idempotency journal and an authoritative
//    finalized-chain reader which verifies the exact committed registration transaction and
//    immutable archive projection before submitting/reconciling pin and replication mutations;
// 3. readback needs admitted-provider authentication, redirect and DNS-rebinding defenses, and
//    full plan/CAR/bundle verification; and
// 4. daemon assembly needs non-secret public configuration, runtime credential/signer resolution,
//    and a private TLS listener constructed around daemon-owned finalized-state/SoraFS handles.
//
// The protocol core, durable clock and service replay journal, and typed supervisor dependency are
// complete. Until every boundary above is implemented and deployment-qualified, stock `irohad`
// must keep the routes absent. In particular, do not substitute an in-memory backend, treat a
// public query response or publisher-supplied bytes as finality evidence, or revive the retired
// public Torii upload path.

use std::{future::Future, pin::Pin, time::Duration};

use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};

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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;
    use iroha_futures::supervisor::Supervisor;

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
