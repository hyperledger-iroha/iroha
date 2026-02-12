use std::{
    any::Any,
    env,
    ops::{Deref, DerefMut},
    panic::{self, AssertUnwindSafe},
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::Duration,
};

use eyre::{Report, Result};
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer};
use tokio::runtime::{Handle, Runtime, RuntimeFlavor};

use crate::sync::get_status_with_retry;

/// Optional guard that limits concurrent integration tests which spin up a network.
///
/// Defaults to CPU-scaled parallelism (cores / 16). Set `IROHA_TEST_NETWORK_PARALLELISM=<N>`
/// to override or `IROHA_TEST_SERIALIZE_NETWORKS=1` to force serialization explicitly.
#[must_use]
pub struct SerialGuard {
    #[allow(dead_code)]
    guard: Option<NetworkPermit>,
}

/// Network wrapper that keeps the optional serial guard alive for the entire test scope.
pub struct SerializedNetwork {
    network: Option<Network>,
    guard: Option<SerialGuard>,
    shutdown_done: bool,
    runtime_handle: Option<Handle>,
}

impl SerializedNetwork {
    /// Wrap a network with an optional serial guard held for its full lifetime.
    pub fn new(network: Network, guard: SerialGuard) -> Self {
        Self {
            network: Some(network),
            guard: Some(guard),
            shutdown_done: false,
            runtime_handle: Handle::try_current().ok(),
        }
    }

    /// Wrap a network with an explicit runtime handle for shutdown coordination.
    pub fn new_with_handle(network: Network, guard: SerialGuard, handle: Handle) -> Self {
        Self {
            network: Some(network),
            guard: Some(guard),
            shutdown_done: false,
            runtime_handle: Some(handle),
        }
    }

    /// Shut down running peers before releasing the optional serial guard.
    pub fn shutdown_blocking(mut self) {
        self.shutdown_done = true;
        self.shutdown_blocking_inner();
    }

    fn shutdown_blocking_inner(&mut self) {
        let Some(network) = self.network.take() else {
            return;
        };
        let Some(guard) = self.guard.take() else {
            return;
        };

        if !network.peers().iter().any(NetworkPeer::is_running) {
            drop(guard);
            return;
        }

        let handle = self
            .runtime_handle
            .clone()
            .or_else(|| Handle::try_current().ok());
        let runtime_flavor = handle.as_ref().map(Handle::runtime_flavor);
        let inside_runtime = Handle::try_current().is_ok();
        let shutdown = move || {
            if let Some(handle) = handle {
                if matches!(handle.runtime_flavor(), RuntimeFlavor::CurrentThread) {
                    // Current-thread runtimes cannot be driven from another thread.
                    match Runtime::new() {
                        Ok(rt) => {
                            let _ = rt.block_on(network.shutdown());
                        }
                        Err(err) => {
                            eprintln!("warning: failed to create runtime for shutdown: {err}");
                        }
                    }
                } else {
                    let _ = handle.block_on(network.shutdown());
                }
            } else {
                match Runtime::new() {
                    Ok(rt) => {
                        let _ = rt.block_on(network.shutdown());
                    }
                    Err(err) => {
                        eprintln!("warning: failed to create runtime for shutdown: {err}");
                    }
                }
            }
            drop(guard);
        };

        if inside_runtime && matches!(runtime_flavor, Some(RuntimeFlavor::CurrentThread)) {
            // Avoid blocking the current-thread runtime; release the guard after shutdown.
            std::thread::spawn(shutdown);
            return;
        }
        run_shutdown_blocking(shutdown);
    }
}

fn run_shutdown_blocking<F>(shutdown: F)
where
    F: FnOnce() + Send,
{
    if Handle::try_current().is_ok() {
        std::thread::scope(|scope| {
            scope.spawn(shutdown);
        });
        return;
    }
    shutdown();
}

impl Deref for SerializedNetwork {
    type Target = Network;

    fn deref(&self) -> &Self::Target {
        self.network.as_ref().expect("serialized network missing")
    }
}

