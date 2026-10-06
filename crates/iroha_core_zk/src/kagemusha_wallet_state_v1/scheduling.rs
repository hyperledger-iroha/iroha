//! Cooperative fold cancellation and exclusive payment priority.

use std::sync::{
    Arc, Condvar, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

use super::Error;

/// Cloneable cancellation token checked at proving task boundaries.
#[derive(Debug, Clone)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Stop at a cooperative boundary, releasing the caller's proving allocations on return.
    ///
    /// # Errors
    /// `Cancelled` after a payment interaction has requested priority.
    pub fn check(&self) -> Result<(), Error> {
        if self.0.load(Ordering::Acquire) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

#[derive(Default)]
struct State {
    eligible: bool,
    payments: usize,
    running: Option<Cancellation>,
}

struct Inner {
    state: Mutex<State>,
    idle: Condvar,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Shared foreground/charging gate with payment preemption and proof-workspace release.
///
/// Payment arrival can call `payment` from another thread while `fold_once` proves. It first
/// signals cancellation, then waits until the fold call has returned and dropped its work.
/// Holding the returned guard prevents a new background proof from starting. Dropping it
/// permits resumption from the last durable checkpoint. No payment state is rolled back.
#[derive(Clone)]
pub struct Scheduler(Arc<Inner>);

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    /// Initially inactive; enable work explicitly when foreground or charging.
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(State::default()),
            idle: Condvar::new(),
        }))
    }

    /// Update app/device activity. Leaving both states cancels an existing sub-proof.
    pub fn set_activity(&self, foreground: bool, charging: bool) {
        let mut state = self.0.lock();
        state.eligible = foreground || charging;
        if !state.eligible {
            if let Some(token) = &state.running {
                token.cancel();
            }
        }
    }

    /// Signal payment priority and join the current sub-proof before returning.
    #[must_use]
    pub fn payment(&self) -> PaymentGuard {
        let mut state = self.0.lock();
        state.payments += 1;
        if let Some(token) = &state.running {
            token.cancel();
        }
        while state.running.is_some() {
            state = self
                .0
                .idle
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        PaymentGuard(self.clone())
    }

    pub(super) fn start(&self) -> Option<FoldGuard> {
        let mut state = self.0.lock();
        if !state.eligible || state.payments != 0 || state.running.is_some() {
            return None;
        }
        let token = Cancellation::new();
        state.running = Some(token.clone());
        Some(FoldGuard {
            scheduler: self.clone(),
            token,
        })
    }
}

/// Holds payment priority after the cancelled sub-proof has released its allocations.
pub struct PaymentGuard(Scheduler);

impl Drop for PaymentGuard {
    fn drop(&mut self) {
        let mut state = self.0.0.lock();
        state.payments -= 1;
        self.0.0.idle.notify_all();
    }
}

pub(super) struct FoldGuard {
    scheduler: Scheduler,
    pub(super) token: Cancellation,
}

impl Drop for FoldGuard {
    fn drop(&mut self) {
        self.scheduler.0.lock().running = None;
        self.scheduler.0.idle.notify_all();
    }
}
