//! Process-lifetime fail-stop guard for consensus admission and effects.
//!
//! A fatal live-runner or durable-effect failure changes this guard exactly
//! once from open to restart-required. The transition first prevents new
//! output permits, then waits for every already-admitted output to finish.
//! Consequently, once activation returns, no consensus output can cross the
//! guarded boundary until the operating-system process is restarted.

#[cfg(not(test))]
use std::sync::OnceLock;
use std::thread;
use std::sync::{
    Arc, RwLock, RwLockReadGuard, TryLockError,
    atomic::{AtomicU8, Ordering},
};

const OPEN: u8 = 0;
const ACTIVATING: u8 = 1;
const RESTART_REQUIRED: u8 = 2;

/// Shared admission/effect barrier for one consensus process.
#[derive(Debug)]
pub(crate) struct ConsensusOutputGuard {
    state: AtomicU8,
    output: RwLock<()>,
}

impl Default for ConsensusOutputGuard {
    fn default() -> Self {
        Self {
            state: AtomicU8::new(OPEN),
            output: RwLock::new(()),
        }
    }
}

/// Proof that one consensus admission or effect crossed the open guard.
pub(crate) struct ConsensusOutputPermit<'a> {
    output_guard: &'a ConsensusOutputGuard,
    read_guard: Option<RwLockReadGuard<'a, ()>>,
    armed: bool,
}

/// One fail-stop consensus operation admitted while output remains open.
///
/// The operation owns its output permit until [`Self::complete`] is called.
/// Returning an error or unwinding without completion closes the shared guard
/// before releasing that permit, eliminating a drop-then-activate window.
#[must_use = "a fail-stop operation must be explicitly completed on success"]
pub(crate) struct ConsensusFailStopOperation<'a> {
    output_guard: &'a ConsensusOutputGuard,
    permit: Option<ConsensusOutputPermit<'a>>,
}

impl ConsensusOutputGuard {
    /// Construct an isolated open guard for tests or an explicitly scoped runner.
    pub(crate) fn isolated() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Admit one output only while no restart-required transition has begun.
    pub(crate) fn acquire(&self) -> Option<ConsensusOutputPermit<'_>> {
        if self.state.load(Ordering::Acquire) != OPEN {
            self.try_finalize_activation();
            return None;
        }
        let guard = match self.output.try_read() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => {
                self.try_finalize_activation();
                return None;
            }
            Err(TryLockError::Poisoned(poisoned)) => {
                drop(poisoned.into_inner());
                self.activate_restart_required();
                return None;
            }
        };
        if self.state.load(Ordering::Acquire) != OPEN {
            drop(guard);
            self.try_finalize_activation();
            return None;
        }
        Some(ConsensusOutputPermit {
            output_guard: self,
            read_guard: Some(guard),
            armed: true,
        })
    }

    /// Begin an operation whose abnormal exit permanently requires restart.
    pub(crate) fn begin_fail_stop_operation(&self) -> Option<ConsensusFailStopOperation<'_>> {
        let permit = self.acquire()?;
        Some(ConsensusFailStopOperation {
            output_guard: self,
            permit: Some(permit),
        })
    }

    /// Permanently stop new output and drain output already admitted by this guard.
    pub(crate) fn activate_restart_required(&self) {
        match self
            .state
            .compare_exchange(OPEN, ACTIVATING, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(ACTIVATING) => self.drain_and_publish_restart_required(),
            Err(RESTART_REQUIRED) => {}
            Err(_) => unreachable!("consensus output guard has a valid state"),
        }
    }

    /// Stop admitting new consensus output without waiting for admitted work.
    ///
    /// This transition is used by lower-level fail-stop paths which may run
    /// while their caller still owns an output permit.  Waiting for the write
    /// side of `output` there would self-deadlock.  A fail-stop operation,
    /// worker shutdown, or a later [`Self::activate_restart_required`] call
    /// performs the eventual drain; `ACTIVATING` already rejects every new
    /// permit.
    pub(crate) fn close_admission_for_restart(&self) {
        self.begin_activation();
    }

    /// Activate restart recovery from an already-admitted fatal effect.
    ///
    /// The state changes to `ACTIVATING` before the fatal effect releases its read permit. This
    /// closes the otherwise unavoidable drop-then-activate window in which a second output could
    /// acquire a permit after the fatal durability result was already known.
    pub(crate) fn activate_restart_required_from_permit(
        &self,
        mut permit: ConsensusOutputPermit<'_>,
    ) {
        let read_guard = permit.take_for_explicit_activation();
        match self
            .state
            .compare_exchange(OPEN, ACTIVATING, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(ACTIVATING) => {
                drop(read_guard);
                self.drain_and_publish_restart_required();
            }
            Err(RESTART_REQUIRED) => drop(read_guard),
            Err(_) => unreachable!("consensus output guard has a valid state"),
        }
    }

    fn begin_activation(&self) {
        match self
            .state
            .compare_exchange(OPEN, ACTIVATING, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(ACTIVATING | RESTART_REQUIRED) => {}
            Err(_) => unreachable!("consensus output guard has a valid state"),
        }
    }

    fn try_finalize_activation(&self) {
        if thread::panicking() || self.state.load(Ordering::Acquire) != ACTIVATING {
            return;
        }
        let guard = match self.output.try_write() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        };
        if self.state.load(Ordering::Acquire) == ACTIVATING {
            self.state.store(RESTART_REQUIRED, Ordering::Release);
        }
        drop(guard);
    }

    fn drain_and_publish_restart_required(&self) {
        let guard = match self.output.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        self.state.store(RESTART_REQUIRED, Ordering::Release);
        drop(guard);
    }

    /// Return whether activation has begun or completed.
    pub(crate) fn restart_required(&self) -> bool {
        self.state.load(Ordering::Acquire) != OPEN
    }
}