impl DerefMut for SerializedNetwork {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.network.as_mut().expect("serialized network missing")
    }
}

impl Drop for SerializedNetwork {
    fn drop(&mut self) {
        if self.shutdown_done {
            return;
        }
        self.shutdown_blocking_inner();
    }
}

struct NetworkPermit {
    state: Arc<NetworkPermitState>,
}

struct NetworkPermitState {
    state: Mutex<PermitState>,
    cvar: Condvar,
}

struct PermitState {
    limit: usize,
    in_use: usize,
}

impl NetworkPermitState {
    fn new() -> Self {
        Self {
            state: Mutex::new(PermitState {
                limit: network_parallelism_limit(),
                in_use: 0,
            }),
            cvar: Condvar::new(),
        }
    }
}

impl Drop for NetworkPermit {
    fn drop(&mut self) {
        let mut state = self
            .state
            .state
            .lock()
            .expect("network permit mutex poisoned");
        state.in_use = state.in_use.saturating_sub(1);
        self.state.cvar.notify_one();
    }
}

const SERIAL_GUARD_LOG_INTERVAL: Duration = Duration::from_secs(60);
const SERIAL_GUARD_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MIN_NETWORK_PEERS: usize = 4; // DA-enabled consensus can stall with fewer peers.
const DEFAULT_NETWORK_PARALLELISM_PEERS: usize = 64; // Match iroha_test_network default.
const SERIALIZE_NETWORKS_ENV: &str = "IROHA_TEST_SERIALIZE_NETWORKS";
const NETWORK_PARALLELISM_ENV: &str = "IROHA_TEST_NETWORK_PARALLELISM";

fn network_permit_state() -> &'static Arc<NetworkPermitState> {
    static NETWORK_STATE: OnceLock<Arc<NetworkPermitState>> = OnceLock::new();
    NETWORK_STATE.get_or_init(|| Arc::new(NetworkPermitState::new()))
}

fn serialize_networks_enabled() -> bool {
    if let Some(value) = test_override_serialization() {
        return value;
    }
    let Ok(raw) = env::var(SERIALIZE_NETWORKS_ENV) else {
        return false;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Clone, Copy, Default)]
struct TestOverrides {
    serialize: Option<bool>,
    parallelism: Option<usize>,
}

/// Guard that restores network-parallelism overrides on drop.
#[must_use]
pub struct NetworkParallelismGuard {
    previous: TestOverrides,
}

fn test_override_state() -> &'static Mutex<TestOverrides> {
    static OVERRIDES: OnceLock<Mutex<TestOverrides>> = OnceLock::new();
    OVERRIDES.get_or_init(|| Mutex::new(TestOverrides::default()))
}

fn test_override_serialization() -> Option<bool> {
    let guard = test_override_state()
        .lock()
        .expect("test override mutex poisoned");
    guard.serialize
}

fn test_override_parallelism() -> Option<usize> {
    let guard = test_override_state()
        .lock()
        .expect("test override mutex poisoned");
    guard.parallelism
}

/// Override network-parallelism limits for integration tests.
///
/// This is a global override; keep the returned guard alive for the duration of the override.
pub fn override_network_parallelism(
    serialize: Option<bool>,
    parallelism: Option<usize>,
) -> NetworkParallelismGuard {
    let mut guard = test_override_state()
        .lock()
        .expect("test override mutex poisoned");
    let previous = *guard;
    guard.serialize = serialize;
    guard.parallelism = parallelism;
    NetworkParallelismGuard { previous }
}

impl Drop for NetworkParallelismGuard {
    fn drop(&mut self) {
        let mut guard = test_override_state()
            .lock()
            .expect("test override mutex poisoned");
        *guard = self.previous;
    }
}

fn default_network_parallelism() -> usize {
    // Keep parallelism conservative by scaling with cores per minimal test network size.
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let per_network = DEFAULT_NETWORK_PARALLELISM_PEERS.max(1);
    cores.saturating_div(per_network).max(1)
}

