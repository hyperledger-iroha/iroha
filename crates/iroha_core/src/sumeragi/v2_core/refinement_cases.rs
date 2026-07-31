//! Deterministic executable cases for the production Sumeragi v2 refinement gate.
//!
//! Keeping these cases in the `refinement::tests` module preserves their
//! source-linked release inventory while the production refinement relation
//! remains small enough for repository source-budget enforcement.

use super::*;

fn successor_identity(domain: u8, kind: u8, byte: u8) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, [byte; 32])
}

fn in_flight_reservation_identity(byte: u8) -> CanonicalIdentityProjection {
    successor_identity(
        IDENTITY_DOMAIN_DURABLE_ARTIFACT,
        IDENTITY_KIND_LANE_QUEUE_RESERVATION,
        byte,
    )
}

fn in_flight_release_identity(byte: u8) -> CanonicalIdentityProjection {
    successor_identity(
        IDENTITY_DOMAIN_DURABLE_ARTIFACT,
        IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER,
        byte,
    )
}

fn in_flight_owner(
    state: u8,
    reservation_identity: CanonicalIdentityProjection,
    release_identity: CanonicalIdentityProjection,
) -> ProductionInFlightReservationOwnerProjection {
    ProductionInFlightReservationOwnerProjection {
        state,
        reservation_identity,
        release_identity,
    }
}

#[test]
fn in_flight_reservation_kernel_accepts_only_identity_bound_local_owner_steps() {
    let absent = ProductionInFlightReservationOwnerProjection::default();
    let reservation = in_flight_reservation_identity(0x41);
    let foreign_reservation = in_flight_reservation_identity(0x42);
    let release = in_flight_release_identity(0x51);
    let live = in_flight_owner(
        IN_FLIGHT_RESERVATION_STATE_LIVE,
        reservation,
        CanonicalIdentityProjection::zero(),
    );
    let foreign_live = in_flight_owner(
        IN_FLIGHT_RESERVATION_STATE_LIVE,
        foreign_reservation,
        CanonicalIdentityProjection::zero(),
    );
    let committed = in_flight_owner(
        IN_FLIGHT_RESERVATION_STATE_COMMITTED,
        reservation,
        CanonicalIdentityProjection::zero(),
    );
    let prepared = in_flight_owner(
        IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED,
        reservation,
        release,
    );
    let completed = in_flight_owner(
        IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED,
        reservation,
        release,
    );
    let transition = |action, requested_release_identity, before, after| {
        ProductionInFlightReservationTransitionProjection {
            action,
            requested_reservation_identity: reservation,
            requested_release_identity,
            before,
            after,
        }
    };

    let reserve = transition(
        IN_FLIGHT_RESERVATION_ACTION_RESERVE,
        CanonicalIdentityProjection::zero(),
        absent,
        live,
    );
    assert_eq!(
        check_production_in_flight_reservation_transition(reserve)
            .expect("exact reserve must mint checked evidence")
            .into_projection(),
        reserve
    );

    for accepted in [
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RESERVE,
            CanonicalIdentityProjection::zero(),
            live,
            live,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,
            CanonicalIdentityProjection::zero(),
            live,
            absent,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,
            CanonicalIdentityProjection::zero(),
            absent,
            absent,
        ),
        ProductionInFlightReservationTransitionProjection {
            requested_reservation_identity: reservation,
            ..transition(
                IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,
                CanonicalIdentityProjection::zero(),
                foreign_live,
                foreign_live,
            )
        },
        transition(
            IN_FLIGHT_RESERVATION_ACTION_COMMIT,
            CanonicalIdentityProjection::zero(),
            live,
            committed,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_COMMIT,
            CanonicalIdentityProjection::zero(),
            committed,
            committed,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT,
            CanonicalIdentityProjection::zero(),
            committed,
            absent,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT,
            CanonicalIdentityProjection::zero(),
            absent,
            absent,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED,
            CanonicalIdentityProjection::zero(),
            live,
            absent,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE,
            release,
            live,
            prepared,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE,
            release,
            prepared,
            prepared,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE,
            release,
            completed,
            completed,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE,
            release,
            prepared,
            completed,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE,
            release,
            completed,
            completed,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,
            release,
            completed,
            absent,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,
            release,
            absent,
            absent,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
            CanonicalIdentityProjection::zero(),
            absent,
            live,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
            CanonicalIdentityProjection::zero(),
            absent,
            committed,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
            release,
            absent,
            prepared,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
            release,
            absent,
            completed,
        ),
    ] {
        assert!(
            production_in_flight_reservation_transition_kernel(accepted),
            "expected accepted primitive transition: {accepted:?}"
        );
    }

    for rejected in [
        transition(
            IN_FLIGHT_RESERVATION_ACTION_COMMIT,
            CanonicalIdentityProjection::zero(),
            absent,
            committed,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE,
            in_flight_release_identity(0x52),
            live,
            prepared,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,
            CanonicalIdentityProjection::zero(),
            prepared,
            prepared,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,
            CanonicalIdentityProjection::zero(),
            live,
            live,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT,
            CanonicalIdentityProjection::zero(),
            committed,
            committed,
        ),
        transition(
            IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,
            release,
            completed,
            completed,
        ),
        ProductionInFlightReservationTransitionProjection {
            action: 0xff,
            ..reserve
        },
        ProductionInFlightReservationTransitionProjection {
            requested_reservation_identity: foreign_reservation,
            ..reserve
        },
        ProductionInFlightReservationTransitionProjection {
            after: foreign_live,
            ..reserve
        },
        ProductionInFlightReservationTransitionProjection {
            before: in_flight_owner(IN_FLIGHT_RESERVATION_STATE_LIVE, reservation, release),
            ..reserve
        },
    ] {
        assert!(
            check_production_in_flight_reservation_transition(rejected).is_none(),
            "expected rejected primitive transition: {rejected:?}"
        );
    }
}

fn retirement_projection_accepts(
    decision_view: u64,
    decision_proposal_view: u64,
    receipt_view: u64,
    receipt_proposal_view: u64,
) -> bool {
    wal_retirement_authorized_body!(
        true,
        true,
        true,
        1u64,
        9u64,
        7u64,
        1u64,
        9u64,
        decision_view,
        9u64,
        decision_proposal_view,
        2u8,
        2u8,
        7u64,
        1u64,
        9u64,
        7u64,
        1u64,
        9u64,
        receipt_view,
        9u64,
        receipt_proposal_view,
        2u8,
        7u64,
    )
}

fn assert_strict_same_round_timeout_upgrade_kernel_boundaries() {
    let admitted = StrictSameRoundTimeoutUpgradeProjection {
        current_view: 6,
        timeout_view: 5,
        installed_same_round: true,
        selected_prepare_present: true,
        selected_prepare_view: 4,
        highest_prepare_present: true,
        highest_prepare_view: 3,
        locked_prepare_present: true,
        locked_prepare_view: 3,
    };
    assert!(strict_same_round_timeout_upgrade_is_allowed(admitted));

    assert!(!strict_same_round_timeout_upgrade_is_allowed(
        StrictSameRoundTimeoutUpgradeProjection {
            current_view: 7,
            ..admitted
        }
    ));
    assert!(!strict_same_round_timeout_upgrade_is_allowed(
        StrictSameRoundTimeoutUpgradeProjection {
            current_view: 0,
            timeout_view: u64::MAX,
            ..admitted
        }
    ));
    assert!(!strict_same_round_timeout_upgrade_is_allowed(
        StrictSameRoundTimeoutUpgradeProjection {
            installed_same_round: false,
            ..admitted
        }
    ));
    assert!(!strict_same_round_timeout_upgrade_is_allowed(
        StrictSameRoundTimeoutUpgradeProjection {
            selected_prepare_present: false,
            ..admitted
        }
    ));
    assert!(!strict_same_round_timeout_upgrade_is_allowed(
        StrictSameRoundTimeoutUpgradeProjection {
            highest_prepare_view: admitted.selected_prepare_view,
            ..admitted
        }
    ));
    assert!(!strict_same_round_timeout_upgrade_is_allowed(
        StrictSameRoundTimeoutUpgradeProjection {
            locked_prepare_view: admitted.selected_prepare_view,
            ..admitted
        }
    ));
    assert!(strict_same_round_timeout_upgrade_is_allowed(
        StrictSameRoundTimeoutUpgradeProjection {
            highest_prepare_present: false,
            locked_prepare_present: false,
            ..admitted
        }
    ));
}

#[test]
fn strict_same_round_refinement_kernels_reject_split_round_mutations() {
    assert_strict_same_round_timeout_upgrade_kernel_boundaries();

    let pending = PendingProjection {
        record_kind: WAL_RECORD_LOCK_AND_COMMIT,
        continuation: CONTINUATION_SIGN,
        persistence_id: 1,
        height: 9,
        view: 4,
        proposal_present: true,
        proposal_height: 9,
        proposal_view: 4,
        ..PendingProjection::default()
    };
    let boundary = BoundaryCapabilityKey {
        auxiliary_present: true,
        auxiliary_height: 9,
        auxiliary_view: 4,
        auxiliary_proposal_height: 9,
        auxiliary_proposal_view: 4,
        auxiliary_phase: 1,
        ..BoundaryCapabilityKey::none()
    };
    assert!(wal_record_proposal_round_is_exact_body!(
        WAL_RECORD_LOCK_AND_COMMIT,
        pending,
        boundary
    ));

    let split_lock = PendingProjection {
        proposal_view: 3,
        ..pending
    };
    let split_lock_boundary = BoundaryCapabilityKey {
        auxiliary_view: 3,
        auxiliary_proposal_view: 3,
        ..boundary
    };
    assert!(!wal_record_proposal_round_is_exact_body!(
        WAL_RECORD_LOCK_AND_COMMIT,
        split_lock,
        split_lock_boundary
    ));

    let decision = PendingProjection {
        record_kind: WAL_RECORD_DECISION,
        continuation: CONTINUATION_DECIDE,
        ..pending
    };
    assert!(wal_record_proposal_round_is_exact_body!(
        WAL_RECORD_DECISION,
        decision,
        BoundaryCapabilityKey::none()
    ));
    assert!(!wal_record_proposal_round_is_exact_body!(
        WAL_RECORD_DECISION,
        PendingProjection {
            proposal_view: 3,
            ..decision
        },
        BoundaryCapabilityKey::none()
    ));
}

#[test]
fn wal_retirement_authorization_rejects_split_round_decision_and_receipt() {
    assert!(retirement_projection_accepts(4, 4, 4, 4));
    assert!(!retirement_projection_accepts(4, 3, 4, 3));
    assert!(!retirement_projection_accepts(4, 4, 5, 5));
}

#[test]
fn semantic_commit_decision_identity_ignores_only_qc_rounds() {
    let context = successor_identity(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        0x11,
    );
    let subject = successor_identity(
        IDENTITY_DOMAIN_SUBJECT,
        IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
        0x21,
    );
    let old_round = ProductionDecisionIdentityProjection {
        context_id: context,
        height: 9,
        view: 2,
        proposal_height: 9,
        proposal_view: 2,
        phase: 2,
        subject,
        block_hash: successor_identity(IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_BLOCK_HEADER, 0x31),
        payload_hash: successor_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CANONICAL_PAYLOAD,
            0x41,
        ),
        execution_commitment: successor_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTION_COMMITMENT,
            0x51,
        ),
        executed_block_wire_hash: successor_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
            0x61,
        ),
    };
    let later_reproposal = ProductionDecisionIdentityProjection {
        view: 5,
        proposal_view: 5,
        ..old_round
    };
    assert!(production_decision_identity_is_canonical_body!(old_round));
    assert!(production_decision_identity_is_canonical_body!(
        later_reproposal
    ));
    assert!(production_decision_identity_equal_body!(
        old_round,
        later_reproposal
    ));

    let split_round = ProductionDecisionIdentityProjection {
        proposal_view: 4,
        ..later_reproposal
    };
    assert!(!production_decision_identity_is_canonical_body!(
        split_round
    ));
    assert!(!production_decision_identity_equal_body!(
        old_round,
        split_round
    ));

    let altered_subject = ProductionDecisionIdentityProjection {
        subject: successor_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
            0x22,
        ),
        ..later_reproposal
    };
    assert!(!production_decision_identity_equal_body!(
        old_round,
        altered_subject
    ));

    let altered_execution = ProductionDecisionIdentityProjection {
        execution_commitment: successor_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_EXECUTION_COMMITMENT,
            0x52,
        ),
        ..later_reproposal
    };
    assert!(!production_decision_identity_equal_body!(
        old_round,
        altered_execution
    ));
}

fn durable_predecessor(byte: u8) -> ProductionDurablePredecessorIdentityProjection {
    ProductionDurablePredecessorIdentityProjection {
        height: 7,
        block_hash: successor_identity(IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_BLOCK_HEADER, byte),
        artifact_hash: successor_identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_FINALITY_ARTIFACT,
            byte.wrapping_add(1),
        ),
    }
}

fn successor_snapshot(
    parent_height: u64,
    context_byte: u8,
) -> ProductionSuccessorSnapshotProjection {
    let context = successor_identity(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        context_byte,
    );
    ProductionSuccessorSnapshotProjection {
        expected_context_id: context,
        published_context_id: context,
        height: parent_height + 1,
        last_committed_height: parent_height,
        view: 3,
        generation: 19,
        marker_context_id: context,
        marker_height: parent_height + 1,
        marker_view: 3,
        marker_generation: 19,
        marker_kind: SUCCESSOR_MARKER_ACTIVATED,
        marker_age_ms: 0,
    }
}

#[test]
fn applied_successor_kernel_rejects_foreign_same_height_authority_and_status_mutations() {
    let predecessor = durable_predecessor(0x21);
    let successor = successor_snapshot(predecessor.height, 0x31);
    let trace = ProductionAppliedSuccessorTraceProjection {
        authority_kind: SUCCESSOR_AUTHORITY_APPLIED,
        binding: ProductionSuccessorPredecessorBindingProjection {
            expected_predecessor: predecessor,
            authority_predecessor: predecessor,
            successor_context_id: successor.expected_context_id,
        },
        predecessor_status_height: predecessor.height,
        predecessor_stage_before: SUCCESSOR_STAGE_RUNNING,
        predecessor_stage_after: SUCCESSOR_STAGE_COMPLETE,
        successor,
    };
    assert!(production_applied_successor_trace_refines_indexed_activation_kernel(trace));
    assert_eq!(
        check_production_applied_successor_transition(trace)
            .expect("valid applied-successor transition must mint evidence")
            .into_projection(),
        trace
    );
    assert_eq!(trace.predecessor_stage_before, SUCCESSOR_STAGE_RUNNING);
    assert_eq!(trace.predecessor_stage_after, SUCCESSOR_STAGE_COMPLETE);
    assert_eq!(
        trace.successor.height,
        trace.binding.expected_predecessor.height + 1
    );
    assert_eq!(trace.successor.marker_height, trace.successor.height);
    assert!(production_successor_predecessor_binding_kernel(
        trace.binding
    ));

    let mut foreign_block = trace;
    foreign_block.binding.authority_predecessor.block_hash.word0 ^= 1;
    assert!(check_production_applied_successor_transition(foreign_block).is_none());
    assert!(!production_successor_predecessor_binding_kernel(
        foreign_block.binding
    ));
    assert!(!production_applied_successor_trace_refines_indexed_activation_kernel(foreign_block));

    let mut foreign_artifact = trace;
    foreign_artifact
        .binding
        .authority_predecessor
        .artifact_hash
        .word3 ^= 1;
    assert!(!production_successor_predecessor_binding_kernel(
        foreign_artifact.binding
    ));

    let mut reset_rank = trace;
    reset_rank.predecessor_stage_before = SUCCESSOR_STAGE_QUEUED;
    assert!(!production_applied_successor_trace_refines_indexed_activation_kernel(reset_rank));

    let mut retargeted = trace;
    retargeted.successor.published_context_id.word2 ^= 1;
    assert!(!production_applied_successor_trace_refines_indexed_activation_kernel(retargeted));

    let mut wrong_parent = trace;
    wrong_parent.successor.last_committed_height -= 1;
    assert!(!production_applied_successor_trace_refines_indexed_activation_kernel(wrong_parent));
}

