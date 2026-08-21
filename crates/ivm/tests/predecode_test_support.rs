//! Shared-state isolation for grouped predecoder tests.

use std::sync::{Mutex, MutexGuard, OnceLock};

/// Hold the exclusive test lease while observing or overriding predecode globals.
pub(crate) fn exclusive() -> MutexGuard<'static, ()> {
    static LEASE: OnceLock<Mutex<()>> = OnceLock::new();
    LEASE
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn exclusive_lease_blocks_overlapping_global_limit_overrides() {
    use std::{sync::mpsc, thread, time::Duration};

    let (first_ready_tx, first_ready_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();
    let first = thread::spawn(move || {
        let _lease = exclusive();
        let baseline = ivm::ivm_cache::cache_limits();
        let _limits = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
            max_decoded_ops: 1,
            ..baseline
        });
        first_ready_tx
            .send(baseline)
            .expect("signal first override");
        release_first_rx.recv().expect("release first override");
    });
    let baseline = first_ready_rx.recv().expect("first override ready");

    let (second_attempt_tx, second_attempt_rx) = mpsc::channel();
    let (second_acquired_tx, second_acquired_rx) = mpsc::channel();
    let second = thread::spawn(move || {
        second_attempt_tx.send(()).expect("signal second attempt");
        let _lease = exclusive();
        second_acquired_tx
            .send(ivm::ivm_cache::cache_limits())
            .expect("report second limits");
    });
    second_attempt_rx.recv().expect("second override attempted");
    assert!(
        matches!(
            second_acquired_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "second predecode observer entered while an override was active"
    );

    release_first_tx.send(()).expect("release first override");
    first.join().expect("first override thread");
    assert_eq!(
        second_acquired_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("second observer acquires after release"),
        baseline
    );
    second.join().expect("second observer thread");
}
