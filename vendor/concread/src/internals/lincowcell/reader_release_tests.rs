//! Actual active-reader releases remain in the caller's original bounded batch.
use super::*;
use crate::bptree::BptreeMap;
use crate::internals::bptree::cursor::{CursorRead, CursorReadOps, CursorWrite, SuperBlock};
use crate::internals::bptree::node::allocation_tests::without_allocations;
use std::{
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Wake, Waker},
};

type Tree =
    LinCowCell<SuperBlock<usize, usize>, CursorRead<usize, usize>, CursorWrite<usize, usize>>;
fn tree() -> Tree {
    // SAFETY: the new unique tree is immediately installed in its original owner.
    LinCowCell::new(unsafe { SuperBlock::new() })
}
fn pending(wait: &mut crate::release::ReleaseFuture) -> bool {
    Pin::new(wait)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending()
}

#[test]
fn original_public_map_read_releases_wait_until_after_the_caller_fence() {
    struct CheckFence {
        fence: Arc<std::sync::Mutex<()>>,
        calls: AtomicUsize,
    }
    impl Wake for CheckFence {
        fn wake(self: Arc<Self>) {
            assert!(
                self.fence.try_lock().is_ok(),
                "reader callback ran under caller fence"
            );
            self.calls.fetch_add(1, Ordering::SeqCst);
        }
    }
    for nonblocking in [false, true] {
        let map = BptreeMap::<usize, usize>::new();
        let fence = Arc::new(std::sync::Mutex::new(()));
        let check = Arc::new(CheckFence {
            fence: fence.clone(),
            calls: AtomicUsize::new(0),
        });
        let waker = Waker::from(check.clone());
        let mut context = Context::from_waker(&waker);
        let mut wait = map.observe_reader_release().wait_for_release();
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        let mut batch = without_allocations(|| map.reader_release_batch());
        let held = fence.lock().unwrap();
        let reader = without_allocations(|| {
            if nonblocking {
                map.try_read_retaining(&mut batch).unwrap()
            } else {
                map.read_retaining(&mut batch).unwrap()
            }
        });
        assert!(reader.is_empty());
        drop(reader);
        assert_eq!(check.calls.load(Ordering::SeqCst), 0);
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        drop(held);
        without_allocations(|| drop(batch));
        assert_eq!(check.calls.load(Ordering::SeqCst), 1);
        assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    }
}

#[test]
fn a_foreign_batch_refuses_before_blocking_acquisition_or_any_notice() {
    let cell = tree();
    let foreign = tree();
    let mut batch = foreign.reader_release_batch();
    let mut actual_wait = cell.observe_reader_release().wait_for_release();
    let mut foreign_wait = foreign.observe_reader_release().wait_for_release();
    assert!(pending(&mut actual_wait) && pending(&mut foreign_wait));
    // Keeping the actual native lock proves the source check precedes even the
    // blocking API. No guard is acquired and no original source may be signaled.
    let held = cell.active.lock().unwrap();
    assert!(matches!(
        without_allocations(|| cell.read_retaining(&mut batch)),
        Err(OwnedWriteError::Changed)
    ));
    assert!(matches!(
        without_allocations(|| cell.try_read_retaining(&mut batch)),
        Err(OwnedWriteError::Changed)
    ));
    drop(held);
    drop(batch);
    assert!(pending(&mut actual_wait) && pending(&mut foreign_wait));
}

#[test]
fn busy_acquires_nothing_and_an_empty_batch_cannot_fabricate_a_release() {
    let cell = tree();
    let mut batch = cell.reader_release_batch();
    let mut wait = cell.observe_reader_release().wait_for_release();
    assert!(pending(&mut wait));
    let held = cell.active.lock().unwrap();
    assert!(matches!(
        without_allocations(|| cell.try_read_retaining(&mut batch)),
        Err(OwnedWriteError::Busy)
    ));
    drop(batch);
    assert!(pending(&mut wait));
    drop(held);
    assert!(
        pending(&mut wait),
        "raw guard did not claim notification custody"
    );
    drop(cell.try_read().unwrap());
    assert!(
        !pending(&mut wait),
        "the later real notifying owner wakes normally"
    );
}

#[test]
fn acquired_poison_is_recorded_only_after_actual_unlock_and_batch_release() {
    for nonblocking in [false, true] {
        let cell = tree();
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _held = cell.active.lock().unwrap();
            panic!("real raw active mutex unwind");
        }))
        .is_err());
        let observation = cell.observe_reader_release();
        assert!(
            !observation.is_poisoned(),
            "raw poison has not yet been reported"
        );
        let mut wait = observation.clone().wait_for_release();
        assert!(pending(&mut wait));
        let mut batch = cell.reader_release_batch();
        for _ in 0..2 {
            let result = without_allocations(|| {
                if nonblocking {
                    cell.try_read_retaining(&mut batch)
                } else {
                    cell.read_retaining(&mut batch)
                }
            });
            assert!(matches!(result, Err(OwnedWriteError::Poisoned)));
            assert!(
                matches!(cell.active.try_lock(), Err(TryLockError::Poisoned(_))),
                "acquired poisoned guard was actually unlocked"
            );
            assert!(pending(&mut wait));
            assert!(!observation.is_poisoned());
        }
        drop(batch);
        assert!(!pending(&mut wait));
        assert!(observation.is_poisoned());
        assert!(matches!(cell.try_read(), Err(OwnedWriteError::Poisoned)));
    }
}