fn network_parallelism_limit() -> usize {
    if serialize_networks_enabled() {
        return 1;
    }
    if let Some(value) = test_override_parallelism().filter(|value| *value > 0) {
        return value;
    }
    if let Ok(raw) = env::var(NETWORK_PARALLELISM_ENV)
        && let Ok(parsed) = raw.trim().parse::<usize>()
        && parsed > 0
    {
        return parsed;
    }
    default_network_parallelism()
}

fn acquire_network_permit() -> NetworkPermit {
    let state = network_permit_state().clone();
    let mut waited = Duration::ZERO;
    let mut next_log = SERIAL_GUARD_LOG_INTERVAL;
    let mut guard = state.state.lock().expect("network permit mutex poisoned");
    loop {
        guard.limit = network_parallelism_limit();
        if guard.in_use < guard.limit {
            guard.in_use = guard.in_use.saturating_add(1);
            drop(guard);
            return NetworkPermit { state };
        }
        if waited >= next_log {
            eprintln!(
                "network guard has waited {waited:?}; {in_use}/{limit} permits in use",
                in_use = guard.in_use,
                limit = guard.limit
            );
            next_log = next_log.saturating_add(SERIAL_GUARD_LOG_INTERVAL);
        }
        let (next, _) = state
            .cvar
            .wait_timeout(guard, SERIAL_GUARD_POLL_INTERVAL)
            .expect("network permit mutex poisoned during wait");
        guard = next;
        waited = waited.saturating_add(SERIAL_GUARD_POLL_INTERVAL);
    }
}

/// Acquire the global integration-test permit to bound concurrent network startup.
pub fn serial_guard() -> SerialGuard {
    SerialGuard {
        guard: Some(acquire_network_permit()),
    }
}

#[allow(clippy::unused_async)]
async fn serial_guard_async() -> SerialGuard {
    serial_guard()
}

fn panic_reason(panic: &(dyn Any + Send)) -> Option<String> {
    panic
        .downcast_ref::<&str>()
        .map(std::string::ToString::to_string)
        .or_else(|| panic.downcast_ref::<String>().cloned())
}

/// Attempt to start a blocking test network; fail when the sandbox forbids binding sockets.
///
/// # Errors
///
/// Returns the underlying [`eyre::Report`] when network startup fails.
#[allow(dead_code)] // Shared helper: not every integration binary uses it.
pub fn start_network_blocking_or_skip(
    builder: NetworkBuilder,
    context: &str,
) -> Result<Option<(SerializedNetwork, Runtime)>> {
    let guard = serial_guard();
    let builder = builder
        .with_auto_populated_trusted_peers()
        .with_min_peers(MIN_NETWORK_PEERS);
    let (network, runtime) = match panic::catch_unwind(AssertUnwindSafe(|| {
        builder.build_blocking()
    })) {
        Ok(tuple) => tuple,
        Err(panic) => {
            if let Some(reason) = panic_reason(panic.as_ref())
                && is_sandbox_message(&reason)
            {
                eprintln!(
                    "sandboxed network restriction detected while running {context}; skipping network startup ({reason})"
                );
                return Ok(None);
            }
            panic::resume_unwind(panic);
        }
    };
    runtime
        .block_on(async { network.start_all().await })
        .map_err(|err| sandbox_error(err, context))?;
    runtime
        .block_on(async { network.ensure_blocks(1).await })
        .map_err(|err| sandbox_error(err.wrap_err("reach block 1"), context))?;
    get_status_with_retry(&network.client())
        .map_err(|err| sandbox_error(err.wrap_err("wait for /status"), context))?;
    Ok(Some((
        SerializedNetwork::new_with_handle(network, guard, runtime.handle().clone()),
        runtime,
    )))
}

