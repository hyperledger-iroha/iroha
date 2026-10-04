//! Deployment-owned assembly for the stock runtime-provider broker server.
//!
//! This boundary accepts only the sanitized public binding catalog projected
//! by [`IrohaRuntimeProviderBindingsV1`]. Provider credentials, private keys,
//! tokens, attestations, and private evidence remain encapsulated by the
//! deployment-owned backend objects returned by the registry.
use super::api::{
    RuntimeProviderBrokerBackendsV1, RuntimeProviderBrokerLifecycleV1,
    RuntimeProviderBrokerReadinessErrorV1, RuntimeProviderBrokerServerErrorV1,
    serve_runtime_provider_broker_v1, serve_runtime_provider_broker_with_fallible_readiness_v1,
    serve_runtime_provider_broker_with_lifecycle_v1,
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::runtime_provider_registry::RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1;
use crate::runtime_provider_registry::{
    IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderCatalogErrorV1,
    IrohaRuntimeProviderRegistryErrorV1,
};
use clap::Parser;
use iroha_config::parameters::actual::RuntimeProviderBroker;
use std::{
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
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
    policy: RuntimeProviderBroker,
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
        policy: RuntimeProviderBroker,
        registry: &dyn RuntimeProviderBrokerBackendRegistryV1,
    ) -> Result<Self, RuntimeProviderBrokerLauncherErrorV1> {
        if bindings.is_empty() {
            return Err(RuntimeProviderBrokerLauncherErrorV1::EmptyCatalog);
        }
        let backends = registry
            .resolve(&bindings)
            .map_err(RuntimeProviderBrokerLauncherErrorV1::BackendRegistry)?;
        Ok(Self {
            bindings,
            policy,
            backends,
        })
    }
    /// Return the number of exact public provider bindings to be served.
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
    /// Qualify every backend and serve on the configured authenticated
    /// endpoint until the server stops.
    ///
    /// Clients may request only canonical non-empty subsets of this exact
    /// qualified catalog, and every operation remains confined to the subset
    /// authenticated for that session.
    ///
    /// # Errors
    ///
    /// Returns a payload-free server error before accepting clients when the
    /// resolved set is missing, extra, substituted, stale, revoked,
    /// test-marked, or live qualification otherwise fails.
    pub fn serve(self) -> Result<(), RuntimeProviderBrokerLauncherErrorV1> {
        serve_runtime_provider_broker_v1(&self.bindings, &self.policy, self.backends)
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
            &self.policy,
            self.backends,
            lifecycle,
            on_ready,
        )
        .map_err(RuntimeProviderBrokerLauncherErrorV1::Server)
    }
    /// Qualify every backend and serve with a fallible readiness publication.
    ///
    /// The broker remains in its starting state until `on_ready` returns
    /// successfully. A callback failure requests shutdown, removes the bound
    /// endpoint before the accept loop, and returns a payload-free server error.
    ///
    /// # Errors
    ///
    /// Returns the same fail-closed categories as [`Self::serve_with_lifecycle`]
    /// plus [`RuntimeProviderBrokerServerErrorV1::ReadinessUnavailable`].
    pub fn serve_with_fallible_readiness<R>(
        self,
        lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
        on_ready: R,
    ) -> Result<(), RuntimeProviderBrokerLauncherErrorV1>
    where
        R: FnOnce() -> Result<(), RuntimeProviderBrokerReadinessErrorV1>,
    {
        serve_runtime_provider_broker_with_fallible_readiness_v1(
            &self.bindings,
            &self.policy,
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
/// Credential-free command-line contract shared by deployment broker binaries.
///
/// The deployment package statically selects and constructs its concrete
/// [`RuntimeProviderBrokerBackendRegistryV1`]. The process CLI accepts only the
/// canonical public catalog and a file-configured broker policy: it has no
/// private-key, credential, dynamic plugin, or environment-selector argument.
#[derive(Clone, Debug, Parser, PartialEq, Eq)]
#[command(
    name = "sorafs_runtime_provider_broker",
    about = "Serve one exact SoraFS runtime-provider catalog",
    disable_help_subcommand = true
)]
pub struct RuntimeProviderBrokerExecutableArgsV1 {
    /// Absolute path to the canonical secret-free V1 provider catalog.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    catalog: PathBuf,
    /// Absolute path to the secret-free TOML broker policy table.
    #[arg(long = "broker-policy", value_name = "ABSOLUTE_TOML_PATH", value_parser = parse_runtime_provider_broker_policy_path_v1)]
    broker_policy: PathBuf,
}
fn parse_runtime_provider_broker_policy_path_v1(value: &str) -> Result<PathBuf, &'static str> {
    use std::path::Component;
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err("broker policy requires an absolute normal path");
    }
    Ok(path)
}
impl RuntimeProviderBrokerExecutableArgsV1 {
    /// Return the operator-supplied canonical catalog path.
    #[must_use]
    pub fn catalog_path(&self) -> &Path {
        &self.catalog
    }
    /// Return the public TOML policy path.
    #[must_use]
    pub fn broker_policy_path(&self) -> &Path {
        &self.broker_policy
    }
}
/// Fully assembled process shell for a statically linked deployment broker.
///
/// This type standardizes CLI-to-catalog loading, backend resolution,
/// readiness, shutdown, and server startup. It intentionally cannot discover
/// or dynamically load provider implementations. A deployment package must
/// statically supply a reviewed [`RuntimeProviderBrokerBackendRegistryV1`]
/// whose objects retain all credentials and private material internally.
pub struct RuntimeProviderBrokerExecutableV1 {
    deployment: RuntimeProviderBrokerDeploymentV1,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
}
impl RuntimeProviderBrokerExecutableV1 {
    /// Load the exact public catalog and resolve its complete backend set.
    ///
    /// # Errors
    ///
    /// Fails before backend discovery if the platform, path, file metadata,
    /// bounded read, or canonical catalog is invalid. Registry failures remain
    /// payload-free and preserve their exact category.
    pub fn try_from_args(
        args: &RuntimeProviderBrokerExecutableArgsV1,
        registry: &dyn RuntimeProviderBrokerBackendRegistryV1,
    ) -> Result<Self, RuntimeProviderBrokerExecutableErrorV1> {
        let policy = load_runtime_provider_broker_policy_file_v1(args.broker_policy_path())?;
        Self::try_from_catalog_file(args.catalog_path(), policy, registry)
    }
    /// Assemble the feature-isolated disposable broker from a canonically
    /// decoded owner-private catalog and the stock backend registry.
    ///
    /// The caller must obtain `bindings` with
    /// [`load_owner_private_runtime_provider_broker_catalog_file_v1`]. This
    /// path is unavailable in every shipping build.
    ///
    /// # Errors
    ///
    /// Preserves exact backend resolution and deployment errors.
    #[cfg(all(
        feature = "test-network-disposable-broker",
        any(target_os = "linux", target_os = "macos")
    ))]
    pub fn try_from_owner_private_catalog_v1(
        bindings: IrohaRuntimeProviderBindingsV1,
        policy: RuntimeProviderBroker,
        registry: &dyn RuntimeProviderBrokerBackendRegistryV1,
    ) -> Result<Self, RuntimeProviderBrokerExecutableErrorV1> {
        let deployment = RuntimeProviderBrokerDeploymentV1::try_new(bindings, policy, registry)
            .map_err(RuntimeProviderBrokerExecutableErrorV1::Launcher)?;
        Ok(Self {
            deployment,
            lifecycle: Arc::new(RuntimeProviderBrokerLifecycleV1::new()),
        })
    }
    /// Load one canonical catalog file and resolve its complete backend set.
    ///
    /// # Errors
    ///
    /// Returns the same fail-closed categories as [`Self::try_from_args`].
    pub fn try_from_catalog_file(
        catalog_path: &Path,
        policy: RuntimeProviderBroker,
        registry: &dyn RuntimeProviderBrokerBackendRegistryV1,
    ) -> Result<Self, RuntimeProviderBrokerExecutableErrorV1> {
        let bindings = load_runtime_provider_broker_catalog_file_v1(catalog_path)?;
        let deployment = RuntimeProviderBrokerDeploymentV1::try_new(bindings, policy, registry)
            .map_err(RuntimeProviderBrokerExecutableErrorV1::Launcher)?;
        Ok(Self {
            deployment,
            lifecycle: Arc::new(RuntimeProviderBrokerLifecycleV1::new()),
        })
    }
    /// Return the number of exact public bindings selected for this process.
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.deployment.binding_count()
    }
    /// Clone the orderly-shutdown control for an external supervisor hook.
    #[must_use]
    pub fn lifecycle(&self) -> Arc<RuntimeProviderBrokerLifecycleV1> {
        Arc::clone(&self.lifecycle)
    }
    /// Qualify the complete backend set and serve with caller-owned readiness.
    ///
    /// The callback runs only after both live qualification rounds and secure
    /// endpoint publication. Callers that integrate a native supervisor can
    /// retain [`Self::lifecycle`] and request shutdown from its signal hook.
    ///
    /// # Errors
    ///
    /// Fails before readiness for every incomplete, extra, substituted, stale,
    /// revoked, test-marked, unsupported, or endpoint-insecure deployment.
    pub fn serve<R>(self, on_ready: R) -> Result<(), RuntimeProviderBrokerExecutableErrorV1>
    where
        R: FnOnce(),
    {
        self.deployment
            .serve_with_lifecycle(self.lifecycle, on_ready)
            .map_err(RuntimeProviderBrokerExecutableErrorV1::Launcher)
    }
    /// Serve until SIGINT/SIGTERM or an external lifecycle request shuts down.
    ///
    /// This is the standard process entry for a deployment package without a
    /// native supervisor integration. Signal registration completes before
    /// provider qualification or endpoint publication.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeProviderBrokerExecutableErrorV1::SignalUnavailable`]
    /// before serving when the platform signal listener cannot be installed,
    /// or preserves the ordinary fail-closed serving category.
    pub fn serve_until_shutdown_signal<R>(
        self,
        on_ready: R,
    ) -> Result<(), RuntimeProviderBrokerExecutableErrorV1>
    where
        R: FnOnce(),
    {
        install_runtime_provider_broker_shutdown_signals_v1(Arc::clone(&self.lifecycle))?;
        self.serve(on_ready)
    }
    /// Serve under the checked-in Linux `Type=notify` systemd contract.
    ///
    /// This is the credential-free process entry expected by
    /// `iroha-runtime-provider-broker-v1.service`. It resolves the
    /// systemd-provided `NOTIFY_SOCKET` before provider qualification, installs
    /// SIGINT/SIGTERM handling, and publishes the exact `READY=1` datagram only
    /// from the broker's post-qualification, post-bind readiness callback.
    /// `NOTIFY_SOCKET` is supervisor transport metadata, not a provider or
    /// credential selector.
    ///
    /// A missing, malformed, unreachable, or disappearing notification socket
    /// fails closed. In particular, a send failure unwinds the freshly bound
    /// broker endpoint before a client can be accepted and is converted back to
    /// a payload-free error; transport-specific diagnostics remain inside the
    /// notifier boundary.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeProviderBrokerExecutableErrorV1::UnsupportedPlatform`]
    /// outside Linux,
    /// [`RuntimeProviderBrokerExecutableErrorV1::SystemdNotifyUnavailable`]
    /// when the supervisor notification boundary cannot be resolved or used,
    /// or preserves the ordinary signal and fail-closed serving categories.
    pub fn serve_until_shutdown_signal_with_systemd_notify(
        self,
    ) -> Result<(), RuntimeProviderBrokerExecutableErrorV1> {
        #[cfg(target_os = "linux")]
        {
            let notifier = RuntimeProviderBrokerSystemdNotifierV1::from_process_environment()?;
            install_runtime_provider_broker_shutdown_signals_v1(Arc::clone(&self.lifecycle))?;
            match self
                .deployment
                .serve_with_fallible_readiness(self.lifecycle, move || notifier.publish_ready())
            {
                Err(RuntimeProviderBrokerLauncherErrorV1::Server(
                    RuntimeProviderBrokerServerErrorV1::ReadinessUnavailable,
                )) => Err(RuntimeProviderBrokerExecutableErrorV1::SystemdNotifyUnavailable),
                result => result.map_err(RuntimeProviderBrokerExecutableErrorV1::Launcher),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = self;
            Err(RuntimeProviderBrokerExecutableErrorV1::UnsupportedPlatform)
        }
    }
}
impl fmt::Debug for RuntimeProviderBrokerExecutableV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProviderBrokerExecutableV1")
            .field("binding_count", &self.deployment.binding_count())
            .finish_non_exhaustive()
    }
}
/// Payload-free catalog, executable assembly, signal, or serving failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeProviderBrokerExecutableErrorV1 {
    /// The V1 authenticated local transport is unavailable on this platform.
    UnsupportedPlatform,
    /// The catalog path is relative or contains non-normal components.
    InvalidCatalogPath,
    /// The catalog path or its containing directories are not securely owned.
    UntrustedCatalogPath,
    /// The catalog file could not be opened or read safely.
    CatalogUnavailable,
    /// The opened catalog changed while it was being consumed.
    CatalogChanged,
    /// The bounded bytes are not one exact canonical public V1 catalog.
    Catalog(IrohaRuntimeProviderCatalogErrorV1),
    /// The deployment registry, live provider set, or broker server failed.
    Launcher(RuntimeProviderBrokerLauncherErrorV1),
    /// The policy path is relative or contains non-normal components.
    InvalidPolicyPath,
    /// The policy path or containing directories are not securely owned.
    UntrustedPolicyPath,
    /// The public policy file could not be opened or read safely.
    PolicyUnavailable,
    /// The opened policy changed while it was being consumed.
    PolicyChanged,
    /// The policy exceeds its fixed 16 KiB input bound.
    PolicyTooLarge,
    /// The TOML policy or its bounded public fields are invalid.
    InvalidPolicy,
    /// SIGINT/SIGTERM handling could not be installed before serving.
    SignalUnavailable,
    /// The Linux systemd readiness notification boundary was unavailable.
    SystemdNotifyUnavailable,
}
impl fmt::Display for RuntimeProviderBrokerExecutableErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter
                .write_str("runtime-provider broker executable is unsupported on this platform"),
            Self::InvalidCatalogPath => {
                formatter.write_str("runtime-provider broker catalog path is invalid")
            }
            Self::UntrustedCatalogPath => {
                formatter.write_str("runtime-provider broker catalog path is not securely owned")
            }
            Self::CatalogUnavailable => {
                formatter.write_str("runtime-provider broker catalog is unavailable")
            }
            Self::CatalogChanged => {
                formatter.write_str("runtime-provider broker catalog changed while loading")
            }
            Self::Catalog(error) => fmt::Display::fmt(error, formatter),
            Self::Launcher(error) => fmt::Display::fmt(error, formatter),
            Self::InvalidPolicyPath => {
                formatter.write_str("runtime-provider broker policy path is invalid")
            }
            Self::UntrustedPolicyPath => {
                formatter.write_str("runtime-provider broker policy path is not securely owned")
            }
            Self::PolicyUnavailable => {
                formatter.write_str("runtime-provider broker policy is unavailable")
            }
            Self::PolicyChanged => {
                formatter.write_str("runtime-provider broker policy changed while loading")
            }
            Self::PolicyTooLarge => {
                formatter.write_str("runtime-provider broker policy exceeds its input bound")
            }
            Self::InvalidPolicy => formatter.write_str("runtime-provider broker policy is invalid"),
            Self::SignalUnavailable => formatter
                .write_str("runtime-provider broker shutdown signal listener is unavailable"),
            Self::SystemdNotifyUnavailable => formatter
                .write_str("runtime-provider broker systemd notification boundary is unavailable"),
        }
    }
}
impl std::error::Error for RuntimeProviderBrokerExecutableErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Catalog(error) => Some(error),
            Self::Launcher(error) => Some(error),
            Self::UnsupportedPlatform
            | Self::InvalidCatalogPath
            | Self::UntrustedCatalogPath
            | Self::CatalogUnavailable
            | Self::CatalogChanged
            | Self::InvalidPolicyPath
            | Self::UntrustedPolicyPath
            | Self::PolicyUnavailable
            | Self::PolicyChanged
            | Self::PolicyTooLarge
            | Self::InvalidPolicy
            | Self::SignalUnavailable
            | Self::SystemdNotifyUnavailable => None,
        }
    }
}
/// Load one exact secret-free catalog from a secure absolute file path.
///
/// Linux and macOS require every path component to be a root-owned non-symlink
/// directory not writable by group/other. The final file is opened with
/// `O_NOFOLLOW`, must be a root-owned, single-link regular file with no write
/// or special mode bit, and is read through the fixed 256 KiB ceiling.
/// Windows and other platforms fail before filesystem access because V1 has no
/// authenticated local broker transport there.
///
/// # Errors
///
/// Rejects unsupported platforms, relative or non-normal paths, insecure
/// ownership/modes, symlinks, non-regular or multiply linked files, concurrent
/// mutation, oversized input, and every noncanonical catalog representation.
pub fn load_runtime_provider_broker_catalog_file_v1(
    catalog_path: &Path,
) -> Result<IrohaRuntimeProviderBindingsV1, RuntimeProviderBrokerExecutableErrorV1> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        load_runtime_provider_broker_catalog_file_on_unix_v1(
            catalog_path,
            trusted_runtime_provider_catalog_owner_uid_v1(),
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = catalog_path;
        Err(RuntimeProviderBrokerExecutableErrorV1::UnsupportedPlatform)
    }
}
/// Fixed byte ceiling for a standalone public broker TOML policy.
#[cfg(any(target_os = "linux", target_os = "macos"))]
const RUNTIME_PROVIDER_BROKER_POLICY_MAX_BYTES_V1: usize = 16 * 1024;