impl<'a> ConsensusOutputPermit<'a> {
    fn take_for_explicit_activation(&mut self) -> RwLockReadGuard<'a, ()> {
        self.armed = false;
        self.read_guard
            .take()
            .expect("an admitted output permit owns its read guard")
    }
}

impl Drop for ConsensusOutputPermit<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let panicking = thread::panicking();
        if panicking {
            // Close admission before releasing the permit, but never wait for
            // nested permits or acquire a writer while unwinding.
            self.output_guard.begin_activation();
        }
        drop(self.read_guard.take());
        if !panicking {
            self.output_guard.try_finalize_activation();
        }
    }
}

impl ConsensusFailStopOperation<'_> {
    /// Borrow the held output permit for a guarded downstream effect.
    pub(crate) fn permit(&self) -> &ConsensusOutputPermit<'_> {
        self.permit
            .as_ref()
            .expect("incomplete fail-stop operation owns its output permit")
    }

    /// Mark the operation successful and release its output permit normally.
    pub(crate) fn complete(mut self) {
        drop(self.permit.take());
    }
}

impl Drop for ConsensusFailStopOperation<'_> {
    fn drop(&mut self) {
        if let Some(permit) = self.permit.take() {
            self.output_guard
                .activate_restart_required_from_permit(permit);
        }
    }
}

/// Return the sole guard used by production consensus in this process.
#[cfg(not(test))]
pub(crate) fn process_consensus_output_guard() -> Arc<ConsensusOutputGuard> {
    static PROCESS_GUARD: OnceLock<Arc<ConsensusOutputGuard>> = OnceLock::new();
    Arc::clone(PROCESS_GUARD.get_or_init(ConsensusOutputGuard::isolated))
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    };

    use super::ConsensusOutputGuard;

    #[test]
    fn activation_drains_existing_output_and_rejects_every_later_output() {
        let guard = ConsensusOutputGuard::isolated();
        let permit = guard.acquire().expect("initial output permit");
        let concurrent = guard
            .acquire()
            .expect("open-state permits must not serialize nonblocking ingress");
        drop(concurrent);
        let activated = Arc::new(AtomicBool::new(false));
        let activated_worker = Arc::clone(&activated);
        let worker_guard = Arc::clone(&guard);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            worker_guard.activate_restart_required();
            activated_worker.store(true, Ordering::Release);
            done_tx.send(()).expect("publish completed activation");
        });

        assert!(
            done_rx.recv_timeout(Duration::from_millis(25)).is_err(),
            "activation must wait for already-admitted output"
        );
        let activation_deadline = std::time::Instant::now() + Duration::from_secs(1);
        while !guard.restart_required() && std::time::Instant::now() < activation_deadline {
            thread::yield_now();
        }
        assert!(guard.restart_required());
        assert!(guard.acquire().is_none());
        drop(permit);
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("activation completes after output drains");
        worker.join().expect("join activation worker");

        assert!(activated.load(Ordering::Acquire));
        assert!(guard.restart_required());
        assert!(guard.acquire().is_none());
    }

    #[test]
    fn fatal_permit_closes_gate_before_it_is_released() {
        let guard = ConsensusOutputGuard::isolated();
        let earlier_permit = guard.acquire().expect("earlier output permit");
        let worker_guard = Arc::clone(&guard);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let fatal_permit = worker_guard.acquire().expect("fatal output permit");
            ready_tx
                .send(())
                .expect("announce acquisition of fatal output permit");
            worker_guard.activate_restart_required_from_permit(fatal_permit);
            done_tx.send(()).expect("publish completed activation");
        });

        ready_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("fatal output must be admitted before activation");
        let deadline = Instant::now() + Duration::from_secs(1);
        while !guard.restart_required() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(
            guard.restart_required(),
            "activation must close the gate while the fatal permit is still held"
        );
        assert!(
            guard.acquire().is_none(),
            "no output may enter after the fatal result becomes known"
        );
        assert!(
            done_rx.recv_timeout(Duration::from_millis(25)).is_err(),
            "activation must still drain outputs admitted before the fatal result"
        );

        drop(earlier_permit);
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("activation completes after all earlier output drains");
        worker.join().expect("join fatal activation worker");
        assert!(guard.acquire().is_none());
    }

    #[test]
    fn completed_fail_stop_operation_leaves_guard_open() {
        let guard = ConsensusOutputGuard::isolated();
        guard
            .begin_fail_stop_operation()
            .expect("admit fail-stop operation")
            .complete();

        assert!(!guard.restart_required());
        assert!(guard.acquire().is_some());
    }

    #[test]
    fn fail_stop_operation_latches_restart_required_during_unwind() {
        let guard = ConsensusOutputGuard::isolated();
        let unwind = std::panic::catch_unwind({
            let guard = Arc::clone(&guard);
            move || {
                let _operation = guard
                    .begin_fail_stop_operation()
                    .expect("admit fail-stop operation");
                panic!("model an abnormal consensus operation");
            }
        });

        assert!(unwind.is_err());
        assert!(guard.restart_required());
        assert!(guard.acquire().is_none());
    }
}
