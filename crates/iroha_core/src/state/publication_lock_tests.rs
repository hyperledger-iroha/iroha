//! Real State fence releases wake retries without a new block or publication.

use crate::publication_lock::*;
use crate::state::{State, World};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

fn state() -> State {
    State::new_for_testing(
        World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    )
}

fn locks(state: &State) -> [&PublicationMutex; 3] {
    [
        &state.state_commit_lock,
        &state.state_write_lock,
        &state.lane_lifecycle_lock,
    ]
}

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn poll(wait: &mut mv::ReleaseFuture, count: &Arc<WakeCount>) -> Poll<()> {
    let waker = Waker::from(Arc::clone(count));
    Pin::new(wait).poll(&mut Context::from_waker(&waker))
}
fn waiting(lock: &PublicationMutex) -> mv::ReleaseFuture {
    lock.try_lock_or_wait()
        .err()
        .expect("actual mutex is held")
        .wait_for_release()
}

#[test]
fn fair_unlock_releases_the_physical_mutex_before_waking_publication_retries() {
    struct CheckUnlocked {
        lock: Arc<PublicationMutex>,
        count: AtomicUsize,
    }
    impl Wake for CheckUnlocked {
        fn wake(self: Arc<Self>) {
            let _guard = self
                .lock
                .try_lock()
                .expect("the actual mutex must unlock before notification");
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    let lock = Arc::new(PublicationMutex::default());
    let guard = lock.lock();
    let mut wait = waiting(&lock);
    let check = Arc::new(CheckUnlocked {
        lock: Arc::clone(&lock),
        count: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&check));
    let mut context = Context::from_waker(&waker);
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    guard.unlock_fair();
    assert_eq!(check.count.load(Ordering::SeqCst), 1);
    assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    assert!(lock.try_lock_or_wait().is_ok());
}

#[test]
fn every_state_fence_wakes_on_normal_and_aborted_release_without_publication() {
    let state = state();
    let height = state.committed_height();
    let generation = state.state_view_generation();
    for lock in locks(&state) {
        for aborted in [false, true] {
            let guard = lock.lock();
            let mut wait = waiting(lock);
            let count = Arc::new(WakeCount::default());
            assert!(poll(&mut wait, &count).is_pending());
            if aborted {
                let aborted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let _actual_guard = guard;
                    panic!("abort without publishing");
                }));
                assert!(aborted.is_err());
            } else {
                drop(guard);
            }
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            assert!(poll(&mut wait, &count).is_ready());
            let retry = lock
                .try_lock_or_wait()
                .expect("parking-lot fence remains usable");
            let next_wait = lock
                .try_lock_or_wait()
                .err()
                .expect("ordinary contention after unwind");
            drop(retry);
            assert!(poll(&mut next_wait.wait_for_release(), &count).is_ready());
        }
    }
    assert_eq!(state.committed_height(), height);
    assert_eq!(state.state_view_generation(), generation);
}

#[test]
fn state_fence_release_before_registration_survives_a_successor_guard() {
    let state = state();
    let generation = state.state_view_generation();
    for lock in locks(&state) {
        let original = lock.lock();
        let mut original_wait = waiting(lock);
        drop(original);
        let successor = lock.lock();
        let mut successor_wait = waiting(lock);
        let count = Arc::new(WakeCount::default());
        assert!(poll(&mut original_wait, &count).is_ready());
        assert!(poll(&mut successor_wait, &count).is_pending());
        drop(successor);
        assert!(poll(&mut successor_wait, &count).is_ready());
    }
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.state_view_generation(), generation);
}

#[test]
fn state_fence_waits_are_independent_of_other_fences_and_other_states() {
    let state = state();
    let other = self::state();
    for (index, lock) in locks(&state).into_iter().enumerate() {
        let guard = lock.lock();
        let mut wait = waiting(lock);
        let count = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &count).is_pending());
        for (other_index, other_lock) in locks(&state).into_iter().enumerate() {
            if index != other_index {
                drop(other_lock.lock());
            }
        }
        for other_lock in locks(&other) {
            drop(other_lock.lock());
        }
        assert_eq!(count.0.load(Ordering::SeqCst), 0);
        assert!(poll(&mut wait, &count).is_pending());
        drop(guard);
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &count).is_ready());
    }
}

