//! Original-candidate DA projection controls; these do not authorize a carrier.

use super::*;
use crate::query::store::LiveQueryStore;
use iroha_data_model::da::{
    commitment::{DaCommitmentBundle, DaProofScheme, RetentionClass},
    types::BlobDigest,
};

fn state() -> State {
    State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn record(lane: LaneId, tag: u8) -> DaCommitmentRecord {
    DaCommitmentRecord::new(
        lane,
        1,
        0,
        BlobDigest::new([tag; 32]),
        ManifestDigest::new([tag + 1; 32]),
        DaProofScheme::MerkleSha256,
        Hash::prehashed([tag + 2; 32]),
        None,
        RetentionClass::default(),
        StorageTicketId::new([tag + 3; 32]),
        iroha_crypto::Signature::try_from_bytes(&[tag; 64]).unwrap(),
    )
}

fn prepare(
    state: &State,
    height: u64,
    records: Vec<DaCommitmentRecord>,
) -> PreparedDaCommitmentEffects {
    PreparedDaCommitmentEffects::prepare(
        PendingDaCommitmentBundle {
            block_height: height,
            bundle: DaCommitmentBundle::new(records),
        },
        &state.nexus_snapshot(),
        state.canonical_runtime.view().get(),
    )
}

#[test]
fn dropped_projection_never_mutates_any_da_index() {
    let state = state();
    let record = record(LaneId::SINGLE, 9);
    let prepared = prepare(&state, 1, vec![record]);
    assert_eq!(prepared.active.len(), 1);
    assert_eq!(prepared.query_visible.len(), 1);
    drop(prepared);
    assert!(state.da_commitments.read().bundle_at(1).is_none());
    assert!(
        state
            .da_shard_cursors
            .read()
            .get(0, LaneId::SINGLE)
            .is_none()
    );
}

#[test]
fn ahead_disposable_reset_journal_cannot_suppress_original_visibility() {
    let state = state();
    let record = record(LaneId::SINGLE, 9);
    let prepared = prepare(&state, 1, vec![record.clone()]);
    let bundle_allocation = prepared.pending.bundle.commitments.as_ptr();
    state
        .da_shard_cursors
        .write()
        .mark_lanes_canonically_reset(&BTreeSet::from([LaneId::SINGLE]), 999);
    let prepared_with_ahead_cache = prepare(&state, 1, vec![record.clone()]);
    assert_eq!(
        prepared_with_ahead_cache.query_visible,
        prepared.query_visible
    );
    assert_eq!(
        prepared_with_ahead_cache.identity_visible,
        prepared.identity_visible
    );
    let post = {
        let mut generation_notice = state.state_view_publication();
        let _writer = state.state_write_lock.lock();
        let generation = generation_notice.begin();
        publish(prepared, &state, &generation, false)
    };
    assert!(
        post.lane_config.is_none(),
        "replay does not schedule disposable persistence"
    );
    let commitments = state.da_commitments.read();
    assert_eq!(
        commitments.bundle_at(1).unwrap().commitments.as_ptr(),
        bundle_allocation,
        "publication moves the original bundle allocation"
    );
    assert!(commitments.get_by_manifest(&record.manifest_hash).is_some());
    assert!(
        commitments
            .get_committed_by_key(&DaCommitmentKey::from_record(&record))
            .is_some()
    );
}

#[test]
fn retained_canonical_recreation_hides_old_identity_even_with_empty_caches() {
    let state = state();
    let record = record(LaneId::SINGLE, 12);
    let nexus = state.nexus_snapshot();
    let mut runtime = state.canonical_runtime.view().get().clone();
    let lineage = runtime
        .lane_incarnation_lineage
        .iter_mut()
        .find(|entry| entry.lane_id == LaneId::SINGLE)
        .unwrap();
    lineage.activation_height = 5;
    for height in [4, 5, 6] {
        let prepared = PreparedDaCommitmentEffects::prepare(
            PendingDaCommitmentBundle {
                block_height: height,
                bundle: DaCommitmentBundle::new(vec![record.clone()]),
            },
            &nexus,
            &runtime,
        );
        assert_eq!(prepared.query_visible.is_empty(), height <= 5);
        assert_eq!(prepared.identity_visible.is_empty(), height <= 5);
        assert_eq!(prepared.pending.bundle.commitments, vec![record.clone()]);
    }
}

#[test]
fn retired_lane_keeps_original_bundle_position_and_reserved_identity() {
    let state = state();
    let active = record(LaneId::SINGLE, 18);
    let retired = record(LaneId::new(1), 27);
    let prepared = prepare(&state, 3, vec![retired.clone(), active.clone()]);
    assert_eq!(prepared.active, vec![active.clone()]);
    let original = prepared.pending.bundle.commitments.clone();
    {
        let mut generation_notice = state.state_view_publication();
        let _writer = state.state_write_lock.lock();
        let generation = generation_notice.begin();
        let post = publish(prepared, &state, &generation, true);
        assert!(post.lane_config.is_some());
    }
    let commitments = state.da_commitments.read();
    assert_eq!(commitments.bundle_at(3).unwrap().commitments, original);
    assert!(
        commitments
            .get_by_manifest(&retired.manifest_hash)
            .is_none()
    );
    for record in [active, retired] {
        let retained = commitments
            .get_committed_by_key(&DaCommitmentKey::from_record(&record))
            .unwrap();
        assert_eq!(
            usize::try_from(retained.location.index_in_bundle).unwrap(),
            original
                .iter()
                .position(|candidate| candidate == &record)
                .unwrap()
        );
    }
}

#[test]
fn confidential_receipt_and_cursor_use_original_position_policy_and_shard() {
    use iroha_data_model::{
        da::confidential_compute::ConfidentialComputeMechanism,
        nexus::{LaneConfig, LaneStorageProfile},
    };
    let state = state();
    let lane = LaneId::new(2);
    let mut nexus = state.nexus_snapshot();
    nexus.lane_catalog = LaneCatalog::new(
        std::num::NonZeroU32::new(3).unwrap(),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: lane,
                alias: "retained-confidential".to_owned(),
                shard_id: Some(iroha_model_base::topology::ShardId::new(7)),
                storage: LaneStorageProfile::SplitReplica,
                confidential_compute: Some(ConfidentialComputePolicy::new(
                    ConfidentialComputeMechanism::Encryption,
                    std::num::NonZeroU32::new(4).unwrap(),
                    BTreeSet::from(["retained-audience".to_owned()]),
                )),
                ..LaneConfig::default()
            },
        ],
    )
    .unwrap();
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    let retired = record(LaneId::new(1), 30);
    let confidential = record(lane, 39);
    let prepared = PreparedDaCommitmentEffects::prepare(
        PendingDaCommitmentBundle {
            block_height: 3,
            bundle: DaCommitmentBundle::new(vec![confidential.clone(), retired]),
        },
        &nexus,
        state.canonical_runtime.view().get(),
    );
    assert_eq!(prepared.confidential.len(), 1);
    assert_eq!(prepared.confidential[0].1.index_in_bundle, 1);
    // The future candidate catalog differs from the current committed catalog.
    // Neither publishing component may re-read the live catalog for this input.
    assert!(state.nexus_snapshot().lane_config.entry(lane).is_none());
    let post = {
        let mut generation_notice = state.state_view_publication();
        let _writer = state.state_write_lock.lock();
        let generation = generation_notice.begin();
        publish(prepared, &state, &generation, true)
    };
    assert_eq!(post.lane_config.as_ref().unwrap().shard_id(lane), 7);
    assert!(state.da_shard_cursors.read().get(7, lane).is_some());
    assert!(state.da_shard_cursors.read().get(2, lane).is_none());
    let store = state.da_confidential_compute.read();
    let receipt = store
        .get_by_lane_epoch_sequence(lane.as_u32(), 1, 0)
        .unwrap();
    assert_eq!(receipt.location.index_in_bundle, 1);
    assert_eq!(receipt.receipt.key_version.get(), 4);
    assert_eq!(
        receipt.receipt.allowed_audiences,
        BTreeSet::from(["retained-audience".to_owned()])
    );
}