/// Load the public broker policy from one secure, bounded TOML file.
///
/// The file contains the fields of `[runtime_provider_broker]` directly, without
/// a section header or node configuration. The same parser and defaults as the
/// node apply, and ambient environment values never override this policy. The
/// root-owned path, no-symlink, single-link, read-only mode and stable file
/// identity checks are identical to the catalog loader; the byte ceiling is 16 KiB.
///
/// # Errors
///
/// Rejects unsupported platforms, insecure or changed files, oversized or
/// malformed TOML, unknown fields, invalid endpoints, and invalid policy bounds.
pub fn load_runtime_provider_broker_policy_file_v1(
    policy_path: &Path,
) -> Result<RuntimeProviderBroker, RuntimeProviderBrokerExecutableErrorV1> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        load_runtime_provider_broker_policy_file_on_unix_v1(
            policy_path,
            trusted_runtime_provider_catalog_owner_uid_v1(),
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = policy_path;
        Err(RuntimeProviderBrokerExecutableErrorV1::UnsupportedPlatform)
    }
}

/// Read a public policy owned by the service UID for the disposable test broker.
///
/// Only the expected owner differs from the shipping policy loader. All path,
/// file identity, input bounds, TOML field and operation-bound checks still apply.
///
/// # Errors
///
/// Rejects any untrusted file or invalid public broker policy.
#[cfg(all(
    feature = "test-network-disposable-broker",
    any(target_os = "linux", target_os = "macos")
))]
pub fn load_owner_private_runtime_provider_broker_policy_file_v1(
    policy_path: &Path,
) -> Result<RuntimeProviderBroker, RuntimeProviderBrokerExecutableErrorV1> {
    load_runtime_provider_broker_policy_file_on_unix_v1(
        policy_path,
        rustix::process::geteuid().as_raw(),
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn load_runtime_provider_broker_policy_file_on_unix_v1(
    policy_path: &Path,
    trusted_owner_uid: u32,
) -> Result<RuntimeProviderBroker, RuntimeProviderBrokerExecutableErrorV1> {
    use RuntimeProviderBrokerExecutableErrorV1 as Error;
    let bytes = read_runtime_provider_broker_public_file_on_unix_v1(
        policy_path,
        trusted_owner_uid,
        RUNTIME_PROVIDER_BROKER_POLICY_MAX_BYTES_V1,
    )
    .map_err(|error| match error {
        Error::InvalidCatalogPath => Error::InvalidPolicyPath,
        Error::UntrustedCatalogPath => Error::UntrustedPolicyPath,
        Error::CatalogUnavailable => Error::PolicyUnavailable,
        Error::CatalogChanged => Error::PolicyChanged,
        Error::Catalog(IrohaRuntimeProviderCatalogErrorV1::ArtifactTooLarge) => {
            Error::PolicyTooLarge
        }
        _ => Error::InvalidPolicy,
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::InvalidPolicy)?;
    let table = text.parse().map_err(|_| Error::InvalidPolicy)?;
    RuntimeProviderBroker::from_toml_source(iroha_config_base::toml::TomlSource::inline(table))
        .map_err(|_| Error::InvalidPolicy)
}

/// Read an owner-private public catalog only for the non-shipping disposable broker.
///
/// It retains the stock no-symlink, all-ancestor, single-link, mode, inode,
/// bounded-read, and canonical Norito checks. Only the expected file owner is
/// the local service UID rather than the host package administrator.
///
/// # Errors
///
/// Rejects any untrusted path, file identity, or catalog encoding.
#[cfg(all(
    feature = "test-network-disposable-broker",
    any(target_os = "linux", target_os = "macos")
))]
pub fn load_owner_private_runtime_provider_broker_catalog_file_v1(
    catalog_path: &Path,
) -> Result<IrohaRuntimeProviderBindingsV1, RuntimeProviderBrokerExecutableErrorV1> {
    load_runtime_provider_broker_catalog_file_on_unix_v1(
        catalog_path,
        rustix::process::geteuid().as_raw(),
    )
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeProviderPublicFileIdentityV1 {
    device: u64,
    inode: u64,
    length: u64,
    owner: u32,
    mode: u32,
    links: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
impl RuntimeProviderPublicFileIdentityV1 {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            owner: metadata.uid(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn trusted_runtime_provider_catalog_owner_uid_v1() -> u32 {
    #[cfg(test)]
    {
        // Unit fixtures cannot install root-owned files. Shipping builds and
        // the production loader in feature-isolated builds accept root only.
        rustix::process::geteuid().as_raw()
    }
    #[cfg(not(test))]
    {
        0
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_runtime_provider_public_file_path_v1(
    catalog_path: &Path,
    trusted_owner_uid: u32,
) -> Result<(), RuntimeProviderBrokerExecutableErrorV1> {
    use std::{os::unix::fs::MetadataExt as _, path::Component};
    if !catalog_path.is_absolute()
        || catalog_path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(RuntimeProviderBrokerExecutableErrorV1::InvalidCatalogPath);
    }
    let parent = catalog_path
        .parent()
        .ok_or(RuntimeProviderBrokerExecutableErrorV1::InvalidCatalogPath)?;
    for directory in parent.ancestors() {
        let metadata = std::fs::symlink_metadata(directory)
            .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || (metadata.uid() != 0 && metadata.uid() != trusted_owner_uid)
            || metadata.mode() & 0o022 != 0
        {
            return Err(RuntimeProviderBrokerExecutableErrorV1::UntrustedCatalogPath);
        }
    }
    Ok(())
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn read_runtime_provider_broker_public_file_on_unix_v1(
    catalog_path: &Path,
    trusted_owner_uid: u32,
    maximum_bytes: usize,
) -> Result<Vec<u8>, RuntimeProviderBrokerExecutableErrorV1> {
    use std::io::Read as _;
    validate_runtime_provider_public_file_path_v1(catalog_path, trusted_owner_uid)?;
    let descriptor = rustix::fs::open(
        catalog_path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
    let mut file = std::fs::File::from(descriptor);
    let before_metadata = file
        .metadata()
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
    let before = RuntimeProviderPublicFileIdentityV1::from_metadata(&before_metadata);
    if !before_metadata.is_file()
        || before.owner != trusted_owner_uid
        || before.mode & 0o7222 != 0
        || before.links != 1
    {
        return Err(RuntimeProviderBrokerExecutableErrorV1::UntrustedCatalogPath);
    }
    if before.length > maximum_bytes as u64 {
        return Err(RuntimeProviderBrokerExecutableErrorV1::Catalog(
            IrohaRuntimeProviderCatalogErrorV1::ArtifactTooLarge,
        ));
    }
    let declared_length = usize::try_from(before.length)
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
    let mut bytes = Vec::with_capacity(declared_length);
    (&mut file)
        .take(maximum_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
    if bytes.len() > maximum_bytes {
        return Err(RuntimeProviderBrokerExecutableErrorV1::Catalog(
            IrohaRuntimeProviderCatalogErrorV1::ArtifactTooLarge,
        ));
    }
    let after = file
        .metadata()
        .map(|metadata| RuntimeProviderPublicFileIdentityV1::from_metadata(&metadata))
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
    if before != after || bytes.len() != declared_length {
        return Err(RuntimeProviderBrokerExecutableErrorV1::CatalogChanged);
    }
    Ok(bytes)
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn load_runtime_provider_broker_catalog_file_on_unix_v1(
    catalog_path: &Path,
    trusted_owner_uid: u32,
) -> Result<IrohaRuntimeProviderBindingsV1, RuntimeProviderBrokerExecutableErrorV1> {
    let bytes = read_runtime_provider_broker_public_file_on_unix_v1(
        catalog_path,
        trusted_owner_uid,
        RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1,
    )?;
    IrohaRuntimeProviderBindingsV1::load_canonical_v1(&bytes)
        .map_err(RuntimeProviderBrokerExecutableErrorV1::Catalog)
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn install_runtime_provider_broker_shutdown_signals_v1(
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
) -> Result<(), RuntimeProviderBrokerExecutableErrorV1> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::SignalUnavailable)?;
    let (mut interrupt, mut terminate) = {
        let _runtime_scope = runtime.enter();
        let interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::SignalUnavailable)?;
        let terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::SignalUnavailable)?;
        (interrupt, terminate)
    };
    std::thread::Builder::new()
        .name("runtime-provider-broker-signal".to_owned())
        .spawn(move || {
            runtime.block_on(async move {
                tokio::select! {
                    _ = interrupt.recv() => {}
                    _ = terminate.recv() => {}
                }
            });
            lifecycle.request_shutdown();
        })
        .map(drop)
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::SignalUnavailable)
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn install_runtime_provider_broker_shutdown_signals_v1(
    _lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
) -> Result<(), RuntimeProviderBrokerExecutableErrorV1> {
    Err(RuntimeProviderBrokerExecutableErrorV1::UnsupportedPlatform)
}
#[cfg(any(target_os = "linux", all(test, target_os = "macos")))]
const SYSTEMD_READY_MESSAGE_V1: &[u8] = b"READY=1";
#[cfg(any(target_os = "linux", all(test, target_os = "macos")))]
struct RuntimeProviderBrokerSystemdNotifierV1 {
    socket: std::os::unix::net::UnixDatagram,
}
#[cfg(any(target_os = "linux", all(test, target_os = "macos")))]
impl RuntimeProviderBrokerSystemdNotifierV1 {
    #[cfg(target_os = "linux")]
    fn from_process_environment() -> Result<Self, RuntimeProviderBrokerExecutableErrorV1> {
        let notify_socket = std::env::var_os("NOTIFY_SOCKET")
            .ok_or(RuntimeProviderBrokerExecutableErrorV1::SystemdNotifyUnavailable)?;
        Self::try_from_notify_socket(notify_socket.as_os_str())
            .map_err(|()| RuntimeProviderBrokerExecutableErrorV1::SystemdNotifyUnavailable)
    }
    fn try_from_notify_socket(notify_socket: &std::ffi::OsStr) -> Result<Self, ()> {
        use std::os::unix::{ffi::OsStrExt as _, net::UnixDatagram};
        let raw = notify_socket.as_bytes();
        if raw.is_empty() {
            return Err(());
        }
        let socket = UnixDatagram::unbound().map_err(|_| ())?;
        socket
            .set_write_timeout(Some(std::time::Duration::from_secs(1)))
            .map_err(|_| ())?;
        if raw[0] == b'@' {
            #[cfg(target_os = "linux")]
            {
                use std::{os::linux::net::SocketAddrExt as _, os::unix::net::SocketAddr};
                if raw.len() == 1 {
                    return Err(());
                }
                let address = SocketAddr::from_abstract_name(&raw[1..]).map_err(|_| ())?;
                socket.connect_addr(&address).map_err(|_| ())?;
                return Ok(Self { socket });
            }
            #[cfg(not(target_os = "linux"))]
            return Err(());
        }
        let path = Path::new(notify_socket);
        if !path.is_absolute() {
            return Err(());
        }
        socket.connect(path).map_err(|_| ())?;
        Ok(Self { socket })
    }
    fn publish_ready(self) -> Result<(), RuntimeProviderBrokerReadinessErrorV1> {
        match self.socket.send(SYSTEMD_READY_MESSAGE_V1) {
            Ok(sent) if sent == SYSTEMD_READY_MESSAGE_V1.len() => Ok(()),
            Ok(_) | Err(_) => Err(RuntimeProviderBrokerReadinessErrorV1),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::IrohaRuntimeProviderSlotV1;
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn policy() -> RuntimeProviderBroker {
        RuntimeProviderBroker::from_toml_source(iroha_config_base::toml::TomlSource::inline(
            "".parse().expect("empty broker policy table"),
        ))
        .expect("validated default broker policy")
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use std::{fs, sync::atomic::AtomicBool};
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
            "threshold-prf://sorafs/privacy/primary",
            7,
            [0x51; 32],
        )
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn write_catalog_file(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt as _;
        let root = std::env::current_dir()
            .expect("resolve current directory")
            .canonicalize()
            .expect("canonicalize current directory");
        let directory = tempfile::Builder::new()
            .prefix("runtime-provider-broker-catalog-")
            .tempdir_in(root)
            .expect("create secure catalog directory");
        let path = directory.path().join("providers.norito");
        fs::write(&path, bytes).expect("write catalog fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400))
            .expect("secure catalog fixture");
        (directory, path)
    }
    #[test]
    fn empty_catalog_is_rejected_without_backend_discovery() {
        let registry = RecordingRegistry::available();
        let result = RuntimeProviderBrokerDeploymentV1::try_new(
            IrohaRuntimeProviderBindingsV1::empty_for_test("sora.production"),
            policy(),
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
        let result =
            RuntimeProviderBrokerDeploymentV1::try_new(qualified_catalog(), policy(), &registry);
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
    fn executable_cli_requires_catalog_and_public_policy_without_endpoint_override() {
        let base = [
            "sorafs_runtime_provider_broker",
            "--catalog",
            "/var/lib/iroha/runtime-provider-catalog-v1.norito",
            "--broker-policy",
            "/var/lib/iroha/broker-policy.toml",
        ];
        let args = RuntimeProviderBrokerExecutableArgsV1::try_parse_from(base)
            .expect("parse exact public launcher inputs");
        assert_eq!(args.catalog_path(), Path::new(base[2]));
        assert_eq!(args.broker_policy_path(), Path::new(base[4]));
        assert!(RuntimeProviderBrokerExecutableArgsV1::try_parse_from(&base[..1]).is_err());
        assert!(RuntimeProviderBrokerExecutableArgsV1::try_parse_from(&base[..3]).is_err());
        for invalid in [
            "../broker-policy.toml",
            "/var/lib/iroha/../broker-policy.toml",
        ] {
            let mut arguments = base;
            arguments[4] = invalid;
            assert!(
                RuntimeProviderBrokerExecutableArgsV1::try_parse_from(arguments).is_err(),
                "relative or non-normal policy paths fail before credential loading"
            );
        }
        for forbidden in [
            "--broker-endpoint",
            "--socket",
            "--private-key",
            "--credential",
            "--backend-plugin",
            "--test-provider",
        ] {
            assert!(
                RuntimeProviderBrokerExecutableArgsV1::try_parse_from(
                    base.into_iter().chain([forbidden, "forbidden"]),
                )
                .is_err(),
                "{forbidden} must not enter the executable contract"
            );
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn catalog_file_loader_roundtrips_exact_canonical_public_bytes() {
        let catalog = qualified_catalog();
        let bytes = catalog.export_canonical_v1().expect("encode catalog");
        let (_directory, path) = write_catalog_file(&bytes);
        let loaded = load_runtime_provider_broker_catalog_file_v1(&path)
            .expect("load secure canonical catalog");
        assert_eq!(
            loaded.export_canonical_v1().expect("re-encode catalog"),
            bytes
        );
    }
    #[cfg(all(
        feature = "test-network-disposable-broker",
        any(target_os = "linux", target_os = "macos")
    ))]
    #[test]
    fn disposable_catalog_uses_the_same_canonical_loader_and_backend_assembly() {
        let catalog = qualified_catalog();
        let bytes = catalog.export_canonical_v1().expect("encode catalog");
        let (_directory, path) = write_catalog_file(&bytes);
        let loaded = load_owner_private_runtime_provider_broker_catalog_file_v1(&path)
            .expect("load exact owner-private catalog");
        assert_eq!(loaded, catalog);
        let (_policy_directory, policy_path) =
            write_catalog_file(b"observer_operation_timeout_ms = 4000\n");
        let owner_policy = load_owner_private_runtime_provider_broker_policy_file_v1(&policy_path)
            .expect("load owner-private public policy");
        let executable = RuntimeProviderBrokerExecutableV1::try_from_owner_private_catalog_v1(
            loaded,
            owner_policy.clone(),
            &RecordingRegistry::available(),
        )
        .expect("assemble exact stock backend set");
        assert_eq!(executable.binding_count(), catalog.len());
        assert_eq!(executable.deployment.policy, owner_policy);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn catalog_file_loader_rejects_relative_missing_and_noncanonical_input() {
        let registry = RecordingRegistry::available();
        let relative = RuntimeProviderBrokerExecutableV1::try_from_catalog_file(
            Path::new("providers.norito"),
            policy(),
            &registry,
        );
        assert!(matches!(
            relative,
            Err(RuntimeProviderBrokerExecutableErrorV1::InvalidCatalogPath)
        ));
        assert_eq!(registry.calls.load(Ordering::Relaxed), 0);
        let root = std::env::current_dir()
            .expect("resolve current directory")
            .canonicalize()
            .expect("canonicalize current directory");
        let missing = RuntimeProviderBrokerExecutableV1::try_from_catalog_file(
            &root.join("absent-runtime-provider-catalog-v1.norito"),
            policy(),
            &registry,
        );
        assert!(matches!(
            missing,
            Err(RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)
        ));
        assert_eq!(registry.calls.load(Ordering::Relaxed), 0);
        let (_directory, path) = write_catalog_file(b"not a canonical catalog");
        assert!(matches!(
            RuntimeProviderBrokerExecutableV1::try_from_catalog_file(&path, policy(), &registry),
            Err(RuntimeProviderBrokerExecutableErrorV1::Catalog(
                IrohaRuntimeProviderCatalogErrorV1::NonCanonicalEncoding
            ))
        ));
        assert_eq!(registry.calls.load(Ordering::Relaxed), 0);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn catalog_file_loader_rejects_symlink_writable_and_oversized_files() {
        use std::os::unix::{fs::PermissionsExt as _, fs::symlink};
        let bytes = qualified_catalog()
            .export_canonical_v1()
            .expect("encode catalog");
        let (directory, path) = write_catalog_file(&bytes);
        let symlink_path = directory.path().join("catalog-link.norito");
        symlink(&path, &symlink_path).expect("create catalog symlink");
        assert!(matches!(
            load_runtime_provider_broker_catalog_file_v1(&symlink_path),
            Err(RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("make catalog owner writable");
        assert!(matches!(
            load_runtime_provider_broker_catalog_file_v1(&path),
            Err(RuntimeProviderBrokerExecutableErrorV1::UntrustedCatalogPath)
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o4400))
            .expect("make catalog set-user-ID");
        assert!(matches!(
            load_runtime_provider_broker_catalog_file_v1(&path),
            Err(RuntimeProviderBrokerExecutableErrorV1::UntrustedCatalogPath)
        ));
        let oversized = vec![0xA5; RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1 + 1];
        let (_oversized_directory, oversized_path) = write_catalog_file(&oversized);
        assert!(matches!(
            load_runtime_provider_broker_catalog_file_v1(&oversized_path),
            Err(RuntimeProviderBrokerExecutableErrorV1::Catalog(
                IrohaRuntimeProviderCatalogErrorV1::ArtifactTooLarge
            ))
        ));
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn policy_file_loader_preserves_defaults_and_finite_explicit_policy() {
        for (contents, milliseconds) in [
            ("", 15_000),
            ("observer_operation_timeout_ms = 1\n", 1),
            ("observer_operation_timeout_ms = 4000\n", 4_000),
            ("observer_operation_timeout_ms = 15000\n", 15_000),
        ] {
            let (_directory, path) = write_catalog_file(contents.as_bytes());
            let loaded = load_runtime_provider_broker_policy_file_v1(&path)
                .expect("load secure public policy");
            assert_eq!(
                loaded.observer_operation_timeout,
                std::time::Duration::from_millis(milliseconds)
            );
            assert_eq!(loaded.endpoint_path, policy().endpoint_path);
            assert_eq!(
                loaded.credential_max_memory_bytes,
                policy().credential_max_memory_bytes
            );
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn policy_file_loader_rejects_bad_policy_before_backend_discovery() {
        let registry = RecordingRegistry::available();
        let (_catalog_directory, catalog) = write_catalog_file(
            &qualified_catalog()
                .export_canonical_v1()
                .expect("encode catalog"),
        );
        for contents in [
            b"observer_operation_timeout_ms = 0".as_slice(),
            b"observer_operation_timeout_ms = 15001",
            b"credential_max_memory_bytes = 0",
            b"endpoint_path = 'relative.sock'",
            b"private_key = 'secret'",
            b"extends = '/missing.toml'",
            b"[runtime_provider_broker]",
            b"\xff",
            b"not TOML",
        ] {
            let (_directory, path) = write_catalog_file(contents);
            assert!(matches!(
                RuntimeProviderBrokerExecutableV1::try_from_args(
                    &RuntimeProviderBrokerExecutableArgsV1 {
                        catalog: catalog.clone(),
                        broker_policy: path
                    },
                    &registry
                ),
                Err(RuntimeProviderBrokerExecutableErrorV1::InvalidPolicy)
            ));
        }
        assert_eq!(registry.calls.load(Ordering::Relaxed), 0);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn policy_file_loader_rejects_relative_missing_symlink_writable_linked_and_oversized_files() {
        use RuntimeProviderBrokerExecutableErrorV1 as Error;
        use std::os::unix::{fs::PermissionsExt as _, fs::symlink};
        assert!(matches!(
            load_runtime_provider_broker_policy_file_v1(Path::new("policy.toml")),
            Err(Error::InvalidPolicyPath)
        ));
        let (directory, path) = write_catalog_file(b"");
        assert!(matches!(
            load_runtime_provider_broker_policy_file_v1(&directory.path().join("missing.toml")),
            Err(Error::PolicyUnavailable)
        ));
        let link = directory.path().join("policy-link.toml");
        symlink(&path, &link).expect("create policy symlink");
        assert!(matches!(
            load_runtime_provider_broker_policy_file_v1(&link),
            Err(Error::PolicyUnavailable)
        ));
        for mode in [0o600, 0o4400] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                .expect("change policy mode");
            assert!(matches!(
                load_runtime_provider_broker_policy_file_v1(&path),
                Err(Error::UntrustedPolicyPath)
            ));
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        let hard_link = directory.path().join("policy-hardlink.toml");
        fs::hard_link(&path, &hard_link).expect("create policy hard link");
        assert!(matches!(
            load_runtime_provider_broker_policy_file_v1(&path),
            Err(Error::UntrustedPolicyPath)
        ));
        let (_oversized_directory, oversized) =
            write_catalog_file(&vec![b' '; RUNTIME_PROVIDER_BROKER_POLICY_MAX_BYTES_V1 + 1]);
        assert!(matches!(
            load_runtime_provider_broker_policy_file_v1(&oversized),
            Err(Error::PolicyTooLarge)
        ));
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn executable_preserves_registry_failure_and_redacts_catalog_details() {
        let bytes = qualified_catalog()
            .export_canonical_v1()
            .expect("encode catalog");
        let (_directory, path) = write_catalog_file(&bytes);
        let failing =
            RecordingRegistry::failing(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
        let (_policy_directory, policy_path) =
            write_catalog_file(b"observer_operation_timeout_ms = 4000\n");
        let args = RuntimeProviderBrokerExecutableArgsV1 {
            catalog: path.clone(),
            broker_policy: policy_path,
        };
        assert!(matches!(
            RuntimeProviderBrokerExecutableV1::try_from_args(&args, &failing),
            Err(RuntimeProviderBrokerExecutableErrorV1::Launcher(
                RuntimeProviderBrokerLauncherErrorV1::BackendRegistry(
                    IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected
                )
            ))
        ));
        assert_eq!(failing.calls.load(Ordering::Relaxed), 1);
        let configured = RuntimeProviderBrokerExecutableV1::try_from_args(
            &args,
            &RecordingRegistry::available(),
        )
        .expect("assemble file-configured policy");
        assert_eq!(
            configured.deployment.policy.observer_operation_timeout,
            std::time::Duration::from_secs(4)
        );
        let executable = RuntimeProviderBrokerExecutableV1::try_from_catalog_file(
            &path,
            policy(),
            &RecordingRegistry::available(),
        )
        .expect("assemble executable shell");
        assert_eq!(executable.binding_count(), 1);
        assert_eq!(executable.deployment.policy, policy());
        let debug = format!("{executable:?}");
        assert!(debug.contains("binding_count: 1"));
        assert!(!debug.contains("providers.norito"));
        assert!(!debug.contains("threshold-prf://sorafs/privacy/primary"));
        let lifecycle = executable.lifecycle();
        lifecycle.request_shutdown();
        let ready = AtomicBool::new(false);
        executable
            .serve(|| ready.store(true, Ordering::Relaxed))
            .expect("pre-requested shutdown exits without endpoint access");
        assert!(!ready.load(Ordering::Relaxed));
    }
    #[test]
    fn executable_errors_are_stable_and_payload_free() {
        let catalog = RuntimeProviderBrokerExecutableErrorV1::Catalog(
            IrohaRuntimeProviderCatalogErrorV1::InvalidBinding,
        );
        assert_eq!(
            catalog.to_string(),
            "runtime-provider catalog contains an invalid binding"
        );
        assert!(std::error::Error::source(&catalog).is_some());
        for error in [
            RuntimeProviderBrokerExecutableErrorV1::UnsupportedPlatform,
            RuntimeProviderBrokerExecutableErrorV1::InvalidCatalogPath,
            RuntimeProviderBrokerExecutableErrorV1::UntrustedCatalogPath,
            RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable,
            RuntimeProviderBrokerExecutableErrorV1::CatalogChanged,
            RuntimeProviderBrokerExecutableErrorV1::InvalidPolicyPath,
            RuntimeProviderBrokerExecutableErrorV1::UntrustedPolicyPath,
            RuntimeProviderBrokerExecutableErrorV1::PolicyUnavailable,
            RuntimeProviderBrokerExecutableErrorV1::PolicyChanged,
            RuntimeProviderBrokerExecutableErrorV1::PolicyTooLarge,
            RuntimeProviderBrokerExecutableErrorV1::InvalidPolicy,
            RuntimeProviderBrokerExecutableErrorV1::SignalUnavailable,
            RuntimeProviderBrokerExecutableErrorV1::SystemdNotifyUnavailable,
        ] {
            assert!(!error.to_string().contains('/'));
            assert!(std::error::Error::source(&error).is_none());
        }
        let readiness = RuntimeProviderBrokerReadinessErrorV1;
        assert_eq!(
            readiness.to_string(),
            "runtime-provider broker readiness publication failed"
        );
        assert!(std::error::Error::source(&readiness).is_none());
        let server = RuntimeProviderBrokerServerErrorV1::ReadinessUnavailable;
        assert_eq!(
            server.to_string(),
            "runtime-provider broker readiness publication is unavailable"
        );
        assert!(std::error::Error::source(&server).is_none());
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn systemd_notifier_rejects_noncanonical_socket_addresses() {
        for address in ["", "relative.sock", "@"] {
            assert!(
                RuntimeProviderBrokerSystemdNotifierV1::try_from_notify_socket(
                    std::ffi::OsStr::new(address),
                )
                .is_err(),
                "invalid NOTIFY_SOCKET address must fail: {address:?}"
            );
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn systemd_notifier_publishes_only_exact_ready_datagram() {
        use std::os::unix::net::UnixDatagram;
        let directory =
            crate::runtime_provider_broker::socket_fixture_directory::new_broker_socket_test_directory();
        let path = directory.path().join("notify.sock");
        let notification_socket =
            UnixDatagram::bind(&path).expect("bind fake systemd notification socket");
        notification_socket
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .expect("bound fake systemd receive timeout");
        let notifier =
            RuntimeProviderBrokerSystemdNotifierV1::try_from_notify_socket(path.as_os_str())
                .expect("connect systemd notifier");
        notifier.publish_ready().expect("publish READY=1");
        let mut datagram = [0_u8; 32];
        let byte_count = notification_socket
            .recv(&mut datagram)
            .expect("receive READY=1");
        assert_eq!(&datagram[..byte_count], SYSTEMD_READY_MESSAGE_V1);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn systemd_notifier_rejects_send_after_supervisor_disappears() {
        use std::os::unix::net::UnixDatagram;
        let directory =
            crate::runtime_provider_broker::socket_fixture_directory::new_broker_socket_test_directory();
        let path = directory.path().join("notify.sock");
        let receiver = UnixDatagram::bind(&path).expect("bind fake systemd notification socket");
        let notifier =
            RuntimeProviderBrokerSystemdNotifierV1::try_from_notify_socket(path.as_os_str())
                .expect("connect systemd notifier");
        drop(receiver);
        fs::remove_file(&path).expect("remove stopped systemd notification socket");
        assert!(notifier.publish_ready().is_err());
    }
    #[test]
    fn assembled_launcher_reports_only_public_summary() {
        let registry = RecordingRegistry::available();
        let deployment =
            RuntimeProviderBrokerDeploymentV1::try_new(qualified_catalog(), policy(), &registry)
                .expect("assemble public catalog");
        assert_eq!(deployment.binding_count(), 1);
        let debug = format!("{deployment:?}");
        assert!(debug.contains("sora.production"));
        assert!(debug.contains("binding_count: 1"));
        assert!(!debug.contains("threshold-prf://sorafs/privacy/primary"));
        assert!(!debug.contains("RuntimeProviderBrokerBackendsV1"));
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn serve_rejects_incomplete_backend_set_before_endpoint_access() {
        let registry = RecordingRegistry::available();
        let deployment =
            RuntimeProviderBrokerDeploymentV1::try_new(qualified_catalog(), policy(), &registry)
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
        let deployment =
            RuntimeProviderBrokerDeploymentV1::try_new(qualified_catalog(), policy(), &registry)
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
