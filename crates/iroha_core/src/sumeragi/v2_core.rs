//! Pure persistence-aware Sumeragi v2 reducer embedded in `iroha_core`.
//!
//! The package-local modules below are the authoritative dependency-free
//! transition relation used by production, simulation, and formal refinement.
//! The excluded `iroha_sumeragi_core` crate is only a verification harness over
//! these sources, so publishing `iroha_core` never depends on files outside its
//! own package root.
// These dependency-free source modules also form the public API of the
// standalone `iroha_sumeragi_core` verification crate. Some public accessors
// are not used by the private embedded adapter, so its compilation cannot
// observe their external consumers.
#[allow(dead_code)]
mod committee;
#[allow(dead_code)]
mod quorum;
#[macro_use]
mod refinement;
#[allow(dead_code)]
mod reducer;
#[macro_use]
mod scheduler;
#[allow(dead_code)]
mod types;
// The physical WAL framing/append API is also the public surface of the
// standalone `iroha_sumeragi_core` verification crate. The embedded adapter
// currently consumes only the logical replay subset.
#[allow(dead_code)]
mod wal;
// The dependency-free reducer and the configured exact-output geometry must
// reserve the same complete effect batch. Array-length equality makes drift a
// production compile error without introducing `iroha_config` into the formal
// harness.
const _: [(); refinement::MAX_EFFECTS_PER_STEP] =
    [(); iroha_config::parameters::defaults::sumeragi::V2_MAX_EFFECTS_PER_STEP];
