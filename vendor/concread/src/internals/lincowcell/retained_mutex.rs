//! Original native mutex with explicit poison for retained publication custody.
//!
//! Ordinary guards poison conservatively on unwind. A retained guard protects
//! an unchanged original root: it only becomes a mutation guard on mutable
//! access or explicit joint-publication entry. The permanent poison flag is
//! monotonic; completing a successful mutation only disarms that guard.

use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        LockResult, PoisonError, TryLockError, TryLockResult,
    },
};

#[derive(Debug)]
pub(super) struct Mutex<T> {
    inner: std::sync::Mutex<T>,
    poisoned: AtomicBool,
}

impl<T> Mutex<T> {
    pub(super) fn new(value: T) -> Self {
        // Initialize any native backing at owner construction, never at the
        // first retained source read on a previously unused thread. A standard
        // mutex has no parking_lot deadlock-detector per-thread allocation.
        let inner = std::sync::Mutex::new(value);
        drop(inner.lock().unwrap_or_else(PoisonError::into_inner));
        Self {
            inner,
            poisoned: AtomicBool::new(false),
        }
    }

    pub(super) fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        self.finish(
            self.inner.lock().unwrap_or_else(PoisonError::into_inner),
            true,
        )
    }

    pub(super) fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        self.try_acquire(true)
    }

    pub(super) fn lock_retained(&self) -> LockResult<MutexGuard<'_, T>> {
        self.finish(
            self.inner.lock().unwrap_or_else(PoisonError::into_inner),
            false,
        )
    }

    pub(super) fn try_lock_retained(&self) -> TryLockResult<MutexGuard<'_, T>> {
        self.try_acquire(false)
    }

    fn try_acquire(&self, armed: bool) -> TryLockResult<MutexGuard<'_, T>> {
        let guard = match self.inner.try_lock() {
            Ok(guard) => guard,
            // The backing primitive cannot distinguish read-only retained
            // abandonment from interrupted publication. The original explicit
            // flag below remains the sole poison verdict for both lock paths.
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => return Err(TryLockError::WouldBlock),
        };
        self.finish(guard, armed).map_err(TryLockError::Poisoned)
    }

    fn finish<'a>(
        &'a self,
        inner: std::sync::MutexGuard<'a, T>,
        armed: bool,
    ) -> LockResult<MutexGuard<'a, T>> {
        let guard = MutexGuard {
            inner,
            poisoned: &self.poisoned,
            panicking_on_entry: std::thread::panicking(),
            armed,
            retained: !armed,
            _not_send: PhantomData,
        };
        if self.is_poisoned() {
            Err(PoisonError::new(guard))
        } else {
            Ok(guard)
        }
    }

    pub(super) fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Acquire)
    }

    pub(super) fn poison_flag(&self) -> &AtomicBool {
        &self.poisoned
    }
}

#[derive(Debug)]
pub(super) struct MutexGuard<'a, T> {
    // The poison verdict is stored before this actual native guard unlocks.
    inner: std::sync::MutexGuard<'a, T>,
    poisoned: &'a AtomicBool,
    panicking_on_entry: bool,
    armed: bool,
    retained: bool,
    // Preserve the original guard's !Send / conditional Sync contract even if
    // the internal backing implementation changes.
    _not_send: PhantomData<std::sync::MutexGuard<'a, ()>>,
}

impl<T> MutexGuard<'_, T> {
    pub(super) fn is_retained(&self) -> bool {
        self.retained
    }

    pub(super) fn begin_retained_mutation(&mut self) {
        self.armed = true;
    }

    // Only the sealed original engine calls this after all joint roots point
    // at their completed generation. It never modifies a poison flag.
    pub(super) fn complete_retained_mutation(&mut self) {
        self.armed = !self.retained;
    }
}
impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}
impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.begin_retained_mutation();
        &mut self.inner
    }
}
impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        if self.armed && !self.panicking_on_entry && std::thread::panicking() {
            self.poisoned.store(true, Ordering::Release);
        }
    }
}
