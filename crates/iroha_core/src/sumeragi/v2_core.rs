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

pub(crate) use quorum::{Quorum, QuorumError};
pub(crate) use reducer::{
    BodyState, DurableCommitReceipt, Effect, EquivocationKind, Event, IgnoreReason, Reducer,
    ReducerError, SignableMessage, StepDisposition,
};
#[cfg(test)]
pub(crate) use reducer::{EquivocationEvidence, StepOutcome};
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
    IDENTITY_KIND_SIDECAR_CHUNK, IDENTITY_KIND_SIDECAR_PAYLOAD, IDENTITY_KIND_SIDECAR_REQUEST,
    IDENTITY_KIND_SIDECAR_RESPONSE, IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE,
    IDENTITY_KIND_SIDECAR_SIBLING_STATE, IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE,
    IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE, IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD,
    IDENTITY_KIND_WIRE_BLOCK_SUBJECT, IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
    IN_FLIGHT_RESERVATION_ACTION_COMMIT, IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE,
    IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT, IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,
    IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE, IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED,
    IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT, IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,
    IN_FLIGHT_RESERVATION_ACTION_RESERVE, IN_FLIGHT_RESERVATION_STATE_ABSENT,
    IN_FLIGHT_RESERVATION_STATE_COMMITTED, IN_FLIGHT_RESERVATION_STATE_LIVE,
    IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED, IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED,
    LEADER_WIRE_ADMISSION_COALESCE, LEADER_WIRE_ADMISSION_INSERT, LEADER_WIRE_ADMISSION_REACTIVATE,
    LEADER_WIRE_ADMISSION_REPLACE_TERMINAL, LEADER_WIRE_LIFECYCLE_ABSENT,
    LEADER_WIRE_LIFECYCLE_DORMANT, LEADER_WIRE_LIFECYCLE_INGRESS, LEADER_WIRE_LIFECYCLE_RUNTIME,
    LEADER_WIRE_LIFECYCLE_TERMINAL, LEADER_WIRE_LIFECYCLE_VOLATILE_TERMINAL, MAX_EFFECTS_PER_STEP,
    ProductionApplicationTraceProjection, ProductionAppliedSuccessorTraceProjection,
    ProductionDecisionIdentityProjection, ProductionDecisionRecoveryTraceProjection,
    ProductionDurableBodyIdentityProjection, ProductionDurablePredecessorIdentityProjection,
    ProductionHistoricalBodyPipelineTraceProjection,
    ProductionHistoricalCertificateTraceProjection, ProductionInFlightReservationOwnerProjection,
    ProductionInFlightReservationTransitionProjection,
    ProductionIngressIdentityAndClassTraceProjection, ProductionLeaderWireAdmissionTraceProjection,
    ProductionQuorumCertificateIdentityProjection, ProductionRecoveredSuccessorTraceProjection,
    ProductionReliableFlushApplicationProjection, ProductionReliableFlushTraceProjection,
    ProductionSuccessorPredecessorBindingProjection, ProductionSuccessorSnapshotProjection,
    ProductionSuccessorStartupLifecycleProjection,
    ProductionTerminalApplicationWithoutSuccessorActivationProjection, SERVICE_CLASS_COMPLETION,
    SERVICE_CLASS_NONE, SERVICE_CLASS_NORMAL, SERVICE_CLASS_PROGRESS, SUCCESSOR_AUTHORITY_APPLIED,
    SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP, SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
    SUCCESSOR_LIFECYCLE_BEGIN, SUCCESSOR_LIFECYCLE_FAIL, SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP,
    SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP, SUCCESSOR_MARKER_ACTIVATED, SUCCESSOR_STAGE_COMPLETE,
    SUCCESSOR_STAGE_NONE, SUCCESSOR_STAGE_QUEUED, SUCCESSOR_STAGE_RUNNING, TagProjection,
    check_production_application_transition, check_production_applied_successor_transition,
    check_production_body_capacity_retirement_effective_lock_transition,
    check_production_body_ownership_effective_lock_transition,
    check_production_body_service_effective_lock_transition,
    check_production_decision_recovery_transition,
    check_production_historical_body_pipeline_transition,
    check_production_historical_certificate_transition,
    check_production_in_flight_reservation_transition, check_production_ingress_transition,
    check_production_leader_wire_admission_transition,
    check_production_recovered_successor_transition,
    check_production_reliable_flush_application_transition,
    check_production_reliable_flush_link_transition,
    check_production_reliable_flush_worker_transition,
    check_production_successor_startup_lifecycle_transition,
    check_production_terminal_application_transition, classify_exact_body_completion_ownership,
    exact_body_stage_is_owned, plan_exact_body_owner_binding, plan_exact_body_owner_rebind,
    plan_exact_body_retirement_accounting, prepend_causal_continuation,
    production_durable_predecessor_identity_kernel,
    production_successor_predecessor_binding_kernel, select_bounded_service_class,
};
pub use refinement::{
    CheckedProductionTransition, ProductionTwoStageRelayRetryTraceProjection,
    check_production_two_stage_relay_retry_transition,
    production_two_stage_relay_retry_trace_refines_source_fairness_kernel,
};
#[cfg(test)]
pub(crate) use refinement::{
    production_reliable_flush_trace_refines_outbound_ownership_kernel,
    production_reliable_flush_two_phase_link_kernel,
};
pub(crate) use scheduler::{ScheduleState, ScheduledWork};
pub(crate) use types::{
    CertificateRef, ChainId, ConsensusMessageV2, ContextId, Digest, EventTag, Generation,
    HeightContext, HeightContextError, MAX_VOTING_ROSTER_LEN, OpaqueSignature, PayloadManifest,
    Phase, Proposal, ProposalJustification, QuorumCertificate, Round, SignatureShare,
    SignedProposal, SignedTimeoutVote, SignedVote, Subject, TimeoutCertificate,
    TimeoutSignatureGroup, TimeoutVote, Validator, ValidatorId, Vote, VotingMode, VotingPower,
};
pub(crate) use wal::{
    DurableState, PersistenceId, ReplayError, SAFETY_WAL_HASH_LEN, WalAppendError, WalAppendIo,
    WalAppendState, WalCodecError, WalEntry, WalFileIdentity, WalFrameCorruption,
    WalHeaderCorruption, WalIdentityField, WalIoStage, WalRecord, WalRetirementAuthorization,
    encode_wal_file_header, recover_wal_file,
};
#[cfg(test)]
pub(crate) use wal::{
    SAFETY_WAL_FILE_HEADER_LEN, SAFETY_WAL_FILE_MAGIC, SAFETY_WAL_FORMAT_VERSION,
    SAFETY_WAL_FRAME_HEADER_LEN, SAFETY_WAL_FRAME_MAGIC,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod network_simulation;
