use std::{
    any::Any,
    env,
    ops::{Deref, DerefMut},
    panic::{self, AssertUnwindSafe},
    sync::{Arc, OnceLock},
    thread,
    time::Duration,
};

use eyre::{Report, Result};
use iroha_test_network::{Network, NetworkBuilder};
use tokio::{
    runtime::{Handle, Runtime},
    sync::{Mutex, OwnedMutexGuard},
};

/// Optional guard that serializes integration tests which spin up a network.
///
/// Set `IROHA_TEST_SERIALIZE_NETWORKS=1` to enable serialization.
#[must_use]
pub struct SerialGuard {
    _guard: Option<OwnedMutexGuard<()>>,
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
        if self.network.peers().iter().any(|peer| peer.is_running()) {
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

const SERIAL_GUARD_LOG_INTERVAL: Duration = Duration::from_secs(60);
const SERIAL_GUARD_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MIN_NETWORK_PEERS: usize = 4; // DA-enabled consensus can stall with fewer peers.
const SERIALIZE_NETWORKS_ENV: &str = "IROHA_TEST_SERIALIZE_NETWORKS";

fn serial_lock() -> &'static Arc<Mutex<()>> {
    static SERIAL_LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
    SERIAL_LOCK.get_or_init(|| Arc::new(Mutex::default()))
}

fn serialize_networks_enabled() -> bool {
    let Ok(raw) = env::var(SERIALIZE_NETWORKS_ENV) else {
        return false;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Acquire the global integration-test mutex to serialize network startup when enabled.
pub fn serial_guard() -> SerialGuard {
    if !serialize_networks_enabled() {
        return SerialGuard { _guard: None };
    }
    let lock = serial_lock();
    let mut waited = Duration::ZERO;
    let mut next_log = SERIAL_GUARD_LOG_INTERVAL;
    loop {
        if let Ok(guard) = lock.clone().try_lock_owned() {
            if waited >= SERIAL_GUARD_LOG_INTERVAL {
                eprintln!(
                    "serial guard acquired the global mutex after waiting {waited:?}; continuing with serialized network startup"
                );
            }
            return SerialGuard {
                _guard: Some(guard),
            };
        }
        if waited >= next_log {
            eprintln!(
                "serial guard has waited {waited:?} to serialize network startup; continuing to wait to avoid port contention"
            );
            next_log = next_log.saturating_add(SERIAL_GUARD_LOG_INTERVAL);
        }
        thread::sleep(SERIAL_GUARD_POLL_INTERVAL);
        waited = waited.saturating_add(SERIAL_GUARD_POLL_INTERVAL);
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
    Ok(Some((SerializedNetwork::new(network, guard), runtime)))
}

/// Build a blocking test network without starting peers; skip when the sandbox forbids binding.
///
/// # Errors
///
/// Returns the underlying [`eyre::Report`] when the build panics for non-sandbox reasons.
#[allow(dead_code)] // Shared helper: not every integration binary uses it.
pub fn build_network_blocking_or_skip(
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
                    "sandboxed network restriction detected while running {context}; skipping network build ({reason})"
                );
                return Ok(None);
            }
            panic::resume_unwind(panic);
        }
    };
    Ok(Some((SerializedNetwork::new(network, guard), runtime)))
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
    Ok(Some(SerializedNetwork::new(network, guard)))
}

/// Build an async test network without starting peers; skip when the sandbox forbids binding.
///
/// # Errors
///
/// Returns the underlying [`eyre::Report`] when the build panics for non-sandbox reasons.
#[allow(dead_code)] // Shared helper: not every integration binary uses it.
pub fn build_network_or_skip(
    builder: NetworkBuilder,
    context: &str,
) -> Result<Option<SerializedNetwork>> {
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
                return Ok(None);
            }
            panic::resume_unwind(panic);
        }
    };
    Ok(Some(SerializedNetwork::new(network, guard)))
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
    use std::{
        env,
        sync::{Mutex, MutexGuard, OnceLock},
    };

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_env_guard() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var_os(key);
            set_env_var(key, value);
            Self { key, original }
        }

        fn clear(key: &'static str) -> Self {
            let original = env::var_os(key);
            remove_env_var(key);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = self.original.as_ref() {
                set_env_var(self.key, value);
            } else {
                remove_env_var(self.key);
            }
        }
    }

    fn set_env_var<K, V>(key: K, value: V)
    where
        K: AsRef<std::ffi::OsStr>,
        V: AsRef<std::ffi::OsStr>,
    {
        unsafe { env::set_var(key, value) }
    }

    fn remove_env_var<K>(key: K)
    where
        K: AsRef<std::ffi::OsStr>,
    {
        unsafe { env::remove_var(key) }
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
        let _env_guard = lock_env_guard();
        let _serialize_guard = EnvVarGuard::set(SERIALIZE_NETWORKS_ENV, "1");
        let lock = serial_lock().clone();
        let guard = serial_guard();
        let network = NetworkBuilder::new().build();
        let serialized = SerializedNetwork::new(network, guard);

        assert!(
            lock.clone().try_lock_owned().is_err(),
            "serial guard should be held while serialized network is alive"
        );

        drop(serialized);
        let released = lock
            .clone()
            .try_lock_owned()
            .expect("serial guard should release after serialized network drops");
        drop(released);
    }

    #[test]
    fn serialized_network_shutdown_blocking_releases_guard() {
        let _env_guard = lock_env_guard();
        let _serialize_guard = EnvVarGuard::set(SERIALIZE_NETWORKS_ENV, "1");
        let lock = serial_lock().clone();
        let guard = serial_guard();
        let network = NetworkBuilder::new().build();
        let serialized = SerializedNetwork::new(network, guard);

        assert!(
            lock.clone().try_lock_owned().is_err(),
            "serial guard should be held while serialized network is alive"
        );

        serialized.shutdown_blocking();
        let released = lock
            .clone()
            .try_lock_owned()
            .expect("serial guard should release after shutdown_blocking");
        drop(released);
    }

    #[test]
    fn serial_guard_is_noop_by_default() {
        let _env_guard = lock_env_guard();
        let _clear_guard = EnvVarGuard::clear(SERIALIZE_NETWORKS_ENV);
        let lock = serial_lock().clone();
        let guard = serial_guard();
        let probe = lock.clone().try_lock_owned();
        assert!(
            probe.is_ok(),
            "serial guard should be disabled unless {SERIALIZE_NETWORKS_ENV} is set"
        );
        drop(probe);
        drop(guard);
    }

    #[test]
    fn build_network_or_skip_returns_network() {
        let result = build_network_or_skip(
            NetworkBuilder::new(),
            "build_network_or_skip_returns_network",
        )
        .expect("build should succeed");
        assert!(result.is_some(), "expected a network to be built");
    }

    #[test]
    fn build_network_blocking_or_skip_returns_network() {
        let result = build_network_blocking_or_skip(
            NetworkBuilder::new(),
            "build_network_blocking_or_skip_returns_network",
        )
        .expect("build should succeed");
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
