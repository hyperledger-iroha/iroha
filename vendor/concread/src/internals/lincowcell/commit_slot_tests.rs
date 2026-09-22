//! Caller-owned preparation keeps original data and reader-lock refusal custody.

use super::*;
use crate::internals::bptree::{
    cursor::{CursorRead, CursorReadOps, CursorWrite, SuperBlock},
    node::{allocation_tests::without_allocations, assert_released},
};
use std::{
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
    task::{Context, Waker},
};

type Tree<V> = LinCowCell<SuperBlock<usize, V>, CursorRead<usize, V>, CursorWrite<usize, V>>;

fn tree<V: Clone>() -> Tree<V> {
    // The original tree transfers immediately to its only linear owner.
    LinCowCell::new(unsafe { SuperBlock::new() })
}

#[test]
fn commit_slot_reader_busy_retains_original_writer_and_publishes_same_successor() {
    let cell = tree::<usize>();
    let original = cell.read();
    let mut writer = cell.write();
    writer.insert(1, 7);
    let cursor = std::ptr::from_ref(&*writer.work);
    let base = std::ptr::from_ref(&*writer.base);
    let held_reader = cell.lock_active();
    let mut slot = without_allocations(|| writer.commit_slot());
    without_allocations(|| {
        assert_eq!(slot.try_prepare(), Err(OwnedWriteError::Busy));
        assert!(!slot.is_prepared());
        assert!(cell.write.try_lock().is_err());
        assert_eq!(std::ptr::from_ref(slot.as_ref()), cursor);
    });
    drop(held_reader);
    without_allocations(|| slot.try_prepare().expect("original reader released"));
    assert!(cell.active.try_lock().is_err());
    let (writer, reader_release) = without_allocations(|| slot.abort_retaining());
    assert!(reader_release.is_some());
    assert!(cell.active.try_lock().is_ok());
    assert!(cell.write.try_lock().is_err());
    assert_eq!(std::ptr::from_ref(&*writer.work), cursor);
    assert_eq!(std::ptr::from_ref(&*writer.base), base);
    drop(reader_release);
    let mut slot = without_allocations(|| writer.commit_slot());
    without_allocations(|| slot.prepare());
    let retirement = without_allocations(|| slot.into_prepared().publish().release());
    assert_eq!(cell.read().as_ref().search(&1), Some(&7));
    assert_eq!(original.as_ref().search(&1), None);
    drop(retirement);
    drop(original);
    drop(cell);
    assert_released();
}

struct CloneFault(Arc<AtomicBool>);
impl Clone for CloneFault {
    fn clone(&self) -> Self {
        assert!(!self.0.load(SeqCst), "actual published-node copy fault");
        Self(Arc::clone(&self.0))
    }
}

#[test]
fn commit_slot_failed_cursor_keeps_both_locks_until_caller_abandons_original() {
    let fault = Arc::new(AtomicBool::new(false));
    let cell = tree::<CloneFault>();
    let mut first = cell.write();
    first.insert(1, CloneFault(Arc::clone(&fault)));
    drop(first.prepare_commit().publish().release());
    let original = cell.read();
    let mut writer = cell.write();
    let cursor = std::ptr::from_ref(&*writer.work);
    fault.store(true, SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        writer.insert(2, CloneFault(Arc::clone(&fault)));
    }))
    .is_err());
    fault.store(false, SeqCst);
    let observation = cell.observe_reader_release();
    let mut wait = observation.clone().wait_for_release();
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    let mut slot = writer.commit_slot();
    assert!(catch_unwind(AssertUnwindSafe(|| slot.prepare())).is_err());
    assert!(!slot.is_prepared());
    assert!(
        cell.active.try_lock().is_err(),
        "validation keeps actual acquired reader"
    );
    assert!(
        cell.write.try_lock().is_err(),
        "validation keeps original writer"
    );
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    assert!(catch_unwind(AssertUnwindSafe(|| slot.prepare())).is_err());
    let (writer, release) = slot.abort_retaining();
    assert_eq!(std::ptr::from_ref(&*writer.work), cursor);
    assert!(release.is_some());
    assert!(cell.active.try_lock().is_ok());
    assert!(cell.write.try_lock().is_err());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    // Raw internal detachment is cleanup custody only: never inspect, reattach,
    // or publish the failed cursor. The public map seam enforces this opacity.
    let cleanup = writer.detach();
    assert!(cell.write.try_lock().is_ok());
    assert!(
        !cell.is_poisoned(),
        "caught validation kept the actual guards alive"
    );
    assert!(original.as_ref().search(&1).is_some());
    assert!(original.as_ref().search(&2).is_none());
    drop(cleanup);
    drop(release);
    assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    assert!(!observation.is_poisoned());
    drop(original);
    drop(cell);
    assert_released();
}
