//! Actual reader/writer release, callback, and deferred-custody controls.

use super::*;
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

struct Probe {
    original: Arc<PublicationRwLock<u32>>,
    sibling: Arc<crate::publication_lock::PublicationMutex>,
    calls: AtomicUsize,
    writer_free: AtomicUsize,
    sibling_free: AtomicUsize,
}

impl Probe {
    fn new(original: &Arc<PublicationRwLock<u32>>) -> Arc<Self> {
        Arc::new(Self {
            original: Arc::clone(original),
            sibling: Arc::new(crate::publication_lock::PublicationMutex::default()),
            calls: AtomicUsize::new(0),
            writer_free: AtomicUsize::new(0),
            sibling_free: AtomicUsize::new(0),
        })
    }

    fn inspect(&self) {
        // Nonblocking physical probes only. Assertions happen after cleanup,
        // including when this callback runs during an original guard's unwind.
        let writer = self.original.try_write();
        let sibling = self.sibling.try_lock();
        self.writer_free
            .fetch_add(usize::from(writer.is_some()), Ordering::SeqCst);
        self.sibling_free
            .fetch_add(usize::from(sibling.is_some()), Ordering::SeqCst);
        self.calls.fetch_add(1, Ordering::SeqCst);
    }

    fn assert_released_once(&self) {
        assert_eq!(self.calls.load(Ordering::SeqCst), 1);
        assert_eq!(self.writer_free.load(Ordering::SeqCst), 1);
        assert_eq!(self.sibling_free.load(Ordering::SeqCst), 1);
    }
}

impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        self.inspect();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.inspect();
    }
}

#[test]
fn readers_exclude_writer_and_each_real_release_requires_reacquisition() {
    let original = PublicationRwLock::new(7_u32);
    let first = original.read();
    let second = original.try_read().expect("concurrent original reader");
    assert_eq!(*first, *second);
    let wait = original
        .try_write_or_wait()
        .err()
        .expect("both readers held");
    let mut pending = std::pin::pin!(wait.clone().wait_for_release());
    let mut context = Context::from_waker(Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    let foreign = PublicationRwLock::new(9_u32);
    drop(foreign.write());
    drop(foreign.read());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    drop(first);
    assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
    assert!(
        original.try_write().is_none(),
        "one release is not exclusive authority"
    );
    let retry = original
        .try_write_or_wait()
        .err()
        .expect("second reader still held");
    assert_ne!(wait, retry);
    drop(second);
    let mut retry = std::pin::pin!(retry.wait_for_release());
    assert_eq!(retry.as_mut().poll(&mut context), Poll::Ready(()));
    assert_eq!(
        *original
            .try_write_or_wait()
            .expect("original readers released"),
        7
    );
}

#[test]
fn writer_excludes_both_access_modes_and_release_before_poll_is_observed() {
    let mut original = PublicationRwLock::<Vec<u8>>::default();
    let exclusive_observation = original.released.observe();
    original.get_mut().push(1);
    let mut exclusive_pending = std::pin::pin!(exclusive_observation.wait_for_release());
    assert!(
        exclusive_pending
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    let mut held = original.write();
    held.push(3);
    assert_eq!(format!("{held:?}"), "[1, 3]");
    assert!(original.try_read().is_none());
    assert!(original.try_write().is_none());
    let wait = original
        .try_write_or_wait()
        .err()
        .expect("original writer held");
    assert!(format!("{original:?}").contains("PublicationRwLock"));
    drop(held);
    let mut pending = std::pin::pin!(wait.wait_for_release());
    assert_eq!(
        pending
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    );
    let reader = original.read();
    assert_eq!(&*reader, &[1, 3]);
    assert_eq!(format!("{reader:?}"), "[1, 3]");
}

#[test]
fn reader_and_writer_wakes_probe_the_original_unlocked_backend() {
    for writer in [false, true] {
        let original = Arc::new(PublicationRwLock::new(7_u32));
        let probe = Probe::new(&original);
        let read = (!writer).then(|| original.read());
        let write = writer.then(|| original.write());
        let wait = original
            .try_write_or_wait()
            .err()
            .expect("original physical owner");
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut pending = std::pin::pin!(wait.wait_for_release());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        drop(read);
        drop(write);
        probe.assert_released_once();
        assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
    }
}

#[test]
fn deferred_read_and_write_notifications_wait_for_enclosing_sibling_release() {
    for writer in [false, true] {
        let original = Arc::new(PublicationRwLock::new(7_u32));
        let probe = Probe::new(&original);
        let sibling = probe.sibling.lock();
        let read = (!writer).then(|| original.read());
        let write = writer.then(|| original.write());
        let wait = original
            .try_write_or_wait()
            .err()
            .expect("original physical owner");
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut pending = std::pin::pin!(wait.wait_for_release());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        let retired = match (read, write) {
            (Some(read), None) => read.release_deferred(),
            (None, Some(write)) => write.release_deferred(),
            _ => unreachable!("one original physical access mode"),
        };
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        assert!(pending.as_mut().poll(&mut context).is_pending());
        drop(sibling);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        drop(retired);
        probe.assert_released_once();
        assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
    }
}

#[test]
fn foreign_batches_return_the_original_reader_or_writer_without_unlocking() {
    let original = PublicationRwLock::new(7_u32);
    let foreign = PublicationRwLock::new(9_u32);
    let mut foreign_batch = foreign.deferred_releases();
    let held = original.read();
    let wait = original
        .try_write_or_wait()
        .err()
        .expect("original reader held");
    let held = held
        .try_release_into(&mut foreign_batch)
        .err()
        .expect("foreign batch rejected");
    assert!(original.try_write().is_none());
    assert_eq!(*held, 7);
    let mut pending = std::pin::pin!(wait.wait_for_release());
    let mut context = Context::from_waker(Waker::noop());
    drop(foreign_batch);
    assert!(pending.as_mut().poll(&mut context).is_pending());
    drop(held);
    assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));

    let mut foreign_batch = foreign.deferred_releases();
    let mut held = original.write();
    *held = 11;
    let wait = original
        .try_write_or_wait()
        .err()
        .expect("original writer held");
    let held = held
        .try_release_into(&mut foreign_batch)
        .err()
        .expect("foreign batch rejected");
    assert!(original.try_read().is_none());
    assert_eq!(*held, 11);
    let mut pending = std::pin::pin!(wait.wait_for_release());
    drop(foreign_batch);
    assert!(pending.as_mut().poll(&mut context).is_pending());
    drop(held);
    assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
}

