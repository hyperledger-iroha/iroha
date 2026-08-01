//! Deployment-owned assembly for the stock runtime-provider broker server.
//!
//! This boundary accepts only the sanitized public binding catalog projected
//! by [`IrohaRuntimeProviderBindingsV1`]. Provider credentials, private keys,
//! tokens, attestations, and private evidence remain encapsulated by the
//! deployment-owned backend objects returned by the registry.

use std::{fmt, sync::Arc};

use super::api::{
    RuntimeProviderBrokerBackendsV1, RuntimeProviderBrokerLifecycleV1,
    RuntimeProviderBrokerServerErrorV1, serve_runtime_provider_broker_v1,
    serve_runtime_provider_broker_with_lifecycle_v1,
};
use crate::runtime_provider_registry::{
    IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderRegistryErrorV1,
};

/// Deployment-owned resolver for the complete broker-server backend set.
///
/// Implementations use stable public handles from `bindings` to locate
/// already-provisioned adapters. Credentials and private material must remain
/// inside those adapters and must never be returned separately, logged, or
/// added to [`iroha_config`]. The stock server independently validates the
/// exact backend set and live provider qualification before publishing
/// readiness or accepting a client.
pub trait RuntimeProviderBrokerBackendRegistryV1: Send + Sync {
    /// Resolve every backend requested by one exact public catalog.
    ///
    /// # Errors
    ///
    /// Returns a payload-free error if any backend is unavailable, missing,
    /// substituted, stale, revoked, test-marked, or otherwise incomplete.
    fn resolve(
        &self,
        bindings: &IrohaRuntimeProviderBindingsV1,
    ) -> Result<RuntimeProviderBrokerBackendsV1, IrohaRuntimeProviderRegistryErrorV1>;
}

/// Fully assembled deployment-owned broker launch.
///
/// Construction retains only the public binding catalog and opaque backend
/// trait objects. It performs no environment discovery and has a deliberately
/// redacted [`Debug`] implementation.
pub struct RuntimeProviderBrokerDeploymentV1 {
    bindings: IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
}

impl RuntimeProviderBrokerDeploymentV1 {
    /// Resolve the complete backend set for a non-empty public catalog.
    ///
    /// The deployment registry receives only `bindings`; it never receives the
    /// node configuration from which the catalog was projected.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeProviderBrokerLauncherErrorV1::EmptyCatalog`] when a
    /// broker process was enabled without any provider roles, or preserves the
    /// registry's payload-free failure category when resolution fails.
    pub fn try_new(
        bindings: IrohaRuntimeProviderBindingsV1,
        registry: &dyn RuntimeProviderBrokerBackendRegistryV1,
    ) -> Result<Self, RuntimeProviderBrokerLauncherErrorV1> {
        if bindings.is_empty() {
            return Err(RuntimeProviderBrokerLauncherErrorV1::EmptyCatalog);
        }
        let backends = registry
            .resolve(&bindings)
            .map_err(RuntimeProviderBrokerLauncherErrorV1::BackendRegistry)?;
        Ok(Self { bindings, backends })
    }

    /// Return the number of exact public provider bindings to be served.
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    /// Qualify every backend and serve on the platform-fixed authenticated
    /// endpoint until the server stops.
    ///
    /// # Errors
    ///
    /// Returns a payload-free server error before accepting clients when the
    /// resolved set is missing, extra, substituted, stale, revoked,
    /// test-marked, or live qualification otherwise fails.
    pub fn serve(self) -> Result<(), RuntimeProviderBrokerLauncherErrorV1> {
        serve_runtime_provider_broker_v1(&self.bindings, self.backends)
            .map_err(RuntimeProviderBrokerLauncherErrorV1::Server)
    }

    /// Qualify every backend and serve with caller-owned readiness and shutdown.
    ///
    /// `on_ready` runs only after the complete catalog passes both startup
    /// qualification rounds and the authenticated endpoint is securely bound.
    /// See [`serve_runtime_provider_broker_with_lifecycle_v1`] for the callback
    /// and shutdown contract.
    ///
    /// # Errors
    ///
    /// Returns a payload-free server error before readiness when the resolved
    /// set or any live qualification is not exact.
    pub fn serve_with_lifecycle<R>(
        self,
        lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
        on_ready: R,
    ) -> Result<(), RuntimeProviderBrokerLauncherErrorV1>
    where
        R: FnOnce(),
    {
        serve_runtime_provider_broker_with_lifecycle_v1(
            &self.bindings,
            self.backends,
            lifecycle,
            on_ready,
        )
        .map_err(RuntimeProviderBrokerLauncherErrorV1::Server)
    }
}

impl fmt::Debug for RuntimeProviderBrokerDeploymentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProviderBrokerDeploymentV1")
            .field("chain_id", &self.bindings.chain_id())
            .field("binding_count", &self.bindings.len())
            .finish_non_exhaustive()
    }
}

/// Payload-free deployment broker assembly or serving failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeProviderBrokerLauncherErrorV1 {
    /// A broker process was enabled without any provider role.
    EmptyCatalog,
    /// The deployment registry could not resolve the complete backend set.
    BackendRegistry(IrohaRuntimeProviderRegistryErrorV1),
    /// Live qualification, endpoint security, or serving failed.
    Server(RuntimeProviderBrokerServerErrorV1),
}