#[allow(unused_imports)]
pub(crate) use committee::{
    Committee, CommitteeError, CommitteeRole, MAX_COMMITTEE_SIZE, MIN_COMMITTEE_SIZE,
    ValidatorIndex,
};
pub(crate) use quorum::{Quorum, QuorumError};
pub(crate) use reducer::{
    BodyState, DurableCommitReceipt, Effect, EquivocationEvidence, EquivocationKind, Event,
    IgnoreReason, Reducer, ReducerError, SignableMessage, StepDisposition, StepOutcome,
};
pub(crate) use refinement::{
    CanonicalIdentityProjection, EFFECTIVE_LOCK_TRACE_OWNER, EFFECTIVE_LOCK_TRACE_RETIRE,
    EFFECTIVE_LOCK_TRACE_SERVICE, EVENT_PERSISTENCE_FAILED, EffectiveLockTraceProjection,
    ExactBodyCompletionOwnership, ExactBodyOwnerProjection, ExactBodyRetirementAccounting,
    IDENTITY_DOMAIN_CONTEXT, IDENTITY_DOMAIN_DURABLE_ARTIFACT, IDENTITY_DOMAIN_PAYLOAD,
    IDENTITY_DOMAIN_PEER, IDENTITY_DOMAIN_PROCESS_LOCAL, IDENTITY_DOMAIN_SUBJECT,
    IDENTITY_KIND_BLOCK_HEADER, IDENTITY_KIND_CANONICAL_PAYLOAD,
    IDENTITY_KIND_CERTIFIED_BODY_REQUEST, IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST,
    IDENTITY_KIND_CONSENSUS_MESSAGE, IDENTITY_KIND_DURABLE_BODY_FRAME,
    IDENTITY_KIND_EXECUTED_BLOCK_WIRE, IDENTITY_KIND_EXECUTION_COMMITMENT,
    IDENTITY_KIND_FINALITY_ARTIFACT, IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER,
    IDENTITY_KIND_LANE_QUEUE_RESERVATION, IDENTITY_KIND_LEADER_WIRE_LIFECYCLE,
    IDENTITY_KIND_MERGE_ENTRY, IDENTITY_KIND_NETWORK_RESPONSE, IDENTITY_KIND_PAYLOAD_MANIFEST,
    IDENTITY_KIND_PEER, IDENTITY_KIND_QUORUM_CERTIFICATE, IDENTITY_KIND_REFERENCE_DIGEST,
    IDENTITY_KIND_REPLY_DELIVERY_ROUTE, IDENTITY_KIND_REPLY_PAYLOAD,
    IDENTITY_KIND_REPLY_SOURCE_KEY, IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,
    IDENTITY_KIND_RUNTIME_CANDIDATE_SEMANTIC, IDENTITY_KIND_RUNTIME_CAUSAL_CANDIDATE,
    IDENTITY_KIND_RUNTIME_EFFECT, IDENTITY_KIND_RUNTIME_LIFECYCLE_OWNER,
    IDENTITY_KIND_SIDECAR_CHUNK, IDENTITY_KIND_SIDECAR_PAYLOAD, IDENTITY_KIND_SIDECAR_REQUEST,
    IDENTITY_KIND_SIDECAR_RESPONSE, IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE,
    IDENTITY_KIND_SIDECAR_SIBLING_STATE, IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE,
    IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE, IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD,
    IDENTITY_KIND_WIRE_BLOCK_SUBJECT, IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA,
    IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING,
    IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED, IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY,
    IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE,
    IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH, IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_COMMIT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE,
    IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V1,
    IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_PLAN_TOMBSTONE,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO,
    IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V1,
    IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY, IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY,
    IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT, IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
    IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED, IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED, IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_ABSENT,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_REPLICA_QUEUE_FIFO_PRESERVED,
    IN_FLIGHT_RESERVATION_ACTION_COMMIT, IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE,
    IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT, IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,
    IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE, IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
    IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT, IN_FLIGHT_RESERVATION_ACTION_RESERVE,
    IN_FLIGHT_RESERVATION_STATE_ABSENT, IN_FLIGHT_RESERVATION_STATE_COMMITTED,
    IN_FLIGHT_RESERVATION_STATE_LIVE, IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED,
    IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED, LEADER_WIRE_ADMISSION_COALESCE,
    LEADER_WIRE_ADMISSION_INSERT, LEADER_WIRE_ADMISSION_REACTIVATE,
    LEADER_WIRE_ADMISSION_REPLACE_TERMINAL, LEADER_WIRE_LIFECYCLE_ABSENT,
    LEADER_WIRE_LIFECYCLE_DORMANT, LEADER_WIRE_LIFECYCLE_INGRESS, LEADER_WIRE_LIFECYCLE_RUNTIME,
    LEADER_WIRE_LIFECYCLE_TERMINAL, LEADER_WIRE_LIFECYCLE_VOLATILE_TERMINAL,
    MAX_CAUSAL_SUCCESSORS_PER_COMMAND, MAX_EFFECTS_PER_STEP, ProductionApplicationTraceProjection,
    ProductionAppliedSuccessorTraceProjection, ProductionDecisionIdentityProjection,
    ProductionDecisionRecoveryTraceProjection, ProductionDigest256Projection,
    ProductionDurableBodyIdentityProjection, ProductionDurablePredecessorIdentityProjection,
    ProductionEffectToCandidateTraceProjection, ProductionHistoricalBodyPipelineTraceProjection,
    ProductionHistoricalCertificateTraceProjection,
    ProductionInFlightFirstReleaseCarrierProjection,
    ProductionInFlightFirstReleaseDecisionProjection,
    ProductionInFlightFirstReleaseHistoryProjection, ProductionInFlightFirstReleaseQueueProjection,
    ProductionInFlightFirstReleaseReleaseProjection,
    ProductionInFlightFirstReleaseSessionProjection, ProductionInFlightFirstReleaseStateProjection,
    ProductionInFlightFirstReleaseTransitionProjection,
    ProductionInFlightFirstReleaseTransitionWitnessV1,
    ProductionInFlightReservationOwnerProjection,
    ProductionInFlightReservationTransitionProjection,
    ProductionIngressIdentityAndClassTraceProjection,
    ProductionIngressReservationMaterializationTraceProjection,
    ProductionLeaderWireAdmissionTraceProjection, ProductionQuorumCertificateIdentityProjection,
    ProductionRecoveredSuccessorTraceProjection, ProductionReliableFlushApplicationProjection,
    ProductionReliableFlushTraceProjection, ProductionSuccessorPredecessorBindingProjection,
    ProductionSuccessorSnapshotProjection, ProductionSuccessorStartupLifecycleProjection,
    ProductionTerminalApplicationWithoutSuccessorActivationProjection,
    RUNTIME_CANDIDATE_KIND_APPLY, RUNTIME_CANDIDATE_KIND_FETCH_BODY, RUNTIME_CANDIDATE_KIND_NONE,
    RUNTIME_CANDIDATE_KIND_SIGN_PROPOSAL, RUNTIME_CANDIDATE_KIND_SIGN_TIMEOUT,
    RUNTIME_CANDIDATE_KIND_SIGN_VOTE, RUNTIME_CANDIDATE_KIND_STORE_BODY,
    RUNTIME_CANDIDATE_KIND_VALIDATE_BODY, RUNTIME_EFFECT_CAUSALITY_FRESH,
    RUNTIME_EFFECT_CAUSALITY_INHERIT, RUNTIME_EFFECT_KIND_APPLY, RUNTIME_EFFECT_KIND_BROADCAST,
    RUNTIME_EFFECT_KIND_ENTER_VIEW, RUNTIME_EFFECT_KIND_FETCH_BODY,
    RUNTIME_EFFECT_KIND_OPAQUE_TEST, RUNTIME_EFFECT_KIND_REPORT_EQUIVOCATION,
    RUNTIME_EFFECT_KIND_REPORT_INVALID_CERTIFIED_BODY, RUNTIME_EFFECT_KIND_SIGN_PROPOSAL,
    RUNTIME_EFFECT_KIND_SIGN_TIMEOUT, RUNTIME_EFFECT_KIND_SIGN_VOTE,
    RUNTIME_EFFECT_KIND_STORE_BODY, RUNTIME_EFFECT_KIND_VALIDATE_BODY, SERVICE_CLASS_COMPLETION,
    SERVICE_CLASS_NONE, SERVICE_CLASS_NORMAL, SERVICE_CLASS_PROGRESS, SUCCESSOR_AUTHORITY_APPLIED,
    SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP, SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
    SUCCESSOR_LIFECYCLE_BEGIN, SUCCESSOR_LIFECYCLE_FAIL, SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP,
    SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP, SUCCESSOR_MARKER_ACTIVATED, SUCCESSOR_STAGE_COMPLETE,
    SUCCESSOR_STAGE_NONE, SUCCESSOR_STAGE_QUEUED, SUCCESSOR_STAGE_RUNNING, TagProjection,
    check_production_application_transition, check_production_applied_successor_transition,
    check_production_body_capacity_retirement_effective_lock_transition,
    check_production_body_ownership_effective_lock_transition,
    check_production_body_service_effective_lock_transition,
    check_production_decision_recovery_transition, check_production_effect_to_candidate_transition,
    check_production_historical_body_pipeline_transition,
    check_production_historical_certificate_transition,
    check_production_in_flight_first_release_observe_replica_queue_release_transition,
    check_production_in_flight_reservation_transition,
    check_production_ingress_reservation_materialization_transition,
    check_production_ingress_transition, check_production_leader_wire_admission_transition,
    check_production_recovered_successor_transition,
    check_production_reliable_flush_application_transition,
    check_production_reliable_flush_link_transition,
    check_production_reliable_flush_worker_transition,
    check_production_successor_startup_lifecycle_transition,
    check_production_terminal_application_transition, classify_exact_body_completion_ownership,
    exact_body_stage_is_owned, plan_exact_body_owner_binding, plan_exact_body_owner_rebind,
    plan_exact_body_retirement_accounting, prepend_causal_continuation,
    production_durable_predecessor_identity_kernel,
    production_in_flight_first_release_state_kernel,
    production_in_flight_first_release_terminal_owner,
    production_in_flight_first_release_witness_binding_kernel,
    production_successor_predecessor_binding_kernel, select_bounded_service_class,
};
pub use refinement::{
    CheckedProductionTransition, ProductionTwoStageRelayRetryTraceProjection,
    check_production_two_stage_relay_retry_transition,
    production_two_stage_relay_retry_trace_refines_source_fairness_kernel,
};
/// Schema of the production-reachable first-release transition witness.
pub(crate) const PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION: u16 = 1;
/// SHA-256 identity of the reviewed TLA+ action source for witness schema V1.
///
/// The formal source checker recomputes this value from
/// `SumeragiV2InFlightFirstRelease.tla`; changing the model without deliberately
/// advancing this identity fails the source-bound formal preflight.
pub(crate) const PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256:
    ProductionDigest256Projection = ProductionDigest256Projection {
    word0: 0x2518_68a6_cc66_0bd6,
    word1: 0x1e6f_b4e0_0592_3b04,
    word2: 0xb529_3995_6821_3ffd,
    word3: 0x03e2_1fda_4686_67d2,
};
/// Explicit classification accepted by the production trace replay reducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductionInFlightFirstReleaseReplayStepV1 {
    /// A state-changing member of the composed first-release `Next` relation.
    ComposedNext,
    /// A strict non-producer replica re-authenticating an already-proved FIFO-only direct release.
    ReleaseReservationDirectProofStutter,
    /// Reservation snapshot reconstruction which changes no abstract fact.
    RecoverReservationSnapshotStutter,
    /// Post-carrier receipt/index repair which changes no abstract fact.
    RepairPostCarrierEvidenceStutter,
}
fn append_first_release_identity_v1(bytes: &mut Vec<u8>, identity: CanonicalIdentityProjection) {
    bytes.push(identity.domain);
    bytes.push(identity.kind);
    bytes.extend_from_slice(&identity.word0.to_be_bytes());
    bytes.extend_from_slice(&identity.word1.to_be_bytes());
    bytes.extend_from_slice(&identity.word2.to_be_bytes());
    bytes.extend_from_slice(&identity.word3.to_be_bytes());
}
fn append_first_release_bool_v1(bytes: &mut Vec<u8>, value: bool) {
    bytes.push(u8::from(value));
}
/// Canonically encode the complete fixed-width abstract state for witness V1.
fn canonical_first_release_state_bytes_v1(
    state: ProductionInFlightFirstReleaseStateProjection,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(448);
    bytes.extend_from_slice(b"iroha-sumeragi-v2-in-flight-first-release-state-v1\0");
    bytes.push(state.validator_count);
    bytes.extend_from_slice(&state.producer.to_be_bytes());
    bytes.extend_from_slice(&state.producer_selected_owner.to_be_bytes());
    bytes.extend_from_slice(&state.replicated_carrier_owners.to_be_bytes());
    bytes.extend_from_slice(&state.payload_binding_a.to_be_bytes());
    append_first_release_identity_v1(&mut bytes, state.binding_a);
    bytes.push(state.queue.plan_state);
    bytes.extend_from_slice(&state.queue.selected_count.to_be_bytes());
    bytes.push(state.queue.reservation_state);
    bytes.extend_from_slice(&state.carrier.kura_active.to_be_bytes());
    bytes.extend_from_slice(&state.carrier.execution_input_durable.to_be_bytes());
    append_first_release_bool_v1(&mut bytes, state.carrier.ready_qc_durable);
    bytes.extend_from_slice(&state.session.bodies.to_be_bytes());
    bytes.extend_from_slice(&state.session.ready_authorized.to_be_bytes());
    bytes.extend_from_slice(&state.session.crashed.to_be_bytes());
    append_first_release_bool_v1(&mut bytes, state.session.producer_alive);
    append_first_release_bool_v1(&mut bytes, state.history.ever_queue_plan_v1);
    append_first_release_bool_v1(&mut bytes, state.history.ever_reservation_v1);
    bytes.extend_from_slice(&state.history.ever_execution_input_durable.to_be_bytes());
    bytes.extend_from_slice(&state.history.ever_ready_authorized.to_be_bytes());
    bytes.extend_from_slice(&state.history.ready_signed.to_be_bytes());
    append_first_release_bool_v1(&mut bytes, state.history.ever_ready_qc_durable);
    bytes.extend_from_slice(&state.history.reservation_committed_prefix.to_be_bytes());
    bytes.extend_from_slice(&state.history.queue_plan_tombstoned_prefix.to_be_bytes());
    bytes.extend_from_slice(
        &state
            .history
            .reservation_commit_forgotten_prefix
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&state.history.pending_high_water.to_be_bytes());
    bytes.extend_from_slice(&state.history.released_high_water.to_be_bytes());
    append_first_release_identity_v1(&mut bytes, state.decision.lane_commit_scope);
    append_first_release_identity_v1(&mut bytes, state.decision.release_scope);
    bytes.extend_from_slice(&state.decision.lane_commit_owner.to_be_bytes());
    bytes.extend_from_slice(&state.decision.release_owner.to_be_bytes());
    append_first_release_bool_v1(&mut bytes, state.decision.wsv_committed);
    bytes.push(state.decision.application_count);
    bytes.extend_from_slice(&state.decision.applied_by.to_be_bytes());
    append_first_release_bool_v1(&mut bytes, state.release.kura_retired);
    bytes.extend_from_slice(&state.release.pending_prefix.to_be_bytes());
    bytes.extend_from_slice(&state.release.released_prefix.to_be_bytes());
    append_first_release_bool_v1(&mut bytes, state.release.fifo_restored);
    bytes
}
fn production_in_flight_first_release_state_digest_v1(
    state: ProductionInFlightFirstReleaseStateProjection,
) -> ProductionDigest256Projection {
    let bytes = iroha_crypto::sha256(canonical_first_release_state_bytes_v1(state));
    ProductionDigest256Projection {
        word0: u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
        word1: u64::from_be_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]),
        word2: u64::from_be_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
        ]),
        word3: u64::from_be_bytes([
            bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31],
        ]),
    }
}
fn production_in_flight_first_release_transition_witness_v1(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
) -> ProductionInFlightFirstReleaseTransitionWitnessV1 {
    ProductionInFlightFirstReleaseTransitionWitnessV1 {
        schema_version: PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TRANSITION_WITNESS_VERSION,
        action: projection.action,
        actor: projection.actor,
        target: projection.target,
        before_state_digest: production_in_flight_first_release_state_digest_v1(projection.before),
        after_state_digest: production_in_flight_first_release_state_digest_v1(projection.after),
        source_identity: PRODUCTION_IN_FLIGHT_FIRST_RELEASE_TLA_SOURCE_SHA256,
    }
}
/// Independently authenticate one V1 witness against its exact projection.
#[must_use]
pub(crate) fn authenticate_production_in_flight_first_release_transition_witness_v1(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    witness: ProductionInFlightFirstReleaseTransitionWitnessV1,
) -> bool {
    refinement::production_in_flight_first_release_transition_kernel(projection)
        && production_in_flight_first_release_witness_binding_kernel(projection, witness)
        && witness == production_in_flight_first_release_transition_witness_v1(projection)
}
/// Replay one classified trace step through the sole composed production relation.
///
/// The reducer rejects all named stutters unless the caller classifies them
/// explicitly. Every other accepted step must change the abstract state and be
/// a member of the same composed `Next` relation used by the production gates
/// and the Verus instantiation.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_replay_step_v1(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    classification: ProductionInFlightFirstReleaseReplayStepV1,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    let classified = match classification {
        ProductionInFlightFirstReleaseReplayStepV1::ComposedNext => {
            projection.action != IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT
                && projection.action != IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER
                && projection.before != projection.after
        }
        ProductionInFlightFirstReleaseReplayStepV1::ReleaseReservationDirectProofStutter => {
            projection.action == IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT
                && projection.before == projection.after
        }
        ProductionInFlightFirstReleaseReplayStepV1::RecoverReservationSnapshotStutter => {
            projection.action == IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT
                && projection.before == projection.after
        }
        ProductionInFlightFirstReleaseReplayStepV1::RepairPostCarrierEvidenceStutter => {
            projection.action == IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER
                && projection.before == projection.after
        }
    };
    if !classified {
        return None;
    }
    let checked = refinement::check_production_in_flight_first_release_transition(projection)?;
    let witness = production_in_flight_first_release_transition_witness_v1(projection);
    authenticate_production_in_flight_first_release_transition_witness_v1(projection, witness)
        .then(|| checked.with_first_release_witness(witness))
}
/// Check and witness any of the 27 first-release production actions.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_transition(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    let classification = match projection.action {
        IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT
            if projection.before == projection.after =>
        {
            ProductionInFlightFirstReleaseReplayStepV1::ReleaseReservationDirectProofStutter
        }
        IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT => {
            ProductionInFlightFirstReleaseReplayStepV1::RecoverReservationSnapshotStutter
        }
        IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER => {
            ProductionInFlightFirstReleaseReplayStepV1::RepairPostCarrierEvidenceStutter
        }
        _ => ProductionInFlightFirstReleaseReplayStepV1::ComposedNext,
    };
    check_production_in_flight_first_release_replay_step_v1(projection, classification)
}
fn witness_derived_first_release_transition(
    checked: CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    check_production_in_flight_first_release_transition(checked.into_projection())
}
/// Derive, check, and witness `FanoutFromProducer`.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_fanout_from_producer_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    replica: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    witness_derived_first_release_transition(
        refinement::check_production_in_flight_first_release_fanout_from_producer_transition(
            before, replica,
        )?,
    )
}
/// Derive, check, and witness `ServeLateBody`.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_serve_late_body_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    source: u128,
    target: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    witness_derived_first_release_transition(
        refinement::check_production_in_flight_first_release_serve_late_body_transition(
            before, source, target,
        )?,
    )
}
/// Derive, check, and witness `Crash`.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_crash_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    actor: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    witness_derived_first_release_transition(
        refinement::check_production_in_flight_first_release_crash_transition(before, actor)?,
    )
}
/// Derive, check, and witness `Recover`.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_recover_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    actor: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    witness_derived_first_release_transition(
        refinement::check_production_in_flight_first_release_recover_transition(before, actor)?,
    )
}
/// Derive, check, and witness the reservation-snapshot stutter.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_recover_reservation_snapshot_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    witness_derived_first_release_transition(
        refinement::check_production_in_flight_first_release_recover_reservation_snapshot_transition(
            before,
        )?,
    )
}
/// Derive, check, and witness `RehydrateLocalKuraCustody`.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_rehydrate_local_kura_custody_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
    actor: u128,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    witness_derived_first_release_transition(
        refinement::check_production_in_flight_first_release_rehydrate_local_kura_custody_transition(
            before, actor,
        )?,
    )
}
/// Derive, check, and witness the post-carrier evidence-repair stutter.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_repair_post_carrier_evidence_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    witness_derived_first_release_transition(
        refinement::check_production_in_flight_first_release_repair_post_carrier_evidence_transition(
            before,
        )?,
    )
}
#[cfg(test)]
pub(crate) use refinement::{
    production_reliable_flush_trace_refines_outbound_ownership_kernel,
    production_reliable_flush_two_phase_link_kernel,
};
pub(crate) use scheduler::{ScheduleState, ScheduledWork};
#[cfg(test)]
pub(crate) use types::FUTURE_TIMEOUT_VOTE_LOOKAHEAD;
pub(crate) use types::{
    CertificateRef, ConsensusMessageV2, ContextId, Digest, EventTag, Generation, HeightContext,
    HeightContextError, MAX_VOTING_ROSTER_LEN, NetworkId, OpaqueSignature, PayloadManifest, Phase,
    Proposal, ProposalJustification, QuorumCertificate, Round, SignatureShare, SignedProposal,
    SignedTimeoutVote, SignedVote, Subject, TimeoutCertificate, TimeoutSignatureGroup, TimeoutVote,
    Validator, ValidatorId, Vote, VotingMode, VotingPower, timeout_vote_view_is_admissible,
};
pub(crate) use wal::{
    DurableState, PersistenceId, ReplayError, SAFETY_WAL_FILE_HEADER_LEN,
    SAFETY_WAL_FRAME_HEADER_LEN, SAFETY_WAL_FRAME_MAGIC, SAFETY_WAL_HASH_LEN,
    SAFETY_WAL_MAX_RECORD_BYTES, WalAppendError, WalAppendIo, WalAppendState, WalCodecError,
    WalEntry, WalFileIdentity, WalFrameCorruption, WalHeaderCorruption, WalIdentityField,
    WalIoStage, WalRecord, WalRetirementAuthorization, encode_wal_file_header, recover_wal_file,
};
#[cfg(test)]
pub(crate) use wal::{SAFETY_WAL_FILE_MAGIC, SAFETY_WAL_FORMAT_VERSION};
#[cfg(test)]
mod tests;