#[test]
fn original_batch_retains_multiple_actual_read_and_write_releases() {
    let original = Arc::new(PublicationRwLock::new(7_u32));
    let probe = Probe::new(&original);
    let mut batch = original.deferred_releases();
    let first = original.read();
    let second = original.read();
    let wait = original
        .try_write_or_wait()
        .err()
        .expect("both readers held");
    let waker = Waker::from(Arc::clone(&probe));
    let mut context = Context::from_waker(&waker);
    let mut pending = std::pin::pin!(wait.wait_for_release());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    assert!(first.try_release_into(&mut batch).is_ok());
    assert!(second.try_release_into(&mut batch).is_ok());
    let mut writer = original.try_write().expect("both actual readers released");
    *writer = 12;
    assert!(writer.try_release_into(&mut batch).is_ok());
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    assert!(pending.as_mut().poll(&mut context).is_pending());
    drop(batch);
    probe.assert_released_once();
    assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
    assert_eq!(*original.read(), 12);
}

#[test]
fn reader_and_writer_unwind_unlock_before_callback_without_poison() {
    for writer in [false, true] {
        let original = Arc::new(PublicationRwLock::new(7_u32));
        let probe = Probe::new(&original);
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut observation = None;
        let mut pending = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _read = (!writer).then(|| original.read());
            let mut write = writer.then(|| original.write());
            if let Some(write) = &mut write {
                **write = 13;
            }
            let wait = original
                .try_write_or_wait()
                .err()
                .expect("original physical owner");
            observation = Some(wait.clone());
            pending = Some(Box::pin(wait.wait_for_release()));
            assert!(
                pending
                    .as_mut()
                    .unwrap()
                    .as_mut()
                    .poll(&mut context)
                    .is_pending()
            );
            panic!("unwind while the actual original reader or writer is held");
        }));
        assert!(result.is_err());
        probe.assert_released_once();
        assert!(!observation.unwrap().is_poisoned());
        assert_eq!(
            pending.as_mut().unwrap().as_mut().poll(&mut context),
            Poll::Ready(())
        );
        assert_eq!(*original.read(), if writer { 13 } else { 7 });
    }
}

#[test]
fn wake_panic_cannot_poison_the_already_unlocked_writer() {
    struct PanicWake;
    impl Wake for PanicWake {
        fn wake(self: Arc<Self>) {
            panic!("injected wake callback failure");
        }
    }
    let original = PublicationRwLock::new(7_u32);
    let held = original.write();
    let wait = original
        .try_write_or_wait()
        .err()
        .expect("original writer held");
    let waker = Waker::from(Arc::new(PanicWake));
    let mut context = Context::from_waker(&waker);
    let mut pending = std::pin::pin!(wait.clone().wait_for_release());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(held))).is_err());
    assert!(!wait.is_poisoned());
    assert!(original.try_write().is_some());
    assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
}
