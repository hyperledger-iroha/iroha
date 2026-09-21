//! Actual Kura fences retain exact release-driven retries without storage writes.

use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

fn fences(kura: &Kura) -> [(&'static str, &PublicationMutex); 4] {
    [
        ("prune_lock", &kura.prune_lock),
        ("canonical_chain_lock", &kura.canonical_chain_lock),
        ("lane_geometry_lock", &kura.lane_geometry_lock),
        ("sidecar_lock", &kura.sidecar_lock),
    ]
}

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn poll(wait: &mut concread::release::ReleaseFuture, count: &Arc<WakeCount>) -> Poll<()> {
    let waker = Waker::from(Arc::clone(count));
    Pin::new(wait).poll(&mut Context::from_waker(&waker))
}

fn busy(kura: &Kura, expected: &str) -> concread::release::ReleaseWait {
    match kura.try_publication_lease() {
        Err(KuraPublicationPreparationError::Busy { field, wait }) => {
            assert_eq!(field, expected);
            wait
        }
        Err(KuraPublicationPreparationError::Storage(error)) => {
            panic!("unexpected storage refusal: {error}")
        }
        Ok(_) => panic!("joint acquisition must refuse the actual held fence"),
    }
}

#[test]
fn preparation_diagnostics_identify_busy_owner_and_storage_failure() {
    let kura = Kura::blank_kura_for_testing();
    let held = kura.sidecar_lock.lock();
    let error = kura.try_publication_lease().err().expect("sidecar is held");
    let diagnostic = format!("{error:?}");
    assert!(diagnostic.contains("sidecar_lock"));
    assert!(diagnostic.contains("wait"));
    drop(held);

    kura.canonical_storage_poisoned
        .store(true, Ordering::Release);
    let error = kura
        .try_publication_lease()
        .err()
        .expect("storage is poisoned");
    assert!(format!("{error:?}").contains("CanonicalStoragePoisoned"));
}

#[test]
fn every_busy_kura_fence_releases_earlier_guards_and_wakes_only_on_its_owner() {
    let kura = Kura::blank_kura_for_testing();
    let other = Kura::blank_kura_for_testing();
    for (blocked, (name, lock)) in fences(&kura).into_iter().enumerate() {
        let held = lock.lock();
        let mut wait = busy(&kura, name).wait_for_release();
        let count = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &count).is_pending());
        for (index, (_, unlocked)) in fences(&kura).into_iter().enumerate() {
            if index != blocked {
                drop(
                    unlocked
                        .try_lock_or_wait()
                        .expect("earlier guards released"),
                );
            }
        }
        drop(other.try_publication_lease().expect("independent Kura"));
        assert_eq!(count.0.load(Ordering::SeqCst), 0);
        assert!(poll(&mut wait, &count).is_pending());
        drop(held);
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &count).is_ready());
        drop(kura.try_publication_lease().expect("exact owner retry"));
    }
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 0);
}

#[test]
fn joint_kura_lease_holds_every_actual_fence_and_unwind_releases_each() {
    let kura = Kura::blank_kura_for_testing();
    for abort in [false, true] {
        let lease = kura.try_publication_lease().expect("all fences acquired");
        let count = Arc::new(WakeCount::default());
        let mut waits: Vec<_> = fences(&kura)
            .into_iter()
            .map(|(_, lock)| {
                lock.try_lock_or_wait()
                    .err()
                    .expect("lease owns actual fence")
                    .wait_for_release()
            })
            .collect();
        for wait in &mut waits {
            assert!(poll(wait, &count).is_pending());
        }
        if abort {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _lease = lease;
                panic!("abort before publishing anything");
            }));
            assert!(result.is_err());
        } else {
            drop(lease);
        }
        assert_eq!(count.0.load(Ordering::SeqCst), 4);
        for wait in &mut waits {
            assert!(poll(wait, &count).is_ready());
        }
        drop(
            kura.try_publication_lease()
                .expect("no poisoning on unwind"),
        );
    }
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 0);
}

