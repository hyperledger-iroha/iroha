//! Retained physical custody distinguishes abandonment from failed mutation.

use super::*;
use crate::internals::bptree::node::allocation_tests::without_allocations;
use std::{
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    task::{Context, Waker},
};

#[derive(Debug)]
struct Scalar {
    live: usize,
    fail_transfer: bool,
}
impl LinCowCellCapable<usize, usize> for Scalar {
    type WriterInput = ();
    fn create_reader(&self) -> usize {
        self.live
    }
    fn create_writer(&self, (): ()) -> usize {
        self.live + 1
    }
    fn pre_commit(&mut self, new: usize, _: &usize) -> usize {
        self.live = new;
        new
    }
}
impl retained_commit::Sealed for Scalar {}
impl LinCowCellRetainedCommit<usize, usize> for Scalar {
    type Retirement = ();
    fn validate_commit(&self, _: &usize, prev: &usize) {
        assert_eq!(self.live, *prev);
    }
    fn pre_commit_retaining(&mut self, new: usize, _: &usize) -> (usize, ()) {
        self.live = new;
        assert!(
            !self.fail_transfer,
            "actual original root mutation interrupted"
        );
        (new, ())
    }
}
fn scalar(fail_transfer: bool) -> LinCowCell<Scalar, usize, usize> {
    LinCowCell::new(Scalar {
        live: 7,
        fail_transfer,
    })
}

#[test]
fn retained_native_acquisition_outer_unwind_preserves_original_root_and_retry() {
    for prepared in [false, true] {
        let cell = scalar(false);
        let original = cell.read();
        let work = cell.write().detach();
        let pointer = std::ptr::from_ref(work.as_ref());
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let acquired = cell.try_acquire_owned_retained(work).unwrap();
            let writer = acquired.validate().unwrap();
            assert_eq!(std::ptr::from_ref(writer.as_ref()), pointer);
            let mut slot = writer.commit_slot();
            if prepared {
                slot.try_prepare().unwrap();
            }
            panic!("later unrelated component failed");
        }))
        .is_err());
        assert!(!cell.is_poisoned());
        assert!(!cell.active.is_poisoned());
        assert!(!cell.observe_reader_release().is_poisoned());
        assert_eq!(*cell.read().as_ref(), 7);
        assert_eq!(*original.as_ref(), 7);
        drop(cell.write().prepare_commit().publish().release());
        assert_eq!(*cell.read().as_ref(), 8);
    }
}

#[test]
fn retained_native_phases_move_original_owners_without_allocations() {
    let cell = scalar(false);
    let work = cell.write().detach();
    let pointer = std::ptr::from_ref(work.as_ref());
    let mut slot = without_allocations(|| {
        let writer = cell
            .try_acquire_owned_retained(work)
            .unwrap()
            .validate()
            .unwrap();
        assert_eq!(std::ptr::from_ref(writer.as_ref()), pointer);
        writer.commit_slot()
    });
    without_allocations(|| slot.try_prepare().unwrap());
    let (writer, notice) = without_allocations(|| slot.abort_retaining());
    let work = without_allocations(|| writer.detach());
    assert_eq!(std::ptr::from_ref(work.as_ref()), pointer);
    without_allocations(|| drop(notice));
    let retired = without_allocations(|| {
        let writer = cell
            .try_acquire_owned_retained(work)
            .unwrap()
            .validate()
            .unwrap();
        let mut slot = writer.commit_slot();
        slot.try_prepare().unwrap();
        slot.into_prepared().publish().release()
    });
    assert_eq!(*cell.read().as_ref(), 8);
    drop(retired);
}

#[test]
fn actual_joint_root_transfer_unwind_poison_is_permanent() {
    let cell = scalar(true);
    let work = cell.write().detach();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let writer = cell
            .try_acquire_owned_retained(work)
            .unwrap()
            .validate()
            .unwrap();
        drop(writer.prepare_commit().publish().release());
    }))
    .is_err());
    assert!(cell.is_poisoned());
    assert!(cell.active.is_poisoned());
    assert!(cell.observe_reader_release().is_poisoned());
    assert!(matches!(cell.try_read(), Err(OwnedWriteError::Poisoned)));
    let acquired = cell
        .try_acquire_writer()
        .expect("poison retains actual physical ownership");
    assert!(acquired.is_poisoned());
    drop(acquired);
    assert!(cell.is_poisoned());
}

#[test]
fn raw_native_root_and_active_lock_panics_remain_poisoned() {
    for active in [false, true] {
        let cell = scalar(false);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            if active {
                let _raw = cell.active.lock().unwrap();
                panic!("raw active owner failure");
            } else {
                let _raw = cell.write.lock().unwrap();
                panic!("raw root owner failure");
            }
        }))
        .is_err());
        assert_eq!(cell.active.is_poisoned(), active);
        assert_eq!(cell.is_poisoned(), !active);
    }
}

#[test]
fn retained_mutable_access_arms_poison_before_exposing_data() {
    for active in [false, true] {
        let cell = scalar(false);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            if active {
                let mut guard = cell.active.try_lock_retained().unwrap();
                let original = guard.clone();
                *guard = original;
                panic!("active mutation interrupted");
            } else {
                let mut guard = cell.write.try_lock_retained().unwrap();
                guard.data.live = 99;
                panic!("root mutation interrupted");
            }
        }))
        .is_err());
        assert_eq!(cell.active.is_poisoned(), active);
        assert_eq!(cell.is_poisoned(), !active);
    }
}

