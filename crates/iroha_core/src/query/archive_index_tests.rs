//! Actual archive readers and writers drive retry without timers or publication.

use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

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

fn waiting<T>(index: &ArchiveIndexLock<T>) -> mv::ReleaseFuture {
    match index.try_write() {
        Err(ArchiveIndexLockError::Busy(wait)) => wait.wait_for_release(),
        outcome => panic!("expected actual physical contention: {outcome:?}"),
    }
}

#[test]
fn reader_release_before_registration_survives_successor_writer() {
    let index = ArchiveIndexLock::new(7);
    let reader = index.read().unwrap();
    assert_eq!(*reader, 7);
    let mut before_release = waiting(&index);
    drop(reader);
    let mut successor = index.try_write().unwrap();
    *successor = 9;
    let mut after_release = waiting(&index);
    let count = Arc::new(WakeCount::default());
    assert!(poll(&mut before_release, &count).is_ready());
    assert!(poll(&mut after_release, &count).is_pending());
    drop(successor);
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut after_release, &count).is_ready());
    assert_eq!(*index.read().unwrap(), 9);
}

#[test]
fn each_reader_release_wakes_only_its_index_and_retry_rechecks_remaining_readers() {
    let index = ArchiveIndexLock::new(1);
    let foreign = ArchiveIndexLock::new(2);
    let first = index.read().unwrap();
    let second = index.read().unwrap();
    let mut first_wait = waiting(&index);
    let count = Arc::new(WakeCount::default());
    assert!(poll(&mut first_wait, &count).is_pending());
    drop(foreign.read().unwrap());
    drop(foreign.write().unwrap());
    assert_eq!(count.0.load(Ordering::SeqCst), 0);
    assert!(poll(&mut first_wait, &count).is_pending());
    drop(first);
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut first_wait, &count).is_ready());
    let mut second_wait = waiting(&index);
    assert!(poll(&mut second_wait, &count).is_pending());
    drop(second);
    assert_eq!(count.0.load(Ordering::SeqCst), 2);
    assert!(poll(&mut second_wait, &count).is_ready());
    assert!(index.try_write().is_ok());
}

#[test]
fn notification_runs_only_after_the_physical_writer_is_unlocked() {
    struct ProbeOnWake {
        index: Arc<ArchiveIndexLock<usize>>,
        unlocked: AtomicBool,
    }
    impl Wake for ProbeOnWake {
        fn wake(self: Arc<Self>) {
            // The release notifier must have dropped the raw writer before it
            // invokes this callback, which synchronously probes that same lock.
            let guard = self.index.try_write().expect("writer released before wake");
            assert_eq!(*guard, 11);
            self.unlocked.store(true, Ordering::SeqCst);
        }
    }
    let index = Arc::new(ArchiveIndexLock::new(10));
    let mut writer = index.write().unwrap();
    *writer = 11;
    let mut wait = waiting(&index);
    let probe = Arc::new(ProbeOnWake {
        index: Arc::clone(&index),
        unlocked: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&probe));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    drop(writer);
    assert!(probe.unlocked.load(Ordering::SeqCst));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
}

#[test]
fn reader_unwind_notifies_without_poisoning_later_writers() {
    let index = ArchiveIndexLock::new(3);
    let reader = index.read().unwrap();
    let mut wait = waiting(&index);
    let count = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &count).is_pending());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _reader = reader;
        panic!("reader abort");
    }));
    assert!(result.is_err());
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &count).is_ready());
    assert_eq!(*index.try_write().unwrap(), 3);
    assert_eq!(*index.write().unwrap(), 3);
}

#[test]
fn writer_unwind_notifies_and_poison_never_becomes_a_contention_retry() {
    let index = ArchiveIndexLock::new(4);
    let writer = index.write().unwrap();
    let mut wait = waiting(&index);
    let count = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &count).is_pending());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _writer = writer;
        panic!("writer abort");
    }));
    assert!(result.is_err());
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
    assert!(poll(&mut wait, &count).is_ready());
    assert!(matches!(
        index.try_write(),
        Err(ArchiveIndexLockError::Poisoned)
    ));
    assert!(matches!(
        index.write(),
        Err(ArchiveIndexLockError::Poisoned)
    ));
    assert!(matches!(index.read(), Err(ArchiveIndexLockError::Poisoned)));
    // Model the brief erroneous read-acquisition window, still wrapping that
    // real guard so its eventual release follows the production notification.
    let erroneous_reader = index.wrap_read(index.inner.read().unwrap_err().into_inner());
    assert!(matches!(
        index.try_write(),
        Err(ArchiveIndexLockError::Poisoned)
    ));
    drop(erroneous_reader);
}