#[test]
fn post_persistence_uses_captured_cursor_without_new_reader_release() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };

    let state = state();
    let prepared = prepare(&state, 1, vec![record(LaneId::SINGLE, 9)]);
    assert!(!state.da_shard_cursor_journal_path().as_os_str().is_empty());
    let mut indexes = effect_publication::StateEffectLocks::new(&state);
    let mut notice = state.state_view_publication();
    let commit = state.state_commit_lock.lock();
    let write = state.state_write_lock.lock();
    indexes.try_prepare().expect("original effect writers");
    let generation = notice.begin();
    let mut post = prepared.publish(&state, &mut indexes, &generation, true);
    indexes
        .da_shard_cursors
        .as_mut()
        .unwrap()
        .mark_lanes_canonically_reset(&BTreeSet::from([LaneId::SINGLE]), 7);
    post.capture_snapshot(&state, indexes.da_shard_cursors.as_ref().unwrap());
    assert_eq!(
        post.snapshot
            .as_ref()
            .unwrap()
            .canonical_reset_height_for_lane(LaneId::SINGLE),
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
    drop(write);
    drop(commit);
    drop(indexes);
    assert_eq!(pending.as_mut().poll(&mut context), Poll::Ready(()));
}

// Component tests use the same real index preparation kernel as both publishers.
fn publish(
    prepared: PreparedDaCommitmentEffects,
    state: &State,
    generation: &StateViewGenerationWriteGuard<'_>,
    process: bool,
) -> DaCommitmentPostPublication {
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
