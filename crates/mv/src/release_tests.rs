//! Release observations survive registration races without owning data locks.

use super::*;
use crate::{PublicationPreparationError, cell::Cell, storage::Storage};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    task::Wake,
};

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn poll(wait: &mut ReleaseFuture, wake: &Arc<WakeCount>) -> Poll<()> {
    Pin::new(wait).poll(&mut Context::from_waker(&Waker::from(Arc::clone(wake))))
}
fn busy<E>(error: PublicationPreparationError<E>) -> ReleaseFuture {
    let PublicationPreparationError::Busy(wait) = error else {
        panic!("expected exact lock contention");
    };
    wait.wait_for_release()
}

#[test]
fn release_before_registration_is_retained_and_other_sources_do_not_wake() {
    let source = ReleaseNotification::default();
    let other = ReleaseNotification::default();
    let lock = Mutex::new(());
    let guard = source.guard(lock.lock().unwrap());
    let mut wait = source.observe().wait_for_release();
    let wake = Arc::new(WakeCount::default());
    drop(other.guard(()));
    assert!(poll(&mut wait, &wake).is_pending());
    let mut before_first_poll = source.observe().wait_for_release();
    drop(guard);
    assert!(lock.try_lock().is_ok());
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
    assert!(poll(&mut before_first_poll, &wake).is_ready());
}

#[test]
fn cancellation_and_waker_replacement_do_not_steal_another_wait() {
    let source = ReleaseNotification::default();
    let observation = source.observe();
    let mut canceled = observation.clone().wait_for_release();
    let mut retained = observation.wait_for_release();
    let old = Arc::new(WakeCount::default());
    let new = Arc::new(WakeCount::default());
    assert!(poll(&mut canceled, &old).is_pending());
    assert!(poll(&mut retained, &old).is_pending());
    assert!(poll(&mut retained, &new).is_pending());
    drop(canceled);
    assert_eq!(source.state.lock().unwrap().waiters.len(), 1);
    drop(source.guard(()));
    assert_eq!(old.0.load(Ordering::SeqCst), 0);
    assert_eq!(new.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut retained, &new).is_ready());
    assert!(source.state.lock().unwrap().waiters.is_empty());
    let mut canceled = source.observe().wait_for_release();
    assert!(poll(&mut canceled, &old).is_pending());
    drop(canceled);
    assert_eq!(source.state.lock().unwrap().waiters.capacity(), 0);
}

#[test]
fn release_racing_first_poll_cannot_be_lost() {
    for _ in 0..128 {
        let source = ReleaseNotification::default();
        let mut wait = source.observe().wait_for_release();
        let wake = Arc::new(WakeCount::default());
        std::thread::scope(|scope| {
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let release_barrier = Arc::clone(&barrier);
            let source = &source;
            let release = scope.spawn(move || {
                let guard = source.guard(());
                release_barrier.wait();
                drop(guard);
            });
            barrier.wait();
            let first = poll(&mut wait, &wake);
            release.join().unwrap();
            if first.is_pending() {
                assert_eq!(wake.0.load(Ordering::SeqCst), 1);
            }
            assert!(poll(&mut wait, &wake).is_ready());
        });
    }
}

#[test]
fn a_nonpoisoning_guard_unwind_does_not_poison_later_contention() {
    let source = ReleaseNotification::default();
    let mut wait = source.observe().wait_for_release();
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = source.guard(());
        panic!("read-only owner unwind");
    }));
    assert!(unwound.is_err());
    assert!(poll(&mut wait, &wake).is_ready());
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(matches!(
        PublicationPreparationError::<()>::after_failed_acquisition(source.observe()),
        PublicationPreparationError::Busy(_)
    ));
}

#[test]
fn inner_guard_destructor_panic_still_signals_after_its_physical_lock_releases() {
    struct PanicOnDrop<'a> {
        _guard: std::sync::MutexGuard<'a, ()>,
    }
    impl Drop for PanicOnDrop<'_> {
        fn drop(&mut self) {
            panic!("inner destructor failed");
        }
    }
    let source = ReleaseNotification::default();
    let physical = Mutex::new(());
    let guard = source.poisoning_guard(PanicOnDrop {
        _guard: physical.lock().unwrap(),
    });
    let mut wait = source.observe().wait_for_release();
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(guard)));
    assert!(unwound.is_err());
    assert!(matches!(
        physical.try_lock(),
        Err(std::sync::TryLockError::Poisoned(_))
    ));
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
    assert!(matches!(
        PublicationPreparationError::<()>::after_failed_acquisition(source.observe()),
        PublicationPreparationError::Poisoned
    ));
}

