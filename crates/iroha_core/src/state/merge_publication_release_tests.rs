//! Merge-cache retries must observe the completed State publication boundary.

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
    blocked: AtomicBool,
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
        let mut indexes = effect_publication::StateEffectLocks::new(&self.state);
        let indexes_free = indexes.try_prepare().is_ok();
        drop(indexes);
        let state_free = self.state.state_write_lock.try_lock().is_some();
        let commit_free = self.state.state_commit_lock.try_lock().is_some();
        let lifecycle_free = self.state.lane_lifecycle_lock.try_lock().is_some();
        if !indexes_free
            || !state_free
            || !commit_free
            || !lifecycle_free
            || self.state.state_view_generation() % 2 != 0
        {
            self.blocked.store(true, Ordering::SeqCst);
        }
        self.running.store(false, Ordering::SeqCst);
    }
}

#[test]
fn drain_observation_and_pending_admission_view_retain_all_index_releases() {
    for has_pending in [false, true] {
        let (state, validators, _, _) = configured_two_lane_merge_state();
        let lane = LaneId::new(1);
        let incarnation = state.lane_incarnation(lane).expect("active participant");
        if has_pending {
            let plan = crate::queue::RoutingPlan::native_amx(
                crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                vec![crate::queue::RouteLeg::new(
                    crate::queue::RoutingDecision::new(lane, DataSpaceId::UNIVERSAL),
                    crate::queue::RouteLegRole::Participant,
                )],
            );
            let (_, input) = queue_plan_admission_certificate_for_state_test(
                &state,
                plan,
                &validators,
                queue_plan_authority_height_for_state_test(&state),
                0x61,
            );
            state
                .kura
                .persist_pending_queue_plan_admission_certificate(&input)
                .expect("persist complete authenticated admission input");
        }
        let state = Arc::new(state);
        let probe = Arc::new(Probe {
            state: Arc::clone(&state),
            running: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
            blocked: AtomicBool::new(false),
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
        watch!(da_shard_cursors);
        watch!(merge_admission);
        watch!(lane_relays);
        if has_pending {
            watch!(latest_block_header);
            watch!(lane_manifests);
            let held = state.sccp_registry_cache.lock();
            let wait = state
                .sccp_registry_cache
                .try_lock_or_wait()
                .err()
                .expect("actual held cache");
            original_releases.push(held.release_deferred());
            let mut future = Box::pin(wait.wait_for_release());
            assert!(future.as_mut().poll(&mut context).is_pending());
            pending.push(future);
        }
        let result = {
            let mut releases = LaneLifecycleReleases::new(&state);
            let _commit = state.state_commit_lock.lock();
            let _lifecycle = state.lane_lifecycle_lock.lock();
            let _write = state.state_write_lock.lock();
            state.lane_has_drain_blocking_evidence_with_releases(
                lane,
                DataSpaceId::UNIVERSAL,
                incarnation,
                &mut releases,
            )
        };
        assert_eq!(
            result.expect("authenticate original drain evidence"),
            has_pending
        );
        assert!(
            pending
                .iter_mut()
                .all(|future| future.as_mut().poll(&mut context).is_ready())
        );
        assert!(probe.calls.load(Ordering::SeqCst) > 0);
        assert!(!probe.blocked.load(Ordering::SeqCst));
        drop(original_releases);
    }
}

#[test]
fn certified_lane_reads_and_persistence_refusal_defer_cursor_retry_until_state_unlock() {
    let state = Arc::new(State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let (session, pops) = sample_committed_lane_block_session_for_state_test(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        Hash::new(b"inactive certificate incarnation"),
        1,
        1,
    );
    for case in 0..3 {
        let probe = Arc::new(Probe {
            state: Arc::clone(&state),
            running: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
            blocked: AtomicBool::new(false),
        });
        let held = state.da_shard_cursors.read();
        let wait = state
            .da_shard_cursors
            .try_write_or_wait()
            .err()
            .expect("actual cursor reader blocks publication");
        let original_release = held.release_deferred();
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut pending = Box::pin(wait.wait_for_release());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        match case {
            0 => assert!(
                state
                    .latest_certified_lane_block_frontier_sessions_snapshot_cached()
                    .is_empty()
            ),
            1 => {
                let repair = state
                    .lane_application_certified_repair_snapshot_cached(8)
                    .expect("empty authoritative certified frontier");
                assert!(repair.earliest_unapplied.is_empty());
                assert!(repair.pair_repairs.is_empty());
            }
            _ => {
                let error = state
                    .persist_committed_lane_block_session_lifecycle_bound(&session, &pops)
                    .expect_err("inactive incarnation must not receive persistence authority");
                assert!(
                    error.contains("outside the committed lane lifecycle"),
                    "{error}"
                );
            }
        }
        assert!(pending.as_mut().poll(&mut context).is_ready());
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1, "case {case}");
        assert!(!probe.blocked.load(Ordering::SeqCst), "case {case}");
        drop(original_release);
    }
}

#[test]
fn merge_cache_success_and_refusal_notify_only_after_state_unlock() {
    // Exercise both idempotent branches, fresh cache repair, and refusal after
    // acquiring the actual admission writer, and exhausted-generation unwind.
    // Every carrier is durable and has
    // already been published by the canonical State commit fixture.
    for case in 0..5 {
        let (state, validators, committers, parent) = configured_single_lane_merge_state();
        let entry = next_relay_merge_entry(&state, 1, &validators, &committers);
        store_and_commit_exact_merge_carrier(&state, &parent, &entry);
        if case == 1 || case == 2 {
            state.merge_ledger.entries.write().clear();
        }
        if case == 2 {
            *state.merge_admission.write() = MergeAdmissionState::default();
        } else if case == 3 {
            let mut conflicting = entry.clone();
            conflicting.activation_root = Hash::new(b"conflicting retained merge cache");
            state.merge_admission.write().record(&conflicting);
        } else if case == 4 {
            state.view_generation.store(u64::MAX - 1, Ordering::Release);
        }
        let state = Arc::new(state);
        let probe = Arc::new(Probe {
            state: Arc::clone(&state),
            running: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
            blocked: AtomicBool::new(false),
        });
        let held = state.merge_admission.read();
        let wait = state
            .merge_admission
            .try_write_or_wait()
            .err()
            .expect("actual admission reader blocks publication");
        let original_release = held.release_deferred();
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut pending = Box::pin(wait.wait_for_release());
        assert!(pending.as_mut().poll(&mut context).is_pending());
        let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state.record_globally_committed_merge_entry(
                &entry,
                MergeLedgerPublicationMode::LiveCommit,
            )
        }));
        if case == 4 {
            assert!(
                attempted.is_err(),
                "exhaustion refuses before opening visibility"
            );
            assert_eq!(state.view_generation.load(Ordering::Acquire), u64::MAX - 1);
            assert_eq!(state.merge_ledger.latest().as_deref(), Some(&entry));
        } else if case == 3 {
            let result = attempted.expect("conflict returns a typed refusal");
            assert!(matches!(
                result,
                Err(MergeLedgerCommitError::Persistence(
                    crate::kura::Error::MergeCarrierConflict(_)
                ))
            ));
        } else {
            let (stored, event) = attempted
                .expect("ordinary cache publication does not unwind")
                .expect("repair exact committed carrier cache");
            assert_eq!(stored.as_ref(), &entry);
            assert_eq!(event.is_some(), case != 0);
            assert_eq!(state.merge_ledger.snapshot().len(), 1);
        }
        assert!(pending.as_mut().poll(&mut context).is_ready());
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1, "case {case}");
        assert!(!probe.blocked.load(Ordering::SeqCst), "case {case}");
        drop(original_release);
    }
}
