/// Extract the sole terminal economic owner from a valid composed state.
///
/// Commit cleanup leaves the effect owned only by canonical WSV. Ordered and
/// direct release leave the transaction owned only by ordinary FIFO.
/// Non-terminal and malformed states have no terminal owner.
#[allow(dead_code)] // Consumed by the verification harness and refinement tests.
pub(crate) const fn production_in_flight_first_release_terminal_owner(
    projection: ProductionInFlightFirstReleaseStateProjection,
) -> Option<ProductionInFlightFirstReleaseTerminalOwnerProjection> {
    if !production_in_flight_first_release_state_kernel(projection) {
        None
    } else if projection.queue.reservation_state
        == IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN
        && projection.history.reservation_commit_forgotten_prefix == projection.queue.selected_count
    {
        Some(ProductionInFlightFirstReleaseTerminalOwnerProjection {
            ordinary_fifo_owner: false,
            canonical_wsv_owner: true,
            commit_terminal: true,
            release_terminal: false,
        })
    } else if projection.queue.reservation_state
        == IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN
        || projection.queue.reservation_state == IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED
    {
        Some(ProductionInFlightFirstReleaseTerminalOwnerProjection {
            ordinary_fifo_owner: true,
            canonical_wsv_owner: false,
            commit_terminal: false,
            release_terminal: true,
        })
    } else {
        None
    }
}
/// Check an applied-predecessor successor transition and mint opaque evidence
/// only for an accepted projection.
#[must_use]
pub(crate) fn check_production_applied_successor_transition(
    projection: ProductionAppliedSuccessorTraceProjection,
) -> Option<CheckedProductionTransition<ProductionAppliedSuccessorTraceProjection>> {
    if production_applied_successor_trace_refines_indexed_activation_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check a recovered successor transition and mint opaque evidence only for
/// an accepted projection.
#[must_use]
pub(crate) fn check_production_recovered_successor_transition(
    projection: ProductionRecoveredSuccessorTraceProjection,
) -> Option<CheckedProductionTransition<ProductionRecoveredSuccessorTraceProjection>> {
    if production_recovered_successor_trace_refines_indexed_activation_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one successor startup lifecycle transition.
#[must_use]
pub(crate) fn check_production_successor_startup_lifecycle_transition(
    projection: ProductionSuccessorStartupLifecycleProjection,
) -> Option<CheckedProductionTransition<ProductionSuccessorStartupLifecycleProjection>> {
    if production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one authenticated historical certificate handoff.
#[must_use]
pub(crate) fn check_production_historical_certificate_transition(
    projection: ProductionHistoricalCertificateTraceProjection,
) -> Option<CheckedProductionTransition<ProductionHistoricalCertificateTraceProjection>> {
    if production_historical_certificate_trace_refines_indexed_async_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one authenticated historical body-pipeline handoff.
#[must_use]
pub(crate) fn check_production_historical_body_pipeline_transition(
    projection: ProductionHistoricalBodyPipelineTraceProjection,
) -> Option<CheckedProductionTransition<ProductionHistoricalBodyPipelineTraceProjection>> {
    if production_historical_body_pipeline_trace_refines_indexed_async_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one reducer durable-intent transition.
#[must_use]
pub(crate) fn check_production_durable_intent_transition(
    projection: ProductionDurableIntentTraceProjection,
) -> Option<CheckedProductionTransition<ProductionDurableIntentTraceProjection>> {
    if production_durable_intent_trace_refines_progress_witness_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one pending-Decision recovery transition.
#[must_use]
pub(crate) fn check_production_decision_recovery_transition(
    projection: ProductionDecisionRecoveryTraceProjection,
) -> Option<CheckedProductionTransition<ProductionDecisionRecoveryTraceProjection>> {
    if production_decision_trace_refines_recovery_witness_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one protected scheduler selection.
#[must_use]
pub(crate) fn check_production_scheduler_transition(
    projection: ProductionSchedulerTraceProjection,
) -> Option<CheckedProductionTransition<ProductionSchedulerTraceProjection>> {
    if production_scheduler_trace_refines_protected_ownership_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one bounded ingress admission before queue mutation.
#[must_use]
pub(crate) fn check_production_ingress_transition(
    projection: ProductionIngressIdentityAndClassTraceProjection,
) -> Option<CheckedProductionTransition<ProductionIngressIdentityAndClassTraceProjection>> {
    if production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check replacement of one exact unpublished reservation by its physical
/// reducer command without minting or consuming another occupied slot.
#[must_use]
pub(crate) fn check_production_ingress_reservation_materialization_transition(
    projection: ProductionIngressReservationMaterializationTraceProjection,
) -> Option<CheckedProductionTransition<ProductionIngressReservationMaterializationTraceProjection>>
{
    if production_ingress_reservation_materialization_refines_protected_ownership_kernel(projection)
    {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one effect-to-candidate handoff before retaining its producer episode
/// or publishing a new asynchronous owner.
#[must_use]
pub(crate) fn check_production_effect_to_candidate_transition(
    projection: ProductionEffectToCandidateTraceProjection,
) -> Option<CheckedProductionTransition<ProductionEffectToCandidateTraceProjection>> {
    if production_effect_to_candidate_refines_async_ownership_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check a complete leader-wire admission and mint opaque evidence only for
/// the exact prospective lifecycle transition.
#[must_use]
pub(crate) fn check_production_leader_wire_admission_transition(
    projection: ProductionLeaderWireAdmissionTraceProjection,
) -> Option<CheckedProductionTransition<ProductionLeaderWireAdmissionTraceProjection>> {
    if production_leader_wire_admission_refines_lifecycle_ownership_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one two-stage relay retry before reinserting it.
#[must_use]
pub fn check_production_two_stage_relay_retry_transition(
    projection: ProductionTwoStageRelayRetryTraceProjection,
) -> Option<CheckedProductionTransition<ProductionTwoStageRelayRetryTraceProjection>> {
    if production_two_stage_relay_retry_trace_refines_source_fairness_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check the worker-side half of a reliable writer-flush transition.
#[must_use]
pub(crate) fn check_production_reliable_flush_worker_transition(
    projection: ProductionReliableFlushTraceProjection,
) -> Option<CheckedProductionTransition<ProductionReliableFlushTraceProjection>> {
    if production_reliable_flush_trace_refines_outbound_ownership_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check the lane-application half of a reliable writer-flush transition.
#[must_use]
pub(crate) fn check_production_reliable_flush_application_transition(
    projection: ProductionReliableFlushApplicationProjection,
) -> Option<CheckedProductionTransition<ProductionReliableFlushApplicationProjection>> {
    if production_reliable_flush_application_refines_source_lane_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check that the two halves of a reliable writer flush name the same exact
/// occurrence.
#[must_use]
pub(crate) fn check_production_reliable_flush_link_transition(
    worker: ProductionReliableFlushTraceProjection,
    application: ProductionReliableFlushApplicationProjection,
) -> Option<
    CheckedProductionTransition<(
        ProductionReliableFlushTraceProjection,
        ProductionReliableFlushApplicationProjection,
    )>,
> {
    if production_reliable_flush_two_phase_link_kernel(worker, application) {
        Some(CheckedProductionTransition::unwitnessed((
            worker,
            application,
        )))
    } else {
        None
    }
}
/// Check one durable application completion transition.
#[must_use]
pub(crate) fn check_production_application_transition(
    projection: ProductionApplicationTraceProjection,
) -> Option<CheckedProductionTransition<ProductionApplicationTraceProjection>> {
    if production_application_trace_refines_decision_completion_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check the terminal application boundary before successor construction.
#[must_use]
pub(crate) fn check_production_terminal_application_transition(
    projection: ProductionTerminalApplicationWithoutSuccessorActivationProjection,
) -> Option<
    CheckedProductionTransition<ProductionTerminalApplicationWithoutSuccessorActivationProjection>,
> {
    if production_terminal_application_without_successor_activation_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one primitive reservation-journal owner transition.
#[must_use]
pub(crate) fn check_production_in_flight_reservation_transition(
    projection: ProductionInFlightReservationTransitionProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightReservationTransitionProjection>> {
    if production_in_flight_reservation_transition_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
/// Check one complete bounded first-release carrier transition.
#[must_use]
#[allow(dead_code)] // Consumed by the verification harness and refinement tests.
pub(crate) fn check_production_in_flight_first_release_transition(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    if production_in_flight_first_release_transition_kernel(projection) {
        Some(CheckedProductionTransition::unwitnessed(projection))
    } else {
        None
    }
}
fn check_derived_production_in_flight_first_release_transition(
    action: u8,
    actor: u128,
    target: u128,
    before: ProductionInFlightFirstReleaseStateProjection,
    after: ProductionInFlightFirstReleaseStateProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    check_production_in_flight_first_release_transition(
        ProductionInFlightFirstReleaseTransitionProjection {
            action,
            actor,
            target,
            before,
            after,
        },
    )
}
/// Derive and check one `FanoutFromProducer` action.
///
/// `replica` is the one-hot validator bitmap receiving volatile body custody.
/// The full transition checker rejects a producer, malformed bitmap, crashed
/// recipient, absent producer custody, or otherwise malformed pre-state.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_fanout_from_producer_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    replica: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    let mut after = before;
    after.session.bodies |= replica;
    check_derived_production_in_flight_first_release_transition(
        IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER,
        replica,
        0,
        before,
        after,
    )
}
/// Derive and check one `ServeLateBody` action.
///
/// `source` and `target` are one-hot validator bitmaps. The transition checker
/// authenticates source custody and rejects a self-send or crashed target.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_serve_late_body_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    source: u128,
    target: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    let mut after = before;
    after.session.bodies |= target;
    check_derived_production_in_flight_first_release_transition(
        IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY,
        source,
        target,
        before,
        after,
    )
}
/// Derive and check one `Crash` action.
///
/// A crash removes exactly the actor's volatile body and READY custody, marks
/// that validator crashed, and clears producer liveness only for the producer.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_crash_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    actor: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    let mut after = before;
    after.session.crashed |= actor;
    after.session.bodies &= !actor;
    after.session.ready_authorized &= !actor;
    after.session.producer_alive = before.session.producer_alive && actor != before.producer;
    check_derived_production_in_flight_first_release_transition(
        IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH,
        actor,
        0,
        before,
        after,
    )
}
/// Derive and check one `Recover` action.
///
/// Recovery removes only the actor's crashed bit. It cannot fabricate volatile
/// body custody, READY authorization, or producer liveness.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_recover_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    actor: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    let mut after = before;
    after.session.crashed &= !actor;
    check_derived_production_in_flight_first_release_transition(
        IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER,
        actor,
        0,
        before,
        after,
    )
}
/// Derive and check the exact `RecoverReservationSnapshot` stutter.
///
/// Snapshot replay rebuilds process-local indexes only, so no composed safety
/// fact is permitted to change.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_recover_reservation_snapshot_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    check_derived_production_in_flight_first_release_transition(
        IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT,
        0,
        0,
        before,
        before,
    )
}
/// Derive and check one `RehydrateLocalKuraCustody` action.
///
/// The actor is a one-hot local validator with exact durable Kura payload
/// ownership, no crash marker, and no volatile body custody. Rehydration adds
/// only that body custody. It revives producer liveness only when the actor is
/// the frozen producer and never invents READY authorization.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_rehydrate_local_kura_custody_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    actor: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    let mut after = before;
    after.session.bodies |= actor;
    if actor == before.producer {
        after.session.producer_alive = true;
    }
    check_derived_production_in_flight_first_release_transition(
        IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY,
        actor,
        0,
        before,
        after,
    )
}
/// Derive and check the exact `RepairPostCarrierEvidence` stutter.
///
/// Post-carrier repair is authorized only after canonical WSV application and
/// cannot change any composed safety fact.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_repair_post_carrier_evidence_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    check_derived_production_in_flight_first_release_transition(
        IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER,
        0,
        0,
        before,
        before,
    )
}
