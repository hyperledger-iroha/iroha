//! Actual storage mutex custody with release-driven local publication retries.
//!
//! The physical mutex and its notification travel together through replay swaps
//! and shared commit-lock Arcs. No raw mutex or synthetic notification escapes.

/// A physical storage fence which notifies only after its actual guard releases.
/// Parking-lot mutexes do not poison; unwind remains an ordinary release.
#[derive(Default)]
pub(crate) struct PublicationMutex<T = ()> {
    inner: parking_lot::Mutex<T>,
    released: concread::release::ReleaseNotification,
}

/// An original storage mutex guard, retaining the established lock-order boundary.
pub(crate) struct PublicationGuard<'state, T = ()> {
    inner: concread::release::ReleaseGuard<'state, PhysicalPublicationGuard<'state, T>>,
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
    /// Unlock this original mutex into its exact source's retained release batch.
    /// A foreign batch returns the original guard without unlocking or notifying.
    pub(crate) fn try_release_into(
        self,
        batch: &mut concread::release::DeferredReleaseBatch,
    ) -> Result<(), Self> {
        self.inner
            .try_release_into(batch, drop)
            .map_err(|inner| Self { inner })
    }

    /// Unlock the actual mutex and retain its original notification until the
    /// enclosing aggregate has released every other physical fence.
    pub(crate) fn release_deferred(self) -> concread::release::DeferredRelease {
        self.inner.release_deferred(drop).1
    }

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
    /// An empty, allocation-free release batch bound to this actual mutex.
    pub(crate) fn deferred_releases(&self) -> concread::release::DeferredReleaseBatch {
        self.released.deferred_batch()
    }

    /// Bind the original protected value and its release observation at creation.
    pub(crate) fn new(value: T) -> Self {
        Self {
            inner: parking_lot::Mutex::new(value),
            released: concread::release::ReleaseNotification::default(),
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
    pub(crate) fn try_lock_or_wait(
        &self,
    ) -> Result<PublicationGuard<'_, T>, concread::release::ReleaseWait> {
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

/// Same-source release custody, retained outside the complete State owner.
/// Each short physical lock borrows this owner and records its release on Drop.
/// The batch never accepts a caller-selected notification source.
pub(crate) struct DeferredPublicationFence<'state, T> {
    mutex: &'state PublicationMutex<T>,
    releases: concread::release::DeferredReleaseBatch,
}

/// Physical guard whose original notification stays with its enclosing fence.
pub(crate) struct DeferredPublicationGuard<'fence, 'state, T> {
    guard: Option<PublicationGuard<'state, T>>,
    releases: &'fence mut concread::release::DeferredReleaseBatch,
}

impl<'state, T> DeferredPublicationFence<'state, T> {
    /// Acquire the original mutex; release notification stays in this owner.
    pub(crate) fn lock(&mut self) -> DeferredPublicationGuard<'_, 'state, T> {
        DeferredPublicationGuard {
            guard: Some(self.mutex.lock()),
            releases: &mut self.releases,
        }
    }
}

impl<T> std::ops::Deref for DeferredPublicationGuard<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.guard
            .as_deref()
            .expect("original deferred physical guard")
    }
}

impl<T> std::ops::DerefMut for DeferredPublicationGuard<'_, '_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard
            .as_deref_mut()
            .expect("original deferred physical guard")
    }
}

impl<T> Drop for DeferredPublicationGuard<'_, '_, T> {
    fn drop(&mut self) {
        if let Some(guard) = self.guard.take() {
            // Both fields are private and created together by defer_notifications.
            // The source cannot change between acquisition and physical release.
            assert!(
                guard.try_release_into(self.releases).is_ok(),
                "original fence release source"
            );
        }
    }
}