impl fmt::Display for RuntimeProviderBrokerLauncherErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCatalog => formatter.write_str("runtime-provider broker catalog is empty"),
            Self::BackendRegistry(error) => fmt::Display::fmt(error, formatter),
            Self::Server(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for RuntimeProviderBrokerLauncherErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EmptyCatalog => None,
            Self::BackendRegistry(error) => Some(error),
            Self::Server(error) => Some(error),
        }
    }
}

// Release deployment requirement: supply and supervise a concrete vendor
// adapter registry plus an executable that calls this launcher. That external
// package owns HSM/KMS, WebAuthn, authenticated transport, immutable-query,
// publication, and sealed-CAS credentials; this repository intentionally does
// not include a dummy, test, file-key, or dynamically selected fallback.

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;
    use crate::IrohaRuntimeProviderSlotV1;

    struct RecordingRegistry {
        calls: AtomicUsize,
        outcome: Result<RuntimeProviderBrokerBackendsV1, IrohaRuntimeProviderRegistryErrorV1>,
    }

    impl RecordingRegistry {
        fn available() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                outcome: Ok(RuntimeProviderBrokerBackendsV1::new()),
            }
        }

        fn failing(error: IrohaRuntimeProviderRegistryErrorV1) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                outcome: Err(error),
            }
        }
    }

    impl RuntimeProviderBrokerBackendRegistryV1 for RecordingRegistry {
        fn resolve(
            &self,
            bindings: &IrohaRuntimeProviderBindingsV1,
        ) -> Result<RuntimeProviderBrokerBackendsV1, IrohaRuntimeProviderRegistryErrorV1> {
            assert!(!bindings.is_empty());
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.outcome.clone()
        }
    }

    fn qualified_catalog() -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1::qualified_for_test(
            "sora.production",
            IrohaRuntimeProviderSlotV1::PrivacyCyclePrfProvider,
            "hsm://sorafs/privacy/prf/primary",
            7,
            [0x51; 32],
        )
    }

    #[test]
    fn empty_catalog_is_rejected_without_backend_discovery() {
        let registry = RecordingRegistry::available();
        let result = RuntimeProviderBrokerDeploymentV1::try_new(
            IrohaRuntimeProviderBindingsV1::empty_for_test("sora.production"),
            &registry,
        );

        assert_eq!(
            result.expect_err("an enabled broker requires at least one role"),
            RuntimeProviderBrokerLauncherErrorV1::EmptyCatalog
        );
        assert_eq!(registry.calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn backend_registry_failure_category_is_preserved() {
        let registry =
            RecordingRegistry::failing(IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked);
        let result = RuntimeProviderBrokerDeploymentV1::try_new(qualified_catalog(), &registry);

        assert_eq!(
            result.expect_err("stale provider must reject broker assembly"),
            RuntimeProviderBrokerLauncherErrorV1::BackendRegistry(
                IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
            )
        );
        assert_eq!(registry.calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn launcher_errors_expose_only_payload_free_sources() {
        let empty = RuntimeProviderBrokerLauncherErrorV1::EmptyCatalog;
        assert_eq!(
            empty.to_string(),
            "runtime-provider broker catalog is empty"
        );
        assert!(std::error::Error::source(&empty).is_none());

        let registry = RuntimeProviderBrokerLauncherErrorV1::BackendRegistry(
            IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected,
        );
        assert_eq!(
            registry.to_string(),
            "runtime-provider binding is test-marked"
        );
        assert!(std::error::Error::source(&registry).is_some());

        let server = RuntimeProviderBrokerLauncherErrorV1::Server(
            RuntimeProviderBrokerServerErrorV1::BindingMismatch,
        );
        assert_eq!(
            server.to_string(),
            "runtime-provider broker binding is not qualified"
        );
        assert!(std::error::Error::source(&server).is_some());
    }

    #[test]
    fn assembled_launcher_reports_only_public_summary() {
        let registry = RecordingRegistry::available();
        let deployment = RuntimeProviderBrokerDeploymentV1::try_new(qualified_catalog(), &registry)
            .expect("assemble public catalog");

        assert_eq!(deployment.binding_count(), 1);
        let debug = format!("{deployment:?}");
        assert!(debug.contains("sora.production"));
        assert!(debug.contains("binding_count: 1"));
        assert!(!debug.contains("hsm://sorafs/privacy/prf/primary"));
        assert!(!debug.contains("RuntimeProviderBrokerBackendsV1"));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn serve_rejects_incomplete_backend_set_before_endpoint_access() {
        let registry = RecordingRegistry::available();
        let deployment = RuntimeProviderBrokerDeploymentV1::try_new(qualified_catalog(), &registry)
            .expect("assemble public catalog");

        assert_eq!(
            deployment
                .serve()
                .expect_err("missing PRF provider must fail before serving"),
            RuntimeProviderBrokerLauncherErrorV1::Server(
                RuntimeProviderBrokerServerErrorV1::BackendSetMismatch
            )
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn pre_requested_shutdown_suppresses_readiness_callback() {
        let registry = RecordingRegistry::available();
        let deployment = RuntimeProviderBrokerDeploymentV1::try_new(qualified_catalog(), &registry)
            .expect("assemble public catalog");
        let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
        lifecycle.request_shutdown();
        let ready = AtomicBool::new(false);

        deployment
            .serve_with_lifecycle(lifecycle, || ready.store(true, Ordering::Relaxed))
            .expect("pre-start shutdown exits without endpoint access");
        assert!(!ready.load(Ordering::Relaxed));
    }
}