#[test]
fn two_stage_relay_retry_kernel_rejects_source_rotation_eligibility_and_fifo_mutations() {
    let trace = ProductionTwoStageRelayRetryTraceProjection {
        daemon_source_capacity_matches_two_upstream_lanes: true,
        class_corridor_covers_authenticated_sources: true,
        authenticated_source_matches_resource_owner: true,
        retry_route_same_delivery: true,
        retry_route_active: true,
        selected_eligible: true,
        ready_sources_before: 2,
        selected_source_rank_before: 0,
        ready_sources_after: 2,
        selected_source_rank_after: 1,
        source_depth_before: 2,
        selected_item_rank_before: 0,
        source_depth_after: 2,
        selected_item_rank_after: 1,
        total_depth_before: 3,
        total_depth_after: 3,
        source_capacity: 4,
        total_capacity: 8,
    };
    assert!(production_two_stage_relay_retry_trace_refines_source_fairness_kernel(trace));
    assert_eq!(
        check_production_two_stage_relay_retry_transition(trace)
            .expect("valid relay retry must mint evidence")
            .into_projection(),
        trace
    );
    assert!(trace.daemon_source_capacity_matches_two_upstream_lanes);
    assert!(trace.class_corridor_covers_authenticated_sources);
    assert_eq!(trace.total_depth_after, trace.total_depth_before);
    assert_eq!(
        trace.selected_source_rank_after,
        trace.ready_sources_after - 1
    );
    assert_eq!(trace.selected_item_rank_after, trace.source_depth_after - 1);
    assert!(trace.source_depth_after <= trace.source_capacity);

    for mutant in [
        ProductionTwoStageRelayRetryTraceProjection {
            daemon_source_capacity_matches_two_upstream_lanes: false,
            ..trace
        },
        ProductionTwoStageRelayRetryTraceProjection {
            class_corridor_covers_authenticated_sources: false,
            ..trace
        },
        ProductionTwoStageRelayRetryTraceProjection {
            authenticated_source_matches_resource_owner: false,
            ..trace
        },
        ProductionTwoStageRelayRetryTraceProjection {
            selected_source_rank_after: 0,
            ..trace
        },
        ProductionTwoStageRelayRetryTraceProjection {
            selected_eligible: false,
            ..trace
        },
        ProductionTwoStageRelayRetryTraceProjection {
            selected_item_rank_after: 0,
            ..trace
        },
    ] {
        assert!(!production_two_stage_relay_retry_trace_refines_source_fairness_kernel(mutant));
        assert!(check_production_two_stage_relay_retry_transition(mutant).is_none());
    }
}

#[test]
fn recovered_successor_kernel_keeps_complete_tip_and_snapshot_authority_disjoint() {
    let predecessor = durable_predecessor(0x41);
    let successor = successor_snapshot(predecessor.height, 0x51);
    let complete_tip = ProductionRecoveredSuccessorTraceProjection {
        authority_kind: SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP,
        predecessor,
        snapshot_record_hash: CanonicalIdentityProjection::zero(),
        snapshot_height: 0,
        snapshot_block_hash: CanonicalIdentityProjection::zero(),
        authority_context_id: successor.expected_context_id,
        published_status_height_before: 0,
        successor,
    };
    assert!(production_recovered_successor_trace_refines_indexed_activation_kernel(complete_tip));
    assert_eq!(
        check_production_recovered_successor_transition(complete_tip)
            .expect("valid complete-tip recovery must mint evidence")
            .into_projection(),
        complete_tip
    );
    assert_eq!(complete_tip.published_status_height_before, 0);
    assert_eq!(
        complete_tip.successor.height,
        complete_tip.successor.last_committed_height + 1
    );

    let snapshot = ProductionRecoveredSuccessorTraceProjection {
        authority_kind: SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
        predecessor: ProductionDurablePredecessorIdentityProjection::default(),
        snapshot_record_hash: successor_identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD,
            0x61,
        ),
        snapshot_height: predecessor.height,
        snapshot_block_hash: successor_identity(
            IDENTITY_DOMAIN_SUBJECT,
            IDENTITY_KIND_BLOCK_HEADER,
            0x62,
        ),
        authority_context_id: successor.expected_context_id,
        published_status_height_before: 0,
        successor,
    };
    assert!(production_recovered_successor_trace_refines_indexed_activation_kernel(snapshot));
    assert_eq!(snapshot.published_status_height_before, 0);
    assert_eq!(
        snapshot.snapshot_height,
        snapshot.successor.last_committed_height
    );

    let mut snapshot_as_tip = snapshot;
    snapshot_as_tip.authority_kind = SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP;
    assert!(
        !production_recovered_successor_trace_refines_indexed_activation_kernel(snapshot_as_tip)
    );

    let mut tip_as_snapshot = complete_tip;
    tip_as_snapshot.authority_kind = SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP;
    assert!(
        !production_recovered_successor_trace_refines_indexed_activation_kernel(tip_as_snapshot)
    );

    let mut occupied_registry = complete_tip;
    occupied_registry.published_status_height_before = predecessor.height;
    assert!(check_production_recovered_successor_transition(occupied_registry).is_none());
    assert!(
        !production_recovered_successor_trace_refines_indexed_activation_kernel(occupied_registry)
    );

    let mut stale_snapshot_anchor = snapshot;
    stale_snapshot_anchor.snapshot_height -= 1;
    assert!(
        !production_recovered_successor_trace_refines_indexed_activation_kernel(
            stale_snapshot_anchor
        )
    );
}

#[test]
fn successor_startup_lifecycle_preserves_running_on_failure_and_separates_restart_sources() {
    let begin = ProductionSuccessorStartupLifecycleProjection {
        transition_kind: SUCCESSOR_LIFECYCLE_BEGIN,
        authority_kind: SUCCESSOR_AUTHORITY_APPLIED,
        status_height: 7,
        stage_before: SUCCESSOR_STAGE_QUEUED,
        stage_after: SUCCESSOR_STAGE_RUNNING,
        published_height_before: 7,
        published_height_after: 7,
        restart_required_before: false,
        restart_required_after: false,
    };
    assert!(production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(begin));
    assert_eq!(
        check_production_successor_startup_lifecycle_transition(begin)
            .expect("valid startup transition must mint evidence")
            .into_projection(),
        begin
    );
    assert_eq!(begin.published_height_after, begin.published_height_before);
    assert!(!begin.restart_required_after);

    let failure = ProductionSuccessorStartupLifecycleProjection {
        transition_kind: SUCCESSOR_LIFECYCLE_FAIL,
        authority_kind: SUCCESSOR_AUTHORITY_APPLIED,
        stage_before: SUCCESSOR_STAGE_RUNNING,
        stage_after: SUCCESSOR_STAGE_RUNNING,
        restart_required_after: true,
        ..begin
    };
    assert!(production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(failure));
    assert_eq!(failure.stage_after, failure.stage_before);
    assert!(failure.restart_required_after);

    let mut fabricated_completion = failure;
    fabricated_completion.stage_after = SUCCESSOR_STAGE_COMPLETE;
    assert!(
        check_production_successor_startup_lifecycle_transition(fabricated_completion).is_none()
    );
    assert!(
        !production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(
            fabricated_completion
        )
    );

    let recovered_retry = ProductionSuccessorStartupLifecycleProjection {
        transition_kind: SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP,
        authority_kind: SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP,
        status_height: 8,
        stage_before: SUCCESSOR_STAGE_NONE,
        stage_after: SUCCESSOR_STAGE_NONE,
        published_height_before: 0,
        published_height_after: 0,
        restart_required_before: false,
        restart_required_after: false,
    };
    assert!(
        production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(recovered_retry)
    );

    let mut snapshot_as_retry = recovered_retry;
    snapshot_as_retry.authority_kind = SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP;
    assert!(
        !production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(snapshot_as_retry)
    );

    let snapshot_bootstrap = ProductionSuccessorStartupLifecycleProjection {
        transition_kind: SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP,
        authority_kind: SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
        ..recovered_retry
    };
    assert!(
        production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(snapshot_bootstrap)
    );
}

#[test]
fn historical_certificate_kernel_rejects_foreign_admission_and_unretired_request() {
    let context = successor_identity(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        0x71,
    );
    let request = successor_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST,
        0x72,
    );
    let certificate = successor_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_QUORUM_CERTIFICATE,
        0x73,
    );
    let message = successor_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_CONSENSUS_MESSAGE,
        0x74,
    );
    let trace = ProductionHistoricalCertificateTraceProjection {
        context_id: context,
        context_height: 9,
        certificate_context_id: context,
        certificate_height: 9,
        request_hash: request,
        response_request_hash: request,
        response_certificate: certificate,
        message_certificate: certificate,
        message_hash: message,
        admitted_message_hash: message,
        request_present_before: true,
        request_present_after: false,
    };
    assert!(production_historical_certificate_trace_refines_indexed_async_kernel(trace));
    assert_eq!(
        check_production_historical_certificate_transition(trace)
            .expect("valid historical certificate handoff must mint evidence")
            .into_projection(),
        trace
    );
    assert_eq!(trace.certificate_height, trace.context_height);
    assert!(trace.request_present_before);
    assert!(!trace.request_present_after);
    assert_eq!(trace.message_hash, trace.admitted_message_hash);

    let mut foreign_admission = trace;
    foreign_admission.admitted_message_hash.word1 ^= 1;
    assert!(check_production_historical_certificate_transition(foreign_admission).is_none());
    assert!(
        !production_historical_certificate_trace_refines_indexed_async_kernel(foreign_admission)
    );

    let mut unretired = trace;
    unretired.request_present_after = true;
    assert!(!production_historical_certificate_trace_refines_indexed_async_kernel(unretired));
}

#[test]
fn historical_body_pipeline_kernel_rejects_request_subject_and_owner_substitution() {
    let context = successor_identity(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        0x81,
    );
    let request = successor_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_CERTIFIED_BODY_REQUEST,
        0x82,
    );
    let subject = successor_identity(
        IDENTITY_DOMAIN_SUBJECT,
        IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
        0x83,
    );
    let manifest = successor_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_PAYLOAD_MANIFEST,
        0x84,
    );
    let payload = successor_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_CANONICAL_PAYLOAD,
        0x85,
    );
    let tag = TagProjection {
        height: 12,
        view: 6,
        generation: 4,
    };
    let trace = ProductionHistoricalBodyPipelineTraceProjection {
        context_id: context,
        context_height: 12,
        request_hash: request,
        pending_request_hash: request,
        authenticated_request_hash: request,
        fetch_tag: tag,
        round_context_id: context,
        round_height: 12,
        round_view: 5,
        subject,
        manifest_round_context_id: context,
        manifest_round_height: 12,
        manifest_round_view: 5,
        manifest_subject: subject,
        response_manifest: manifest,
        ready_manifest: manifest,
        subject_payload_hash: payload,
        body_payload_hash: payload,
        owner_present_after: true,
        owner_tag: tag,
        owner_round_context_id: context,
        owner_round_height: 12,
        owner_round_view: 5,
        owner_subject: subject,
        pending_fetch_present_after: false,
        request_present_after: false,
    };
    assert!(production_historical_body_pipeline_trace_refines_indexed_async_kernel(trace));
    assert_eq!(
        check_production_historical_body_pipeline_transition(trace)
            .expect("valid historical body handoff must mint evidence")
            .into_projection(),
        trace
    );
    assert!(trace.owner_present_after);
    assert_eq!(trace.owner_tag, trace.fetch_tag);
    assert!(!trace.pending_fetch_present_after);
    assert!(!trace.request_present_after);

    let mut replayed_request = trace;
    replayed_request.request_present_after = true;
    assert!(check_production_historical_body_pipeline_transition(replayed_request).is_none());
    assert!(
        !production_historical_body_pipeline_trace_refines_indexed_async_kernel(replayed_request)
    );

    let mut retargeted_subject = trace;
    retargeted_subject.manifest_subject.word2 ^= 1;
    assert!(
        !production_historical_body_pipeline_trace_refines_indexed_async_kernel(retargeted_subject)
    );

    let mut foreign_owner = trace;
    foreign_owner.owner_tag.generation += 1;
    assert!(!production_historical_body_pipeline_trace_refines_indexed_async_kernel(foreign_owner));
}

fn progress_identity(byte: u64) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection {
        domain: 1,
        kind: 1,
        word0: byte,
        word1: byte,
        word2: byte,
        word3: byte,
    }
}

fn durable_timeout_progress_witness() -> LockedCommitProgressWitnessProjection {
    LockedCommitProgressWitnessProjection {
        context_id: progress_identity(1),
        current_height: 7,
        current_view: 3,
        local_validator_present: true,
        local_validator: ValidatorId::repeat(4),
        locked_context_id: progress_identity(1),
        locked_height: 7,
        locked_view: 1,
        locked_subject: progress_identity(2),
        timeout_intent_present: true,
        timeout_intent_durable: true,
        timeout_context_id: progress_identity(1),
        timeout_height: 7,
        timeout_view: 3,
        timeout_signer: ValidatorId::repeat(4),
        ..LockedCommitProgressWitnessProjection::default()
    }
}

fn durable_reproposal_progress_witness() -> LockedCommitProgressWitnessProjection {
    LockedCommitProgressWitnessProjection {
        context_id: progress_identity(1),
        current_height: 7,
        current_view: 3,
        local_validator_present: true,
        local_validator: ValidatorId::repeat(4),
        locked_context_id: progress_identity(1),
        locked_height: 7,
        locked_view: 1,
        locked_subject: progress_identity(2),
        installed_timeout_present: true,
        installed_timeout_durable: true,
        installed_timeout_context_id: progress_identity(1),
        installed_timeout_height: 7,
        installed_timeout_view: 2,
        ..LockedCommitProgressWitnessProjection::default()
    }
}

#[test]
fn locked_commit_progress_witness_accepts_exact_owners_and_rejects_mutations() {
    let timeout = durable_timeout_progress_witness();
    assert!(locked_commit_progress_witness_is_valid(timeout));

    let mut stale_timeout = timeout;
    stale_timeout.timeout_view -= 1;
    assert!(!locked_commit_progress_witness_is_valid(stale_timeout));

    let mut wrong_timeout = timeout;
    wrong_timeout.timeout_signer = ValidatorId::repeat(5);
    assert!(!locked_commit_progress_witness_is_valid(wrong_timeout));

    let mut wrong_lock_context = timeout;
    wrong_lock_context.locked_context_id = progress_identity(9);
    assert!(!locked_commit_progress_witness_is_valid(wrong_lock_context));

    let mut wrong_lock_height = timeout;
    wrong_lock_height.locked_height += 1;
    assert!(!locked_commit_progress_witness_is_valid(wrong_lock_height));

    let mut volatile_timeout = timeout;
    volatile_timeout.timeout_intent_durable = false;
    assert!(!locked_commit_progress_witness_is_valid(volatile_timeout));

    let reproposal = durable_reproposal_progress_witness();
    assert!(locked_commit_progress_witness_is_valid(reproposal));

    let mut absent_reproposal = reproposal;
    absent_reproposal.installed_timeout_present = false;
    assert!(!locked_commit_progress_witness_is_valid(absent_reproposal));

    let mut volatile_reproposal = reproposal;
    volatile_reproposal.installed_timeout_durable = false;
    assert!(!locked_commit_progress_witness_is_valid(
        volatile_reproposal
    ));

    let mut foreign_reproposal_context = reproposal;
    foreign_reproposal_context.installed_timeout_context_id = progress_identity(9);
    assert!(!locked_commit_progress_witness_is_valid(
        foreign_reproposal_context
    ));

    let mut foreign_reproposal_height = reproposal;
    foreign_reproposal_height.installed_timeout_height += 1;
    assert!(!locked_commit_progress_witness_is_valid(
        foreign_reproposal_height
    ));

    let mut stale_reproposal = reproposal;
    stale_reproposal.installed_timeout_view -= 1;
    assert!(!locked_commit_progress_witness_is_valid(stale_reproposal));

    let mut nonhistorical_reproposal = reproposal;
    nonhistorical_reproposal.locked_view = nonhistorical_reproposal.current_view;
    assert!(!locked_commit_progress_witness_is_valid(
        nonhistorical_reproposal
    ));

    let mut pending = timeout;
    pending.timeout_intent_present = false;
    pending.timeout_intent_durable = false;
    pending.current_view = pending.locked_view;
    pending.pending = PendingProjection {
        record_kind: WAL_RECORD_LOCK_AND_COMMIT,
        continuation: CONTINUATION_SIGN,
        persistence_id: 9,
        context_id: pending.context_id,
        height: pending.current_height,
        view: pending.current_view,
        proposal_present: true,
        proposal_height: pending.locked_height,
        proposal_view: pending.locked_view,
        subject: pending.locked_subject,
    };
    assert!(locked_commit_progress_witness_is_valid(pending));

    let mut closed_origin_pending = pending;
    closed_origin_pending.current_view += 1;
    closed_origin_pending.pending.view = closed_origin_pending.current_view;
    assert!(!locked_commit_progress_witness_is_valid(
        closed_origin_pending
    ));

    let mut nonexact_pending = pending;
    nonexact_pending.pending.proposal_view += 1;
    assert!(!locked_commit_progress_witness_is_valid(nonexact_pending));

    let mut foreign_height_pending = pending;
    foreign_height_pending.locked_height += 1;
    foreign_height_pending.pending.proposal_height = foreign_height_pending.locked_height;
    assert!(!locked_commit_progress_witness_is_valid(
        foreign_height_pending
    ));

    let mut commit = timeout;
    commit.timeout_intent_present = false;
    commit.timeout_intent_durable = false;
    commit.current_view = commit.locked_view;
    commit.commit_intent_present = true;
    commit.commit_context_id = commit.context_id;
    commit.commit_height = commit.current_height;
    commit.commit_view = commit.current_view;
    commit.commit_proposal_height = commit.locked_height;
    commit.commit_proposal_view = commit.locked_view;
    commit.commit_phase = 2;
    commit.commit_subject = commit.locked_subject;
    commit.commit_signer = commit.local_validator;
    commit.commit_signature_pending = true;
    assert!(locked_commit_progress_witness_is_valid(commit));

    let mut closed_origin_commit = commit;
    closed_origin_commit.current_view += 1;
    closed_origin_commit.commit_view = closed_origin_commit.current_view;
    assert!(!locked_commit_progress_witness_is_valid(
        closed_origin_commit
    ));

    let mut nonexact_commit = commit;
    nonexact_commit.commit_subject.word0 ^= 1;
    assert!(!locked_commit_progress_witness_is_valid(nonexact_commit));

    let mut foreign_height_commit = commit;
    foreign_height_commit.locked_height += 1;
    foreign_height_commit.commit_proposal_height = foreign_height_commit.locked_height;
    assert!(!locked_commit_progress_witness_is_valid(
        foreign_height_commit
    ));
}

