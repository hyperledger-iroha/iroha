//! Process-owned backend state with leases that survive administrative release.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

// An unavailable driver is not a permanent property of a process: a device
// can appear after startup. Failed discovery remains cached for a bounded
// interval so ordinary CPU execution never repeatedly probes the driver.
const DISCOVERY_RETRY: Duration = Duration::from_secs(30);

enum State<T> {
    Undiscovered,
    Unavailable { retry_after: Instant },
    Ready(Arc<T>),
}

pub(super) struct ProcessOwner<T>(Mutex<State<T>>);

impl<T> ProcessOwner<T> {
    pub(super) const fn new() -> Self {
        Self(Mutex::new(State::Undiscovered))
    }

    /// Initialize at most once per discovery epoch and retain an independent lease.
    pub(super) fn acquire(&self, initialize: impl FnOnce() -> Option<T>) -> Option<Arc<T>> {
        self.acquire_with_clock(Instant::now, initialize)
    }

    fn acquire_with_clock(
        &self,
        now: impl FnOnce() -> Instant,
        initialize: impl FnOnce() -> Option<T>,
    ) -> Option<Arc<T>> {
        let mut state = self.0.lock().ok()?;
        let now = now();
        if matches!(*state, State::Undiscovered)
            || matches!(*state, State::Unavailable { retry_after } if now >= retry_after)
        {
            *state = initialize().map_or_else(
                || State::Unavailable {
                    retry_after: now.checked_add(DISCOVERY_RETRY).unwrap_or(now),
                },
                |value| State::Ready(Arc::new(value)),
            );
        }
        match &*state {
            State::Ready(value) => Some(Arc::clone(value)),
            State::Undiscovered | State::Unavailable { .. } => None,
        }
    }

    /// Start a new discovery epoch without invalidating in-flight leases.
    pub(super) fn release(&self) {
        let previous = self
            .0
            .lock()
            .ok()
            .map(|mut state| std::mem::replace(&mut *state, State::Undiscovered));
        // Destructors run after unlocking; backend cleanup may call other owners.
        drop(previous);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn concurrent_workers_share_one_discovery_and_state() {
        let owner = ProcessOwner::new();
        let discoveries = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        owner
                            .acquire(|| {
                                discoveries.fetch_add(1, Ordering::SeqCst);
                                Some(37)
                            })
                            .expect("available backend")
                    })
                })
                .collect();
            let values: Vec<_> = workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect();
            assert!(values.iter().all(|value| Arc::ptr_eq(value, &values[0])));
            assert_eq!(*values[0], 37);
        });
        assert_eq!(discoveries.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unavailable_discovery_retries_after_cooldown_or_release() {
        let owner: ProcessOwner<u8> = ProcessOwner::new();
        let now = Instant::now();
        assert!(owner.acquire_with_clock(|| now, || None).is_none());
        assert!(
            owner
                .acquire_with_clock(
                    || now + DISCOVERY_RETRY - Duration::from_nanos(1),
                    || panic!("must not repeat failed discovery before cooldown")
                )
                .is_none()
        );
        let recovered = owner
            .acquire_with_clock(|| now + DISCOVERY_RETRY, || Some(7))
            .expect("newly available device");
        assert_eq!(*recovered, 7);
        assert_eq!(
            *owner
                .acquire_with_clock(
                    || now + DISCOVERY_RETRY * 2,
                    || { panic!("a qualified backend remains owned") }
                )
                .unwrap(),
            7
        );
        owner.release();
        assert_eq!(*owner.acquire(|| Some(8)).unwrap(), 8);
        assert_eq!(*recovered, 7, "release retains the old borrower's state");
    }

    #[test]
    fn concurrent_retry_discovers_only_once() {
        let owner: ProcessOwner<u8> = ProcessOwner::new();
        let now = Instant::now();
        assert!(owner.acquire_with_clock(|| now, || None).is_none());
        let attempts = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        owner
                            .acquire_with_clock(
                                || now + DISCOVERY_RETRY,
                                || {
                                    attempts.fetch_add(1, Ordering::SeqCst);
                                    Some(17)
                                },
                            )
                            .expect("recovered backend")
                    })
                })
                .collect();
            let values: Vec<_> = workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect();
            assert!(values.iter().all(|value| Arc::ptr_eq(value, &values[0])));
        });
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn release_retains_borrowers_and_reclaims_after_last_lease() {
        let owner = ProcessOwner::new();
        let first = owner.acquire(|| Some(11)).unwrap();
        let weak = Arc::downgrade(&first);
        let borrower = owner.acquire(|| panic!("already discovered")).unwrap();
        owner.release();
        let next = owner.acquire(|| Some(12)).unwrap();
        assert!(!Arc::ptr_eq(&next, &first));
        drop(first);
        assert_eq!(*borrower, 11);
        assert!(weak.upgrade().is_some());
        drop(borrower);
        assert!(weak.upgrade().is_none());
        assert_eq!(*next, 12);
    }

    #[test]
    fn released_backend_destructor_can_reenter_owner() {
        struct Backend(Arc<ProcessOwner<Backend>>);
        impl Drop for Backend {
            fn drop(&mut self) {
                assert!(self.0.0.try_lock().is_ok(), "cleanup held owner lock");
            }
        }
        let owner = Arc::new(ProcessOwner::new());
        drop(owner.acquire(|| Some(Backend(Arc::clone(&owner)))));
        owner.release();
    }
}
