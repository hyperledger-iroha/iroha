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

pub(crate) use quorum::{Quorum, QuorumError};
pub(crate) use reducer::{
    BodyState, DurableCommitReceipt, Effect, EquivocationKind, Event, IgnoreReason, Reducer,
    ReducerError, SignableMessage, StepDisposition,
};
#[cfg(test)]
pub(crate) use reducer::{EquivocationEvidence, StepOutcome};
pub(crate) use refinement::{
    CanonicalIdentityProjection, EFFECTIVE_LOCK_TRACE_OWNER, EFFECTIVE_LOCK_TRACE_RETIRE,
    EFFECTIVE_LOCK_TRACE_SERVICE, EffectiveLockTraceProjection, ExactBodyCompletionOwnership,
    ExactBodyOwnerProjection, ExactBodyRetirementAccounting, IDENTITY_DOMAIN_CONTEXT,
    IDENTITY_DOMAIN_DURABLE_ARTIFACT, IDENTITY_DOMAIN_PAYLOAD, IDENTITY_DOMAIN_PEER,
    IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_BLOCK_HEADER, IDENTITY_KIND_CANONICAL_PAYLOAD,
    IDENTITY_KIND_DURABLE_BODY_FRAME, IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
    IDENTITY_KIND_EXECUTION_COMMITMENT, IDENTITY_KIND_FINALITY_ARTIFACT, IDENTITY_KIND_MERGE_ENTRY,
    IDENTITY_KIND_NETWORK_RESPONSE, IDENTITY_KIND_PAYLOAD_MANIFEST, IDENTITY_KIND_PEER,
    IDENTITY_KIND_QUORUM_CERTIFICATE, IDENTITY_KIND_REFERENCE_DIGEST, IDENTITY_KIND_REPLY_PAYLOAD,
    IDENTITY_KIND_SIDECAR_CHUNK, IDENTITY_KIND_SIDECAR_PAYLOAD, IDENTITY_KIND_SIDECAR_REQUEST,
    IDENTITY_KIND_SIDECAR_RESPONSE, IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
    IDENTITY_KIND_WIRE_HEIGHT_CONTEXT, MAX_EFFECTS_PER_STEP, ProductionApplicationTraceProjection,
    ProductionDecisionIdentityProjection, ProductionDecisionRecoveryTraceProjection,
    ProductionDurableBodyIdentityProjection, ProductionIngressIdentityAndClassTraceProjection,
    ProductionQuorumCertificateIdentityProjection, ProductionReliableFlushTraceProjection,
    ProductionSchedulerTraceProjection, SERVICE_CLASS_COMPLETION, SERVICE_CLASS_NONE,
    SERVICE_CLASS_NORMAL, SERVICE_CLASS_PROGRESS, TagProjection,
    classify_exact_body_completion_ownership, exact_body_stage_is_owned,
    plan_exact_body_owner_binding, plan_exact_body_owner_rebind,
    plan_exact_body_retirement_accounting, prepend_causal_continuation,
    production_application_trace_refines_decision_completion_kernel,
    production_body_capacity_retirement_preserves_effective_lock_kernel,
    production_body_ownership_preserves_effective_lock_kernel,
    production_body_service_refines_async_fairness_kernel,
    production_decision_trace_refines_recovery_witness_kernel,
    production_ingress_identity_and_class_trace_refines_protected_ownership_kernel,
    production_reliable_flush_trace_refines_outbound_ownership_kernel,
    production_scheduler_trace_refines_protected_ownership_kernel, select_bounded_service_class,
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
