//! Release observations survive registration races without owning data locks.

use super::*;
use crate::{PublicationPreparationError, cell::Cell, storage::Storage};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
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

struct ObserveOnDrop {
    source: Arc<ReleaseNotification>,
    state_was_unlocked: Arc<AtomicBool>,
    drops: Arc<AtomicUsize>,
}

impl Wake for ObserveOnDrop {
    fn wake(self: Arc<Self>) {}
}

impl Drop for ObserveOnDrop {
    fn drop(&mut self) {
        // Detect the deadlock without blocking the test on a reentrant observe.
        let unlocked = self.source.state.try_lock().is_ok();
        self.state_was_unlocked.store(unlocked, Ordering::SeqCst);
        self.drops.fetch_add(1, Ordering::SeqCst);
        if unlocked {
            drop(self.source.observe());
        }
    }
}

#[test]
fn replacing_a_waker_allows_its_destructor_to_observe_the_same_source() {
    let source = Arc::new(ReleaseNotification::default());
    let mut wait = source.observe().wait_for_release();
    let state_was_unlocked = Arc::new(AtomicBool::new(false));
    let drops = Arc::new(AtomicUsize::new(0));
    let old = Waker::from(Arc::new(ObserveOnDrop {
        source: Arc::clone(&source),
        state_was_unlocked: Arc::clone(&state_was_unlocked),
        drops: Arc::clone(&drops),
    }));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&old))
            .is_pending()
    );
    drop(old);

    let replacement = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &replacement).is_pending());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(state_was_unlocked.load(Ordering::SeqCst));
    drop(source.guard(()));
    assert_eq!(replacement.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &replacement).is_ready());
}

#[test]
fn ready_wait_releases_its_last_waker_outside_the_notification_lock() {
    use std::sync::mpsc;

    struct PausedWake {
        started: mpsc::SyncSender<()>,
        resume: Mutex<mpsc::Receiver<()>>,
    }
    impl Wake for PausedWake {
        fn wake(self: Arc<Self>) {
            self.started.send(()).unwrap();
            self.resume.lock().unwrap().recv().unwrap();
        }
    }

    let source = Arc::new(ReleaseNotification::default());
    let observation = source.observe();
    let mut first = observation.clone().wait_for_release();
    let mut ready = observation.wait_for_release();
    let (started, started_rx) = mpsc::sync_channel(0);
    let (resume, resume_rx) = mpsc::sync_channel(0);
    let first_waker = Waker::from(Arc::new(PausedWake {
        started,
        resume: Mutex::new(resume_rx),
    }));
    assert!(
        Pin::new(&mut first)
            .poll(&mut Context::from_waker(&first_waker))
            .is_pending()
    );

    let state_was_unlocked = Arc::new(AtomicBool::new(false));
    let drops = Arc::new(AtomicUsize::new(0));
    let old = Waker::from(Arc::new(ObserveOnDrop {
        source: Arc::clone(&source),
        state_was_unlocked: Arc::clone(&state_was_unlocked),
        drops: Arc::clone(&drops),
    }));
    assert!(
        Pin::new(&mut ready)
            .poll(&mut Context::from_waker(&old))
            .is_pending()
    );
    drop(old);

    std::thread::scope(|scope| {
        let source = Arc::clone(&source);
        let release = scope.spawn(move || drop(source.guard(())));
        // The release advanced the sequence and took both registrations, but
        // its first callback prevents it from upgrading the second weak owner.
        started_rx.recv().unwrap();
        let result = poll(&mut ready, &Arc::new(WakeCount::default()));
        resume.send(()).unwrap();
        release.join().unwrap();
        assert!(result.is_ready());
    });
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(state_was_unlocked.load(Ordering::SeqCst));
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
fn acquisition_unwind_notifies_after_raw_lock_release_without_a_published_guard() {
    let source = ReleaseNotification::default();
    let physical = Mutex::new(());
    let mut wait = source.observe().wait_for_release();
    let wake = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wake).is_pending());
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        source.with_acquisition_unwind_notification(|| {
            let _raw = physical.lock().unwrap();
            panic!("clone failed before returning its acquired writer");
        });
    }));
    assert!(failure.is_err());
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

    let source = ReleaseNotification::default();
    let physical = Mutex::new(());
    let mut wait = source.observe().wait_for_release();
    let guard = source.with_acquisition_unwind_notification(|| physical.lock().unwrap());
    let guard = source.poisoning_guard(guard);
    assert!(poll(&mut wait, &wake).is_pending());
    assert_eq!(wake.0.load(Ordering::SeqCst), 1);
    drop(guard);
    assert!(poll(&mut wait, &wake).is_ready());
    assert_eq!(wake.0.load(Ordering::SeqCst), 2);
}

#[test]
fn storage_first_undo_clone_panic_wakes_an_already_registered_retry() {
    use std::sync::{Barrier, atomic::AtomicBool};

    #[derive(Debug)]
    struct CloneGate {
        value: u64,
        armed: Arc<AtomicBool>,
        entered: Arc<Barrier>,
        finish: Arc<Barrier>,
    }
    impl Clone for CloneGate {
        fn clone(&self) -> Self {
            if self.armed.swap(false, Ordering::SeqCst) {
                self.entered.wait();
                self.finish.wait();
                panic!("first undo clone failed while a retry was waiting");
            }
            Self {
                value: self.value,
                armed: Arc::clone(&self.armed),
                entered: Arc::clone(&self.entered),
                finish: Arc::clone(&self.finish),
            }
        }
    }
    let armed = Arc::new(AtomicBool::new(false));
    let entered = Arc::new(Barrier::new(2));
    let finish = Arc::new(Barrier::new(2));
    let value = CloneGate {
        value: 7,
        armed: Arc::clone(&armed),
        entered: Arc::clone(&entered),
        finish: Arc::clone(&finish),
    };
    let target = Storage::from_iter([(1_u64, value.clone())]);
    let mut tip = target.block();
    let _ = tip.insert(1, value.clone());
    tip.commit();
    let mut block = target.block();
    let _ = block.insert(1, value);
    let original = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
    armed.store(true, Ordering::SeqCst);
    std::thread::scope(|scope| {
        let failing = scope.spawn(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(target.block())))
        });
        entered.wait();
        let (journal, error) = original
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("raw undo writer remains held during its clone");
        let mut wait = busy(error);
        let wake = Arc::new(WakeCount::default());
        assert!(poll(&mut wait, &wake).is_pending());
        finish.wait();
        assert!(failing.join().unwrap().is_err());
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut wait, &wake).is_ready());
        let (_, error) = journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("poison requires owner reconstruction");
        assert!(matches!(error, PublicationPreparationError::Poisoned));
    });
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
