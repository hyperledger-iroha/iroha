//! Lock-release observations for retrying local publication without a timer.

use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll, Waker},
};

#[derive(Default)]
struct State {
    sequence: u64,
    poisoned: bool,
    waiters: Vec<Weak<Mutex<Option<Waker>>>>,
}

/// Notification source belonging to one physical lock, not a State generation.
///
/// Observe before attempting acquisition and wrap every acquired guard with
/// [`Self::guard`]. A refused acquisition can then return its observation even
/// if the blocking owner released before the caller registered an async waiter.
/// Readers that exclude writers must also be wrapped. Signals grant no mutation
/// authority: every retry must acquire the lock and authenticate its predecessor.
#[derive(Default)]
pub struct ReleaseNotification {
    state: Arc<Mutex<State>>,
}

impl ReleaseNotification {
    /// Observe releases before probing this notification's physical lock.
    pub fn observe(&self) -> ReleaseWait {
        let sequence = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sequence;
        ReleaseWait {
            state: Arc::clone(&self.state),
            sequence,
        }
    }

    /// Bind a physical guard to notification after its actual release.
    /// Read guards and locks without poisoning use this form: their unwind does
    /// not turn later ordinary contention into a poisoned-writer failure.
    pub fn guard<T>(&self, guard: T) -> ReleaseGuard<'_, T> {
        ReleaseGuard {
            inner: Some(guard),
            notification: self,
            poison_on_unwind: false,
        }
    }

    /// Bind a guard whose underlying lock becomes poisoned when it unwinds.
    /// This retains that distinction for try-lock APIs that erase poison into
    /// the same absence result as ordinary contention.
    pub fn poisoning_guard<T>(&self, guard: T) -> ReleaseGuard<'_, T> {
        ReleaseGuard {
            inner: Some(guard),
            notification: self,
            poison_on_unwind: true,
        }
    }

    /// Cover acquisition that may panic after locking but before returning its
    /// physical guard, for example while cloning an EBR generation. Normal
    /// completion emits no signal: the caller must immediately wrap the returned
    /// physical guard. Unwinding signals only after the inner acquisition stack
    /// has released its raw lock, so an already-waiting retry learns of poison.
    pub(crate) fn with_acquisition_unwind_notification<T>(&self, acquire: impl FnOnce() -> T) -> T {
        struct Acquisition<'a> {
            notification: &'a ReleaseNotification,
            armed: bool,
        }
        impl Drop for Acquisition<'_> {
            fn drop(&mut self) {
                if self.armed {
                    self.notification.released(true);
                }
            }
        }
        let mut acquisition = Acquisition {
            notification: self,
            armed: true,
        };
        let guard = acquire();
        acquisition.armed = false;
        guard
    }

    fn released(&self, poisoned: bool) {
        let waiters = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            // Exhaustion makes observations immediately ready, never silently
            // aliases an old waiter. This counter is only a wake hint.
            state.sequence = state.sequence.saturating_add(1);
            state.poisoned |= poisoned;
            std::mem::take(&mut state.waiters)
        };
        for waiter in waiters.into_iter().filter_map(|waiter| waiter.upgrade()) {
            let waker = waiter.lock().unwrap_or_else(|p| p.into_inner()).take();
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }
}

/// Opaque observation of one lock's release, safe to retain across async dispatch.
///
/// This is neither a publication token nor a guarantee the lock is now free.
/// Clones observe the same cut and may wait independently. Observation itself
/// allocates nothing; a polled pending future retains one registration and waker.
#[derive(Clone)]
pub struct ReleaseWait {
    state: Arc<Mutex<State>>,
    sequence: u64,
}

impl std::fmt::Debug for ReleaseWait {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReleaseWait").finish_non_exhaustive()
    }
}

impl PartialEq for ReleaseWait {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state) && self.sequence == other.sequence
    }
}
impl Eq for ReleaseWait {}

impl ReleaseWait {
    pub(crate) fn is_poisoned(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .poisoned
    }

    /// Wait for a release after the observation, including one before first poll.
    ///
    /// Dropping the future cancels its registration; it does not consume another
    /// waiter's wake. Callers must bound/admit their retained pending futures.
    pub fn wait_for_release(self) -> ReleaseFuture {
        ReleaseFuture {
            observation: self,
            registration: None,
        }
    }
}

/// Future for one opaque release observation. It retains no physical lock guard.
pub struct ReleaseFuture {
    observation: ReleaseWait,
    registration: Option<Arc<Mutex<Option<Waker>>>>,
}

impl Future for ReleaseFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        let mut state = this
            .observation
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if state.sequence != this.observation.sequence || state.sequence == u64::MAX {
            this.registration = None;
            return Poll::Ready(());
        }
        if let Some(registration) = &this.registration {
            let mut waker = registration.lock().unwrap_or_else(|p| p.into_inner());
            if waker.as_ref().is_none_or(|old| !old.will_wake(cx.waker())) {
                *waker = Some(cx.waker().clone());
            }
        } else {
            let registration = Arc::new(Mutex::new(Some(cx.waker().clone())));
            state.waiters.retain(|waiter| waiter.strong_count() != 0);
            state.waiters.push(Arc::downgrade(&registration));
            this.registration = Some(registration);
        }
        Poll::Pending
    }
}

impl Drop for ReleaseFuture {
    fn drop(&mut self) {
        let Some(registration) = self.registration.take() else {
            return;
        };
        let weak = Arc::downgrade(&registration);
        let mut state = self
            .observation
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        state
            .waiters
            .retain(|waiter| !waiter.ptr_eq(&weak) && waiter.strong_count() != 0);
        if state.waiters.is_empty() {
            // A canceled cohort must release its registration allocation even
            // when the physical lock will never be acquired or released again.
            state.waiters = Vec::new();
        }
    }
}

/// Physical guard that signals only after its inner guard has been released.
///
/// Drop covers abort/unwind as well as ordinary release. This wrapper deliberately
/// exposes no extraction that could separate the guard from its release signal.
pub struct ReleaseGuard<'owner, T> {
    inner: Option<T>,
    notification: &'owner ReleaseNotification,
    poison_on_unwind: bool,
}

impl<T> ReleaseGuard<'_, T> {
    /// Consume a guard through its commit operation, then notify after it returns.
    /// Unwinding also releases the guard before notification.
    pub(crate) fn release_with<R>(mut self, consume: impl FnOnce(T) -> R) -> R {
        consume(self.inner.take().expect("owned release guard"))
    }
}

impl<T> Deref for ReleaseGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("owned release guard")
    }
}
impl<T> DerefMut for ReleaseGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("owned release guard")
    }
}
impl<T> Drop for ReleaseGuard<'_, T> {
    fn drop(&mut self) {
        struct SignalAfterRelease<'a> {
            notification: &'a ReleaseNotification,
            poison_on_unwind: bool,
        }
        impl Drop for SignalAfterRelease<'_> {
            fn drop(&mut self) {
                self.notification
                    .released(self.poison_on_unwind && std::thread::panicking());
            }
        }
        let signal = SignalAfterRelease {
            notification: self.notification,
            poison_on_unwind: self.poison_on_unwind,
        };
        // Even a panic in the inner destructor must run its drop glue before
        // signaling. The local guard keeps that ordering on both exits.
        drop(self.inner.take());
        drop(signal);
    }
}

#[cfg(test)]
#[path = "release_tests.rs"]
mod tests;