#[test]
fn durable_intent_refinement_accepts_exact_stutters_and_rejects_mutations() {
    let durable_intent = durable_begin_trace();
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(durable_intent));
    assert!(durable_intent.durable_sequence_after >= durable_intent.durable_sequence_before);
    assert!(effect_count(durable_intent.effects, EFFECT_PERSIST) <= 1);

    let mut wrong_event = durable_intent;
    wrong_event.event_kind = EVENT_PERSISTED;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_event));

    let mut wrong_event_tag = durable_intent;
    wrong_event_tag.event_tag.generation += 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_event_tag));

    let mut wrong_context = durable_intent;
    wrong_context.pending_after.context_id.word3 ^= 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_context));

    let mut wrong_subject = durable_intent;
    wrong_subject.pending_after.subject.word0 ^= 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_subject));

    let mut wrong_continuation = durable_intent;
    wrong_continuation.boundary_granted.continuation = CONTINUATION_NONE;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_continuation));

    let mut wrong_wal_id = durable_intent;
    wrong_wal_id.boundary_granted.persistence_id += 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_wal_id));

    let mut consistently_skipped_wal_id = durable_intent;
    consistently_skipped_wal_id.pending_after.persistence_id += 1;
    consistently_skipped_wal_id.boundary_claimed.persistence_id += 1;
    consistently_skipped_wal_id.boundary_granted.persistence_id += 1;
    consistently_skipped_wal_id
        .effects
        .slot0
        .requested
        .persistence_id += 1;
    consistently_skipped_wal_id
        .effects
        .slot0
        .granted
        .persistence_id += 1;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(
            consistently_skipped_wal_id
        ),
        "a mutually consistent projection must not skip the next durable WAL id"
    );

    let mut wrong_effect = durable_intent;
    wrong_effect.effects.slot0.granted.persistence_id += 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_effect));

    let mut timeout_with_high_qc = durable_intent;
    timeout_with_high_qc.event_kind = 5;
    timeout_with_high_qc.boundary_claimed.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
    timeout_with_high_qc.boundary_claimed.continuation = CONTINUATION_INSTALL_TIMEOUT;
    timeout_with_high_qc.boundary_claimed.proposal_present = false;
    timeout_with_high_qc.boundary_claimed.proposal_height = 0;
    timeout_with_high_qc.boundary_claimed.proposal_view = 0;
    timeout_with_high_qc.boundary_granted = timeout_with_high_qc.boundary_claimed;
    timeout_with_high_qc.pending_after.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
    timeout_with_high_qc.pending_after.continuation = CONTINUATION_INSTALL_TIMEOUT;
    timeout_with_high_qc.pending_after.view += 4;
    timeout_with_high_qc.pending_after.proposal_present = false;
    timeout_with_high_qc.pending_after.proposal_height = 0;
    timeout_with_high_qc.pending_after.proposal_view = 0;
    timeout_with_high_qc.boundary_claimed.auxiliary_present = true;
    timeout_with_high_qc.boundary_claimed.auxiliary_context_id =
        timeout_with_high_qc.boundary_claimed.context_id;
    timeout_with_high_qc.boundary_claimed.auxiliary_height =
        timeout_with_high_qc.owner_tag_before.height;
    timeout_with_high_qc.boundary_claimed.auxiliary_view =
        timeout_with_high_qc.owner_tag_before.view;
    timeout_with_high_qc
        .boundary_claimed
        .auxiliary_proposal_height = timeout_with_high_qc.owner_tag_before.height;
    timeout_with_high_qc
        .boundary_claimed
        .auxiliary_proposal_view = timeout_with_high_qc.owner_tag_before.view;
    timeout_with_high_qc.boundary_claimed.auxiliary_phase = 1;
    timeout_with_high_qc.boundary_claimed.auxiliary_subject =
        timeout_with_high_qc.boundary_claimed.subject.subject;
    timeout_with_high_qc.boundary_granted = timeout_with_high_qc.boundary_claimed;
    timeout_with_high_qc.effects.slot0.requested.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
    timeout_with_high_qc.effects.slot0.requested.view = timeout_with_high_qc.pending_after.view;
    timeout_with_high_qc.effects.slot0.requested.proposal_height = 0;
    timeout_with_high_qc.effects.slot0.requested.proposal_view = 0;
    timeout_with_high_qc.effects.slot0.requested.subject =
        timeout_with_high_qc.boundary_claimed.subject.subject;
    timeout_with_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_present = true;
    timeout_with_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_context_id = timeout_with_high_qc.boundary_claimed.auxiliary_context_id;
    timeout_with_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_height = timeout_with_high_qc.boundary_claimed.auxiliary_height;
    timeout_with_high_qc.effects.slot0.requested.auxiliary_view =
        timeout_with_high_qc.boundary_claimed.auxiliary_view;
    timeout_with_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_proposal_height = timeout_with_high_qc
        .boundary_claimed
        .auxiliary_proposal_height;
    timeout_with_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_proposal_view = timeout_with_high_qc
        .boundary_claimed
        .auxiliary_proposal_view;
    timeout_with_high_qc.effects.slot0.requested.auxiliary_phase =
        timeout_with_high_qc.boundary_claimed.auxiliary_phase;
    timeout_with_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_subject = timeout_with_high_qc.boundary_claimed.subject.subject;
    timeout_with_high_qc.effects.slot0.granted = timeout_with_high_qc.effects.slot0.requested;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(timeout_with_high_qc));

    let mut wrong_timeout_high_qc = timeout_with_high_qc;
    wrong_timeout_high_qc.effects.slot0.requested.subject = Subject::repeat(9);
    wrong_timeout_high_qc.effects.slot0.granted.subject = Subject::repeat(9);
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(wrong_timeout_high_qc)
    );

    let mut substituted_timeout_evidence = timeout_with_high_qc;
    substituted_timeout_evidence
        .effects
        .slot0
        .requested
        .auxiliary_subject = Subject::repeat(9);
    substituted_timeout_evidence
        .effects
        .slot0
        .granted
        .auxiliary_subject = Subject::repeat(9);
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(
            substituted_timeout_evidence
        )
    );

    let mut timeout_without_high_qc = timeout_with_high_qc;
    let absent_subject = Subject::default();
    let absent_subject_identity = CanonicalIdentityProjection::from_bytes(
        IDENTITY_DOMAIN_SUBJECT,
        IDENTITY_KIND_CONSENSUS_SUBJECT,
        *absent_subject.as_bytes(),
    );
    timeout_without_high_qc.boundary_claimed.subject.subject = absent_subject;
    timeout_without_high_qc.boundary_claimed.subject_identity = absent_subject_identity;
    timeout_without_high_qc.boundary_claimed.auxiliary_present = false;
    timeout_without_high_qc
        .boundary_claimed
        .auxiliary_context_id = ContextId::repeat(0);
    timeout_without_high_qc.boundary_claimed.auxiliary_height = 0;
    timeout_without_high_qc.boundary_claimed.auxiliary_view = 0;
    timeout_without_high_qc
        .boundary_claimed
        .auxiliary_proposal_height = 0;
    timeout_without_high_qc
        .boundary_claimed
        .auxiliary_proposal_view = 0;
    timeout_without_high_qc.boundary_claimed.auxiliary_phase = 0;
    timeout_without_high_qc.boundary_claimed.auxiliary_subject = Subject::repeat(0);
    timeout_without_high_qc.boundary_granted = timeout_without_high_qc.boundary_claimed;
    timeout_without_high_qc.pending_after.subject = absent_subject_identity;
    timeout_without_high_qc.effects.slot0.requested.subject = absent_subject;
    timeout_without_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_present = false;
    timeout_without_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_context_id = ContextId::repeat(0);
    timeout_without_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_height = 0;
    timeout_without_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_view = 0;
    timeout_without_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_proposal_height = 0;
    timeout_without_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_proposal_view = 0;
    timeout_without_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_phase = 0;
    timeout_without_high_qc
        .effects
        .slot0
        .requested
        .auxiliary_subject = Subject::repeat(0);
    timeout_without_high_qc.effects.slot0.granted = timeout_without_high_qc.effects.slot0.requested;
    assert!(
        production_durable_intent_trace_refines_progress_witness_kernel(timeout_without_high_qc)
    );

    let mut regressive_timeout = timeout_with_high_qc;
    // The immediately preceding timeout round may carry a strict
    // higher-PrepareQC upgrade; two rounds behind is genuinely stale.
    regressive_timeout.pending_after.view =
        regressive_timeout.owner_tag_before.view.saturating_sub(2);
    regressive_timeout.effects.slot0.requested.view = regressive_timeout.pending_after.view;
    regressive_timeout.effects.slot0.granted.view = regressive_timeout.pending_after.view;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(regressive_timeout));

    let mut overflowing_timeout = timeout_with_high_qc;
    overflowing_timeout.pending_after.view = u64::MAX;
    overflowing_timeout.effects.slot0.requested.view = u64::MAX;
    overflowing_timeout.effects.slot0.granted.view = u64::MAX;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(overflowing_timeout));

    let mut wrong_record_height = durable_intent;
    wrong_record_height.pending_after.height += 1;
    wrong_record_height.effects.slot0.requested.height += 1;
    wrong_record_height.effects.slot0.granted.height += 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_record_height));

    let mut stale_generation = durable_intent;
    stale_generation.event_tag.generation += 1;
    stale_generation.pending_after = stale_generation.pending_before;
    stale_generation.boundary_claimed = BoundaryCapabilityKey::none();
    stale_generation.boundary_granted = BoundaryCapabilityKey::none();
    stale_generation.effects = EffectTrace::empty();
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_generation));

    let mut stale_height = stale_generation;
    stale_height.event_tag = stale_height.owner_tag_before;
    stale_height.event_tag.height += 1;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_height));

    let mut stale_view = stale_generation;
    stale_view.event_tag = stale_view.owner_tag_before;
    stale_view.event_tag.view += 1;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_view));

    let mut stale_while_pending = stale_generation;
    stale_while_pending.pending_before = durable_intent.pending_after;
    stale_while_pending.pending_after = durable_intent.pending_after;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_while_pending));

    let mut stale_owner_mutation = stale_generation;
    stale_owner_mutation.owner_tag_after.generation += 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(stale_owner_mutation));

    let mut stale_pending_mutation = stale_generation;
    stale_pending_mutation.pending_after.persistence_id += 1;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(stale_pending_mutation)
    );

    let mut stale_sequence_mutation = stale_generation;
    stale_sequence_mutation.durable_sequence_after += 1;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(stale_sequence_mutation)
    );

    let mut stale_boundary = stale_generation;
    stale_boundary.boundary_claimed = durable_intent.boundary_claimed;
    stale_boundary.boundary_granted = durable_intent.boundary_granted;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(stale_boundary));

    let mut stale_effect = stale_generation;
    assert!(push_authorized(&mut stale_effect.effects, EFFECT_REPORT));
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(stale_effect));

    let mut stale_non_completion_id = stale_generation;
    stale_non_completion_id.event_persistence_id = 91;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(stale_non_completion_id)
    );

    let mut stale_persisted = stale_generation;
    stale_persisted.event_kind = EVENT_PERSISTED;
    stale_persisted.event_persistence_id = 91;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_persisted));

    let mut stale_persistence_failed = stale_persisted;
    stale_persistence_failed.event_kind = EVENT_PERSISTENCE_FAILED;
    assert!(
        production_durable_intent_trace_refines_progress_witness_kernel(stale_persistence_failed)
    );

    let mut unmatched_persisted = stale_persisted;
    unmatched_persisted.event_tag = unmatched_persisted.owner_tag_before;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(unmatched_persisted));

    let mut unmatched_persistence_failed = unmatched_persisted;
    unmatched_persistence_failed.event_kind = EVENT_PERSISTENCE_FAILED;
    assert!(
        production_durable_intent_trace_refines_progress_witness_kernel(
            unmatched_persistence_failed
        )
    );

    let mut completion_with_effect = unmatched_persisted;
    assert!(push_authorized(
        &mut completion_with_effect.effects,
        EFFECT_REPORT
    ));
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(completion_with_effect)
    );

    let mut completion_while_pending = unmatched_persisted;
    completion_while_pending.pending_before = durable_intent.pending_after;
    completion_while_pending.pending_after = durable_intent.pending_after;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(completion_while_pending)
    );

    let mut matching_non_completion_id = unmatched_persisted;
    matching_non_completion_id.event_kind = 0;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(
            matching_non_completion_id
        )
    );
}

#[test]
fn lock_and_commit_requires_one_current_vote_and_proposal_round() {
    let begin = lock_and_commit_begin_trace();
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(begin));
    assert_eq!(
        check_production_durable_intent_transition(begin)
            .expect("valid durable intent must mint evidence")
            .into_projection(),
        begin
    );

    let mut split_round = begin;
    split_round.pending_after.proposal_view -= 1;
    split_round.boundary_claimed.proposal_view -= 1;
    split_round.boundary_claimed.auxiliary_view -= 1;
    split_round.boundary_claimed.auxiliary_proposal_view -= 1;
    split_round.boundary_granted = split_round.boundary_claimed;
    split_round.effects.slot0.requested.proposal_view -= 1;
    split_round.effects.slot0.requested.auxiliary_view -= 1;
    split_round.effects.slot0.requested.auxiliary_proposal_view -= 1;
    split_round.effects.slot0.granted = split_round.effects.slot0.requested;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(split_round),
        "a new Commit cannot combine the current vote round with an older proposal round"
    );
    assert!(check_production_durable_intent_transition(split_round).is_none());

    let mut substituted_primary_origin = begin;
    substituted_primary_origin
        .effects
        .slot0
        .requested
        .proposal_view += 1;
    substituted_primary_origin
        .effects
        .slot0
        .granted
        .proposal_view += 1;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(
            substituted_primary_origin
        )
    );

    let mut substituted_auxiliary_origin = begin;
    substituted_auxiliary_origin
        .effects
        .slot0
        .requested
        .auxiliary_proposal_view += 1;
    substituted_auxiliary_origin
        .effects
        .slot0
        .granted
        .auxiliary_proposal_view += 1;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(
            substituted_auxiliary_origin
        )
    );

    let mut acknowledge = begin;
    acknowledge.event_kind = EVENT_PERSISTED;
    acknowledge.event_persistence_id = begin.pending_after.persistence_id;
    acknowledge.pending_before = begin.pending_after;
    acknowledge.pending_after = PendingProjection::default();
    acknowledge.boundary_claimed.kind = BOUNDARY_ACKNOWLEDGE_WAL;
    acknowledge.boundary_granted = acknowledge.boundary_claimed;
    acknowledge.effects = EffectTrace::empty();
    acknowledge.durable_sequence_after = acknowledge.durable_sequence_before + 1;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(acknowledge));

    let mut substituted_ack_origin = acknowledge;
    substituted_ack_origin.boundary_claimed.proposal_view += 1;
    substituted_ack_origin.boundary_granted.proposal_view += 1;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(substituted_ack_origin),
        "acknowledgement must retain the same-round proposal field from the pending record"
    );
}

