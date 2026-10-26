//! Physical release waits retain the original snapshot refusal and finite owner.

use super::*;
use iroha_allocation::{AllocationRefusal, release::ReleaseNotification};
use mv::storage::{AdmittedStorageError, StorageRole};
use std::{
    sync::{
        Mutex, TryLockError,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

fn busy(release: iroha_allocation::release::ReleaseWait) -> WorldStateSnapshotError {
    WorldStateSnapshotError::Acquisition(AdmittedStorageError::Busy {
        role: StorageRole::Current,
        release,
    })
}

#[test]
fn names_retained_view_waits_only_for_its_original_publication_release() {
    let storage = Mutex::new(());
    let released = ReleaseNotification::default();
    let held = released.guard(storage.lock().unwrap());
    let budget = AllocationBudget::new(4096);
    let (entered, waiting) = mpsc::channel();
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| {
            let mut entered = Some(entered);
            snapshot_after_busy(&budget, Instant::now() + Duration::from_secs(2), || {
                let original = released.observe();
                match storage.try_lock() {
                    Ok(guard) => {
                        drop(guard);
                        Ok(7)
                    }
                    Err(TryLockError::WouldBlock) => {
                        if let Some(entered) = entered.take() {
                            entered.send(()).unwrap();
                        }
                        Err(WorldStateSnapshotError::View(StateViewError::Busy(
                            original,
                        )))
                    }
                    Err(TryLockError::Poisoned(_)) => panic!("fixture storage poisoned"),
                }
            })
            .unwrap()
        });
        waiting.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(held);
        assert_eq!(reader.join().unwrap(), 7);
    });
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn names_snapshot_hundred_concurrent_waiters_complete_after_original_writer_releases() {
    const REQUESTS: usize = 100;
    let storage = Mutex::new(());
    let released = ReleaseNotification::default();
    let held = released.guard(storage.lock().unwrap());
    let active = AtomicUsize::new(0);
    let maximum = AtomicUsize::new(0);
    let completed = AtomicUsize::new(0);
    let (entered, arrivals) = mpsc::channel();
    std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..REQUESTS {
            let entered = entered.clone();
            let (storage, released, active, maximum, completed) =
                (&storage, &released, &active, &maximum, &completed);
            workers.push(scope.spawn(move || {
                let budget = AllocationBudget::new(4096);
                let mut first_refusal = Some(entered);
                snapshot_after_busy(&budget, Instant::now() + Duration::from_secs(10), || {
                    let observation = released.observe();
                    let guard = match storage.try_lock() {
                        Ok(guard) => released.guard(guard),
                        Err(TryLockError::WouldBlock) => {
                            if let Some(entered) = first_refusal.take() {
                                entered.send(()).unwrap();
                            }
                            return Err(busy(observation));
                        }
                        Err(TryLockError::Poisoned(_)) => {
                            return Err(WorldStateSnapshotError::Acquisition(
                                AdmittedStorageError::Poisoned {
                                    role: StorageRole::Current,
                                },
                            ));
                        }
                    };
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(now, Ordering::SeqCst);
                    // Controlled snapshot work under the actual single writer.
                    std::thread::sleep(Duration::from_millis(1));
                    completed.fetch_add(1, Ordering::SeqCst);
                    active.fetch_sub(1, Ordering::SeqCst);
                    drop(guard);
                    Ok(())
                })
                .unwrap();
                assert_eq!(budget.reserved_bytes(), 0);
            }));
        }
        for _ in 0..REQUESTS {
            arrivals.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert_eq!(completed.load(Ordering::SeqCst), 0);
        drop(held);
        for worker in workers {
            worker.join().unwrap();
        }
    });
    assert_eq!(completed.load(Ordering::SeqCst), REQUESTS);
    assert_eq!(maximum.load(Ordering::SeqCst), 1);
    assert_eq!(active.load(Ordering::SeqCst), 0);
}

