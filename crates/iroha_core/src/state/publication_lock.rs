//! Actual State mutex custody with release-driven local publication retries.
//!
//! The physical mutex and its notification travel together through replay swaps
//! and shared commit-lock Arcs. No raw mutex or synthetic notification escapes.

/// A physical State fence which notifies only after its actual guard releases.
/// Parking-lot mutexes do not poison; unwind remains an ordinary release.
#[derive(Default)]
pub(super) struct StatePublicationMutex {
    inner: parking_lot::Mutex<()>,
    released: mv::ReleaseNotification,
}

/// An original State mutex guard, retaining the established lock-order boundary.
pub(crate) struct StatePublicationGuard<'state> {
    // Fields drop in declaration order: the actual mutex must unlock before
    // the notification guard signals, including on unwind.
    guard: parking_lot::MutexGuard<'state, ()>,
    _release: mv::ReleaseGuard<'state, ()>,
}

impl StatePublicationGuard<'_> {
    /// Hand the mutex to its next waiter before notifying publication retries.
    pub(super) fn unlock_fair(self) {
        let Self { guard, _release } = self;
        parking_lot::MutexGuard::unlock_fair(guard);
    }
}

impl StatePublicationMutex {
    /// Acquire the existing physical fence for its original synchronous caller.
    pub(super) fn lock(&self) -> StatePublicationGuard<'_> {
        StatePublicationGuard {
            guard: self.inner.lock(),
            _release: self.released.guard(()),
        }
    }

    /// Probe the physical fence without waiting or retaining a retry observation.
    pub(super) fn try_lock(&self) -> Option<StatePublicationGuard<'_>> {
        self.inner.try_lock().map(|guard| StatePublicationGuard {
            guard,
            _release: self.released.guard(()),
        })
    }

    /// Acquire the exact fence or return its pre-probe release observation.
    ///
    /// The aggregate publisher must release every earlier acquired fence before
    /// awaiting this event. A wake grants no predecessor or publication authority;
    /// retry must re-acquire the same target and rejoin its original journals.
    /// No publication, height change, timer, or scheduled polling is required.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "TODO: connect publication retries to the consuming State publisher"
        )
    )]
    pub(super) fn try_lock_or_wait(&self) -> Result<StatePublicationGuard<'_>, mv::ReleaseWait> {
        let wait = self.released.observe();
        self.try_lock().ok_or(wait)
    }
}

#[cfg(test)]
#[path = "publication_lock_tests.rs"]
mod tests;