#[test]
fn durable_intent_accepts_only_empty_persistence_completion_stutters() {
    let owner = TagProjection {
        height: 4,
        view: 2,
        generation: 3,
    };
    let stale = ProductionDurableIntentTraceProjection {
        event_tag: TagProjection {
            height: owner.height,
            view: owner.view - 1,
            generation: owner.generation - 1,
        },
        owner_tag_before: owner,
        owner_tag_after: owner,
        durable_sequence_before: 8,
        durable_sequence_after: 8,
        ..ProductionDurableIntentTraceProjection::default()
    };
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale));

    let mut stale_persisted = stale;
    stale_persisted.event_kind = EVENT_PERSISTED;
    stale_persisted.event_persistence_id = 9;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(stale_persisted));

    let mut stale_persistence_failed = stale_persisted;
    stale_persistence_failed.event_kind = EVENT_PERSISTENCE_FAILED;
    assert!(
        production_durable_intent_trace_refines_progress_witness_kernel(stale_persistence_failed)
    );

    let mut current_owner_completion = stale_persisted;
    current_owner_completion.event_tag = owner;
    assert!(
        production_durable_intent_trace_refines_progress_witness_kernel(current_owner_completion)
    );

    let mut current_owner_failure = stale_persistence_failed;
    current_owner_failure.event_tag = owner;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(current_owner_failure));

    let mut zero_id_completion = current_owner_completion;
    zero_id_completion.event_persistence_id = 0;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(zero_id_completion));

    let mut non_persistence_payload = stale_persisted;
    non_persistence_payload.event_kind = EVENT_SIGNED;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(non_persistence_payload)
    );

    let mut changed_owner = stale;
    changed_owner.owner_tag_after.generation += 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(changed_owner));

    let mut invented_persist = stale;
    invented_persist.effects.slot0.kind = EFFECT_PERSIST;
    invented_persist.effects.len = 1;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(invented_persist));
}

#[test]
fn durable_timeout_boundary_preserves_record_and_successor_owner_rounds() {
    let mut begin = durable_begin_trace();
    begin.event_kind = 5;
    begin.boundary_claimed.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
    begin.boundary_claimed.continuation = CONTINUATION_INSTALL_TIMEOUT;
    begin.boundary_claimed.proposal_present = false;
    begin.boundary_claimed.proposal_height = 0;
    begin.boundary_claimed.proposal_view = 0;
    begin.boundary_claimed.auxiliary_present = true;
    begin.boundary_claimed.auxiliary_context_id = begin.boundary_claimed.context_id;
    begin.boundary_claimed.auxiliary_height = begin.owner_tag_before.height;
    begin.boundary_claimed.auxiliary_view = begin.owner_tag_before.view;
    begin.boundary_claimed.auxiliary_proposal_height = begin.owner_tag_before.height;
    begin.boundary_claimed.auxiliary_proposal_view = begin.owner_tag_before.view;
    begin.boundary_claimed.auxiliary_phase = 1;
    begin.boundary_claimed.auxiliary_subject = begin.boundary_claimed.subject.subject;
    begin.pending_after.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
    begin.pending_after.continuation = CONTINUATION_INSTALL_TIMEOUT;
    begin.pending_after.view = 5;
    begin.pending_after.proposal_present = false;
    begin.pending_after.proposal_height = 0;
    begin.pending_after.proposal_view = 0;
    begin.effects.slot0.requested.record_kind = WAL_RECORD_INSTALL_TIMEOUT;
    begin.effects.slot0.requested.view = 5;
    begin.effects.slot0.requested.proposal_height = 0;
    begin.effects.slot0.requested.proposal_view = 0;
    begin.effects.slot0.requested.subject = begin.boundary_claimed.subject.subject;
    begin.effects.slot0.requested.auxiliary_present = true;
    begin.effects.slot0.requested.auxiliary_context_id =
        begin.boundary_claimed.auxiliary_context_id;
    begin.effects.slot0.requested.auxiliary_height = begin.boundary_claimed.auxiliary_height;
    begin.effects.slot0.requested.auxiliary_view = begin.boundary_claimed.auxiliary_view;
    begin.effects.slot0.requested.auxiliary_proposal_height =
        begin.boundary_claimed.auxiliary_proposal_height;
    begin.effects.slot0.requested.auxiliary_proposal_view =
        begin.boundary_claimed.auxiliary_proposal_view;
    begin.effects.slot0.requested.auxiliary_phase = begin.boundary_claimed.auxiliary_phase;
    begin.effects.slot0.requested.auxiliary_subject = begin.boundary_claimed.subject.subject;
    begin.boundary_granted = begin.boundary_claimed;
    begin.effects.slot0.granted = begin.effects.slot0.requested;
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(begin));

    let immediate_predecessor_view = begin.owner_tag_before.view - 1;
    let mut predecessor_begin = begin;
    predecessor_begin.pending_after.view = immediate_predecessor_view;
    predecessor_begin.boundary_claimed.auxiliary_view = immediate_predecessor_view;
    predecessor_begin.boundary_claimed.auxiliary_proposal_view = immediate_predecessor_view;
    predecessor_begin.boundary_granted = predecessor_begin.boundary_claimed;
    predecessor_begin.effects.slot0.requested.view = immediate_predecessor_view;
    predecessor_begin.effects.slot0.requested.auxiliary_view = immediate_predecessor_view;
    predecessor_begin
        .effects
        .slot0
        .requested
        .auxiliary_proposal_view = immediate_predecessor_view;
    predecessor_begin.effects.slot0.granted = predecessor_begin.effects.slot0.requested;
    assert!(
        production_durable_intent_trace_refines_progress_witness_kernel(predecessor_begin),
        "an exact immediate-predecessor TC with a same-round high PrepareQC is owned"
    );

    let mut predecessor_without_high = predecessor_begin;
    predecessor_without_high.boundary_claimed.auxiliary_present = false;
    predecessor_without_high.boundary_granted = predecessor_without_high.boundary_claimed;
    predecessor_without_high
        .effects
        .slot0
        .requested
        .auxiliary_present = false;
    predecessor_without_high.effects.slot0.granted =
        predecessor_without_high.effects.slot0.requested;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(predecessor_without_high),
        "a no-high predecessor TC cannot claim the exceptional owner relation"
    );

    let mut missing_high_prepare_subject = begin;
    missing_high_prepare_subject
        .effects
        .slot0
        .requested
        .auxiliary_subject = Subject::default();
    missing_high_prepare_subject.effects.slot0.granted =
        missing_high_prepare_subject.effects.slot0.requested;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(
            missing_high_prepare_subject
        )
    );

    let mut missing_primary_subject = begin;
    missing_primary_subject.effects.slot0.requested.subject = Subject::default();
    missing_primary_subject.effects.slot0.granted = missing_primary_subject.effects.slot0.requested;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(missing_primary_subject)
    );

    let mut mismatched_persist_round = begin;
    mismatched_persist_round.effects.slot0.requested.view = 4;
    mismatched_persist_round.effects.slot0.granted.view = 4;
    assert!(
        !production_durable_intent_trace_refines_progress_witness_kernel(mismatched_persist_round)
    );

    let successor = TagProjection {
        height: begin.owner_tag_before.height,
        view: begin.pending_after.view + 1,
        generation: 0,
    };
    let mut acknowledge_boundary = begin.boundary_claimed;
    acknowledge_boundary.kind = BOUNDARY_ACKNOWLEDGE_WAL;
    acknowledge_boundary.tag = successor;
    let acknowledge = ProductionDurableIntentTraceProjection {
        event_tag: begin.owner_tag_before,
        owner_tag_before: begin.owner_tag_before,
        owner_tag_after: successor,
        event_kind: EVENT_PERSISTED,
        event_persistence_id: begin.pending_after.persistence_id,
        pending_before: begin.pending_after,
        pending_after: PendingProjection::default(),
        boundary_claimed: acknowledge_boundary,
        boundary_granted: acknowledge_boundary,
        effects: EffectTrace::empty(),
        durable_sequence_before: begin.durable_sequence_before,
        durable_sequence_after: begin.durable_sequence_before + 1,
    };
    assert!(production_durable_intent_trace_refines_progress_witness_kernel(acknowledge));

    let predecessor_successor = TagProjection {
        height: predecessor_begin.owner_tag_before.height,
        view: predecessor_begin.owner_tag_before.view,
        generation: predecessor_begin.owner_tag_before.generation + 1,
    };
    let mut predecessor_acknowledge_boundary = predecessor_begin.boundary_claimed;
    predecessor_acknowledge_boundary.kind = BOUNDARY_ACKNOWLEDGE_WAL;
    predecessor_acknowledge_boundary.tag = predecessor_successor;
    let predecessor_acknowledge = ProductionDurableIntentTraceProjection {
        event_tag: predecessor_begin.owner_tag_before,
        owner_tag_before: predecessor_begin.owner_tag_before,
        owner_tag_after: predecessor_successor,
        event_kind: EVENT_PERSISTED,
        event_persistence_id: predecessor_begin.pending_after.persistence_id,
        pending_before: predecessor_begin.pending_after,
        pending_after: PendingProjection::default(),
        boundary_claimed: predecessor_acknowledge_boundary,
        boundary_granted: predecessor_acknowledge_boundary,
        effects: EffectTrace::empty(),
        durable_sequence_before: predecessor_begin.durable_sequence_before,
        durable_sequence_after: predecessor_begin.durable_sequence_before + 1,
    };
    assert!(
        production_durable_intent_trace_refines_progress_witness_kernel(predecessor_acknowledge),
        "acknowledging an immediate-predecessor TC changes generation, not view"
    );

    let mut wrong_successor = acknowledge;
    wrong_successor.owner_tag_after.view -= 1;
    wrong_successor.boundary_claimed.tag = wrong_successor.owner_tag_after;
    wrong_successor.boundary_granted.tag = wrong_successor.owner_tag_after;
    assert!(!production_durable_intent_trace_refines_progress_witness_kernel(wrong_successor));
}