#[test]
fn cell_abort_detach_and_publication_release_the_actual_busy_writer() {
    for finish in 0..5 {
        let target = Cell::new(10_u64);
        let original = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
        let block = target.block();
        let (journal, error) = original
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("block owns writers");
        let mut wait = busy(error);
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        match finish {
            0 => drop(block),
            1 => {
                block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            }
            2 => {
                assert!(block.try_detach(|_| Err::<(), _>("capacity")).is_err());
            }
            3 => block.commit(),
            _ => {
                let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let _block = block;
                    panic!("abort owner");
                }));
                assert!(unwind.is_err());
            }
        }
        assert!(poll(&mut wait, &wake).is_ready());
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        let retry = journal.try_prepare_publication(&target, |_, _| Ok::<_, ()>(()));
        if finish == 4 {
            assert!(matches!(
                retry.err().unwrap().1,
                PublicationPreparationError::Poisoned
            ));
        } else if finish == 3 {
            assert!(matches!(
                retry.err().unwrap().1,
                PublicationPreparationError::Changed
            ));
        } else {
            assert!(retry.is_ok());
        }
    }
}

#[test]
fn storage_prepared_drop_abort_and_publish_release_the_original_writers() {
    for finish in 0..3 {
        let target: Storage<u64, u64> = [(1, 10)].into_iter().collect();
        let journal = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
        let competitor = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
        let prepared = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|_| panic!("first publisher"));
        let (competitor, error) = competitor
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("prepared publisher owns writers");
        let mut wait = busy(error);
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        match finish {
            0 => drop(prepared),
            1 => {
                prepared.abort();
            }
            _ => {
                prepared.publish();
            }
        }
        assert!(poll(&mut wait, &wake).is_ready());
        let retry = competitor.try_prepare_publication(&target, |_, _| Ok::<_, ()>(()));
        if finish == 2 {
            assert!(matches!(
                retry.err().unwrap().1,
                PublicationPreparationError::Changed
            ));
        } else {
            assert!(retry.is_ok());
        }
    }
}

#[test]
fn partial_writer_acquisition_does_not_wake_its_own_refused_lock() {
    let target = Cell::new(10_u64);
    let journal = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
    let current = target.blocks_released.guard(target.blocks.write());
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .err()
        .expect("current writer is busy");
    let mut wait = busy(error);
    let wake = Arc::new(WakeCount::default());
    assert!(
        target.revert.try_write().is_some(),
        "partial undo writer released"
    );
    assert!(
        poll(&mut wait, &wake).is_pending(),
        "undo release is not current release"
    );
    drop(current);
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &wake).is_ready());
    assert!(
        journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .is_ok()
    );
}

#[test]
fn cell_prepared_and_storage_original_guards_notify_every_release_path() {
    for finish in 0..3 {
        let target = Cell::new(10_u64);
        let journal = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
        let prepared = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|_| panic!("prepare"));
        let mut wait = target.revert_released.observe().wait_for_release();
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        match finish {
            0 => drop(prepared),
            1 => {
                prepared.abort();
            }
            _ => {
                prepared.publish();
            }
        }
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &wake).is_ready());
        assert!(target.revert.try_write().is_some());
        assert!(target.blocks.try_write().is_some());
    }
    for finish in 0..4 {
        let target: Storage<u64, u64> = [(1, 10)].into_iter().collect();
        let journal = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
        let block = target.block();
        let (_, error) = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("original writer");
        let mut wait = busy(error);
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        match finish {
            0 => drop(block),
            1 => {
                block.try_detach(|_| Ok::<_, ()>(())).unwrap();
            }
            2 => {
                assert!(block.try_detach(|_| Err::<(), _>("capacity")).is_err());
            }
            _ => block.commit(),
        }
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &wake).is_ready());
        assert!(target.revert.try_write().is_some());
        assert!(target.blocks.try_write().is_some());
    }
}
