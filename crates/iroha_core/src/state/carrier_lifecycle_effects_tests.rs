//! Original lifecycle projections and post-generation cursor behavior.

use super::*;
use crate::{governance::manifest::LaneManifestStatus, query::store::LiveQueryStore};
use iroha_crypto::privacy::{LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment};

fn fixture() -> (
    Box<State>,
    PendingAutoscaleLaneLifecycle,
    iroha_config::parameters::actual::Nexus,
) {
    let state = Box::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let block = state.merge_preexecution_block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
    let plan = iroha_data_model::nexus::LaneLifecyclePlan {
        additions: vec![iroha_data_model::nexus::LaneConfig {
            id: LaneId::new(1),
            alias: "retained-lifecycle-effects".to_owned(),
            ..iroha_data_model::nexus::LaneConfig::default()
        }],
        retire: Vec::new(),
    };
    let mut update = prepare_lane_lifecycle_update(
        &block.nexus,
        &block.lane_incarnations,
        &block.lane_incarnation_lineage,
        &block.lane_incarnation_activation_heights,
        &block.network_id,
        block._curr_block.hash(),
        &plan,
        1,
        false,
    )
    .unwrap();
    // These tests qualify the projection, not lifecycle admission: an inactive
    // reset target must never receive a new canonical watermark.
    update.lanes_to_reset.insert(LaneId::new(9));
    let lane = update
        .updated_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == LaneId::new(1))
        .unwrap();
    let manifests = Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::from([(
        lane.id,
        LaneManifestStatus {
            lane: lane.id,
            alias: lane.alias.clone(),
            dataspace: lane.dataspace_id,
            visibility: lane.visibility,
            storage: lane.storage,
            governance: lane.governance.clone(),
            manifest_path: None,
            governance_rules: None,
            privacy_commitments: vec![LanePrivacyCommitment::merkle(
                LaneCommitmentId::new(1),
                MerkleCommitment::from_root_bytes([0xA5; 32], 12),
            )],
        },
    )])));
    let mut nexus = block.nexus.clone();
    nexus.lane_catalog = update.updated_catalog.clone();
    nexus.lane_config = update.updated_lane_config.clone();
    let pending = PendingAutoscaleLaneLifecycle {
        expected_incarnation_root: lane_lifecycle_incarnation_root(
            &block.nexus.lane_catalog,
            &block.lane_incarnations,
        )
        .unwrap(),
        catalog_update: update,
        updated_lane_manifests: manifests,
        plan,
        transition: PendingAutoscaleTransition::Manual,
        transition_height: 1,
        runtime_catalog: None,
    };
    drop(block);
    (state, pending, nexus)
}

#[test]
fn lifecycle_capture_is_static_and_drop_preserves_original_runtime() {
    fn assert_static_send<T: Send + 'static>() {}
    assert_static_send::<PreparedLaneLifecycleEffects>();
    assert_static_send::<LaneLifecyclePostPublication>();
    let (state, pending, nexus) = fixture();
    let manifests = Arc::clone(&state.lane_manifests.read());
    let privacy = Arc::clone(&state.lane_privacy_registry.read());
    let generation = state.state_view_generation();
    let prepared = PreparedLaneLifecycleEffects::prepare(&pending, &nexus);
    assert!(Arc::ptr_eq(
        &prepared.manifests,
        &pending.updated_lane_manifests
    ));
    assert!(prepared.privacy.lane(LaneId::new(1)).is_some());
    assert_eq!(
        prepared.active_reset_lanes,
        BTreeSet::from([LaneId::new(1)])
    );
    let captured = Arc::downgrade(&prepared.privacy);
    drop(pending);
    drop(nexus);
    let prepared = std::thread::spawn(move || prepared).join().unwrap();
    assert!(captured.upgrade().is_some());
    drop(prepared);
    assert!(captured.upgrade().is_none());
    assert!(Arc::ptr_eq(&state.lane_manifests.read(), &manifests));
    assert!(Arc::ptr_eq(&state.lane_privacy_registry.read(), &privacy));
    assert_eq!(state.state_view_generation(), generation);
    assert!(
        state
            .da_shard_cursors
            .read()
            .canonical_reset_heights()
            .is_empty()
    );
}

