//! Panic hook suppression helpers.

use std::{cell::Cell, future::Future};

thread_local! {
    static SUPPRESSION_DEPTH: Cell<u32> = const { Cell::new(0) };
}

tokio::task_local! {
    static ASYNC_SUPPRESSION_DEPTH: u32;
}

/// RAII guard that suppresses panic hook side-effects on the current thread.
pub struct ScopedSuppressor;

impl ScopedSuppressor {
    /// Create a new scoped suppressor.
    #[must_use]
    pub fn new() -> Self {
        SUPPRESSION_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
        Self
    }
}

impl Default for ScopedSuppressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ScopedSuppressor {
    fn drop(&mut self) {
        SUPPRESSION_DEPTH.with(|depth| {
            let current = depth.get();
            debug_assert!(current > 0, "panic hook suppressor drop underflow");
            depth.set(current.saturating_sub(1));
        });
    }
}

/// Returns true if the panic hook should suspend shutdown signalling in this scope.
#[must_use]
pub fn is_suppressed() -> bool {
    SUPPRESSION_DEPTH.with(|depth| depth.get() > 0)
        || ASYNC_SUPPRESSION_DEPTH
            .try_with(|depth| *depth > 0)
            .unwrap_or(false)
}

/// Run a closure while suppressing panic hook shutdown signalling on the current thread.
pub fn with_hook_suppressed<R>(f: impl FnOnce() -> R) -> R {
    let _guard = ScopedSuppressor::new();
    f()
}

/// Poll a future with panic-hook shutdown signalling suppressed for this task.
///
/// A thread-local guard cannot safely be held across an `.await`, because the
/// runtime may resume the future on another worker. Task-local suppression
/// follows the future between workers and remains isolated from sibling tasks.
pub async fn with_hook_suppressed_async<F>(future: F) -> F::Output
where
    F: Future,
{
    let depth = ASYNC_SUPPRESSION_DEPTH
        .try_with(|depth| depth.saturating_add(1))
        .unwrap_or(1);
    ASYNC_SUPPRESSION_DEPTH.scope(depth, future).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppression_is_scoped() {
        assert!(!is_suppressed());
        {
            let _guard = ScopedSuppressor::new();
            assert!(is_suppressed());
            {
                let _nested = ScopedSuppressor::new();
                assert!(is_suppressed());
            }
            assert!(is_suppressed());
        }
        assert!(!is_suppressed());
    }

    #[test]
    fn with_hook_suppressed_runs_closure() {
        assert!(!is_suppressed());
        let value = with_hook_suppressed(|| {
            assert!(is_suppressed());
            42
        });
        assert_eq!(value, 42);
        assert!(!is_suppressed());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn async_suppression_follows_only_the_scoped_task() {
        assert!(!is_suppressed());
        with_hook_suppressed_async(async {
            tokio::task::yield_now().await;
            assert!(is_suppressed());
            with_hook_suppressed_async(async {
                tokio::task::yield_now().await;
                assert!(is_suppressed());
            })
            .await;
            assert!(is_suppressed());

            let sibling = tokio::spawn(async { is_suppressed() })
                .await
                .expect("sibling task should complete");
            assert!(!sibling, "task-local suppression must not leak");
        })
        .await;
        assert!(!is_suppressed());
    }
}