#[test]
fn real_canonical_and_queue_plan_leases_wake_the_same_physical_wait() {
    let kura = Kura::blank_kura_for_testing();
    let canonical = kura.canonical_publication_lease();
    let mut wait = busy(&kura, "canonical_chain_lock").wait_for_release();
    let count = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &count).is_pending());
    drop(canonical);
    assert!(poll(&mut wait, &count).is_ready());
    let queue = kura
        .try_queue_plan_publication_at_height(0)
        .unwrap()
        .unwrap();
    let mut wait = busy(&kura, "canonical_chain_lock").wait_for_release();
    assert!(poll(&mut wait, &count).is_pending());
    drop(queue);
    assert!(poll(&mut wait, &count).is_ready());
    assert_eq!(count.0.load(Ordering::SeqCst), 2);
    drop(
        kura.try_publication_lease()
            .expect("exact retry after production lease"),
    );
}

#[test]
fn release_before_registration_and_successor_contention_remain_distinct() {
    let kura = Kura::blank_kura_for_testing();
    for (name, lock) in fences(&kura) {
        let held = lock.lock();
        let mut before = busy(&kura, name).wait_for_release();
        drop(held);
        let successor = lock.lock();
        let mut after = busy(&kura, name).wait_for_release();
        let count = Arc::new(WakeCount::default());
        assert!(poll(&mut before, &count).is_ready());
        assert!(poll(&mut after, &count).is_pending());
        drop(successor);
        assert!(poll(&mut after, &count).is_ready());
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn storage_reconstruction_errors_release_all_fences_without_a_busy_retry() {
    let kura = Kura::blank_kura_for_testing();
    for poisoned in [false, true] {
        let flag = if poisoned {
            &kura.canonical_storage_poisoned
        } else {
            &kura.prune_recovery_required
        };
        flag.store(true, Ordering::Release);
        let prune = kura.prune_lock.lock();
        let sidecar = kura.sidecar_lock.lock();
        if poisoned {
            assert!(matches!(
                kura.try_publication_lease(),
                Err(KuraPublicationPreparationError::Storage(
                    Error::CanonicalStoragePoisoned
                ))
            ));
        } else {
            // A healthy active prune also sets the recovery flag until its
            // durable intent finishes. Its actual release is still reachable.
            assert!(matches!(
                kura.try_publication_lease(),
                Err(KuraPublicationPreparationError::Busy {
                    field: "prune_lock",
                    ..
                })
            ));
        }
        drop(prune);
        let canonical = kura.canonical_chain_lock.lock();
        // Once prune is free, a retained intent needs recovery. Neither this
        // permanent refusal nor poison waits on unrelated canonical/sidecar work.
        assert!(matches!(
            kura.try_publication_lease(),
            Err(KuraPublicationPreparationError::Storage(_))
        ));
        drop(canonical);
        drop(sidecar);
        for (_, lock) in fences(&kura) {
            drop(
                lock.try_lock_or_wait()
                    .expect("storage error releases fence"),
            );
        }
        flag.store(false, Ordering::Release);
        drop(kura.try_publication_lease().expect("test flag reset"));
    }
}

#[test]
fn cancellation_of_one_waiter_does_not_consume_another_waiters_release() {
    let kura = Kura::blank_kura_for_testing();
    for (name, lock) in fences(&kura) {
        let held = lock.lock();
        let mut canceled = busy(&kura, name).wait_for_release();
        let mut retained = busy(&kura, name).wait_for_release();
        let canceled_count = Arc::new(WakeCount::default());
        let retained_count = Arc::new(WakeCount::default());
        assert!(poll(&mut canceled, &canceled_count).is_pending());
        assert!(poll(&mut retained, &retained_count).is_pending());
        drop(canceled);
        held.unlock_fair();
        assert_eq!(canceled_count.0.load(Ordering::SeqCst), 0);
        assert_eq!(retained_count.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut retained, &retained_count).is_ready());
    }
}

#[test]
fn deferred_kura_lease_unlocks_every_original_fence_before_reentrant_callbacks() {
    struct Reenter {
        kura: Arc<Kura>,
        outer: Arc<PublicationMutex>,
        wakes: AtomicUsize,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            assert!(
                self.outer.try_lock_or_wait().is_ok(),
                "enclosing fence released"
            );
            for (name, lock) in fences(&self.kura) {
                assert!(lock.try_lock_or_wait().is_ok(), "original {name} released");
            }
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }
    }
    for unwind in [false, true] {
        let kura = Kura::blank_kura_for_testing();
        let outer = Arc::new(PublicationMutex::default());
        let guard = outer.lock();
        let lease = kura.try_publication_lease().unwrap();
        let probe = Arc::new(Reenter {
            kura: Arc::clone(&kura),
            outer: Arc::clone(&outer),
            wakes: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut waits: Vec<_> = fences(&kura)
            .into_iter()
            .map(|(_, lock)| lock.try_lock_or_wait().err().unwrap().wait_for_release())
            .collect();
        for wait in &mut waits {
            assert!(
                Pin::new(wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
        }
        let released = lease.release_deferred();
        assert_eq!(probe.wakes.load(Ordering::SeqCst), 0);
        if unwind {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let _released = released;
                    let _outer = guard;
                    panic!("Kura completion unwind");
                }))
                .is_err()
            );
        } else {
            drop(guard);
            drop(released);
        }
        assert_eq!(probe.wakes.load(Ordering::SeqCst), 4);
        for wait in &mut waits {
            assert!(
                Pin::new(wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_ready()
            );
        }
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 0);
    }
}