#[test]
fn lifecycle_publication_moves_original_projections_and_defers_disk_until_generation_closes() {
    let (state, mut pending, nexus) = fixture();
    let prepared = PreparedLaneLifecycleEffects::prepare(&pending, &nexus);
    let manifests = Arc::clone(&prepared.manifests);
    let privacy = Arc::clone(&prepared.privacy);
    let expected_config = prepared.lane_config.clone();
    // Mutating the source after capture cannot retarget retained projections.
    pending.updated_lane_manifests = Arc::new(LaneManifestRegistry::default());
    pending.catalog_update.lanes_to_reset.clear();
    pending.transition_height = 99;
    drop(pending);
    let path = state.da_shard_cursor_journal_path();
    let disk_before = std::fs::read(&path).ok();
    let generation = state.state_view_generation();
    let mut publication_notice = state.state_view_publication();
    let _commit = state.state_commit_lock.lock();
    let _lifecycle = state.lane_lifecycle_lock.lock();
    #[cfg(feature = "telemetry")]
    state.telemetry.set_da_receipt_cursor(1, 7, 99);
    let post = {
        let _write = state.state_write_lock.lock();
        let publication = publication_notice.begin();
        let post = publish(prepared, &state, &publication, true);
        assert_eq!(state.state_view_generation(), generation + 1);
        assert!(Arc::ptr_eq(&state.lane_manifests.read(), &manifests));
        assert!(Arc::ptr_eq(&state.lane_privacy_registry.read(), &privacy));
        assert_eq!(
            state
                .da_shard_cursors
                .read()
                .canonical_reset_height_for_lane(LaneId::new(1)),
            Some(1)
        );
        assert_eq!(
            state
                .da_shard_cursors
                .read()
                .canonical_reset_height_for_lane(LaneId::new(9)),
            None
        );
        assert_eq!(std::fs::read(&path).ok(), disk_before);
        #[cfg(feature = "telemetry")]
        {
            use iroha_data_model::da::{
                commitment::{DaProofScheme, RetentionClass},
                types::BlobDigest,
            };
            assert!(
                state
                    .telemetry
                    .metrics_ref()
                    .da_receipt_cursor_status()
                    .is_empty()
            );
            // The real same-carrier DA cursor path runs after lifecycle reset.
            // Post-generation lifecycle work must not erase its new metrics.
            let record = DaCommitmentRecord::new(
                LaneId::new(1),
                1,
                0,
                BlobDigest::new([1; 32]),
                ManifestDigest::new([2; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([3; 32]),
                None,
                RetentionClass::default(),
                StorageTicketId::new([4; 32]),
                iroha_crypto::Signature::try_from_bytes(&[5; 64]).unwrap(),
            );
            state
                .advance_da_receipt_cursors_from_bundle(1, &[record])
                .unwrap();
        }
        post
    };
    assert_eq!(state.state_view_generation(), generation + 2);
    assert!(post.persist_cursor_journal);
    post.publish(&state);
    #[cfg(feature = "telemetry")]
    {
        let cursors = state.telemetry.metrics_ref().da_receipt_cursor_status();
        assert_eq!(cursors.len(), 1);
        assert_eq!(cursors[0].lane_id, 1);
        assert_eq!(cursors[0].epoch, 1);
        assert_eq!(cursors[0].highest_sequence, 0);
    }
    let persisted = DaShardCursorJournal::load(&expected_config, &path).unwrap();
    assert_eq!(
        persisted.canonical_reset_height_for_lane(LaneId::new(1)),
        Some(1)
    );
    assert_eq!(
        persisted.canonical_reset_height_for_lane(LaneId::new(9)),
        None
    );
    assert!(Arc::ptr_eq(&state.lane_privacy_registry.read(), &privacy));
}

#[test]
fn lifecycle_replay_prevalidation_keeps_canonical_watermark_without_cursor_publication() {
    let (state, pending, nexus) = fixture();
    let prepared = PreparedLaneLifecycleEffects::prepare(&pending, &nexus);
    let path = state.da_shard_cursor_journal_path();
    let disk_before = std::fs::read(&path).ok();
    let mut publication_notice = state.state_view_publication();
    let _commit = state.state_commit_lock.lock();
    let _lifecycle = state.lane_lifecycle_lock.lock();
    let post = {
        let _write = state.state_write_lock.lock();
        let publication = publication_notice.begin();
        publish(prepared, &state, &publication, false)
    };
    assert!(!post.persist_cursor_journal);
    post.publish(&state);
    assert_eq!(std::fs::read(&path).ok(), disk_before);
    assert_eq!(
        state
            .da_shard_cursors
            .read()
            .canonical_reset_height_for_lane(LaneId::new(1)),
        Some(1)
    );
}

#[test]
fn post_persistence_uses_captured_cursor_without_new_reader_release() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    let (state, pending_lifecycle, nexus) = fixture();
    let prepared = PreparedLaneLifecycleEffects::prepare(&pending_lifecycle, &nexus);
    let path = state.da_shard_cursor_journal_path();
    assert!(!path.as_os_str().is_empty());
    let mut indexes = effect_publication::StateEffectLocks::new(&state);
    let mut notice = state.state_view_publication();
    let commit = state.state_commit_lock.lock();
    let lifecycle = state.lane_lifecycle_lock.lock();
    let write = state.state_write_lock.lock();
    indexes.try_prepare().expect("original effect writers");
    let generation = notice.begin();
    let mut post = prepared.publish(&state, &mut indexes, &generation, true);
    // A same-carrier cursor update after lifecycle publication must be in the
    // final persisted image, without reopening the live index afterward.
    indexes
        .da_shard_cursors
        .as_mut()
        .unwrap()
        .mark_lanes_canonically_reset(&BTreeSet::from([LaneId::new(1)]), 7);
    post.capture_snapshot(&state, indexes.da_shard_cursors.as_ref().unwrap());
    assert_eq!(
        post.snapshot
            .as_ref()
            .unwrap()
            .canonical_reset_height_for_lane(LaneId::new(1)),
        Some(7)
    );
    let wait = state
        .da_shard_cursors
        .try_write_or_wait()
        .expect_err("original writer held");
    let mut pending = std::pin::pin!(wait.wait_for_release());
    let mut context = Context::from_waker(Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    drop(generation);
    indexes.release_writers();
    post.publish(&state);
    assert!(
        pending.as_mut().poll(&mut context).is_pending(),
        "post work must not emit a fresh reader release under State fences"
    );
    let persisted = DaShardCursorJournal::load(&nexus.lane_config, &path).unwrap();
    assert_eq!(
        persisted.canonical_reset_height_for_lane(LaneId::new(1)),
        Some(7)
    );
    drop(write);
    drop(lifecycle);
    drop(commit);
    drop(indexes);
    assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
}

// Component tests use the same real index preparation kernel as both publishers.
fn publish(
    prepared: PreparedLaneLifecycleEffects,
    state: &State,
    generation: &StateViewGenerationWriteGuard<'_>,
    process: bool,
) -> LaneLifecyclePostPublication {
    let mut indexes = effect_publication::StateEffectLocks::new(state);
    indexes.try_prepare().expect("uncontended original indexes");
    let mut post = prepared.publish(state, &mut indexes, generation, process);
    post.capture_snapshot(
        state,
        indexes
            .da_shard_cursors
            .as_ref()
            .expect("original cursor writer"),
    );
    indexes.release_writers();
    post
}
