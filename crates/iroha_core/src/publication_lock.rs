//! Actual storage mutex custody with release-driven local publication retries.
//!
//! The physical mutex and its notification travel together through replay swaps
//! and shared commit-lock Arcs. No raw mutex or synthetic notification escapes.

/// A physical storage fence which notifies only after its actual guard releases.
/// Parking-lot mutexes do not poison; unwind remains an ordinary release.
#[derive(Default)]
pub(crate) struct PublicationMutex<T = ()> {
    inner: parking_lot::Mutex<T>,
    released: mv::ReleaseNotification,
}

/// An original storage mutex guard, retaining the established lock-order boundary.
pub(crate) struct PublicationGuard<'state, T = ()> {
    inner: mv::ReleaseGuard<'state, PhysicalPublicationGuard<'state, T>>,
}

/// Original physical guard whose unlock policy is selected before its release.
struct PhysicalPublicationGuard<'state, T> {
    guard: Option<parking_lot::MutexGuard<'state, T>>,
    fair: bool,
}

impl<T> Drop for PhysicalPublicationGuard<'_, T> {
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

impl<T> PublicationGuard<'_, T> {
    /// Preserve QueuePlan's fair unlock before waiting for Kura publication.
    /// The outer release guard signals only after the physical unlock completes.
    pub(crate) fn unlock_fair(mut self) {
        self.inner.fair = true;
        drop(self);
    }
}

impl<T> std::ops::Deref for PublicationGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner
            .guard
            .as_deref()
            .expect("retained physical guard")
    }
}

impl<T> std::ops::DerefMut for PublicationGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner
            .guard
            .as_deref_mut()
            .expect("retained physical guard")
    }
}

impl<T> PublicationMutex<T> {
    /// Bind the original protected value and its release observation at creation.
    pub(crate) fn new(value: T) -> Self {
        Self {
            inner: parking_lot::Mutex::new(value),
            released: mv::ReleaseNotification::default(),
        }
    }

    /// Bind notification to the acquired physical owner without exposing it.
    fn wrap<'state>(
        &'state self,
        guard: parking_lot::MutexGuard<'state, T>,
    ) -> PublicationGuard<'state, T> {
        PublicationGuard {
            inner: self.released.guard(PhysicalPublicationGuard {
                guard: Some(guard),
                fair: false,
            }),
        }
    }

    /// Acquire the existing physical fence for its original synchronous caller.
    pub(crate) fn lock(&self) -> PublicationGuard<'_, T> {
        self.wrap(self.inner.lock())
    }

    /// Probe the physical fence without waiting or retaining a retry observation.
    pub(crate) fn try_lock(&self) -> Option<PublicationGuard<'_, T>> {
        self.inner.try_lock().map(|guard| self.wrap(guard))
    }

    /// Acquire the exact fence or return its pre-probe release observation.
    ///
    /// The aggregate publisher must release every earlier acquired fence before
    /// awaiting this event. A wake grants no predecessor or publication authority;
    /// retry must re-acquire the same target and rejoin its original journals.
    /// No publication, height change, timer, or scheduled polling is required.
    pub(crate) fn try_lock_or_wait(&self) -> Result<PublicationGuard<'_, T>, mv::ReleaseWait> {
        let wait = self.released.observe();
        self.try_lock().ok_or(wait)
    }
}

impl<T> std::fmt::Debug for PublicationMutex<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PublicationMutex")
            .finish_non_exhaustive()
    }
}
