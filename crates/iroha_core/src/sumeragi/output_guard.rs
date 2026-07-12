//! Process-lifetime fail-stop guard for consensus admission and effects.
//!
//! A fatal live-runner or durable-effect failure changes this guard exactly
//! once from open to restart-required. The transition first prevents new
//! output permits, then waits for every already-admitted output to finish.
//! Consequently, once activation returns, no consensus output can cross the
//! guarded boundary until the operating-system process is restarted.

use std::sync::{
    Arc, OnceLock, RwLock, RwLockReadGuard, TryLockError,
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
    _guard: RwLockReadGuard<'a, ()>,
}

impl ConsensusOutputGuard {
    /// Construct an isolated open guard for tests or an explicitly scoped runner.
    pub(crate) fn isolated() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Admit one output only while no restart-required transition has begun.
    pub(crate) fn acquire(&self) -> Option<ConsensusOutputPermit<'_>> {
        if self.state.load(Ordering::Acquire) != OPEN {
            return None;
        }
        let guard = match self.output.try_read() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return None,
            Err(TryLockError::Poisoned(poisoned)) => {
                drop(poisoned.into_inner());
                self.activate_restart_required();
                return None;
            }
        };
        if self.state.load(Ordering::Acquire) != OPEN {
            return None;
        }
        Some(ConsensusOutputPermit { _guard: guard })
    }

    /// Permanently stop new output and drain output already admitted by this guard.
    pub(crate) fn activate_restart_required(&self) {
        match self.state.compare_exchange(
            OPEN,
            ACTIVATING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                let guard = match self.output.write() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                self.state.store(RESTART_REQUIRED, Ordering::Release);
                drop(guard);
            }
            Err(ACTIVATING) => {
                // Synchronize with the activating thread so callers do not
                // begin teardown before every admitted output has drained.
                let guard = match self.output.write() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                drop(guard);
            }
            Err(RESTART_REQUIRED) => {}
            Err(_) => unreachable!("consensus output guard has a valid state"),
        }
    }

    /// Return whether activation has begun or completed.
    pub(crate) fn restart_required(&self) -> bool {
        self.state.load(Ordering::Acquire) != OPEN
    }
}

/// Return the sole guard used by production consensus in this process.
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
        time::Duration,
    };

    use super::ConsensusOutputGuard;

    #[test]
    fn activation_drains_existing_output_and_rejects_every_later_output() {
        let guard = ConsensusOutputGuard::isolated();
        let permit = guard.acquire().expect("initial output permit");
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
}
