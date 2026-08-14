// Signed retirement-snapshot reconciliation coverage stays in queue::tests.
use crate::kura::{
    AutonomousLaneRetirementSnapshotAttemptAnchorV1, AutonomousLaneRetirementSnapshotEvidenceV1,
};
fn snapshot_lifecycle_projection_fixture(
    reservation_group: LaneQueueReservationGroupBindingV1,
    ordered_keys: Vec<LaneQueueReservationKeyV2>,
    recovered_state: ProductionInFlightFirstReleaseStateProjection,
    cursor_seed: &[u8],
) -> LaneReservationSnapshotLifecycleProjectionV1 {
    LaneReservationSnapshotLifecycleProjectionV1 {
        height_context_id: iroha_data_model::block::consensus_v2::HeightContextId(
            HashOf::<iroha_data_model::block::consensus_v2::HeightContext>::from_untyped_unchecked(
                Hash::new(b"snapshot-planner-height-context"),
            ),
        ),
        origin_proposal_hash: Hash::new(b"snapshot-planner-origin-proposal"),
        executable_payload_hash: Hash::new(b"snapshot-planner-executable-payload"),
        cursor_sequence: 1,
        cursor_hash: Hash::new(cursor_seed),
        cursor_phase: AutonomousLifecycleCursorPhaseKindV2::Live,
        owner_generation: 1,
        source_generation: None,
        validator_set_hash_version: 1,
        validator_set_hash: HashOf::<Vec<PeerId>>::from_untyped_unchecked(Hash::new(
            b"snapshot-planner-validator-set",
        )),
        validator_count: 1,
        local_validator_index: 0,
        local_actor: 1,
        producer: 1,
        reservation_group,
        ordered_keys,
        cursor_before: recovered_state,
        cursor_after: None,
        recovered_state,
    }
}
fn retired_release_snapshot_state_fixture(
    reservation_group: LaneQueueReservationGroupBindingV1,
    phase: AutonomousLaneRetirementQueueSnapshotPhaseV1,
    validator_count: u8,
    producer_index: u16,
    local_actor_index: u16,
    released_prefix: u64,
) -> ProductionInFlightFirstReleaseStateProjection {
    assert!((1..=128).contains(&validator_count));
    assert!(producer_index < u16::from(validator_count));
    assert!(local_actor_index < u16::from(validator_count));
    let producer = 1_u128 << u32::from(producer_index);
    let local_actor = 1_u128 << u32::from(local_actor_index);
    let validator_mask = if validator_count == 128 {
        u128::MAX
    } else {
        (1_u128 << validator_count) - 1
    };
    let reservation_state = match phase {
        AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared => {
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED
        }
        AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed => {
            assert_eq!(released_prefix, reservation_group.reservation_count);
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED
        }
    };
    let binding_a = canonical_lane_queue_reservation_group_identity_projection(reservation_group);
    let state = ProductionInFlightFirstReleaseStateProjection {
        validator_count,
        producer,
        producer_selected_owner: producer,
        replicated_carrier_owners: validator_mask & !producer,
        payload_binding_a: producer | local_actor,
        binding_a,
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
            selected_count: reservation_group.reservation_count,
            reservation_state,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection {
            kura_active: producer | local_actor,
            execution_input_durable: 0,
            ready_qc_durable: false,
        },
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: producer | local_actor,
            ready_authorized: 0,
            crashed: 0,
            producer_alive: true,
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v4: true,
            ever_reservation_v5: true,
            pending_high_water: reservation_group.reservation_count,
            released_high_water: released_prefix,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection {
            release_scope: binding_a,
            release_owner: local_actor,
            ..ProductionInFlightFirstReleaseDecisionProjection::default()
        },
        release: ProductionInFlightFirstReleaseReleaseProjection {
            kura_retired: true,
            pending_prefix: reservation_group.reservation_count,
            released_prefix,
            fifo_restored: false,
        },
    };
    assert!(production_in_flight_first_release_state_kernel(state));
    state
}
fn retired_release_lifecycle_projection_fixture(
    reservation_group: LaneQueueReservationGroupBindingV1,
    ordered_keys: Vec<LaneQueueReservationKeyV2>,
    recovered_state: ProductionInFlightFirstReleaseStateProjection,
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
    validator_set_hash: HashOf<Vec<PeerId>>,
    producer_index: u16,
    local_actor_index: u16,
    cursor_seed: &[u8],
) -> LaneReservationSnapshotLifecycleProjectionV1 {
    let validator_count = recovered_state.validator_count;
    LaneReservationSnapshotLifecycleProjectionV1 {
        height_context_id: iroha_data_model::block::consensus_v2::HeightContextId(
            HashOf::<iroha_data_model::block::consensus_v2::HeightContext>::from_untyped_unchecked(
                Hash::new(b"retired-release-height-context"),
            ),
        ),
        origin_proposal_hash,
        executable_payload_hash,
        cursor_sequence: 1,
        cursor_hash: Hash::new(cursor_seed),
        cursor_phase: AutonomousLifecycleCursorPhaseKindV2::Live,
        owner_generation: 1,
        source_generation: None,
        validator_set_hash_version: 1,
        validator_set_hash,
        validator_count,
        local_validator_index: local_actor_index,
        local_actor: 1_u128 << u32::from(local_actor_index),
        producer: 1_u128 << u32::from(producer_index),
        reservation_group,
        ordered_keys,
        cursor_before: recovered_state,
        cursor_after: None,
        recovered_state,
    }
}
#[derive(Clone, Copy)]
enum RetiredReleasePairMismatch {
    None,
    OriginProposal,
    ExecutablePayload,
    LocalActor,
    RetirementHash,
}
fn authorize_retired_release_pair_fixture(
    phase: AutonomousLaneRetirementQueueSnapshotPhaseV1,
    local_actor_index: u16,
    released_prefix: u64,
    mismatch: RetiredReleasePairMismatch,
) -> Result<LaneReservationSnapshotRecoveryAuthorization, LaneQueueReservationError> {
    let fixture = replayed_snapshot_recovery_fixture(&[2], Some((0, phase)));
    let (reservation_group, ordered_keys) = fixture.groups[0].clone();
    let exact_barrier = match phase {
        AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared => fixture
            .snapshot
            .prepared_release_barriers
            .first()
            .expect("prepared release fixture has its barrier"),
        AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed => {
            &fixture
                .snapshot
                .completed_releases
                .first()
                .expect("completed release fixture has its barrier")
                .barrier
        }
    };
    let exact_origin_proposal_hash = exact_barrier.origin_proposal_hash;
    let exact_executable_payload_hash = exact_barrier.executable_payload_hash;
    let exact_retirement_hash = exact_barrier.retirement_hash;
    let validator_set_hash =
        HashOf::<Vec<PeerId>>::from_untyped_unchecked(Hash::new(b"retired-release-validator-set"));
    let producer_index = 0;
    let anchor_local_actor_index = local_actor_index;
    let recovered_state = retired_release_snapshot_state_fixture(
        reservation_group,
        phase,
        2,
        producer_index,
        anchor_local_actor_index,
        released_prefix,
    );
    let anchor = AutonomousLaneRetirementSnapshotAttemptAnchorV1::from_exact_parts_for_test(
        if matches!(mismatch, RetiredReleasePairMismatch::OriginProposal) {
            Hash::new(b"retired-release-wrong-origin-proposal")
        } else {
            exact_origin_proposal_hash
        },
        if matches!(mismatch, RetiredReleasePairMismatch::ExecutablePayload) {
            Hash::new(b"retired-release-wrong-executable-payload")
        } else {
            exact_executable_payload_hash
        },
        1,
        validator_set_hash,
        2,
        producer_index,
        anchor_local_actor_index,
    )
    .expect("construct bounded retirement snapshot anchor");
    let evidence = AutonomousLaneRetirementSnapshotEvidenceV1::from_exact_parts_for_test(
        phase,
        reservation_group,
        if matches!(mismatch, RetiredReleasePairMismatch::RetirementHash) {
            Hash::new(b"retired-release-wrong-retirement")
        } else {
            exact_retirement_hash
        },
        anchor,
        recovered_state,
    );
    let planner_evidence = LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
        fixture.snapshot.clone(),
        vec![(
            reservation_group,
            ordered_keys.clone(),
            LaneReservationSnapshotPlannerProjectionKind::RetiredRelease { evidence },
        )],
    );
    let lifecycle_local_actor_index = match (mismatch, local_actor_index) {
        (RetiredReleasePairMismatch::LocalActor, 0) => 1,
        (RetiredReleasePairMismatch::LocalActor, _) => 0,
        (_, index) => index,
    };
    let lifecycle = retired_release_lifecycle_projection_fixture(
        reservation_group,
        ordered_keys,
        recovered_state,
        exact_origin_proposal_hash,
        exact_executable_payload_hash,
        validator_set_hash,
        producer_index,
        lifecycle_local_actor_index,
        b"retired-release-cursor",
    );
    fixture.queue.authorize_lane_reservation_snapshot_recovery(
        checked_startup_reconciliation_receipt(&fixture.queue),
        vec![lifecycle],
        Some(planner_evidence),
    )
}
#[test]
fn snapshot_recovery_accepts_prepared_retired_release_pair_for_local_committee_roles() {
    for local_actor_index in [0, 1] {
        let authorization = authorize_retired_release_pair_fixture(
            AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared,
            local_actor_index,
            1,
            RetiredReleasePairMismatch::None,
        )
        .expect("authorize exact partial-prefix retirement evidence and signed cursor pair");
        assert!(authorization.checked_groups.is_empty());
        assert_eq!(authorization.checked_planner_groups.len(), 1);
        let recovered = authorization.checked_planner_groups[0].recovered_state;
        let local_actor = 1_u128 << u32::from(local_actor_index);
        assert_eq!(recovered.payload_binding_a, 1 | local_actor);
        assert_eq!(recovered.decision.release_owner, local_actor);
        assert_eq!(
            recovered.queue.reservation_state,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED
        );
        assert_eq!(recovered.release.pending_prefix, 2);
        assert_eq!(recovered.release.released_prefix, 1);
        authorization
            .into_reconciliation_receipt()
            .expect("consume exact paired action-25 stutter");
    }
}
#[test]
fn snapshot_recovery_accepts_completed_retired_release_pair_with_full_prefix() {
    let authorization = authorize_retired_release_pair_fixture(
        AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed,
        1,
        2,
        RetiredReleasePairMismatch::None,
    )
    .expect("authorize exact full-prefix completed retirement evidence and signed cursor pair");
    assert!(authorization.checked_groups.is_empty());
    assert_eq!(authorization.checked_planner_groups.len(), 1);
    let recovered = authorization.checked_planner_groups[0].recovered_state;
    assert_eq!(
        recovered.queue.reservation_state,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED
    );
    assert_eq!(recovered.release.pending_prefix, 2);
    assert_eq!(recovered.release.released_prefix, 2);
    assert!(!recovered.release.fifo_restored);
    authorization
        .into_reconciliation_receipt()
        .expect("consume exact completed-release action-25 stutter");
}
#[test]
fn snapshot_recovery_rejects_retired_release_pair_identity_drift() {
    for mismatch in [
        RetiredReleasePairMismatch::OriginProposal,
        RetiredReleasePairMismatch::ExecutablePayload,
        RetiredReleasePairMismatch::LocalActor,
        RetiredReleasePairMismatch::RetirementHash,
    ] {
        assert!(matches!(
            authorize_retired_release_pair_fixture(
                AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared,
                1,
                1,
                mismatch,
            ),
            Err(LaneQueueReservationError::InvalidIdentity(_))
        ));
    }
}
#[test]
fn retirement_snapshot_test_anchor_rejects_out_of_bounds_actor_indices() {
    let validator_set_hash = HashOf::<Vec<PeerId>>::from_untyped_unchecked(Hash::new(
        b"retired-release-invalid-validator-set",
    ));
    assert!(
        AutonomousLaneRetirementSnapshotAttemptAnchorV1::from_exact_parts_for_test(
            Hash::new(b"retired-release-origin"),
            Hash::new(b"retired-release-payload"),
            1,
            validator_set_hash,
            0,
            0,
            0,
        )
        .is_err()
    );
    assert!(
        AutonomousLaneRetirementSnapshotAttemptAnchorV1::from_exact_parts_for_test(
            Hash::new(b"retired-release-origin"),
            Hash::new(b"retired-release-payload"),
            1,
            validator_set_hash,
            2,
            0,
            2,
        )
        .is_err()
    );
}
