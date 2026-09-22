//! Real relay/lifecycle index releases must follow every enclosing fence.

use super::*;
use concread::release::{DeferredRelease, ReleaseFuture};
use std::{
    future::Future,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};

struct Probe {
    state: Arc<State>,
    calls: AtomicUsize,
    blocked: AtomicUsize,
    odd: AtomicUsize,
    unavailable: AtomicUsize,
    cleanup: Mutex<Vec<DeferredRelease>>,
}
impl Probe {
    fn new(state: Arc<State>) -> Arc<Self> {
        Arc::new(Self {
            state,
            calls: AtomicUsize::new(0),
            blocked: AtomicUsize::new(0),
            odd: AtomicUsize::new(0),
            unavailable: AtomicUsize::new(0),
            cleanup: Mutex::new(Vec::new()),
        })
    }
    fn check(&self) {
        assert!(self.calls.load(Ordering::SeqCst) > 0);
        assert_eq!(self.blocked.load(Ordering::SeqCst), 0);
        assert_eq!(self.odd.load(Ordering::SeqCst), 0);
        assert_eq!(self.unavailable.load(Ordering::SeqCst), 0);
    }
}
impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        let Ok(mut cleanup) = self.cleanup.try_lock() else {
            self.unavailable.fetch_add(1, Ordering::SeqCst);
            return;
        };
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.odd.fetch_add(
            usize::from(self.state.state_view_generation() % 2 != 0),
            Ordering::SeqCst,
        );
        macro_rules! fence {
            ($field:ident) => {
                if let Some(guard) = self.state.$field.try_lock() {
                    cleanup.push(guard.release_deferred());
                } else {
                    self.blocked.fetch_add(1, Ordering::SeqCst);
                }
            };
        }
        fence!(state_commit_lock);
        fence!(state_write_lock);
        fence!(lane_lifecycle_lock);
        if self.state.geometry_publication.try_lock().is_none() {
            self.blocked.fetch_add(1, Ordering::SeqCst);
        }
        macro_rules! index {
            ($field:ident) => {
                if let Some(guard) = self.state.$field.try_write() {
                    cleanup.push(guard.release_deferred());
                } else {
                    self.blocked.fetch_add(1, Ordering::SeqCst);
                }
            };
        }
        index!(merge_admission);
        index!(lane_relays);
        index!(lane_manifests);
        index!(lane_privacy_registry);
        index!(da_commitments);
        index!(da_confidential_compute);
        index!(da_receipt_cursors);
        index!(da_shard_cursors);
        index!(da_pin_intents);
        index!(da_indexes_hydrated);
        // All physical probes are nonblocking. Keep their actual cleanup outside Wake.
    }
}

