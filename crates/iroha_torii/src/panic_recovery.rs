//! Explicit panic-recovery boundaries for request-owned work.
//!
//! These helpers are intentionally limited to operations whose API contract
//! maps a dependency/provider panic to a controlled error. Consensus and
//! supervisor invariants must remain on ordinary, unsuppressed tasks.

use std::{future::Future, panic::AssertUnwindSafe};

use futures::FutureExt as _;
use thiserror::Error;
use tokio::task::{JoinError, JoinHandle};

/// Failure returned after joining an explicitly recoverable task.
#[derive(Debug, Error)]
pub(crate) enum RecoverableTaskError {
    /// The operation panicked after the shutdown hook was suppressed.
    #[error("recoverable operation panicked")]
    Panicked,
    /// Tokio could not complete the task for a reason other than the captured panic.
    #[error("recoverable task could not be joined: {0}")]
    Join(#[source] JoinError),
}

/// Run blocking request work with suppression installed on its physical worker.
pub(crate) fn spawn_blocking_recoverable<F, T>(operation: F) -> JoinHandle<std::thread::Result<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || iroha_core::panic_hook::catch_unwind_suppressed(operation))
}

/// Poll asynchronous request work in the current task with shutdown-hook
/// suppression and convert a panic into an opaque error payload.
pub(crate) async fn catch_async_recoverable<F, T>(future: F) -> std::thread::Result<T>
where
    F: Future<Output = T>,
{
    let guarded = iroha_core::panic_hook::with_hook_suppressed_async(future);
    AssertUnwindSafe(guarded).catch_unwind().await
}

/// Run an asynchronous request task with task-local panic-hook suppression.
pub(crate) fn spawn_joined_recoverable<F, T>(future: F) -> JoinHandle<std::thread::Result<T>>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    tokio::spawn(catch_async_recoverable(future))
}

/// Join an explicitly recoverable task without exposing its panic payload.
pub(crate) async fn join_recoverable<T>(
    task: JoinHandle<std::thread::Result<T>>,
) -> Result<T, RecoverableTaskError> {
    match task.await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err(RecoverableTaskError::Panicked),
        Err(error) => Err(RecoverableTaskError::Join(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blocking_panic_is_controlled_and_suppression_clears() {
        let task = spawn_blocking_recoverable(|| {
            assert!(iroha_core::panic_hook::is_suppressed());
            panic!("injected recoverable worker panic");
        });
        assert!(matches!(
            join_recoverable(task).await,
            Err(RecoverableTaskError::Panicked)
        ));
        let stale = tokio::task::spawn_blocking(iroha_core::panic_hook::is_suppressed)
            .await
            .expect("blocking worker probe must join");
        assert!(!stale);
    }

    #[tokio::test]
    async fn joined_async_panic_is_controlled() {
        let task = spawn_joined_recoverable(async {
            assert!(iroha_core::panic_hook::is_suppressed());
            panic!("injected recoverable async panic");
        });
        assert!(matches!(
            join_recoverable(task).await,
            Err(RecoverableTaskError::Panicked)
        ));
    }

    #[tokio::test]
    async fn upgrade_callback_panic_is_controlled_and_drops_owned_cleanup() {
        struct CleanupProbe(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for CleanupProbe {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::Release);
            }
        }

        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cleanup = CleanupProbe(std::sync::Arc::clone(&dropped));
        let result = catch_async_recoverable(async move {
            let _cleanup = cleanup;
            assert!(iroha_core::panic_hook::is_suppressed());
            panic!("injected WebSocket upgrade callback panic");
        })
        .await;
        assert!(result.is_err());
        assert!(dropped.load(std::sync::atomic::Ordering::Acquire));
        assert!(!iroha_core::panic_hook::is_suppressed());
    }

    #[test]
    fn ordinary_invariant_panics_remain_unsuppressed() {
        let result = std::panic::catch_unwind(|| {
            assert!(!iroha_core::panic_hook::is_suppressed());
            panic!("injected invariant panic");
        });
        assert!(result.is_err());
    }
}
