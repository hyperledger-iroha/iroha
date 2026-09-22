//! Original physical fence release batches outlive enclosing publication owners.
use super::*;
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};

struct CheckPhysicalFences {
    locks: [Arc<PublicationMutex>; 2],
    calls: AtomicUsize,
    busy: AtomicUsize,
}
impl Wake for CheckPhysicalFences {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        for lock in &self.locks {
            if lock.inner.try_lock().is_none() {
                self.busy.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}

#[test]
fn scoped_fence_coalesces_original_releases_after_all_siblings_on_return_and_unwind() {
    for unwind in [false, true] {
        let locks = [
            Arc::new(PublicationMutex::default()),
            Arc::new(PublicationMutex::default()),
        ];
        let callback = Arc::new(CheckPhysicalFences {
            locks: locks.clone(),
            calls: AtomicUsize::new(0),
            busy: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&callback));
        let mut cx = Context::from_waker(&waker);
        let mut outer = locks[0].defer_notifications();
        let guard = outer.lock();
        let wait = locks[0]
            .try_lock_or_wait()
            .err()
            .expect("original guard is physically held");
        let mut future = Box::pin(wait.clone().wait_for_release());
        assert!(future.as_mut().poll(&mut cx).is_pending());
        drop(guard);
        assert_eq!(callback.calls.load(Ordering::SeqCst), 0);
        assert!(locks[0].inner.try_lock().is_some());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut outer = outer;
            let _sibling = locks[1].lock();
            let _original = outer.lock();
            assert!(locks[0].inner.try_lock().is_none());
            if unwind {
                panic!("later aggregate preparation failed");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
        assert_eq!(callback.busy.load(Ordering::SeqCst), 0);
        assert!(future.as_mut().poll(&mut cx).is_ready());
        // PublicationMutex uses a non-poisoning parking-lot mutex.
        assert!(!wait.is_poisoned());
    }
}

#[test]
fn empty_scoped_fence_emits_no_synthetic_release() {
    let original: PublicationMutex = PublicationMutex::default();
    let held = original.lock();
    let mut future = Box::pin(
        original
            .try_lock_or_wait()
            .err()
            .unwrap()
            .wait_for_release(),
    );
    drop(original.defer_notifications());
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(held);
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
}

#[test]
fn scoped_guard_preserves_mutations_and_defers_release_on_return_and_unwind() {
    for unwind in [false, true] {
        let original = PublicationMutex::new(vec![1_u8]);
        let mut owner = original.defer_notifications();
        let mut wait = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut guard = owner.lock();
            assert_eq!(&*guard, &[1]);
            guard.push(2);
            wait = original.try_lock_or_wait().err();
            if unwind {
                panic!("unwind through original typed deferred guard");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(&*original.inner.try_lock().unwrap(), &[1, 2]);
        let mut future = Box::pin(wait.unwrap().wait_for_release());
        let mut context = Context::from_waker(Waker::noop());
        assert!(future.as_mut().poll(&mut context).is_pending());
        drop(owner);
        assert!(future.as_mut().poll(&mut context).is_ready());
        assert_eq!(&*original.lock(), &[1, 2]);
    }
}
