//! Original-candidate DA projection controls; these do not authorize a carrier.

use super::*;
use crate::query::store::LiveQueryStore;
use iroha_data_model::da::{
    commitment::{DaCommitmentBundle, DaProofScheme, RetentionClass},
    confidential_compute::ConfidentialComputePolicy,
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
    PreparedDaCommitmentEffects::try_prepare(
        PendingDaCommitmentBundle {
            block_height: height,
            bundle: DaCommitmentBundle::new(records),
        },
        &state.nexus_snapshot(),
        state.canonical_runtime.view().get(),
        &state.ivm_execution_budget(),
    )
    .unwrap_or_else(|(_, error)| panic!("original DA projection admission: {error}"))
}

#[test]
fn dropped_projection_never_mutates_any_da_index() {
    let state = state();
    let record = record(LaneId::SINGLE, 9);
    let prepared = prepare(&state, 1, vec![record]);
    assert_eq!(prepared.active.as_slice().len(), 1);
    assert_eq!(prepared.query_visible.as_slice().len(), 1);
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
        prepared_with_ahead_cache.query_visible.as_slice(),
        prepared.query_visible.as_slice()
    );
    assert_eq!(
        prepared_with_ahead_cache.identity_visible.as_slice(),
        prepared.identity_visible.as_slice()
    );
    let post = {
        let mut generation_notice = state.state_view_publication();
        let _writer = state.state_write_lock.lock();
        let generation = generation_notice.begin();
        publish(
            prepared,
            &state,
            &state.nexus_snapshot().lane_config,
            &generation,
            false,
        )
    };
    assert!(
        !post.persist,
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
        let prepared = PreparedDaCommitmentEffects::try_prepare(
            PendingDaCommitmentBundle {
                block_height: height,
                bundle: DaCommitmentBundle::new(vec![record.clone()]),
            },
            &nexus,
            &runtime,
            &state.ivm_execution_budget(),
        )
        .unwrap_or_else(|(_, error)| panic!("original DA projection admission: {error}"));
        assert_eq!(prepared.query_visible.as_slice().is_empty(), height <= 5);
        assert_eq!(prepared.identity_visible.as_slice().is_empty(), height <= 5);
        assert_eq!(prepared.pending.bundle.commitments, vec![record.clone()]);
    }
}