#[test]
fn names_snapshot_wait_deadline_keeps_original_busy_and_ignores_unrelated_wakes() {
    let storage = Mutex::new(());
    let released = ReleaseNotification::default();
    let held = released.guard(storage.lock().unwrap());
    let original = released.observe();
    let budget = AllocationBudget::new(4096);
    let mut attempts = 0;
    let worker = std::thread::current();
    let started = Instant::now();
    let deadline = started + Duration::from_millis(30);
    let error = std::thread::scope(|scope| {
        scope.spawn(move || {
            for _ in 0..10 {
                worker.unpark();
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        snapshot_after_busy::<()>(&budget, deadline, || {
            attempts += 1;
            assert!(storage.try_lock().is_err());
            Err(busy(original.clone()))
        })
        .unwrap_err()
    });
    assert!(Instant::now() >= deadline);
    assert_eq!(attempts, 1);
    assert!(matches!(error, WorldStateSnapshotError::Acquisition(
        AdmittedStorageError::Busy { release, .. }) if release == original));
    assert_eq!(budget.reserved_bytes(), 0);
    // The expired waiter was unlinked; the real later release remains harmless.
    drop(held);
    assert!(storage.try_lock().is_ok());
}

#[test]
fn names_snapshot_no_waiter_capacity_retains_one_original_attempt() {
    let released = ReleaseNotification::default();
    let original = released.observe();
    let budget = AllocationBudget::new(0);
    let mut attempts = 0;
    let error = snapshot_after_busy::<()>(&budget, Instant::now() + Duration::from_secs(1), || {
        attempts += 1;
        Err(busy(original.clone()))
    })
    .unwrap_err();
    assert_eq!(attempts, 1);
    assert!(matches!(error, WorldStateSnapshotError::Acquisition(
        AdmittedStorageError::Busy { release, .. }) if release == original));
    assert_eq!(
        snapshot_after_busy(&budget, Instant::now(), || Ok(7)).unwrap(),
        7
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn names_snapshot_never_retries_poison_capacity_or_invalid_originals() {
    let source = AllocationBudget::new(1);
    let occupied = source.try_reserve_bytes(1).unwrap();
    let refusal = source.try_reserve_bytes(1).unwrap_err();
    assert!(matches!(refusal, AllocationRefusal::Capacity { .. }));
    let errors = [
        WorldStateSnapshotError::Acquisition(AdmittedStorageError::Poisoned {
            role: StorageRole::Current,
        }),
        WorldStateSnapshotError::Acquisition(AdmittedStorageError::Changed),
        WorldStateSnapshotError::Acquisition(AdmittedStorageError::Allocation(refusal.clone())),
        WorldStateSnapshotError::Allocation(refusal),
        WorldStateSnapshotError::Invalid("original certified content rejected".into()),
        WorldStateSnapshotError::CutChanged {
            requested_height: 2,
            observed_height: 3,
        },
    ];
    for error in errors {
        let expected = error.to_string();
        let budget = AllocationBudget::new(4096);
        let mut original = Some(error);
        let actual =
            snapshot_after_busy::<()>(&budget, Instant::now() + Duration::from_secs(1), || {
                Err(original
                    .take()
                    .expect("terminal refusal must not be retried"))
            })
            .unwrap_err();
        assert_eq!(actual.to_string(), expected);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    drop(occupied);
}

#[test]
fn names_snapshot_release_does_not_authorize_a_newer_cut() {
    let storage = Mutex::new(());
    let released = ReleaseNotification::default();
    let height = AtomicUsize::new(2);
    let held = released.guard(storage.lock().unwrap());
    let (entered, arrived) = mpsc::channel();
    std::thread::scope(|scope| {
        let (storage, released, height) = (&storage, &released, &height);
        let worker = scope.spawn(move || {
            let budget = AllocationBudget::new(4096);
            let mut attempts = 0;
            let error =
                snapshot_after_busy::<()>(&budget, Instant::now() + Duration::from_secs(2), || {
                    attempts += 1;
                    let observation = released.observe();
                    let Ok(guard) = storage.try_lock() else {
                        entered.send(()).unwrap();
                        return Err(busy(observation));
                    };
                    let _guard = released.guard(guard);
                    Err(WorldStateSnapshotError::CutChanged {
                        requested_height: 2,
                        observed_height: height.load(Ordering::SeqCst) as u64,
                    })
                })
                .unwrap_err();
            assert_eq!(attempts, 2);
            assert!(matches!(
                error,
                WorldStateSnapshotError::CutChanged {
                    requested_height: 2,
                    observed_height: 3,
                }
            ));
            assert_eq!(budget.reserved_bytes(), 0);
        });
        arrived.recv_timeout(Duration::from_secs(1)).unwrap();
        height.store(3, Ordering::SeqCst);
        drop(held);
        worker.join().unwrap();
    });
}

#[test]
fn names_snapshot_unwind_releases_prepaid_waiter() {
    let budget = AllocationBudget::new(4096);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = snapshot_after_busy::<()>(&budget, Instant::now() + Duration::from_secs(1), || {
            panic!("original snapshot callback unwound");
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}
