//! Original State index contention and joint physical cleanup controls.

use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

fn state() -> Arc<State> {
    Arc::new(State::new_for_testing(
        World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    ))
}

fn contents(state: &State) -> Vec<(&'static str, String)> {
    macro_rules! snapshot {
        ($($field:ident: $ty:ty,)*) => {{
            let mut values = vec![$((stringify!($field), format!("{:?}", &*state.$field.read())),)*];
            values.push(("sccp_registry_cache", format!("{:?}", &*state.sccp_registry_cache.lock())));
            values
        }};
    }
    effect_indexes!(snapshot)
}

/// Observe every original physical index and the outer State writer without
/// assertions, blocking, publishing, or creating replacement lock owners.
struct Probe {
    state: Arc<State>,
    calls: AtomicUsize,
    indexes_free: AtomicUsize,
    fence_free: AtomicUsize,
}

impl Probe {
    fn new(state: &Arc<State>) -> Arc<Self> {
        Arc::new(Self {
            state: Arc::clone(state),
            calls: AtomicUsize::new(0),
            indexes_free: AtomicUsize::new(0),
            fence_free: AtomicUsize::new(0),
        })
    }

    fn inspect(&self) {
        macro_rules! probe {
            ($($field:ident: $ty:ty,)*) => {{
                let mut free = 0;
                $(free += usize::from(self.state.$field.try_write().is_some());)*
                free + usize::from(self.state.sccp_registry_cache.try_lock().is_some())
            }};
        }
        let free = effect_indexes!(probe);
        self.indexes_free.fetch_add(free, Ordering::SeqCst);
        self.fence_free.fetch_add(
            usize::from(self.state.state_write_lock.try_lock().is_some()),
            Ordering::SeqCst,
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
    }

    fn assert_originals_released(&self) {
        assert_eq!(self.calls.load(Ordering::SeqCst), 1);
        assert_eq!(self.indexes_free.load(Ordering::SeqCst), 12);
        assert_eq!(self.fence_free.load(Ordering::SeqCst), 1);
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

fn assert_physical_prefix(slot: &StateEffectLocks<'_>, blocked_index: usize) {
    macro_rules! inspect {
        ($($field:ident: $ty:ty,)*) => {{
            let mut index = 0;
            $(
                assert_eq!(slot.$field.is_some(), index < blocked_index, "retained {}", stringify!($field));
                assert_eq!(slot.target.$field.try_write().is_none(), index <= blocked_index, "physical {}", stringify!($field));
                index += 1;
            )*
            index
        }};
    }
    let sccp_index = effect_indexes!(inspect);
    assert_eq!(sccp_index, 11);
    assert!(slot.sccp_registry_cache.is_none());
    assert_eq!(
        slot.target.sccp_registry_cache.try_lock().is_none(),
        blocked_index == sccp_index
    );
    assert!(!slot.complete);
}

fn refusal<G>(
    state: &Arc<State>,
    blocker: G,
    expected: concread::release::ReleaseWait,
    field: &'static str,
    blocked_index: usize,
    before: &[(&'static str, String)],
) {
    // Retained cleanup precedes the outer fence, as in the real publisher.
    let mut cleanup = StateEffectLocks::new(state);
    let fence = state.state_write_lock.lock();
    let generation = state.state_view_generation();
    let (actual_field, wait) = cleanup.try_prepare().expect_err("actual original blocker");
    assert_eq!(actual_field, field);
    assert_eq!(
        wait, expected,
        "refusal must return the exact blocking source and observation"
    );
    assert_physical_prefix(&cleanup, blocked_index);
    let mut blocker_wait = wait.wait_for_release();
    assert!(
        Pin::new(&mut blocker_wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    let probe = Probe::new(state);
    let waker = Waker::from(Arc::clone(&probe));
    let mut context = Context::from_waker(&waker);
    let mut prefix_wait = if blocked_index > 0 {
        let mut pending = state
            .latest_block_header
            .try_write_or_wait()
            .err()
            .expect("actual first original writer retained")
            .wait_for_release();
        assert!(Pin::new(&mut pending).poll(&mut context).is_pending());
        Some(pending)
    } else {
        None
    };
    cleanup.release_writers();
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    if let Some(pending) = &mut prefix_wait {
        assert!(Pin::new(pending).poll(&mut context).is_pending());
    }
    assert!(
        Pin::new(&mut blocker_wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending(),
        "earlier acquired releases cannot substitute for the actual blocker"
    );
    drop(blocker);
    assert_eq!(
        Pin::new(&mut blocker_wait).poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    );
    drop(fence);
    drop(cleanup);
    if let Some(pending) = &mut prefix_wait {
        probe.assert_originals_released();
        assert_eq!(Pin::new(pending).poll(&mut context), Poll::Ready(()));
    } else {
        assert_eq!(
            probe.calls.load(Ordering::SeqCst),
            0,
            "unacquired prefix emits no release"
        );
    }
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        contents(state).as_slice(),
        before,
        "refusal cannot mutate any original index"
    );
}

#[test]
fn every_effect_reader_and_writer_refusal_retains_original_prefix_and_wait() {
    let state = state();
    let before = contents(&state);
    macro_rules! check {
        ($($field:ident: $ty:ty,)*) => {{
            let mut index = 0;
            $(
                let held = state.$field.read();
                let expected = state.$field.try_write_or_wait().err().expect("actual original reader");
                refusal(&state, held, expected, stringify!($field), index, &before);
                let held = state.$field.write();
                let expected = state.$field.try_write_or_wait().err().expect("actual original writer");
                refusal(&state, held, expected, stringify!($field), index, &before);
                index += 1;
            )*
            index
        }};
    }
    assert_eq!(effect_indexes!(check), 11);
}

#[test]
fn sccp_refusal_retains_all_original_effect_writers() {
    let state = state();
    let before = contents(&state);
    let held = state.sccp_registry_cache.lock();
    let expected = state
        .sccp_registry_cache
        .try_lock_or_wait()
        .err()
        .expect("actual original SCCP writer");
    refusal(&state, held, expected, "sccp_registry_cache", 11, &before);
}

fn complete_scope(unwind: bool) {
    let state = state();
    let before = contents(&state);
    let generation = state.state_view_generation();
    let probe = Probe::new(&state);
    let waker = Waker::from(Arc::clone(&probe));
    let mut context = Context::from_waker(&waker);
    let mut cleanup = StateEffectLocks::new(&state);
    let fence = state.state_write_lock.lock();
    let merge_probe = Probe::new(&state);
    let merge_waker = Waker::from(Arc::clone(&merge_probe));
    let mut merge_context = Context::from_waker(&merge_waker);
    let mut merge_pending = None;
    let mut merge_observation = None;
    let merge_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cleanup.with_merge_admission(|_| {
            let wait = state
                .merge_admission
                .try_write_or_wait()
                .expect_err("actual short original reader");
            merge_observation = Some(wait.clone());
            let mut pending = Box::pin(wait.wait_for_release());
            assert!(pending.as_mut().poll(&mut merge_context).is_pending());
            merge_pending = Some(pending);
            if unwind {
                panic!("actual short validation read unwind");
            }
        });
    }));
    assert_eq!(merge_result.is_err(), unwind);
    assert!(!merge_observation.unwrap().is_poisoned());
    assert!(
        merge_pending
            .as_mut()
            .unwrap()
            .as_mut()
            .poll(&mut merge_context)
            .is_pending()
    );
    assert_eq!(merge_probe.calls.load(Ordering::SeqCst), 0);
    cleanup
        .try_prepare()
        .expect("all original physical indexes available");
    assert!(cleanup.complete);
    macro_rules! held {
        ($($field:ident: $ty:ty,)*) => { $(assert!(state.$field.try_write().is_none(), "{} held", stringify!($field));)* };
    }
    effect_indexes!(held);
    assert!(state.sccp_registry_cache.try_lock().is_none());
    let mut pending = state
        .latest_block_header
        .try_write_or_wait()
        .err()
        .expect("original index writer")
        .wait_for_release();
    assert!(Pin::new(&mut pending).poll(&mut context).is_pending());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let scope = cleanup.physical_scope();
        assert!(scope.complete);
        if unwind {
            panic!("actual enclosing preparation unwind");
        }
        drop(scope);
    }));
    assert_eq!(result.is_err(), unwind);
    assert!(!cleanup.complete);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    assert!(Pin::new(&mut pending).poll(&mut context).is_pending());
    drop(fence);
    drop(cleanup);
    probe.assert_originals_released();
    merge_probe.assert_originals_released();
    assert_eq!(
        merge_pending
            .as_mut()
            .unwrap()
            .as_mut()
            .poll(&mut merge_context),
        Poll::Ready(())
    );
    assert_eq!(Pin::new(&mut pending).poll(&mut context), Poll::Ready(()));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(contents(&state), before);
}

#[test]
fn complete_effect_scope_releases_all_indexes_before_outer_fence_callbacks() {
    complete_scope(false);
}

#[test]
fn unwinding_effect_scope_releases_all_indexes_before_outer_fence_callbacks() {
    complete_scope(true);
}

#[test]
fn synchronous_effect_preparation_releases_prefix_while_waiting_for_original_sccp() {
    use std::{sync::mpsc, time::Duration};

    let state = state();
    let before = contents(&state);
    let blocker = state.sccp_registry_cache.lock();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let worker_state = Arc::clone(&state);
    let worker = std::thread::spawn(move || {
        let mut cleanup = StateEffectLocks::new(&worker_state);
        let _ = started_tx.send(());
        cleanup.prepare_blocking();
        let complete = cleanup.complete;
        drop(cleanup);
        let _ = done_tx.send(complete);
    });
    // Record every observation before assertions. Regardless of a channel
    // timeout, release the only external blocker and join the original worker.
    let started = started_rx.recv_timeout(Duration::from_secs(5));
    let early = done_rx.recv_timeout(Duration::from_millis(50));
    macro_rules! free_prefix {
        ($($field:ident: $ty:ty,)*) => { [$(state.$field.try_write().is_some(),)*] };
    }
    let prefix = effect_indexes!(free_prefix);
    let sccp_busy = state.sccp_registry_cache.try_lock().is_none();
    drop(blocker);
    let completed = if early.is_ok() {
        early.clone()
    } else {
        done_rx.recv_timeout(Duration::from_secs(5))
    };
    let joined = worker.join();
    assert!(started.is_ok());
    assert_eq!(early, Err(mpsc::RecvTimeoutError::Timeout));
    assert!(prefix.into_iter().all(|free| free));
    assert!(sccp_busy);
    assert_eq!(completed, Ok(true));
    assert!(joined.is_ok());
    assert_eq!(contents(&state), before);
}