fn watch<T>(index: &PublicationRwLock<T>, probe: &Arc<Probe>) -> (ReleaseFuture, DeferredRelease) {
    let guard = index.read();
    let wait = index
        .try_write_or_wait()
        .err()
        .expect("original held reader");
    let release = guard.release_deferred();
    let mut future = wait.wait_for_release();
    let waker = Waker::from(Arc::clone(probe));
    assert!(
        std::pin::Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    (future, release)
}

fn ready(watches: &mut [(ReleaseFuture, DeferredRelease)], probe: &Arc<Probe>) {
    let waker = Waker::from(Arc::clone(probe));
    assert!(watches.iter_mut().all(|(future, _)| {
        std::pin::Pin::new(future)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    }));
    probe.check();
}

#[test]
fn authenticated_relay_install_releases_cursor_and_relay_after_lifecycle() {
    let _status = crate::sumeragi::status::lane_relay_test_guard();
    let (state, _, validators) = lane_relay_manifest_test_state();
    configure_commit_topology_preserving_world_peers(&state, 1);
    let envelope = sample_lane_relay_envelope_for_state(&state, 1, LaneId::SINGLE, &validators);
    state
        .validate_or_record_lane_relay(&envelope, false)
        .expect("actual QC authentication");
    let state = Arc::new(state);
    let probe = Probe::new(Arc::clone(&state));
    let mut watches = vec![
        watch(&state.da_shard_cursors, &probe),
        watch(&state.lane_relays, &probe),
    ];
    assert_eq!(
        state
            .publish_prevalidated_lane_relay(&envelope, envelope.block_header.height().get())
            .expect("publish authenticated original"),
        LaneRelayInsert::Inserted
    );
    ready(&mut watches, &probe);
    assert_eq!(state.lane_relay_snapshot(), vec![envelope]);
}

#[test]
fn relay_final_incarnation_refusal_releases_original_cursor_after_lifecycle() {
    let (state, _, validators) = lane_relay_manifest_test_state();
    configure_commit_topology_preserving_world_peers(&state, 1);
    let mut envelope = sample_lane_relay_envelope_for_state(&state, 1, LaneId::SINGLE, &validators);
    // Exercise the defining final-stage stale-incarnation refusal, not a forged
    // successful admission. The cache must stay empty.
    envelope.lane_incarnation = Hash::new(b"foreign final-stage incarnation");
    let state = Arc::new(state);
    let probe = Probe::new(Arc::clone(&state));
    let mut watches = vec![watch(&state.da_shard_cursors, &probe)];
    let result =
        state.publish_prevalidated_lane_relay(&envelope, envelope.block_header.height().get());
    assert!(matches!(
        result,
        Err(LaneRelayError::LaneIncarnationMismatch { .. })
    ));
    ready(&mut watches, &probe);
    assert!(state.lane_relay_snapshot().is_empty());
}

#[test]
fn manifest_install_and_unwind_defer_indexes_until_even_generation_and_free_fence() {
    for unwind in [false, true] {
        let state = Arc::new(blank_test_state());
        let manifests = state.lane_manifests.read().clone();
        let before = state.state_view_generation();
        let probe = Probe::new(Arc::clone(&state));
        let mut watches = vec![watch(&state.lane_manifests, &probe)];
        if !unwind {
            watches.push(watch(&state.lane_privacy_registry, &probe));
        }
        if unwind {
            lifecycle_index_publication::panic_after_manifest_write_for_test();
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state.install_lane_manifests(&manifests);
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(state.state_view_generation(), before + 2);
        ready(&mut watches, &probe);
    }
}

#[test]
fn compatible_manifest_refresh_defers_real_read_refusal_and_write_success() {
    for refuse in [false, true] {
        let state = blank_test_state();
        let nexus = state.nexus_snapshot();
        let original =
            Arc::new(LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance));
        // Status-only fixtures deliberately cannot authenticate catalog binding.
        // Load a real immutable source set for the incompatible-policy branch.
        let source_dir = tempfile::tempdir().unwrap();
        let alias = &nexus.lane_catalog.lanes()[0].alias;
        std::fs::write(
            source_dir.path().join(format!("{alias}.manifest.json")),
            norito::json::to_vec(&norito::json!({"lane": alias})).unwrap(),
        )
        .unwrap();
        state.install_lane_manifests(&original);
        let candidate = if refuse {
            Arc::new(LaneManifestRegistry::from_config(
                &nexus.lane_catalog,
                &nexus.governance,
                &iroha_config::parameters::actual::LaneRegistry {
                    manifest_directory: Some(source_dir.path().to_path_buf()),
                    ..Default::default()
                },
            ))
        } else {
            Arc::clone(&original)
        };
        assert!(candidate.is_bound_to_catalog(&nexus.lane_catalog));
        if refuse {
            assert_ne!(
                candidate.consensus_policy_digest(),
                original.consensus_policy_digest()
            );
        }
        let state = Arc::new(state);
        let probe = Probe::new(Arc::clone(&state));
        let mut watches = vec![watch(&state.lane_manifests, &probe)];
        if !refuse {
            watches.push(watch(&state.lane_privacy_registry, &probe));
        }
        assert_eq!(
            state.install_lane_manifests_if_consensus_compatible(&candidate),
            !refuse
        );
        ready(&mut watches, &probe);
        assert_eq!(
            state.lane_manifests.read().consensus_policy_digest(),
            original.consensus_policy_digest()
        );
    }
}

#[test]
fn lifecycle_transition_and_partial_manifest_unwind_defer_original_indexes() {
    let _status = crate::sumeragi::status::lane_relay_test_guard();
    for unwind in [false, true] {
        let state = Arc::new(blank_test_state());
        let probe = Probe::new(Arc::clone(&state));
        let mut watches = vec![
            watch(&state.lane_manifests, &probe),
            watch(&state.da_shard_cursors, &probe),
            watch(&state.da_indexes_hydrated, &probe),
        ];
        if !unwind {
            watches.extend([
                watch(&state.lane_privacy_registry, &probe),
                watch(&state.merge_admission, &probe),
                watch(&state.lane_relays, &probe),
                watch(&state.da_commitments, &probe),
                watch(&state.da_confidential_compute, &probe),
                watch(&state.da_receipt_cursors, &probe),
                watch(&state.da_pin_intents, &probe),
            ]);
        } else {
            lifecycle_index_publication::panic_after_manifest_write_for_test();
        }
        let plan = iroha_data_model::nexus::LaneLifecyclePlan {
            additions: vec![LaneConfig {
                id: LaneId::new(1),
                alias: "custody-lane".to_owned(),
                ..LaneConfig::default()
            }],
            retire: Vec::new(),
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state.apply_lane_lifecycle(&plan)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            result
                .expect("normal return")
                .expect("actual lifecycle transition");
        }
        ready(&mut watches, &probe);
    }
}

// Append to the existing state/tests/lifecycle_index_release_tests module.
// Reuses its original-source watch and nonblocking Probe, without new hooks.
#[test]
fn committed_drain_metadata_defers_manifest_read_until_original_fences_release() {
    for refuse in [false, true] {
        // Open the original Kura at the fixture's authoritative catalog.
        let nexus = autoscale_transition_test_nexus(
            vec![
                LaneConfig {
                    id: LaneId::new(0),
                    alias: "core".to_owned(),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "governance".to_owned(),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(2),
                    alias: "zk".to_owned(),
                    ..LaneConfig::default()
                },
            ],
            3,
            4,
            200,
        );
        let state = State::new_with_nexus_for_testing(
            World::default(),
            nexus,
            LiveQueryStore::start_test(),
        );
        let kura = Arc::clone(&state.kura);
        state
            .apply_lane_lifecycle_with_options(
                &iroha_data_model::nexus::LaneLifecyclePlan {
                    additions: vec![autoscale_elastic_lane_config(
                        LaneId::new(3),
                        DataSpaceId::UNIVERSAL,
                        1,
                    )],
                    retire: Vec::new(),
                },
                false,
                true,
            )
            .expect("seed internally managed public-profile elastic lane");
        seed_governed_autoscale_committee_for_test(&state, 4);
        // The quiet carrier still commits maintenance work.
        state.nexus.write().autoscale.per_lane_target_tps = nonzero!(100_u32);
        seed_consensus_keys_with_pops(&state, &autoscale_drain_keypairs_for_test(4));
        let first = autoscale_signed_block_with_committed_fragments(None, 100, 0);
        let second = autoscale_signed_block_with_committed_fragments(Some(&first), 200, 0);
        store_committed_autoscale_history_block_for_test(&state, &kura, &first);
        store_block_for_state_commit(&kura, &second);
        let state = Arc::new(state);
        let header = second.header();
        let height = header.height().get();
        let header_hash = header.hash();
        let mut state_block = state.block(header);
        insert_empty_transaction_block_for_state_commit(&mut state_block, &second);
        let committed = ValidBlock::new_unverified_for_tests(second)
            .commit_unchecked()
            .unpack(|_| {});
        state_block.maybe_apply_nexus_autoscale(&committed);
        let governance = state.nexus_snapshot().governance;
        let pending = state_block
            .pending_autoscale_lifecycle
            .as_mut()
            .expect("actual cold carrier stages its irreversible drain intent");
        assert!(matches!(
            &pending.transition,
            PendingAutoscaleTransition::DrainIntent { .. }
        ));
        assert_eq!(pending.transition_height, height);
        assert!(pending.plan.additions.is_empty());
        assert!(pending.plan.retire.is_empty());
        if refuse {
            // Keep the actual staged intent/catalog/frontier intact; only replace
            // the staged policy so committed validation must reject its digest.
            let changed_policy = Arc::new(
                LaneManifestRegistry::empty()
                    .rebind(&pending.catalog_update.updated_catalog, &governance),
            );
            assert_ne!(
                changed_policy.consensus_policy_digest(),
                pending.updated_lane_manifests.consensus_policy_digest(),
            );
            pending.updated_lane_manifests = changed_policy;
        }

        // Install the waiter only after staging. The wake below therefore comes
        // from the committed validation path, not from fixture construction.
        let probe = Probe::new(Arc::clone(&state));
        let mut watches = vec![watch(&state.lane_manifests, &probe)];
        let mut releases = LaneLifecycleReleases::new(&state);
        let (result, while_guarded) = {
            let _commit = state.state_commit_lock.lock();
            let _lifecycle = state.lane_lifecycle_lock.lock();
            let _write = state.state_write_lock.lock();
            let result = state.preflight_committed_autoscale_lane_geometry(
                pending,
                height,
                header_hash,
                None,
                &mut releases,
            );
            (result, probe.calls.load(Ordering::SeqCst))
        };
        if refuse {
            assert!(
                matches!(
                    &result,
                    Err(LaneLifecycleError::Storage(message))
                        if message == "installed lane manifest policy changed after drain staging"
                ),
                "staged-policy mismatch must reach its exact refusal: {result:?}"
            );
        } else {
            assert!(
                result.is_ok(),
                "actual staged drain must validate: {result:?}"
            );
        }
        assert_eq!(
            while_guarded, 0,
            "the original manifest reader must not notify under committing fences"
        );
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        // Retire every executing original before delivering retained index notices,
        // matching the actual outer State custody. This validation publishes nothing.
        drop(state_block);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        drop(releases);
        ready(&mut watches, &probe);
    }
}
