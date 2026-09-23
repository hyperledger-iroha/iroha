//! Lock-release observations for retrying local publication without a timer.

use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
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
pub struct ReleaseNotification {
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for ReleaseNotification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReleaseNotification")
            .finish_non_exhaustive()
    }
}

impl Default for ReleaseNotification {
    fn default() -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        // Some platforms allocate native mutex storage on first acquisition.
        // Pay that construction cost here, before allocation-free observations
        // or a release that may itself be returning exhausted capacity.
        drop(state.lock().unwrap_or_else(|p| p.into_inner()));
        Self { state }
    }
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

    /// Retain any number of actual releases of this source in constant space.
    /// An empty batch emits no notification. Each release must be transferred
    /// by its original guard; this constructor grants no authority to signal.
    pub fn deferred_batch(&self) -> DeferredReleaseBatch {
        DeferredReleaseBatch {
            notification: ReleaseNotification {
                state: Arc::clone(&self.state),
            },
            released: false,
            poisoned: false,
        }
    }

    /// Check original batch custody before acquiring a physical owner. This
    /// observation grants no authority to record a release or signal a wake.
    pub(crate) fn owns_batch(&self, batch: &DeferredReleaseBatch) -> bool {
        Arc::ptr_eq(&self.state, &batch.notification.state)
    }

    /// Bind a physical guard to notification after its actual release.
    /// Read guards and locks without poisoning use this form: their unwind does
    /// not turn later ordinary contention into a poisoned-writer failure.
    pub fn guard<T>(&self, guard: T) -> ReleaseGuard<'_, T> {
        ReleaseGuard {
            inner: Some(guard),
            notification: self,
            poison: PoisonPolicy::Never,
        }
    }

    /// Bind a guard whose underlying lock becomes poisoned when it unwinds.
    /// This retains that distinction for try-lock APIs that erase poison into
    /// the same absence result as ordinary contention.
    pub fn poisoning_guard<T>(&self, guard: T) -> ReleaseGuard<'_, T> {
        ReleaseGuard {
            inner: Some(guard),
            notification: self,
            poison: PoisonPolicy::Unwind,
        }
    }

    /// Bind the native lock's permanent poison flag to its exact release.
    /// Private retained owners distinguish immutable abandonment from a failed
    /// mutation; observe after unlocking, before any user wake callback.
    pub(crate) fn observed_guard<'a, T>(
        &'a self,
        guard: T,
        poisoned: &'a AtomicBool,
    ) -> ReleaseGuard<'a, T> {
        ReleaseGuard {
            inner: Some(guard),
            notification: self,
            poison: PoisonPolicy::Observed(poisoned),
        }
    }

    /// Cover acquisition that may panic after locking but before returning its
    /// physical guard, for example while cloning an EBR generation. Normal
    /// completion emits no signal: the caller must immediately wrap the returned
    /// physical guard. Unwinding signals only after the inner acquisition stack
    /// has released its raw lock, so an already-waiting retry learns of poison.
    pub fn with_acquisition_unwind_notification<T>(&self, acquire: impl FnOnce() -> T) -> T {
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
        struct WakeCohort {
            waiters: std::vec::IntoIter<Weak<Mutex<Option<Waker>>>>,
        }
        impl WakeCohort {
            fn drain(&mut self) {
                for waiter in self.waiters.by_ref().filter_map(|waiter| waiter.upgrade()) {
                    let waker = waiter.lock().unwrap_or_else(|p| p.into_inner()).take();
                    // The registration guard is gone before either the wake
                    // callback or its consumed waker's destructor can run.
                    if let Some(waker) = waker {
                        waker.wake();
                    }
                }
            }
        }
        impl Drop for WakeCohort {
            fn drop(&mut self) {
                // A failed callback must not strand the remaining original
                // registrations after their sequence has already advanced.
                // Preserve its panic while waking the unvisited cohort. A
                // second callback panic has ordinary double-panic semantics.
                self.drain();
            }
        }
        let waiters = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            // Exhaustion makes observations immediately ready, never silently
            // aliases an old waiter. This counter is only a wake hint.
            state.sequence = state.sequence.saturating_add(1);
            state.poisoned |= poisoned;
            std::mem::take(&mut state.waiters)
        };
        let mut cohort = WakeCohort {
            waiters: waiters.into_iter(),
        };
        cohort.drain();
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
    /// Whether a released physical owner reported permanent mutex poison.
    pub fn is_poisoned(&self) -> bool {
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
        // Raw waker callbacks may reenter this notification. Clone before any
        // internal lock, and retain replaced wakers until both locks are gone.
        let replacement = cx.waker().clone();
        let mut state = this
            .observation
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if state.sequence != this.observation.sequence || state.sequence == u64::MAX {
            drop(state);
            this.registration = None;
            return Poll::Ready(());
        }
        let retired = if let Some(registration) = &this.registration {
            let mut waker = registration.lock().unwrap_or_else(|p| p.into_inner());
            if waker.as_ref().is_none_or(|old| !old.will_wake(cx.waker())) {
                waker.replace(replacement)
            } else {
                Some(replacement)
            }
        } else {
            let registration = Arc::new(Mutex::new(Some(replacement)));
            // Initialize native mutex storage before publishing this waiter.
            // The first release must not allocate in order to take its waker;
            // dropping this fresh guard invokes no waker callback.
            drop(registration.lock().unwrap_or_else(|p| p.into_inner()));
            state.waiters.retain(|waiter| waiter.strong_count() != 0);
            state.waiters.push(Arc::downgrade(&registration));
            this.registration = Some(registration);
            None
        };
        drop(state);
        drop(retired);
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

#[derive(Clone, Copy)]
enum PoisonPolicy<'a> {
    Never,
    Unwind,
    Observed(&'a AtomicBool),
    Fixed(bool),
}
impl PoisonPolicy<'_> {
    fn observe(self) -> bool {
        match self {
            Self::Never => false,
            Self::Unwind => std::thread::panicking(),
            Self::Observed(flag) => flag.load(Ordering::Acquire),
            Self::Fixed(value) => value,
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
    poison: PoisonPolicy<'owner>,
}

/// Original notification retained after physical unlock until aggregate cleanup.
/// This owns the same notification state and allocates no replacement source.
/// Dropping it delivers the recorded release, even if later cleanup unwinds.
pub struct DeferredRelease {
    notification: ReleaseNotification,
    poisoned: bool,
}

/// Bounded custody of actual releases from one original physical lock.
///
/// All recorded releases remain deferred until this owner drops. They coalesce
/// into one wake hint: waiters must still reacquire and authenticate the lock.
/// Recording does not allocate, invoke callbacks, or create another source.
#[must_use = "retain recorded releases through all enclosing physical owners"]
pub struct DeferredReleaseBatch {
    notification: ReleaseNotification,
    released: bool,
    poisoned: bool,
}

impl Drop for DeferredReleaseBatch {
    fn drop(&mut self) {
        if self.released {
            self.notification.released(self.poisoned);
        }
    }
}

impl Drop for DeferredRelease {
    fn drop(&mut self) {
        self.notification.released(self.poisoned);
    }
}

impl<'owner, T> ReleaseGuard<'owner, T> {
    /// Release this actual guard into a batch of the same original source.
    /// A foreign batch returns the unchanged guard without calling `release`.
    /// The callback must unlock the physical owner on success and unwind;
    /// returned values may retain cleanup but never the physical guard.
    /// Acquisition poison is recorded before any later cleanup can unwind.
    pub fn try_release_into<R>(
        mut self,
        batch: &mut DeferredReleaseBatch,
        release: impl FnOnce(T) -> R,
    ) -> Result<R, Self> {
        if !Arc::ptr_eq(&self.notification.state, &batch.notification.state) {
            return Err(self);
        }
        struct Record<'a> {
            batch: &'a mut DeferredReleaseBatch,
            poison: PoisonPolicy<'a>,
        }
        impl Drop for Record<'_> {
            fn drop(&mut self) {
                self.batch.released = true;
                self.batch.poisoned |= self.poison.observe();
            }
        }
        // On callback unwind the original physical owner drops before this
        // record. The batch remains in its caller's aggregate throughout.
        let record = Record {
            batch,
            poison: self.poison,
        };
        let inner = self.inner.take().expect("owned release guard");
        let _transferred = std::mem::ManuallyDrop::new(self);
        let result = release(inner);
        drop(record);
        Ok(result)
    }

    /// Release a physical owner now, retaining its original notification by value.
    /// The callback must release the physical guard on success and unwind. Normal
    /// release allocates nothing and runs no wake callback; the returned owner is
    /// dropped only after every enclosing publication fence has released.
    pub fn release_deferred<R>(self, release: impl FnOnce(T) -> R) -> (R, DeferredRelease) {
        let policy = self.poison;
        let mut retirement = self.release_retaining(release);
        // Actual primitive poison is established when its guard is released.
        let poisoned = policy.observe();
        let notification = DeferredRelease {
            notification: ReleaseNotification {
                state: Arc::clone(&retirement.notification.state),
            },
            poisoned,
        };
        let retained = retirement.inner.take().expect("owned release retirement");
        let _transferred = std::mem::ManuallyDrop::new(retirement);
        (retained, notification)
    }

    /// Release this actual guard into its original batch with an exact poison verdict.
    /// A foreign batch returns the unchanged guard without invoking either callback.
    /// `release` must unlock on success and unwind; `observe_poison` must inspect
    /// only the corresponding native mutex and cannot invoke user code.
    pub fn try_release_into_observed<R>(
        mut self,
        batch: &mut DeferredReleaseBatch,
        release: impl FnOnce(T) -> R,
        observe_poison: impl Fn() -> bool,
    ) -> Result<R, Self> {
        if !Arc::ptr_eq(&self.notification.state, &batch.notification.state) {
            return Err(self);
        }
        struct Record<'a, F: Fn() -> bool> {
            batch: &'a mut DeferredReleaseBatch,
            observe_poison: F,
        }
        impl<F: Fn() -> bool> Drop for Record<'_, F> {
            fn drop(&mut self) {
                self.batch.released = true;
                self.batch.poisoned |= (self.observe_poison)();
            }
        }
        let record = Record {
            batch,
            observe_poison,
        };
        let inner = self.inner.take().expect("original physical guard");
        let _transferred = std::mem::ManuallyDrop::new(self);
        let result = release(inner);
        drop(record);
        Ok(result)
    }

    /// Change ownership phase while the caller retains original unwind notification.
    ///
    /// The outer error returns a foreign-batch guard untouched. The inner result
    /// transfers the original notification with either the new or refused guard.
    /// Only a callee unwind records a release in `batch`, after `consume` has
    /// destroyed its physical guard. Completed payloads and unused charges must
    /// already belong to the caller's acquisition slot before another conversion.
    /// `observe_poison` inspects only this original native mutex and cannot panic.
    pub fn try_map_preserving_release_into<R, E>(
        mut self,
        batch: &mut DeferredReleaseBatch,
        consume: impl FnOnce(T) -> Result<R, (T, E)>,
        observe_poison: impl Fn() -> bool,
    ) -> Result<Result<ReleaseGuard<'owner, R>, (Self, E)>, Self> {
        if !Arc::ptr_eq(&self.notification.state, &batch.notification.state) {
            return Err(self);
        }
        struct Record<'a, F: Fn() -> bool> {
            batch: &'a mut DeferredReleaseBatch,
            observe_poison: F,
            armed: bool,
        }
        impl<F: Fn() -> bool> Drop for Record<'_, F> {
            fn drop(&mut self) {
                if self.armed {
                    self.batch.released = true;
                    self.batch.poisoned |= (self.observe_poison)();
                }
            }
        }
        let mut record = Record {
            batch,
            observe_poison,
            armed: true,
        };
        let inner = self.inner.take().expect("original physical guard");
        let transferred = std::mem::ManuallyDrop::new(self);
        let result = consume(inner);
        record.armed = false;
        drop(record);
        Ok(match result {
            Ok(inner) => Ok(ReleaseGuard {
                inner: Some(inner),
                notification: transferred.notification,
                poison: transferred.poison,
            }),
            Err((inner, error)) => Err((
                Self {
                    inner: Some(inner),
                    notification: transferred.notification,
                    poison: transferred.poison,
                },
                error,
            )),
        })
    }

    /// Attempt a phase change while retaining the original guard on refusal.
    /// Neither success nor refusal emits a release; the returned owner remains
    /// responsible for the same physical lock. Unwind releases before signaling.
    pub fn try_map_preserving_release<R, E>(
        mut self,
        consume: impl FnOnce(T) -> Result<R, (T, E)>,
    ) -> Result<ReleaseGuard<'owner, R>, (Self, E)> {
        let result = consume(self.inner.take().expect("owned release guard"));
        let result = match result {
            Ok(inner) => Ok(ReleaseGuard {
                inner: Some(inner),
                notification: self.notification,
                poison: self.poison,
            }),
            Err((inner, error)) => Err((
                Self {
                    inner: Some(inner),
                    notification: self.notification,
                    poison: self.poison,
                },
                error,
            )),
        };
        let _transferred = std::mem::ManuallyDrop::new(self);
        result
    }

    /// Transfer the original release notification across an ownership phase.
    /// No notification or old-owner destructor runs after a successful transfer.
    /// An unwind still drops the consumed physical owner before signaling.
    pub fn map_preserving_release<R>(
        mut self,
        consume: impl FnOnce(T) -> R,
    ) -> ReleaseGuard<'owner, R> {
        let inner = consume(self.inner.take().expect("owned release guard"));
        let result = ReleaseGuard {
            inner: Some(inner),
            notification: self.notification,
            poison: self.poison,
        };
        // The empty predecessor owns no allocation or physical guard. Its
        // notification has moved to result and must not run at this transition.
        let _transferred = std::mem::ManuallyDrop::new(self);
        result
    }

    /// Release the physical owner while retaining cleanup and its notification.
    /// The callback must return only owners whose destruction cannot poison the
    /// released physical lock. A callback unwind retains the original poisoning
    /// behavior; a later cleanup unwind must not poison an already healthy lock.
    pub fn release_retaining<R>(self, release: impl FnOnce(T) -> R) -> ReleaseGuard<'owner, R> {
        let mut retirement = self.map_preserving_release(release);
        retirement.poison = match retirement.poison {
            PoisonPolicy::Observed(flag) => PoisonPolicy::Fixed(flag.load(Ordering::Acquire)),
            PoisonPolicy::Fixed(value) => PoisonPolicy::Fixed(value),
            _ => PoisonPolicy::Never,
        };
        retirement
    }

    /// Consume a guard through its commit operation, then notify after it returns.
    /// Unwinding also releases the guard before notification.
    pub fn release_with<R>(mut self, consume: impl FnOnce(T) -> R) -> R {
        consume(self.inner.take().expect("owned release guard"))
    }

    /// Release this actual owner and report its physical poison after unlock.
    /// `consume` must release the lock on success and unwind. Unlike thread-local
    /// panic state, the observation also preserves poison predating acquisition
    /// and excludes a later callback panic after a healthy physical release.
    pub fn release_with_observed_poison<R>(
        mut self,
        consume: impl FnOnce(T) -> R,
        observe_poison: impl Fn() -> bool,
    ) -> R {
        struct Signal<'a, F: Fn() -> bool> {
            notification: &'a ReleaseNotification,
            observe_poison: F,
        }
        impl<F: Fn() -> bool> Drop for Signal<'_, F> {
            fn drop(&mut self) {
                self.notification.released((self.observe_poison)());
            }
        }
        let signal = Signal {
            notification: self.notification,
            observe_poison,
        };
        let inner = self.inner.take().expect("owned release guard");
        let _transferred = std::mem::ManuallyDrop::new(self);
        let result = consume(inner);
        drop(signal);
        result
    }

    /// Release both original physical owners before either notification runs.
    /// The callback must release both owners on success and unwind; returned
    /// values may retain cleanup, but never physical guards. Observe the actual
    /// two locks' poison state after release, before any arbitrary wake callback.
    pub fn release_pair_with<S, R>(
        self,
        other: ReleaseGuard<'_, S>,
        consume: impl FnOnce(T, S) -> R,
        observe_poison: impl Fn() -> (bool, bool),
    ) -> R {
        match self.try_map_pair_preserving_release::<_, (), (), R>(
            other,
            |first, second| Err(consume(first, second)),
            observe_poison,
        ) {
            Err(result) => result,
            Ok(_) => unreachable!("release never transfers physical owners"),
        }
    }

    /// Transfer both original guards through one fallible construction phase.
    /// Success retains both original notifications without invoking callbacks.
    /// On error or unwind, `consume` must release both physical guards before
    /// returning or unwinding; neither notification runs until that completes.
    /// Freeze both physical poison verdicts before any wake callback can panic.
    pub fn try_map_pair_preserving_release<'other, S, A, B, E>(
        mut self,
        mut other: ReleaseGuard<'other, S>,
        consume: impl FnOnce(T, S) -> Result<(A, B), E>,
        observe_poison: impl Fn() -> (bool, bool),
    ) -> Result<(ReleaseGuard<'owner, A>, ReleaseGuard<'other, B>), E> {
        struct Signal<'a> {
            notification: &'a ReleaseNotification,
            poisoned: bool,
        }
        impl Drop for Signal<'_> {
            fn drop(&mut self) {
                self.notification.released(self.poisoned);
            }
        }
        struct PairSignals<'a, 'b, F: Fn() -> (bool, bool)> {
            first: &'a ReleaseNotification,
            second: &'b ReleaseNotification,
            observe_poison: F,
            armed: bool,
        }
        impl<F: Fn() -> (bool, bool)> Drop for PairSignals<'_, '_, F> {
            fn drop(&mut self) {
                if !self.armed {
                    return;
                }
                let (first, second) = (self.observe_poison)();
                let first = Signal {
                    notification: self.first,
                    poisoned: first,
                };
                let second = Signal {
                    notification: self.second,
                    poisoned: second,
                };
                // Both verdicts are frozen before the first callback. Unwind
                // must still deliver the other original release with its own
                // physical verdict, not the callback's panic state.
                drop(first);
                drop(second);
            }
        }
        let mut signals = PairSignals {
            first: self.notification,
            second: other.notification,
            observe_poison,
            armed: true,
        };
        let first = self.inner.take().expect("owned first release guard");
        let second = other.inner.take().expect("owned second release guard");
        // Only empty wrappers remain; the pair owns both original signals.
        let _first = std::mem::ManuallyDrop::new(self);
        let _second = std::mem::ManuallyDrop::new(other);
        match consume(first, second) {
            Ok((first, second)) => {
                signals.armed = false;
                Ok((
                    ReleaseGuard {
                        inner: Some(first),
                        notification: _first.notification,
                        poison: _first.poison,
                    },
                    ReleaseGuard {
                        inner: Some(second),
                        notification: _second.notification,
                        poison: _second.poison,
                    },
                ))
            }
            Err(error) => {
                drop(signals);
                Err(error)
            }
        }
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
            poison: PoisonPolicy<'a>,
        }
        impl Drop for SignalAfterRelease<'_> {
            fn drop(&mut self) {
                self.notification.released(self.poison.observe());
            }
        }
        let signal = SignalAfterRelease {
            notification: self.notification,
            poison: self.poison,
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