struct ReenterEveryKuraFence {
    kura: Arc<Kura>,
    blocked: Option<&'static str>,
    wakes: AtomicUsize,
}
impl Wake for ReenterEveryKuraFence {
    fn wake(self: Arc<Self>) {
        for (name, lock) in fences(&self.kura) {
            if self.blocked != Some(name) {
                assert!(lock.try_lock_or_wait().is_ok(), "{name} still held at wake");
            }
        }
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn partial_kura_refusal_releases_every_acquired_fence_before_callbacks() {
    let kura = Kura::blank_kura_for_testing();
    for (blocked, lock) in fences(&kura).into_iter().skip(1) {
        let first = kura.prune_lock.lock();
        let mut wait = kura
            .prune_lock
            .try_lock_or_wait()
            .err()
            .unwrap()
            .wait_for_release();
        let initial = first.release_deferred();
        let callback = Arc::new(ReenterEveryKuraFence {
            kura: Arc::clone(&kura),
            blocked: Some(blocked),
            wakes: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&callback));
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let held = lock.lock();
        let mut refused = busy(&kura, blocked).wait_for_release();
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        assert!(
            Pin::new(&mut refused)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        drop(held);
        assert!(
            Pin::new(&mut refused)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        drop(wait);
        drop(initial);
        drop(kura.try_publication_lease().unwrap());
    }
}

#[test]
fn full_and_partial_kura_abandonment_release_jointly_even_on_unwind() {
    let kura = Kura::blank_kura_for_testing();
    for count in 1..=4 {
        for unwind in [false, true] {
            let mut owner = AcquiredKuraPublicationFences::new(&kura);
            owner.prune = Some(kura.prune_lock.lock());
            if count >= 2 {
                owner.canonical = Some(kura.canonical_chain_lock.lock());
            }
            if count >= 3 {
                owner.geometry = Some(kura.lane_geometry_lock.lock());
            }
            if count >= 4 {
                owner.sidecar = Some(kura.sidecar_lock.lock());
            }
            let callback = Arc::new(ReenterEveryKuraFence {
                kura: Arc::clone(&kura),
                blocked: None,
                wakes: AtomicUsize::new(0),
            });
            let waker = Waker::from(Arc::clone(&callback));
            let mut waits: Vec<_> = fences(&kura)
                .into_iter()
                .take(count)
                .map(|(_, lock)| lock.try_lock_or_wait().err().unwrap().wait_for_release())
                .collect();
            for wait in &mut waits {
                assert!(
                    Pin::new(wait)
                        .poll(&mut Context::from_waker(&waker))
                        .is_pending()
                );
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let _original = owner;
                assert!(!unwind, "partial Kura acquisition unwound");
            }));
            assert_eq!(result.is_err(), unwind);
            assert_eq!(callback.wakes.load(Ordering::SeqCst), count);
            for wait in &mut waits {
                assert!(
                    Pin::new(wait)
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_ready()
                );
            }
        }
    }
    // The actual successful production wrapper must retain this same owner.
    let lease = kura.try_publication_lease().unwrap();
    let callback = Arc::new(ReenterEveryKuraFence {
        kura: Arc::clone(&kura),
        blocked: None,
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    let mut wait = kura
        .sidecar_lock
        .try_lock_or_wait()
        .err()
        .unwrap()
        .wait_for_release();
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    drop(lease);
    assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
}

#[test]
fn cold_kura_sidecar_wakes_after_joint_success_and_real_storage_refusal() {
    for failure in [None, Some(false), Some(true)] {
        let (_directory, kura, expected, hash) =
            super::super::tests::pending_canonical_merge_capacity_fixture();
        if let Some(corrupt) = failure {
            let path = kura.pending_merge_entry_path(hash);
            if corrupt {
                std::fs::write(path, b"corrupt exact pending entry").unwrap();
            } else {
                std::fs::remove_file(path).unwrap();
            }
        }
        let sidecar = kura.sidecar_lock.lock();
        let mut wait = kura
            .sidecar_lock
            .try_lock_or_wait()
            .err()
            .unwrap()
            .wait_for_release();
        let initial = sidecar.release_deferred();
        let callback = Arc::new(ReenterEveryKuraFence {
            kura: Arc::clone(&kura),
            blocked: None,
            wakes: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&callback));
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let result = kura.try_publication_lease();
        match result {
            Ok(lease) => {
                assert!(failure.is_none());
                assert_eq!(lease.pending_canonical_bytes(), expected);
                assert_eq!(callback.wakes.load(Ordering::SeqCst), 0);
                drop(lease);
            }
            Err(KuraPublicationPreparationError::Storage(_)) => assert!(failure.is_some()),
            Err(error) => panic!("storage refusal must not become Busy: {error:?}"),
        }
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        drop(wait);
        drop(initial);
    }
}

#[test]
fn repeated_cold_kura_lookups_retain_one_batch_through_outer_unwind() {
    let (_directory, kura, expected, _) =
        super::super::tests::pending_canonical_merge_capacity_fixture();
    let sidecar = kura.sidecar_lock.lock();
    let mut wait = kura
        .sidecar_lock
        .try_lock_or_wait()
        .err()
        .unwrap()
        .wait_for_release();
    let initial = sidecar.release_deferred();
    let callback = Arc::new(ReenterEveryKuraFence {
        kura: Arc::clone(&kura),
        blocked: None,
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let mut owner = AcquiredKuraPublicationFences::new(&kura);
    owner.prune = Some(kura.prune_lock.lock());
    owner.canonical = Some(kura.canonical_chain_lock.lock());
    for _ in 0..3 {
        kura.invalidate_pending_budget_cache();
        assert_eq!(
            kura.try_pending_canonical_capacity_bytes_under_prune_and_canonical_guards(&mut owner)
                .unwrap(),
            expected
        );
        assert!(owner.sidecar.is_none());
        assert_eq!(callback.wakes.load(Ordering::SeqCst), 0);
        // The real sidecar is physically available before merge-log operations.
        owner.sidecar = Some(kura.sidecar_lock.try_lock_or_wait().unwrap());
        owner.release_cold_sidecar().unwrap();
    }
    assert_eq!(kura.pending_budget_raw_scans.load(Ordering::Relaxed), 3);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _original = owner;
            panic!("cold scan caller unwound");
        }))
        .is_err()
    );
    assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
    drop(wait);
    drop(initial);
}

#[test]
fn foreign_cold_batch_returns_original_guard_for_joint_cleanup() {
    let kura = Kura::blank_kura_for_testing();
    let foreign = Kura::blank_kura_for_testing();
    let mut owner = AcquiredKuraPublicationFences::new(&kura);
    owner.prune = Some(kura.prune_lock.lock());
    owner.sidecar = Some(kura.sidecar_lock.lock());
    owner.cold_sidecar = Some(foreign.sidecar_lock.deferred_releases());
    assert!(matches!(
        owner.release_cold_sidecar(),
        Err(KuraPublicationPreparationError::Storage(_))
    ));
    assert!(owner.sidecar.is_some());
    assert!(kura.sidecar_lock.try_lock_or_wait().is_err());
    let callback = Arc::new(ReenterEveryKuraFence {
        kura: Arc::clone(&kura),
        blocked: None,
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&callback));
    let mut wait = kura
        .sidecar_lock
        .try_lock_or_wait()
        .err()
        .unwrap()
        .wait_for_release();
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    drop(owner);
    assert_eq!(callback.wakes.load(Ordering::SeqCst), 1);
}
