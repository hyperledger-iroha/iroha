//! Logical custody of one archive predecessor without retaining its index lock.
//!
//! The caller acquires and checks this reservation while holding its existing
//! archive index writer. Every index mutation must either observe no reservation
//! or present this exact move-only owner. Committed readers remain independent.
//! Both candidate captures retain this owner after releasing their physical
//! writers. TODO: carry its typed wait through actual pre-vote admission.

use std::sync::{Arc, Weak};

use parking_lot::Mutex;
use tokio::sync::watch;

/// A local archive reservation's release, never a consensus verdict or deadline.
///
/// This observer names the particular owner that prevented admission. A later
/// reservation cannot make an already released ticket pending again. Cloning or
/// dropping a ticket neither prolongs nor releases the underlying reservation.
#[derive(Clone, Debug)]
pub struct ArchiveCaptureWait {
    released: watch::Receiver<bool>,
}

impl ArchiveCaptureWait {
    /// Return whether this particular owner has released its reservation.
    #[must_use]
    pub fn is_released(&self) -> bool {
        *self.released.borrow()
    }

    /// Wait until this particular reservation is released, without polling.
    ///
    /// Release before registration remains visible through the retained watch
    /// value. The caller must drop all State and archive writers before awaiting
    /// this event, then retry admission; release is not a grant of capacity.
    pub async fn wait_for_release(&mut self) {
        while !*self.released.borrow_and_update() {
            if self.released.changed().await.is_err() {
                // Only the reservation owns the sender. Its disappearance also
                // means that exact owner cannot retain the archive reservation.
                return;
            }
        }
    }
}

#[derive(Debug)]
struct ReleaseSignal {
    released: watch::Sender<bool>,
}

#[derive(Debug, Default)]
struct GateState {
    active: Weak<ReleaseSignal>,
}

/// One exact archive's original predecessor and admitted insertion capacity.
///
/// This gate is not cloneable. Its lifetime-free reservation retains only a
/// small identity/release object, never an archive, State view or physical lock.
#[derive(Debug, Default)]
pub(crate) struct ArchiveCaptureGate {
    state: Arc<Mutex<GateState>>,
}

impl ArchiveCaptureGate {
    /// Reserve the original archive cut while its physical index writer is held.
    ///
    /// The same physical writer must protect the accompanying transition and
    /// capacity checks, and remain held until this reservation is installed.
    /// Only one candidate can own that predecessor; losing admission returns
    /// the exact release event needed to retry after dropping those writers.
    pub(crate) fn try_reserve(&self) -> Result<ArchiveCaptureReservation, ArchiveCaptureWait> {
        let mut state = self.state.lock();
        if let Some(active) = state.active.upgrade() {
            return Err(ArchiveCaptureWait {
                released: active.released.subscribe(),
            });
        }
        let (released, _) = watch::channel(false);
        let signal = Arc::new(ReleaseSignal { released });
        state.active = Arc::downgrade(&signal);
        Ok(ArchiveCaptureReservation {
            state: Arc::clone(&self.state),
            signal,
        })
    }

    /// Check ordinary mutation admission while the physical index writer is held.
    ///
    /// Readers do not need this check. A prepared insertion must instead present
    /// its exact reservation, preserving its original admission across retries.
    pub(crate) fn ensure_unreserved(&self) -> Result<(), ArchiveCaptureWait> {
        let state = self.state.lock();
        match state.active.upgrade() {
            Some(active) => Err(ArchiveCaptureWait {
                released: active.released.subscribe(),
            }),
            None => Ok(()),
        }
    }
}

/// Move-only custody of the exact predecessor admitted under its archive writer.
///
/// No path, height, scalar generation, cloned gate or caller-supplied identifier
/// can substitute for this owner. Drop releases custody and wakes all observers.
#[derive(Debug)]
pub(crate) struct ArchiveCaptureReservation {
    state: Arc<Mutex<GateState>>,
    signal: Arc<ReleaseSignal>,
}

impl ArchiveCaptureReservation {
    /// Check the actual original gate under the archive's physical index writer.
    pub(crate) fn authorizes(&self, gate: &ArchiveCaptureGate) -> bool {
        Arc::ptr_eq(&self.state, &gate.state)
            && gate
                .state
                .lock()
                .active
                .ptr_eq(&Arc::downgrade(&self.signal))
    }
}