#[test]
fn successful_reads_coalesce_without_retaining_physical_locks() {
    let cell = tree();
    let mut batch = cell.reader_release_batch();
    let mut wait = cell.observe_reader_release().wait_for_release();
    assert!(pending(&mut wait));
    for _ in 0..256 {
        drop(without_allocations(|| {
            cell.try_read_retaining(&mut batch).unwrap()
        }));
        assert!(cell.active.try_lock().is_ok());
    }
    assert!(pending(&mut wait));
    drop(batch);
    assert!(!pending(&mut wait));
}

#[test]
fn retained_reader_and_release_batch_have_independent_lifetimes() {
    let map = BptreeMap::<usize, usize>::new();
    let mut batch = map.reader_release_batch();
    let mut wait = map.observe_reader_release().wait_for_release();
    assert!(pending(&mut wait));
    let reader = map.read_retaining(&mut batch).unwrap();
    drop(batch);
    assert!(!pending(&mut wait));
    let mut writer = map.write();
    writer.insert(1, 7);
    writer.commit();
    assert!(
        reader.is_empty(),
        "original reader outlives the release batch and publication"
    );
    assert_eq!(map.read().get(&1), Some(&7));
}

#[test]
fn one_original_batch_can_span_distinct_committed_generations() {
    let cell = tree();
    let mut batch = cell.reader_release_batch();
    let first = cell.read_retaining(&mut batch).unwrap();
    let mut writer = cell.write();
    writer.insert(1, 7);
    writer.commit();
    let second = cell.try_read_retaining(&mut batch).unwrap();
    assert_eq!(first.search(&1), None);
    assert_eq!(second.search(&1), Some(&7));
    assert!(!first.predecessor().same_predecessor(&second.predecessor()));
    drop((first, second));
    drop(batch);
}

#[test]
fn outer_unwind_keeps_completed_immutable_reads_unpoisoned_and_deferred() {
    let cell = tree();
    let observation = cell.observe_reader_release();
    let mut wait = observation.clone().wait_for_release();
    assert!(pending(&mut wait));
    let mut batch = cell.reader_release_batch();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _reader = cell.read_retaining(&mut batch).unwrap();
        panic!("outer caller still owns its release batch");
    }))
    .is_err());
    assert!(pending(&mut wait));
    assert!(!cell.active.is_poisoned());
    drop(batch);
    assert!(!pending(&mut wait));
    assert!(!observation.is_poisoned());
    assert!(cell.try_read().is_ok());
}

#[test]
fn actual_guard_unwind_transfer_records_poison_before_any_batch_callback() {
    let cell = tree();
    let observation = cell.observe_reader_release();
    let mut wait = observation.clone().wait_for_release();
    assert!(pending(&mut wait));
    let mut batch = cell.reader_release_batch();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let active = cell.active.lock().unwrap();
        let guard = cell
            .active_released
            .observed_guard(active, cell.active.poison_flag());
        let _: Result<(), _> = guard.try_release_into(&mut batch, |_active| {
            panic!("actual raw guard unwinds inside the sole transfer engine");
        });
    }))
    .is_err());
    assert!(cell.active.is_poisoned());
    assert!(pending(&mut wait));
    assert!(!observation.is_poisoned());
    drop(batch);
    assert!(!pending(&mut wait));
    assert!(observation.is_poisoned());
}

#[test]
fn actual_original_batch_survives_map_destruction_without_losing_wake() {
    let (batch, observation) = {
        let cell = tree();
        let mut batch = cell.reader_release_batch();
        let observation = cell.observe_reader_release();
        drop(cell.read_retaining(&mut batch).unwrap());
        (batch, observation)
    };
    let mut wait = observation.wait_for_release();
    assert!(pending(&mut wait));
    drop(batch);
    assert!(!pending(&mut wait));
}

#[test]
fn existing_default_read_still_notifies_its_actual_release_immediately() {
    let cell = tree();
    for nonblocking in [false, true] {
        let mut wait = cell.observe_reader_release().wait_for_release();
        assert!(pending(&mut wait));
        let reader = if nonblocking {
            cell.try_read().unwrap()
        } else {
            cell.read()
        };
        assert!(!pending(&mut wait));
        drop(reader);
    }
}
