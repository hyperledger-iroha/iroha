//! Deployment-owned assembly for the stock runtime-provider broker server.
//!
//! This boundary accepts only the sanitized public binding catalog projected
//! by [`IrohaRuntimeProviderBindingsV1`]. Provider credentials, private keys,
//! tokens, attestations, and private evidence remain encapsulated by the
//! deployment-owned backend objects returned by the registry.

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::Parser;

use super::api::{
    RuntimeProviderBrokerBackendsV1, RuntimeProviderBrokerLifecycleV1,
    RuntimeProviderBrokerServerErrorV1, serve_runtime_provider_broker_v1,
    serve_runtime_provider_broker_with_lifecycle_v1,
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::runtime_provider_registry::RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1;
use crate::runtime_provider_registry::{
    IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderCatalogErrorV1,
    IrohaRuntimeProviderRegistryErrorV1,
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

/// Credential-free command-line contract shared by deployment broker binaries.
///
/// The deployment package statically selects and constructs its concrete
/// [`RuntimeProviderBrokerBackendRegistryV1`]. The process CLI accepts only the
/// canonical public catalog path: it has no private-key, credential, dynamic
/// plugin, environment-selector, or endpoint-override argument.
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
}

impl RuntimeProviderBrokerExecutableArgsV1 {
    /// Return the operator-supplied canonical catalog path.
    #[must_use]
    pub fn catalog_path(&self) -> &Path {
        &self.catalog
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
        Self::try_from_catalog_file(args.catalog_path(), registry)
    }

    /// Load one canonical catalog file and resolve its complete backend set.
    ///
    /// # Errors
    ///
    /// Returns the same fail-closed categories as [`Self::try_from_args`].
    pub fn try_from_catalog_file(
        catalog_path: &Path,
        registry: &dyn RuntimeProviderBrokerBackendRegistryV1,
    ) -> Result<Self, RuntimeProviderBrokerExecutableErrorV1> {
        let bindings = load_runtime_provider_broker_catalog_file_v1(catalog_path)?;
        let deployment = RuntimeProviderBrokerDeploymentV1::try_new(bindings, registry)
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
    /// SIGINT/SIGTERM handling could not be installed before serving.
    SignalUnavailable,
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
            Self::SignalUnavailable => formatter
                .write_str("runtime-provider broker shutdown signal listener is unavailable"),
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
            | Self::SignalUnavailable => None,
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
        load_runtime_provider_broker_catalog_file_on_unix_v1(catalog_path)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = catalog_path;
        Err(RuntimeProviderBrokerExecutableErrorV1::UnsupportedPlatform)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeProviderCatalogFileIdentityV1 {
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
impl RuntimeProviderCatalogFileIdentityV1 {
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
        // Unit fixtures cannot install root-owned files. This branch is absent
        // from every production build; the shipped loader accepts root only.
        rustix::process::geteuid().as_raw()
    }
    #[cfg(not(test))]
    {
        0
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_runtime_provider_catalog_path_v1(
    catalog_path: &Path,
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
    let trusted_owner_uid = trusted_runtime_provider_catalog_owner_uid_v1();
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
fn load_runtime_provider_broker_catalog_file_on_unix_v1(
    catalog_path: &Path,
) -> Result<IrohaRuntimeProviderBindingsV1, RuntimeProviderBrokerExecutableErrorV1> {
    use std::io::Read as _;

    validate_runtime_provider_catalog_path_v1(catalog_path)?;
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
    let before = RuntimeProviderCatalogFileIdentityV1::from_metadata(&before_metadata);
    let trusted_owner_uid = trusted_runtime_provider_catalog_owner_uid_v1();
    if !before_metadata.is_file()
        || before.owner != trusted_owner_uid
        || before.mode & 0o7222 != 0
        || before.links != 1
    {
        return Err(RuntimeProviderBrokerExecutableErrorV1::UntrustedCatalogPath);
    }
    if before.length > RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1 as u64 {
        return Err(RuntimeProviderBrokerExecutableErrorV1::Catalog(
            IrohaRuntimeProviderCatalogErrorV1::ArtifactTooLarge,
        ));
    }
    let declared_length = usize::try_from(before.length)
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
    let mut bytes = Vec::with_capacity(declared_length);
    (&mut file)
        .take(RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1 as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
    if bytes.len() > RUNTIME_PROVIDER_CATALOG_MAX_BYTES_V1 {
        return Err(RuntimeProviderBrokerExecutableErrorV1::Catalog(
            IrohaRuntimeProviderCatalogErrorV1::ArtifactTooLarge,
        ));
    }
    let after = file
        .metadata()
        .map(|metadata| RuntimeProviderCatalogFileIdentityV1::from_metadata(&metadata))
        .map_err(|_| RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)?;
    if before != after || bytes.len() != declared_length {
        return Err(RuntimeProviderBrokerExecutableErrorV1::CatalogChanged);
    }
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use std::{fs, sync::atomic::AtomicBool};

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
    fn executable_cli_accepts_only_the_public_catalog_path() {
        let args = RuntimeProviderBrokerExecutableArgsV1::try_parse_from([
            "sorafs_runtime_provider_broker",
            "--catalog",
            "/var/lib/iroha/runtime-provider-catalog-v1.norito",
        ])
        .expect("parse the sole public launcher input");
        assert_eq!(
            args.catalog_path(),
            Path::new("/var/lib/iroha/runtime-provider-catalog-v1.norito")
        );
        assert!(
            RuntimeProviderBrokerExecutableArgsV1::try_parse_from([
                "sorafs_runtime_provider_broker"
            ])
            .is_err()
        );
        for forbidden in [
            "--socket",
            "--private-key",
            "--credential",
            "--backend-plugin",
            "--test-provider",
        ] {
            assert!(
                RuntimeProviderBrokerExecutableArgsV1::try_parse_from([
                    "sorafs_runtime_provider_broker",
                    "--catalog",
                    "/var/lib/iroha/runtime-provider-catalog-v1.norito",
                    forbidden,
                    "forbidden",
                ])
                .is_err(),
                "{forbidden} must not enter the common executable contract"
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

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn catalog_file_loader_rejects_relative_missing_and_noncanonical_input() {
        let registry = RecordingRegistry::available();
        let relative = RuntimeProviderBrokerExecutableV1::try_from_catalog_file(
            Path::new("providers.norito"),
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
            &registry,
        );
        assert!(matches!(
            missing,
            Err(RuntimeProviderBrokerExecutableErrorV1::CatalogUnavailable)
        ));
        assert_eq!(registry.calls.load(Ordering::Relaxed), 0);

        let (_directory, path) = write_catalog_file(b"not a canonical catalog");
        assert!(matches!(
            RuntimeProviderBrokerExecutableV1::try_from_catalog_file(&path, &registry),
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
    fn executable_preserves_registry_failure_and_redacts_catalog_details() {
        let bytes = qualified_catalog()
            .export_canonical_v1()
            .expect("encode catalog");
        let (_directory, path) = write_catalog_file(&bytes);
        let failing =
            RecordingRegistry::failing(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
        let args = RuntimeProviderBrokerExecutableArgsV1 {
            catalog: path.clone(),
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

        let executable = RuntimeProviderBrokerExecutableV1::try_from_catalog_file(
            &path,
            &RecordingRegistry::available(),
        )
        .expect("assemble executable shell");
        assert_eq!(executable.binding_count(), 1);
        let debug = format!("{executable:?}");
        assert!(debug.contains("binding_count: 1"));
        assert!(!debug.contains("providers.norito"));
        assert!(!debug.contains("hsm://sorafs/privacy/prf/primary"));

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
            RuntimeProviderBrokerExecutableErrorV1::SignalUnavailable,
        ] {
            assert!(!error.to_string().contains('/'));
            assert!(std::error::Error::source(&error).is_none());
        }
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
