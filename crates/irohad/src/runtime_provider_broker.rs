//! Platform-fixed local runtime-provider broker for the stock daemon launcher.
//!
//! The broker carries only public provider bindings and operation payloads over
//! a service-UID-owned local Unix socket. Broker and clients must run under the
//! same effective service UID; the UID is pinned independently in each process
//! before endpoint access, so supplementary-group membership never authorizes
//! a peer. It never discovers endpoints or credentials. V1 uses a bounded
//! canonical Norito exchange framed by a four-byte big-endian body length, with
//! an exact-network, client-selected subset-catalog handshake and monotonically
//! ordered, session-bound requests. Every requested binding must be present
//! byte-for-byte in the server's qualified catalog, and each session may use
//! only the subset it authenticated during that handshake.
//!
//! The stock registry enumerates every current V1 provider slot explicitly;
//! unknown future role identifiers fail closed.
//!
//! Hedging/billing slots 35–40 expose only finalized query and verification,
//! External statement signing, immutable publication/readback, authoritative
//! acknowledgement reconciliation, and sealed monotonic epoch witnesses.
//! Every call is bounded and identity-qualified before and after external
//! work. Automatic hedge execution is intentionally absent from the V1 broker
//! surface and therefore remains disabled.
//!
//! Linux and macOS use peer-credential-checked Unix sockets. Other platforms,
//! including Windows V1 builds, compile the stock registry but reject every
//! non-empty broker-backed catalog until an equivalently authenticated
//! platform transport exists.
/// Launcher-facing broker lifecycle, registry, backend set, and serve boundary.
mod api;
/// Standard deployment-owned broker assembly without credential discovery.
mod launcher;
pub use api::StockRuntimeProviderBrokerRegistryV1;
pub use api::{
    BootleLanternIssuanceBrokerBackendErrorV1, BootleLanternIssuanceBrokerBackendV1,
    BoundGlobalBeaconPartialSignerBrokerBackendV1,
    BoundParliamentTlePartialReleaseSignerBrokerBackendV1, ConsensusSignerProviderQualificationV1,
    GlobalBeaconPartialSignerBrokerBackendErrorV1, GlobalBeaconPartialSignerBrokerBackendV1,
    ParliamentTlePartialReleaseSignerBrokerBackendErrorV1,
    ParliamentTlePartialReleaseSignerBrokerBackendV1, RuntimeProviderBrokerBackendsV1,
    RuntimeProviderBrokerLifecycleV1, RuntimeProviderBrokerReadinessErrorV1,
    RuntimeProviderBrokerServerErrorV1, StockGovernanceDagServiceRuntimeProviderRegistryV1,
    serve_runtime_provider_broker_v1, serve_runtime_provider_broker_with_fallible_readiness_v1,
    serve_runtime_provider_broker_with_lifecycle_v1,
};
pub use launcher::{
    RuntimeProviderBrokerBackendRegistryV1, RuntimeProviderBrokerDeploymentV1,
    RuntimeProviderBrokerExecutableArgsV1, RuntimeProviderBrokerExecutableErrorV1,
    RuntimeProviderBrokerExecutableV1, RuntimeProviderBrokerLauncherErrorV1,
    load_runtime_provider_broker_catalog_file_v1,
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod protocol {
    include!("runtime_provider_broker/protocol.rs");
}