#[test]
fn retired_lane_keeps_original_bundle_position_and_reserved_identity() {
    let state = state();
    let active = record(LaneId::SINGLE, 18);
    let retired = record(LaneId::new(1), 27);
    let prepared = prepare(&state, 3, vec![retired.clone(), active.clone()]);
    assert_eq!(prepared.active.as_slice(), &[0]);
    assert_eq!(
        prepared.pending.bundle.commitments[prepared.active.as_slice()[0]],
        active
    );
    let original = prepared.pending.bundle.commitments.clone();
    {
        let mut generation_notice = state.state_view_publication();
        let _writer = state.state_write_lock.lock();
        let generation = generation_notice.begin();
        let post = publish(
            prepared,
            &state,
            &state.nexus_snapshot().lane_config,
            &generation,
            true,
        );
        assert!(post.persist);
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
    let prepared = PreparedDaCommitmentEffects::try_prepare(
        PendingDaCommitmentBundle {
            block_height: 3,
            bundle: DaCommitmentBundle::new(vec![confidential.clone(), retired]),
        },
        &nexus,
        state.canonical_runtime.view().get(),
        &state.ivm_execution_budget(),
    )
    .unwrap_or_else(|(_, error)| panic!("original DA projection admission: {error}"));
    assert_eq!(prepared.confidential.as_slice().len(), 1);
    assert_eq!(prepared.confidential.as_slice(), &[1]);
    assert_eq!(&prepared.pending.bundle.commitments[1], &confidential);
    // The future candidate catalog differs from the current committed catalog.
    // Neither publishing component may re-read the live catalog for this input.
    assert!(state.nexus_snapshot().lane_config.entry(lane).is_none());
    let post = {
        let mut generation_notice = state.state_view_publication();
        let _writer = state.state_write_lock.lock();
        let generation = generation_notice.begin();
        publish(prepared, &state, &nexus.lane_config, &generation, true)
    };
    assert!(post.persist);
    assert_eq!(nexus.lane_config.shard_id(lane), 7);
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
    let nexus = state.nexus_snapshot();
    let mut post = prepared.publish(&state, &mut indexes, &nexus.lane_config, &generation, true);
    indexes
        .da_shard_cursors
        .as_mut()
        .unwrap()
        .mark_lanes_canonically_reset(&BTreeSet::from([LaneId::SINGLE]), 7);
    post.capture_snapshot(
        &state,
        &nexus.lane_config,
        indexes.da_shard_cursors.as_ref().unwrap(),
    );
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
    lane_config: &iroha_config::parameters::actual::LaneConfig,
    generation: &StateViewGenerationWriteGuard<'_>,
    process: bool,
) -> DaCommitmentPostPublication {
    let mut indexes = effect_publication::StateEffectLocks::new(state);
    indexes.try_prepare().expect("uncontended original indexes");
    let mut post = prepared.publish(state, &mut indexes, lane_config, generation, process);
    post.capture_snapshot(
        state,
        lane_config,
        indexes
            .da_shard_cursors
            .as_ref()
            .expect("original cursor writer"),
    );
    indexes.release_writers();
    post
}

#[test]
fn visibility_admission_returns_original_bundle_before_any_partial_charge() {
    let state = state();
    let records = vec![record(LaneId::SINGLE, 9), record(LaneId::new(1), 18)];
    let backing = records.as_ptr();
    let signatures = records
        .iter()
        .map(|record| record.acknowledgement_sig.payload().as_ptr())
        .collect::<Vec<_>>();
    let mut pending = PendingDaCommitmentBundle {
        block_height: 3,
        bundle: DaCommitmentBundle::new(records),
    };
    // Only half of total descriptor capacity is free; no partial owner remains.
    let demand = projection_bytes(2);
    let budget = AllocationBudget::new(demand);
    let occupied = budget.try_reserve_bytes(demand / 2).unwrap();
    let nexus = state.nexus_snapshot();
    let runtime = state.canonical_runtime.view();
    for _ in 0..3 {
        let (returned, reason) =
            match PreparedDaCommitmentEffects::try_prepare(pending, &nexus, runtime.get(), &budget)
            {
                Ok(_) => panic!("all actual descriptor layouts must be admitted together"),
                Err(refusal) => refusal,
            };
        assert!(matches!(
            reason.allocation_refusal(),
            Some(AllocationRefusal::Capacity { .. })
        ));
        assert_eq!(
            reason.allocation_refusal(),
            Some(&budget.try_reserve_bytes(demand).unwrap_err())
        );
        assert_eq!(budget.reserved_bytes(), demand / 2);
        assert_eq!(returned.bundle.commitments.as_ptr(), backing);
        assert_eq!(
            returned
                .bundle
                .commitments
                .iter()
                .map(|record| record.acknowledgement_sig.payload().as_ptr())
                .collect::<Vec<_>>(),
            signatures
        );
        assert_eq!(returned.block_height, 3);
        pending = returned;
    }
    drop(occupied);
    let prepared =
        PreparedDaCommitmentEffects::try_prepare(pending, &nexus, runtime.get(), &budget)
            .unwrap_or_else(|(_, error)| {
                panic!("same original admits after capacity releases: {error}")
            });
    assert_eq!(prepared.pending.bundle.commitments.as_ptr(), backing);
    assert!(prepared.query_visible.belongs_to(&budget));
    assert!(prepared.identity_visible.belongs_to(&budget));
    assert!(prepared.active.belongs_to(&budget));
    assert!(prepared.confidential.belongs_to(&budget));
    let foreign = AllocationBudget::new(demand);
    assert!(!prepared.query_visible.belongs_to(&foreign));
    assert!(!prepared.identity_visible.belongs_to(&foreign));
    assert!(!prepared.active.belongs_to(&foreign));
    assert!(!prepared.confidential.belongs_to(&foreign));
    assert_eq!(budget.reserved_bytes(), demand);
    drop(prepared);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(foreign.reserved_bytes(), 0);
}

#[test]
fn visibility_sort_deduplicates_in_place_without_refunding_retained_backing() {
    let count = 4;
    let demand = projection_bytes(count);
    let budget = AllocationBudget::new(demand);
    let (mut query, identity, active, confidential) = reserve_projection(count, &budget).unwrap();
    let low = DaCommitmentKey::from_record(&record(LaneId::SINGLE, 9));
    let high = DaCommitmentKey::from_record(&record(LaneId::new(3), 18));
    for key in [high, low, high, low] {
        query.push_reserved(key);
    }
    let backing = query.as_slice().as_ptr();
    sort_unique(&mut query);
    assert_eq!(query.as_slice(), &[low, high]);
    assert_eq!(query.as_slice().as_ptr(), backing);
    assert_eq!(query.capacity(), count);
    assert_eq!(budget.reserved_bytes(), demand);
    drop(query);
    assert_eq!(
        budget.reserved_bytes(),
        demand - Layout::array::<DaCommitmentKey>(count).unwrap().size()
    );
    drop(identity);
    assert_eq!(
        budget.reserved_bytes(),
        2 * Layout::array::<usize>(count).unwrap().size()
    );
    drop((active, confidential));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn empty_visibility_needs_no_backing_and_size_overflow_reserves_nothing() {
    let budget = AllocationBudget::new(0);
    let (query, identity, active, confidential) = reserve_projection(0, &budget).unwrap();
    assert!(active.as_slice().is_empty());
    assert!(confidential.as_slice().is_empty());
    assert!(query.as_slice().is_empty());
    assert!(identity.as_slice().is_empty());
    assert_eq!(query.capacity(), 0);
    assert_eq!(identity.capacity(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
    let error = match reserve_projection(usize::MAX, &budget) {
        Ok(_) => panic!("unrepresentable key backing must refuse"),
        Err(error) => error,
    };
    assert_eq!(
        error.allocation_refusal(),
        Some(&AllocationRefusal::DemandOverflow)
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

fn projection_bytes(count: usize) -> usize {
    2 * (Layout::array::<DaCommitmentKey>(count).unwrap().size()
        + Layout::array::<usize>(count).unwrap().size())
}
