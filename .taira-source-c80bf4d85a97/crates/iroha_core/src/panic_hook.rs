//! Panic hook suppression helpers.

use std::cell::Cell;

thread_local! {
    static SUPPRESSION_DEPTH: Cell<u32> = const { Cell::new(0) };
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

/// Returns true if the panic hook should suspend shutdown signalling on this thread.
#[must_use]
pub fn is_suppressed() -> bool {
    SUPPRESSION_DEPTH.with(|depth| depth.get() > 0)
}

/// Run a closure while suppressing panic hook shutdown signalling on the current thread.
pub fn with_hook_suppressed<R>(f: impl FnOnce() -> R) -> R {
    let _guard = ScopedSuppressor::new();
    f()
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
}