#[test]
fn successful_retained_publish_then_outer_unwind_preserves_complete_generation() {
    let cell = scalar(false);
    let work = cell.write().detach();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let writer = cell
            .try_acquire_owned_retained(work)
            .unwrap()
            .validate()
            .unwrap();
        let _published = writer.prepare_commit().publish();
        panic!("later outer component failed after complete publication");
    }))
    .is_err());
    assert!(!cell.is_poisoned());
    assert!(!cell.active.is_poisoned());
    assert!(!cell.observe_reader_release().is_poisoned());
    assert_eq!(*cell.read().as_ref(), 8);
}

#[test]
fn retained_reader_wait_tracks_actual_release_without_fabricated_poison() {
    let cell = scalar(false);
    let work = cell.write().detach();
    let writer = cell
        .try_acquire_owned_retained(work)
        .unwrap()
        .validate()
        .unwrap();
    let mut slot = writer.commit_slot();
    slot.try_prepare().unwrap();
    let observation = cell.observe_reader_release();
    let mut wait = observation.clone().wait_for_release();
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    let (writer, notice) = without_allocations(|| slot.abort_retaining());
    let work = without_allocations(|| writer.detach());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    without_allocations(|| drop(notice));
    assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    assert!(!observation.is_poisoned());
    drop(work);
}

#[test]
fn default_owned_preparation_retains_conservative_outer_unwind_poison() {
    let cell = scalar(false);
    let work = cell.write().detach();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let writer = cell.try_acquire_owned(work).unwrap().validate().unwrap();
        let _prepared = writer.prepare_commit();
        panic!("ordinary custody remains conservative");
    }))
    .is_err());
    assert!(cell.is_poisoned());
    assert!(cell.active.is_poisoned());
    assert!(cell.observe_reader_release().is_poisoned());
}

#[test]
fn retained_busy_and_stale_refusals_return_same_original_work() {
    let cell = scalar(false);
    let work = cell.write().detach();
    let pointer = std::ptr::from_ref(work.as_ref());
    let held = cell.write.lock().unwrap();
    let (work, error) = without_allocations(|| cell.try_acquire_owned_retained(work).unwrap_err());
    assert_eq!(error, OwnedWriteError::Busy);
    assert_eq!(std::ptr::from_ref(work.as_ref()), pointer);
    drop(held);
    cell.write().commit();
    let acquired = without_allocations(|| cell.try_acquire_owned_retained(work).unwrap());
    let (acquired, error) = without_allocations(|| acquired.validate().unwrap_err());
    assert_eq!(error, OwnedWriteError::Changed);
    let work = without_allocations(|| acquired.abort());
    assert_eq!(std::ptr::from_ref(work.as_ref()), pointer);
    assert!(!cell.is_poisoned());
    assert_eq!(*cell.read().as_ref(), 8);
    drop(work);
}

#[test]
fn deferred_observed_notice_freezes_actual_poison_after_original_unlock() {
    struct Retain<'a, 'out> {
        guard: Option<crate::release::ReleaseGuard<'a, retained_mutex::MutexGuard<'a, usize>>>,
        notice: &'out mut Option<crate::release::DeferredRelease>,
    }
    impl Drop for Retain<'_, '_> {
        fn drop(&mut self) {
            let (_, notice) = self.guard.take().unwrap().release_deferred(drop);
            *self.notice = Some(notice);
        }
    }
    for mutated in [false, true] {
        let mutex = Mutex::new(7_usize);
        let source = crate::release::ReleaseNotification::default();
        let observation = source.observe();
        let mut wait = observation.clone().wait_for_release();
        let mut context = Context::from_waker(Waker::noop());
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        let mut notice = None;
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let mut guard = mutex.try_lock_retained().unwrap();
            if mutated {
                *guard = 8;
            }
            let _owner = Retain {
                guard: Some(source.observed_guard(guard, mutex.poison_flag())),
                notice: &mut notice,
            };
            panic!("outer cleanup retains actual notice");
        }))
        .is_err());
        assert_eq!(mutex.is_poisoned(), mutated);
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        without_allocations(|| drop(notice));
        assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
        assert_eq!(observation.is_poisoned(), mutated);
    }
}

#[test]
fn chained_retirement_preserves_original_recorded_poison_verdict() {
    for poisoned in [false, true] {
        let mutex = Mutex::new(7_usize);
        if poisoned {
            assert!(catch_unwind(AssertUnwindSafe(|| {
                let _raw = mutex.lock().unwrap();
                panic!("actual primitive failure before cleanup");
            }))
            .is_err());
        }
        let source = crate::release::ReleaseNotification::default();
        let observation = source.observe();
        let mut wait = observation.clone().wait_for_release();
        let mut context = Context::from_waker(Waker::noop());
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        // A poisoned guard may only be released; it grants no retry authority.
        let guard = mutex
            .lock_retained()
            .unwrap_or_else(|error| error.into_inner());
        let first = source.observed_guard(guard, mutex.poison_flag());
        let first_retirement = without_allocations(|| first.release_retaining(drop));
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        let second_retirement = without_allocations(|| {
            first_retirement
                .map_preserving_release(|()| ())
                .release_retaining(|()| ())
        });
        assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
        without_allocations(|| drop(second_retirement));
        assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
        assert_eq!(observation.is_poisoned(), poisoned);
        assert_eq!(mutex.is_poisoned(), poisoned);
    }
}