impl Drop for ArchiveCaptureReservation {
    fn drop(&mut self) {
        let mut state = self.state.lock();
        if state.active.ptr_eq(&Arc::downgrade(&self.signal)) {
            state.active = Weak::new();
        }
        drop(state);
        self.signal.released.send_replace(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Barrier, time::Duration};

    #[test]
    fn only_the_exact_original_gate_accepts_its_retained_owner() {
        let original = ArchiveCaptureGate::default();
        let foreign = ArchiveCaptureGate::default();
        let reservation = original.try_reserve().unwrap();
        assert!(reservation.authorizes(&original));
        assert!(!reservation.authorizes(&foreign));
        assert!(foreign.ensure_unreserved().is_ok());
        assert!(original.ensure_unreserved().is_err());
        drop(reservation);
        assert!(original.ensure_unreserved().is_ok());
    }

    #[test]
    fn observers_neither_own_nor_cancel_the_reservation() {
        let gate = ArchiveCaptureGate::default();
        let reservation = gate.try_reserve().unwrap();
        let wait = gate.try_reserve().unwrap_err();
        let retained_wait = wait.clone();
        assert!(!wait.is_released());
        drop(wait);
        assert!(gate.ensure_unreserved().is_err());
        assert!(reservation.authorizes(&gate));
        drop(reservation);
        assert!(retained_wait.is_released());
        assert!(gate.ensure_unreserved().is_ok());
    }

    #[tokio::test]
    async fn release_before_wait_registration_cannot_be_missed() {
        let gate = ArchiveCaptureGate::default();
        let reservation = gate.try_reserve().unwrap();
        let mut wait = gate.ensure_unreserved().unwrap_err();
        drop(reservation);
        tokio::time::timeout(Duration::from_secs(1), wait.wait_for_release())
            .await
            .unwrap();
        assert!(wait.is_released());
    }

    #[tokio::test]
    async fn active_wait_is_woken_by_the_actual_owner_drop() {
        let gate = ArchiveCaptureGate::default();
        let reservation = gate.try_reserve().unwrap();
        let mut wait = gate.ensure_unreserved().unwrap_err();
        let (started, entered) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            started.send(()).unwrap();
            wait.wait_for_release().await;
            assert!(wait.is_released());
        });
        entered.await.unwrap();
        assert!(!task.is_finished());
        drop(reservation);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn old_wait_remains_released_while_a_new_owner_is_active() {
        let gate = ArchiveCaptureGate::default();
        let first = gate.try_reserve().unwrap();
        let mut old_wait = gate.ensure_unreserved().unwrap_err();
        drop(first);
        let second = gate.try_reserve().unwrap();
        let new_wait = gate.ensure_unreserved().unwrap_err();
        tokio::time::timeout(Duration::from_secs(1), old_wait.wait_for_release())
            .await
            .unwrap();
        assert!(old_wait.is_released());
        assert!(!new_wait.is_released());
        assert!(second.authorizes(&gate));
        drop(second);
        assert!(new_wait.is_released());
    }

    #[test]
    fn move_to_another_worker_preserves_custody_without_retaining_the_archive() {
        fn assert_static_send_sync<T: Send + Sync + 'static>() {}
        assert_static_send_sync::<ArchiveCaptureReservation>();
        assert_static_send_sync::<ArchiveCaptureWait>();
        let gate = ArchiveCaptureGate::default();
        let reservation = gate.try_reserve().unwrap();
        let wait = gate.ensure_unreserved().unwrap_err();
        drop(gate);
        assert!(!wait.is_released());
        std::thread::spawn(move || drop(reservation))
            .join()
            .unwrap();
        assert!(wait.is_released());
    }

    #[test]
    fn concurrent_attempts_retain_exactly_one_original_owner() {
        let gate = Arc::new(ArchiveCaptureGate::default());
        let start = Arc::new(Barrier::new(4));
        let workers = (0..4)
            .map(|_| {
                let gate = Arc::clone(&gate);
                let start = Arc::clone(&start);
                std::thread::spawn(move || {
                    start.wait();
                    gate.try_reserve()
                })
            })
            .collect::<Vec<_>>();
        let mut owner = None;
        let mut waiters = Vec::new();
        for worker in workers {
            match worker.join().unwrap() {
                Ok(reservation) => {
                    assert!(owner.is_none());
                    assert!(reservation.authorizes(&gate));
                    owner = Some(reservation);
                }
                Err(wait) => waiters.push(wait),
            }
        }
        assert_eq!(waiters.len(), 3);
        assert!(waiters.iter().all(|wait| !wait.is_released()));
        drop(owner);
        assert!(waiters.iter().all(ArchiveCaptureWait::is_released));
        assert!(gate.ensure_unreserved().is_ok());
    }
}
