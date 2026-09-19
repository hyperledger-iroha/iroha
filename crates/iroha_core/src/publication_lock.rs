//! Actual storage mutex custody with release-driven local publication retries.
//!
//! The physical mutex and its notification travel together through replay swaps
//! and shared commit-lock Arcs. No raw mutex or synthetic notification escapes.

/// A physical storage fence which notifies only after its actual guard releases.
/// Parking-lot mutexes do not poison; unwind remains an ordinary release.
#[derive(Default)]
pub(crate) struct PublicationMutex {
    inner: parking_lot::Mutex<()>,
    released: mv::ReleaseNotification,
}

/// An original storage mutex guard, retaining the established lock-order boundary.
pub(crate) struct PublicationGuard<'state> {
    inner: mv::ReleaseGuard<'state, PhysicalPublicationGuard<'state>>,
}

/// Original physical guard whose unlock policy is selected before its release.
struct PhysicalPublicationGuard<'state> {
    guard: Option<parking_lot::MutexGuard<'state, ()>>,
    fair: bool,
}

impl Drop for PhysicalPublicationGuard<'_> {
    fn drop(&mut self) {
        if let Some(guard) = self.guard.take() {
            if self.fair {
                parking_lot::MutexGuard::unlock_fair(guard);
            } else {
                drop(guard);
            }
        }
    }
}

impl PublicationGuard<'_> {
    /// Preserve QueuePlan's fair unlock before waiting for Kura publication.
    /// The outer release guard signals only after the physical unlock completes.
    pub(crate) fn unlock_fair(mut self) {
        self.inner.fair = true;
        drop(self);
    }
}

impl PublicationMutex {
    /// Bind notification to the acquired physical owner without exposing it.
    fn wrap<'state>(
        &'state self,
        guard: parking_lot::MutexGuard<'state, ()>,
    ) -> PublicationGuard<'state> {
        PublicationGuard {
            inner: self.released.guard(PhysicalPublicationGuard {
                guard: Some(guard),
                fair: false,
            }),
        }
    }

    /// Acquire the existing physical fence for its original synchronous caller.
    pub(crate) fn lock(&self) -> PublicationGuard<'_> {
        self.wrap(self.inner.lock())
    }

    /// Probe the physical fence without waiting or retaining a retry observation.
    pub(crate) fn try_lock(&self) -> Option<PublicationGuard<'_>> {
        self.inner.try_lock().map(|guard| self.wrap(guard))
    }

    /// Acquire the exact fence or return its pre-probe release observation.
    ///
    /// The aggregate publisher must release every earlier acquired fence before
    /// awaiting this event. A wake grants no predecessor or publication authority;
    /// retry must re-acquire the same target and rejoin its original journals.
    /// No publication, height change, timer, or scheduled polling is required.
    pub(crate) fn try_lock_or_wait(&self) -> Result<PublicationGuard<'_>, mv::ReleaseWait> {
        let wait = self.released.observe();
        self.try_lock().ok_or(wait)
    }
}

impl std::fmt::Debug for PublicationMutex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PublicationMutex")
            .finish_non_exhaustive()
    }
}