/// Build a blocking test network without starting peers; skip when the sandbox forbids binding.
///
/// Returns `None` when sandbox restrictions prevent binding; panics for non-sandbox failures.
#[allow(dead_code)] // Shared helper: not every integration binary uses it.
pub fn build_network_blocking_or_skip(
    builder: NetworkBuilder,
    context: &str,
) -> Option<(SerializedNetwork, Runtime)> {
    let guard = serial_guard();
    let builder = builder
        .with_auto_populated_trusted_peers()
        .with_min_peers(MIN_NETWORK_PEERS);
    let (network, runtime) = match panic::catch_unwind(AssertUnwindSafe(|| {
        builder.build_blocking()
    })) {
        Ok(tuple) => tuple,
        Err(panic) => {
            if let Some(reason) = panic_reason(panic.as_ref())
                && is_sandbox_message(&reason)
            {
                eprintln!(
                    "sandboxed network restriction detected while running {context}; skipping network build ({reason})"
                );
                return None;
            }
            panic::resume_unwind(panic);
        }
    };
    Some((
        SerializedNetwork::new_with_handle(network, guard, runtime.handle().clone()),
        runtime,
    ))
}

/// Attempt to start an async test network; fail when the sandbox forbids binding sockets.
///
/// # Errors
///
/// Returns the underlying [`eyre::Report`] when the async startup fails.
#[allow(dead_code)] // Shared helper: not every integration binary uses it.
pub async fn start_network_async_or_skip(
    builder: NetworkBuilder,
    context: &str,
) -> Result<Option<SerializedNetwork>> {
    let guard = serial_guard_async().await;
    let builder = builder
        .with_auto_populated_trusted_peers()
        .with_min_peers(MIN_NETWORK_PEERS);
    let network = match panic::catch_unwind(AssertUnwindSafe(|| builder.build())) {
        Ok(network) => network,
        Err(panic) => {
            if let Some(reason) = panic_reason(panic.as_ref())
                && is_sandbox_message(&reason)
            {
                eprintln!(
                    "sandboxed network restriction detected while running {context}; skipping network startup ({reason})"
                );
                return Ok(None);
            }
            panic::resume_unwind(panic);
        }
    };
    network
        .start_all()
        .await
        .map_err(|err| sandbox_error(err, context))?;
    network
        .ensure_blocks(1)
        .await
        .map_err(|err| sandbox_error(err.wrap_err("reach block 1"), context))?;
    let client = network.client();
    let status = tokio::task::spawn_blocking(move || get_status_with_retry(&client))
        .await
        .map_err(|err| sandbox_error(Report::new(err), context))?;
    status.map_err(|err| sandbox_error(err.wrap_err("wait for /status"), context))?;
    Ok(Some(SerializedNetwork::new(network, guard)))
}

/// Build an async test network without starting peers; skip when the sandbox forbids binding.
///
/// Returns `None` when sandbox restrictions prevent binding; panics for non-sandbox failures.
#[allow(dead_code)] // Shared helper: not every integration binary uses it.
pub fn build_network_or_skip(builder: NetworkBuilder, context: &str) -> Option<SerializedNetwork> {
    let guard = serial_guard();
    let builder = builder
        .with_auto_populated_trusted_peers()
        .with_min_peers(MIN_NETWORK_PEERS);
    let network = match panic::catch_unwind(AssertUnwindSafe(|| builder.build())) {
        Ok(network) => network,
        Err(panic) => {
            if let Some(reason) = panic_reason(panic.as_ref())
                && is_sandbox_message(&reason)
            {
                eprintln!(
                    "sandboxed network restriction detected while running {context}; skipping network build ({reason})"
                );
                return None;
            }
            panic::resume_unwind(panic);
        }
    };
    Some(SerializedNetwork::new(network, guard))
}

/// Convert a result into an optional value, tagging sandbox-related denials as errors.
///
/// # Errors
///
/// Propagates the provided [`eyre::Report`], wrapping sandbox denials with a clearer message.
pub fn handle_result<T>(result: Result<T>, context: &str) -> Result<Option<T>> {
    result
        .map(Some)
        .map_err(|err| sandbox_error(err.wrap_err(context.to_string()), context))
}