#[test]
fn remaining_progress_witness_kernels_reject_primitive_trace_mutations() {
    let identity =
        |domain, kind, byte| CanonicalIdentityProjection::from_bytes(domain, kind, [byte; 32]);
    let context = identity(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        1,
    );
    let subject = identity(IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_WIRE_BLOCK_SUBJECT, 2);
    let block_hash = identity(IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_BLOCK_HEADER, 3);
    let payload_hash = identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_CANONICAL_PAYLOAD, 4);
    let execution = identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_EXECUTION_COMMITMENT,
        5,
    );
    let executed_wire = identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
        6,
    );
    let decision = ProductionDecisionIdentityProjection {
        context_id: context,
        height: 9,
        view: 4,
        proposal_height: 9,
        proposal_view: 4,
        phase: 2,
        subject,
        block_hash,
        payload_hash,
        execution_commitment: execution,
        executed_block_wire_hash: executed_wire,
    };
    let commit_qc = ProductionQuorumCertificateIdentityProjection {
        decision,
        certificate: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_QUORUM_CERTIFICATE, 7),
        signer_count: 3,
        aggregate_signature_len: 96,
    };
    let manifest = identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_PAYLOAD_MANIFEST, 8);
    let durable_body = ProductionDurableBodyIdentityProjection {
        context_id: context,
        height: 9,
        view: 4,
        subject,
        block_hash,
        payload_hash,
        manifest,
        frame: identity(
            IDENTITY_DOMAIN_DURABLE_ARTIFACT,
            IDENTITY_KIND_DURABLE_BODY_FRAME,
            9,
        ),
    };
    let recovery = ProductionDecisionRecoveryTraceProjection {
        state_height: 8,
        expected_context_id: context,
        expected_height: 9,
        expected_block_hash: block_hash,
        frozen_context_id: context,
        frozen_height: 9,
        replay_tag: TagProjection {
            height: 9,
            view: 4,
            generation: 12,
        },
        owner_tag: TagProjection {
            height: 9,
            view: 4,
            generation: 12,
        },
        replay_generation: 12,
        commit_qc,
        manifest_round: TagProjection {
            height: 9,
            view: 4,
            generation: 0,
        },
        manifest_subject: subject,
        manifest,
        durable_body,
        validated_body: durable_body,
        validated_execution_commitment: execution,
        stage: 1,
    };
    assert!(production_decision_trace_refines_recovery_witness_kernel(
        recovery
    ));
    assert_eq!(
        check_production_decision_recovery_transition(recovery)
            .expect("valid Decision recovery must mint evidence")
            .into_projection(),
        recovery
    );
    assert!(recovery.expected_height > 0);
    assert!(recovery.state_height <= recovery.expected_height);
    assert!(recovery.expected_height - recovery.state_height <= 1);
    assert_eq!(recovery.durable_body.height, recovery.frozen_height);
    assert_eq!(recovery.stage, 1);
    let split_round_body = ProductionDurableBodyIdentityProjection {
        view: 2,
        ..durable_body
    };
    let split_round_commit_qc = ProductionQuorumCertificateIdentityProjection {
        decision: ProductionDecisionIdentityProjection {
            proposal_view: 2,
            ..decision
        },
        ..commit_qc
    };
    assert!(!production_decision_trace_refines_recovery_witness_kernel(
        ProductionDecisionRecoveryTraceProjection {
            commit_qc: split_round_commit_qc,
            manifest_round: TagProjection {
                view: 2,
                ..recovery.manifest_round
            },
            durable_body: split_round_body,
            validated_body: split_round_body,
            ..recovery
        }
    ));
    assert!(!production_decision_trace_refines_recovery_witness_kernel(
        ProductionDecisionRecoveryTraceProjection {
            commit_qc: split_round_commit_qc,
            manifest_round: TagProjection {
                view: 3,
                ..recovery.manifest_round
            },
            durable_body: split_round_body,
            validated_body: split_round_body,
            ..recovery
        }
    ));
    assert!(!production_decision_trace_refines_recovery_witness_kernel(
        ProductionDecisionRecoveryTraceProjection {
            commit_qc: split_round_commit_qc,
            manifest_round: TagProjection {
                view: 5,
                ..recovery.manifest_round
            },
            durable_body: ProductionDurableBodyIdentityProjection {
                view: 5,
                ..durable_body
            },
            validated_body: ProductionDurableBodyIdentityProjection {
                view: 5,
                ..durable_body
            },
            ..recovery
        }
    ));
    let reproposal_commit_qc = ProductionQuorumCertificateIdentityProjection {
        decision: ProductionDecisionIdentityProjection {
            view: 5,
            proposal_view: 5,
            ..decision
        },
        ..commit_qc
    };
    let reproposal_body = ProductionDurableBodyIdentityProjection {
        view: 5,
        ..durable_body
    };
    assert!(production_decision_trace_refines_recovery_witness_kernel(
        ProductionDecisionRecoveryTraceProjection {
            commit_qc: reproposal_commit_qc,
            manifest_round: TagProjection {
                view: 5,
                ..recovery.manifest_round
            },
            durable_body: reproposal_body,
            validated_body: reproposal_body,
            ..recovery
        }
    ));
    for view in [3, 7] {
        let owner_tag = TagProjection {
            view,
            ..recovery.owner_tag
        };
        assert!(production_decision_trace_refines_recovery_witness_kernel(
            ProductionDecisionRecoveryTraceProjection {
                replay_tag: owner_tag,
                owner_tag,
                ..recovery
            }
        ));
    }
    let replaced_recovery_owner = ProductionDecisionRecoveryTraceProjection {
        owner_tag: TagProjection {
            view: 5,
            ..recovery.owner_tag
        },
        ..recovery
    };
    assert!(!production_decision_trace_refines_recovery_witness_kernel(
        replaced_recovery_owner
    ));
    assert!(check_production_decision_recovery_transition(replaced_recovery_owner).is_none());

    let scheduler = ProductionSchedulerTraceProjection {
        fifo_owed_before: false,
        timeout_due: false,
        periodic_timer_due: true,
        fifo_ready: true,
        selected: 2,
        fifo_owed_after: true,
    };
    assert!(production_scheduler_trace_refines_protected_ownership_kernel(scheduler));
    assert_eq!(
        check_production_scheduler_transition(scheduler)
            .expect("valid scheduler choice must mint evidence")
            .into_projection(),
        scheduler
    );
    assert!(scheduler.selected <= 3);
    assert_eq!(scheduler.selected, 2);
    assert_eq!(scheduler.fifo_owed_after, scheduler.fifo_ready);
    let replaced_scheduler_owner = ProductionSchedulerTraceProjection {
        selected: 3,
        ..scheduler
    };
    assert!(
        !production_scheduler_trace_refines_protected_ownership_kernel(replaced_scheduler_owner)
    );
    assert!(check_production_scheduler_transition(replaced_scheduler_owner).is_none());

    let ingress = ProductionIngressIdentityAndClassTraceProjection {
        incoming_height: 4,
        incoming_view: 2,
        incoming_generation: 3,
        incoming_class: SERVICE_CLASS_PROGRESS,
        stored_height: 4,
        stored_view: 2,
        stored_generation: 3,
        stored_class: SERVICE_CLASS_PROGRESS,
        queue_len_before: 1,
        queue_len_after: 2,
        queue_capacity: 4,
        ordinal_source_before: 11,
        physical_admission_ordinal: 11,
        lifecycle_ordinal: 11,
        ordinal_source_after: 12,
        dormant_reservations_before: 0,
        dormant_reservations_after: 0,
        dormant_owner_ordinal: 0,
        ordinal_minted: true,
    };
    assert!(
        production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(ingress)
    );
    assert_eq!(
        check_production_ingress_transition(ingress)
            .expect("valid ingress admission must mint evidence")
            .into_projection(),
        ingress
    );
    assert_eq!(ingress.incoming_height, ingress.stored_height);
    assert_eq!(ingress.incoming_view, ingress.stored_view);
    assert_eq!(ingress.incoming_generation, ingress.stored_generation);
    assert_eq!(ingress.incoming_class, ingress.stored_class);
    assert!(ingress.queue_len_after > ingress.queue_len_before);
    assert!(ingress.queue_len_after <= ingress.queue_capacity);
    let replaced_ingress_owner = ProductionIngressIdentityAndClassTraceProjection {
        stored_generation: 4,
        ..ingress
    };
    assert!(
        !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            replaced_ingress_owner
        )
    );
    assert!(check_production_ingress_transition(replaced_ingress_owner).is_none());

    let uncommitted_ingress_ordinal = ProductionIngressIdentityAndClassTraceProjection {
        ordinal_source_after: ingress.ordinal_source_before,
        ..ingress
    };
    assert!(
        !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            uncommitted_ingress_ordinal
        )
    );
    assert!(check_production_ingress_transition(uncommitted_ingress_ordinal).is_none());

    let skipped_ingress_ordinal = ProductionIngressIdentityAndClassTraceProjection {
        ordinal_source_before: 10,
        ..ingress
    };
    assert!(
        !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            skipped_ingress_ordinal
        )
    );
    assert!(check_production_ingress_transition(skipped_ingress_ordinal).is_none());

    let dormant_replacement = ProductionIngressIdentityAndClassTraceProjection {
        lifecycle_ordinal: 7,
        dormant_reservations_before: 2,
        dormant_reservations_after: 1,
        dormant_owner_ordinal: 7,
        ..ingress
    };
    assert!(
        production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            dormant_replacement
        )
    );
    assert!(check_production_ingress_transition(dormant_replacement).is_some());

    let replaced_dormant_owner = ProductionIngressIdentityAndClassTraceProjection {
        lifecycle_ordinal: 7,
        dormant_reservations_before: 2,
        dormant_reservations_after: 1,
        dormant_owner_ordinal: 8,
        ..ingress
    };
    assert!(
        !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            replaced_dormant_owner
        )
    );
    assert!(check_production_ingress_transition(replaced_dormant_owner).is_none());

    let over_capacity_dormant_owner = ProductionIngressIdentityAndClassTraceProjection {
        dormant_reservations_before: 3,
        dormant_reservations_after: 3,
        ..ingress
    };
    assert!(
        !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            over_capacity_dormant_owner
        )
    );
    assert!(check_production_ingress_transition(over_capacity_dormant_owner).is_none());

    let materialized_reservation = ProductionIngressIdentityAndClassTraceProjection {
        ordinal_source_before: 12,
        physical_admission_ordinal: 11,
        ordinal_source_after: 12,
        ordinal_minted: false,
        ..ingress
    };
    assert!(
        production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            materialized_reservation
        )
    );
    assert!(check_production_ingress_transition(materialized_reservation).is_some());

    let flush = ProductionReliableFlushTraceProjection {
        status: 2,
        semantic_target: identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, 20),
        authenticated_source: identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, 21),
        source_key_identity: identity(
            IDENTITY_DOMAIN_PROCESS_LOCAL,
            IDENTITY_KIND_REPLY_SOURCE_KEY,
            35,
        ),
        delivery_route_identity: identity(
            IDENTITY_DOMAIN_PROCESS_LOCAL,
            IDENTITY_KIND_REPLY_DELIVERY_ROUTE,
            36,
        ),
        writer_occurrence_identity: identity(
            IDENTITY_DOMAIN_PROCESS_LOCAL,
            IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,
            37,
        ),
        requester: identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, 20),
        responder: identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, 22),
        connection_tenure_ordinal_high: 0,
        connection_tenure_ordinal_low: 1,
        delivery_ordinal_high: 0,
        delivery_ordinal_low: 2,
        ticket_id: 3,
        ticket_rank: 1,
        ticket_topic: 3,
        reply_writer_timeout_attempt: 4,
        canonical_request_digest: identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_REPLY_PAYLOAD,
            23,
        ),
        stream_wire_bytes: 512,
        request_id: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_SIDECAR_REQUEST, 24),
        service_generation: 6,
        stream_epoch: 5,
        semantic_sequence: 7,
        entry_hash: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_MERGE_ENTRY, 25),
        encoded_len: 256,
        epoch_id: 4,
        reference_digest: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_REFERENCE_DIGEST, 26),
        canonical_response_hash: identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_NETWORK_RESPONSE,
            27,
        ),
        sidecar_response_hash: identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_SIDECAR_RESPONSE,
            28,
        ),
        chunk_hash: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_SIDECAR_CHUNK, 29),
        payload_digest: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_SIDECAR_PAYLOAD, 30),
        chunk_index: 0,
        chunk_count: 2,
        message_cursor_before: 0,
        message_cursor_after: 1,
        chunk_cursor_before: 0,
        chunk_cursor_after: 1,
        flushing_before: 1,
        flushing_after: 0,
        admitted_before: 0,
        admitted_after: 1,
        capacity: 2,
    };
    assert!(production_reliable_flush_trace_refines_outbound_ownership_kernel(flush));
    assert_eq!(
        check_production_reliable_flush_worker_transition(flush)
            .expect("valid worker flush must mint evidence")
            .into_projection(),
        flush
    );
    assert!((1..=3).contains(&flush.status));
    assert!(flush.chunk_index < flush.chunk_count);
    assert_eq!(flush.chunk_cursor_before, flush.chunk_index);
    assert!(flush.flushing_after <= flush.capacity);
    assert!(
        !production_reliable_flush_trace_refines_outbound_ownership_kernel(
            ProductionReliableFlushTraceProjection {
                stream_epoch: 0,
                ..flush
            }
        )
    );
    assert!(
        check_production_reliable_flush_worker_transition(ProductionReliableFlushTraceProjection {
            stream_epoch: 0,
            ..flush
        })
        .is_none()
    );
    assert!(
        !production_reliable_flush_trace_refines_outbound_ownership_kernel(
            ProductionReliableFlushTraceProjection {
                semantic_sequence: 0,
                ..flush
            }
        )
    );
    assert!(
        !production_reliable_flush_trace_refines_outbound_ownership_kernel(
            ProductionReliableFlushTraceProjection {
                service_generation: 0,
                ..flush
            }
        )
    );
    assert!(
        !production_reliable_flush_trace_refines_outbound_ownership_kernel(
            ProductionReliableFlushTraceProjection {
                admitted_after: 0,
                ..flush
            }
        )
    );

    let gate_residual = identity(
        IDENTITY_DOMAIN_PROCESS_LOCAL,
        IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE,
        31,
    );
    let outbound_residual = identity(
        IDENTITY_DOMAIN_PROCESS_LOCAL,
        IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE,
        32,
    );
    let shared_transfer = identity(
        IDENTITY_DOMAIN_PROCESS_LOCAL,
        IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE,
        33,
    );
    let sibling_state = identity(
        IDENTITY_DOMAIN_PROCESS_LOCAL,
        IDENTITY_KIND_SIDECAR_SIBLING_STATE,
        34,
    );
    let mut lane_application = ProductionReliableFlushApplicationProjection {
        semantic_target: flush.semantic_target,
        authenticated_source: flush.authenticated_source,
        source_key_identity: flush.source_key_identity,
        delivery_route_identity: flush.delivery_route_identity,
        writer_occurrence_identity: flush.writer_occurrence_identity,
        requester: flush.requester,
        responder: flush.responder,
        connection_tenure_ordinal_high: flush.connection_tenure_ordinal_high,
        connection_tenure_ordinal_low: flush.connection_tenure_ordinal_low,
        delivery_ordinal_high: flush.delivery_ordinal_high,
        delivery_ordinal_low: flush.delivery_ordinal_low,
        ticket_id: flush.ticket_id,
        ticket_rank: flush.ticket_rank,
        ticket_topic: flush.ticket_topic,
        reply_writer_timeout_attempt: flush.reply_writer_timeout_attempt,
        canonical_request_digest: flush.canonical_request_digest,
        stream_wire_bytes: flush.stream_wire_bytes,
        request_id: flush.request_id,
        service_generation: flush.service_generation,
        stream_epoch: flush.stream_epoch,
        semantic_sequence: flush.semantic_sequence,
        entry_hash: flush.entry_hash,
        encoded_len: flush.encoded_len,
        epoch_id: flush.epoch_id,
        reference_digest: flush.reference_digest,
        canonical_response_hash: flush.canonical_response_hash,
        sidecar_response_hash: flush.sidecar_response_hash,
        chunk_hash: flush.chunk_hash,
        payload_digest: flush.payload_digest,
        chunk_index: flush.chunk_index,
        chunk_count: flush.chunk_count,
        message_cursor_before: flush.message_cursor_before,
        message_cursor_after: flush.message_cursor_after,
        chunk_cursor_before: flush.chunk_cursor_before,
        chunk_cursor_after: flush.chunk_cursor_after,
        marker_request_id: flush.request_id,
        marker_service_generation: flush.service_generation,
        marker_stream_epoch: flush.stream_epoch,
        marker_semantic_sequence: flush.semantic_sequence,
        marker_entry_hash: flush.entry_hash,
        marker_encoded_len: flush.encoded_len,
        marker_epoch_id: flush.epoch_id,
        marker_reference_digest: flush.reference_digest,
        marker_requester: flush.requester,
        marker_responder: flush.responder,
        marker_canonical_response_hash: flush.canonical_response_hash,
        marker_sidecar_response_hash: flush.sidecar_response_hash,
        marker_chunk_hash: flush.chunk_hash,
        marker_payload_digest: flush.payload_digest,
        marker_chunk_index: flush.chunk_index,
        marker_chunk_count: flush.chunk_count,
        marker_topic: flush.ticket_topic,
        claim_acquired: true,
        gate_marker_present_before: true,
        gate_marker_present_after: false,
        gate_cursor_before: 0,
        gate_cursor_after: 1,
        gate_complete_after: false,
        gate_attempt_present_after: true,
        outbound_attempt_present_before: true,
        outbound_route_bound_before: true,
        outbound_route_active_before: true,
        outbound_cursor_before: 0,
        outbound_cursor_after: 1,
        outbound_in_flight_before_present: true,
        outbound_in_flight_before: 0,
        outbound_queued_before: false,
        outbound_order_count_before: 0,
        outbound_order_rank_before: 0,
        sibling_order_len_before: 2,
        outbound_attempt_present_after: true,
        outbound_in_flight_after_present: false,
        outbound_queued_after: true,
        outbound_order_count_after: 1,
        outbound_order_rank_after: 2,
        sibling_order_len_after: 2,
        inserted_preserved: true,
        inserted_equals_now: false,
        target_gate_residual_records_equal: true,
        target_gate_residual_before: gate_residual,
        target_gate_residual_after: gate_residual,
        target_outbound_residual_records_equal: true,
        target_outbound_residual_before: outbound_residual,
        target_outbound_residual_after: outbound_residual,
        shared_transfer_present_before: true,
        shared_transfer_present_after: true,
        shared_transfer_other_attempts_before: false,
        shared_transfer_records_equal: true,
        shared_transfer_state_before: shared_transfer,
        shared_transfer_state_after: shared_transfer,
        sibling_records_equal: true,
        sibling_state_before: sibling_state,
        sibling_state_after: sibling_state,
    };
    assert!(production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    assert!(production_reliable_flush_two_phase_link_kernel(
        flush,
        lane_application
    ));
    assert_eq!(
        check_production_reliable_flush_application_transition(lane_application)
            .expect("valid lane flush application must mint evidence")
            .into_projection(),
        lane_application
    );
    assert_eq!(
        check_production_reliable_flush_link_transition(flush, lane_application)
            .expect("linked flush occurrence must mint evidence")
            .into_projection(),
        (flush, lane_application)
    );
    let disconnected_application_timeout_attempt = ProductionReliableFlushApplicationProjection {
        reply_writer_timeout_attempt: lane_application
            .reply_writer_timeout_attempt
            .saturating_add(1),
        ..lane_application
    };
    assert!(
        production_reliable_flush_application_refines_source_lane_kernel(
            disconnected_application_timeout_attempt
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        flush,
        disconnected_application_timeout_attempt
    ));
    assert!(
        check_production_reliable_flush_link_transition(
            flush,
            disconnected_application_timeout_attempt
        )
        .is_none()
    );
    let disconnected_worker_timeout_attempt = ProductionReliableFlushTraceProjection {
        reply_writer_timeout_attempt: flush.reply_writer_timeout_attempt.saturating_add(1),
        ..flush
    };
    assert!(
        production_reliable_flush_trace_refines_outbound_ownership_kernel(
            disconnected_worker_timeout_attempt
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        disconnected_worker_timeout_attempt,
        lane_application
    ));

    let zero_stream_epoch_application = ProductionReliableFlushApplicationProjection {
        stream_epoch: 0,
        marker_stream_epoch: 0,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            zero_stream_epoch_application
        )
    );
    assert!(
        check_production_reliable_flush_application_transition(zero_stream_epoch_application)
            .is_none()
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        ProductionReliableFlushTraceProjection {
            stream_epoch: 0,
            ..flush
        },
        zero_stream_epoch_application
    ));
    let disconnected_marker_stream_epoch = ProductionReliableFlushApplicationProjection {
        marker_stream_epoch: lane_application.marker_stream_epoch + 1,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            disconnected_marker_stream_epoch
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        flush,
        disconnected_marker_stream_epoch
    ));
    let disconnected_occurrence_stream_epoch = ProductionReliableFlushApplicationProjection {
        stream_epoch: lane_application.stream_epoch + 1,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            disconnected_occurrence_stream_epoch
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        flush,
        disconnected_occurrence_stream_epoch
    ));
    let disconnected_worker_stream_epoch = ProductionReliableFlushTraceProjection {
        stream_epoch: flush.stream_epoch + 1,
        ..flush
    };
    assert!(
        production_reliable_flush_trace_refines_outbound_ownership_kernel(
            disconnected_worker_stream_epoch
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        disconnected_worker_stream_epoch,
        lane_application
    ));

    let zero_service_generation_application = ProductionReliableFlushApplicationProjection {
        service_generation: 0,
        marker_service_generation: 0,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            zero_service_generation_application
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        ProductionReliableFlushTraceProjection {
            service_generation: 0,
            ..flush
        },
        zero_service_generation_application
    ));
    let disconnected_marker_service_generation = ProductionReliableFlushApplicationProjection {
        marker_service_generation: lane_application.marker_service_generation + 1,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            disconnected_marker_service_generation
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        flush,
        disconnected_marker_service_generation
    ));
    let disconnected_occurrence_service_generation = ProductionReliableFlushApplicationProjection {
        service_generation: lane_application.service_generation + 1,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            disconnected_occurrence_service_generation
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        flush,
        disconnected_occurrence_service_generation
    ));
    let disconnected_worker_service_generation = ProductionReliableFlushTraceProjection {
        service_generation: flush.service_generation + 1,
        ..flush
    };
    assert!(
        production_reliable_flush_trace_refines_outbound_ownership_kernel(
            disconnected_worker_service_generation
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        disconnected_worker_service_generation,
        lane_application
    ));

    let zero_semantic_sequence_application = ProductionReliableFlushApplicationProjection {
        semantic_sequence: 0,
        marker_semantic_sequence: 0,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            zero_semantic_sequence_application
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        ProductionReliableFlushTraceProjection {
            semantic_sequence: 0,
            ..flush
        },
        zero_semantic_sequence_application
    ));
    let disconnected_marker_semantic_sequence = ProductionReliableFlushApplicationProjection {
        marker_semantic_sequence: lane_application.marker_semantic_sequence + 1,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            disconnected_marker_semantic_sequence
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        flush,
        disconnected_marker_semantic_sequence
    ));
    let disconnected_occurrence_semantic_sequence = ProductionReliableFlushApplicationProjection {
        semantic_sequence: lane_application.semantic_sequence + 1,
        ..lane_application
    };
    assert!(
        !production_reliable_flush_application_refines_source_lane_kernel(
            disconnected_occurrence_semantic_sequence
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        flush,
        disconnected_occurrence_semantic_sequence
    ));
    let disconnected_worker_semantic_sequence = ProductionReliableFlushTraceProjection {
        semantic_sequence: flush.semantic_sequence + 1,
        ..flush
    };
    assert!(
        production_reliable_flush_trace_refines_outbound_ownership_kernel(
            disconnected_worker_semantic_sequence
        )
    );
    assert!(!production_reliable_flush_two_phase_link_kernel(
        disconnected_worker_semantic_sequence,
        lane_application
    ));

    lane_application.marker_chunk_index = 1;
    assert!(!production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    lane_application.marker_chunk_index = 0;
    lane_application.gate_cursor_after = 2;
    assert!(!production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    lane_application.gate_cursor_after = 1;
    lane_application.sibling_records_equal = false;
    assert!(!production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    lane_application.sibling_records_equal = true;
    lane_application.target_gate_residual_after = sibling_state;
    assert!(!production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    lane_application.target_gate_residual_after = gate_residual;
    lane_application.shared_transfer_state_after = sibling_state;
    assert!(!production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    lane_application.shared_transfer_state_after = shared_transfer;
    lane_application.inserted_preserved = false;
    assert!(!production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    lane_application.inserted_preserved = true;
    lane_application.outbound_order_count_after = 2;
    assert!(!production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    lane_application.outbound_order_count_after = 1;
    lane_application.outbound_order_rank_after = 1;
    assert!(!production_reliable_flush_application_refines_source_lane_kernel(lane_application));
    lane_application.outbound_order_rank_after = 2;
    let disconnected_worker = ProductionReliableFlushTraceProjection {
        chunk_hash: identity(IDENTITY_DOMAIN_PAYLOAD, IDENTITY_KIND_SIDECAR_CHUNK, 35),
        ..flush
    };
    assert!(production_reliable_flush_trace_refines_outbound_ownership_kernel(disconnected_worker));
    assert!(!production_reliable_flush_two_phase_link_kernel(
        disconnected_worker,
        lane_application
    ));

    let artifact_hash = identity(
        IDENTITY_DOMAIN_DURABLE_ARTIFACT,
        IDENTITY_KIND_FINALITY_ARTIFACT,
        11,
    );
    let application = ProductionApplicationTraceProjection {
        task_tag: TagProjection {
            height: 9,
            view: 4,
            generation: 12,
        },
        owner_tag: TagProjection {
            height: 9,
            view: 4,
            generation: 12,
        },
        task_generation: 12,
        context_id: context,
        context_height: 9,
        commit_qc,
        validated_body: durable_body,
        validated_execution_commitment: execution,
        proposal_block_hash: block_hash,
        proposal_payload_hash: payload_hash,
        committed_block_hash: block_hash,
        executed_block_wire_hash: executed_wire,
        kura_decision: decision,
        kura_artifact_hash: artifact_hash,
        artifact_context_id: context,
        artifact_height: 9,
        artifact_subject: subject,
        artifact_block_hash: block_hash,
        artifact_commit_qc: commit_qc,
        artifact_hash,
        state_height_after: 9,
        task_work_id: 11,
        completion_work_id: 11,
    };
    assert!(production_application_trace_refines_decision_completion_kernel(application));
    assert_eq!(
        check_production_application_transition(application)
            .expect("valid durable application must mint evidence")
            .into_projection(),
        application
    );
    assert!(application.context_height > 0);
    assert_eq!(application.state_height_after, application.context_height);
    assert_eq!(application.artifact_height, application.context_height);
    assert_eq!(application.completion_work_id, application.task_work_id);
    assert_eq!(application.artifact_context_id, application.context_id);
    let split_round_application = ProductionApplicationTraceProjection {
        commit_qc: split_round_commit_qc,
        validated_body: ProductionDurableBodyIdentityProjection {
            view: 2,
            ..application.validated_body
        },
        kura_decision: split_round_commit_qc.decision,
        artifact_commit_qc: split_round_commit_qc,
        ..application
    };
    assert!(
        !production_application_trace_refines_decision_completion_kernel(split_round_application)
    );
    let reproposal_application = ProductionApplicationTraceProjection {
        commit_qc: reproposal_commit_qc,
        validated_body: ProductionDurableBodyIdentityProjection {
            view: 5,
            ..application.validated_body
        },
        // A prior same-body decision is semantically equivalent even
        // though the exact later QC remains the application artifact.
        kura_decision: decision,
        artifact_commit_qc: reproposal_commit_qc,
        ..application
    };
    assert!(
        production_application_trace_refines_decision_completion_kernel(reproposal_application)
    );
    assert!(
        !production_application_trace_refines_decision_completion_kernel(
            ProductionApplicationTraceProjection {
                validated_body: ProductionDurableBodyIdentityProjection {
                    view: 5,
                    ..application.validated_body
                },
                ..application
            }
        )
    );
    for view in [3, 7] {
        let owner_tag = TagProjection {
            view,
            generation: 15,
            ..application.owner_tag
        };
        assert!(
            production_application_trace_refines_decision_completion_kernel(
                ProductionApplicationTraceProjection {
                    task_tag: owner_tag,
                    owner_tag,
                    task_generation: 15,
                    ..application
                }
            )
        );
    }
    assert!(
        !production_application_trace_refines_decision_completion_kernel(
            ProductionApplicationTraceProjection {
                owner_tag: TagProjection {
                    view: 5,
                    ..application.owner_tag
                },
                ..application
            }
        )
    );
    let replaced_completion_owner = ProductionApplicationTraceProjection {
        completion_work_id: 12,
        ..application
    };
    assert!(
        !production_application_trace_refines_decision_completion_kernel(replaced_completion_owner)
    );
    assert!(check_production_application_transition(replaced_completion_owner).is_none());

    let terminal_application = ProductionTerminalApplicationWithoutSuccessorActivationProjection {
        context_id: context,
        context_height: 9,
        receipt_context_id: context,
        receipt_height: 9,
        receipt_block_hash: block_hash,
        receipt_artifact_hash: artifact_hash,
        artifact_context_id: context,
        artifact_height: 9,
        artifact_block_hash: block_hash,
        artifact_hash,
        predecessor: ProductionDurablePredecessorIdentityProjection {
            height: 9,
            block_hash,
            artifact_hash,
        },
        pending_successor_activation_present: false,
    };
    assert!(
        production_terminal_application_without_successor_activation_kernel(terminal_application)
    );
    assert_eq!(
        check_production_terminal_application_transition(terminal_application)
            .expect("valid terminal application must mint evidence")
            .into_projection(),
        terminal_application
    );
    assert_eq!(
        terminal_application.receipt_height,
        terminal_application.context_height
    );
    assert_eq!(
        terminal_application.artifact_height,
        terminal_application.context_height
    );
    assert!(!terminal_application.pending_successor_activation_present);
    let premature_successor = ProductionTerminalApplicationWithoutSuccessorActivationProjection {
        pending_successor_activation_present: true,
        ..terminal_application
    };
    assert!(
        !production_terminal_application_without_successor_activation_kernel(premature_successor)
    );
    assert!(check_production_terminal_application_transition(premature_successor).is_none());
    assert!(
        !production_terminal_application_without_successor_activation_kernel(
            ProductionTerminalApplicationWithoutSuccessorActivationProjection {
                receipt_artifact_hash: identity(
                    IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                    IDENTITY_KIND_FINALITY_ARTIFACT,
                    12,
                ),
                ..terminal_application
            }
        )
    );
}

#[test]
fn leader_wire_admission_gate_separates_insert_reactivation_coalescing_and_replacement() {
    let lifecycle_identity = |byte| {
        CanonicalIdentityProjection::from_bytes(
            IDENTITY_DOMAIN_PROCESS_LOCAL,
            IDENTITY_KIND_LEADER_WIRE_LIFECYCLE,
            [byte; 32],
        )
    };
    let first_identity = lifecycle_identity(1);
    let second_identity = lifecycle_identity(2);
    let insert = ProductionLeaderWireAdmissionTraceProjection {
        operation: LEADER_WIRE_ADMISSION_INSERT,
        incoming_identity: first_identity,
        stored_identity: first_identity,
        incoming_view: 2,
        stored_view: 2,
        incoming_admission_ordinal: 11,
        stored_admission_ordinal: 11,
        incoming_scheduler_ordinal: 21,
        stored_scheduler_ordinal: 21,
        last_admission_ordinal_before: 10,
        last_admission_ordinal_after: 11,
        scheduler_ordinal_high_watermark_before: 20,
        scheduler_ordinal_high_watermark_after: 21,
        records_before: 2,
        records_after: 3,
        capacity: 4,
        status_before: LEADER_WIRE_LIFECYCLE_ABSENT,
        status_after: LEADER_WIRE_LIFECYCLE_INGRESS,
        ..ProductionLeaderWireAdmissionTraceProjection::default()
    };
    assert!(production_leader_wire_admission_refines_lifecycle_ownership_kernel(insert));
    assert_eq!(
        check_production_leader_wire_admission_transition(insert)
            .expect("an absent slot mints one exact ingress lifecycle")
            .into_projection(),
        insert
    );
    assert!(
        check_production_leader_wire_admission_transition(
            ProductionLeaderWireAdmissionTraceProjection {
                stored_identity: second_identity,
                ..insert
            }
        )
        .is_none(),
        "the prospective stored identity cannot be substituted"
    );

    let reactivate = ProductionLeaderWireAdmissionTraceProjection {
        operation: LEADER_WIRE_ADMISSION_REACTIVATE,
        incumbent_identity: first_identity,
        incumbent_view: 2,
        incumbent_admission_ordinal: 11,
        incumbent_scheduler_ordinal: 21,
        last_admission_ordinal_before: 11,
        last_admission_ordinal_after: 11,
        scheduler_ordinal_high_watermark_before: 21,
        scheduler_ordinal_high_watermark_after: 21,
        records_before: 3,
        records_after: 3,
        status_before: LEADER_WIRE_LIFECYCLE_DORMANT,
        replay_dormant_before: true,
        runtime_owner_before: true,
        runtime_owner_after: true,
        ..insert
    };
    assert!(production_leader_wire_admission_refines_lifecycle_ownership_kernel(reactivate));
    assert!(
        check_production_leader_wire_admission_transition(
            ProductionLeaderWireAdmissionTraceProjection {
                incoming_admission_ordinal: 99,
                incoming_scheduler_ordinal: 100,
                ..reactivate
            }
        )
        .is_some(),
        "a speculative retry ordinal is discarded in favor of the incumbent lifecycle"
    );
    assert!(
        check_production_leader_wire_admission_transition(
            ProductionLeaderWireAdmissionTraceProjection {
                replay_dormant_before: false,
                ..reactivate
            }
        )
        .is_none(),
        "reactivation must consume the exact restart-dormant potential"
    );
    assert!(
        check_production_leader_wire_admission_transition(
            ProductionLeaderWireAdmissionTraceProjection {
                incoming_scheduler_ordinal: 22,
                stored_scheduler_ordinal: 22,
                ..reactivate
            }
        )
        .is_none(),
        "an exact retry cannot publish a new scheduler position"
    );

    let coalesce = ProductionLeaderWireAdmissionTraceProjection {
        operation: LEADER_WIRE_ADMISSION_COALESCE,
        status_before: LEADER_WIRE_LIFECYCLE_INGRESS,
        status_after: LEADER_WIRE_LIFECYCLE_INGRESS,
        replay_dormant_before: false,
        runtime_owner_before: true,
        runtime_owner_after: true,
        ..reactivate
    };
    assert!(production_leader_wire_admission_refines_lifecycle_ownership_kernel(coalesce));
    assert_eq!(
        check_production_leader_wire_admission_transition(coalesce)
            .expect("a duplicate retry coalesces without a lifecycle mutation")
            .into_projection(),
        coalesce
    );

    let terminal_coalesce = ProductionLeaderWireAdmissionTraceProjection {
        status_before: LEADER_WIRE_LIFECYCLE_TERMINAL,
        status_after: LEADER_WIRE_LIFECYCLE_TERMINAL,
        terminal_evidence_before: true,
        terminal_evidence_after: true,
        ..coalesce
    };
    assert!(
        production_leader_wire_admission_refines_lifecycle_ownership_kernel(terminal_coalesce),
        "an exact drained request remains coalesced by its stable tombstone"
    );

    let replacement = ProductionLeaderWireAdmissionTraceProjection {
        operation: LEADER_WIRE_ADMISSION_REPLACE_TERMINAL,
        incoming_identity: second_identity,
        incumbent_identity: first_identity,
        stored_identity: second_identity,
        incoming_view: 3,
        incumbent_view: 2,
        stored_view: 3,
        incoming_admission_ordinal: 12,
        incumbent_admission_ordinal: 11,
        stored_admission_ordinal: 12,
        incoming_scheduler_ordinal: 22,
        incumbent_scheduler_ordinal: 21,
        stored_scheduler_ordinal: 22,
        last_admission_ordinal_before: 11,
        last_admission_ordinal_after: 12,
        scheduler_ordinal_high_watermark_before: 21,
        scheduler_ordinal_high_watermark_after: 22,
        records_before: 3,
        records_after: 3,
        capacity: 4,
        status_before: LEADER_WIRE_LIFECYCLE_TERMINAL,
        status_after: LEADER_WIRE_LIFECYCLE_INGRESS,
        runtime_owner_before: true,
        terminal_evidence_before: true,
        ..ProductionLeaderWireAdmissionTraceProjection::default()
    };
    assert!(production_leader_wire_admission_refines_lifecycle_ownership_kernel(replacement));
    for invalid in [
        ProductionLeaderWireAdmissionTraceProjection {
            incoming_view: 2,
            stored_view: 2,
            ..replacement
        },
        ProductionLeaderWireAdmissionTraceProjection {
            incoming_identity: first_identity,
            stored_identity: first_identity,
            ..replacement
        },
        ProductionLeaderWireAdmissionTraceProjection {
            scheduler_ordinal_high_watermark_after: 21,
            ..replacement
        },
        ProductionLeaderWireAdmissionTraceProjection {
            status_before: LEADER_WIRE_LIFECYCLE_RUNTIME,
            terminal_evidence_before: false,
            ..replacement
        },
        ProductionLeaderWireAdmissionTraceProjection {
            incumbent_identity: CanonicalIdentityProjection::zero(),
            ..replacement
        },
    ] {
        assert!(
            check_production_leader_wire_admission_transition(invalid).is_none(),
            "terminal replacement must advance identity, view, and both high-watermarks"
        );
    }
}

#[test]
fn effective_lock_trace_wrappers_accept_only_their_exact_live_projection() {
    let enter_view = EffectiveLockTraceProjection {
        kind: EFFECTIVE_LOCK_TRACE_ENTER_VIEW,
        relation_exact: true,
        protected_before: 1,
        protected_after: 1,
        owner_before: 1,
        owner_after: 1,
        ..EffectiveLockTraceProjection::default()
    };
    let enter_view_identity = EnterViewProjection::default();
    assert!(
        production_enter_view_uses_post_install_effective_lock_kernel(
            enter_view,
            enter_view_identity,
        )
    );
    assert_eq!(
        check_production_enter_view_effective_lock_transition(enter_view, enter_view_identity,)
            .expect("exact EnterView trace mints checked evidence")
            .into_projection(),
        (enter_view, enter_view_identity),
    );
    assert_eq!(enter_view.kind, EFFECTIVE_LOCK_TRACE_ENTER_VIEW);
    assert_eq!(enter_view.protected_after, enter_view.protected_before);
    assert_eq!(enter_view.owner_after, enter_view.owner_before);
    assert_eq!(
        enter_view_identity.effect_protected_lock.present,
        enter_view_identity.durable_lock_after.present
    );
    assert_eq!(
        enter_view_identity.following_fetch_lock.present,
        enter_view_identity.durable_lock_after.present
    );
    assert!(
        !production_enter_view_uses_post_install_effective_lock_kernel(
            EffectiveLockTraceProjection {
                owner_after: 0,
                ..enter_view
            },
            enter_view_identity,
        )
    );
    assert!(
        check_production_enter_view_effective_lock_transition(
            EffectiveLockTraceProjection {
                owner_after: 0,
                ..enter_view
            },
            enter_view_identity,
        )
        .is_none()
    );

    let ownership = EffectiveLockTraceProjection {
        kind: EFFECTIVE_LOCK_TRACE_OWNER,
        relation_exact: true,
        protected_after: 1,
        owner_after: 1,
        ..EffectiveLockTraceProjection::default()
    };
    assert!(production_body_ownership_preserves_effective_lock_kernel(
        ownership
    ));
    assert_eq!(
        check_production_body_ownership_effective_lock_transition(ownership)
            .expect("exact body owner trace mints checked evidence")
            .into_projection(),
        ownership,
    );
    assert_eq!(ownership.owner_after, 1);
    assert!(ownership.protected_after >= ownership.protected_before);
    assert_eq!(ownership.owner_reused, ownership.owner_before == 1);
    assert!(!production_body_ownership_preserves_effective_lock_kernel(
        EffectiveLockTraceProjection {
            owner_reused: true,
            ..ownership
        }
    ));
    assert!(
        check_production_body_ownership_effective_lock_transition(EffectiveLockTraceProjection {
            owner_reused: true,
            ..ownership
        })
        .is_none()
    );

    let retirement = EffectiveLockTraceProjection {
        kind: EFFECTIVE_LOCK_TRACE_RETIRE,
        relation_exact: true,
        ready_before: 13,
        retired_retained: 3,
        retired_ready: 4,
        ready_after: 6,
        store_before: 11,
        retired_store: 5,
        store_after: 6,
        ..EffectiveLockTraceProjection::default()
    };
    assert!(production_body_capacity_retirement_preserves_effective_lock_kernel(retirement));
    assert_eq!(
        check_production_body_capacity_retirement_effective_lock_transition(retirement)
            .expect("exact body retirement trace mints checked evidence")
            .into_projection(),
        retirement,
    );
    assert_eq!(
        retirement.ready_after,
        retirement.ready_before - retirement.retired_retained - retirement.retired_ready
    );
    assert_eq!(
        retirement.store_after,
        retirement.store_before - retirement.retired_store
    );
    assert!(
        !production_body_capacity_retirement_preserves_effective_lock_kernel(
            EffectiveLockTraceProjection {
                ready_after: 7,
                ..retirement
            }
        )
    );
    assert!(
        check_production_body_capacity_retirement_effective_lock_transition(
            EffectiveLockTraceProjection {
                ready_after: 7,
                ..retirement
            }
        )
        .is_none()
    );

    let service = EffectiveLockTraceProjection {
        kind: EFFECTIVE_LOCK_TRACE_SERVICE,
        relation_exact: true,
        cursor_before: SERVICE_CLASS_COMPLETION,
        completion_ready: true,
        progress_ready: true,
        selected: SERVICE_CLASS_COMPLETION,
        cursor_after: SERVICE_CLASS_PROGRESS,
        ..EffectiveLockTraceProjection::default()
    };
    assert!(production_body_service_refines_async_fairness_kernel(
        service
    ));
    assert_eq!(
        check_production_body_service_effective_lock_transition(service)
            .expect("exact service trace mints checked evidence")
            .into_projection(),
        service,
    );
    assert!((SERVICE_CLASS_COMPLETION..=SERVICE_CLASS_NORMAL).contains(&service.selected));
    assert!((SERVICE_CLASS_COMPLETION..=SERVICE_CLASS_NORMAL).contains(&service.cursor_after));
    assert!(service.completion_ready);
    assert!(!production_body_service_refines_async_fairness_kernel(
        EffectiveLockTraceProjection {
            selected: SERVICE_CLASS_PROGRESS,
            ..service
        }
    ));
    assert!(
        check_production_body_service_effective_lock_transition(EffectiveLockTraceProjection {
            selected: SERVICE_CLASS_PROGRESS,
            ..service
        })
        .is_none()
    );
}

fn capability(kind: u8, nonce: u64) -> EffectCapabilityKey {
    EffectCapabilityKey {
        kind,
        persistence_id: nonce,
        ..EffectCapabilityKey::default()
    }
}

fn durable_begin_trace() -> ProductionDurableIntentTraceProjection {
    let context = ContextId::repeat(1);
    let subject = Subject::repeat(2);
    let context_identity = CanonicalIdentityProjection::from_bytes(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_CONSENSUS_CONTEXT,
        *context.as_bytes(),
    );
    let subject_identity = CanonicalIdentityProjection::from_bytes(
        IDENTITY_DOMAIN_SUBJECT,
        IDENTITY_KIND_CONSENSUS_SUBJECT,
        *subject.as_bytes(),
    );
    let tag = TagProjection {
        height: 4,
        view: 2,
        generation: 3,
    };
    let boundary = BoundaryCapabilityKey {
        kind: BOUNDARY_BEGIN_WAL,
        record_kind: WAL_RECORD_PROPOSAL_INTENT,
        continuation: CONTINUATION_SIGN,
        persistence_id: 8,
        context_id: context,
        context_identity,
        tag,
        subject: SubjectProjection {
            present: true,
            subject,
        },
        subject_identity,
        proposal_present: true,
        proposal_height: tag.height,
        proposal_view: tag.view,
        ..BoundaryCapabilityKey::none()
    };
    let persist = EffectCapabilityKey {
        kind: EFFECT_PERSIST,
        tag,
        context_id: context,
        height: tag.height,
        view: tag.view,
        proposal_height: tag.height,
        proposal_view: tag.view,
        subject,
        persistence_id: boundary.persistence_id,
        record_kind: boundary.record_kind,
        ..EffectCapabilityKey::default()
    };
    let mut effects = EffectTrace::empty();
    assert!(effects.push(persist, persist));
    ProductionDurableIntentTraceProjection {
        event_tag: tag,
        owner_tag_before: tag,
        owner_tag_after: tag,
        event_kind: 0,
        event_persistence_id: 0,
        pending_before: PendingProjection::default(),
        pending_after: PendingProjection {
            record_kind: boundary.record_kind,
            continuation: boundary.continuation,
            persistence_id: boundary.persistence_id,
            context_id: context_identity,
            height: tag.height,
            view: tag.view,
            proposal_present: true,
            proposal_height: tag.height,
            proposal_view: tag.view,
            subject: subject_identity,
        },
        boundary_claimed: boundary,
        boundary_granted: boundary,
        effects,
        durable_sequence_before: 7,
        durable_sequence_after: 7,
    }
}

fn lock_and_commit_begin_trace() -> ProductionDurableIntentTraceProjection {
    let mut trace = durable_begin_trace();
    let proposal_view = trace.owner_tag_before.view;
    trace.event_kind = 10;
    trace.pending_after.record_kind = WAL_RECORD_LOCK_AND_COMMIT;
    trace.pending_after.proposal_present = true;
    trace.pending_after.proposal_height = trace.owner_tag_before.height;
    trace.pending_after.proposal_view = proposal_view;
    trace.boundary_claimed.record_kind = WAL_RECORD_LOCK_AND_COMMIT;
    trace.boundary_claimed.proposal_present = true;
    trace.boundary_claimed.proposal_height = trace.owner_tag_before.height;
    trace.boundary_claimed.proposal_view = proposal_view;
    trace.boundary_claimed.auxiliary_present = true;
    trace.boundary_claimed.auxiliary_context_id = trace.boundary_claimed.context_id;
    trace.boundary_claimed.auxiliary_height = trace.owner_tag_before.height;
    trace.boundary_claimed.auxiliary_view = proposal_view;
    trace.boundary_claimed.auxiliary_proposal_height = trace.owner_tag_before.height;
    trace.boundary_claimed.auxiliary_proposal_view = proposal_view;
    trace.boundary_claimed.auxiliary_phase = 1;
    trace.boundary_claimed.auxiliary_subject = trace.boundary_claimed.subject.subject;
    trace.boundary_granted = trace.boundary_claimed;

    let persist = {
        let persist = &mut trace.effects.slot0.requested;
        persist.record_kind = WAL_RECORD_LOCK_AND_COMMIT;
        persist.proposal_height = trace.owner_tag_before.height;
        persist.proposal_view = proposal_view;
        persist.phase = 2;
        persist.auxiliary_present = true;
        persist.auxiliary_context_id = trace.boundary_claimed.auxiliary_context_id;
        persist.auxiliary_height = trace.boundary_claimed.auxiliary_height;
        persist.auxiliary_view = trace.boundary_claimed.auxiliary_view;
        persist.auxiliary_proposal_height = trace.boundary_claimed.auxiliary_proposal_height;
        persist.auxiliary_proposal_view = trace.boundary_claimed.auxiliary_proposal_view;
        persist.auxiliary_phase = trace.boundary_claimed.auxiliary_phase;
        persist.auxiliary_subject = trace.boundary_claimed.auxiliary_subject;
        *persist
    };
    trace.effects.slot0.granted = persist;
    trace
}

fn push_authorized(trace: &mut EffectTrace, kind: u8) -> bool {
    let key = capability(kind, u64::from(trace.len) + 1);
    trace.push(key, key)
}

fn base_facts() -> TransitionFacts {
    let volatile = VolatileSummary {
        durable_signable_limit: 1,
        ..VolatileSummary::default()
    };
    TransitionFacts {
        before_invariant: true,
        after_invariant: true,
        context_unchanged: true,
        whole_state_unchanged: true,
        tag_matches: true,
        busy_fence_open: true,
        event_kind: 7,
        action_kind: ACTION_STUTTER,
        wal_record_kind: WAL_RECORD_NONE,
        signed_message_kind: SIGNED_MESSAGE_NONE,
        replay_effect_kind: REPLAY_EFFECT_NONE,
        validator_count: 4,
        volatile_before: volatile,
        volatile_after: volatile,
        durable_unchanged: true,
        pending_unchanged: true,
        generation_unchanged: true,
        application_unchanged: true,
        begin_persist_exact: false,
        acknowledge_persist_exact: false,
        application_transition_exact: true,
        acknowledgement_continuation: CONTINUATION_NONE,
        install_view_unchanged: false,
        timeout_vote_pool_unchanged: true,
        formed_timeouts_unchanged: true,
        timeout_control_unchanged: true,
        timeout_control_after_absent: true,
        enter_view_exact: true,
        effects: EffectTrace::empty(),
    }
}

fn owner(
    height: u64,
    view: u64,
    generation: u64,
    key: u64,
    manifest_hash: Option<u64>,
) -> ExactBodyOwnerProjection<u64, u64> {
    ExactBodyOwnerProjection {
        tag: TagProjection {
            height,
            view,
            generation,
        },
        key,
        manifest_hash,
    }
}

#[test]
fn exact_body_owner_binding_rejects_stale_generation_and_conflicting_evidence() {
    let current = owner(9, 4, 7, 11, Some(23));
    for conflicting in [
        owner(10, 4, 7, 11, Some(23)),
        owner(9, 5, 7, 11, Some(23)),
        owner(9, 4, 7, 12, Some(23)),
    ] {
        assert!(
            plan_exact_body_owner_binding(Some(current), conflicting).is_none(),
            "height, view, and round/subject identity are immutable"
        );
    }
    assert!(
        plan_exact_body_owner_binding(Some(current), owner(9, 4, 8, 11, Some(23))).is_none(),
        "a different generation cannot overwrite an exact owner"
    );
    assert!(
        plan_exact_body_owner_binding(Some(current), owner(9, 4, 7, 11, Some(24))).is_none(),
        "a different manifest identity cannot overwrite an exact owner"
    );

    let enriched =
        plan_exact_body_owner_binding(Some(owner(9, 4, 7, 11, None)), owner(9, 4, 7, 11, Some(23)))
            .expect("one certified fetch may acquire its exact manifest identity");
    assert!(enriched.already_owned);
    assert_eq!(enriched.owner, current);
}

#[test]
fn exact_body_owner_rebind_preserves_key_and_evidence_and_advances_incarnation() {
    let previous = owner(9, 4, 7, 11, Some(23));
    let rebound = plan_exact_body_owner_rebind(
        previous,
        previous,
        TagProjection {
            height: 9,
            view: 5,
            generation: 0,
        },
    )
    .expect("later-view generation reset is accepted");
    assert_eq!(rebound.key, previous.key);
    assert_eq!(rebound.manifest_hash, previous.manifest_hash);

    let same_view = plan_exact_body_owner_rebind(
        previous,
        previous,
        TagProjection {
            height: 9,
            view: 4,
            generation: 8,
        },
    )
    .expect("same-view higher-generation rebind is accepted");
    assert_eq!(same_view.key, previous.key);
    assert_eq!(same_view.manifest_hash, previous.manifest_hash);

    for wrong in [
        TagProjection {
            height: 10,
            view: 5,
            generation: 0,
        },
        TagProjection {
            height: 9,
            view: 3,
            generation: 8,
        },
        TagProjection {
            height: 9,
            view: 4,
            generation: 7,
        },
    ] {
        assert!(plan_exact_body_owner_rebind(previous, previous, wrong).is_none());
    }
    assert!(
        plan_exact_body_owner_rebind(
            previous,
            owner(9, 4, 7, 12, Some(23)),
            TagProjection {
                height: 9,
                view: 5,
                generation: 0,
            },
        )
        .is_none(),
        "a wrong round/subject owner cannot be rebound"
    );
    assert!(
        plan_exact_body_owner_rebind(
            previous,
            owner(9, 4, 7, 11, Some(24)),
            TagProjection {
                height: 9,
                view: 5,
                generation: 0,
            },
        )
        .is_none(),
        "a conflicting manifest identity cannot be rebound"
    );
    assert!(
        plan_exact_body_owner_rebind(
            previous,
            owner(9, 3, 6, 11, Some(23)),
            TagProjection {
                height: 9,
                view: 5,
                generation: 0,
            },
        )
        .is_none(),
        "the previous stage tag must be the exact installed owner"
    );
}

#[test]
fn exact_body_completion_classifier_rejects_duplicate_or_conflicting_owners() {
    for ingress_owners in 0..=2 {
        for ingress_exact in 0..=2 {
            for deferred_owners in 0..=2 {
                for deferred_exact in 0..=2 {
                    let expected = match (
                        ingress_owners,
                        ingress_exact,
                        deferred_owners,
                        deferred_exact,
                    ) {
                        (0, 0, 0, 0) => ExactBodyCompletionOwnership::Vacant,
                        (1, 1, 0, 0) | (0, 0, 1, 1) => ExactBodyCompletionOwnership::Exact,
                        _ => ExactBodyCompletionOwnership::Invalid,
                    };
                    assert_eq!(
                        classify_exact_body_completion_ownership(
                            ingress_owners,
                            ingress_exact,
                            deferred_owners,
                            deferred_exact,
                        ),
                        expected,
                    );
                }
            }
        }
    }
}

#[test]
fn exact_body_retirement_accounting_rejects_capacity_leakage() {
    let accounting = plan_exact_body_retirement_accounting(100, 20, 30, 80, 35)
        .expect("exact owned bytes fit both counters");
    assert_eq!(accounting.ready_after, 50);
    assert_eq!(accounting.store_after, 45);
    assert!(plan_exact_body_retirement_accounting(49, 20, 30, 80, 35).is_none());
    assert!(plan_exact_body_retirement_accounting(100, 20, 30, 34, 35).is_none());
    assert_eq!(
        plan_exact_body_retirement_accounting(u64::MAX, 0, 0, u64::MAX, 0),
        Some(ExactBodyRetirementAccounting {
            ready_after: u64::MAX,
            store_after: u64::MAX,
        })
    );
    assert_eq!(
        plan_exact_body_retirement_accounting(u64::MAX, u64::MAX, 0, 0, 0),
        Some(ExactBodyRetirementAccounting {
            ready_after: 0,
            store_after: 0,
        })
    );
    assert!(
        plan_exact_body_retirement_accounting(u64::MAX, u64::MAX, u64::MAX, 0, 0).is_none(),
        "sequential retirement rejects an overflowing combined claim"
    );
}

#[test]
fn bounded_service_kernel_exhaustively_selects_each_readiness_combination() {
    let classes = [
        SERVICE_CLASS_COMPLETION,
        SERVICE_CLASS_PROGRESS,
        SERVICE_CLASS_NORMAL,
    ];
    for cursor in classes {
        for ready_mask in 0u8..8 {
            let completion_ready = ready_mask & 0b001 != 0;
            let progress_ready = ready_mask & 0b010 != 0;
            let normal_ready = ready_mask & 0b100 != 0;
            let ready = |class| match class {
                SERVICE_CLASS_COMPLETION => completion_ready,
                SERVICE_CLASS_PROGRESS => progress_ready,
                SERVICE_CLASS_NORMAL => normal_ready,
                _ => false,
            };
            let cursor_index = classes
                .iter()
                .position(|class| *class == cursor)
                .expect("cursor is one of the three classes");
            let expected = (0..3)
                .map(|offset| classes[(cursor_index + offset) % 3])
                .find(|class| ready(*class));
            let selection = select_bounded_service_class(
                cursor,
                completion_ready,
                progress_ready,
                normal_ready,
            );
            assert_eq!(selection.selected, expected.unwrap_or(SERVICE_CLASS_NONE));
            let expected_next = expected.map_or(cursor, |selected| match selected {
                SERVICE_CLASS_COMPLETION => SERVICE_CLASS_PROGRESS,
                SERVICE_CLASS_PROGRESS => SERVICE_CLASS_NORMAL,
                SERVICE_CLASS_NORMAL => SERVICE_CLASS_COMPLETION,
                _ => unreachable!("selected class came from the canonical set"),
            });
            assert_eq!(selection.next, expected_next);
        }
    }

    let first = select_bounded_service_class(SERVICE_CLASS_COMPLETION, true, true, true);
    let second = select_bounded_service_class(first.next, true, true, true);
    let third = select_bounded_service_class(second.next, true, true, true);
    assert_eq!(
        [first.selected, second.selected, third.selected],
        [
            SERVICE_CLASS_COMPLETION,
            SERVICE_CLASS_PROGRESS,
            SERVICE_CLASS_NORMAL,
        ]
    );
    assert_eq!(third.next, SERVICE_CLASS_COMPLETION);

    for invalid_cursor in [0, 4, 99, u8::MAX] {
        let invalid = select_bounded_service_class(invalid_cursor, true, true, true);
        assert_eq!(invalid.selected, SERVICE_CLASS_NONE);
        assert_eq!(invalid.next, SERVICE_CLASS_NONE);
    }
}

#[test]
fn source_linked_effective_lock_body_kernels_reject_adversarial_inputs() {
    exact_body_owner_binding_rejects_stale_generation_and_conflicting_evidence();
    exact_body_owner_rebind_preserves_key_and_evidence_and_advances_incarnation();
    exact_body_completion_classifier_rejects_duplicate_or_conflicting_owners();
    exact_body_retirement_accounting_rejects_capacity_leakage();
    bounded_service_kernel_exhaustively_selects_each_readiness_combination();
}

#[test]
fn stutter_and_exact_begin_are_accepted() {
    assert!(accepts_facts(base_facts()));

    let mut facts = base_facts();
    facts.action_kind = ACTION_BEGIN_WAL;
    facts.wal_record_kind = WAL_RECORD_PROPOSAL_INTENT;
    facts.pending_unchanged = false;
    facts.begin_persist_exact = true;
    assert!(push_authorized(&mut facts.effects, EFFECT_PERSIST));
    assert!(accepts_facts(facts));
}

#[test]
fn unauthorized_or_misordered_effects_fail_closed() {
    let mut unauthorized = base_facts();
    assert!(unauthorized.effects.push(
        capability(EFFECT_BROADCAST, 1),
        capability(EFFECT_BROADCAST, 2),
    ));
    assert!(!accepts_facts(unauthorized));

    let mut signing_not_last = base_facts();
    signing_not_last.event_kind = EVENT_SIGNED;
    assert!(push_authorized(&mut signing_not_last.effects, EFFECT_SIGN));
    assert!(push_authorized(
        &mut signing_not_last.effects,
        EFFECT_BROADCAST
    ));
    assert!(!accepts_facts(signing_not_last));

    let mut persist_and_sign = base_facts();
    persist_and_sign.action_kind = ACTION_BEGIN_WAL;
    persist_and_sign.wal_record_kind = WAL_RECORD_PROPOSAL_INTENT;
    persist_and_sign.pending_unchanged = false;
    persist_and_sign.begin_persist_exact = true;
    assert!(push_authorized(
        &mut persist_and_sign.effects,
        EFFECT_PERSIST
    ));
    assert!(push_authorized(&mut persist_and_sign.effects, EFFECT_SIGN));
    assert!(!accepts_facts(persist_and_sign));

    struct OpaqueOrderToken(&'static str);

    let mut pending = VecDeque::from([
        OpaqueOrderToken("old-tail-0"),
        OpaqueOrderToken("old-tail-1"),
    ]);
    prepend_causal_continuation(
        &mut pending,
        vec![
            OpaqueOrderToken("continuation-0"),
            OpaqueOrderToken("continuation-1"),
            OpaqueOrderToken("continuation-2"),
        ],
    );
    assert_eq!(
        pending.into_iter().map(|token| token.0).collect::<Vec<_>>(),
        [
            "continuation-0",
            "continuation-1",
            "continuation-2",
            "old-tail-0",
            "old-tail-1",
        ],
        "persisted continuation order is causal FIFO order"
    );

    let mut forward_iteration_mutant = VecDeque::from([
        OpaqueOrderToken("old-tail-0"),
        OpaqueOrderToken("old-tail-1"),
    ]);
    for item in [
        OpaqueOrderToken("continuation-0"),
        OpaqueOrderToken("continuation-1"),
        OpaqueOrderToken("continuation-2"),
    ] {
        forward_iteration_mutant.push_front(item);
    }
    assert_eq!(
        forward_iteration_mutant
            .into_iter()
            .map(|token| token.0)
            .collect::<Vec<_>>(),
        [
            "continuation-2",
            "continuation-1",
            "continuation-0",
            "old-tail-0",
            "old-tail-1",
        ],
        "the compact forward-iteration mutant reverses the continuation"
    );
}

#[test]
fn stale_or_busy_input_must_be_an_exact_empty_stutter() {
    let mut stale = base_facts();
    stale.tag_matches = false;
    assert!(accepts_facts(stale));

    stale.application_transition_exact = false;
    stale.application_unchanged = false;
    assert!(!accepts_facts(stale));

    let mut busy = base_facts();
    busy.busy_fence_open = false;
    assert!(push_authorized(&mut busy.effects, EFFECT_FETCH));
    assert!(!accepts_facts(busy));
}

#[test]
fn trace_capacity_is_fail_closed() {
    let mut trace = EffectTrace::empty();
    for _ in 0..MAX_EFFECTS_PER_STEP {
        assert!(push_authorized(&mut trace, EFFECT_BROADCAST));
    }
    assert!(!push_authorized(&mut trace, EFFECT_BROADCAST));
}

#[test]
fn volatile_bounds_and_action_record_pairs_fail_closed() {
    let mut too_many_vote_pools = base_facts();
    too_many_vote_pools.volatile_after.vote_pools = 3;
    assert!(!accepts_facts(too_many_vote_pools));

    let mut invented_signature = base_facts();
    invented_signature.volatile_before.awaiting_signature = true;
    invented_signature.volatile_after.awaiting_signature = true;
    invented_signature.volatile_before.durable_signable_limit = 0;
    invented_signature.volatile_after.durable_signable_limit = 0;
    assert!(!accepts_facts(invented_signature));

    let mut bad_ack = base_facts();
    bad_ack.action_kind = ACTION_ACKNOWLEDGE_WAL;
    bad_ack.wal_record_kind = WAL_RECORD_DECISION;
    bad_ack.event_kind = EVENT_PERSISTED;
    bad_ack.pending_unchanged = false;
    bad_ack.acknowledge_persist_exact = true;
    bad_ack.acknowledgement_continuation = CONTINUATION_INSTALL_TIMEOUT;
    assert!(!accepts_facts(bad_ack));

    let mut same_round_install = base_facts();
    same_round_install.whole_state_unchanged = false;
    same_round_install.action_kind = ACTION_ACKNOWLEDGE_WAL;
    same_round_install.wal_record_kind = WAL_RECORD_INSTALL_TIMEOUT;
    same_round_install.event_kind = EVENT_PERSISTED;
    same_round_install.durable_unchanged = false;
    same_round_install.pending_unchanged = false;
    same_round_install.generation_unchanged = false;
    same_round_install.acknowledge_persist_exact = true;
    same_round_install.acknowledgement_continuation = CONTINUATION_INSTALL_TIMEOUT;
    same_round_install.install_view_unchanged = true;
    same_round_install.volatile_before.timeout_vote_pools = 1;
    same_round_install.volatile_before.timeout_vote_entries = 2;
    same_round_install.volatile_after.timeout_vote_pools = 1;
    same_round_install.volatile_after.timeout_vote_entries = 2;
    assert!(push_authorized(
        &mut same_round_install.effects,
        EFFECT_ENTER_VIEW
    ));
    assert!(
        accepts_facts(same_round_install),
        "a lock-only TC install preserves the exact current timeout pool"
    );
    let mut full_same_round_control = same_round_install;
    full_same_round_control.volatile_after.outbound_control = 4;
    assert!(
        accepts_facts(full_same_round_control),
        "install may retain CommitVote, PrepareQC, TimeoutVote, and TC"
    );
    let mut overflowing_same_round_control = full_same_round_control;
    overflowing_same_round_control
        .volatile_after
        .outbound_control = 5;
    assert!(!accepts_facts(overflowing_same_round_control));

    let mut erased_same_round_pool = same_round_install;
    erased_same_round_pool.volatile_after.timeout_vote_pools = 0;
    erased_same_round_pool.volatile_after.timeout_vote_entries = 0;
    assert!(!accepts_facts(erased_same_round_pool));

    let mut substituted_same_size_pool = same_round_install;
    substituted_same_size_pool.timeout_vote_pool_unchanged = false;
    assert!(!accepts_facts(substituted_same_size_pool));

    let mut substituted_formed_marker = same_round_install;
    substituted_formed_marker.formed_timeouts_unchanged = false;
    assert!(!accepts_facts(substituted_formed_marker));

    let mut substituted_timeout_control = same_round_install;
    substituted_timeout_control.timeout_control_unchanged = false;
    assert!(!accepts_facts(substituted_timeout_control));

    let mut advancing_install_keeps_old_pool = same_round_install;
    advancing_install_keeps_old_pool.install_view_unchanged = false;
    assert!(!accepts_facts(advancing_install_keeps_old_pool));

    let mut advancing_install = advancing_install_keeps_old_pool;
    advancing_install.volatile_after.timeout_vote_pools = 0;
    advancing_install.volatile_after.timeout_vote_entries = 0;
    assert!(
        accepts_facts(advancing_install),
        "an advancing TC install clears timeout pools, markers, and control"
    );

    let mut advancing_install_keeps_timeout_control = advancing_install;
    advancing_install_keeps_timeout_control.timeout_control_after_absent = false;
    assert!(!accepts_facts(advancing_install_keeps_timeout_control));
}

#[test]
fn decision_ack_retires_competing_owners_and_keeps_one_body_pipeline() {
    let mut terminal = base_facts();
    terminal.action_kind = ACTION_ACKNOWLEDGE_WAL;
    terminal.wal_record_kind = WAL_RECORD_DECISION;
    terminal.event_kind = EVENT_PERSISTED;
    terminal.pending_unchanged = false;
    terminal.acknowledge_persist_exact = true;
    terminal.acknowledgement_continuation = CONTINUATION_DECIDE;
    terminal.volatile_before.body_work = 2;
    terminal.volatile_after.body_work = 1;
    terminal.volatile_after.outbound_control = 1;
    terminal.volatile_after.durable_signable_limit = 0;
    assert!(accepts_facts(terminal));

    let mut stale_pipeline = terminal;
    stale_pipeline.volatile_after.body_work = 2;
    assert!(!accepts_facts(stale_pipeline));

    let mut stale_candidate = terminal;
    stale_candidate.volatile_after.candidate_present = true;
    assert!(!accepts_facts(stale_candidate));

    let mut stale_signature = terminal;
    stale_signature.volatile_after.signature_queue = 1;
    stale_signature.volatile_after.durable_signable_limit = 1;
    assert!(!accepts_facts(stale_signature));

    let mut missing_pipeline = terminal;
    missing_pipeline.volatile_before.body_work = 0;
    assert!(!accepts_facts(missing_pipeline));

    let mut dropped_pipeline = terminal;
    dropped_pipeline.volatile_after.body_work = 0;
    assert!(!accepts_facts(dropped_pipeline));
}

#[test]
fn body_pipeline_classifier_rejects_non_pipeline_effects() {
    let mut stored = base_facts();
    stored.action_kind = ACTION_BODY_PROGRESS;
    stored.event_kind = EVENT_BODY_AVAILABLE;
    assert!(push_authorized(&mut stored.effects, EFFECT_STORE));
    assert!(accepts_facts(stored));

    let mut validated = base_facts();
    validated.action_kind = ACTION_BODY_PROGRESS;
    validated.event_kind = 10;
    assert!(push_authorized(&mut validated.effects, EFFECT_REPORT));
    assert!(accepts_facts(validated));

    let mut invented_broadcast = validated;
    invented_broadcast.effects = EffectTrace::empty();
    assert!(push_authorized(
        &mut invented_broadcast.effects,
        EFFECT_BROADCAST
    ));
    assert!(!accepts_facts(invented_broadcast));

    let mut invented_fetch = validated;
    invented_fetch.effects = EffectTrace::empty();
    assert!(push_authorized(&mut invented_fetch.effects, EFFECT_FETCH));
    assert!(!accepts_facts(invented_fetch));
}

#[test]
fn retransmit_may_reconstruct_one_final_decision_body_stage() {
    let mut store_retry = base_facts();
    store_retry.action_kind = ACTION_VOLATILE_PROTOCOL;
    store_retry.event_kind = 7;
    for _ in 0..7 {
        assert!(push_authorized(&mut store_retry.effects, EFFECT_BROADCAST));
    }
    assert!(push_authorized(&mut store_retry.effects, EFFECT_STORE));
    assert!(accepts_facts(store_retry));

    let mut validate_retry = base_facts();
    validate_retry.action_kind = ACTION_VOLATILE_PROTOCOL;
    validate_retry.event_kind = 7;
    assert!(push_authorized(
        &mut validate_retry.effects,
        EFFECT_BROADCAST
    ));
    assert!(push_authorized(
        &mut validate_retry.effects,
        EFFECT_VALIDATE
    ));
    assert!(accepts_facts(validate_retry));

    let mut not_final = validate_retry;
    not_final.effects = EffectTrace::empty();
    assert!(push_authorized(&mut not_final.effects, EFFECT_VALIDATE));
    assert!(push_authorized(&mut not_final.effects, EFFECT_BROADCAST));
    assert!(!accepts_facts(not_final));

    let mut mixed_stages = validate_retry;
    mixed_stages.effects = EffectTrace::empty();
    assert!(push_authorized(&mut mixed_stages.effects, EFFECT_STORE));
    assert!(push_authorized(&mut mixed_stages.effects, EFFECT_VALIDATE));
    assert!(!accepts_facts(mixed_stages));

    let mut fetch_and_store = validate_retry;
    fetch_and_store.effects = EffectTrace::empty();
    assert!(push_authorized(&mut fetch_and_store.effects, EFFECT_FETCH));
    assert!(push_authorized(&mut fetch_and_store.effects, EFFECT_STORE));
    assert!(!accepts_facts(fetch_and_store));

    let mut report_and_store = validate_retry;
    report_and_store.effects = EffectTrace::empty();
    assert!(push_authorized(
        &mut report_and_store.effects,
        EFFECT_REPORT
    ));
    assert!(push_authorized(&mut report_and_store.effects, EFFECT_STORE));
    assert!(!accepts_facts(report_and_store));

    let mut apply_and_fetch = validate_retry;
    apply_and_fetch.effects = EffectTrace::empty();
    assert!(push_authorized(&mut apply_and_fetch.effects, EFFECT_APPLY));
    assert!(push_authorized(&mut apply_and_fetch.effects, EFFECT_FETCH));
    assert!(!accepts_facts(apply_and_fetch));

    let mut fetch_not_final = validate_retry;
    fetch_not_final.effects = EffectTrace::empty();
    assert!(push_authorized(&mut fetch_not_final.effects, EFFECT_FETCH));
    assert!(push_authorized(
        &mut fetch_not_final.effects,
        EFFECT_BROADCAST
    ));
    assert!(!accepts_facts(fetch_not_final));

    let mut wrong_event = validate_retry;
    wrong_event.event_kind = 6;
    assert!(!accepts_facts(wrong_event));
}

#[test]
fn signed_classifier_and_inactive_slots_are_canonical() {
    let mut invented_signed_transition = base_facts();
    invented_signed_transition.event_kind = EVENT_SIGNED;
    invented_signed_transition.action_kind = ACTION_VOLATILE_PROTOCOL;
    assert!(!accepts_facts(invented_signed_transition));

    let mut noncanonical_empty = base_facts();
    noncanonical_empty.effects.slot0 = EffectSlotProjection {
        kind: EFFECT_BROADCAST,
        requested: EffectCapabilityKey::none(),
        granted: EffectCapabilityKey::none(),
    };
    assert!(!accepts_facts(noncanonical_empty));

    let mut impossible_roster = base_facts();
    impossible_roster.validator_count = u64::MAX / 2 + 1;
    assert!(!accepts_facts(impossible_roster));
}

#[test]
fn replay_resume_has_a_distinct_one_shot_effect_relation() {
    let mut resumed = base_facts();
    resumed.event_kind = EVENT_RESUME_AFTER_REPLAY;
    resumed.action_kind = ACTION_RESUME_AFTER_REPLAY;
    resumed.replay_effect_kind = REPLAY_EFFECT_PREPARE;
    resumed.volatile_after.replay_resumed = true;
    resumed.volatile_after.awaiting_signature = true;
    assert!(push_authorized(&mut resumed.effects, EFFECT_SIGN));
    assert!(accepts_facts(resumed));

    let mut stale_did_work = resumed;
    stale_did_work.tag_matches = false;
    assert!(!accepts_facts(stale_did_work));

    let mut replayed_twice = resumed;
    replayed_twice.volatile_before.replay_resumed = true;
    assert!(!accepts_facts(replayed_twice));

    let mut decision_fetch = base_facts();
    decision_fetch.event_kind = EVENT_RESUME_AFTER_REPLAY;
    decision_fetch.action_kind = ACTION_RESUME_AFTER_REPLAY;
    decision_fetch.replay_effect_kind = REPLAY_EFFECT_DECISION;
    decision_fetch.volatile_after.replay_resumed = true;
    decision_fetch.volatile_after.body_work = 1;
    assert!(push_authorized(&mut decision_fetch.effects, EFFECT_FETCH));
    assert!(accepts_facts(decision_fetch));
}
