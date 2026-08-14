//! Process-lifetime fail-stop guard for consensus admission and effects.
//!
//! A fatal live-runner or durable-effect failure permanently changes this
//! guard from open to restart-required at its API boundary. Internally,
//! `ACTIVATING` already rejects every new permit while admitted output drains;
//! a later non-panicking permit/acquire path or explicit blocking activation
//! publishes the final `RESTART_REQUIRED` state. Panic and incomplete-operation
//! drops close admission before releasing their read permits without blocking
//! on a writer.
#[cfg(not(test))]
use std::sync::OnceLock;
use std::sync::{
    Arc, RwLock, RwLockReadGuard, TryLockError,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use std::thread;
const OPEN: u8 = 0;
const ACTIVATING: u8 = 1;
const RESTART_REQUIRED: u8 = 2;
/// Shared admission/effect barrier for one consensus process.
#[derive(Debug)]
pub(crate) struct ConsensusOutputGuard {
    state: AtomicU8,
    authoritative_worker_launch_claimed: AtomicBool,
    output: RwLock<()>,
}
impl Default for ConsensusOutputGuard {
    fn default() -> Self {
        Self {
            state: AtomicU8::new(OPEN),
            authoritative_worker_launch_claimed: AtomicBool::new(false),
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
                // This path may be reached by code that already owns another
                // permit from the same guard.  Close admission synchronously,
                // but never wait for a write lock here.
                self.begin_activation();
                self.try_finalize_activation();
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
    /// Permanently claim the one authoritative worker launch allowed in this process.
    ///
    /// The claim is intentionally never released, including after orderly worker exit.
    /// Reusing generation zero at the same height is safe only after process replacement,
    /// which constructs a new process-global guard.
    pub(crate) fn claim_authoritative_worker_launch(&self) -> bool {
        self.authoritative_worker_launch_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    /// Permanently stop new output and, outside panic unwinding, drain admitted output.
    ///
    /// During unwinding this only closes admission; taking a writer there
    /// could deadlock on nested permits and poison the lock.
    pub(crate) fn activate_restart_required(&self) {
        if thread::panicking() {
            self.begin_activation();
            return;
        }
        match self
            .state
            .compare_exchange(OPEN, ACTIVATING, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) | Err(ACTIVATING) => self.drain_and_publish_restart_required(),
            Err(RESTART_REQUIRED) => {}
            Err(_) => unreachable!("consensus output guard has a valid state"),
        }
    }
    /// Permanently close admission without blocking for an in-flight drain.
    ///
    /// Panic guards use this before their stack can release nested permits.
    /// A later non-panicking permit/acquire path finalizes the internal state.
    pub(crate) fn close_admission_for_restart(&self) {
        self.begin_activation();
    }
    /// Activate restart recovery from an already-admitted fatal effect.
    ///
    /// The state changes to `ACTIVATING` before the fatal effect releases its read permit. This
    /// closes the otherwise unavoidable drop-then-activate window in which a second output could
    /// acquire a permit after the fatal durability result was already known. Finalization only
    /// attempts a nonblocking write so a nested operation cannot wait on an outer permit held by
    /// the same thread; explicit [`Self::activate_restart_required`] still performs a blocking
    /// drain when the caller knows no nested permit remains.
    pub(crate) fn activate_restart_required_from_permit(
        &self,
        mut permit: ConsensusOutputPermit<'_>,
    ) {
        let read_guard = permit.take_for_explicit_activation();
        self.begin_activation();
        drop(read_guard);
        self.try_finalize_activation();
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
    /// Return whether this live permit belongs to the exact guard.
    ///
    /// Borrow-bound prepared transactions use this immediately before their
    /// infallible mutation tail. A foreign or already-consumed permit cannot
    /// authorize output. Restart activation which begins after acquisition
    /// does not retroactively revoke this permit: activation drains every
    /// operation which already crossed the open gate.
    pub(crate) fn authorizes(&self, output_guard: &ConsensusOutputGuard) -> bool {
        std::ptr::eq(self.output_guard, output_guard) && self.armed && self.read_guard.is_some()
    }
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
    use super::ConsensusOutputGuard;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    };
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
    fn fatal_permit_closes_gate_without_waiting_for_earlier_output() {
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
        done_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("fatal permit activation must not wait on an earlier permit");
        worker.join().expect("join fatal activation worker");
        assert_eq!(
            guard.state.load(Ordering::Acquire),
            super::ACTIVATING,
            "an earlier output must keep nonblocking activation pending"
        );
        drop(earlier_permit);
        assert_eq!(
            guard.state.load(Ordering::Acquire),
            super::RESTART_REQUIRED,
            "the last ordinary permit drop must finalize activation"
        );
        assert!(guard.acquire().is_none());
    }
    #[test]
    fn nested_ordinary_permit_panic_closes_without_deadlock_and_finalizes_later() {
        let guard = ConsensusOutputGuard::isolated();
        let surviving_permit = guard.acquire().expect("admit surviving output");
        let unwind = std::panic::catch_unwind({
            let guard = Arc::clone(&guard);
            move || {
                let _outer = guard.acquire().expect("admit outer nested output");
                let _inner = guard.acquire().expect("admit inner nested output");
                panic!("model ingress panic with nested ordinary permits");
            }
        });
        assert!(unwind.is_err(), "nested panic must unwind without deadlock");
        assert_eq!(
            guard.state.load(Ordering::Acquire),
            super::ACTIVATING,
            "panic drop must close admission before the surviving read permit drains"
        );
        assert!(
            guard.acquire().is_none(),
            "no later output may cross a panic-closed gate"
        );
        assert_eq!(
            guard.state.load(Ordering::Acquire),
            super::ACTIVATING,
            "rejected admission cannot finalize while an earlier output remains"
        );
        drop(surviving_permit);
        assert_eq!(
            guard.state.load(Ordering::Acquire),
            super::RESTART_REQUIRED,
            "the last non-panicking permit drop must finalize activation"
        );
        assert!(guard.acquire().is_none());
    }
    #[test]
    fn nested_fail_stop_operation_panic_closes_without_blocking_outer_permit() {
        let guard = ConsensusOutputGuard::isolated();
        let surviving_permit = guard.acquire().expect("admit surviving output");
        let unwind = std::panic::catch_unwind({
            let guard = Arc::clone(&guard);
            move || {
                let _outer = guard.acquire().expect("admit outer ordinary output");
                let _fatal = guard
                    .begin_fail_stop_operation()
                    .expect("admit nested fail-stop operation");
                panic!("model nested fail-stop panic");
            }
        });
        assert!(
            unwind.is_err(),
            "fail-stop panic must not deadlock on its outer ordinary permit"
        );
        assert_eq!(guard.state.load(Ordering::Acquire), super::ACTIVATING);
        assert!(guard.acquire().is_none());
        assert_eq!(guard.state.load(Ordering::Acquire), super::ACTIVATING);
        drop(surviving_permit);
        assert_eq!(guard.state.load(Ordering::Acquire), super::RESTART_REQUIRED);
    }
    #[test]
    fn nested_fail_stop_error_closes_without_blocking_outer_operation() {
        let guard = ConsensusOutputGuard::isolated();
        let outer = guard
            .begin_fail_stop_operation()
            .expect("admit outer fail-stop operation");
        let inner = guard
            .begin_fail_stop_operation()
            .expect("admit inner fail-stop operation");
        drop(inner);
        assert_eq!(guard.state.load(Ordering::Acquire), super::ACTIVATING);
        assert!(guard.acquire().is_none());
        outer.complete();
        assert_eq!(guard.state.load(Ordering::Acquire), super::RESTART_REQUIRED);
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
    fn permit_authorizes_only_its_exact_guard_for_its_full_lifetime() {
        let guard = ConsensusOutputGuard::isolated();
        let foreign = ConsensusOutputGuard::isolated();
        let permit = guard.acquire().expect("admit exact output permit");
        assert!(permit.authorizes(&guard));
        assert!(!permit.authorizes(&foreign));
        guard.close_admission_for_restart();
        assert!(
            permit.authorizes(&guard),
            "restart drains rather than revokes already-admitted output"
        );
        assert!(guard.acquire().is_none());
        drop(permit);
        assert!(guard.restart_required());
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