#[test]
fn real_consensus_and_lifecycle_leases_signal_the_same_original_fences() {
    let state = state();
    let generation = state.state_view_generation();
    let count = Arc::new(WakeCount::default());
    let consensus = state.consensus_publication_lease();
    let mut commit_wait = waiting(&state.state_commit_lock);
    let lifecycle = state.lock_lane_lifecycle_work_admission();
    let mut lifecycle_wait = waiting(&state.lane_lifecycle_lock);
    assert!(poll(&mut commit_wait, &count).is_pending());
    assert!(poll(&mut lifecycle_wait, &count).is_pending());
    drop(consensus);
    assert!(poll(&mut commit_wait, &count).is_ready());
    assert!(poll(&mut lifecycle_wait, &count).is_pending());
    drop(lifecycle);
    assert!(poll(&mut lifecycle_wait, &count).is_ready());
    assert_eq!(count.0.load(Ordering::SeqCst), 2);
    assert_eq!(state.state_view_generation(), generation);
}

#[test]
fn shared_commit_mutex_and_replay_swap_keep_the_actual_release_source() {
    let mut state = state();
    let mut replacement = self::state();
    let original_commit = Arc::clone(&state.state_commit_lock);
    let guard = original_commit.lock();
    let mut wait = waiting(&state.state_commit_lock);
    // The production replay handoff swaps these exact fields to preserve their
    // physical owners. Notification must follow the mutex, not the State address.
    std::mem::swap(
        &mut state.state_commit_lock,
        &mut replacement.state_commit_lock,
    );
    let count = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &count).is_pending());
    drop(state.state_commit_lock.lock());
    assert!(poll(&mut wait, &count).is_pending());
    drop(guard);
    assert!(poll(&mut wait, &count).is_ready());
    assert!(replacement.state_commit_lock.try_lock_or_wait().is_ok());
    for lifecycle in [false, true] {
        let original = if lifecycle {
            &state.lane_lifecycle_lock
        } else {
            &state.state_write_lock
        };
        let guard = original.lock();
        let mut wait = waiting(original);
        drop(guard);
        if lifecycle {
            std::mem::swap(
                &mut state.lane_lifecycle_lock,
                &mut replacement.lane_lifecycle_lock,
            );
        } else {
            std::mem::swap(
                &mut state.state_write_lock,
                &mut replacement.state_write_lock,
            );
        }
        assert!(poll(&mut wait, &count).is_ready());
    }
}

#[test]
fn cancelling_one_state_fence_waiter_preserves_other_waiters() {
    let state = state();
    for lock in locks(&state) {
        let guard = lock.lock();
        let mut canceled = waiting(lock);
        let mut retained = waiting(lock);
        let canceled_count = Arc::new(WakeCount::default());
        let retained_count = Arc::new(WakeCount::default());
        assert!(poll(&mut canceled, &canceled_count).is_pending());
        assert!(poll(&mut retained, &retained_count).is_pending());
        drop(canceled);
        drop(guard);
        assert_eq!(canceled_count.0.load(Ordering::SeqCst), 0);
        assert_eq!(retained_count.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut retained, &retained_count).is_ready());
    }
}

#[test]
fn fair_unlock_releases_actual_state_fence_before_notification() {
    let state = state();
    let generation = state.state_view_generation();
    for lock in locks(&state) {
        let guard = lock.lock();
        let mut wait = waiting(lock);
        let count = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &count).is_pending());
        guard.unlock_fair();
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &count).is_ready());
        let successor = lock.try_lock_or_wait().expect("physical guard released");
        let mut successor_wait = waiting(lock);
        assert!(poll(&mut successor_wait, &count).is_pending());
        drop(successor);
        assert!(poll(&mut successor_wait, &count).is_ready());
    }
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.state_view_generation(), generation);
}
