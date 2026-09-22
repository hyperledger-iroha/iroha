//! Original notification custody across generation completion, refusal and unwind.

use super::*;
use std::{
    future::Future,
    sync::{Arc, atomic::AtomicUsize},
    task::{Context, Wake, Waker},
};

#[derive(Default)]
struct Count(AtomicUsize);

impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn completed_intervals_notify_only_when_the_original_owner_retires() {
    let generation = AtomicU64::new(0);
    let notification = tokio::sync::Notify::new();
    let count = Arc::new(Count::default());
    let waker = Waker::from(Arc::clone(&count));
    let mut wait = std::pin::pin!(notification.notified());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let mut original = StateViewPublication::new(&generation, &notification);
    for before in [0, 2] {
        let guard = original.begin();
        assert_eq!(generation.load(Ordering::Acquire), before + 1);
        drop(guard);
        assert_eq!(generation.load(Ordering::Acquire), before + 2);
        assert_eq!(count.0.load(Ordering::SeqCst), 0);
        assert!(
            wait.as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    drop(original);
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
}

#[test]
fn unused_owner_does_not_change_generation_or_notify() {
    let generation = AtomicU64::new(0);
    let notification = tokio::sync::Notify::new();
    let count = Arc::new(Count::default());
    let waker = Waker::from(Arc::clone(&count));
    let mut wait = std::pin::pin!(notification.notified());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    drop(StateViewPublication::new(&generation, &notification));
    assert_eq!(generation.load(Ordering::Acquire), 0);
    assert_eq!(count.0.load(Ordering::SeqCst), 0);
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
}

#[test]
fn unwind_closes_generation_without_consuming_the_callers_notification() {
    let generation = AtomicU64::new(0);
    let notification = tokio::sync::Notify::new();
    let count = Arc::new(Count::default());
    let waker = Waker::from(Arc::clone(&count));
    let mut wait = std::pin::pin!(notification.notified());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let mut original = StateViewPublication::new(&generation, &notification);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = original.begin();
        panic!("unwind the actual visibility interval");
    }));
    assert!(result.is_err());
    assert_eq!(generation.load(Ordering::Acquire), 2);
    assert_eq!(count.0.load(Ordering::SeqCst), 0);
    drop(original);
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
}

#[test]
fn competing_owner_cannot_perturb_an_active_generation() {
    let generation = AtomicU64::new(0);
    let notification = tokio::sync::Notify::new();
    let mut original = StateViewPublication::new(&generation, &notification);
    let guard = original.begin();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut competing = StateViewPublication::new(&generation, &notification);
        let _guard = competing.begin();
    }));
    assert!(result.is_err());
    assert_eq!(generation.load(Ordering::Acquire), 1);
    drop(guard);
    assert_eq!(generation.load(Ordering::Acquire), 2);
}

#[test]
fn generation_exhaustion_refuses_before_changing_or_notifying() {
    let generation = AtomicU64::new(u64::MAX - 1);
    let notification = tokio::sync::Notify::new();
    let count = Arc::new(Count::default());
    let waker = Waker::from(Arc::clone(&count));
    let mut wait = std::pin::pin!(notification.notified());
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut original = StateViewPublication::new(&generation, &notification);
        let _guard = original.begin();
    }));
    assert!(result.is_err());
    assert_eq!(generation.load(Ordering::Acquire), u64::MAX - 1);
    assert_eq!(count.0.load(Ordering::SeqCst), 0);
    assert!(
        wait.as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
}
