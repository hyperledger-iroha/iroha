//! Actual replay installation retains displaced State through the commit fence.

use super::*;
use crate::state::{
    REPLAY_PUBLICATION_PANIC_AFTER_INSTALL, REPLAY_PUBLICATION_PAUSE_BEFORE_INSTALL,
    replay_blocks_from_kura_range,
};
use mv::allocation::{AllocationBudget, AllocationRefusal};
use std::{
    future::Future,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Wake, Waker},
};

struct CommitReleaseProbe {
    membership: AllocationBudget,
    calls: AtomicUsize,
    reserved_at_unlock: AtomicUsize,
}
impl Wake for CommitReleaseProbe {
    fn wake(self: Arc<Self>) {
        self.reserved_at_unlock
            .store(self.membership.reserved_bytes(), Ordering::SeqCst);
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

struct AfterUnlockProbe {
    commit: Arc<crate::publication_lock::PublicationMutex>,
    calls: AtomicUsize,
    blocked: AtomicBool,
}
impl Wake for AfterUnlockProbe {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.commit.try_lock().is_none() {
            self.blocked.store(true, Ordering::SeqCst);
        }
    }
}

fn original_refund_wait(budget: &AllocationBudget) -> concread::release::ReleaseFuture {
    assert!(
        budget.reserved_bytes() > 0,
        "the original State owns real credits"
    );
    match budget.try_reserve_bytes(budget.limit_bytes()) {
        Err(AllocationRefusal::Capacity { release, .. }) => release.wait_for_release(),
        other => panic!("expected original occupied-pool refusal, got {other:?}"),
    }
}

fn replay_retirement(unwind: bool) {
    let fixture = StrictReplayFixture::new();
    let mut state = fixture.replay_state(Arc::clone(&fixture.kura));
    REPLAY_PUBLICATION_PAUSE_BEFORE_INSTALL.with(|armed| armed.set(true));
    let refusal = replay_blocks_from_kura_range(&fixture.kura, &mut state, 1, 1)
        .expect_err("retain an actually executed and fully prepared replay image");
    assert!(format!("{refusal:#}").contains("injected local replay publication refusal"));
    let prepared = state.pending_replay_publication.as_ref().unwrap();
    assert!(prepared.preparation_complete);
    assert!(prepared.receipt.as_ref().unwrap().geometry.is_empty());
    assert_eq!(state.committed_height(), 0);

    let (hash_budget, membership_budget) = state.history_allocation_budgets();
    let reserved_before = membership_budget.reserved_bytes();
    let commit = Arc::clone(&state.state_commit_lock);
    let commit_probe = Arc::new(CommitReleaseProbe {
        membership: membership_budget.clone(),
        calls: AtomicUsize::new(0),
        reserved_at_unlock: AtomicUsize::new(usize::MAX),
    });
    // Obtain a real release observation from this exact held fence. Retain the
    // setup release separately so only the production acquisition can wake it.
    let setup_guard = commit.lock();
    let commit_wait = match commit.try_lock_or_wait() {
        Err(wait) => wait,
        Ok(_) => panic!("the original commit fence is already held"),
    };
    let setup_release = setup_guard.release_deferred();
    let mut commit_future = Box::pin(commit_wait.wait_for_release());
    let commit_waker = Waker::from(Arc::clone(&commit_probe));
    let mut commit_context = Context::from_waker(&commit_waker);
    assert!(
        commit_future
            .as_mut()
            .poll(&mut commit_context)
            .is_pending()
    );

    let mut futures = [
        state
            .block_hashes
            .map()
            .unwrap()
            .observe_reader_release()
            .wait_for_release(),
        state
            .transactions
            .reader_release_wait_for_tests()
            .wait_for_release(),
        original_refund_wait(&hash_budget),
        original_refund_wait(&membership_budget),
    ]
    .map(Box::pin);
    let probes = std::array::from_fn::<_, 4, _>(|_| {
        Arc::new(AfterUnlockProbe {
            commit: Arc::clone(&commit),
            calls: AtomicUsize::new(0),
            blocked: AtomicBool::new(false),
        })
    });
    let wakers = probes
        .each_ref()
        .map(|probe| Waker::from(Arc::clone(probe)));
    for (future, waker) in futures.iter_mut().zip(&wakers) {
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(waker))
                .is_pending()
        );
    }

    REPLAY_PUBLICATION_PANIC_AFTER_INSTALL.with(|armed| armed.set(unwind));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        replay_blocks_from_kura_range(&fixture.kura, &mut state, 1, 1)
            .expect("resume the original production replay publication");
    }));
    if unwind {
        let panic = result.expect_err("actual post-install hook must unwind");
        assert_eq!(
            panic.downcast_ref::<&str>().copied(),
            Some("injected replay publication unwind after State installation"),
        );
    } else {
        result.expect("actual replay publication succeeds");
    }
    assert!(!REPLAY_PUBLICATION_PANIC_AFTER_INSTALL.with(|armed| armed.replace(false)));
    assert_eq!(commit_probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        commit_probe.reserved_at_unlock.load(Ordering::SeqCst),
        reserved_before,
        "the displaced State must still own its original credits when the actual fence unlocks",
    );
    assert!(
        membership_budget.reserved_bytes() < reserved_before,
        "the displaced State really retired after unlock; retaining it forever cannot pass",
    );
    assert!(commit_future.as_mut().poll(&mut commit_context).is_ready());
    for ((future, waker), probe) in futures.iter_mut().zip(&wakers).zip(&probes) {
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert!(!probe.blocked.load(Ordering::SeqCst));
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(waker))
                .is_ready()
        );
    }
    assert!(Arc::ptr_eq(&commit, &state.state_commit_lock));
    assert_eq!(
        state.committed_height(),
        1,
        "the actual authenticated image was installed"
    );
    assert!(state.pending_replay_publication.is_none());
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state),
        fixture.expected_snapshot,
    );
    drop(setup_release);
}

strict_replay_test!(
    production_replay_retires_original_state_after_commit_unlock,
    {
        replay_retirement(false);
    }
);

strict_replay_test!(
    production_replay_unwind_retires_original_state_after_commit_unlock,
    {
        replay_retirement(true);
    }
);
