//! Allocation-free, nonblocking access to optional public-key validation caches.
//!
//! Contention only removes a cache optimization. Never wait, spin, allocate lock
//! bookkeeping, or retain a borrowed cache value beyond the access closure.

use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

/// One inline cache value whose mutable access can be declined immediately.
pub(crate) struct TryCell<T> {
    held: AtomicBool,
    value: UnsafeCell<T>,
}

// SAFETY: A successful acquire CAS is the only path to the value. It excludes
// every other accessor until the closure ends and the guard releases ownership.
// The value can move between accessing threads, so it must be Send.
#[allow(unsafe_code)]
unsafe impl<T: Send> Sync for TryCell<T> {}

impl<T> TryCell<T> {
    /// Initialize fixed cache storage without runtime allocation.
    pub(crate) const fn new(value: T) -> Self {
        Self {
            held: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    /// Access the cache once, or decline immediately while another access is active.
    pub(crate) fn try_with<R>(&self, access: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()?;
        let _release = Release(&self.held);
        // SAFETY: This scope owns the successful acquire above. No other access
        // can alias this mutable reference; the closure's borrow cannot escape
        // in R. Release runs after the closure, including during unwinding.
        #[allow(unsafe_code)]
        Some(access(unsafe { &mut *self.value.get() }))
    }
}

struct Release<'a>(&'a AtomicBool);

impl Drop for Release<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::TryCell;

    #[test]
    fn active_access_excludes_reentrancy_and_other_threads() {
        let cell = TryCell::new(7);
        assert_eq!(
            cell.try_with(|value| {
                *value = 9;
                assert_eq!(cell.try_with(|_| ()), None);
                std::thread::scope(|scope| {
                    assert_eq!(scope.spawn(|| cell.try_with(|_| ())).join().unwrap(), None);
                });
                *value
            }),
            Some(9)
        );
        std::thread::scope(|scope| {
            assert_eq!(
                scope
                    .spawn(|| cell.try_with(|value| *value))
                    .join()
                    .unwrap(),
                Some(9)
            );
        });
    }

    #[test]
    fn unwinding_releases_access_and_publishes_the_value() {
        let cell = TryCell::new(0);
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cell.try_with(|value| {
                *value = 11;
                panic!("release cache access on unwind");
            });
        }));
        assert!(failed.is_err());
        assert_eq!(cell.try_with(|value| *value), Some(11));
    }
}
