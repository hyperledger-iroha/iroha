//! Runtime owner mechanics through real State scopes; no carrier publication claim.

use super::*;
use crate::query::store::LiveQueryStore;

fn state() -> State {
    State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn header() -> BlockHeader {
    BlockHeader::new(std::num::NonZeroU64::MIN, None, None, 1, 0)
}

#[test]
fn abandoned_runtime_children_and_blocks_do_not_publish() {
    let state = state();
    let original = state.canonical_runtime.view().get().clone();
    {
        let mut block = state.merge_preexecution_block(header());
        {
            let mut transaction = block.transaction();
            // Low-level owner mutation tests rollback, not lifecycle authorization.
            transaction
                .canonical_runtime
                .get_mut()
                .autoscale_last_transition_height = 7;
        }
        assert_eq!(block.canonical_runtime.get(), &original);
        block
            .canonical_runtime
            .get_mut()
            .autoscale_last_transition_height = 9;
    }
    assert_eq!(state.canonical_runtime.view().get(), &original);
    assert!(state.canonical_runtime.predecessor_view().is_none());
}

#[test]
fn actual_replacement_uses_retained_runtime_including_retired_lineage() {
    let state = state();
    let predecessor = state.canonical_runtime.view().get().clone();
    // Explicit retained-metadata fixture: no SignedBlock/finality/publication is fabricated.
    let mut retained = state.canonical_runtime.block();
    retained
        .get_mut()
        .lane_incarnation_lineage
        .push(SnapshotLaneIncarnationLineage {
            lane_id: LaneId::new(7),
            generation: 1,
            incarnation: Hash::new(b"retired runtime lineage"),
            activation_height: 0,
        });
    retained.commit();
    let tip = state.canonical_runtime.view().get().clone();
    assert_ne!(tip, predecessor);
    {
        let ordinary = state.merge_preexecution_block(header());
        assert_eq!(ordinary.canonical_runtime.get(), &tip);
        assert!(
            ordinary
                .lane_incarnation_lineage
                .contains_key(&LaneId::new(7))
        );
    }
    {
        let replacement = state.block_and_revert(header());
        assert_eq!(replacement.canonical_runtime.get(), &predecessor);
        assert!(
            !replacement
                .lane_incarnation_lineage
                .contains_key(&LaneId::new(7))
        );
    }
    assert_eq!(state.canonical_runtime.view().get(), &tip);
    assert_eq!(
        state.canonical_runtime.predecessor_view().get(),
        &Some(predecessor)
    );
}

#[test]
fn runtime_mv_json_retains_current_undo_and_rejects_retired_record_only_shape() {
    let state = state();
    let predecessor = state.canonical_runtime.view().get().clone();
    let mut retained = state.canonical_runtime.block();
    retained.get_mut().autoscale_last_transition_height = 3;
    retained.commit();
    let json = norito::json::to_json(&state.canonical_runtime).unwrap();
    let decoded: Cell<SnapshotNexusRuntime> = norito::json::from_str(&json).unwrap();
    assert_eq!(decoded.view().get(), state.canonical_runtime.view().get());
    assert_eq!(decoded.predecessor_view().get(), &Some(predecessor.clone()));
    assert_eq!(decoded.block_and_revert().get(), &predecessor);
    let retired = norito::json::to_json(state.canonical_runtime.view().get()).unwrap();
    assert!(norito::json::from_str::<Cell<SnapshotNexusRuntime>>(&retired).is_err());
}

#[test]
fn unowned_runtime_projection_change_cannot_publish() {
    let state = state();
    let original = state.canonical_runtime.view().get().clone();
    let mut block = state.merge_preexecution_block(header());
    block.nexus.autoscale.last_transition_height = 1;
    assert!(block.validate_canonical_runtime_projection().is_err());
    assert!(matches!(
        block.commit(),
        Err(TransactionsBlockError::AutoscaleLaneLifecycle)
    ));
    assert_eq!(state.canonical_runtime.view().get(), &original);
}

#[test]
fn incomplete_runtime_lineage_is_a_fallible_error_and_cannot_replace_owner() {
    let state = state();
    let original = state.canonical_runtime.view().get().clone();
    let mut malformed = original.clone();
    malformed.lane_incarnation_lineage.clear();
    assert!(malformed.active_incarnations().is_err());
    assert!(malformed.active_activation_heights().is_err());
    assert!(
        state
            .project_canonical_runtime(&malformed, &state.world.view())
            .is_err()
    );
    assert!(
        state
            .install_canonical_runtime_projection(
                &state.nexus_snapshot(),
                &BTreeMap::new(),
                &VecDeque::new(),
            )
            .is_err()
    );
    assert_eq!(state.canonical_runtime.view().get(), &original);
    assert!(state.canonical_runtime.predecessor_view().is_none());
}

#[test]
fn retained_runtime_policy_is_not_reinterpreted_by_a_later_configured_baseline() {
    let state = state();
    let original = state.canonical_runtime.view().get().clone();
    let mut different_baseline = state.nexus.read().clone();
    different_baseline.autoscale.enabled = !original.owner_policy.autoscale_enabled;
    different_baseline.staking.max_validators =
        std::num::NonZeroU32::new(original.owner_policy.max_validators + 1).unwrap();
    different_baseline.autoscale.scale_out_window_blocks = std::num::NonZeroU16::MAX;
    let projected = original.nexus_projection(&different_baseline).unwrap();
    assert_eq!(
        SnapshotNexusOwnerPolicy::from_nexus(&projected),
        original.owner_policy
    );
    assert_eq!(
        projected.autoscale.scale_out_window_blocks.get(),
        original.autoscale_scale_out_window_blocks
    );
}

#[test]
fn consensus_only_apply_refuses_a_staged_runtime_transition() {
    let state = state();
    let original = state.canonical_runtime.view().get().clone();
    let mut block = state.merge_preexecution_block(header());
    {
        let mut transaction = block.transaction();
        transaction
            .canonical_runtime
            .get_mut()
            .autoscale_last_transition_height = 1;
        transaction.apply_consensus_effects();
    }
    assert_eq!(block.canonical_runtime.get(), &original);
    assert!(matches!(
        block.commit(),
        Err(TransactionsBlockError::ExecutionOutputCapacity)
    ));
    assert_eq!(state.canonical_runtime.view().get(), &original);
}

fn snapshot_capture_test(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .expect("spawn bounded-stack State fixture")
        .join()
        .expect("snapshot capture fixture must pass");
}

#[test]
fn snapshot_capture_refuses_active_publisher_without_waiting() {
    snapshot_capture_test(|| {
        let state = state();
        let original_nexus = state.nexus_snapshot();
        let publication = state.begin_state_view_write();
        assert!(state.try_nexus_snapshot_once().unwrap().is_none());
        assert_eq!(
            state.nexus_ownership_projection().lane_catalog,
            original_nexus.lane_catalog
        );
        // Cursor persistence also runs inside lane-geometry publication. Its
        // ownership-only input must not wait on the generation held here.
        state.persist_da_shard_cursor_journal();
        assert!(state.try_view_once().unwrap().is_none());
        let error = match crate::snapshot::CapturedStateSnapshot::capture(&state) {
            Ok(_) => panic!("odd publication generation cannot yield a snapshot"),
            Err(error) => error,
        };
        assert!(matches!(error, crate::snapshot::SnapshotCaptureError::Busy));
        assert!(matches!(
            MergeLedgerCommitError::from(error),
            MergeLedgerCommitError::ExecutionObservationChanged
        ));
        drop(publication);
        assert_eq!(
            state
                .try_nexus_snapshot_once()
                .unwrap()
                .expect("completed publication must admit a Nexus snapshot")
                .dataspace_catalog,
            original_nexus.dataspace_catalog
        );
        // Manifest refresh owns another publication guard while checking its
        // catalog binding, including rejection of an unbound registry.
        assert!(!state.install_lane_manifests_if_consensus_compatible(&Arc::new(
            LaneManifestRegistry::default()
        )));
        assert!(crate::snapshot::CapturedStateSnapshot::capture(&state).is_ok());
    });
}

#[test]
fn snapshot_capture_discards_bytes_after_completed_publication() {
    snapshot_capture_test(|| {
        let state = state();
        let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
        let error = match crate::snapshot::CapturedStateSnapshot::capture_with_publication_for_test(
            &state,
            || {
                // Even a semantically empty real publication invalidates the observation.
                let publication = state.begin_state_view_write();
                drop(publication);
            },
        ) {
            Ok(_) => panic!("changed generation cannot yield the partial capture"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            crate::snapshot::SnapshotCaptureError::Changed
        ));
        assert!(matches!(
            TransactionsBlockError::from(error),
            TransactionsBlockError::SnapshotObservationChanged
        ));
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
    });
}

#[test]
fn snapshot_capture_stable_malformed_runtime_is_fallible_and_read_only() {
    snapshot_capture_test(|| {
        let state = state();
        {
            let publication = state.begin_state_view_write();
            let mut runtime = state.canonical_runtime.block();
            runtime.get_mut().lane_incarnation_lineage.clear();
            runtime.commit();
            drop(publication);
        }
        let before = norito::json::to_json(&state.canonical_runtime).unwrap();
        let world_before = norito::json::to_json(&state.world).unwrap();
        let error = match crate::snapshot::CapturedStateSnapshot::capture(&state) {
            Ok(_) => panic!("incomplete lineage must fail the actual projection"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            crate::snapshot::SnapshotCaptureError::Runtime(_)
        ));
        assert!(!error.is_observation_changed());
        assert!(matches!(
            TransactionsBlockError::from(error),
            TransactionsBlockError::SnapshotProjection
        ));
        assert_eq!(
            norito::json::to_json(&state.canonical_runtime).unwrap(),
            before
        );
        assert_eq!(norito::json::to_json(&state.world).unwrap(), world_before);
    });
}

#[test]
fn snapshot_capture_retains_exact_topology_bytes_after_later_publication() {
    snapshot_capture_test(|| {
        let state = state();
        let captured = crate::snapshot::CapturedStateSnapshot::capture(&state).unwrap();
        let original = captured.as_json().to_owned();
        let hash = captured.canonical_hash().unwrap();
        {
            // Explicit topology-owner fixture, not authenticated carrier application.
            let publication = state.begin_state_view_write();
            let mut topology = state.commit_topology.block();
            topology.get_mut().push(iroha_model_base::peer::PeerId::new(
                iroha_test_samples::ALICE_KEYPAIR.public_key().clone(),
            ));
            topology.commit();
            drop(publication);
        }
        let current = crate::snapshot::CapturedStateSnapshot::capture(&state).unwrap();
        assert_eq!(captured.as_json(), original);
        assert_ne!(current.as_json(), original);
        assert_eq!(captured.canonical_hash().unwrap(), hash);
        // Consensus topology is deliberately redacted only from the existing WSV hash,
        // while the restart payload must retain both exact topology Cell cuts.
        assert_eq!(current.canonical_hash().unwrap(), hash);
        let encoded: norito::json::Value = norito::json::from_str(current.as_json()).unwrap();
        assert_eq!(
            encoded["commit_topology"]["revert"],
            norito::json::Value::Array(Vec::new())
        );
        assert_eq!(
            encoded["commit_topology"]["blocks"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    });
}