/// Translate sandbox-related startup failures into explicit test failures.
///
/// # Errors
///
/// Returns the original [`eyre::Report`] when the message does not match known sandbox denials.
pub fn handle_sandbox_error<T>(err: Report, context: &str) -> Result<Option<T>> {
    Err(sandbox_error(err, context))
}

/// Extract a human-readable explanation when an error looks like a sandbox denial.
pub fn sandbox_reason(err: &Report) -> Option<String> {
    detect_sandbox_reason(err)
}

fn sandbox_error(err: Report, context: &str) -> Report {
    if let Some(reason) = detect_sandbox_reason(&err) {
        err.wrap_err(format!(
            "sandboxed network restriction detected while running {context}; integration tests must not be skipped ({reason})"
        ))
    } else {
        err
    }
}

fn detect_sandbox_reason(err: &Report) -> Option<String> {
    for cause in err.chain() {
        let text = cause.to_string();
        if is_sandbox_message(&text) {
            return Some(text);
        }
    }

    let display = err.to_string();
    if is_sandbox_message(&display) {
        return Some(display);
    }

    None
}

fn is_sandbox_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("permission denied") || lower.contains("operation not permitted")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use std::time::Instant;

    use super::*;
    use tempfile::TempDir;
    use toml::Value as TomlValue;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[allow(unsafe_code)]
    fn remove_env_var(key: &str) {
        // Safety: tests serialize env mutation with ENV_LOCK.
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[allow(unsafe_code)]
    fn set_env_var(key: &str, value: &str) {
        // Safety: tests serialize env mutation with ENV_LOCK.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    struct EnvRestore {
        key: &'static str,
        value: Option<String>,
    }

    impl EnvRestore {
        fn remove(key: &'static str) -> Self {
            let value = std::env::var(key).ok();
            remove_env_var(key);
            Self { key, value }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            set_env_var(key, value);
            Self {
                key,
                value: previous,
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.value {
                set_env_var(self.key, value);
            } else {
                remove_env_var(self.key);
            }
        }
    }

    fn lock_env_guard() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| {
                eprintln!("warning: env lock poisoned; continuing with recovered guard");
                err.into_inner()
            })
    }

    struct BuildEnvRestore {
        _reentrant_build: EnvRestore,
        _permit_dir: EnvRestore,
        _permit_dir_owner: TempDir,
    }

    fn allow_reentrant_build_guard() -> BuildEnvRestore {
        let permit_dir_owner = tempfile::tempdir().expect("sandbox permit dir");
        let permit_dir = permit_dir_owner.path().to_string_lossy().into_owned();
        BuildEnvRestore {
            _reentrant_build: EnvRestore::set("IROHA_TEST_ALLOW_REENTRANT_BUILD", "1"),
            _permit_dir: EnvRestore::set("IROHA_TEST_NETWORK_PERMIT_DIR", &permit_dir),
            _permit_dir_owner: permit_dir_owner,
        }
    }

    fn network_permit_snapshot() -> (usize, usize) {
        let state = network_permit_state();
        let guard = state.state.lock().expect("network permit mutex poisoned");
        (guard.limit, guard.in_use)
    }

    fn wait_for_network_permits_to_drain(context: &str) {
        let timeout = Duration::from_secs(30);
        let start = Instant::now();
        loop {
            let (_, in_use) = network_permit_snapshot();
            if in_use == 0 {
                return;
            }
            if start.elapsed() >= timeout {
                panic!(
                    "{context}: timed out waiting for network permits to drain; {in_use} still in use"
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn skip_if_sandboxed(test_name: &str) -> bool {
        match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
            Ok(listener) => {
                drop(listener);
                false
            }
            Err(err) => {
                if is_sandbox_message(&err.to_string())
                    || err.kind() == std::io::ErrorKind::PermissionDenied
                {
                    eprintln!(
                        "sandboxed network restriction detected while running {test_name}; skipping"
                    );
                    true
                } else {
                    false
                }
            }
        }
    }

    #[test]
    fn detects_sandbox_messages() {
        assert!(is_sandbox_message("operation not permitted"));
        assert!(sandbox_reason(&Report::msg("Permission denied")).is_some());
        let err =
            handle_result::<()>(Err(Report::msg("Operation not permitted")), "ctx").unwrap_err();
        let err_text = err.to_string();
        assert!(
            err_text.contains("sandboxed network restriction detected"),
            "permission errors should be reported as sandbox denials, got {err_text:?}"
        );

        // Non-permission path should propagate without sandbox context
        let err =
            handle_result::<()>(Err(Report::msg("peer exited unexpectedly")), "ctx").unwrap_err();
        assert!(
            !err.to_string()
                .contains("sandboxed network restriction detected"),
            "non-permission errors should not be tagged as sandbox denials"
        );
    }

    #[test]
    fn extracts_panic_reason() {
        let panic_payload: Box<dyn Any + Send> = Box::new("operation not permitted");
        assert_eq!(
            panic_reason(panic_payload.as_ref()),
            Some("operation not permitted".to_string())
        );
    }

    #[test]
    fn env_restore_roundtrips_env_var() {
        let _env_guard = lock_env_guard();
        let key = "IROHA_TEST_SANDBOX_ENV_RESTORE";
        let original = std::env::var(key).ok();

        remove_env_var(key);
        {
            let _restore = EnvRestore::remove(key);
            assert!(std::env::var(key).is_err());
        }
        assert!(std::env::var(key).is_err());

        set_env_var(key, "1");
        {
            let _restore = EnvRestore::remove(key);
            assert!(std::env::var(key).is_err());
        }
        assert_eq!(std::env::var(key).as_deref(), Ok("1"));

        match original {
            Some(value) => set_env_var(key, &value),
            None => remove_env_var(key),
        }
    }

    #[test]
    fn serialized_network_holds_serial_guard() {
        if skip_if_sandboxed("serialized_network_holds_serial_guard") {
            return;
        }
        let _env_guard = lock_env_guard();
        let _build_guard = allow_reentrant_build_guard();
        let _override_guard = override_network_parallelism(Some(true), None);
        let guard = serial_guard();
        let network = NetworkBuilder::new()
            .with_auto_populated_trusted_peers()
            .build();
        let serialized = SerializedNetwork::new(network, guard);

        let (limit, in_use) = network_permit_snapshot();
        assert_eq!(limit, 1);
        assert_eq!(in_use, 1);

        drop(serialized);
        let (_, in_use) = network_permit_snapshot();
        assert_eq!(in_use, 0);
    }

    #[test]
    fn network_permit_releases_mutex_after_acquire() {
        let _env_guard = lock_env_guard();
        let _override_guard = override_network_parallelism(None, Some(1));
        let permit = acquire_network_permit();
        let state = network_permit_state();
        assert!(
            state.state.try_lock().is_ok(),
            "network permit mutex should be released after acquiring"
        );
        drop(permit);
    }

    #[test]
    fn serialized_network_shutdown_blocking_releases_guard() {
        if skip_if_sandboxed("serialized_network_shutdown_blocking_releases_guard") {
            return;
        }
        let _env_guard = lock_env_guard();
        let _build_guard = allow_reentrant_build_guard();
        let _override_guard = override_network_parallelism(Some(true), None);
        let guard = serial_guard();
        let network = NetworkBuilder::new()
            .with_auto_populated_trusted_peers()
            .build();
        let serialized = SerializedNetwork::new(network, guard);

        let (limit, in_use) = network_permit_snapshot();
        assert_eq!(limit, 1);
        assert_eq!(in_use, 1);

        serialized.shutdown_blocking();
        let (_, in_use) = network_permit_snapshot();
        assert_eq!(in_use, 0);
    }

    #[test]
    fn serialized_network_new_with_handle_stores_handle() {
        if skip_if_sandboxed("serialized_network_new_with_handle_stores_handle") {
            return;
        }
        let _env_guard = lock_env_guard();
        let _build_guard = allow_reentrant_build_guard();
        let _override_guard = override_network_parallelism(Some(true), None);
        let guard = serial_guard();
        let network = NetworkBuilder::new()
            .with_auto_populated_trusted_peers()
            .build();
        let rt = Runtime::new().expect("runtime");
        let serialized = SerializedNetwork::new_with_handle(network, guard, rt.handle().clone());

        assert!(serialized.runtime_handle.is_some());
    }

    #[test]
    fn serialized_network_drop_completes_on_current_thread_runtime() {
        if skip_if_sandboxed("serialized_network_drop_completes_on_current_thread_runtime") {
            return;
        }
        let _env_guard = lock_env_guard();
        let _build_guard = allow_reentrant_build_guard();
        let guard = serial_guard();
        let network = NetworkBuilder::new()
            .with_auto_populated_trusted_peers()
            .with_min_peers(MIN_NETWORK_PEERS)
            .build();
        let runtime = Runtime::new().expect("runtime");
        runtime.block_on(async {
            network.start_all().await.expect("start network");
        });

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("current-thread runtime");
            let handle = rt.handle().clone();
            rt.block_on(async move {
                let serialized = SerializedNetwork::new_with_handle(network, guard, handle);
                drop(serialized);
            });
            wait_for_network_permits_to_drain(
                "serialized_network_drop_completes_on_current_thread_runtime",
            );
            let _ = tx.send(());
        });

        assert!(
            rx.recv_timeout(Duration::from_secs(120)).is_ok(),
            "serialized network drop should complete under current-thread runtime"
        );
    }

    #[test]
    fn serial_guard_applies_default_parallelism() {
        let _env_guard = lock_env_guard();
        wait_for_network_permits_to_drain("serial_guard_applies_default_parallelism");
        let _serialize_guard = EnvRestore::remove(SERIALIZE_NETWORKS_ENV);
        let _parallelism_guard = EnvRestore::remove(NETWORK_PARALLELISM_ENV);
        let _override_guard = override_network_parallelism(Some(false), None);
        let expected = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1)
            .saturating_div(DEFAULT_NETWORK_PARALLELISM_PEERS.max(1))
            .max(1);
        let (_, in_use_before) = network_permit_snapshot();
        let guard = serial_guard();
        let (limit, in_use) = network_permit_snapshot();
        assert_eq!(
            limit, expected,
            "default network parallelism should scale with CPU"
        );
        assert_eq!(in_use, in_use_before.saturating_add(1));
        drop(guard);
        let (_, in_use_after) = network_permit_snapshot();
        assert_eq!(in_use_after, in_use_before);
    }

    #[test]
    fn network_parallelism_env_override_applies() {
        let _env_guard = lock_env_guard();
        wait_for_network_permits_to_drain("network_parallelism_env_override_applies");
        let _override_guard = override_network_parallelism(None, Some(2));

        let guard = serial_guard();
        let (limit, in_use) = network_permit_snapshot();
        assert_eq!(limit, 2);
        assert_eq!(in_use, 1);
        drop(guard);
    }

    #[test]
    fn serialization_overrides_parallelism_limit() {
        let _env_guard = lock_env_guard();
        wait_for_network_permits_to_drain("serialization_overrides_parallelism_limit");
        let _override_guard = override_network_parallelism(Some(true), Some(4));

        let guard = serial_guard();
        let (limit, in_use) = network_permit_snapshot();
        assert_eq!(limit, 1);
        assert_eq!(in_use, 1);
        drop(guard);
    }

    #[test]
    fn build_network_or_skip_returns_network() {
        if skip_if_sandboxed("build_network_or_skip_returns_network") {
            return;
        }
        let _env_guard = lock_env_guard();
        let _build_guard = allow_reentrant_build_guard();
        let result = build_network_or_skip(
            NetworkBuilder::new(),
            "build_network_or_skip_returns_network",
        );
        let Some(network) = result else {
            panic!("expected a network to be built");
        };
        let mut layers = network.config_layers();
        let trusted = layers
            .next()
            .expect("trusted peers layer must be present")
            .into_owned();
        assert!(trusted.contains_key("trusted_peers"));
        let pops = trusted
            .get("trusted_peers_pop")
            .and_then(TomlValue::as_array)
            .expect("trusted_peers_pop array must be present");
        assert!(
            !pops.is_empty(),
            "trusted_peers_pop should include at least one entry"
        );
    }

    #[test]
    fn build_network_blocking_or_skip_returns_network() {
        if skip_if_sandboxed("build_network_blocking_or_skip_returns_network") {
            return;
        }
        let _env_guard = lock_env_guard();
        let _build_guard = allow_reentrant_build_guard();
        let result = build_network_blocking_or_skip(
            NetworkBuilder::new(),
            "build_network_blocking_or_skip_returns_network",
        );
        let Some((network, _rt)) = result else {
            panic!("expected a network to be built");
        };
        let mut layers = network.config_layers();
        let trusted = layers
            .next()
            .expect("trusted peers layer must be present")
            .into_owned();
        assert!(trusted.contains_key("trusted_peers"));
        let pops = trusted
            .get("trusted_peers_pop")
            .and_then(TomlValue::as_array)
            .expect("trusted_peers_pop array must be present");
        assert!(
            !pops.is_empty(),
            "trusted_peers_pop should include at least one entry"
        );
    }

    #[test]
    fn start_network_blocking_or_skip_includes_trusted_peer_pops() -> Result<()> {
        if skip_if_sandboxed("start_network_blocking_or_skip_includes_trusted_peer_pops") {
            return Ok(());
        }
        let _env_guard = lock_env_guard();
        let _build_guard = allow_reentrant_build_guard();
        let result = start_network_blocking_or_skip(
            NetworkBuilder::new(),
            "start_network_blocking_or_skip_includes_trusted_peer_pops",
        )?;
        let Some((network, _rt)) = result else {
            return Ok(());
        };
        let mut layers = network.config_layers();
        let trusted = layers
            .next()
            .expect("trusted peers layer must be present")
            .into_owned();
        assert!(trusted.contains_key("trusted_peers"));
        let pops = trusted
            .get("trusted_peers_pop")
            .and_then(TomlValue::as_array)
            .expect("trusted_peers_pop array must be present");
        assert!(
            !pops.is_empty(),
            "trusted_peers_pop should include at least one entry"
        );
        Ok(())
    }

    #[tokio::test]
    async fn start_network_async_or_skip_includes_trusted_peer_pops() -> Result<()> {
        if skip_if_sandboxed("start_network_async_or_skip_includes_trusted_peer_pops") {
            return Ok(());
        }
        let _env_guard = lock_env_guard();
        let _build_guard = allow_reentrant_build_guard();
        let result = start_network_async_or_skip(
            NetworkBuilder::new(),
            "start_network_async_or_skip_includes_trusted_peer_pops",
        )
        .await?;
        let Some(network) = result else {
            return Ok(());
        };
        let mut layers = network.config_layers();
        let trusted = layers
            .next()
            .expect("trusted peers layer must be present")
            .into_owned();
        assert!(trusted.contains_key("trusted_peers"));
        let pops = trusted
            .get("trusted_peers_pop")
            .and_then(TomlValue::as_array)
            .expect("trusted_peers_pop array must be present");
        assert!(
            !pops.is_empty(),
            "trusted_peers_pop should include at least one entry"
        );
        network.shutdown().await;
        Ok(())
    }

    #[test]
    fn run_shutdown_blocking_executes_outside_runtime() {
        let counter = AtomicUsize::new(0);
        run_shutdown_blocking(|| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn run_shutdown_blocking_executes_inside_runtime() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_handle = Arc::clone(&counter);
        let rt = Runtime::new().expect("runtime");
        rt.block_on(async {
            run_shutdown_blocking(move || {
                counter_handle.fetch_add(1, Ordering::SeqCst);
            });
        });
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
