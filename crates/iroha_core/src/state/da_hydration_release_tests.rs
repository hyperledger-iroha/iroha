//! Hydration retries may reenter only after the entire rebuild has unlocked.

use super::*;
use std::{
    future::Future,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Wake, Waker},
};

struct Probe {
    state: Arc<State>,
    running: AtomicBool,
    calls: AtomicUsize,
    blocked: AtomicUsize,
}

impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        let fences_free = self.state.da_index_hydration_fence.try_lock().is_some()
            && self.state.state_write_lock.try_lock().is_some();
        let mut indexes = effect_publication::StateEffectLocks::new(&self.state);
        let indexes_free = indexes.try_prepare().is_ok();
        drop(indexes);
        if !fences_free || !indexes_free {
            self.blocked.fetch_add(1, Ordering::SeqCst);
        }
        self.running.store(false, Ordering::SeqCst);
    }
}

fn state() -> Arc<State> {
    Arc::new(State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    ))
}

#[test]
fn hydration_and_rewind_release_every_index_and_fence_before_retry_callbacks() {
    for rewind in [false, true] {
        let state = state();
        let probe = Arc::new(Probe {
            state: Arc::clone(&state),
            running: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
            blocked: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut pending = Vec::new();
        let mut original_releases = Vec::new();
        macro_rules! watch {
            ($field:ident) => {{
                let held = state.$field.read();
                let wait = state
                    .$field
                    .try_write_or_wait()
                    .err()
                    .expect("actual held reader");
                original_releases.push(held.release_deferred());
                let mut future = Box::pin(wait.wait_for_release());
                assert!(future.as_mut().poll(&mut context).is_pending());
                pending.push(future);
            }};
        }
        watch!(da_commitments);
        watch!(da_confidential_compute);
        watch!(da_receipt_cursors);
        watch!(da_shard_cursors);
        watch!(da_pin_intents);
        // Ensure's initial cache probe precedes every enclosing fence and may
        // notify immediately. Rewind instead clears this marker under its fence.
        if rewind {
            watch!(da_indexes_hydrated);
        }
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        let result = if rewind {
            state.rewind_da_indexes_to_height(0)
        } else {
            state.ensure_da_indexes_hydrated()
        };
        assert_eq!(result, Ok(()));
        assert!(probe.calls.load(Ordering::SeqCst) > 0);
        assert_eq!(probe.blocked.load(Ordering::SeqCst), 0);
        assert!(
            pending
                .iter_mut()
                .all(|f| f.as_mut().poll(&mut context).is_ready())
        );
        assert_eq!(*state.da_indexes_hydrated.read(), Some(Ok(())));
        let journal = state.da_shard_cursor_journal_path();
        assert!(
            journal.is_file(),
            "rebuild persists the captured cursor journal"
        );
        DaShardCursorJournal::load(&state.nexus_snapshot().lane_config, &journal)
            .expect("published hydration journal");
        drop(original_releases);
    }
}

#[test]
fn failed_rewind_releases_its_status_and_state_fences_before_retry() {
    let state = state();
    let mut hashes = state.block_hashes.block();
    hashes.push_for_tests(HashOf::from_untyped_unchecked(Hash::new(
        b"missing DA body",
    )));
    hashes.commit_for_tests();
    let probe = Arc::new(Probe {
        state: Arc::clone(&state),
        running: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
        blocked: AtomicUsize::new(0),
    });
    let held = state.da_indexes_hydrated.read();
    let wait = state
        .da_indexes_hydrated
        .try_write_or_wait()
        .err()
        .expect("held original marker");
    let original_release = held.release_deferred();
    let waker = Waker::from(Arc::clone(&probe));
    let mut context = Context::from_waker(&waker);
    let mut pending = Box::pin(wait.wait_for_release());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    let result = state.rewind_da_indexes_to_height(1);
    assert!(matches!(
        result,
        Err(DaIndexHydrationError::MissingBlock { .. })
    ));
    assert_eq!(*state.da_indexes_hydrated.read(), Some(result));
    assert!(pending.as_mut().poll(&mut context).is_ready());
    assert!(probe.calls.load(Ordering::SeqCst) > 0);
    assert_eq!(probe.blocked.load(Ordering::SeqCst), 0);
    drop(original_release);
}