impl<T> PublicationMutex<T> {
    /// Retain all original release hints through an enclosing aggregate's Drop.
    pub(crate) fn defer_notifications(&self) -> DeferredPublicationFence<'_, T> {
        DeferredPublicationFence {
            mutex: self,
            releases: self.deferred_releases(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };

    #[test]
    fn typed_guard_notifies_after_unlock_and_preserves_value() {
        struct AcquireOnWake {
            original: Arc<PublicationMutex<Vec<u8>>>,
            observed: AtomicBool,
        }
        impl Wake for AcquireOnWake {
            fn wake(self: Arc<Self>) {
                let value = self
                    .original
                    .try_lock()
                    .expect("wake follows physical unlock");
                assert_eq!(&*value, &[1, 2]);
                self.observed.store(true, Ordering::SeqCst);
            }
        }
        let original = Arc::new(PublicationMutex::new(vec![1_u8]));
        let mut held = original.lock();
        held.push(2);
        let wait = match original.try_lock_or_wait() {
            Err(wait) => wait,
            Ok(_) => panic!("original backend is still held"),
        };
        let probe = Arc::new(AcquireOnWake {
            original: Arc::clone(&original),
            observed: AtomicBool::new(false),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut pending = Box::pin(wait.wait_for_release());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        let foreign = PublicationMutex::new(vec![8_u8]);
        drop(foreign.lock());
        assert!(!probe.observed.load(Ordering::SeqCst));
        assert!(pending.as_mut().poll(&mut context).is_pending());
        drop(held);
        assert!(probe.observed.load(Ordering::SeqCst));
        assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
    }

    #[test]
    fn typed_fair_unlock_and_release_before_poll_are_observable() {
        let original = PublicationMutex::new(String::from("original"));
        let mut held = original.lock();
        held.push_str(" retained");
        let wait = match original.try_lock_or_wait() {
            Err(wait) => wait,
            Ok(_) => panic!("original backend is still held"),
        };
        held.unlock_fair();
        let mut pending = Box::pin(wait.wait_for_release());
        assert_eq!(
            pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(())
        );
        assert_eq!(&*original.lock(), "original retained");
    }

    #[test]
    fn typed_unwind_releases_original_value_without_poisoning() {
        let original = PublicationMutex::new(vec![1_u8]);
        let mut wait = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut held = original.lock();
            held.push(3);
            wait = original.try_lock_or_wait().err();
            panic!("exercise the real backend guard's unwind release");
        }));
        assert!(result.is_err());
        let mut pending = Box::pin(
            wait.expect("held original returns its wait")
                .wait_for_release(),
        );
        assert_eq!(
            pending
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(())
        );
        let value = original
            .try_lock_or_wait()
            .ok()
            .expect("parking-lot is not poisoned");
        assert_eq!(&*value, &[1, 3]);
    }

    #[test]
    fn deferred_notification_reenters_all_original_fences_after_outer_unlock() {
        struct AcquireBoth {
            locks: [Arc<PublicationMutex>; 2],
            observed: AtomicBool,
        }
        impl Wake for AcquireBoth {
            fn wake(self: Arc<Self>) {
                let first = self.locks[0]
                    .inner
                    .try_lock()
                    .expect("first physical unlock");
                let second = self.locks[1]
                    .inner
                    .try_lock()
                    .expect("outer physical unlock");
                self.observed.store(true, Ordering::SeqCst);
                drop((first, second));
            }
        }
        for unwind in [false, true] {
            let locks = [
                Arc::new(PublicationMutex::default()),
                Arc::new(PublicationMutex::default()),
            ];
            let held = locks[0].lock();
            let outer = locks[1].lock();
            let mut pending = Box::pin(
                locks[0]
                    .try_lock_or_wait()
                    .err()
                    .unwrap()
                    .wait_for_release(),
            );
            let probe = Arc::new(AcquireBoth {
                locks: locks.clone(),
                observed: AtomicBool::new(false),
            });
            let waker = Waker::from(Arc::clone(&probe));
            assert!(
                pending
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
            let released = held.release_deferred();
            assert!(locks[0].inner.try_lock().is_some());
            assert!(!probe.observed.load(Ordering::SeqCst));
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                        // Reverse local order unlocks the enclosing owner first.
                        let _released = released;
                        let _outer = outer;
                        panic!("completion cleanup unwind");
                    }))
                    .is_err()
                );
            } else {
                drop(outer);
                drop(released);
            }
            assert!(probe.observed.load(Ordering::SeqCst));
            assert!(
                pending
                    .as_mut()
                    .poll(&mut Context::from_waker(&waker))
                    .is_ready()
            );
        }
    }
}

#[cfg(test)]
#[path = "publication_lock_scoped_tests.rs"]
mod scoped_tests;
