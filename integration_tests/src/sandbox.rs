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
use tokio::runtime::{Handle, Runtime};

use crate::sync::get_status_with_retry;

/// Optional guard that limits concurrent integration tests which spin up a network.
///
/// Defaults to CPU-scaled parallelism (cores / 4). Set `IROHA_TEST_NETWORK_PARALLELISM=<N>`
/// to override or `IROHA_TEST_SERIALIZE_NETWORKS=1` to force serialization explicitly.
#[must_use]
pub struct SerialGuard {
    _guard: Option<NetworkPermit>,
}

/// Network wrapper that keeps the optional serial guard alive for the entire test scope.
pub struct SerializedNetwork {
    network: Network,
    _guard: SerialGuard,
    shutdown_done: bool,
}

impl SerializedNetwork {
    /// Wrap a network with an optional serial guard held for its full lifetime.
    pub fn new(network: Network, guard: SerialGuard) -> Self {
        Self {
            network,
            _guard: guard,
            shutdown_done: false,
        }
    }

    /// Shut down running peers before releasing the optional serial guard.
    pub fn shutdown_blocking(mut self) {
        self.shutdown_done = true;
        self.shutdown_blocking_inner();
    }

    fn shutdown_blocking_inner(&self) {
        if self.network.peers().iter().any(NetworkPeer::is_running) {
            let shutdown = || match Runtime::new() {
                Ok(rt) => {
                    let _ = rt.block_on(self.network.shutdown());
                }
                Err(err) => {
                    eprintln!("warning: failed to create runtime for shutdown: {err}");
                }
            };
            run_shutdown_blocking(shutdown);
        }
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
        &self.network
    }
}

impl DerefMut for SerializedNetwork {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.network
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
const DEFAULT_NETWORK_PARALLELISM_PEERS: usize = 4; // Match iroha_test_network default.
const SERIALIZE_NETWORKS_ENV: &str = "IROHA_TEST_SERIALIZE_NETWORKS";
const NETWORK_PARALLELISM_ENV: &str = "IROHA_TEST_NETWORK_PARALLELISM";

fn network_permit_state() -> &'static Arc<NetworkPermitState> {
    static NETWORK_STATE: OnceLock<Arc<NetworkPermitState>> = OnceLock::new();
    NETWORK_STATE.get_or_init(|| Arc::new(NetworkPermitState::new()))
}

fn serialize_networks_enabled() -> bool {
    #[cfg(test)]
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

#[cfg(test)]
#[derive(Clone, Copy, Default)]
struct TestOverrides {
    serialize: Option<bool>,
    parallelism: Option<usize>,
}

#[cfg(test)]
struct OverrideGuard {
    previous: TestOverrides,
}

#[cfg(test)]
fn test_override_state() -> &'static Mutex<TestOverrides> {
    static OVERRIDES: OnceLock<Mutex<TestOverrides>> = OnceLock::new();
    OVERRIDES.get_or_init(|| Mutex::new(TestOverrides::default()))
}

#[cfg(test)]
fn test_override_serialization() -> Option<bool> {
    let guard = test_override_state()
        .lock()
        .expect("test override mutex poisoned");
    guard.serialize
}

#[cfg(test)]
fn test_override_parallelism() -> Option<usize> {
    let guard = test_override_state()
        .lock()
        .expect("test override mutex poisoned");
    guard.parallelism
}

#[cfg(test)]
fn set_test_overrides(serialize: Option<bool>, parallelism: Option<usize>) -> OverrideGuard {
    let mut guard = test_override_state()
        .lock()
        .expect("test override mutex poisoned");
    let previous = *guard;
    guard.serialize = serialize;
    guard.parallelism = parallelism;
    OverrideGuard { previous }
}

#[cfg(test)]
impl Drop for OverrideGuard {
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
    #[cfg(test)]
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
        _guard: Some(acquire_network_permit()),
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
    let builder = builder.with_min_peers(MIN_NETWORK_PEERS);
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
    Ok(Some((SerializedNetwork::new(network, guard), runtime)))
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
    let builder = builder.with_min_peers(MIN_NETWORK_PEERS);
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
    Some((SerializedNetwork::new(network, guard), runtime))
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
    let builder = builder.with_min_peers(MIN_NETWORK_PEERS);
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
    let builder = builder.with_min_peers(MIN_NETWORK_PEERS);
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

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvRestore {
        key: &'static str,
        value: Option<String>,
    }

    impl EnvRestore {
        fn remove(key: &'static str) -> Self {
            let value = std::env::var(key).ok();
            // SAFETY: tests serialize env mutations via `lock_env_guard`.
            unsafe { std::env::remove_var(key) };
            Self { key, value }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.value {
                // SAFETY: tests serialize env mutations via `lock_env_guard`.
                unsafe { std::env::set_var(self.key, value) };
            } else {
                // SAFETY: tests serialize env mutations via `lock_env_guard`.
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }

    fn lock_env_guard() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn network_permit_snapshot() -> (usize, usize) {
        let state = network_permit_state();
        let guard = state.state.lock().expect("network permit mutex poisoned");
        (guard.limit, guard.in_use)
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
    fn serialized_network_holds_serial_guard() {
        if skip_if_sandboxed("serialized_network_holds_serial_guard") {
            return;
        }
        let _env_guard = lock_env_guard();
        let _override_guard = set_test_overrides(Some(true), None);
        let guard = serial_guard();
        let network = NetworkBuilder::new().build();
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
        let _override_guard = set_test_overrides(None, Some(1));
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
        let _override_guard = set_test_overrides(Some(true), None);
        let guard = serial_guard();
        let network = NetworkBuilder::new().build();
        let serialized = SerializedNetwork::new(network, guard);

        let (limit, in_use) = network_permit_snapshot();
        assert_eq!(limit, 1);
        assert_eq!(in_use, 1);

        serialized.shutdown_blocking();
        let (_, in_use) = network_permit_snapshot();
        assert_eq!(in_use, 0);
    }

    #[test]
    fn serial_guard_applies_default_parallelism() {
        let _env_guard = lock_env_guard();
        let _serialize_guard = EnvRestore::remove(SERIALIZE_NETWORKS_ENV);
        let _parallelism_guard = EnvRestore::remove(NETWORK_PARALLELISM_ENV);
        let _override_guard = set_test_overrides(Some(false), None);
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
        let _override_guard = set_test_overrides(None, Some(2));

        let guard = serial_guard();
        let (limit, in_use) = network_permit_snapshot();
        assert_eq!(limit, 2);
        assert_eq!(in_use, 1);
        drop(guard);
    }

    #[test]
    fn serialization_overrides_parallelism_limit() {
        let _env_guard = lock_env_guard();
        let _override_guard = set_test_overrides(Some(true), Some(4));

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
        let result = build_network_or_skip(
            NetworkBuilder::new(),
            "build_network_or_skip_returns_network",
        );
        assert!(result.is_some(), "expected a network to be built");
    }

    #[test]
    fn build_network_blocking_or_skip_returns_network() {
        if skip_if_sandboxed("build_network_blocking_or_skip_returns_network") {
            return;
        }
        let _env_guard = lock_env_guard();
        let result = build_network_blocking_or_skip(
            NetworkBuilder::new(),
            "build_network_blocking_or_skip_returns_network",
        );
        assert!(result.is_some(), "expected a network to be built");
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
