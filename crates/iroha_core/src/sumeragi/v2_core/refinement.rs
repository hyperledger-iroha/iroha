//! Executable refinement gate shared by production and Verus.
//!
//! The reducer projects each candidate transition into concrete
//! [`TransitionProjection`] primitives and must pass this gate before replacing
//! its caller-visible state.  The kernel privately derives all boolean facts
//! and effect/action authorization from those primitives.  Its expressions are
//! defined by macros because the exact same executable expressions are
//! instantiated in `verus_proofs.rs`; normal builds therefore do not need to
//! link `vstd`.

// The fixed-width proof projection is intentionally passed by value so the
// normal Rust and Verus instantiations share one branch-complete expression.
// Production inlines this private gate; changing it to borrowed wrappers would
// create a second, unverified calling relation solely to silence this lint.
#![allow(clippy::large_types_passed_by_value)]

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};

use super::{
    ConsensusMessageV2, ContextId, Digest, DurableState, HeightContext, Reducer, Round,
    SignedTimeoutVote, Subject, TimeoutCertificate, ValidatorId, reducer::PendingPersistence,
};

/// Maximum number of effects one reducer input can emit.
///
/// Retransmission is the largest branch: the seven canonical control-message
/// classes followed by at most one fetch, store, validation, or application
/// effect for the exact durable Decision stage. A future ninth effect fails
/// closed at the refinement boundary until this limit is deliberately revised
/// and re-verified.
pub const MAX_EFFECTS_PER_STEP: usize = 8;

pub const EFFECT_NONE: u8 = 0;
pub const EFFECT_PERSIST: u8 = 1;
pub const EFFECT_FETCH: u8 = 2;
pub const EFFECT_STORE: u8 = 3;
pub const EFFECT_VALIDATE: u8 = 4;
pub const EFFECT_SIGN: u8 = 5;
pub const EFFECT_BROADCAST: u8 = 6;
pub const EFFECT_APPLY: u8 = 7;
pub const EFFECT_ENTER_VIEW: u8 = 8;
pub const EFFECT_REPORT: u8 = 9;

pub const EVENT_BODY_AVAILABLE: u8 = 8;
pub const EVENT_BODY_STORED: u8 = 9;
pub const EVENT_PERSISTED: u8 = 11;
/// Safety-WAL persistence failure delivered to the reducer.
pub const EVENT_PERSISTENCE_FAILED: u8 = 12;
pub const EVENT_SIGNED: u8 = 13;
pub const EVENT_RESUME_AFTER_REPLAY: u8 = 15;

pub const CONTINUATION_NONE: u8 = 0;
pub const CONTINUATION_SIGN: u8 = 1;
pub const CONTINUATION_INSTALL_TIMEOUT: u8 = 2;
pub const CONTINUATION_DECIDE: u8 = 3;

/// Prepend one persisted reducer continuation without reversing its order.
///
/// `V2Adapter::drive_effects` removes a `Persist` effect from the head of its
/// private work queue, acknowledges the WAL record synchronously, and obtains
/// the reducer effects causally enabled by that acknowledgement. Those effects
/// must run before the queue's old tail and in the exact order emitted by the
/// reducer. Iterating the continuation backwards while pushing each item to
/// the front implements precisely `continuation ++ old_tail`.
///
/// This helper is intentionally generic: the production call moves concrete
/// effects, while the refinement regression uses opaque tokens to pin the
/// queue transformation independently of effect identity or cloning.
#[allow(dead_code)]
pub fn prepend_causal_continuation<T>(pending: &mut VecDeque<T>, continuation: Vec<T>) {
    for item in continuation.into_iter().rev() {
        pending.push_front(item);
    }
}

/// Caller-visible reducer action classes checked at the commit boundary.
///
/// WAL begin/acknowledgement actions carry the exact [`WAL_RECORD_*`] class;
/// every other action must carry [`WAL_RECORD_NONE`].  The split is deliberately
/// small: it classifies the exact atomicity boundary implemented by
/// `Reducer::step`, including production macro-steps that combine ingress,
/// certificate formation, and creation of one pending WAL append.
#[allow(dead_code)]
pub const ACTION_STUTTER: u8 = 0;
#[allow(dead_code)]
pub const ACTION_BEGIN_WAL: u8 = 1;
#[allow(dead_code)]
pub const ACTION_ACKNOWLEDGE_WAL: u8 = 2;
#[allow(dead_code)]
pub const ACTION_BODY_PROGRESS: u8 = 3;
#[allow(dead_code)]
pub const ACTION_VOLATILE_PROTOCOL: u8 = 4;
#[allow(dead_code)]
pub const ACTION_COMPLETE_APPLICATION: u8 = 5;
/// Consume the one recovery-pending transition created by successful replay.
#[allow(dead_code)]
pub const ACTION_RESUME_AFTER_REPLAY: u8 = 6;

/// No WAL record participates in this reducer action.
pub const WAL_RECORD_NONE: u8 = 0;
/// `WalRecord::ProposalIntent`.
pub const WAL_RECORD_PROPOSAL_INTENT: u8 = 1;
/// `WalRecord::PrepareIntent`.
pub const WAL_RECORD_PREPARE_INTENT: u8 = 2;
/// `WalRecord::ObservePrepare`.
pub const WAL_RECORD_OBSERVE_PREPARE: u8 = 3;
/// `WalRecord::LockAndCommit`.
pub const WAL_RECORD_LOCK_AND_COMMIT: u8 = 4;
/// `WalRecord::TimeoutIntent`.
pub const WAL_RECORD_TIMEOUT_INTENT: u8 = 5;
/// `WalRecord::InstallTimeout`.
pub const WAL_RECORD_INSTALL_TIMEOUT: u8 = 6;
/// `WalRecord::Decision`.
pub const WAL_RECORD_DECISION: u8 = 7;

/// No successfully completed signature participates in this action.
pub const SIGNED_MESSAGE_NONE: u8 = 0;
/// Completion of a proposal signature.
pub const SIGNED_MESSAGE_PROPOSAL: u8 = 1;
/// Completion of a Prepare-vote signature.
pub const SIGNED_MESSAGE_PREPARE: u8 = 2;
/// Completion of a Commit-vote signature.
pub const SIGNED_MESSAGE_COMMIT: u8 = 3;
/// Completion of a timeout-vote signature.
pub const SIGNED_MESSAGE_TIMEOUT: u8 = 4;

/// Replay's first item is absent because no durable work needs reconstruction.
pub const REPLAY_EFFECT_NONE: u8 = 0;
/// Replay's first item resumes an already-durable proposal intent.
pub const REPLAY_EFFECT_PROPOSAL: u8 = 1;
/// Replay's first item resumes an already-durable Prepare intent.
pub const REPLAY_EFFECT_PREPARE: u8 = 2;
/// Replay's first item resumes an already-durable Commit intent.
pub const REPLAY_EFFECT_COMMIT: u8 = 3;
/// Replay's first item resumes an already-durable timeout intent.
pub const REPLAY_EFFECT_TIMEOUT: u8 = 4;
/// Replay's sole item resumes acquisition of a durably decided body.
pub const REPLAY_EFFECT_DECISION: u8 = 5;

/// No durable-boundary capability is claimed by a transition.
pub const BOUNDARY_NONE: u8 = 0;
/// Capability to create one pending WAL append.
pub const BOUNDARY_BEGIN_WAL: u8 = 1;
/// Capability to acknowledge and install one pending WAL append.
pub const BOUNDARY_ACKNOWLEDGE_WAL: u8 = 2;
/// Capability to acknowledge local application of a durable decision.
pub const BOUNDARY_COMPLETE_APPLICATION: u8 = 3;
/// Capability to consume the one recovery-resumption transition.
pub const BOUNDARY_RESUME_AFTER_REPLAY: u8 = 4;

/// No bounded runtime ingress class was selected.
#[allow(dead_code)] // Used by the production runtime, outside the pure harness crate.
pub const SERVICE_CLASS_NONE: u8 = 0;
/// Trusted local completion ingress class.
#[allow(dead_code)] // Used by the production runtime, outside the pure harness crate.
pub const SERVICE_CLASS_COMPLETION: u8 = 1;
/// Certified protocol progress ingress class.
#[allow(dead_code)] // Used by the production runtime, outside the pure harness crate.
pub const SERVICE_CLASS_PROGRESS: u8 = 2;
/// Ordinary proposal and vote ingress class.
#[allow(dead_code)] // Used by the production runtime, outside the pure harness crate.
pub const SERVICE_CLASS_NORMAL: u8 = 3;

/// No leader-wire lifecycle occupied the addressed bounded slot.
pub(crate) const LEADER_WIRE_LIFECYCLE_ABSENT: u8 = 0;
/// A restart-restored lifecycle owns anti-ABA state but no selector turn.
pub(crate) const LEADER_WIRE_LIFECYCLE_DORMANT: u8 = 1;
/// Exact bytes and one fair-ingress position own the lifecycle.
pub(crate) const LEADER_WIRE_LIFECYCLE_INGRESS: u8 = 2;
/// The serialized runtime owns the exact lifecycle.
pub(crate) const LEADER_WIRE_LIFECYCLE_RUNTIME: u8 = 3;
/// Same-process terminal memory exists without restart-stable authority.
pub(crate) const LEADER_WIRE_LIFECYCLE_VOLATILE_TERMINAL: u8 = 4;
/// Independently verified durable evidence permanently retires the lifecycle.
pub(crate) const LEADER_WIRE_LIFECYCLE_TERMINAL: u8 = 5;

/// Insert a previously absent bounded lifecycle slot.
pub(crate) const LEADER_WIRE_ADMISSION_INSERT: u8 = 1;
/// Atomically reactivate an exact restart-dormant lifecycle.
pub(crate) const LEADER_WIRE_ADMISSION_REACTIVATE: u8 = 2;
/// Coalesce an exact retry without changing lifecycle state.
pub(crate) const LEADER_WIRE_ADMISSION_COALESCE: u8 = 3;
/// Replace a terminal slot with a strictly newer view and both new ordinals.
pub(crate) const LEADER_WIRE_ADMISSION_REPLACE_TERMINAL: u8 = 4;

/// A persisted view transition selected the effective lock and its fetch.
pub const EFFECTIVE_LOCK_TRACE_ENTER_VIEW: u8 = 1;
/// The body pipeline bound or monotonically enriched its exact owner.
pub const EFFECTIVE_LOCK_TRACE_OWNER: u8 = 2;
/// Supersession retired the exact body-capacity residuals.
pub const EFFECTIVE_LOCK_TRACE_RETIRE: u8 = 3;
/// One bounded ingress invocation selected an exact ready service class.
pub const EFFECTIVE_LOCK_TRACE_SERVICE: u8 = 4;

/// No quorum-certificate evidence is present at this projection position.
pub(crate) const CERTIFICATE_EVIDENCE_ABSENT: u8 = 0;
/// The certificate is byte-for-byte equal to the transition's local lock.
pub(crate) const CERTIFICATE_EVIDENCE_LOCAL: u8 = 1;
/// The certificate is byte-for-byte equal to the incoming timeout high-QC.
pub(crate) const CERTIFICATE_EVIDENCE_INCOMING: u8 = 2;
/// The certificate is not owned by either authenticated transition source.
pub(crate) const CERTIFICATE_EVIDENCE_FOREIGN: u8 = 3;

/// Domain for a frozen consensus context digest.
pub(crate) const IDENTITY_DOMAIN_CONTEXT: u8 = 1;
/// Domain for a canonical block or consensus subject component.
pub(crate) const IDENTITY_DOMAIN_SUBJECT: u8 = 2;
/// Domain for canonical body, request, or response bytes.
pub(crate) const IDENTITY_DOMAIN_PAYLOAD: u8 = 3;
/// Domain for an authenticated network source or semantic origin.
pub(crate) const IDENTITY_DOMAIN_PEER: u8 = 4;
/// Domain for a durable receipt or finality artifact.
pub(crate) const IDENTITY_DOMAIN_DURABLE_ARTIFACT: u8 = 5;
/// Domain for process-local identities which must never enter wire or consensus state.
pub(crate) const IDENTITY_DOMAIN_PROCESS_LOCAL: u8 = 6;
/// Canonical identity kind for one frozen consensus context.
pub(crate) const IDENTITY_KIND_CONSENSUS_CONTEXT: u8 = 1;
/// Canonical identity kind for one consensus subject.
pub(crate) const IDENTITY_KIND_CONSENSUS_SUBJECT: u8 = 1;
/// Canonical identity kind for one wire-level frozen height context.
pub(crate) const IDENTITY_KIND_WIRE_HEIGHT_CONTEXT: u8 = 2;
/// Canonical identity kind for one wire-level block subject.
pub(crate) const IDENTITY_KIND_WIRE_BLOCK_SUBJECT: u8 = 2;
/// Canonical identity kind for one block-header hash.
pub(crate) const IDENTITY_KIND_BLOCK_HEADER: u8 = 3;
/// Canonical identity kind for one canonical payload hash.
pub(crate) const IDENTITY_KIND_CANONICAL_PAYLOAD: u8 = 1;
/// Canonical identity kind for one execution commitment.
pub(crate) const IDENTITY_KIND_EXECUTION_COMMITMENT: u8 = 2;
/// Canonical identity kind for one complete quorum certificate.
pub(crate) const IDENTITY_KIND_QUORUM_CERTIFICATE: u8 = 3;
/// Canonical identity kind for one payload manifest.
pub(crate) const IDENTITY_KIND_PAYLOAD_MANIFEST: u8 = 4;
/// Canonical identity kind for one result-bearing block wire.
pub(crate) const IDENTITY_KIND_EXECUTED_BLOCK_WIRE: u8 = 5;
/// Canonical identity kind for one signed sidecar request.
pub(crate) const IDENTITY_KIND_SIDECAR_REQUEST: u8 = 6;
/// Canonical identity kind for one actor-admitted reply payload.
pub(crate) const IDENTITY_KIND_REPLY_PAYLOAD: u8 = 7;
/// Canonical identity kind for one merge-ledger entry.
pub(crate) const IDENTITY_KIND_MERGE_ENTRY: u8 = 8;
/// Canonical identity kind for one sidecar ledger-reference digest.
pub(crate) const IDENTITY_KIND_REFERENCE_DIGEST: u8 = 9;
/// Canonical identity kind for one complete network response.
pub(crate) const IDENTITY_KIND_NETWORK_RESPONSE: u8 = 10;
/// Canonical identity kind for one certified sidecar response.
pub(crate) const IDENTITY_KIND_SIDECAR_RESPONSE: u8 = 11;
/// Canonical identity kind for one certified sidecar chunk.
pub(crate) const IDENTITY_KIND_SIDECAR_CHUNK: u8 = 12;
/// Canonical identity kind for one sidecar payload digest.
pub(crate) const IDENTITY_KIND_SIDECAR_PAYLOAD: u8 = 13;
/// Canonical identity kind for one signed CommitQC discovery request.
pub(crate) const IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST: u8 = 14;
/// Canonical identity kind for one complete v2 consensus envelope.
pub(crate) const IDENTITY_KIND_CONSENSUS_MESSAGE: u8 = 15;
/// Canonical identity kind for one signed certified-body request.
pub(crate) const IDENTITY_KIND_CERTIFIED_BODY_REQUEST: u8 = 16;
/// Process-local identity kind for the exact non-target sidecar lane state.
pub(crate) const IDENTITY_KIND_SIDECAR_SIBLING_STATE: u8 = 1;
/// Process-local identity kind for immutable shared sidecar response state.
pub(crate) const IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE: u8 = 2;
/// Process-local identity kind for unchanged target gate reservation state.
pub(crate) const IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE: u8 = 3;
/// Process-local identity kind for unchanged target outbound route state.
pub(crate) const IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE: u8 = 4;
/// Process-local identity kind for one opaque authenticated reply-source owner.
pub(crate) const IDENTITY_KIND_REPLY_SOURCE_KEY: u8 = 5;
/// Process-local identity kind for one exact admitted reply delivery route.
pub(crate) const IDENTITY_KIND_REPLY_DELIVERY_ROUTE: u8 = 6;
/// Process-local identity kind for one actor-minted writer-flush occurrence.
pub(crate) const IDENTITY_KIND_REPLY_WRITER_OCCURRENCE: u8 = 7;
/// Process-local identity kind for one durable leader-wire lifecycle owner.
pub(crate) const IDENTITY_KIND_LEADER_WIRE_LIFECYCLE: u8 = 8;
/// Canonical identity kind for one checksummed durable body frame.
pub(crate) const IDENTITY_KIND_DURABLE_BODY_FRAME: u8 = 1;
/// Canonical identity kind for one finality artifact.
pub(crate) const IDENTITY_KIND_FINALITY_ARTIFACT: u8 = 2;
/// Canonical identity kind for one authenticated snapshot-bootstrap record.
pub(crate) const IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD: u8 = 3;
/// Canonical identity kind for one exact lane queue reservation key.
pub(crate) const IDENTITY_KIND_LANE_QUEUE_RESERVATION: u8 = 4;
/// Canonical identity kind for one exact ordered lane-release barrier.
pub(crate) const IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER: u8 = 5;
/// Canonical identity kind for one authenticated peer.
pub(crate) const IDENTITY_KIND_PEER: u8 = 1;

/// No durable queue owner exists for the projected reservation.
pub(crate) const IN_FLIGHT_RESERVATION_STATE_ABSENT: u8 = 0;
/// The exact reservation owns its transaction outside the ordinary queue.
pub(crate) const IN_FLIGHT_RESERVATION_STATE_LIVE: u8 = 1;
/// The exact reservation is committed pending QueuePlan tombstone durability.
pub(crate) const IN_FLIGHT_RESERVATION_STATE_COMMITTED: u8 = 2;
/// The exact reservation is held by an ordered release barrier.
pub(crate) const IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED: u8 = 3;
/// The exact released record is retained pending FIFO restoration cleanup.
pub(crate) const IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED: u8 = 4;

/// Install retained ownership from one validated checksummed V5 journal snapshot.
pub(crate) const IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT: u8 = 1;
/// Persist one exact live reservation.
pub(crate) const IN_FLIGHT_RESERVATION_ACTION_RESERVE: u8 = 2;
/// Directly release exact aborted or recovery-orphaned work to the queue.
pub(crate) const IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT: u8 = 3;
/// Persist one exact live reservation as committed.
pub(crate) const IN_FLIGHT_RESERVATION_ACTION_COMMIT: u8 = 4;
/// Forget an exact commit barrier after independent QueuePlan cleanup.
pub(crate) const IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT: u8 = 5;
// Verification action tag 6 is intentionally unassigned; retired inputs fail closed.
/// Bind an exact live reservation to an ordered release barrier.
pub(crate) const IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE: u8 = 7;
/// Move an exact prepared reservation to restartable release completion.
pub(crate) const IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE: u8 = 8;
/// Forget an exact completed release after FIFO restoration.
pub(crate) const IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE: u8 = 9;

/// No selected QueuePlan conjunction is durable in the composed projection.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT: u8 = 0;
/// Every selected transaction has one exact live QueuePlan V4 claim.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED: u8 = 1;
/// Every selected QueuePlan V4 claim has been durably tombstoned.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED: u8 = 2;

/// No reservation owner exists in the composed in-flight projection.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT: u8 = 0;
/// The selected batch is owned by durable reservation journal V5 records.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE: u8 = 1;
/// Reservation Commit is durable after canonical WSV application.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED: u8 = 2;
/// Reservation Commit cleanup is durable after QueuePlan tombstoning.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN: u8 = 3;
/// Ordered release is protected by a durable PrepareRelease barrier.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED: u8 = 4;
/// CompleteRelease is durable while FIFO publication remains restartable.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED: u8 = 5;
/// FIFO restoration and ordered-release cleanup are both complete.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN: u8 = 6;
/// Aborted or recovery-orphaned work was directly restored to ordinary FIFO.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED: u8 = 7;

/// Observe one exact selected QueuePlan V4 claim conjunction.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4: u8 = 1;
/// Fsync the selected batch's exact reservation journal V5 records.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5: u8 = 2;
/// Persist the executable payload as an active Kura lane artifact.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA: u8 = 3;
/// Move volatile payload custody from the producer to one replica.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER: u8 = 4;
/// Move volatile late-body custody between two authenticated validators.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY: u8 = 5;
/// Persist one validator's exact execution-input artifact.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT: u8 = 6;
/// Authorize a local READY signature from durable execution input.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY: u8 = 7;
/// Record one local READY signature after authorization.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY: u8 = 8;
/// Persist the quorum-complete READY certificate.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC: u8 = 9;
/// Lose only one validator's volatile session custody.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH: u8 = 10;
/// Re-admit one crashed validator without fabricating volatile custody.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER: u8 = 11;
/// Persist one exact-scope lane consensus decision.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT: u8 = 12;
/// Atomically apply the canonical global carrier to WSV exactly once.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER: u8 = 13;
/// Advance one reservation Commit prefix key after canonical WSV application.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED: u8 = 14;
/// Advance one QueuePlan V4 tombstone prefix key after the full Commit prefix.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_PLAN_TOMBSTONE: u8 = 15;
/// Advance one ForgetCommit prefix key after the full tombstone prefix.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_COMMIT: u8 = 16;
/// Persist Kura retirement and select the exact release scope.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT: u8 = 17;
/// Advance one durable ReleasePending claim prefix.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING: u8 = 18;
/// Persist the exact ordered PrepareRelease barrier.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE: u8 = 19;
/// Advance one durable Released claim prefix.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED: u8 = 20;
/// Persist CompleteRelease after every Released claim.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE: u8 = 21;
/// Restore the selected batch to ordinary FIFO order.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO: u8 = 22;
/// Forget the release barrier only after FIFO restoration.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE: u8 = 23;
/// Repair post-carrier evidence without changing the safety projection.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER: u8 = 24;
/// Rebuild local reservation ownership from the already-durable V6 snapshot envelope.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT: u8 = 25;
/// Directly restore exact aborted/recovery-orphaned work to ordinary FIFO.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT: u8 = 26;
/// Restore one validator's volatile body custody from its exact durable Kura payload.
pub(crate) const IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY: u8 = 27;

/// Successor authority derived from an applied predecessor in this process.
pub(crate) const SUCCESSOR_AUTHORITY_APPLIED: u8 = 1;
/// Successor authority reconstructed from an exact complete durable tip.
pub(crate) const SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP: u8 = 2;
/// First executable context authenticated by an audited snapshot envelope.
pub(crate) const SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP: u8 = 3;

/// No successor-work stage participates in a lifecycle projection.
pub(crate) const SUCCESSOR_STAGE_NONE: u8 = 0;
/// Successor construction is queued behind durable application.
pub(crate) const SUCCESSOR_STAGE_QUEUED: u8 = 1;
/// Successor construction owns the serialized runner.
pub(crate) const SUCCESSOR_STAGE_RUNNING: u8 = 2;
/// Successor publication completed.
pub(crate) const SUCCESSOR_STAGE_COMPLETE: u8 = 3;

/// Begin the applied predecessor's fallible successor construction.
pub(crate) const SUCCESSOR_LIFECYCLE_BEGIN: u8 = 1;
/// Latch a startup failure without fabricating completion.
pub(crate) const SUCCESSOR_LIFECYCLE_FAIL: u8 = 2;
/// Re-enter startup from an exact complete durable tip.
pub(crate) const SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP: u8 = 3;
/// Enter the first executable height from audited snapshot authority.
pub(crate) const SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP: u8 = 4;

/// Exact successor-activation progress marker.
pub(crate) const SUCCESSOR_MARKER_ACTIVATED: u8 = 1;

// Verus cannot read an ordinary Rust `const` from inside `verus!`: it treats
// that item as an opaque external function. Keep the reviewed wire/refinement
// tags as literal macro arms so the shared production decision bodies and
// their Verus instantiations expand to the same primitive values. The
// compile-time assertions below bind every macro arm back to its public or
// crate-visible production constant, making drift a build failure.
macro_rules! refinement_tag_value {
    (EFFECT_PERSIST) => {
        1u8
    };
    (EVENT_PERSISTED) => {
        11u8
    };
    (EVENT_PERSISTENCE_FAILED) => {
        12u8
    };
    (EVENT_SIGNED) => {
        13u8
    };
    (EVENT_RESUME_AFTER_REPLAY) => {
        15u8
    };
    (CONTINUATION_NONE) => {
        0u8
    };
    (CONTINUATION_SIGN) => {
        1u8
    };
    (CONTINUATION_INSTALL_TIMEOUT) => {
        2u8
    };
    (CONTINUATION_DECIDE) => {
        3u8
    };
    (WAL_RECORD_NONE) => {
        0u8
    };
    (WAL_RECORD_PROPOSAL_INTENT) => {
        1u8
    };
    (WAL_RECORD_PREPARE_INTENT) => {
        2u8
    };
    (WAL_RECORD_OBSERVE_PREPARE) => {
        3u8
    };
    (WAL_RECORD_LOCK_AND_COMMIT) => {
        4u8
    };
    (WAL_RECORD_TIMEOUT_INTENT) => {
        5u8
    };
    (WAL_RECORD_INSTALL_TIMEOUT) => {
        6u8
    };
    (WAL_RECORD_DECISION) => {
        7u8
    };
    (REPLAY_EFFECT_NONE) => {
        0u8
    };
    (BOUNDARY_NONE) => {
        0u8
    };
    (BOUNDARY_BEGIN_WAL) => {
        1u8
    };
    (BOUNDARY_ACKNOWLEDGE_WAL) => {
        2u8
    };
    (BOUNDARY_COMPLETE_APPLICATION) => {
        3u8
    };
    (BOUNDARY_RESUME_AFTER_REPLAY) => {
        4u8
    };
    (CERTIFICATE_EVIDENCE_ABSENT) => {
        0u8
    };
    (CERTIFICATE_EVIDENCE_LOCAL) => {
        1u8
    };
    (CERTIFICATE_EVIDENCE_INCOMING) => {
        2u8
    };
    (IDENTITY_DOMAIN_CONTEXT) => {
        1u8
    };
    (IDENTITY_DOMAIN_SUBJECT) => {
        2u8
    };
    (IDENTITY_DOMAIN_PAYLOAD) => {
        3u8
    };
    (IDENTITY_DOMAIN_PEER) => {
        4u8
    };
    (IDENTITY_DOMAIN_DURABLE_ARTIFACT) => {
        5u8
    };
    (IDENTITY_DOMAIN_PROCESS_LOCAL) => {
        6u8
    };
    (IDENTITY_KIND_CONSENSUS_CONTEXT) => {
        1u8
    };
    (IDENTITY_KIND_CONSENSUS_SUBJECT) => {
        1u8
    };
    (IDENTITY_KIND_WIRE_HEIGHT_CONTEXT) => {
        2u8
    };
    (IDENTITY_KIND_WIRE_BLOCK_SUBJECT) => {
        2u8
    };
    (IDENTITY_KIND_BLOCK_HEADER) => {
        3u8
    };
    (IDENTITY_KIND_CANONICAL_PAYLOAD) => {
        1u8
    };
    (IDENTITY_KIND_EXECUTION_COMMITMENT) => {
        2u8
    };
    (IDENTITY_KIND_QUORUM_CERTIFICATE) => {
        3u8
    };
    (IDENTITY_KIND_PAYLOAD_MANIFEST) => {
        4u8
    };
    (IDENTITY_KIND_EXECUTED_BLOCK_WIRE) => {
        5u8
    };
    (IDENTITY_KIND_SIDECAR_REQUEST) => {
        6u8
    };
    (IDENTITY_KIND_REPLY_PAYLOAD) => {
        7u8
    };
    (IDENTITY_KIND_MERGE_ENTRY) => {
        8u8
    };
    (IDENTITY_KIND_REFERENCE_DIGEST) => {
        9u8
    };
    (IDENTITY_KIND_NETWORK_RESPONSE) => {
        10u8
    };
    (IDENTITY_KIND_SIDECAR_RESPONSE) => {
        11u8
    };
    (IDENTITY_KIND_SIDECAR_CHUNK) => {
        12u8
    };
    (IDENTITY_KIND_SIDECAR_PAYLOAD) => {
        13u8
    };
    (IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST) => {
        14u8
    };
    (IDENTITY_KIND_CONSENSUS_MESSAGE) => {
        15u8
    };
    (IDENTITY_KIND_CERTIFIED_BODY_REQUEST) => {
        16u8
    };
    (IDENTITY_KIND_SIDECAR_SIBLING_STATE) => {
        1u8
    };
    (IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE) => {
        2u8
    };
    (IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE) => {
        3u8
    };
    (IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE) => {
        4u8
    };
    (IDENTITY_KIND_REPLY_SOURCE_KEY) => {
        5u8
    };
    (IDENTITY_KIND_REPLY_DELIVERY_ROUTE) => {
        6u8
    };
    (IDENTITY_KIND_REPLY_WRITER_OCCURRENCE) => {
        7u8
    };
    (IDENTITY_KIND_LEADER_WIRE_LIFECYCLE) => {
        8u8
    };
    (IDENTITY_KIND_DURABLE_BODY_FRAME) => {
        1u8
    };
    (IDENTITY_KIND_FINALITY_ARTIFACT) => {
        2u8
    };
    (IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD) => {
        3u8
    };
    (IDENTITY_KIND_LANE_QUEUE_RESERVATION) => {
        4u8
    };
    (IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER) => {
        5u8
    };
    (IDENTITY_KIND_PEER) => {
        1u8
    };
    (IN_FLIGHT_RESERVATION_STATE_ABSENT) => {
        0u8
    };
    (IN_FLIGHT_RESERVATION_STATE_LIVE) => {
        1u8
    };
    (IN_FLIGHT_RESERVATION_STATE_COMMITTED) => {
        2u8
    };
    (IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED) => {
        3u8
    };
    (IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED) => {
        4u8
    };
    (IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT) => {
        1u8
    };
    (IN_FLIGHT_RESERVATION_ACTION_RESERVE) => {
        2u8
    };
    (IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT) => {
        3u8
    };
    (IN_FLIGHT_RESERVATION_ACTION_COMMIT) => {
        4u8
    };
    (IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT) => {
        5u8
    };
    (IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE) => {
        7u8
    };
    (IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE) => {
        8u8
    };
    (IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE) => {
        9u8
    };
    (IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT) => {
        0u8
    };
    (IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED) => {
        1u8
    };
    (IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED) => {
        2u8
    };
    (IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT) => {
        0u8
    };
    (IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE) => {
        1u8
    };
    (IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED) => {
        2u8
    };
    (IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN) => {
        3u8
    };
    (IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED) => {
        4u8
    };
    (IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED) => {
        5u8
    };
    (IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN) => {
        6u8
    };
    (IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED) => {
        7u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4) => {
        1u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5) => {
        2u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA) => {
        3u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER) => {
        4u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY) => {
        5u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT) => {
        6u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY) => {
        7u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY) => {
        8u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC) => {
        9u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH) => {
        10u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER) => {
        11u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT) => {
        12u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER) => {
        13u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED) => {
        14u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_PLAN_TOMBSTONE) => {
        15u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_COMMIT) => {
        16u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT) => {
        17u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING) => {
        18u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE) => {
        19u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED) => {
        20u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE) => {
        21u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO) => {
        22u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE) => {
        23u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER) => {
        24u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT) => {
        25u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT) => {
        26u8
    };
    (IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY) => {
        27u8
    };
    (SUCCESSOR_AUTHORITY_APPLIED) => {
        1u8
    };
    (SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP) => {
        2u8
    };
    (SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP) => {
        3u8
    };
    (SUCCESSOR_STAGE_NONE) => {
        0u8
    };
    (SUCCESSOR_STAGE_QUEUED) => {
        1u8
    };
    (SUCCESSOR_STAGE_RUNNING) => {
        2u8
    };
    (SUCCESSOR_STAGE_COMPLETE) => {
        3u8
    };
    (SUCCESSOR_LIFECYCLE_BEGIN) => {
        1u8
    };
    (SUCCESSOR_LIFECYCLE_FAIL) => {
        2u8
    };
    (SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP) => {
        3u8
    };
    (SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP) => {
        4u8
    };
    (SUCCESSOR_MARKER_ACTIVATED) => {
        1u8
    };
}

macro_rules! assert_refinement_tag_values {
    ($($tag:ident),+ $(,)?) => {
        $(const _: [(); $tag as usize] = [(); refinement_tag_value!($tag) as usize];)+
    };
}

assert_refinement_tag_values!(
    EFFECT_PERSIST,
    EVENT_PERSISTED,
    EVENT_PERSISTENCE_FAILED,
    EVENT_SIGNED,
    EVENT_RESUME_AFTER_REPLAY,
    CONTINUATION_NONE,
    CONTINUATION_SIGN,
    CONTINUATION_INSTALL_TIMEOUT,
    CONTINUATION_DECIDE,
    WAL_RECORD_NONE,
    WAL_RECORD_PROPOSAL_INTENT,
    WAL_RECORD_PREPARE_INTENT,
    WAL_RECORD_OBSERVE_PREPARE,
    WAL_RECORD_LOCK_AND_COMMIT,
    WAL_RECORD_TIMEOUT_INTENT,
    WAL_RECORD_INSTALL_TIMEOUT,
    WAL_RECORD_DECISION,
    REPLAY_EFFECT_NONE,
    BOUNDARY_NONE,
    BOUNDARY_BEGIN_WAL,
    BOUNDARY_ACKNOWLEDGE_WAL,
    BOUNDARY_COMPLETE_APPLICATION,
    BOUNDARY_RESUME_AFTER_REPLAY,
    CERTIFICATE_EVIDENCE_ABSENT,
    CERTIFICATE_EVIDENCE_LOCAL,
    CERTIFICATE_EVIDENCE_INCOMING,
    IDENTITY_DOMAIN_CONTEXT,
    IDENTITY_DOMAIN_SUBJECT,
    IDENTITY_DOMAIN_PAYLOAD,
    IDENTITY_DOMAIN_PEER,
    IDENTITY_DOMAIN_DURABLE_ARTIFACT,
    IDENTITY_DOMAIN_PROCESS_LOCAL,
    IDENTITY_KIND_CONSENSUS_CONTEXT,
    IDENTITY_KIND_CONSENSUS_SUBJECT,
    IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
    IDENTITY_KIND_WIRE_BLOCK_SUBJECT,
    IDENTITY_KIND_BLOCK_HEADER,
    IDENTITY_KIND_CANONICAL_PAYLOAD,
    IDENTITY_KIND_EXECUTION_COMMITMENT,
    IDENTITY_KIND_QUORUM_CERTIFICATE,
    IDENTITY_KIND_PAYLOAD_MANIFEST,
    IDENTITY_KIND_EXECUTED_BLOCK_WIRE,
    IDENTITY_KIND_SIDECAR_REQUEST,
    IDENTITY_KIND_REPLY_PAYLOAD,
    IDENTITY_KIND_MERGE_ENTRY,
    IDENTITY_KIND_REFERENCE_DIGEST,
    IDENTITY_KIND_NETWORK_RESPONSE,
    IDENTITY_KIND_SIDECAR_RESPONSE,
    IDENTITY_KIND_SIDECAR_CHUNK,
    IDENTITY_KIND_SIDECAR_PAYLOAD,
    IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST,
    IDENTITY_KIND_CONSENSUS_MESSAGE,
    IDENTITY_KIND_CERTIFIED_BODY_REQUEST,
    IDENTITY_KIND_SIDECAR_SIBLING_STATE,
    IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE,
    IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE,
    IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE,
    IDENTITY_KIND_REPLY_SOURCE_KEY,
    IDENTITY_KIND_REPLY_DELIVERY_ROUTE,
    IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,
    IDENTITY_KIND_LEADER_WIRE_LIFECYCLE,
    IDENTITY_KIND_DURABLE_BODY_FRAME,
    IDENTITY_KIND_FINALITY_ARTIFACT,
    IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD,
    IDENTITY_KIND_LANE_QUEUE_RESERVATION,
    IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER,
    IDENTITY_KIND_PEER,
    IN_FLIGHT_RESERVATION_STATE_ABSENT,
    IN_FLIGHT_RESERVATION_STATE_LIVE,
    IN_FLIGHT_RESERVATION_STATE_COMMITTED,
    IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED,
    IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED,
    IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,
    IN_FLIGHT_RESERVATION_ACTION_RESERVE,
    IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,
    IN_FLIGHT_RESERVATION_ACTION_COMMIT,
    IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT,
    IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE,
    IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE,
    IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,
    IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT,
    IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
    IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN,
    IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED,
    IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4,
    IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5,
    IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA,
    IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY,
    IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC,
    IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_PLAN_TOMBSTONE,
    IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_COMMIT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING,
    IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE,
    IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED,
    IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO,
    IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE,
    IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT,
    IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY,
    SUCCESSOR_AUTHORITY_APPLIED,
    SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP,
    SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP,
    SUCCESSOR_STAGE_NONE,
    SUCCESSOR_STAGE_QUEUED,
    SUCCESSOR_STAGE_RUNNING,
    SUCCESSOR_STAGE_COMPLETE,
    SUCCESSOR_LIFECYCLE_BEGIN,
    SUCCESSOR_LIFECYCLE_FAIL,
    SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP,
    SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP,
    SUCCESSOR_MARKER_ACTIVATED,
);

/// Lossless fixed-width view of one existing canonical 256-bit identity.
///
/// `domain` and `kind` are kept outside the digest so projections of equal
/// bytes used for different protocol objects cannot be confused. The four
/// words preserve every input bit; this helper does not hash or truncate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CanonicalIdentityProjection {
    pub(crate) domain: u8,
    pub(crate) kind: u8,
    pub(crate) word0: u64,
    pub(crate) word1: u64,
    pub(crate) word2: u64,
    pub(crate) word3: u64,
}

impl CanonicalIdentityProjection {
    /// Canonical absent identity used by fixed-width optional projections.
    #[must_use]
    pub(crate) const fn zero() -> Self {
        Self {
            domain: 0,
            kind: 0,
            word0: 0,
            word1: 0,
            word2: 0,
            word3: 0,
        }
    }

    /// Project all 32 canonical bytes into four big-endian words.
    #[must_use]
    pub(crate) const fn from_bytes(domain: u8, kind: u8, bytes: [u8; 32]) -> Self {
        Self {
            domain,
            kind,
            word0: u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]),
            word1: u64::from_be_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ]),
            word2: u64::from_be_bytes([
                bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22],
                bytes[23],
            ]),
            word3: u64::from_be_bytes([
                bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30],
                bytes[31],
            ]),
        }
    }
}

macro_rules! canonical_identity_equal_body {
    ($left:expr, $right:expr) => {{
        $left.domain == $right.domain
            && $left.kind == $right.kind
            && $left.word0 == $right.word0
            && $left.word1 == $right.word1
            && $left.word2 == $right.word2
            && $left.word3 == $right.word3
    }};
}

macro_rules! canonical_identity_is_typed_body {
    ($identity:expr, $domain:expr, $kind:expr) => {{ $identity.domain == $domain && $identity.kind == $kind }};
}

macro_rules! canonical_identity_is_zero_body {
    ($identity:expr) => {{
        $identity.domain == 0u8
            && $identity.kind == 0u8
            && $identity.word0 == 0u64
            && $identity.word1 == 0u64
            && $identity.word2 == 0u64
            && $identity.word3 == 0u64
    }};
}

// Exact signable-vote statement identity shared by production and Verus.
// Authenticated signer identity is deliberately absent: distinct roster
// members must be able to contribute signatures to one semantic statement.
// Signature and roster validation remain independent ingress obligations.
macro_rules! vote_statement_identity_equal_body {
    (
        $left_context:expr,
        $left_height:expr,
        $left_view:expr,
        $left_proposal_height:expr,
        $left_proposal_view:expr,
        $left_phase:expr,
        $left_subject:expr,
        $right_context:expr,
        $right_height:expr,
        $right_view:expr,
        $right_proposal_height:expr,
        $right_proposal_view:expr,
        $right_phase:expr,
        $right_subject:expr $(,)?
    ) => {{
        $left_context == $right_context
            && $left_height == $right_height
            && $left_view == $right_view
            && $left_proposal_height == $right_proposal_height
            && $left_proposal_view == $right_proposal_view
            && $left_phase == $right_phase
            && $left_subject == $right_subject
    }};
}

// Stable certificate body identity used only after the caller has validated
// the certificate phase. Reproposal/finality rounds and signer evidence are
// representation details; context, height, and subject are semantic.
macro_rules! certificate_height_subject_identity_equal_body {
    (
        $left_context:expr,
        $left_height:expr,
        $left_subject:expr,
        $right_context:expr,
        $right_height:expr,
        $right_subject:expr $(,)?
    ) => {{
        $left_context == $right_context
            && $left_height == $right_height
            && $left_subject == $right_subject
    }};
}

// One fixed-width predicate decides whether an already-installed timeout
// round may be replayed as a lock-only upgrade. Production WAL replay,
// reducer admission/acknowledgement, and the Verus WAL relation instantiate
// this exact expression from their primitive projections. Keeping the
// subtraction form inside the kernel is essential: it expresses the exact
// predecessor relation without overflowing fixed-width production views and
// remains the same expression over Verus mathematical integers.
macro_rules! strict_same_round_timeout_upgrade_body {
    ($projection:expr, $zero:expr, $one:expr) => {{
        let projection = $projection;
        projection.current_view > $zero
            && projection.timeout_view == projection.current_view - $one
            && projection.installed_same_round
            && projection.selected_prepare_present
            && (!projection.highest_prepare_present
                || projection.selected_prepare_view > projection.highest_prepare_view)
            && (!projection.locked_prepare_present
                || projection.selected_prepare_view > projection.locked_prepare_view)
    }};
}

/// Primitive production projection for a strict same-round timeout upgrade.
///
/// The caller derives presence and exact installed-round identity from the
/// authenticated certificate and durable state. The pure kernel below owns
/// every rank, predecessor, and overflow comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StrictSameRoundTimeoutUpgradeProjection {
    pub(crate) current_view: u64,
    pub(crate) timeout_view: u64,
    pub(crate) installed_same_round: bool,
    pub(crate) selected_prepare_present: bool,
    pub(crate) selected_prepare_view: u64,
    pub(crate) highest_prepare_present: bool,
    pub(crate) highest_prepare_view: u64,
    pub(crate) locked_prepare_present: bool,
    pub(crate) locked_prepare_view: u64,
}

/// Decide whether a second certificate for the installed timeout round may
/// update only the durable lock while retaining the current view.
#[must_use]
pub(crate) const fn strict_same_round_timeout_upgrade_is_allowed(
    projection: StrictSameRoundTimeoutUpgradeProjection,
) -> bool {
    let zero: u64 = 0;
    let one: u64 = 1;
    strict_same_round_timeout_upgrade_body!(projection, zero, one)
}

// A locally generated proposal in a non-zero view must carry the exact latest
// durable timeout certificate, rather than merely some valid certificate for
// the predecessor round. The full concrete equality classification below
// binds otherwise-unbounded group/signature evidence; the remaining primitive
// fields make every consensus-relevant projection explicit to both production
// and Verus.
macro_rules! local_proposal_timeout_justification_body {
    ($projection:expr, $zero:expr, $one:expr, $absent_evidence:expr) => {{
        let projection = $projection;
        projection.current_view > $zero
            && projection.proposal_view == projection.current_view
            && projection.proposal_timeout_context_id == projection.expected_context_id
            && projection.durable_timeout_context_id == projection.expected_context_id
            && projection.proposal_timeout_height == projection.expected_height
            && projection.durable_timeout_height == projection.expected_height
            && projection.proposal_timeout_view == projection.current_view - $one
            && projection.durable_timeout_view == projection.current_view - $one
            && projection.proposal_timeout_group_count > $zero
            && projection.proposal_timeout_group_count == projection.durable_timeout_group_count
            && projection.proposal_timeout_high_present == projection.durable_timeout_high_present
            && (!projection.proposal_timeout_high_present
                || (projection.proposal_timeout_high_view == projection.durable_timeout_high_view
                    && projection.proposal_timeout_high_subject
                        == projection.durable_timeout_high_subject))
            && projection.proposal_timeout_evidence_identity != $absent_evidence
            && projection.proposal_timeout_evidence_identity
                == projection.durable_timeout_evidence_identity
    }};
}

/// Fixed-width production projection of one local proposal's timeout
/// justification and the latest certificate reconstructed from the safety
/// WAL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalProposalTimeoutJustificationProjection {
    pub(crate) expected_context_id: ContextId,
    pub(crate) expected_height: u64,
    pub(crate) current_view: u64,
    pub(crate) proposal_view: u64,
    pub(crate) proposal_timeout_context_id: ContextId,
    pub(crate) proposal_timeout_height: u64,
    pub(crate) proposal_timeout_view: u64,
    pub(crate) proposal_timeout_group_count: u64,
    pub(crate) proposal_timeout_high_present: bool,
    pub(crate) proposal_timeout_high_view: u64,
    pub(crate) proposal_timeout_high_subject: Subject,
    pub(crate) proposal_timeout_evidence_identity: u8,
    pub(crate) durable_timeout_context_id: ContextId,
    pub(crate) durable_timeout_height: u64,
    pub(crate) durable_timeout_view: u64,
    pub(crate) durable_timeout_group_count: u64,
    pub(crate) durable_timeout_high_present: bool,
    pub(crate) durable_timeout_high_view: u64,
    pub(crate) durable_timeout_high_subject: Subject,
    pub(crate) durable_timeout_evidence_identity: u8,
}

const LOCAL_PROPOSAL_TIMEOUT_EVIDENCE_MATCHED: u8 = 1;
const LOCAL_PROPOSAL_TIMEOUT_EVIDENCE_FOREIGN: u8 = 2;

fn local_proposal_timeout_projection(
    expected_context_id: ContextId,
    expected_height: u64,
    current_view: u64,
    proposal_view: u64,
    proposal_timeout: &TimeoutCertificate,
    durable_timeout: &TimeoutCertificate,
) -> LocalProposalTimeoutJustificationProjection {
    let proposal_high = proposal_timeout.highest_prepare();
    let durable_high = durable_timeout.highest_prepare();
    LocalProposalTimeoutJustificationProjection {
        expected_context_id,
        expected_height,
        current_view,
        proposal_view,
        proposal_timeout_context_id: proposal_timeout.context_id(),
        proposal_timeout_height: proposal_timeout.round().height(),
        proposal_timeout_view: proposal_timeout.round().view(),
        proposal_timeout_group_count: u64::try_from(proposal_timeout.groups().len())
            .unwrap_or(u64::MAX),
        proposal_timeout_high_present: proposal_high.is_some(),
        proposal_timeout_high_view: proposal_high
            .map_or(0, |certificate| certificate.round().view()),
        proposal_timeout_high_subject: proposal_high
            .map_or_else(Subject::default, |certificate| certificate.subject()),
        proposal_timeout_evidence_identity: if proposal_timeout == durable_timeout {
            LOCAL_PROPOSAL_TIMEOUT_EVIDENCE_MATCHED
        } else {
            LOCAL_PROPOSAL_TIMEOUT_EVIDENCE_FOREIGN
        },
        durable_timeout_context_id: durable_timeout.context_id(),
        durable_timeout_height: durable_timeout.round().height(),
        durable_timeout_view: durable_timeout.round().view(),
        durable_timeout_group_count: u64::try_from(durable_timeout.groups().len())
            .unwrap_or(u64::MAX),
        durable_timeout_high_present: durable_high.is_some(),
        durable_timeout_high_view: durable_high.map_or(0, |certificate| certificate.round().view()),
        durable_timeout_high_subject: durable_high
            .map_or_else(Subject::default, |certificate| certificate.subject()),
        durable_timeout_evidence_identity: LOCAL_PROPOSAL_TIMEOUT_EVIDENCE_MATCHED,
    }
}

/// Decide whether a non-zero-view local proposal carries the exact latest
/// timeout certificate recovered from durable state.
///
/// Full certificate equality, including every timeout group and signature
/// share, is classified inside this function. Callers provide certificates,
/// never a precomputed authorization bit.
#[must_use]
pub(crate) fn local_proposal_timeout_justification_is_exact(
    expected_context_id: ContextId,
    expected_height: u64,
    current_view: u64,
    proposal_view: u64,
    proposal_timeout: &TimeoutCertificate,
    durable_timeout: Option<&TimeoutCertificate>,
) -> bool {
    let Some(durable_timeout) = durable_timeout else {
        return false;
    };
    let projection = local_proposal_timeout_projection(
        expected_context_id,
        expected_height,
        current_view,
        proposal_view,
        proposal_timeout,
        durable_timeout,
    );
    let zero: u64 = 0;
    let one: u64 = 1;
    let absent_evidence: u8 = 0;
    local_proposal_timeout_justification_body!(projection, zero, one, absent_evidence)
}

// These expressions are instantiated by both the executable production
// decision gates below and their exact Verus mirrors. Every authorization
// decision is derived from lossless identities, heights, stages, and marker
// fields observed at the enforcing seam; callers cannot supply an already
// computed validity bit.
macro_rules! durable_predecessor_is_canonical_body {
    ($predecessor:expr) => {{
        $predecessor.height > 0u64
            && canonical_identity_is_typed_body!(
                $predecessor.block_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_BLOCK_HEADER)
            )
            && canonical_identity_is_typed_body!(
                $predecessor.artifact_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                refinement_tag_value!(IDENTITY_KIND_FINALITY_ARTIFACT)
            )
    }};
}

macro_rules! durable_predecessor_is_zero_body {
    ($predecessor:expr) => {{
        $predecessor.height == 0u64
            && canonical_identity_is_zero_body!($predecessor.block_hash)
            && canonical_identity_is_zero_body!($predecessor.artifact_hash)
    }};
}

macro_rules! durable_predecessor_equal_body {
    ($left:expr, $right:expr) => {{
        durable_predecessor_is_canonical_body!($left)
            && durable_predecessor_is_canonical_body!($right)
            && $left.height == $right.height
            && canonical_identity_equal_body!($left.block_hash, $right.block_hash)
            && canonical_identity_equal_body!($left.artifact_hash, $right.artifact_hash)
    }};
}

macro_rules! production_successor_snapshot_body {
    ($predecessor_height:expr, $snapshot:expr) => {{
        $predecessor_height > 0u64
            && $predecessor_height < u64::MAX
            && $snapshot.height == $predecessor_height + 1u64
            && $snapshot.last_committed_height == $predecessor_height
            && canonical_identity_is_typed_body!(
                $snapshot.expected_context_id,
                refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
                refinement_tag_value!(IDENTITY_KIND_WIRE_HEIGHT_CONTEXT)
            )
            && canonical_identity_equal_body!(
                $snapshot.expected_context_id,
                $snapshot.published_context_id
            )
            && canonical_identity_equal_body!(
                $snapshot.published_context_id,
                $snapshot.marker_context_id
            )
            && $snapshot.marker_height == $snapshot.height
            && $snapshot.marker_view == $snapshot.view
            && $snapshot.marker_generation == $snapshot.generation
            && $snapshot.marker_kind == refinement_tag_value!(SUCCESSOR_MARKER_ACTIVATED)
            && $snapshot.marker_age_ms == 0u64
    }};
}

macro_rules! production_successor_predecessor_binding_body {
    ($projection:expr) => {{
        durable_predecessor_equal_body!(
            $projection.expected_predecessor,
            $projection.authority_predecessor
        ) && canonical_identity_is_typed_body!(
            $projection.successor_context_id,
            refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
            refinement_tag_value!(IDENTITY_KIND_WIRE_HEIGHT_CONTEXT)
        )
    }};
}

macro_rules! production_applied_successor_trace_body {
    ($projection:expr) => {{
        $projection.authority_kind == refinement_tag_value!(SUCCESSOR_AUTHORITY_APPLIED)
            && production_successor_predecessor_binding_body!($projection.binding)
            && $projection.predecessor_status_height
                == $projection.binding.expected_predecessor.height
            && $projection.predecessor_stage_before
                == refinement_tag_value!(SUCCESSOR_STAGE_RUNNING)
            && $projection.predecessor_stage_after
                == refinement_tag_value!(SUCCESSOR_STAGE_COMPLETE)
            && canonical_identity_equal_body!(
                $projection.binding.successor_context_id,
                $projection.successor.expected_context_id
            )
            && production_successor_snapshot_body!(
                $projection.binding.expected_predecessor.height,
                $projection.successor
            )
    }};
}

macro_rules! production_recovered_successor_trace_body {
    ($projection:expr) => {{
        $projection.published_status_height_before == 0u64
            && canonical_identity_equal_body!(
                $projection.authority_context_id,
                $projection.successor.expected_context_id
            )
            && production_successor_snapshot_body!(
                $projection.successor.last_committed_height,
                $projection.successor
            )
            && (if $projection.authority_kind
                == refinement_tag_value!(SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP)
            {
                durable_predecessor_is_canonical_body!($projection.predecessor)
                    && $projection.predecessor.height == $projection.successor.last_committed_height
                    && canonical_identity_is_zero_body!($projection.snapshot_record_hash)
                    && $projection.snapshot_height == 0u64
                    && canonical_identity_is_zero_body!($projection.snapshot_block_hash)
            } else if $projection.authority_kind
                == refinement_tag_value!(SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP)
            {
                durable_predecessor_is_zero_body!($projection.predecessor)
                    && canonical_identity_is_typed_body!(
                        $projection.snapshot_record_hash,
                        refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                        refinement_tag_value!(IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD)
                    )
                    && $projection.snapshot_height > 0u64
                    && $projection.snapshot_height == $projection.successor.last_committed_height
                    && canonical_identity_is_typed_body!(
                        $projection.snapshot_block_hash,
                        refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                        refinement_tag_value!(IDENTITY_KIND_BLOCK_HEADER)
                    )
            } else {
                false
            })
    }};
}

macro_rules! production_startup_failure_and_restart_trace_body {
    ($projection:expr) => {{
        $projection.status_height > 0u64
            && (if $projection.transition_kind == refinement_tag_value!(SUCCESSOR_LIFECYCLE_BEGIN) {
                $projection.authority_kind == refinement_tag_value!(SUCCESSOR_AUTHORITY_APPLIED)
                    && $projection.stage_before == refinement_tag_value!(SUCCESSOR_STAGE_QUEUED)
                    && $projection.stage_after == refinement_tag_value!(SUCCESSOR_STAGE_RUNNING)
                    && $projection.published_height_before == $projection.status_height
                    && $projection.published_height_after == $projection.status_height
                    && !$projection.restart_required_before
                    && !$projection.restart_required_after
            } else if $projection.transition_kind == refinement_tag_value!(SUCCESSOR_LIFECYCLE_FAIL)
            {
                $projection.authority_kind == refinement_tag_value!(SUCCESSOR_AUTHORITY_APPLIED)
                    && $projection.stage_before == refinement_tag_value!(SUCCESSOR_STAGE_RUNNING)
                    && $projection.stage_after == refinement_tag_value!(SUCCESSOR_STAGE_RUNNING)
                    && $projection.published_height_before == $projection.status_height
                    && $projection.published_height_after == $projection.status_height
                    && !$projection.restart_required_before
                    && $projection.restart_required_after
            } else if $projection.transition_kind
                == refinement_tag_value!(SUCCESSOR_LIFECYCLE_RETRY_COMPLETE_TIP)
            {
                $projection.authority_kind
                    == refinement_tag_value!(SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP)
                    && $projection.stage_before == refinement_tag_value!(SUCCESSOR_STAGE_NONE)
                    && $projection.stage_after == refinement_tag_value!(SUCCESSOR_STAGE_NONE)
                    && $projection.published_height_before == 0u64
                    && $projection.published_height_after == 0u64
                    && !$projection.restart_required_before
                    && !$projection.restart_required_after
            } else if $projection.transition_kind
                == refinement_tag_value!(SUCCESSOR_LIFECYCLE_SNAPSHOT_BOOTSTRAP)
            {
                $projection.authority_kind
                    == refinement_tag_value!(SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP)
                    && $projection.stage_before == refinement_tag_value!(SUCCESSOR_STAGE_NONE)
                    && $projection.stage_after == refinement_tag_value!(SUCCESSOR_STAGE_NONE)
                    && $projection.published_height_before == 0u64
                    && $projection.published_height_after == 0u64
                    && !$projection.restart_required_before
                    && !$projection.restart_required_after
            } else {
                false
            })
    }};
}

// Historical catch-up is admitted only through two exact ownership seams.
// The first consumes an authenticated CommitQC discovery request only after
// the exact certificate envelope entered reducer ingress. The second consumes
// an authenticated certified-body request only after its exact body owner and
// BodyAvailable reservation were committed. Both expressions are instantiated
// unchanged by production and Verus.
macro_rules! production_historical_certificate_trace_body {
    ($projection:expr) => {{
        $projection.context_height > 0u64
            && canonical_identity_is_typed_body!(
                $projection.context_id,
                refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
                refinement_tag_value!(IDENTITY_KIND_WIRE_HEIGHT_CONTEXT)
            )
            && $projection.certificate_height == $projection.context_height
            && canonical_identity_equal_body!(
                $projection.certificate_context_id,
                $projection.context_id
            )
            && canonical_identity_is_typed_body!(
                $projection.request_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST)
            )
            && canonical_identity_equal_body!(
                $projection.request_hash,
                $projection.response_request_hash
            )
            && canonical_identity_is_typed_body!(
                $projection.response_certificate,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_QUORUM_CERTIFICATE)
            )
            && canonical_identity_equal_body!(
                $projection.response_certificate,
                $projection.message_certificate
            )
            && canonical_identity_is_typed_body!(
                $projection.message_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_CONSENSUS_MESSAGE)
            )
            && canonical_identity_equal_body!(
                $projection.message_hash,
                $projection.admitted_message_hash
            )
            && $projection.request_present_before
            && !$projection.request_present_after
    }};
}

macro_rules! production_historical_body_pipeline_trace_body {
    ($projection:expr) => {{
        $projection.context_height > 0u64
            && canonical_identity_is_typed_body!(
                $projection.context_id,
                refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
                refinement_tag_value!(IDENTITY_KIND_WIRE_HEIGHT_CONTEXT)
            )
            && canonical_identity_is_typed_body!(
                $projection.request_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_CERTIFIED_BODY_REQUEST)
            )
            && canonical_identity_equal_body!(
                $projection.request_hash,
                $projection.pending_request_hash
            )
            && canonical_identity_equal_body!(
                $projection.pending_request_hash,
                $projection.authenticated_request_hash
            )
            && $projection.fetch_tag.height == $projection.context_height
            && canonical_identity_equal_body!($projection.round_context_id, $projection.context_id)
            && $projection.round_height == $projection.context_height
            && canonical_identity_is_typed_body!(
                $projection.subject,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_WIRE_BLOCK_SUBJECT)
            )
            && canonical_identity_equal_body!(
                $projection.manifest_round_context_id,
                $projection.round_context_id
            )
            && $projection.manifest_round_height == $projection.round_height
            && $projection.manifest_round_view == $projection.round_view
            && canonical_identity_equal_body!($projection.manifest_subject, $projection.subject)
            && canonical_identity_is_typed_body!(
                $projection.response_manifest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_PAYLOAD_MANIFEST)
            )
            && canonical_identity_equal_body!(
                $projection.response_manifest,
                $projection.ready_manifest
            )
            && canonical_identity_is_typed_body!(
                $projection.subject_payload_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_CANONICAL_PAYLOAD)
            )
            && canonical_identity_equal_body!(
                $projection.subject_payload_hash,
                $projection.body_payload_hash
            )
            && $projection.owner_present_after
            && $projection.owner_tag.height == $projection.fetch_tag.height
            && $projection.owner_tag.view == $projection.fetch_tag.view
            && $projection.owner_tag.generation == $projection.fetch_tag.generation
            && canonical_identity_equal_body!(
                $projection.owner_round_context_id,
                $projection.round_context_id
            )
            && $projection.owner_round_height == $projection.round_height
            && $projection.owner_round_view == $projection.round_view
            && canonical_identity_equal_body!($projection.owner_subject, $projection.subject)
            && !$projection.pending_fetch_present_after
            && !$projection.request_present_after
    }};
}

// One exact body-pipeline identity is carried unchanged through FetchBody,
// BodyAvailable, StoreBody, and ValidateBody. These macros are instantiated by
// typed production helpers below and by Verus over mathematical identities.
// Callers supply identities, never an authorization boolean.
macro_rules! exact_body_owner_equal_body {
    ($left:expr, $right:expr) => {{
        $left.tag.height == $right.tag.height
            && $left.tag.view == $right.tag.view
            && $left.tag.generation == $right.tag.generation
            && $left.key == $right.key
            && $left.manifest_hash == $right.manifest_hash
    }};
}

macro_rules! exact_body_owner_binding_body {
    ($current:expr, $incoming:expr, $owner_type:ident, $binding_type:ident) => {{
        match $current {
            None => Some($binding_type {
                owner: $incoming,
                already_owned: false,
            }),
            Some(current) => {
                if current.tag.height != $incoming.tag.height
                    || current.tag.view != $incoming.tag.view
                    || current.tag.generation != $incoming.tag.generation
                    || current.key != $incoming.key
                {
                    None
                } else {
                    match (current.manifest_hash, $incoming.manifest_hash) {
                        (Some(existing), Some(incoming)) if existing != incoming => None,
                        (Some(existing), _) => Some($binding_type {
                            owner: $owner_type {
                                tag: current.tag,
                                key: current.key,
                                manifest_hash: Some(existing),
                            },
                            already_owned: true,
                        }),
                        (None, incoming) => Some($binding_type {
                            owner: $owner_type {
                                tag: current.tag,
                                key: current.key,
                                manifest_hash: incoming,
                            },
                            already_owned: true,
                        }),
                    }
                }
            }
        }
    }};
}

macro_rules! tag_projection_strictly_advances_body {
    ($later:expr, $previous:expr) => {{
        $later.height == $previous.height
            && ($later.view > $previous.view
                || ($later.view == $previous.view && $later.generation > $previous.generation))
    }};
}

macro_rules! exact_body_owner_rebind_body {
    ($current:expr, $previous:expr, $rebound_tag:expr, $owner_type:ident) => {{
        if !exact_body_owner_equal_body!($current, $previous)
            || !tag_projection_strictly_advances_body!($rebound_tag, $previous.tag)
        {
            None
        } else {
            Some($owner_type {
                tag: $rebound_tag,
                key: $previous.key,
                manifest_hash: $previous.manifest_hash,
            })
        }
    }};
}

// Runtime ingress and the Busy-deferred lane jointly own each logical
// completion slot. Exactly one lane may own one exact evidence value.
macro_rules! exact_body_completion_ownership_body {
    (
        $ingress_owners:expr,
        $ingress_exact:expr,
        $deferred_owners:expr,
        $deferred_exact:expr,
        $vacant:expr,
        $exact:expr,
        $invalid:expr $(,)?
    ) => {{
        if $ingress_owners == 0
            && $ingress_exact == 0
            && $deferred_owners == 0
            && $deferred_exact == 0
        {
            $vacant
        } else if ($ingress_owners == 1
            && $ingress_exact == 1
            && $deferred_owners == 0
            && $deferred_exact == 0)
            || ($ingress_owners == 0
                && $ingress_exact == 0
                && $deferred_owners == 1
                && $deferred_exact == 1)
        {
            $exact
        } else {
            $invalid
        }
    }};
}

// Supersession retires two independently bounded byte classes. Sequential
// subtraction makes both the no-underflow precondition and the exact residual
// explicit without relying on an overflowing `retained + ready` sum.
macro_rules! exact_body_retirement_accounting_body {
    (
        $ready_before:expr,
        $retained_bytes:expr,
        $ready_bytes:expr,
        $store_before:expr,
        $store_bytes:expr,
        $accounting_type:ident $(,)?
    ) => {{
        if $retained_bytes > $ready_before {
            None
        } else {
            let after_retained = $ready_before - $retained_bytes;
            if $ready_bytes > after_retained || $store_bytes > $store_before {
                None
            } else {
                Some($accounting_type {
                    ready_after: after_retained - $ready_bytes,
                    store_after: $store_before - $store_bytes,
                })
            }
        }
    }};
}

// Exact three-class round-robin branch relation used by runtime ingress.
// Every call examines all classes from the persistent cursor and advances the
// cursor past the selected class; an empty call makes one full rotation.
macro_rules! bounded_service_selection_body {
    (
        $cursor:expr,
        $completion_ready:expr,
        $progress_ready:expr,
        $normal_ready:expr,
        $selection_type:ident $(,)?
    ) => {{
        if $cursor == 1u8 {
            if $completion_ready {
                $selection_type {
                    selected: 1u8,
                    next: 2u8,
                }
            } else if $progress_ready {
                $selection_type {
                    selected: 2u8,
                    next: 3u8,
                }
            } else if $normal_ready {
                $selection_type {
                    selected: 3u8,
                    next: 1u8,
                }
            } else {
                $selection_type {
                    selected: 0u8,
                    next: 1u8,
                }
            }
        } else if $cursor == 2u8 {
            if $progress_ready {
                $selection_type {
                    selected: 2u8,
                    next: 3u8,
                }
            } else if $normal_ready {
                $selection_type {
                    selected: 3u8,
                    next: 1u8,
                }
            } else if $completion_ready {
                $selection_type {
                    selected: 1u8,
                    next: 2u8,
                }
            } else {
                $selection_type {
                    selected: 0u8,
                    next: 2u8,
                }
            }
        } else if $cursor == 3u8 {
            if $normal_ready {
                $selection_type {
                    selected: 3u8,
                    next: 1u8,
                }
            } else if $completion_ready {
                $selection_type {
                    selected: 1u8,
                    next: 2u8,
                }
            } else if $progress_ready {
                $selection_type {
                    selected: 2u8,
                    next: 3u8,
                }
            } else {
                $selection_type {
                    selected: 0u8,
                    next: 3u8,
                }
            }
        } else {
            $selection_type {
                selected: 0u8,
                next: 0u8,
            }
        }
    }};
}

// A compact discriminated trace ties the four production effective-lock seams
// to one executable relation. Each producer fills only its action branch and
// must use canonical zeroes for every unrelated field. Verus instantiates this
// exact macro over its mathematical mirror, while production invokes one of
// the four checked wrappers below with values derived from live state.
macro_rules! effective_lock_trace_step_body {
    ($projection:expr) => {{
        if $projection.kind == 1u8 {
            $projection.relation_exact
                && $projection.protected_before <= 1u64
                && $projection.protected_after == $projection.protected_before
                && $projection.owner_before == $projection.protected_before
                && $projection.owner_after == $projection.owner_before
                && !$projection.owner_reused
                && $projection.ready_before == 0u64
                && $projection.retired_retained == 0u64
                && $projection.retired_ready == 0u64
                && $projection.ready_after == 0u64
                && $projection.store_before == 0u64
                && $projection.retired_store == 0u64
                && $projection.store_after == 0u64
                && $projection.cursor_before == 0u8
                && !$projection.completion_ready
                && !$projection.progress_ready
                && !$projection.normal_ready
                && $projection.selected == 0u8
                && $projection.cursor_after == 0u8
        } else if $projection.kind == 2u8 {
            $projection.relation_exact
                && $projection.protected_before <= 1u64
                && $projection.protected_after <= 1u64
                && $projection.protected_before <= $projection.protected_after
                && $projection.owner_before <= 1u64
                && $projection.owner_after == 1u64
                && $projection.owner_reused == ($projection.owner_before == 1u64)
                && $projection.ready_before == 0u64
                && $projection.retired_retained == 0u64
                && $projection.retired_ready == 0u64
                && $projection.ready_after == 0u64
                && $projection.store_before == 0u64
                && $projection.retired_store == 0u64
                && $projection.store_after == 0u64
                && $projection.cursor_before == 0u8
                && !$projection.completion_ready
                && !$projection.progress_ready
                && !$projection.normal_ready
                && $projection.selected == 0u8
                && $projection.cursor_after == 0u8
        } else if $projection.kind == 3u8 {
            $projection.relation_exact
                && $projection.protected_before == 0u64
                && $projection.protected_after == 0u64
                && $projection.owner_before == 0u64
                && $projection.owner_after == 0u64
                && !$projection.owner_reused
                && $projection.retired_retained <= $projection.ready_before
                && $projection.retired_ready
                    <= $projection.ready_before - $projection.retired_retained
                && $projection.ready_after
                    == $projection.ready_before
                        - $projection.retired_retained
                        - $projection.retired_ready
                && $projection.retired_store <= $projection.store_before
                && $projection.store_after == $projection.store_before - $projection.retired_store
                && $projection.cursor_before == 0u8
                && !$projection.completion_ready
                && !$projection.progress_ready
                && !$projection.normal_ready
                && $projection.selected == 0u8
                && $projection.cursor_after == 0u8
        } else if $projection.kind == 4u8 {
            $projection.relation_exact
                && $projection.protected_before == 0u64
                && $projection.protected_after == 0u64
                && $projection.owner_before == 0u64
                && $projection.owner_after == 0u64
                && !$projection.owner_reused
                && $projection.ready_before == 0u64
                && $projection.retired_retained == 0u64
                && $projection.retired_ready == 0u64
                && $projection.ready_after == 0u64
                && $projection.store_before == 0u64
                && $projection.retired_store == 0u64
                && $projection.store_after == 0u64
                && $projection.cursor_before >= 1u8
                && $projection.cursor_before <= 3u8
                && (if $projection.selected == 0u8 {
                    !$projection.completion_ready
                        && !$projection.progress_ready
                        && !$projection.normal_ready
                        && $projection.cursor_after == $projection.cursor_before
                } else if $projection.selected == 1u8 {
                    $projection.completion_ready && $projection.cursor_after == 2u8
                } else if $projection.selected == 2u8 {
                    $projection.progress_ready && $projection.cursor_after == 3u8
                } else if $projection.selected == 3u8 {
                    $projection.normal_ready && $projection.cursor_after == 1u8
                } else {
                    false
                })
        } else {
            false
        }
    }};
}

macro_rules! effective_lock_trace_claim_body {
    ($projection:expr, $kind:expr) => {{ $projection.kind == $kind && effective_lock_trace_step_is_valid($projection) }};
}

macro_rules! pending_projection_is_absent_body {
    ($pending:expr) => {{
        $pending.record_kind == refinement_tag_value!(WAL_RECORD_NONE)
            && $pending.continuation == refinement_tag_value!(CONTINUATION_NONE)
            && $pending.persistence_id == 0u64
            && canonical_identity_is_zero_body!($pending.context_id)
            && $pending.height == 0u64
            && $pending.view == 0u64
            && !$pending.proposal_present
            && $pending.proposal_height == 0u64
            && $pending.proposal_view == 0u64
            && canonical_identity_is_zero_body!($pending.subject)
    }};
}

macro_rules! pending_projection_equal_body {
    ($left:expr, $right:expr) => {{
        $left.record_kind == $right.record_kind
            && $left.continuation == $right.continuation
            && $left.persistence_id == $right.persistence_id
            && canonical_identity_equal_body!($left.context_id, $right.context_id)
            && $left.height == $right.height
            && $left.view == $right.view
            && $left.proposal_present == $right.proposal_present
            && $left.proposal_height == $right.proposal_height
            && $left.proposal_view == $right.proposal_view
            && canonical_identity_equal_body!($left.subject, $right.subject)
    }};
}

// A boundary tag names the reducer owner of the WAL operation, while the
// pending projection names the round carried by the record. Those rounds are
// intentionally distinct for a future TC, an immediate-predecessor TC carrying
// a strictly higher PrepareQC, and a Decision learned outside the local view.
// Every Vote/QC nevertheless binds one same-round proposal; keep that identity
// exact below even when the local owner has advanced farther.
macro_rules! pending_projection_matches_boundary_body {
    ($pending:expr, $boundary:expr) => {{
        $boundary.subject.present
            && $pending.record_kind == $boundary.record_kind
            && $pending.continuation == $boundary.continuation
            && $pending.persistence_id == $boundary.persistence_id
            && canonical_identity_equal_body!($pending.context_id, $boundary.context_identity)
            && $pending.proposal_present == $boundary.proposal_present
            && $pending.proposal_height == $boundary.proposal_height
            && $pending.proposal_view == $boundary.proposal_view
            && canonical_identity_equal_body!($pending.subject, $boundary.subject_identity)
    }};
}

macro_rules! pending_round_can_begin_body {
    ($pending:expr, $owner:expr) => {{
        $pending.height == $owner.height
            && match $pending.record_kind {
                refinement_tag_value!(WAL_RECORD_PROPOSAL_INTENT)
                | refinement_tag_value!(WAL_RECORD_PREPARE_INTENT)
                | refinement_tag_value!(WAL_RECORD_LOCK_AND_COMMIT)
                | refinement_tag_value!(WAL_RECORD_TIMEOUT_INTENT) => $pending.view == $owner.view,
                refinement_tag_value!(WAL_RECORD_OBSERVE_PREPARE) => $pending.view <= $owner.view,
                refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT) => {
                    $pending.view < u64::MAX
                        && ($pending.view >= $owner.view || $pending.view + 1u64 == $owner.view)
                }
                // A valid CommitQC decides independently of how far the local
                // view has advanced. Its exact round remains in `pending` and
                // is checked against the Persist effect below.
                refinement_tag_value!(WAL_RECORD_DECISION) => true,
                _ => false,
            }
    }};
}

macro_rules! pending_round_can_acknowledge_body {
    ($pending:expr, $owner_before:expr, $owner_after:expr) => {{
        if $pending.record_kind == refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT) {
            $pending.height == $owner_before.height
                && $pending.height == $owner_after.height
                && $pending.view < u64::MAX
                && $owner_after.view == $pending.view + 1u64
                && (if $pending.view + 1u64 == $owner_before.view {
                    $owner_before.generation < u64::MAX
                        && $owner_after.generation == $owner_before.generation + 1u64
                } else {
                    $pending.view >= $owner_before.view && $owner_after.generation == 0u64
                })
        } else {
            tag_projection_equal_body!($owner_before, $owner_after)
                && pending_round_can_begin_body!($pending, $owner_after)
        }
    }};
}

macro_rules! wal_record_proposal_round_is_exact_body {
    ($record_kind:expr, $pending:expr, $boundary:expr) => {{
        match $record_kind {
            refinement_tag_value!(WAL_RECORD_PROPOSAL_INTENT)
            | refinement_tag_value!(WAL_RECORD_PREPARE_INTENT)
            | refinement_tag_value!(WAL_RECORD_OBSERVE_PREPARE) => {
                $pending.proposal_present
                    && $pending.proposal_height == $pending.height
                    && $pending.proposal_view == $pending.view
            }
            refinement_tag_value!(WAL_RECORD_LOCK_AND_COMMIT) => {
                $pending.proposal_present
                    && $pending.proposal_height == $pending.height
                    && $pending.proposal_view == $pending.view
                    && $boundary.auxiliary_present
                    && $boundary.auxiliary_context_id == $boundary.context_id
                    && $boundary.auxiliary_height == $pending.proposal_height
                    && $boundary.auxiliary_view == $pending.proposal_view
                    && $boundary.auxiliary_proposal_height == $pending.proposal_height
                    && $boundary.auxiliary_proposal_view == $pending.proposal_view
                    && $boundary.auxiliary_phase == 1u8
                    && $boundary.auxiliary_subject == $boundary.subject.subject
            }
            refinement_tag_value!(WAL_RECORD_DECISION) => {
                $pending.proposal_present
                    && $pending.proposal_height == $pending.height
                    && $pending.proposal_view == $pending.view
            }
            refinement_tag_value!(WAL_RECORD_TIMEOUT_INTENT)
            | refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT) => {
                !$pending.proposal_present
                    && $pending.proposal_height == 0u64
                    && $pending.proposal_view == 0u64
            }
            _ => false,
        }
    }};
}

// An InstallTimeout record may be owned by its timeout round or by the
// immediate successor while an alternate TC supplies a newly learned high
// PrepareQC. Keep that predecessor exception tied to the full, internally
// same-round Prepare certificate projection; a no-high replay cannot mint the
// exceptional owner relation.
macro_rules! install_timeout_boundary_is_exact_body {
    ($boundary:expr, $pending:expr, $owner:expr) => {{
        let immediate_predecessor = $pending.view < $owner.view;
        (!immediate_predecessor || $boundary.auxiliary_present)
            && (if $boundary.auxiliary_present {
                $boundary.auxiliary_context_id == $boundary.context_id
                    && $boundary.auxiliary_height == $pending.height
                    && $boundary.auxiliary_view <= $pending.view
                    && $boundary.auxiliary_proposal_height == $boundary.auxiliary_height
                    && $boundary.auxiliary_proposal_view == $boundary.auxiliary_view
                    && $boundary.auxiliary_phase == 1u8
                    && $boundary.subject.subject == $boundary.auxiliary_subject
            } else {
                true
            })
    }};
}

macro_rules! wal_record_continuation_is_exact_body {
    ($record_kind:expr, $continuation:expr) => {{
        match $record_kind {
            refinement_tag_value!(WAL_RECORD_PROPOSAL_INTENT)
            | refinement_tag_value!(WAL_RECORD_PREPARE_INTENT)
            | refinement_tag_value!(WAL_RECORD_LOCK_AND_COMMIT)
            | refinement_tag_value!(WAL_RECORD_TIMEOUT_INTENT) => {
                $continuation == refinement_tag_value!(CONTINUATION_SIGN)
            }
            refinement_tag_value!(WAL_RECORD_OBSERVE_PREPARE) => {
                $continuation == refinement_tag_value!(CONTINUATION_NONE)
            }
            refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT) => {
                $continuation == refinement_tag_value!(CONTINUATION_INSTALL_TIMEOUT)
            }
            refinement_tag_value!(WAL_RECORD_DECISION) => {
                $continuation == refinement_tag_value!(CONTINUATION_DECIDE)
            }
            _ => false,
        }
    }};
}

macro_rules! wal_record_round_matches_owner_body {
    ($record_kind:expr, $pending:expr, $owner:expr) => {{
        $pending.height == $owner.height
            && match $record_kind {
                refinement_tag_value!(WAL_RECORD_PROPOSAL_INTENT)
                | refinement_tag_value!(WAL_RECORD_PREPARE_INTENT)
                | refinement_tag_value!(WAL_RECORD_LOCK_AND_COMMIT)
                | refinement_tag_value!(WAL_RECORD_TIMEOUT_INTENT) => $pending.view == $owner.view,
                refinement_tag_value!(WAL_RECORD_OBSERVE_PREPARE) => $pending.view <= $owner.view,
                refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT) => {
                    $pending.view < u64::MAX
                        && ($owner.view <= $pending.view || $pending.view + 1u64 == $owner.view)
                }
                refinement_tag_value!(WAL_RECORD_DECISION) => true,
                _ => false,
            }
    }};
}

macro_rules! event_can_start_wal_record_body {
    ($event_kind:expr, $record_kind:expr) => {{
        match $event_kind {
            0u8 => $record_kind == refinement_tag_value!(WAL_RECORD_PROPOSAL_INTENT),
            2u8 | 3u8 => {
                $record_kind == refinement_tag_value!(WAL_RECORD_OBSERVE_PREPARE)
                    || $record_kind == refinement_tag_value!(WAL_RECORD_LOCK_AND_COMMIT)
                    || $record_kind == refinement_tag_value!(WAL_RECORD_DECISION)
            }
            4u8 | 5u8 => $record_kind == refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT),
            6u8 => $record_kind == refinement_tag_value!(WAL_RECORD_TIMEOUT_INTENT),
            10u8 => {
                $record_kind == refinement_tag_value!(WAL_RECORD_PREPARE_INTENT)
                    || $record_kind == refinement_tag_value!(WAL_RECORD_LOCK_AND_COMMIT)
            }
            refinement_tag_value!(EVENT_SIGNED) => {
                $record_kind == refinement_tag_value!(WAL_RECORD_PREPARE_INTENT)
                    || $record_kind == refinement_tag_value!(WAL_RECORD_OBSERVE_PREPARE)
                    || $record_kind == refinement_tag_value!(WAL_RECORD_LOCK_AND_COMMIT)
                    || $record_kind == refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT)
                    || $record_kind == refinement_tag_value!(WAL_RECORD_DECISION)
            }
            _ => false,
        }
    }};
}

macro_rules! persist_slot_matches_boundary_body {
    ($slot:expr, $pending:expr, $boundary:expr) => {{
        if $slot.kind == refinement_tag_value!(EFFECT_PERSIST) {
            $slot.requested.persistence_id == $boundary.persistence_id
                && $slot.requested.record_kind == $boundary.record_kind
                && $slot.requested.context_id == $boundary.context_id
                && $slot.requested.height == $pending.height
                && $slot.requested.view == $pending.view
                && $slot.requested.proposal_height == $boundary.proposal_height
                && $slot.requested.proposal_view == $boundary.proposal_view
                && $slot.requested.subject == $boundary.subject.subject
                && $slot.requested.tag.height == $boundary.tag.height
                && $slot.requested.tag.view == $boundary.tag.view
                && $slot.requested.tag.generation == $boundary.tag.generation
                && $slot.requested.auxiliary_present == $boundary.auxiliary_present
                && $slot.requested.auxiliary_context_id == $boundary.auxiliary_context_id
                && $slot.requested.auxiliary_height == $boundary.auxiliary_height
                && $slot.requested.auxiliary_view == $boundary.auxiliary_view
                && $slot.requested.auxiliary_proposal_height == $boundary.auxiliary_proposal_height
                && $slot.requested.auxiliary_proposal_view == $boundary.auxiliary_proposal_view
                && $slot.requested.auxiliary_phase == $boundary.auxiliary_phase
                && $slot.requested.auxiliary_subject == $boundary.auxiliary_subject
        } else {
            true
        }
    }};
}

macro_rules! tag_projection_equal_body {
    ($left:expr, $right:expr) => {{
        $left.height == $right.height
            && $left.view == $right.view
            && $left.generation == $right.generation
    }};
}

// A validated lock must retain exactly one durable reconstruction witness.
// The expression is shared verbatim with Verus and derives exactness from
// primitive round, context, subject, signer, and persistence observations.
// A durable timeout is deliberately admissible only for a historical lock:
// it witnesses either a still-closing view or an installed predecessor TC
// from which the immutable locked body can be re-proposed in the current
// view. It never authorizes a Commit whose proposal origin is a closed view.
macro_rules! locked_commit_progress_witness_body {
    ($projection:expr) => {{
        let lock_is_active =
            canonical_identity_equal_body!($projection.locked_context_id, $projection.context_id)
                && $projection.locked_height == $projection.current_height;
        let lock_is_historical =
            lock_is_active && $projection.locked_view < $projection.current_view;
        let commit_is_exact = lock_is_active
            && $projection.commit_intent_present
            && $projection.local_validator_present
            && canonical_identity_equal_body!(
                $projection.commit_context_id,
                $projection.context_id
            )
            && $projection.commit_height == $projection.current_height
            && $projection.commit_view == $projection.locked_view
            && $projection.commit_proposal_height == $projection.locked_height
            && $projection.commit_proposal_view == $projection.locked_view
            && $projection.commit_phase == 2u8
            && canonical_identity_equal_body!(
                $projection.commit_subject,
                $projection.locked_subject
            )
            && $projection.commit_signer == $projection.local_validator
            && ($projection.commit_signature_pending || $projection.commit_pooled);
        let pending_lock_and_commit_is_exact = lock_is_active
            && !$projection.commit_intent_present
            && $projection.local_validator_present
            && $projection.pending.record_kind == refinement_tag_value!(WAL_RECORD_LOCK_AND_COMMIT)
            && $projection.pending.continuation == refinement_tag_value!(CONTINUATION_SIGN)
            && $projection.pending.persistence_id > 0u64
            && canonical_identity_equal_body!(
                $projection.pending.context_id,
                $projection.context_id
            )
            && $projection.pending.height == $projection.current_height
            && $projection.pending.view == $projection.current_view
            && $projection.current_view == $projection.locked_view
            && $projection.pending.proposal_present
            && $projection.pending.proposal_height == $projection.locked_height
            && $projection.pending.proposal_view == $projection.locked_view
            && canonical_identity_equal_body!(
                $projection.pending.subject,
                $projection.locked_subject
            );
        let durable_timeout_is_exact = !$projection.commit_intent_present
            && lock_is_historical
            && $projection.local_validator_present
            && $projection.timeout_intent_present
            && $projection.timeout_intent_durable
            && canonical_identity_equal_body!(
                $projection.timeout_context_id,
                $projection.context_id
            )
            && $projection.timeout_height == $projection.current_height
            && $projection.timeout_view == $projection.current_view
            && $projection.timeout_signer == $projection.local_validator;
        let durable_reproposal_is_exact = !$projection.commit_intent_present
            && lock_is_historical
            && $projection.local_validator_present
            && $projection.installed_timeout_present
            && $projection.installed_timeout_durable
            && canonical_identity_equal_body!(
                $projection.installed_timeout_context_id,
                $projection.context_id
            )
            && $projection.installed_timeout_height == $projection.current_height
            && $projection.installed_timeout_view < u64::MAX
            && $projection.installed_timeout_view + 1u64 == $projection.current_view;
        commit_is_exact
            || pending_lock_and_commit_is_exact
            || durable_timeout_is_exact
            || durable_reproposal_is_exact
    }};
}

// These seven expressions are the source-shared production/Verus kernels for
// the progress-witness refinement.  Every field is a primitive observation at
// the enforcing production seam; in particular, callers cannot supply an
// already-computed "valid" or "owned" bit.
macro_rules! production_durable_intent_trace_body {
    ($projection:expr) => {{
        let boundary_exact = $projection.boundary_claimed.kind
            != refinement_tag_value!(BOUNDARY_NONE)
            && boundary_capability_equal_body!(
                $projection.boundary_claimed,
                $projection.boundary_granted
            );
        let persist_effects =
            effect_count_body!($projection.effects, refinement_tag_value!(EFFECT_PERSIST));
        let tag_matches_owner =
            tag_projection_equal_body!($projection.event_tag, $projection.owner_tag_before);
        let persistence_completion = $projection.event_kind
            == refinement_tag_value!(EVENT_PERSISTED)
            || $projection.event_kind == refinement_tag_value!(EVENT_PERSISTENCE_FAILED);
        effect_slots_authorized_body!($projection.effects)
            && persist_effects <= 1u64
            && $projection.durable_sequence_after >= $projection.durable_sequence_before
            && (if !tag_matches_owner {
                boundary_capability_is_absent_body!($projection.boundary_claimed)
                    && boundary_capability_is_absent_body!($projection.boundary_granted)
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && pending_projection_equal_body!(
                        $projection.pending_before,
                        $projection.pending_after
                    )
                    && $projection.effects.len == 0u8
                    && persist_effects == 0u64
                    && (if persistence_completion {
                        $projection.event_persistence_id > 0u64
                    } else {
                        $projection.event_persistence_id == 0u64
                    })
            } else if boundary_exact
                && $projection.boundary_claimed.kind == refinement_tag_value!(BOUNDARY_BEGIN_WAL)
            {
                pending_projection_is_absent_body!($projection.pending_before)
                    && tag_projection_equal_body!(
                        $projection.event_tag,
                        $projection.owner_tag_before
                    )
                    && boundary_identity_is_canonical_body!($projection.boundary_claimed)
                    && $projection.boundary_claimed.subject.present
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_after,
                        $projection.boundary_claimed.tag
                    )
                    && pending_projection_matches_boundary_body!(
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && pending_round_can_begin_body!(
                        $projection.pending_after,
                        $projection.owner_tag_after
                    )
                    && wal_record_round_matches_owner_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.pending_after,
                        $projection.owner_tag_before
                    )
                    && wal_record_proposal_round_is_exact_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && wal_record_continuation_is_exact_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.boundary_claimed.continuation
                    )
                    && (if $projection.boundary_claimed.record_kind
                        == refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT)
                    {
                        install_timeout_boundary_is_exact_body!(
                            $projection.boundary_claimed,
                            $projection.pending_after,
                            $projection.owner_tag_after
                        )
                    } else {
                        true
                    })
                    && event_can_start_wal_record_body!(
                        $projection.event_kind,
                        $projection.boundary_claimed.record_kind
                    )
                    && $projection.durable_sequence_before < u64::MAX
                    && $projection.pending_after.persistence_id
                        == $projection.durable_sequence_before + 1u64
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && $projection.event_kind != refinement_tag_value!(EVENT_PERSISTED)
                    && $projection.event_persistence_id == 0u64
                    && persist_effects == 1u64
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot0,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot1,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot2,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot3,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot4,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot5,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot6,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
                    && persist_slot_matches_boundary_body!(
                        $projection.effects.slot7,
                        $projection.pending_after,
                        $projection.boundary_claimed
                    )
            } else if boundary_exact
                && $projection.boundary_claimed.kind
                    == refinement_tag_value!(BOUNDARY_ACKNOWLEDGE_WAL)
            {
                tag_projection_equal_body!($projection.event_tag, $projection.owner_tag_before)
                    && pending_projection_matches_boundary_body!(
                        $projection.pending_before,
                        $projection.boundary_claimed
                    )
                    && boundary_identity_is_canonical_body!($projection.boundary_claimed)
                    && pending_round_can_acknowledge_body!(
                        $projection.pending_before,
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && $projection.boundary_claimed.subject.present
                    && wal_record_continuation_is_exact_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.boundary_claimed.continuation
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_after,
                        $projection.boundary_claimed.tag
                    )
                    && wal_record_round_matches_owner_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.pending_before,
                        $projection.owner_tag_before
                    )
                    && wal_record_proposal_round_is_exact_body!(
                        $projection.boundary_claimed.record_kind,
                        $projection.pending_before,
                        $projection.boundary_claimed
                    )
                    && (if $projection.boundary_claimed.record_kind
                        == refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT)
                    {
                        install_timeout_boundary_is_exact_body!(
                            $projection.boundary_claimed,
                            $projection.pending_before,
                            $projection.owner_tag_before
                        )
                    } else {
                        true
                    })
                    && (if $projection.boundary_claimed.record_kind
                        == refinement_tag_value!(WAL_RECORD_INSTALL_TIMEOUT)
                    {
                        tag_projection_strictly_advances_body!(
                            $projection.owner_tag_after,
                            $projection.owner_tag_before
                        )
                    } else {
                        tag_projection_equal_body!(
                            $projection.owner_tag_before,
                            $projection.owner_tag_after
                        )
                    })
                    && pending_projection_is_absent_body!($projection.pending_after)
                    && $projection.durable_sequence_before < u64::MAX
                    && $projection.durable_sequence_after
                        == $projection.durable_sequence_before + 1u64
                    && $projection.pending_before.persistence_id
                        == $projection.durable_sequence_after
                    && $projection.event_kind == refinement_tag_value!(EVENT_PERSISTED)
                    && $projection.event_persistence_id
                        == $projection.boundary_claimed.persistence_id
                    && persist_effects == 0u64
            } else if boundary_exact
                && $projection.boundary_claimed.kind
                    == refinement_tag_value!(BOUNDARY_COMPLETE_APPLICATION)
            {
                tag_projection_equal_body!($projection.event_tag, $projection.owner_tag_before)
                    && boundary_identity_is_canonical_body!($projection.boundary_claimed)
                    && $projection.boundary_claimed.subject.present
                    && $projection.boundary_claimed.record_kind
                        == refinement_tag_value!(WAL_RECORD_NONE)
                    && $projection.boundary_claimed.continuation
                        == refinement_tag_value!(CONTINUATION_NONE)
                    && $projection.boundary_claimed.replay_effect_kind
                        == refinement_tag_value!(REPLAY_EFFECT_NONE)
                    && $projection.boundary_claimed.persistence_id == 0u64
                    && $projection.boundary_claimed.replay_plan.len == 0u8
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_after,
                        $projection.boundary_claimed.tag
                    )
                    && pending_projection_equal_body!(
                        $projection.pending_before,
                        $projection.pending_after
                    )
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && $projection.event_kind == 14u8
                    && $projection.event_persistence_id == 0u64
                    && persist_effects == 0u64
            } else if boundary_exact
                && $projection.boundary_claimed.kind
                    == refinement_tag_value!(BOUNDARY_RESUME_AFTER_REPLAY)
            {
                tag_projection_equal_body!($projection.event_tag, $projection.owner_tag_before)
                    && boundary_identity_is_canonical_body!($projection.boundary_claimed)
                    && $projection.boundary_claimed.record_kind
                        == refinement_tag_value!(WAL_RECORD_NONE)
                    && $projection.boundary_claimed.continuation
                        == refinement_tag_value!(CONTINUATION_NONE)
                    && $projection.boundary_claimed.persistence_id
                        == $projection.durable_sequence_before
                    && replay_plan_well_formed_body!(
                        $projection.boundary_claimed.replay_plan,
                        $projection.boundary_claimed.replay_effect_kind
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && tag_projection_equal_body!(
                        $projection.owner_tag_after,
                        $projection.boundary_claimed.tag
                    )
                    && pending_projection_equal_body!(
                        $projection.pending_before,
                        $projection.pending_after
                    )
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && $projection.event_kind == refinement_tag_value!(EVENT_RESUME_AFTER_REPLAY)
                    && $projection.event_persistence_id == 0u64
                    && persist_effects == 0u64
            } else {
                boundary_capability_is_absent_body!($projection.boundary_claimed)
                    && boundary_capability_is_absent_body!($projection.boundary_granted)
                    && tag_projection_equal_body!(
                        $projection.owner_tag_before,
                        $projection.owner_tag_after
                    )
                    && $projection.durable_sequence_after == $projection.durable_sequence_before
                    && pending_projection_equal_body!(
                        $projection.pending_before,
                        $projection.pending_after
                    )
                    && (if persistence_completion {
                        pending_projection_is_absent_body!($projection.pending_before)
                            && $projection.effects.len == 0u8
                            && $projection.event_persistence_id > 0u64
                    } else {
                        $projection.event_persistence_id == 0u64
                    })
                    && persist_effects == 0u64
            })
    }};
}

macro_rules! production_decision_identity_is_canonical_body {
    ($decision:expr) => {{
        canonical_identity_is_typed_body!(
            $decision.context_id,
            refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
            refinement_tag_value!(IDENTITY_KIND_WIRE_HEIGHT_CONTEXT)
        ) && $decision.height > 0u64
            && $decision.proposal_height == $decision.height
            && $decision.proposal_view == $decision.view
            && ($decision.phase == 1u8 || $decision.phase == 2u8)
            && canonical_identity_is_typed_body!(
                $decision.subject,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_WIRE_BLOCK_SUBJECT)
            )
            && canonical_identity_is_typed_body!(
                $decision.block_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_BLOCK_HEADER)
            )
            && canonical_identity_is_typed_body!(
                $decision.payload_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_CANONICAL_PAYLOAD)
            )
            && canonical_identity_is_typed_body!(
                $decision.execution_commitment,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_EXECUTION_COMMITMENT)
            )
            && canonical_identity_is_typed_body!(
                $decision.executed_block_wire_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_EXECUTED_BLOCK_WIRE)
            )
    }};
}

// Semantic Commit identity is independent of the same-round QC that witnessed
// it. Canonicality above still requires each individual QC to use one round;
// equality here retains every immutable body and execution commitment while
// deliberately excluding both certificate/proposal round fields.
macro_rules! production_decision_identity_equal_body {
    ($left:expr, $right:expr) => {{
        production_decision_identity_is_canonical_body!($left)
            && production_decision_identity_is_canonical_body!($right)
            && canonical_identity_equal_body!($left.context_id, $right.context_id)
            && $left.height == $right.height
            && $left.proposal_height == $right.proposal_height
            && $left.phase == $right.phase
            && canonical_identity_equal_body!($left.subject, $right.subject)
            && canonical_identity_equal_body!($left.block_hash, $right.block_hash)
            && canonical_identity_equal_body!($left.payload_hash, $right.payload_hash)
            && canonical_identity_equal_body!(
                $left.execution_commitment,
                $right.execution_commitment
            )
            && canonical_identity_equal_body!(
                $left.executed_block_wire_hash,
                $right.executed_block_wire_hash
            )
    }};
}

macro_rules! production_quorum_certificate_is_canonical_body {
    ($certificate:expr) => {{
        production_decision_identity_is_canonical_body!($certificate.decision)
            && canonical_identity_is_typed_body!(
                $certificate.certificate,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_QUORUM_CERTIFICATE)
            )
            && $certificate.signer_count > 0u64
            && $certificate.aggregate_signature_len > 0u64
    }};
}

macro_rules! production_quorum_certificate_equal_body {
    ($left:expr, $right:expr) => {{
        production_quorum_certificate_is_canonical_body!($left)
            && production_quorum_certificate_is_canonical_body!($right)
            && production_decision_identity_equal_body!($left.decision, $right.decision)
            && canonical_identity_equal_body!($left.certificate, $right.certificate)
            && $left.signer_count == $right.signer_count
            && $left.aggregate_signature_len == $right.aggregate_signature_len
    }};
}

macro_rules! production_durable_body_is_canonical_body {
    ($body:expr) => {{
        canonical_identity_is_typed_body!(
            $body.context_id,
            refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
            refinement_tag_value!(IDENTITY_KIND_WIRE_HEIGHT_CONTEXT)
        ) && $body.height > 0u64
            && canonical_identity_is_typed_body!(
                $body.subject,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_WIRE_BLOCK_SUBJECT)
            )
            && canonical_identity_is_typed_body!(
                $body.block_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_BLOCK_HEADER)
            )
            && canonical_identity_is_typed_body!(
                $body.payload_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_CANONICAL_PAYLOAD)
            )
            && canonical_identity_is_typed_body!(
                $body.manifest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_PAYLOAD_MANIFEST)
            )
            && canonical_identity_is_typed_body!(
                $body.frame,
                refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                refinement_tag_value!(IDENTITY_KIND_DURABLE_BODY_FRAME)
            )
    }};
}

macro_rules! production_durable_body_equal_body {
    ($left:expr, $right:expr) => {{
        production_durable_body_is_canonical_body!($left)
            && production_durable_body_is_canonical_body!($right)
            && canonical_identity_equal_body!($left.context_id, $right.context_id)
            && $left.height == $right.height
            && $left.view == $right.view
            && canonical_identity_equal_body!($left.subject, $right.subject)
            && canonical_identity_equal_body!($left.block_hash, $right.block_hash)
            && canonical_identity_equal_body!($left.payload_hash, $right.payload_hash)
            && canonical_identity_equal_body!($left.manifest, $right.manifest)
            && canonical_identity_equal_body!($left.frame, $right.frame)
    }};
}

macro_rules! production_decision_recovery_trace_body {
    ($projection:expr) => {{
        $projection.expected_height > 0u64
            && $projection.state_height <= $projection.expected_height
            && $projection.expected_height - $projection.state_height <= 1u64
            && canonical_identity_is_typed_body!(
                $projection.expected_context_id,
                refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
                refinement_tag_value!(IDENTITY_KIND_WIRE_HEIGHT_CONTEXT)
            )
            && canonical_identity_equal_body!(
                $projection.expected_context_id,
                $projection.frozen_context_id
            )
            && $projection.expected_height == $projection.frozen_height
            && $projection.replay_tag.height == $projection.frozen_height
            && tag_projection_equal_body!($projection.replay_tag, $projection.owner_tag)
            && $projection.replay_tag.generation == $projection.replay_generation
            && production_quorum_certificate_is_canonical_body!($projection.commit_qc)
            && $projection.commit_qc.decision.phase == 2u8
            && canonical_identity_equal_body!(
                $projection.commit_qc.decision.context_id,
                $projection.frozen_context_id
            )
            && $projection.commit_qc.decision.height == $projection.frozen_height
            && canonical_identity_equal_body!(
                $projection.commit_qc.decision.block_hash,
                $projection.expected_block_hash
            )
            && $projection.manifest_round.height == $projection.commit_qc.decision.proposal_height
            && $projection.manifest_round.view == $projection.commit_qc.decision.proposal_view
            && $projection.manifest_round.generation == 0u64
            && canonical_identity_equal_body!(
                $projection.manifest_subject,
                $projection.commit_qc.decision.subject
            )
            && canonical_identity_is_typed_body!(
                $projection.manifest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_PAYLOAD_MANIFEST)
            )
            && production_durable_body_equal_body!(
                $projection.durable_body,
                $projection.validated_body
            )
            && canonical_identity_equal_body!(
                $projection.durable_body.context_id,
                $projection.frozen_context_id
            )
            && $projection.durable_body.height == $projection.frozen_height
            && $projection.durable_body.view == $projection.manifest_round.view
            && $projection.durable_body.view == $projection.commit_qc.decision.proposal_view
            && canonical_identity_equal_body!(
                $projection.durable_body.subject,
                $projection.commit_qc.decision.subject
            )
            && canonical_identity_equal_body!(
                $projection.durable_body.block_hash,
                $projection.commit_qc.decision.block_hash
            )
            && canonical_identity_equal_body!(
                $projection.durable_body.payload_hash,
                $projection.commit_qc.decision.payload_hash
            )
            && canonical_identity_equal_body!(
                $projection.durable_body.manifest,
                $projection.manifest
            )
            && canonical_identity_equal_body!(
                $projection.validated_execution_commitment,
                $projection.commit_qc.decision.execution_commitment
            )
            && $projection.stage == 1u8
    }};
}

macro_rules! production_scheduler_trace_body {
    ($projection:expr) => {{
        if $projection.timeout_due {
            $projection.selected == 1u8 && $projection.fifo_owed_after == $projection.fifo_ready
        } else if $projection.fifo_ready && $projection.fifo_owed_before {
            $projection.selected == 3u8 && !$projection.fifo_owed_after
        } else if $projection.periodic_timer_due {
            $projection.selected == 2u8 && $projection.fifo_owed_after == $projection.fifo_ready
        } else if $projection.fifo_ready {
            $projection.selected == 3u8 && !$projection.fifo_owed_after
        } else {
            $projection.selected == 0u8 && !$projection.fifo_owed_after
        }
    }};
}

macro_rules! production_ingress_identity_and_class_trace_body {
    ($projection:expr) => {{
        let exact_ordinal_transition = $projection.ordinal_minted
            && $projection.ordinal_source_before == $projection.physical_admission_ordinal
            && $projection.physical_admission_ordinal < u128::MAX
            && $projection.ordinal_source_after == $projection.physical_admission_ordinal + 1u128;
        let exact_dormant_transition = if $projection.dormant_owner_ordinal == 0u128 {
            $projection.dormant_reservations_after == $projection.dormant_reservations_before
        } else {
            $projection.ordinal_minted
                && $projection.dormant_owner_ordinal == $projection.lifecycle_ordinal
                && $projection.lifecycle_ordinal < $projection.physical_admission_ordinal
                && $projection.dormant_reservations_after < u64::MAX
                && $projection.dormant_reservations_before
                    == $projection.dormant_reservations_after + 1u64
        };
        $projection.incoming_height == $projection.stored_height
            && $projection.incoming_view == $projection.stored_view
            && $projection.incoming_generation == $projection.stored_generation
            && $projection.incoming_class == $projection.stored_class
            && $projection.incoming_class >= 1u8
            && $projection.incoming_class <= 3u8
            && $projection.queue_len_before < u64::MAX
            && $projection.queue_len_after == $projection.queue_len_before + 1u64
            && $projection.queue_len_after <= $projection.queue_capacity
            && $projection.dormant_reservations_after <= $projection.queue_capacity
            && $projection.queue_len_after
                <= $projection.queue_capacity - $projection.dormant_reservations_after
            && $projection.physical_admission_ordinal > 0u128
            && $projection.lifecycle_ordinal > 0u128
            && $projection.lifecycle_ordinal <= $projection.physical_admission_ordinal
            && exact_ordinal_transition
            && exact_dormant_transition
    }};
}

macro_rules! production_ingress_reservation_materialization_trace_body {
    ($projection:expr) => {{
        let exact_dormant_transition = if $projection.dormant_owner_ordinal == 0u128 {
            $projection.dormant_reservations_after == $projection.dormant_reservations_before
        } else {
            $projection.dormant_owner_ordinal == $projection.lifecycle_ordinal
                && $projection.lifecycle_ordinal < $projection.physical_admission_ordinal
                && $projection.dormant_reservations_after < u64::MAX
                && $projection.dormant_reservations_before
                    == $projection.dormant_reservations_after + 1u64
        };
        $projection.incoming_height == $projection.stored_height
            && $projection.incoming_view == $projection.stored_view
            && $projection.incoming_generation == $projection.stored_generation
            && $projection.incoming_class == $projection.stored_class
            && $projection.incoming_class >= 1u8
            && $projection.incoming_class <= 3u8
            && $projection.queue_len_before < u64::MAX
            && $projection.queue_len_after == $projection.queue_len_before + 1u64
            && $projection.reserved_slots_before == 1u8
            && $projection.reserved_slots_after == 0u8
            && $projection.queue_len_after <= $projection.queue_capacity
            && $projection.dormant_reservations_after <= $projection.queue_capacity
            && $projection.queue_len_after
                <= $projection.queue_capacity - $projection.dormant_reservations_after
            && $projection.ordinal_source_before == $projection.ordinal_source_after
            && $projection.physical_admission_ordinal > 0u128
            && $projection.physical_admission_ordinal < $projection.ordinal_source_before
            && $projection.lifecycle_ordinal > 0u128
            && $projection.lifecycle_ordinal <= $projection.physical_admission_ordinal
            && exact_dormant_transition
    }};
}

macro_rules! production_leader_wire_admission_trace_body {
    ($projection:expr) => {{
        let incoming_identity_is_typed = canonical_identity_is_typed_body!(
            $projection.incoming_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
            refinement_tag_value!(IDENTITY_KIND_LEADER_WIRE_LIFECYCLE)
        );
        let incumbent_identity_is_typed = canonical_identity_is_typed_body!(
            $projection.incumbent_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
            refinement_tag_value!(IDENTITY_KIND_LEADER_WIRE_LIFECYCLE)
        );
        let stored_identity_is_exact = canonical_identity_equal_body!(
            $projection.incoming_identity,
            $projection.stored_identity
        );
        let common_after = stored_identity_is_exact
            && $projection.incoming_view == $projection.stored_view
            && $projection.incoming_admission_ordinal > 0u128
            && $projection.incoming_scheduler_ordinal > 0u128
            && $projection.stored_admission_ordinal > 0u128
            && $projection.stored_scheduler_ordinal > 0u128
            && $projection.records_before <= $projection.capacity
            && $projection.records_after <= $projection.capacity
            && $projection.capacity > 0u64;
        let incumbent_active_shape = match $projection.status_before {
            LEADER_WIRE_LIFECYCLE_INGRESS => {
                !$projection.terminal_evidence_before && !$projection.replay_dormant_before
            }
            LEADER_WIRE_LIFECYCLE_RUNTIME => {
                $projection.runtime_owner_before
                    && !$projection.terminal_evidence_before
                    && !$projection.replay_dormant_before
            }
            LEADER_WIRE_LIFECYCLE_VOLATILE_TERMINAL => {
                $projection.runtime_owner_before
                    && !$projection.terminal_evidence_before
                    && !$projection.replay_dormant_before
            }
            LEADER_WIRE_LIFECYCLE_TERMINAL => {
                $projection.runtime_owner_before
                    && $projection.terminal_evidence_before
                    && !$projection.replay_dormant_before
            }
            _ => false,
        };
        incoming_identity_is_typed
            && common_after
            && if $projection.operation == LEADER_WIRE_ADMISSION_INSERT {
                canonical_identity_is_zero_body!($projection.incumbent_identity)
                    && $projection.incumbent_view == 0u64
                    && $projection.incumbent_admission_ordinal == 0u128
                    && $projection.incumbent_scheduler_ordinal == 0u128
                    && $projection.status_before == LEADER_WIRE_LIFECYCLE_ABSENT
                    && $projection.status_after == LEADER_WIRE_LIFECYCLE_INGRESS
                    && $projection.stored_admission_ordinal
                        == $projection.incoming_admission_ordinal
                    && $projection.stored_scheduler_ordinal
                        == $projection.incoming_scheduler_ordinal
                    && !$projection.runtime_owner_before
                    && !$projection.runtime_owner_after
                    && !$projection.terminal_evidence_before
                    && !$projection.terminal_evidence_after
                    && !$projection.replay_dormant_before
                    && !$projection.replay_dormant_after
                    && $projection.last_admission_ordinal_before
                        < $projection.incoming_admission_ordinal
                    && $projection.scheduler_ordinal_high_watermark_before
                        < $projection.incoming_scheduler_ordinal
                    && $projection.last_admission_ordinal_after
                        == $projection.incoming_admission_ordinal
                    && $projection.scheduler_ordinal_high_watermark_after
                        == $projection.incoming_scheduler_ordinal
                    && $projection.records_before < u64::MAX
                    && $projection.records_after == $projection.records_before + 1u64
            } else if $projection.operation == LEADER_WIRE_ADMISSION_REACTIVATE {
                incumbent_identity_is_typed
                    && canonical_identity_equal_body!(
                        $projection.incumbent_identity,
                        $projection.incoming_identity
                    )
                    && $projection.incumbent_view == $projection.incoming_view
                    && $projection.incumbent_admission_ordinal
                        == $projection.stored_admission_ordinal
                    && $projection.incumbent_scheduler_ordinal
                        == $projection.stored_scheduler_ordinal
                    && $projection.status_before == LEADER_WIRE_LIFECYCLE_DORMANT
                    && $projection.status_after == LEADER_WIRE_LIFECYCLE_INGRESS
                    && $projection.runtime_owner_before == $projection.runtime_owner_after
                    && !$projection.terminal_evidence_before
                    && !$projection.terminal_evidence_after
                    && $projection.replay_dormant_before
                    && !$projection.replay_dormant_after
                    && $projection.last_admission_ordinal_before
                        == $projection.last_admission_ordinal_after
                    && $projection.scheduler_ordinal_high_watermark_before
                        == $projection.scheduler_ordinal_high_watermark_after
                    && $projection.last_admission_ordinal_before
                        >= $projection.stored_admission_ordinal
                    && $projection.scheduler_ordinal_high_watermark_before
                        >= $projection.stored_scheduler_ordinal
                    && $projection.records_before == $projection.records_after
            } else if $projection.operation == LEADER_WIRE_ADMISSION_COALESCE {
                incumbent_identity_is_typed
                    && canonical_identity_equal_body!(
                        $projection.incumbent_identity,
                        $projection.incoming_identity
                    )
                    && $projection.incumbent_view == $projection.incoming_view
                    && $projection.incumbent_admission_ordinal
                        == $projection.stored_admission_ordinal
                    && $projection.incumbent_scheduler_ordinal
                        == $projection.stored_scheduler_ordinal
                    && incumbent_active_shape
                    && $projection.status_after == $projection.status_before
                    && $projection.runtime_owner_after == $projection.runtime_owner_before
                    && $projection.terminal_evidence_after == $projection.terminal_evidence_before
                    && !$projection.replay_dormant_after
                    && $projection.last_admission_ordinal_before
                        == $projection.last_admission_ordinal_after
                    && $projection.scheduler_ordinal_high_watermark_before
                        == $projection.scheduler_ordinal_high_watermark_after
                    && $projection.last_admission_ordinal_before
                        >= $projection.stored_admission_ordinal
                    && $projection.scheduler_ordinal_high_watermark_before
                        >= $projection.stored_scheduler_ordinal
                    && $projection.records_before == $projection.records_after
            } else if $projection.operation == LEADER_WIRE_ADMISSION_REPLACE_TERMINAL {
                incumbent_identity_is_typed
                    && !canonical_identity_equal_body!(
                        $projection.incumbent_identity,
                        $projection.incoming_identity
                    )
                    && incumbent_active_shape
                    && ($projection.status_before == LEADER_WIRE_LIFECYCLE_VOLATILE_TERMINAL
                        || $projection.status_before == LEADER_WIRE_LIFECYCLE_TERMINAL)
                    && $projection.status_after == LEADER_WIRE_LIFECYCLE_INGRESS
                    && $projection.stored_admission_ordinal
                        == $projection.incoming_admission_ordinal
                    && $projection.stored_scheduler_ordinal
                        == $projection.incoming_scheduler_ordinal
                    && $projection.incoming_view > $projection.incumbent_view
                    && $projection.incoming_admission_ordinal
                        > $projection.incumbent_admission_ordinal
                    && $projection.incoming_scheduler_ordinal
                        > $projection.incumbent_scheduler_ordinal
                    && $projection.last_admission_ordinal_before
                        < $projection.incoming_admission_ordinal
                    && $projection.scheduler_ordinal_high_watermark_before
                        < $projection.incoming_scheduler_ordinal
                    && $projection.last_admission_ordinal_after
                        == $projection.incoming_admission_ordinal
                    && $projection.scheduler_ordinal_high_watermark_after
                        == $projection.incoming_scheduler_ordinal
                    && !$projection.runtime_owner_after
                    && !$projection.terminal_evidence_after
                    && !$projection.replay_dormant_after
                    && $projection.records_before == $projection.records_after
            } else {
                false
            }
    }};
}

macro_rules! production_two_stage_relay_retry_trace_body {
    ($projection:expr) => {{
        $projection.daemon_source_capacity_matches_two_upstream_lanes
            && $projection.class_corridor_covers_authenticated_sources
            && $projection.authenticated_source_matches_resource_owner
            && $projection.retry_route_same_delivery
            && $projection.retry_route_active
            && $projection.selected_eligible
            && $projection.ready_sources_before > 0u64
            && $projection.selected_source_rank_before < $projection.ready_sources_before
            && $projection.ready_sources_after == $projection.ready_sources_before
            && $projection.selected_source_rank_after < $projection.ready_sources_after
            && $projection.selected_source_rank_after == $projection.ready_sources_after - 1u64
            && $projection.source_depth_before > 0u64
            && $projection.selected_item_rank_before < $projection.source_depth_before
            && $projection.source_depth_after == $projection.source_depth_before
            && $projection.selected_item_rank_after < $projection.source_depth_after
            && $projection.selected_item_rank_after == $projection.source_depth_after - 1u64
            && $projection.total_depth_after == $projection.total_depth_before
            && $projection.source_capacity > 0u64
            && $projection.source_capacity < $projection.total_capacity
            && $projection.source_depth_before <= $projection.source_capacity
            && $projection.source_depth_after <= $projection.source_capacity
            && $projection.total_depth_before <= $projection.total_capacity
            && $projection.total_depth_after <= $projection.total_capacity
            && $projection.ready_sources_before <= $projection.total_depth_before
            && $projection.ready_sources_after <= $projection.total_depth_after
    }};
}

macro_rules! production_reliable_flush_trace_body {
    ($projection:expr) => {{
        canonical_identity_is_typed_body!(
            $projection.semantic_target,
            refinement_tag_value!(IDENTITY_DOMAIN_PEER),
            refinement_tag_value!(IDENTITY_KIND_PEER)
        ) && canonical_identity_is_typed_body!(
            $projection.authenticated_source,
            refinement_tag_value!(IDENTITY_DOMAIN_PEER),
            refinement_tag_value!(IDENTITY_KIND_PEER)
        ) && canonical_identity_is_typed_body!(
            $projection.source_key_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
            refinement_tag_value!(IDENTITY_KIND_REPLY_SOURCE_KEY)
        ) && canonical_identity_is_typed_body!(
            $projection.delivery_route_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
            refinement_tag_value!(IDENTITY_KIND_REPLY_DELIVERY_ROUTE)
        ) && canonical_identity_is_typed_body!(
            $projection.writer_occurrence_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
            refinement_tag_value!(IDENTITY_KIND_REPLY_WRITER_OCCURRENCE)
        ) && canonical_identity_is_typed_body!(
            $projection.requester,
            refinement_tag_value!(IDENTITY_DOMAIN_PEER),
            refinement_tag_value!(IDENTITY_KIND_PEER)
        ) && canonical_identity_is_typed_body!(
            $projection.responder,
            refinement_tag_value!(IDENTITY_DOMAIN_PEER),
            refinement_tag_value!(IDENTITY_KIND_PEER)
        ) && canonical_identity_equal_body!($projection.semantic_target, $projection.requester)
            && $projection.ticket_rank > 0u64
            && $projection.ticket_topic == 3u8
            && canonical_identity_is_typed_body!(
                $projection.canonical_request_digest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_REPLY_PAYLOAD)
            )
            && $projection.stream_wire_bytes > 0u64
            && canonical_identity_is_typed_body!(
                $projection.request_id,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_REQUEST)
            )
            && $projection.service_generation > 0u64
            && $projection.stream_epoch > 0u64
            && $projection.semantic_sequence > 0u64
            && canonical_identity_is_typed_body!(
                $projection.entry_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_MERGE_ENTRY)
            )
            && $projection.encoded_len > 0u64
            && canonical_identity_is_typed_body!(
                $projection.reference_digest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_REFERENCE_DIGEST)
            )
            && canonical_identity_is_typed_body!(
                $projection.canonical_response_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_NETWORK_RESPONSE)
            )
            && canonical_identity_is_typed_body!(
                $projection.sidecar_response_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_RESPONSE)
            )
            && canonical_identity_is_typed_body!(
                $projection.chunk_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_CHUNK)
            )
            && canonical_identity_is_typed_body!(
                $projection.payload_digest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_PAYLOAD)
            )
            && $projection.chunk_count > 0u64
            && $projection.chunk_index < $projection.chunk_count
            && $projection.message_cursor_before == 0u64
            && $projection.chunk_cursor_before == $projection.chunk_index
            && $projection.flushing_after <= $projection.capacity
            && $projection.admitted_after <= $projection.capacity - $projection.flushing_after
            && (if $projection.status == 1u8 {
                $projection.flushing_after == $projection.flushing_before
                    && $projection.admitted_after == $projection.admitted_before
                    && $projection.message_cursor_after == $projection.message_cursor_before
                    && $projection.chunk_cursor_after == $projection.chunk_cursor_before
            } else if $projection.status == 2u8 {
                $projection.flushing_after < u64::MAX
                    && $projection.flushing_before == $projection.flushing_after + 1u64
                    && $projection.admitted_before < u64::MAX
                    && $projection.admitted_after == $projection.admitted_before + 1u64
                    && $projection.message_cursor_before < u64::MAX
                    && $projection.message_cursor_after == $projection.message_cursor_before + 1u64
                    && $projection.chunk_cursor_before < u64::MAX
                    && $projection.chunk_cursor_after == $projection.chunk_cursor_before + 1u64
            } else if $projection.status == 3u8 {
                $projection.flushing_after < u64::MAX
                    && $projection.flushing_before == $projection.flushing_after + 1u64
                    && $projection.admitted_after == $projection.admitted_before
                    && $projection.message_cursor_after == $projection.message_cursor_before
                    && $projection.chunk_cursor_after == $projection.chunk_cursor_before
            } else {
                false
            })
    }};
}

// This second reliable-flush kernel is deliberately separate from the worker
// queue kernel above. The worker proves which immutable response occurrence
// crossed the peer writer; this kernel proves how that occurrence is applied
// to one source-owned sidecar lane without changing any sibling lane.
macro_rules! production_reliable_flush_application_body {
    ($projection:expr) => {{
        canonical_identity_is_typed_body!(
            $projection.semantic_target,
            refinement_tag_value!(IDENTITY_DOMAIN_PEER),
            refinement_tag_value!(IDENTITY_KIND_PEER)
        ) && canonical_identity_is_typed_body!(
            $projection.authenticated_source,
            refinement_tag_value!(IDENTITY_DOMAIN_PEER),
            refinement_tag_value!(IDENTITY_KIND_PEER)
        ) && canonical_identity_is_typed_body!(
            $projection.source_key_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
            refinement_tag_value!(IDENTITY_KIND_REPLY_SOURCE_KEY)
        ) && canonical_identity_is_typed_body!(
            $projection.delivery_route_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
            refinement_tag_value!(IDENTITY_KIND_REPLY_DELIVERY_ROUTE)
        ) && canonical_identity_is_typed_body!(
            $projection.writer_occurrence_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
            refinement_tag_value!(IDENTITY_KIND_REPLY_WRITER_OCCURRENCE)
        ) && canonical_identity_is_typed_body!(
            $projection.requester,
            refinement_tag_value!(IDENTITY_DOMAIN_PEER),
            refinement_tag_value!(IDENTITY_KIND_PEER)
        ) && canonical_identity_is_typed_body!(
            $projection.responder,
            refinement_tag_value!(IDENTITY_DOMAIN_PEER),
            refinement_tag_value!(IDENTITY_KIND_PEER)
        ) && canonical_identity_equal_body!($projection.semantic_target, $projection.requester)
            && $projection.ticket_rank > 0u64
            && $projection.ticket_topic == 3u8
            && canonical_identity_is_typed_body!(
                $projection.canonical_request_digest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_REPLY_PAYLOAD)
            )
            && $projection.stream_wire_bytes > 0u64
            && canonical_identity_is_typed_body!(
                $projection.request_id,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_REQUEST)
            )
            && $projection.service_generation > 0u64
            && $projection.stream_epoch > 0u64
            && $projection.semantic_sequence > 0u64
            && canonical_identity_is_typed_body!(
                $projection.entry_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_MERGE_ENTRY)
            )
            && $projection.encoded_len > 0u64
            && canonical_identity_is_typed_body!(
                $projection.reference_digest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_REFERENCE_DIGEST)
            )
            && canonical_identity_is_typed_body!(
                $projection.canonical_response_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_NETWORK_RESPONSE)
            )
            && canonical_identity_is_typed_body!(
                $projection.sidecar_response_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_RESPONSE)
            )
            && canonical_identity_is_typed_body!(
                $projection.chunk_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_CHUNK)
            )
            && canonical_identity_is_typed_body!(
                $projection.payload_digest,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_PAYLOAD)
            )
            && canonical_identity_equal_body!($projection.request_id, $projection.marker_request_id)
            && $projection.marker_service_generation > 0u64
            && $projection.marker_stream_epoch > 0u64
            && $projection.marker_semantic_sequence > 0u64
            && $projection.service_generation == $projection.marker_service_generation
            && $projection.stream_epoch == $projection.marker_stream_epoch
            && $projection.semantic_sequence == $projection.marker_semantic_sequence
            && canonical_identity_equal_body!($projection.entry_hash, $projection.marker_entry_hash)
            && $projection.encoded_len == $projection.marker_encoded_len
            && $projection.epoch_id == $projection.marker_epoch_id
            && canonical_identity_equal_body!(
                $projection.reference_digest,
                $projection.marker_reference_digest
            )
            && canonical_identity_equal_body!($projection.requester, $projection.marker_requester)
            && canonical_identity_equal_body!($projection.responder, $projection.marker_responder)
            && canonical_identity_equal_body!(
                $projection.canonical_response_hash,
                $projection.marker_canonical_response_hash
            )
            && canonical_identity_equal_body!(
                $projection.sidecar_response_hash,
                $projection.marker_sidecar_response_hash
            )
            && canonical_identity_equal_body!($projection.chunk_hash, $projection.marker_chunk_hash)
            && canonical_identity_equal_body!(
                $projection.payload_digest,
                $projection.marker_payload_digest
            )
            && $projection.chunk_index == $projection.marker_chunk_index
            && $projection.chunk_count == $projection.marker_chunk_count
            && $projection.ticket_topic == $projection.marker_topic
            && $projection.chunk_count > 0u64
            && $projection.chunk_index < $projection.chunk_count
            && $projection.message_cursor_before == 0u64
            && $projection.message_cursor_before < u64::MAX
            && $projection.message_cursor_after == $projection.message_cursor_before + 1u64
            && $projection.chunk_cursor_before == $projection.chunk_index
            && $projection.chunk_cursor_before < u64::MAX
            && $projection.chunk_cursor_after == $projection.chunk_cursor_before + 1u64
            && $projection.claim_acquired
            && $projection.gate_marker_present_before
            && !$projection.gate_marker_present_after
            && $projection.gate_cursor_before == $projection.chunk_index
            && $projection.gate_cursor_before < u64::MAX
            && $projection.gate_cursor_after == $projection.gate_cursor_before + 1u64
            && $projection.gate_cursor_after == $projection.chunk_cursor_after
            && $projection.gate_cursor_after <= $projection.chunk_count
            && $projection.gate_complete_after
                == ($projection.gate_cursor_after == $projection.chunk_count)
            && $projection.gate_attempt_present_after
            && canonical_identity_is_typed_body!(
                $projection.target_gate_residual_before,
                refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE)
            )
            && canonical_identity_is_typed_body!(
                $projection.target_gate_residual_after,
                refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE)
            )
            && $projection.target_gate_residual_records_equal
            && canonical_identity_equal_body!(
                $projection.target_gate_residual_before,
                $projection.target_gate_residual_after
            )
            && (!$projection.outbound_attempt_present_before
                || $projection.shared_transfer_present_before)
            && (if $projection.target_outbound_residual_records_equal {
                $projection.outbound_attempt_present_before
                    && $projection.outbound_attempt_present_after
                    && canonical_identity_is_typed_body!(
                        $projection.target_outbound_residual_before,
                        refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                        refinement_tag_value!(IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE)
                    )
                    && canonical_identity_is_typed_body!(
                        $projection.target_outbound_residual_after,
                        refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                        refinement_tag_value!(IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE)
                    )
                    && canonical_identity_equal_body!(
                        $projection.target_outbound_residual_before,
                        $projection.target_outbound_residual_after
                    )
            } else if $projection.outbound_attempt_present_before {
                canonical_identity_is_typed_body!(
                    $projection.target_outbound_residual_before,
                    refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                    refinement_tag_value!(IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE)
                ) && !$projection.outbound_attempt_present_after
                    && canonical_identity_is_zero_body!($projection.target_outbound_residual_after)
            } else {
                canonical_identity_is_zero_body!($projection.target_outbound_residual_before)
                    && !$projection.outbound_attempt_present_after
                    && canonical_identity_is_zero_body!($projection.target_outbound_residual_after)
            })
            && $projection.shared_transfer_present_after
                == ($projection.shared_transfer_present_before
                    && (!$projection.outbound_attempt_present_before
                        || $projection.outbound_attempt_present_after
                        || $projection.shared_transfer_other_attempts_before))
            && (!$projection.shared_transfer_other_attempts_before
                || $projection.shared_transfer_present_before)
            && (if $projection.shared_transfer_present_before {
                canonical_identity_is_typed_body!(
                    $projection.shared_transfer_state_before,
                    refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                    refinement_tag_value!(IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE)
                ) && (if $projection.shared_transfer_present_after {
                    $projection.shared_transfer_records_equal
                        && canonical_identity_is_typed_body!(
                            $projection.shared_transfer_state_after,
                            refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                            refinement_tag_value!(IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE)
                        )
                        && canonical_identity_equal_body!(
                            $projection.shared_transfer_state_before,
                            $projection.shared_transfer_state_after
                        )
                } else {
                    canonical_identity_is_zero_body!($projection.shared_transfer_state_after)
                })
            } else {
                !$projection.shared_transfer_present_after
                    && canonical_identity_is_zero_body!($projection.shared_transfer_state_before)
                    && canonical_identity_is_zero_body!($projection.shared_transfer_state_after)
            })
            && canonical_identity_is_typed_body!(
                $projection.sibling_state_before,
                refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_SIBLING_STATE)
            )
            && canonical_identity_is_typed_body!(
                $projection.sibling_state_after,
                refinement_tag_value!(IDENTITY_DOMAIN_PROCESS_LOCAL),
                refinement_tag_value!(IDENTITY_KIND_SIDECAR_SIBLING_STATE)
            )
            && $projection.sibling_records_equal
            && canonical_identity_equal_body!(
                $projection.sibling_state_before,
                $projection.sibling_state_after
            )
            && (if $projection.outbound_attempt_present_before
                && $projection.outbound_route_active_before
                && !$projection.gate_complete_after
            {
                $projection.inserted_preserved
            } else {
                $projection.inserted_equals_now
            })
            && $projection.outbound_order_count_before <= 1u64
            && $projection.outbound_order_count_after <= 1u64
            && $projection.outbound_queued_before
                == ($projection.outbound_order_count_before == 1u64)
            && $projection.outbound_queued_after == ($projection.outbound_order_count_after == 1u64)
            && $projection.sibling_order_len_after == $projection.sibling_order_len_before
            && (if $projection.outbound_order_count_before == 1u64 {
                $projection.outbound_order_rank_before <= $projection.sibling_order_len_before
            } else {
                $projection.outbound_order_rank_before == 0u64
            })
            && (if $projection.outbound_order_count_after == 1u64 {
                $projection.outbound_order_rank_after <= $projection.sibling_order_len_after
            } else {
                $projection.outbound_order_rank_after == 0u64
            })
            && (if $projection.outbound_attempt_present_before {
                $projection.outbound_route_bound_before
                    && $projection.outbound_cursor_before == $projection.chunk_index
                    && $projection.outbound_cursor_before < u64::MAX
                    && $projection.outbound_cursor_after
                        == $projection.outbound_cursor_before + 1u64
                    && (!$projection.outbound_in_flight_before_present
                        || $projection.outbound_in_flight_before == $projection.chunk_index)
                    && !$projection.outbound_in_flight_after_present
                    && (if $projection.outbound_route_active_before
                        && !$projection.gate_complete_after
                    {
                        $projection.outbound_attempt_present_after
                            && $projection.outbound_queued_after
                            && $projection.outbound_order_count_after == 1u64
                            && (if $projection.outbound_queued_before {
                                $projection.outbound_order_rank_after
                                    == $projection.outbound_order_rank_before
                            } else {
                                $projection.outbound_order_rank_after
                                    == $projection.sibling_order_len_before
                            })
                    } else {
                        !$projection.outbound_attempt_present_after
                            && !$projection.outbound_queued_after
                            && $projection.outbound_order_count_after == 0u64
                    })
            } else {
                !$projection.outbound_route_bound_before
                    && !$projection.outbound_route_active_before
                    && !$projection.outbound_in_flight_before_present
                    && !$projection.outbound_queued_before
                    && $projection.outbound_order_count_before == 0u64
                    && !$projection.outbound_attempt_present_after
                    && !$projection.outbound_in_flight_after_present
                    && !$projection.outbound_queued_after
                    && $projection.outbound_order_count_after == 0u64
            })
    }};
}

// The cross-tool proof consumes this exact bridge: a writer-flush projection
// and a lane-application projection are related only when every immutable
// request, ticket, response, and cursor field is the same flushed occurrence.
macro_rules! production_reliable_flush_two_phase_link_body {
    ($worker:expr, $application:expr) => {{
        $worker.status == 2u8
            && canonical_identity_equal_body!($worker.semantic_target, $application.semantic_target)
            && canonical_identity_equal_body!(
                $worker.authenticated_source,
                $application.authenticated_source
            )
            && canonical_identity_equal_body!(
                $worker.source_key_identity,
                $application.source_key_identity
            )
            && canonical_identity_equal_body!(
                $worker.delivery_route_identity,
                $application.delivery_route_identity
            )
            && canonical_identity_equal_body!(
                $worker.writer_occurrence_identity,
                $application.writer_occurrence_identity
            )
            && canonical_identity_equal_body!($worker.requester, $application.requester)
            && canonical_identity_equal_body!($worker.responder, $application.responder)
            && $worker.connection_tenure_ordinal_high == $application.connection_tenure_ordinal_high
            && $worker.connection_tenure_ordinal_low == $application.connection_tenure_ordinal_low
            && $worker.delivery_ordinal_high == $application.delivery_ordinal_high
            && $worker.delivery_ordinal_low == $application.delivery_ordinal_low
            && $worker.ticket_id == $application.ticket_id
            && $worker.ticket_rank == $application.ticket_rank
            && $worker.ticket_topic == $application.ticket_topic
            && $worker.reply_writer_timeout_attempt == $application.reply_writer_timeout_attempt
            && canonical_identity_equal_body!(
                $worker.canonical_request_digest,
                $application.canonical_request_digest
            )
            && $worker.stream_wire_bytes == $application.stream_wire_bytes
            && canonical_identity_equal_body!($worker.request_id, $application.request_id)
            && $worker.service_generation > 0u64
            && $application.service_generation > 0u64
            && $application.marker_service_generation > 0u64
            && $worker.service_generation == $application.service_generation
            && $worker.service_generation == $application.marker_service_generation
            && $worker.stream_epoch > 0u64
            && $application.stream_epoch > 0u64
            && $application.marker_stream_epoch > 0u64
            && $worker.stream_epoch == $application.stream_epoch
            && $worker.stream_epoch == $application.marker_stream_epoch
            && $worker.semantic_sequence > 0u64
            && $application.semantic_sequence > 0u64
            && $application.marker_semantic_sequence > 0u64
            && $worker.semantic_sequence == $application.semantic_sequence
            && $worker.semantic_sequence == $application.marker_semantic_sequence
            && canonical_identity_equal_body!($worker.entry_hash, $application.entry_hash)
            && $worker.encoded_len == $application.encoded_len
            && $worker.epoch_id == $application.epoch_id
            && canonical_identity_equal_body!(
                $worker.reference_digest,
                $application.reference_digest
            )
            && canonical_identity_equal_body!(
                $worker.canonical_response_hash,
                $application.canonical_response_hash
            )
            && canonical_identity_equal_body!(
                $worker.sidecar_response_hash,
                $application.sidecar_response_hash
            )
            && canonical_identity_equal_body!($worker.chunk_hash, $application.chunk_hash)
            && canonical_identity_equal_body!($worker.payload_digest, $application.payload_digest)
            && $worker.chunk_index == $application.chunk_index
            && $worker.chunk_count == $application.chunk_count
            && $worker.message_cursor_before == $application.message_cursor_before
            && $worker.message_cursor_after == $application.message_cursor_after
            && $worker.chunk_cursor_before == $application.chunk_cursor_before
            && $worker.chunk_cursor_after == $application.chunk_cursor_after
    }};
}

macro_rules! production_application_trace_body {
    ($projection:expr) => {{
        $projection.context_height > 0u64
            && $projection.task_tag.height == $projection.context_height
            // The lifecycle owner and CommitQC round are distinct domains.
            // Finality may arrive from either side of the local view; only the
            // independently captured current owner must equal the task tag.
            && tag_projection_equal_body!(
                $projection.task_tag,
                $projection.owner_tag
            )
            && $projection.task_tag.generation == $projection.task_generation
            && production_quorum_certificate_is_canonical_body!($projection.commit_qc)
            && $projection.commit_qc.decision.phase == 2u8
            && canonical_identity_equal_body!(
                $projection.context_id,
                $projection.commit_qc.decision.context_id
            )
            && $projection.context_height == $projection.commit_qc.decision.height
            && production_durable_body_is_canonical_body!($projection.validated_body)
            && canonical_identity_equal_body!(
                $projection.validated_body.context_id,
                $projection.context_id
            )
            && $projection.validated_body.height == $projection.context_height
            // Each CommitQC binds its proposal and vote to one exact round.
            // A later unchanged-body reproposal therefore carries a newly
            // validated body for that later round rather than a split-round QC.
            && $projection.validated_body.view
                == $projection.commit_qc.decision.proposal_view
            && canonical_identity_equal_body!(
                $projection.validated_body.subject,
                $projection.commit_qc.decision.subject
            )
            && canonical_identity_equal_body!(
                $projection.validated_body.block_hash,
                $projection.proposal_block_hash
            )
            && canonical_identity_equal_body!(
                $projection.validated_body.payload_hash,
                $projection.proposal_payload_hash
            )
            && canonical_identity_equal_body!(
                $projection.validated_execution_commitment,
                $projection.commit_qc.decision.execution_commitment
            )
            && canonical_identity_equal_body!(
                $projection.proposal_block_hash,
                $projection.commit_qc.decision.block_hash
            )
            && canonical_identity_equal_body!(
                $projection.proposal_payload_hash,
                $projection.commit_qc.decision.payload_hash
            )
            && canonical_identity_equal_body!(
                $projection.committed_block_hash,
                $projection.commit_qc.decision.block_hash
            )
            && canonical_identity_equal_body!(
                $projection.executed_block_wire_hash,
                $projection.commit_qc.decision.executed_block_wire_hash
            )
            && production_decision_identity_equal_body!(
                $projection.kura_decision,
                $projection.commit_qc.decision
            )
            && canonical_identity_equal_body!(
                $projection.kura_artifact_hash,
                $projection.artifact_hash
            )
            && canonical_identity_equal_body!(
                $projection.artifact_context_id,
                $projection.context_id
            )
            && $projection.artifact_height == $projection.context_height
            && canonical_identity_equal_body!(
                $projection.artifact_subject,
                $projection.commit_qc.decision.subject
            )
            && canonical_identity_equal_body!(
                $projection.artifact_block_hash,
                $projection.committed_block_hash
            )
            && production_quorum_certificate_equal_body!(
                $projection.artifact_commit_qc,
                $projection.commit_qc
            )
            && canonical_identity_is_typed_body!(
                $projection.artifact_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                refinement_tag_value!(IDENTITY_KIND_FINALITY_ARTIFACT)
            )
            && $projection.state_height_after == $projection.context_height
            && $projection.completion_work_id == $projection.task_work_id
    }};
}

// Exact application finalization is a distinct production boundary from
// successor construction.  This relation deliberately has no `MaxHeight`
// input: the finite terminal horizon is a TLA+ projection, while production
// proves that the authenticated receipt/artifact handoff itself neither owns
// nor publishes a successor activation.
macro_rules! production_terminal_application_without_successor_activation_body {
    ($projection:expr) => {{
        $projection.context_height > 0u64
            && canonical_identity_is_typed_body!(
                $projection.context_id,
                refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
                refinement_tag_value!(IDENTITY_KIND_WIRE_HEIGHT_CONTEXT)
            )
            && canonical_identity_equal_body!(
                $projection.receipt_context_id,
                $projection.context_id
            )
            && canonical_identity_equal_body!(
                $projection.artifact_context_id,
                $projection.context_id
            )
            && $projection.receipt_height == $projection.context_height
            && $projection.artifact_height == $projection.context_height
            && durable_predecessor_is_canonical_body!($projection.predecessor)
            && $projection.predecessor.height == $projection.context_height
            && canonical_identity_is_typed_body!(
                $projection.receipt_block_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_BLOCK_HEADER)
            )
            && canonical_identity_equal_body!(
                $projection.receipt_block_hash,
                $projection.predecessor.block_hash
            )
            && canonical_identity_equal_body!(
                $projection.artifact_block_hash,
                $projection.predecessor.block_hash
            )
            && canonical_identity_is_typed_body!(
                $projection.receipt_artifact_hash,
                refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                refinement_tag_value!(IDENTITY_KIND_FINALITY_ARTIFACT)
            )
            && canonical_identity_equal_body!(
                $projection.receipt_artifact_hash,
                $projection.predecessor.artifact_hash
            )
            && canonical_identity_equal_body!(
                $projection.artifact_hash,
                $projection.predecessor.artifact_hash
            )
            && !$projection.pending_successor_activation_present
    }};
}

// Primitive reservation ownership is deliberately narrower than the
// first-release in-flight TLA+ state. It binds one journal-owned transaction
// identity to one local durable state and, for ordered release, to one exact
// barrier identity. QueuePlan, Kura, carrier/WSV, FIFO collection extraction,
// and action ordering remain independent adapter obligations.
macro_rules! in_flight_reservation_owner_is_well_formed_body {
    ($owner:expr) => {{
        let owner = $owner;
        (owner.state == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT)
            && canonical_identity_is_zero_body!(owner.reservation_identity)
            && canonical_identity_is_zero_body!(owner.release_identity))
            || ((owner.state == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_LIVE)
                || owner.state == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_COMMITTED))
                && canonical_identity_is_typed_body!(
                    owner.reservation_identity,
                    refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                    refinement_tag_value!(IDENTITY_KIND_LANE_QUEUE_RESERVATION)
                )
                && canonical_identity_is_zero_body!(owner.release_identity))
            || ((owner.state
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED)
                || owner.state
                    == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED))
                && canonical_identity_is_typed_body!(
                    owner.reservation_identity,
                    refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                    refinement_tag_value!(IDENTITY_KIND_LANE_QUEUE_RESERVATION)
                )
                && canonical_identity_is_typed_body!(
                    owner.release_identity,
                    refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                    refinement_tag_value!(IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER)
                ))
    }};
}

macro_rules! in_flight_reservation_owner_equal_body {
    ($left:expr, $right:expr) => {{
        $left.state == $right.state
            && canonical_identity_equal_body!(
                $left.reservation_identity,
                $right.reservation_identity
            )
            && canonical_identity_equal_body!($left.release_identity, $right.release_identity)
    }};
}

macro_rules! in_flight_reservation_owner_names_request_body {
    ($owner:expr, $projection:expr) => {{
        canonical_identity_equal_body!(
            $owner.reservation_identity,
            $projection.requested_reservation_identity
        )
    }};
}

macro_rules! in_flight_reservation_owner_names_release_request_body {
    ($owner:expr, $projection:expr) => {{
        in_flight_reservation_owner_names_request_body!($owner, $projection)
            && canonical_identity_equal_body!(
                $owner.release_identity,
                $projection.requested_release_identity
            )
    }};
}

// This identity-preserving primitive journal relation is composed by the
// total fixed-width state/action relation below. Snapshot reconstruction maps
// to a named abstract stutter and direct release maps to its terminal FIFO
// owner. Retired mutation tags are absent from this relation and fail closed.
macro_rules! production_in_flight_reservation_transition_body {
    ($projection:expr) => {{
        let projection = $projection;
        canonical_identity_is_typed_body!(
            projection.requested_reservation_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
            refinement_tag_value!(IDENTITY_KIND_LANE_QUEUE_RESERVATION)
        ) && in_flight_reservation_owner_is_well_formed_body!(projection.before)
            && in_flight_reservation_owner_is_well_formed_body!(projection.after)
            && if projection.action
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT)
            {
                projection.before.state == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT)
                    && projection.after.state
                        != refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT)
                    && in_flight_reservation_owner_names_request_body!(projection.after, projection)
                    && if projection.after.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED)
                        || projection.after.state
                            == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED)
                    {
                        canonical_identity_is_typed_body!(
                            projection.requested_release_identity,
                            refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                            refinement_tag_value!(IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER)
                        ) && in_flight_reservation_owner_names_release_request_body!(
                            projection.after,
                            projection
                        )
                    } else {
                        canonical_identity_is_zero_body!(projection.requested_release_identity)
                    }
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_RESERVE)
            {
                canonical_identity_is_zero_body!(projection.requested_release_identity)
                    && ((projection.before.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT)
                        && projection.after.state
                            == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_LIVE)
                        && in_flight_reservation_owner_names_request_body!(
                            projection.after,
                            projection
                        ))
                        || (projection.before.state
                            == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_LIVE)
                            && in_flight_reservation_owner_names_request_body!(
                                projection.before,
                                projection
                            )
                            && in_flight_reservation_owner_equal_body!(
                                projection.before,
                                projection.after
                            )))
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT)
            {
                canonical_identity_is_zero_body!(projection.requested_release_identity)
                    && ((projection.before.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_LIVE)
                        && in_flight_reservation_owner_names_request_body!(
                            projection.before,
                            projection
                        )
                        && projection.after.state
                            == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT))
                        || (projection.before.state
                            != refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED)
                            && !(projection.before.state
                                == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_LIVE)
                                && in_flight_reservation_owner_names_request_body!(
                                    projection.before,
                                    projection
                                ))
                            && in_flight_reservation_owner_equal_body!(
                                projection.before,
                                projection.after
                            )))
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_COMMIT)
            {
                canonical_identity_is_zero_body!(projection.requested_release_identity)
                    && projection.after.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_COMMITTED)
                    && in_flight_reservation_owner_names_request_body!(projection.after, projection)
                    && ((projection.before.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_LIVE)
                        && in_flight_reservation_owner_names_request_body!(
                            projection.before,
                            projection
                        ))
                        || (projection.before.state
                            == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_COMMITTED)
                            && in_flight_reservation_owner_names_request_body!(
                                projection.before,
                                projection
                            )
                            && in_flight_reservation_owner_equal_body!(
                                projection.before,
                                projection.after
                            )))
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT)
            {
                canonical_identity_is_zero_body!(projection.requested_release_identity)
                    && ((projection.before.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_COMMITTED)
                        && in_flight_reservation_owner_names_request_body!(
                            projection.before,
                            projection
                        )
                        && projection.after.state
                            == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT))
                        || (in_flight_reservation_owner_equal_body!(
                            projection.before,
                            projection.after
                        ) && !(projection.before.state
                            == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_COMMITTED)
                            && in_flight_reservation_owner_names_request_body!(
                                projection.before,
                                projection
                            ))))
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE)
            {
                canonical_identity_is_typed_body!(
                    projection.requested_release_identity,
                    refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                    refinement_tag_value!(IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER)
                ) && ((projection.before.state
                    == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_LIVE)
                    && in_flight_reservation_owner_names_request_body!(
                        projection.before,
                        projection
                    )
                    && projection.after.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED)
                    && in_flight_reservation_owner_names_release_request_body!(
                        projection.after,
                        projection
                    ))
                    || ((projection.before.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED)
                        || projection.before.state
                            == refinement_tag_value!(
                                IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED
                            ))
                        && in_flight_reservation_owner_names_release_request_body!(
                            projection.before,
                            projection
                        )
                        && in_flight_reservation_owner_equal_body!(
                            projection.before,
                            projection.after
                        )))
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE)
            {
                canonical_identity_is_typed_body!(
                    projection.requested_release_identity,
                    refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                    refinement_tag_value!(IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER)
                ) && ((projection.before.state
                    == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_PREPARED)
                    && in_flight_reservation_owner_names_release_request_body!(
                        projection.before,
                        projection
                    )
                    && projection.after.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED)
                    && in_flight_reservation_owner_names_release_request_body!(
                        projection.after,
                        projection
                    ))
                    || (projection.before.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED)
                        && in_flight_reservation_owner_names_release_request_body!(
                            projection.before,
                            projection
                        )
                        && in_flight_reservation_owner_equal_body!(
                            projection.before,
                            projection.after
                        )))
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE)
            {
                canonical_identity_is_typed_body!(
                    projection.requested_release_identity,
                    refinement_tag_value!(IDENTITY_DOMAIN_DURABLE_ARTIFACT),
                    refinement_tag_value!(IDENTITY_KIND_LANE_QUEUE_RELEASE_BARRIER)
                ) && ((projection.before.state
                    == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED)
                    && in_flight_reservation_owner_names_release_request_body!(
                        projection.before,
                        projection
                    )
                    && projection.after.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT))
                    || (in_flight_reservation_owner_equal_body!(
                        projection.before,
                        projection.after
                    ) && !(projection.before.state
                        == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_RELEASE_COMPLETED)
                        && in_flight_reservation_owner_names_release_request_body!(
                            projection.before,
                            projection
                        ))))
            } else {
                false
            }
    }};
}

// The composed first-release projection below mirrors every field in
// `SumeragiV2InFlightFirstRelease.tla` with fixed-width primitives.  It is
// intentionally separate from the per-transaction journal seam above: the
// composed relation owns cross-store order, while the journal seam owns exact
// reservation and release-barrier identities.
macro_rules! in_flight_first_release_bitmap_count_body {
    ($bitmap:expr) => {{
        // Fixed-width SWAR population count. Keep this expression primitive:
        // the same macro is consumed by ordinary Rust and the Verus mirror.
        let bitmap = $bitmap;
        let pairs = bitmap - ((bitmap >> 1u32) & 0x5555_5555_5555_5555_5555_5555_5555_5555u128);
        let nibbles = (pairs & 0x3333_3333_3333_3333_3333_3333_3333_3333u128)
            + ((pairs >> 2u32) & 0x3333_3333_3333_3333_3333_3333_3333_3333u128);
        let bytes = (nibbles + (nibbles >> 4u32)) & 0x0f0f_0f0f_0f0f_0f0f_0f0f_0f0f_0f0f_0f0fu128;
        let words16 = bytes + (bytes >> 8u32);
        let words32 = words16 + (words16 >> 16u32);
        let words64 = words32 + (words32 >> 32u32);
        let words128 = words64 + (words64 >> 64u32);
        (words128 & 0xffu128) as u8
    }};
}

macro_rules! in_flight_first_release_validator_mask_body {
    ($validator_count:expr) => {{
        if $validator_count >= 128u8 {
            !0u128
        } else {
            (1u128 << $validator_count) - 1u128
        }
    }};
}

macro_rules! in_flight_first_release_ready_quorum_body {
    ($validator_count:expr) => {{
        if $validator_count == 0u8 {
            0u8
        } else {
            $validator_count - (($validator_count - 1u8) / 3u8)
        }
    }};
}

macro_rules! in_flight_first_release_single_validator_body {
    ($validator:expr, $validator_mask:expr) => {{
        $validator != 0u128
            && ($validator & !$validator_mask) == 0u128
            && ($validator & ($validator - 1u128)) == 0u128
    }};
}

macro_rules! in_flight_first_release_queue_equal_body {
    ($left:expr, $right:expr) => {{
        $left.plan_state == $right.plan_state
            && $left.selected_count == $right.selected_count
            && $left.reservation_state == $right.reservation_state
    }};
}

macro_rules! in_flight_first_release_carrier_equal_body {
    ($left:expr, $right:expr) => {{
        $left.kura_active == $right.kura_active
            && $left.execution_input_durable == $right.execution_input_durable
            && $left.ready_qc_durable == $right.ready_qc_durable
    }};
}

macro_rules! in_flight_first_release_session_equal_body {
    ($left:expr, $right:expr) => {{
        $left.bodies == $right.bodies
            && $left.ready_authorized == $right.ready_authorized
            && $left.crashed == $right.crashed
            && $left.producer_alive == $right.producer_alive
    }};
}

macro_rules! in_flight_first_release_history_equal_body {
    ($left:expr, $right:expr) => {{
        $left.ever_queue_plan_v4 == $right.ever_queue_plan_v4
            && $left.ever_reservation_v5 == $right.ever_reservation_v5
            && $left.ever_execution_input_durable == $right.ever_execution_input_durable
            && $left.ever_ready_authorized == $right.ever_ready_authorized
            && $left.ready_signed == $right.ready_signed
            && $left.ever_ready_qc_durable == $right.ever_ready_qc_durable
            && $left.reservation_committed_prefix == $right.reservation_committed_prefix
            && $left.queue_plan_tombstoned_prefix == $right.queue_plan_tombstoned_prefix
            && $left.reservation_commit_forgotten_prefix
                == $right.reservation_commit_forgotten_prefix
            && $left.pending_high_water == $right.pending_high_water
            && $left.released_high_water == $right.released_high_water
    }};
}

macro_rules! in_flight_first_release_commit_prefixes_equal_body {
    ($left:expr, $right:expr) => {{
        $left.reservation_committed_prefix == $right.reservation_committed_prefix
            && $left.queue_plan_tombstoned_prefix == $right.queue_plan_tombstoned_prefix
            && $left.reservation_commit_forgotten_prefix
                == $right.reservation_commit_forgotten_prefix
    }};
}

macro_rules! in_flight_first_release_decision_equal_body {
    ($left:expr, $right:expr) => {{
        canonical_identity_equal_body!($left.lane_commit_scope, $right.lane_commit_scope)
            && canonical_identity_equal_body!($left.release_scope, $right.release_scope)
            && $left.lane_commit_owner == $right.lane_commit_owner
            && $left.release_owner == $right.release_owner
            && $left.wsv_committed == $right.wsv_committed
            && $left.application_count == $right.application_count
            && $left.applied_by == $right.applied_by
    }};
}

macro_rules! in_flight_first_release_release_equal_body {
    ($left:expr, $right:expr) => {{
        $left.kura_retired == $right.kura_retired
            && $left.pending_prefix == $right.pending_prefix
            && $left.released_prefix == $right.released_prefix
            && $left.fifo_restored == $right.fifo_restored
    }};
}

macro_rules! in_flight_first_release_static_equal_body {
    ($left:expr, $right:expr) => {{
        $left.validator_count == $right.validator_count
            && $left.producer == $right.producer
            && $left.producer_selected_owner == $right.producer_selected_owner
            && $left.replicated_carrier_owners == $right.replicated_carrier_owners
            && $left.payload_binding_a == $right.payload_binding_a
            && canonical_identity_equal_body!($left.binding_a, $right.binding_a)
    }};
}

macro_rules! in_flight_first_release_state_equal_body {
    ($left:expr, $right:expr) => {{
        in_flight_first_release_static_equal_body!($left, $right)
            && in_flight_first_release_queue_equal_body!($left.queue, $right.queue)
            && in_flight_first_release_carrier_equal_body!($left.carrier, $right.carrier)
            && in_flight_first_release_session_equal_body!($left.session, $right.session)
            && in_flight_first_release_history_equal_body!($left.history, $right.history)
            && in_flight_first_release_decision_equal_body!($left.decision, $right.decision)
            && in_flight_first_release_release_equal_body!($left.release, $right.release)
    }};
}

macro_rules! in_flight_first_release_reservation_state_is_valid_body {
    ($state:expr) => {{
        $state == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT)
            || $state == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
            || $state == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED)
            || $state == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN)
            || $state == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED)
            || $state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED)
            || $state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN)
            || $state == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED)
    }};
}

macro_rules! production_in_flight_first_release_state_body {
    ($state:expr) => {{
        let state = $state;
        let queue = state.queue;
        let carrier = state.carrier;
        let session = state.session;
        let history = state.history;
        let decision = state.decision;
        let release = state.release;
        let validator_mask = in_flight_first_release_validator_mask_body!(state.validator_count);
        let ready_quorum = in_flight_first_release_ready_quorum_body!(state.validator_count);
        state.validator_count >= 1u8
            && state.validator_count <= 128u8
            && in_flight_first_release_single_validator_body!(state.producer, validator_mask)
            && state.producer_selected_owner == state.producer
            && state.replicated_carrier_owners == (validator_mask & !state.producer)
            && (state.payload_binding_a & !validator_mask) == 0u128
            && (state.payload_binding_a & state.producer) == state.producer
            && canonical_identity_is_typed_body!(
                state.binding_a,
                refinement_tag_value!(IDENTITY_DOMAIN_PAYLOAD),
                refinement_tag_value!(IDENTITY_KIND_CANONICAL_PAYLOAD)
            )
            && (queue.plan_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT)
                || queue.plan_state
                    == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                || queue.plan_state
                    == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED))
            && in_flight_first_release_reservation_state_is_valid_body!(queue.reservation_state)
            && (queue.reservation_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT)
                || queue.plan_state
                    != refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT))
            && if queue.plan_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT)
            {
                queue.selected_count == 0u64
            } else {
                queue.selected_count > 0u64 && queue.selected_count <= 4096u64
            }
            && (carrier.kura_active & !validator_mask) == 0u128
            && (carrier.execution_input_durable & !validator_mask) == 0u128
            && (session.bodies & !validator_mask) == 0u128
            && (session.ready_authorized & !validator_mask) == 0u128
            && (session.crashed & !validator_mask) == 0u128
            && (history.ever_execution_input_durable & !validator_mask) == 0u128
            && (history.ever_ready_authorized & !validator_mask) == 0u128
            && (history.ready_signed & !validator_mask) == 0u128
            && (carrier.execution_input_durable & !carrier.kura_active) == 0u128
            && (carrier.kura_active == 0u128 || history.ever_reservation_v5)
            && (session.ready_authorized & !carrier.execution_input_durable) == 0u128
            && (history.ready_signed & !history.ever_ready_authorized) == 0u128
            && (!carrier.ready_qc_durable
                || in_flight_first_release_bitmap_count_body!(history.ready_signed) >= ready_quorum)
            && (!history.ever_queue_plan_v4
                || queue.plan_state
                    != refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT))
            && (!history.ever_reservation_v5
                || queue.reservation_state
                    != refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT))
            && (history.ever_execution_input_durable & !carrier.execution_input_durable) == 0u128
            && (!history.ever_ready_qc_durable || carrier.ready_qc_durable)
            && history.reservation_committed_prefix <= queue.selected_count
            && history.queue_plan_tombstoned_prefix <= history.reservation_committed_prefix
            && history.reservation_commit_forgotten_prefix <= history.queue_plan_tombstoned_prefix
            && history.pending_high_water <= release.pending_prefix
            && history.released_high_water <= release.released_prefix
            && (session.bodies & session.crashed) == 0u128
            && (session.ready_authorized & session.crashed) == 0u128
            && (!session.producer_alive || (session.crashed & state.producer) == 0u128)
            && if decision.lane_commit_owner == 0u128 {
                canonical_identity_is_zero_body!(decision.lane_commit_scope)
            } else {
                in_flight_first_release_single_validator_body!(
                    decision.lane_commit_owner,
                    validator_mask
                ) && canonical_identity_equal_body!(decision.lane_commit_scope, state.binding_a)
            }
            && if decision.release_owner == 0u128 {
                canonical_identity_is_zero_body!(decision.release_scope)
            } else {
                in_flight_first_release_single_validator_body!(
                    decision.release_owner,
                    validator_mask
                ) && canonical_identity_equal_body!(decision.release_scope, state.binding_a)
            }
            && (decision.lane_commit_owner == 0u128 || decision.release_owner == 0u128)
            && decision.application_count <= 1u8
            && decision.wsv_committed == (decision.application_count == 1u8)
            && if decision.application_count == 0u8 {
                decision.applied_by == 0u128
            } else {
                decision.lane_commit_owner != 0u128
                    && decision.applied_by == decision.lane_commit_owner
                    && (decision.applied_by & carrier.execution_input_durable) != 0u128
            }
            && (history.reservation_committed_prefix == 0u64
                || (decision.wsv_committed && decision.lane_commit_owner != 0u128))
            && if queue.plan_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT)
            {
                history.reservation_committed_prefix == 0u64
                    && history.queue_plan_tombstoned_prefix == 0u64
                    && history.reservation_commit_forgotten_prefix == 0u64
            } else if queue.plan_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED)
            {
                history.queue_plan_tombstoned_prefix == queue.selected_count
            } else {
                history.queue_plan_tombstoned_prefix < queue.selected_count
            }
            && if queue.reservation_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED)
            {
                history.reservation_committed_prefix == queue.selected_count
                    && history.reservation_commit_forgotten_prefix < queue.selected_count
            } else if queue.reservation_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN)
            {
                history.reservation_committed_prefix == queue.selected_count
                    && history.queue_plan_tombstoned_prefix == queue.selected_count
                    && history.reservation_commit_forgotten_prefix == queue.selected_count
            } else if queue.reservation_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
            {
                history.reservation_committed_prefix < queue.selected_count
                    && history.queue_plan_tombstoned_prefix == 0u64
                    && history.reservation_commit_forgotten_prefix == 0u64
            } else {
                history.reservation_committed_prefix == 0u64
                    && history.queue_plan_tombstoned_prefix == 0u64
                    && history.reservation_commit_forgotten_prefix == 0u64
            }
            && (queue.reservation_state
                != refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN)
                || queue.plan_state
                    == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED))
            && release.pending_prefix <= queue.selected_count
            && release.released_prefix <= release.pending_prefix
            && (!release.kura_retired || decision.release_owner != 0u128)
            && (release.pending_prefix == 0u64
                || (release.kura_retired && decision.release_owner != 0u128))
            && if queue.reservation_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED)
                || queue.reservation_state
                    == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED)
                || queue.reservation_state
                    == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN)
            {
                release.kura_retired && release.pending_prefix == queue.selected_count
            } else {
                true
            }
            && (release.released_prefix == 0u64
                || ((queue.reservation_state
                    == refinement_tag_value!(
                        IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED
                    )
                    || queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED
                        )
                    || queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN
                        ))
                    && release.pending_prefix == queue.selected_count))
            && if queue.reservation_state
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED)
                || queue.reservation_state
                    == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN)
            {
                release.released_prefix == queue.selected_count
            } else {
                true
            }
            && if release.fifo_restored {
                queue.reservation_state
                    == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED)
                    || queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN
                        )
                    || queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED
                        )
            } else {
                queue.reservation_state
                    != refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN)
                    && queue.reservation_state
                        != refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED
                        )
            }
    }};
}

macro_rules! production_in_flight_first_release_transition_body {
    ($projection:expr) => {{
        let projection = $projection;
        let before = projection.before;
        let after = projection.after;
        let validator_mask = in_flight_first_release_validator_mask_body!(before.validator_count);
        let ready_quorum = in_flight_first_release_ready_quorum_body!(before.validator_count);
        production_in_flight_first_release_state_body!(before)
            && production_in_flight_first_release_state_body!(after)
            && in_flight_first_release_static_equal_body!(before, after)
            && if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT)
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT)
                    && after.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                    && after.queue.selected_count > 0u64
                    && after.queue.selected_count <= 4096u64
                    && after.queue.reservation_state == before.queue.reservation_state
                    && !before.history.ever_queue_plan_v4
                    && after.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && in_flight_first_release_commit_prefixes_equal_body!(
                        before.history,
                        after.history
                    )
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT)
                    && after.queue.plan_state == before.queue.plan_state
                    && after.queue.selected_count == before.queue.selected_count
                    && after.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && in_flight_first_release_commit_prefixes_equal_body!(
                        before.history,
                        after.history
                    )
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.session.crashed & projection.actor) == 0u128
                    && (before.session.bodies & projection.actor) != 0u128
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
                    && after.carrier.kura_active == (before.carrier.kura_active | projection.actor)
                    && after.carrier.execution_input_durable
                        == before.carrier.execution_input_durable
                    && after.carrier.ready_qc_durable == before.carrier.ready_qc_durable
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && projection.actor != before.producer
                    && before.session.producer_alive
                    && (before.session.bodies & before.producer) != 0u128
                    && before.history.ever_reservation_v5
                    && (before.session.crashed & projection.actor) == 0u128
                    && after.session.bodies == (before.session.bodies | projection.actor)
                    && after.session.ready_authorized == before.session.ready_authorized
                    && after.session.crashed == before.session.crashed
                    && after.session.producer_alive == before.session.producer_alive
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && in_flight_first_release_single_validator_body!(
                        projection.target,
                        validator_mask
                    )
                    && projection.actor != projection.target
                    && (before.session.bodies & projection.actor) != 0u128
                    && (before.session.crashed & projection.target) == 0u128
                    && after.session.bodies == (before.session.bodies | projection.target)
                    && after.session.ready_authorized == before.session.ready_authorized
                    && after.session.crashed == before.session.crashed
                    && after.session.producer_alive == before.session.producer_alive
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.carrier.kura_active & projection.actor) != 0u128
                    && (before.session.bodies & projection.actor) != 0u128
                    && (before.session.crashed & projection.actor) == 0u128
                    && after.carrier.kura_active == before.carrier.kura_active
                    && after.carrier.execution_input_durable
                        == (before.carrier.execution_input_durable | projection.actor)
                    && after.carrier.ready_qc_durable == before.carrier.ready_qc_durable
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == (before.history.ever_execution_input_durable | projection.actor)
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && in_flight_first_release_commit_prefixes_equal_body!(
                        before.history,
                        after.history
                    )
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.session.crashed & projection.actor) == 0u128
                    && (before.carrier.execution_input_durable & projection.actor) != 0u128
                    && after.session.bodies == before.session.bodies
                    && after.session.ready_authorized
                        == (before.session.ready_authorized | projection.actor)
                    && after.session.crashed == before.session.crashed
                    && after.session.producer_alive == before.session.producer_alive
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized
                        == (before.history.ever_ready_authorized | projection.actor)
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && in_flight_first_release_commit_prefixes_equal_body!(
                        before.history,
                        after.history
                    )
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.session.crashed & projection.actor) == 0u128
                    && (before.session.ready_authorized & projection.actor) != 0u128
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed
                        == (before.history.ready_signed | projection.actor)
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && in_flight_first_release_commit_prefixes_equal_body!(
                        before.history,
                        after.history
                    )
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && !before.carrier.ready_qc_durable
                    && in_flight_first_release_bitmap_count_body!(before.history.ready_signed)
                        >= ready_quorum
                    && after.carrier.kura_active == before.carrier.kura_active
                    && after.carrier.execution_input_durable
                        == before.carrier.execution_input_durable
                    && after.carrier.ready_qc_durable
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable
                    && in_flight_first_release_commit_prefixes_equal_body!(
                        before.history,
                        after.history
                    )
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.session.crashed & projection.actor) == 0u128
                    && after.session.crashed == (before.session.crashed | projection.actor)
                    && after.session.bodies == (before.session.bodies & !projection.actor)
                    && after.session.ready_authorized
                        == (before.session.ready_authorized & !projection.actor)
                    && after.session.producer_alive
                        == (before.session.producer_alive && projection.actor != before.producer)
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.session.crashed & projection.actor) != 0u128
                    && after.session.crashed == (before.session.crashed & !projection.actor)
                    && after.session.bodies == before.session.bodies
                    && after.session.ready_authorized == before.session.ready_authorized
                    && after.session.producer_alive == before.session.producer_alive
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.history.ready_signed & projection.actor) != 0u128
                    && before.carrier.ready_qc_durable
                    && before.decision.lane_commit_owner == 0u128
                    && before.decision.release_owner == 0u128
                    && (before.payload_binding_a & projection.actor) != 0u128
                    && after.decision.lane_commit_owner == projection.actor
                    && canonical_identity_equal_body!(
                        after.decision.lane_commit_scope,
                        before.binding_a
                    )
                    && after.decision.release_owner == before.decision.release_owner
                    && canonical_identity_equal_body!(
                        after.decision.release_scope,
                        before.decision.release_scope
                    )
                    && after.decision.wsv_committed == before.decision.wsv_committed
                    && after.decision.application_count == before.decision.application_count
                    && after.decision.applied_by == before.decision.applied_by
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && projection.actor == before.decision.lane_commit_owner
                    && before.decision.application_count == 0u8
                    && !before.decision.wsv_committed
                    && canonical_identity_equal_body!(
                        after.decision.lane_commit_scope,
                        before.decision.lane_commit_scope
                    )
                    && canonical_identity_equal_body!(
                        after.decision.release_scope,
                        before.decision.release_scope
                    )
                    && after.decision.lane_commit_owner == before.decision.lane_commit_owner
                    && after.decision.release_owner == before.decision.release_owner
                    && after.decision.wsv_committed
                    && after.decision.application_count == 1u8
                    && after.decision.applied_by == projection.actor
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(
                    IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED
                )
            {
                projection.actor == 0u128
                    && before.history.reservation_committed_prefix < before.queue.selected_count
                    && projection.target
                        == (before.history.reservation_committed_prefix + 1u64) as u128
                    && before.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
                    && before.decision.lane_commit_owner != 0u128
                    && before.decision.wsv_committed
                    && after.queue.plan_state == before.queue.plan_state
                    && after.queue.selected_count == before.queue.selected_count
                    && after.queue.reservation_state
                        == if before.history.reservation_committed_prefix + 1u64
                            == before.queue.selected_count
                        {
                            refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED)
                        } else {
                            refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
                        }
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && after.history.reservation_committed_prefix
                        == before.history.reservation_committed_prefix + 1u64
                    && after.history.queue_plan_tombstoned_prefix
                        == before.history.queue_plan_tombstoned_prefix
                    && after.history.reservation_commit_forgotten_prefix
                        == before.history.reservation_commit_forgotten_prefix
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_PLAN_TOMBSTONE)
            {
                projection.actor == 0u128
                    && before.history.reservation_committed_prefix == before.queue.selected_count
                    && before.history.queue_plan_tombstoned_prefix < before.queue.selected_count
                    && projection.target
                        == (before.history.queue_plan_tombstoned_prefix + 1u64) as u128
                    && before.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED)
                    && after.queue.plan_state
                        == if before.history.queue_plan_tombstoned_prefix + 1u64
                            == before.queue.selected_count
                        {
                            refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED)
                        } else {
                            refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                        }
                    && after.queue.selected_count == before.queue.selected_count
                    && after.queue.reservation_state == before.queue.reservation_state
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && after.history.reservation_committed_prefix
                        == before.history.reservation_committed_prefix
                    && after.history.queue_plan_tombstoned_prefix
                        == before.history.queue_plan_tombstoned_prefix + 1u64
                    && after.history.reservation_commit_forgotten_prefix
                        == before.history.reservation_commit_forgotten_prefix
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_COMMIT)
            {
                projection.actor == 0u128
                    && before.history.queue_plan_tombstoned_prefix == before.queue.selected_count
                    && before.history.reservation_commit_forgotten_prefix
                        < before.queue.selected_count
                    && projection.target
                        == (before.history.reservation_commit_forgotten_prefix + 1u64) as u128
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED)
                    && before.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED)
                    && after.queue.plan_state == before.queue.plan_state
                    && after.queue.selected_count == before.queue.selected_count
                    && after.queue.reservation_state
                        == if before.history.reservation_commit_forgotten_prefix + 1u64
                            == before.queue.selected_count
                        {
                            refinement_tag_value!(
                                IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN
                            )
                        } else {
                            refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED)
                        }
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && after.history.reservation_committed_prefix
                        == before.history.reservation_committed_prefix
                    && after.history.queue_plan_tombstoned_prefix
                        == before.history.queue_plan_tombstoned_prefix
                    && after.history.reservation_commit_forgotten_prefix
                        == before.history.reservation_commit_forgotten_prefix + 1u64
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT)
            {
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.carrier.kura_active & projection.actor) != 0u128
                    && before.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
                    && before.decision.lane_commit_owner == 0u128
                    && before.decision.release_owner == 0u128
                    && (before.payload_binding_a & projection.actor) != 0u128
                    && canonical_identity_equal_body!(
                        after.decision.lane_commit_scope,
                        before.decision.lane_commit_scope
                    )
                    && after.decision.lane_commit_owner == before.decision.lane_commit_owner
                    && after.decision.release_owner == projection.actor
                    && canonical_identity_equal_body!(
                        after.decision.release_scope,
                        before.binding_a
                    )
                    && after.decision.wsv_committed == before.decision.wsv_committed
                    && after.decision.application_count == before.decision.application_count
                    && after.decision.applied_by == before.decision.applied_by
                    && after.release.kura_retired
                    && after.release.pending_prefix == before.release.pending_prefix
                    && after.release.released_prefix == before.release.released_prefix
                    && after.release.fifo_restored == before.release.fifo_restored
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.release.pending_prefix < before.queue.selected_count
                    && before.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                    && before.release.kura_retired
                    && after.release.kura_retired == before.release.kura_retired
                    && after.release.pending_prefix == before.release.pending_prefix + 1u64
                    && after.release.released_prefix == before.release.released_prefix
                    && after.release.fifo_restored == before.release.fifo_restored
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && in_flight_first_release_commit_prefixes_equal_body!(
                        before.history,
                        after.history
                    )
                    && after.history.pending_high_water == before.history.pending_high_water + 1u64
                    && after.history.released_high_water == before.history.released_high_water
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.decision.release_owner != 0u128
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
                    && before.release.pending_prefix == before.queue.selected_count
                    && after.queue.plan_state == before.queue.plan_state
                    && after.queue.selected_count == before.queue.selected_count
                    && after.queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED
                        )
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.decision.release_owner != 0u128
                    && before.release.released_prefix < before.queue.selected_count
                    && before.queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED
                        )
                    && before.release.pending_prefix == before.queue.selected_count
                    && after.release.kura_retired == before.release.kura_retired
                    && after.release.pending_prefix == before.release.pending_prefix
                    && after.release.released_prefix == before.release.released_prefix + 1u64
                    && after.release.fifo_restored == before.release.fifo_restored
                    && after.history.ever_queue_plan_v4 == before.history.ever_queue_plan_v4
                    && after.history.ever_reservation_v5 == before.history.ever_reservation_v5
                    && after.history.ever_execution_input_durable
                        == before.history.ever_execution_input_durable
                    && after.history.ever_ready_authorized == before.history.ever_ready_authorized
                    && after.history.ready_signed == before.history.ready_signed
                    && after.history.ever_ready_qc_durable == before.history.ever_ready_qc_durable
                    && in_flight_first_release_commit_prefixes_equal_body!(
                        before.history,
                        after.history
                    )
                    && after.history.pending_high_water == before.history.pending_high_water
                    && after.history.released_high_water
                        == before.history.released_high_water + 1u64
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
            } else if projection.action
                == refinement_tag_value!(
                    IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE
                )
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.decision.release_owner != 0u128
                    && before.queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED
                        )
                    && before.release.released_prefix == before.queue.selected_count
                    && after.queue.plan_state == before.queue.plan_state
                    && after.queue.selected_count == before.queue.selected_count
                    && after.queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED
                        )
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED
                        )
                    && !before.release.fifo_restored
                    && after.release.kura_retired == before.release.kura_retired
                    && after.release.pending_prefix == before.release.pending_prefix
                    && after.release.released_prefix == before.release.released_prefix
                    && after.release.fifo_restored
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED
                        )
                    && before.release.fifo_restored
                    && after.queue.plan_state == before.queue.plan_state
                    && after.queue.selected_count == before.queue.selected_count
                    && after.queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN
                        )
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.decision.wsv_committed
                    && in_flight_first_release_state_equal_body!(before, after)
            } else if projection.action
                == refinement_tag_value!(
                    IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT
                )
            {
                // A V6 snapshot envelope reconstructs process-local indexes from bytes
                // already represented by the durable abstract owner. It is an
                // exact stutter, never a new reservation acquisition.
                projection.actor == 0u128
                    && projection.target == 0u128
                    && in_flight_first_release_state_equal_body!(before, after)
            } else if projection.action
                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT)
            {
                projection.actor == 0u128
                    && projection.target == 0u128
                    && before.queue.plan_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED)
                    && before.queue.reservation_state
                        == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE)
                    && before.decision.lane_commit_owner == 0u128
                    && before.decision.release_owner == 0u128
                    && after.queue.plan_state == before.queue.plan_state
                    && after.queue.selected_count == before.queue.selected_count
                    && after.queue.reservation_state
                        == refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED
                        )
                    && after.release.kura_retired == before.release.kura_retired
                    && after.release.pending_prefix == before.release.pending_prefix
                    && after.release.released_prefix == before.release.released_prefix
                    && after.release.fifo_restored
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_session_equal_body!(before.session, after.session)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
            } else if projection.action
                == refinement_tag_value!(
                    IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY
                )
            {
                // Startup rehydrates only process-local body custody from the
                // actor's exact durable Kura payload. It does not confer READY
                // authorization or alter any durable/economic fact.
                in_flight_first_release_single_validator_body!(projection.actor, validator_mask)
                    && projection.target == 0u128
                    && (before.session.crashed & projection.actor) == 0u128
                    && (before.carrier.kura_active & projection.actor) != 0u128
                    && (before.session.bodies & projection.actor) == 0u128
                    && !before.release.kura_retired
                    && !before.decision.wsv_committed
                    && before.decision.release_owner == 0u128
                    && before.queue.reservation_state
                        != refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN
                        )
                    && before.queue.reservation_state
                        != refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN
                        )
                    && before.queue.reservation_state
                        != refinement_tag_value!(
                            IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED
                        )
                    && after.session.bodies == (before.session.bodies | projection.actor)
                    && after.session.ready_authorized == before.session.ready_authorized
                    && after.session.crashed == before.session.crashed
                    && after.session.producer_alive
                        == if projection.actor == before.producer {
                            true
                        } else {
                            before.session.producer_alive
                        }
                    && in_flight_first_release_queue_equal_body!(before.queue, after.queue)
                    && in_flight_first_release_carrier_equal_body!(before.carrier, after.carrier)
                    && in_flight_first_release_history_equal_body!(before.history, after.history)
                    && in_flight_first_release_decision_equal_body!(before.decision, after.decision)
                    && in_flight_first_release_release_equal_body!(before.release, after.release)
            } else {
                false
            }
    }};
}

// Keep the durability acknowledgement predicate shared between the ordinary
// Rust WAL lifecycle and its Verus proof.  An append receipt is minted only
// after all three adapter completions have happened in this order; the
// expression below states the safety half of that contract without treating a
// successful write or userspace flush as durable storage.
macro_rules! wal_append_acknowledged_body {
    ($write_complete:expr, $flush_complete:expr, $sync_complete:expr $(,)?) => {{ $write_complete && $flush_complete && $sync_complete }};
}

// Canonical header acceptance is also a shared production/Verus expression.
// The byte parser still returns field-specific errors, then crosses this final
// fail-closed gate before exposing any recovered frame.
macro_rules! wal_header_accepted_body {
    (
        $complete:expr,
        $magic_matches:expr,
        $format_matches:expr,
        $actual_protocol:expr,
        $expected_protocol:expr,
        $actual_chain:expr,
        $expected_chain:expr,
        $actual_key:expr,
        $expected_key:expr,
        $checksum_matches:expr $(,)?
    ) => {{
        $complete
            && $magic_matches
            && $format_matches
            && $actual_protocol == $expected_protocol
            && $actual_chain == $expected_chain
            && $actual_key == $expected_key
            && $checksum_matches
    }};
}

// Complete-frame acceptance is intentionally expressed only over parsed
// primitives.  Production and Verus therefore agree on the exact sequence,
// size, hash-link, and checksum corridor even though BLAKE3 itself stays in the
// cryptographic TCB.
macro_rules! wal_complete_frame_valid_body {
    (
        $before_failed_closed:expr,
        $complete:expr,
        $expected_sequence:expr,
        $maximum_sequence:expr,
        $actual_sequence:expr,
        $payload_len:expr,
        $maximum_payload_len:expr,
        $encoded_previous:expr,
        $expected_previous:expr,
        $encoded_hash:expr,
        $calculated_hash:expr $(,)?
    ) => {{
        !$before_failed_closed
            && $complete
            && $expected_sequence < $maximum_sequence
            && $actual_sequence == $expected_sequence
            && $payload_len <= $maximum_payload_len
            && $encoded_previous == $expected_previous
            && $encoded_hash == $calculated_hash
    }};
}

// This is the fixed-width retirement rule used by the executable lifecycle
// projection and Verus.  Presence bits are derived from typed production
// state: a closed FinalizedHeight and its trusted Kura durability receipt.
// Identity equality is deliberately expanded field by field so neither side
// can authorize pruning with a merely same-height or same-subject artifact.
macro_rules! wal_retirement_authorized_body {
    (
        $height_closed:expr,
        $block_durable:expr,
        $certificate_durable:expr,
        $decision_context:expr,
        $decision_height:expr,
        $decision_subject:expr,
        $decision_certificate_context:expr,
        $decision_certificate_height:expr,
        $decision_certificate_view:expr,
        $decision_proposal_height:expr,
        $decision_proposal_view:expr,
        $decision_certificate_phase:expr,
        $commit_phase:expr,
        $decision_certificate_subject:expr,
        $receipt_context:expr,
        $receipt_height:expr,
        $receipt_subject:expr,
        $receipt_certificate_context:expr,
        $receipt_certificate_height:expr,
        $receipt_certificate_view:expr,
        $receipt_proposal_height:expr,
        $receipt_proposal_view:expr,
        $receipt_certificate_phase:expr,
        $receipt_certificate_subject:expr $(,)?
    ) => {{
        $height_closed
            && $block_durable
            && $certificate_durable
            && $decision_context == $receipt_context
            && $decision_height == $receipt_height
            && $decision_subject == $receipt_subject
            && $decision_certificate_context == $receipt_certificate_context
            && $decision_certificate_height == $receipt_certificate_height
            && $decision_certificate_view == $receipt_certificate_view
            && $decision_proposal_height == $receipt_proposal_height
            && $decision_proposal_view == $receipt_proposal_view
            && $decision_certificate_phase == $commit_phase
            && $decision_certificate_phase == $receipt_certificate_phase
            && $decision_certificate_subject == $receipt_certificate_subject
            && $decision_context == $decision_certificate_context
            && $decision_height == $decision_certificate_height
            && $decision_height == $decision_proposal_height
            && $decision_proposal_view == $decision_certificate_view
            && $decision_subject == $decision_certificate_subject
    }};
}

/// Primitive `(height, view, generation)` lifecycle projection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TagProjection {
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) generation: u64,
}

/// Typed identity of the sole reducer consumer for one exact body pipeline.
///
/// `K` is the complete `(round, subject)` key and `M` is the canonical
/// manifest identity. A missing manifest is permitted only while a certified
/// fetch has not yet acquired the body metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactBodyOwnerProjection<K, M> {
    pub(crate) tag: TagProjection,
    pub(crate) key: K,
    pub(crate) manifest_hash: Option<M>,
}

/// Preflighted exact owner binding derived from typed identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Constructed by the production executor and refinement tests.
pub struct ExactBodyOwnerBindingProjection<K, M> {
    pub(crate) owner: ExactBodyOwnerProjection<K, M>,
    pub(crate) already_owned: bool,
}

/// Bind or monotonically enrich one exact body-pipeline owner.
///
/// The only permitted enrichment changes an absent manifest identity to a
/// present one. The reducer tag, round/subject key, and any existing manifest
/// identity are immutable.
#[allow(dead_code)] // Called by the production executor, outside the pure harness crate.
pub fn plan_exact_body_owner_binding<K, M>(
    current: Option<ExactBodyOwnerProjection<K, M>>,
    incoming: ExactBodyOwnerProjection<K, M>,
) -> Option<ExactBodyOwnerBindingProjection<K, M>>
where
    K: Copy + Eq,
    M: Copy + Eq,
{
    exact_body_owner_binding_body!(
        current,
        incoming,
        ExactBodyOwnerProjection,
        ExactBodyOwnerBindingProjection
    )
}

/// Return whether a pipeline stage carries the exact immutable owner identity.
#[must_use]
#[allow(dead_code)] // Called by the production executor, outside the pure harness crate.
pub fn exact_body_stage_is_owned<K, M>(
    owner: ExactBodyOwnerProjection<K, M>,
    stage: ExactBodyOwnerProjection<K, M>,
) -> bool
where
    K: Copy + Eq,
    M: Copy + Eq,
{
    exact_body_owner_equal_body!(owner, stage)
}

/// Rebind one exact body-pipeline consumer to a strictly newer incarnation.
///
/// The height, round/subject key, and manifest identity remain immutable;
/// `(view, generation)` must advance lexicographically: a later view may reset
/// generation to zero, while a same-view lock upgrade must strictly increment
/// it. This is only a safety transition. It does not claim the asynchronous
/// rebind will be scheduled.
#[allow(dead_code)] // Called by the production executor, outside the pure harness crate.
pub fn plan_exact_body_owner_rebind<K, M>(
    current: ExactBodyOwnerProjection<K, M>,
    previous: ExactBodyOwnerProjection<K, M>,
    rebound_tag: TagProjection,
) -> Option<ExactBodyOwnerProjection<K, M>>
where
    K: Copy + Eq,
    M: Copy + Eq,
{
    exact_body_owner_rebind_body!(current, previous, rebound_tag, ExactBodyOwnerProjection)
}

/// Classification of one logical completion stage across serialized owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Consumed by the production runtime and refinement tests.
pub enum ExactBodyCompletionOwnership {
    /// Neither runtime ingress nor the Busy-deferred lane owns the stage.
    Vacant,
    /// Exactly one lane owns exactly matching trusted evidence.
    Exact,
    /// Evidence conflicts, is duplicated, or its owner count is inconsistent.
    Invalid,
}

/// Classify exact completion ownership across runtime and deferred lanes.
#[must_use]
#[allow(dead_code)] // Called by the production runtime, outside the pure harness crate.
pub fn classify_exact_body_completion_ownership(
    ingress_owners: usize,
    ingress_exact: usize,
    deferred_owners: usize,
    deferred_exact: usize,
) -> ExactBodyCompletionOwnership {
    exact_body_completion_ownership_body!(
        ingress_owners,
        ingress_exact,
        deferred_owners,
        deferred_exact,
        ExactBodyCompletionOwnership::Vacant,
        ExactBodyCompletionOwnership::Exact,
        ExactBodyCompletionOwnership::Invalid,
    )
}

/// Exact residual counters after superseding body-pipeline ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Constructed by the production executor and refinement tests.
pub struct ExactBodyRetirementAccounting {
    pub(crate) ready_after: u64,
    pub(crate) store_after: u64,
}

/// Compute exact body-byte residuals, rejecting any underflow or leakage.
#[must_use]
#[allow(dead_code)] // Called by the production executor, outside the pure harness crate.
pub fn plan_exact_body_retirement_accounting(
    ready_before: u64,
    retained_bytes: u64,
    ready_bytes: u64,
    store_before: u64,
    store_bytes: u64,
) -> Option<ExactBodyRetirementAccounting> {
    exact_body_retirement_accounting_body!(
        ready_before,
        retained_bytes,
        ready_bytes,
        store_before,
        store_bytes,
        ExactBodyRetirementAccounting,
    )
}

/// One exact selection made by the bounded three-class ingress kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Constructed by the production runtime and refinement tests.
pub struct BoundedServiceSelection {
    pub(crate) selected: u8,
    pub(crate) next: u8,
}

/// Select one ready runtime class from the persistent round-robin cursor.
///
/// Invalid cursors fail closed as `(NONE, NONE)`. This function proves only the
/// bounded arbitration decision made when production invokes it; repeated
/// invocation remains an explicit scheduler/host-service premise.
#[must_use]
#[allow(dead_code)] // Called by the production runtime, outside the pure harness crate.
pub fn select_bounded_service_class(
    cursor: u8,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
) -> BoundedServiceSelection {
    bounded_service_selection_body!(
        cursor,
        completion_ready,
        progress_ready,
        normal_ready,
        BoundedServiceSelection,
    )
}

/// Primitive production trace spanning the effective-lock acquisition seams.
///
/// Each action uses one discriminant and canonical zeroes for unrelated
/// fields. The trace contains only facts derived from the live reducer,
/// executor, or runtime state at its enforcement point; no authorization bit
/// is accepted from an external caller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectiveLockTraceProjection {
    pub(crate) kind: u8,
    pub(crate) relation_exact: bool,
    pub(crate) protected_before: u64,
    pub(crate) protected_after: u64,
    pub(crate) owner_before: u64,
    pub(crate) owner_after: u64,
    pub(crate) owner_reused: bool,
    pub(crate) ready_before: u64,
    pub(crate) retired_retained: u64,
    pub(crate) retired_ready: u64,
    pub(crate) ready_after: u64,
    pub(crate) store_before: u64,
    pub(crate) retired_store: u64,
    pub(crate) store_after: u64,
    pub(crate) cursor_before: u8,
    pub(crate) completion_ready: bool,
    pub(crate) progress_ready: bool,
    pub(crate) normal_ready: bool,
    pub(crate) selected: u8,
    pub(crate) cursor_after: u8,
}

/// Complete immutable identity of one durable predecessor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDurablePredecessorIdentityProjection {
    pub(crate) height: u64,
    pub(crate) block_hash: CanonicalIdentityProjection,
    pub(crate) artifact_hash: CanonicalIdentityProjection,
}

/// Expected and returned ownership at the fallible successor-construction seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionSuccessorPredecessorBindingProjection {
    pub(crate) expected_predecessor: ProductionDurablePredecessorIdentityProjection,
    pub(crate) authority_predecessor: ProductionDurablePredecessorIdentityProjection,
    pub(crate) successor_context_id: CanonicalIdentityProjection,
}

/// Primitive fields of the prepared successor status and activation marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionSuccessorSnapshotProjection {
    pub(crate) expected_context_id: CanonicalIdentityProjection,
    pub(crate) published_context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) last_committed_height: u64,
    pub(crate) view: u64,
    pub(crate) generation: u64,
    pub(crate) marker_context_id: CanonicalIdentityProjection,
    pub(crate) marker_height: u64,
    pub(crate) marker_view: u64,
    pub(crate) marker_generation: u64,
    pub(crate) marker_kind: u8,
    pub(crate) marker_age_ms: u64,
}

/// Applied-predecessor successor activation observed at the publication gate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionAppliedSuccessorTraceProjection {
    pub(crate) authority_kind: u8,
    pub(crate) binding: ProductionSuccessorPredecessorBindingProjection,
    pub(crate) predecessor_status_height: u64,
    pub(crate) predecessor_stage_before: u8,
    pub(crate) predecessor_stage_after: u8,
    pub(crate) successor: ProductionSuccessorSnapshotProjection,
}

/// Complete-tip or snapshot recovery activation at an empty registry boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionRecoveredSuccessorTraceProjection {
    pub(crate) authority_kind: u8,
    pub(crate) predecessor: ProductionDurablePredecessorIdentityProjection,
    pub(crate) snapshot_record_hash: CanonicalIdentityProjection,
    pub(crate) snapshot_height: u64,
    pub(crate) snapshot_block_hash: CanonicalIdentityProjection,
    pub(crate) authority_context_id: CanonicalIdentityProjection,
    pub(crate) published_status_height_before: u64,
    pub(crate) successor: ProductionSuccessorSnapshotProjection,
}

/// Primitive successor startup lifecycle transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionSuccessorStartupLifecycleProjection {
    pub(crate) transition_kind: u8,
    pub(crate) authority_kind: u8,
    pub(crate) status_height: u64,
    pub(crate) stage_before: u8,
    pub(crate) stage_after: u8,
    pub(crate) published_height_before: u64,
    pub(crate) published_height_after: u64,
    pub(crate) restart_required_before: bool,
    pub(crate) restart_required_after: bool,
}

/// Exact block-sync CommitQC handoff from authenticated discovery to reducer ingress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionHistoricalCertificateTraceProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) context_height: u64,
    pub(crate) certificate_context_id: CanonicalIdentityProjection,
    pub(crate) certificate_height: u64,
    pub(crate) request_hash: CanonicalIdentityProjection,
    pub(crate) response_request_hash: CanonicalIdentityProjection,
    pub(crate) response_certificate: CanonicalIdentityProjection,
    pub(crate) message_certificate: CanonicalIdentityProjection,
    pub(crate) message_hash: CanonicalIdentityProjection,
    pub(crate) admitted_message_hash: CanonicalIdentityProjection,
    pub(crate) request_present_before: bool,
    pub(crate) request_present_after: bool,
}

/// Exact certified historical body handoff into the ordinary body pipeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionHistoricalBodyPipelineTraceProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) context_height: u64,
    pub(crate) request_hash: CanonicalIdentityProjection,
    pub(crate) pending_request_hash: CanonicalIdentityProjection,
    pub(crate) authenticated_request_hash: CanonicalIdentityProjection,
    pub(crate) fetch_tag: TagProjection,
    pub(crate) round_context_id: CanonicalIdentityProjection,
    pub(crate) round_height: u64,
    pub(crate) round_view: u64,
    pub(crate) subject: CanonicalIdentityProjection,
    pub(crate) manifest_round_context_id: CanonicalIdentityProjection,
    pub(crate) manifest_round_height: u64,
    pub(crate) manifest_round_view: u64,
    pub(crate) manifest_subject: CanonicalIdentityProjection,
    pub(crate) response_manifest: CanonicalIdentityProjection,
    pub(crate) ready_manifest: CanonicalIdentityProjection,
    pub(crate) subject_payload_hash: CanonicalIdentityProjection,
    pub(crate) body_payload_hash: CanonicalIdentityProjection,
    pub(crate) owner_present_after: bool,
    pub(crate) owner_tag: TagProjection,
    pub(crate) owner_round_context_id: CanonicalIdentityProjection,
    pub(crate) owner_round_height: u64,
    pub(crate) owner_round_view: u64,
    pub(crate) owner_subject: CanonicalIdentityProjection,
    pub(crate) pending_fetch_present_after: bool,
    pub(crate) request_present_after: bool,
}

/// Primitive durable-intent ownership observed around one reducer step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDurableIntentTraceProjection {
    pub(crate) event_tag: TagProjection,
    pub(crate) owner_tag_before: TagProjection,
    pub(crate) owner_tag_after: TagProjection,
    pub(crate) event_kind: u8,
    pub(crate) event_persistence_id: u64,
    pub(crate) pending_before: PendingProjection,
    pub(crate) pending_after: PendingProjection,
    pub(crate) boundary_claimed: BoundaryCapabilityKey,
    pub(crate) boundary_granted: BoundaryCapabilityKey,
    pub(crate) effects: EffectTrace,
    pub(crate) durable_sequence_before: u64,
    pub(crate) durable_sequence_after: u64,
}

/// Primitive ownership facts for one validated, undecided durable lock.
///
/// The kernel accepts an active same-round Commit, an exact pending
/// `LockAndCommit`, an acknowledged timeout which is still closing the
/// current view, or the durable predecessor TC which authorizes exact locked
/// body re-proposal. A durable Commit from the old lock round may be
/// retransmitted, but timeout forms are recovery witnesses only: the reducer
/// never appends or signs a fresh Commit for a closed proposal round.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LockedCommitProgressWitnessProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) current_height: u64,
    pub(crate) current_view: u64,
    pub(crate) local_validator_present: bool,
    pub(crate) local_validator: ValidatorId,
    pub(crate) locked_context_id: CanonicalIdentityProjection,
    pub(crate) locked_height: u64,
    pub(crate) locked_view: u64,
    pub(crate) locked_subject: CanonicalIdentityProjection,
    pub(crate) commit_intent_present: bool,
    pub(crate) commit_context_id: CanonicalIdentityProjection,
    pub(crate) commit_height: u64,
    pub(crate) commit_view: u64,
    pub(crate) commit_proposal_height: u64,
    pub(crate) commit_proposal_view: u64,
    pub(crate) commit_phase: u8,
    pub(crate) commit_subject: CanonicalIdentityProjection,
    pub(crate) commit_signer: ValidatorId,
    pub(crate) commit_signature_pending: bool,
    pub(crate) commit_pooled: bool,
    pub(crate) pending: PendingProjection,
    pub(crate) timeout_intent_present: bool,
    pub(crate) timeout_intent_durable: bool,
    pub(crate) timeout_context_id: CanonicalIdentityProjection,
    pub(crate) timeout_height: u64,
    pub(crate) timeout_view: u64,
    pub(crate) timeout_signer: ValidatorId,
    pub(crate) installed_timeout_present: bool,
    pub(crate) installed_timeout_durable: bool,
    pub(crate) installed_timeout_context_id: CanonicalIdentityProjection,
    pub(crate) installed_timeout_height: u64,
    pub(crate) installed_timeout_view: u64,
}

/// Check that a validated lock retains one exact durable progress witness.
#[must_use]
pub(crate) fn locked_commit_progress_witness_is_valid(
    projection: LockedCommitProgressWitnessProjection,
) -> bool {
    locked_commit_progress_witness_body!(projection)
}

/// Complete semantic decision identity repeated by a certificate or receipt.
///
/// Every digest is a lossless four-word projection of an existing canonical
/// 256-bit identity. The projection never hashes or truncates inside the pure
/// kernel; production supplies identities derived at the typed protocol seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDecisionIdentityProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) proposal_height: u64,
    pub(crate) proposal_view: u64,
    pub(crate) phase: u8,
    pub(crate) subject: CanonicalIdentityProjection,
    pub(crate) block_hash: CanonicalIdentityProjection,
    pub(crate) payload_hash: CanonicalIdentityProjection,
    pub(crate) execution_commitment: CanonicalIdentityProjection,
    pub(crate) executed_block_wire_hash: CanonicalIdentityProjection,
}

/// Complete quorum-certificate identity at a production refinement seam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionQuorumCertificateIdentityProjection {
    pub(crate) decision: ProductionDecisionIdentityProjection,
    pub(crate) certificate: CanonicalIdentityProjection,
    pub(crate) signer_count: u64,
    pub(crate) aggregate_signature_len: u64,
}

/// Exact durable-body identity shared by recovery and application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDurableBodyIdentityProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) subject: CanonicalIdentityProjection,
    pub(crate) block_hash: CanonicalIdentityProjection,
    pub(crate) payload_hash: CanonicalIdentityProjection,
    pub(crate) manifest: CanonicalIdentityProjection,
    pub(crate) frame: CanonicalIdentityProjection,
}

/// Primitive pending-Decision recovery boundary selected during startup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionDecisionRecoveryTraceProjection {
    pub(crate) state_height: u64,
    pub(crate) expected_context_id: CanonicalIdentityProjection,
    pub(crate) expected_height: u64,
    pub(crate) expected_block_hash: CanonicalIdentityProjection,
    pub(crate) frozen_context_id: CanonicalIdentityProjection,
    pub(crate) frozen_height: u64,
    pub(crate) replay_tag: TagProjection,
    pub(crate) owner_tag: TagProjection,
    pub(crate) replay_generation: u64,
    pub(crate) commit_qc: ProductionQuorumCertificateIdentityProjection,
    pub(crate) manifest_round: TagProjection,
    pub(crate) manifest_subject: CanonicalIdentityProjection,
    pub(crate) manifest: CanonicalIdentityProjection,
    pub(crate) durable_body: ProductionDurableBodyIdentityProjection,
    pub(crate) validated_body: ProductionDurableBodyIdentityProjection,
    pub(crate) validated_execution_commitment: CanonicalIdentityProjection,
    pub(crate) stage: u8,
}

/// Primitive scheduler input and its concrete selected owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionSchedulerTraceProjection {
    pub(crate) fifo_owed_before: bool,
    pub(crate) timeout_due: bool,
    pub(crate) periodic_timer_due: bool,
    pub(crate) fifo_ready: bool,
    pub(crate) selected: u8,
    pub(crate) fifo_owed_after: bool,
}

/// Primitive identity, queue-class, ordinal, and dormant-owner observation for
/// one newly admitted physical owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionIngressIdentityAndClassTraceProjection {
    pub(crate) incoming_height: u64,
    pub(crate) incoming_view: u64,
    pub(crate) incoming_generation: u64,
    pub(crate) incoming_class: u8,
    pub(crate) stored_height: u64,
    pub(crate) stored_view: u64,
    pub(crate) stored_generation: u64,
    pub(crate) stored_class: u8,
    pub(crate) queue_len_before: u64,
    pub(crate) queue_len_after: u64,
    pub(crate) queue_capacity: u64,
    pub(crate) ordinal_source_before: u128,
    pub(crate) physical_admission_ordinal: u128,
    pub(crate) lifecycle_ordinal: u128,
    pub(crate) ordinal_source_after: u128,
    pub(crate) dormant_reservations_before: u64,
    pub(crate) dormant_reservations_after: u64,
    /// Zero for an ordinary admission; otherwise the exact dormant lifecycle
    /// owner consumed by this physical FIFO publication.
    pub(crate) dormant_owner_ordinal: u128,
    /// Whether this transition advances the shared ordinal source. Every
    /// generic admission must set this; reservation materialization uses its
    /// separate occupancy-preserving projection below.
    pub(crate) ordinal_minted: bool,
}

/// Primitive exact-token observation for replacing one unpublished ingress
/// reservation with its reducer-visible command. The shared ordinal source
/// and effective occupied-slot count both remain unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionIngressReservationMaterializationTraceProjection {
    pub(crate) incoming_height: u64,
    pub(crate) incoming_view: u64,
    pub(crate) incoming_generation: u64,
    pub(crate) incoming_class: u8,
    pub(crate) stored_height: u64,
    pub(crate) stored_view: u64,
    pub(crate) stored_generation: u64,
    pub(crate) stored_class: u8,
    /// Reducer-visible commands after conflicting proposals are retired, but
    /// before the reserved completion is materialized.
    pub(crate) queue_len_before: u64,
    pub(crate) queue_len_after: u64,
    pub(crate) reserved_slots_before: u8,
    pub(crate) reserved_slots_after: u8,
    pub(crate) queue_capacity: u64,
    pub(crate) ordinal_source_before: u128,
    pub(crate) physical_admission_ordinal: u128,
    pub(crate) lifecycle_ordinal: u128,
    pub(crate) ordinal_source_after: u128,
    pub(crate) dormant_reservations_before: u64,
    pub(crate) dormant_reservations_after: u64,
    /// Zero for a fresh reservation; otherwise the exact dormant backing
    /// removed when its aliased token becomes a physical command.
    pub(crate) dormant_owner_ordinal: u128,
}

/// Total prospective state transition for one durable leader-wire admission.
///
/// The projection retains the complete hashed lifecycle identity, both
/// immutable ordinals, the bounded-slot status, replay-dormant membership,
/// and both high-watermarks. It therefore distinguishes exact retry
/// coalescing from restart reactivation and from strictly newer terminal-slot
/// replacement before the persistence gate mutates state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionLeaderWireAdmissionTraceProjection {
    pub(crate) operation: u8,
    pub(crate) incoming_identity: CanonicalIdentityProjection,
    pub(crate) incumbent_identity: CanonicalIdentityProjection,
    pub(crate) stored_identity: CanonicalIdentityProjection,
    pub(crate) incoming_view: u64,
    pub(crate) incumbent_view: u64,
    pub(crate) stored_view: u64,
    pub(crate) incoming_admission_ordinal: u128,
    pub(crate) incumbent_admission_ordinal: u128,
    pub(crate) stored_admission_ordinal: u128,
    pub(crate) incoming_scheduler_ordinal: u128,
    pub(crate) incumbent_scheduler_ordinal: u128,
    pub(crate) stored_scheduler_ordinal: u128,
    pub(crate) last_admission_ordinal_before: u128,
    pub(crate) last_admission_ordinal_after: u128,
    pub(crate) scheduler_ordinal_high_watermark_before: u128,
    pub(crate) scheduler_ordinal_high_watermark_after: u128,
    pub(crate) records_before: u64,
    pub(crate) records_after: u64,
    pub(crate) capacity: u64,
    pub(crate) status_before: u8,
    pub(crate) status_after: u8,
    pub(crate) replay_dormant_before: bool,
    pub(crate) replay_dormant_after: bool,
    pub(crate) runtime_owner_before: bool,
    pub(crate) runtime_owner_after: bool,
    pub(crate) terminal_evidence_before: bool,
    pub(crate) terminal_evidence_after: bool,
}

/// Primitive observation of one exact daemon retry across its source-fair
/// outer queue and source-fair inner Sumeragi ingress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionTwoStageRelayRetryTraceProjection {
    /// Whether the daemon source capacity is exactly safety plus shared-high.
    pub daemon_source_capacity_matches_two_upstream_lanes: bool,
    /// Whether the class and retained corridors cover every configured source.
    pub class_corridor_covers_authenticated_sources: bool,
    /// Whether the authenticated route source equals the resource-credit owner.
    pub authenticated_source_matches_resource_owner: bool,
    /// Whether retry retained the same opaque delivery capability.
    pub retry_route_same_delivery: bool,
    /// Whether the retained retry route remains active.
    pub retry_route_active: bool,
    /// Whether the removed item satisfied the retry eligibility predicate.
    pub selected_eligible: bool,
    /// Number of ready authenticated sources before service.
    pub ready_sources_before: u64,
    /// Selected source's fair rank before service.
    pub selected_source_rank_before: u64,
    /// Number of ready authenticated sources after retry reinsertion.
    pub ready_sources_after: u64,
    /// Selected source's fair rank after retry reinsertion.
    pub selected_source_rank_after: u64,
    /// Selected source lane depth before service.
    pub source_depth_before: u64,
    /// Selected item's FIFO rank before service.
    pub selected_item_rank_before: u64,
    /// Selected source lane depth after retry reinsertion.
    pub source_depth_after: u64,
    /// Retried item's FIFO rank after reinsertion.
    pub selected_item_rank_after: u64,
    /// Total retained depth before service.
    pub total_depth_before: u64,
    /// Total retained depth after retry reinsertion.
    pub total_depth_after: u64,
    /// Per-authenticated-source retained capacity.
    pub source_capacity: u64,
    /// Total retained capacity across authenticated sources.
    pub total_capacity: u64,
}

/// Primitive per-source sidecar-flush cursor movement.
///
/// `stream_epoch` retains the non-zero durable request-stream incarnation,
/// `service_generation` binds it to one responder service lifetime, and
/// `semantic_sequence` identifies the occurrence within that stream. All
/// three are independent of the merge reference's `epoch_id`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionReliableFlushTraceProjection {
    pub(crate) status: u8,
    pub(crate) semantic_target: CanonicalIdentityProjection,
    pub(crate) authenticated_source: CanonicalIdentityProjection,
    pub(crate) source_key_identity: CanonicalIdentityProjection,
    pub(crate) delivery_route_identity: CanonicalIdentityProjection,
    pub(crate) writer_occurrence_identity: CanonicalIdentityProjection,
    pub(crate) requester: CanonicalIdentityProjection,
    pub(crate) responder: CanonicalIdentityProjection,
    pub(crate) connection_tenure_ordinal_high: u64,
    pub(crate) connection_tenure_ordinal_low: u64,
    pub(crate) delivery_ordinal_high: u64,
    pub(crate) delivery_ordinal_low: u64,
    pub(crate) ticket_id: u64,
    pub(crate) ticket_rank: u64,
    pub(crate) ticket_topic: u8,
    pub(crate) reply_writer_timeout_attempt: u8,
    pub(crate) canonical_request_digest: CanonicalIdentityProjection,
    pub(crate) stream_wire_bytes: u64,
    pub(crate) request_id: CanonicalIdentityProjection,
    pub(crate) service_generation: u64,
    pub(crate) stream_epoch: u64,
    pub(crate) semantic_sequence: u64,
    pub(crate) entry_hash: CanonicalIdentityProjection,
    pub(crate) encoded_len: u64,
    pub(crate) epoch_id: u64,
    pub(crate) reference_digest: CanonicalIdentityProjection,
    pub(crate) canonical_response_hash: CanonicalIdentityProjection,
    pub(crate) sidecar_response_hash: CanonicalIdentityProjection,
    pub(crate) chunk_hash: CanonicalIdentityProjection,
    pub(crate) payload_digest: CanonicalIdentityProjection,
    pub(crate) chunk_index: u64,
    pub(crate) chunk_count: u64,
    pub(crate) message_cursor_before: u64,
    pub(crate) message_cursor_after: u64,
    pub(crate) chunk_cursor_before: u64,
    pub(crate) chunk_cursor_after: u64,
    pub(crate) flushing_before: u64,
    pub(crate) flushing_after: u64,
    pub(crate) admitted_before: u64,
    pub(crate) admitted_after: u64,
    pub(crate) capacity: u64,
}

/// Exact lane-side application of one actor-confirmed sidecar writer flush.
///
/// Identity fields mirror the worker projection so the formal harness can
/// prove a non-vacuous two-phase link. The source-key, delivery-route, and
/// writer-occurrence identities are process-local only and never enter wire,
/// persistence, or consensus state. `service_generation`, `stream_epoch`, and
/// `semantic_sequence` are captured from the admitted occurrence, while their
/// `marker_*` counterparts and the other marker fields are independently
/// observed from the retained byte-free gate marker. Sibling state is both
/// compared as exact records by production and committed to a fixed-width,
/// domain-separated projection; it is never reduced to lane counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionReliableFlushApplicationProjection {
    pub(crate) semantic_target: CanonicalIdentityProjection,
    pub(crate) authenticated_source: CanonicalIdentityProjection,
    pub(crate) source_key_identity: CanonicalIdentityProjection,
    pub(crate) delivery_route_identity: CanonicalIdentityProjection,
    pub(crate) writer_occurrence_identity: CanonicalIdentityProjection,
    pub(crate) requester: CanonicalIdentityProjection,
    pub(crate) responder: CanonicalIdentityProjection,
    pub(crate) connection_tenure_ordinal_high: u64,
    pub(crate) connection_tenure_ordinal_low: u64,
    pub(crate) delivery_ordinal_high: u64,
    pub(crate) delivery_ordinal_low: u64,
    pub(crate) ticket_id: u64,
    pub(crate) ticket_rank: u64,
    pub(crate) ticket_topic: u8,
    pub(crate) reply_writer_timeout_attempt: u8,
    pub(crate) canonical_request_digest: CanonicalIdentityProjection,
    pub(crate) stream_wire_bytes: u64,
    pub(crate) request_id: CanonicalIdentityProjection,
    pub(crate) service_generation: u64,
    pub(crate) stream_epoch: u64,
    pub(crate) semantic_sequence: u64,
    pub(crate) entry_hash: CanonicalIdentityProjection,
    pub(crate) encoded_len: u64,
    pub(crate) epoch_id: u64,
    pub(crate) reference_digest: CanonicalIdentityProjection,
    pub(crate) canonical_response_hash: CanonicalIdentityProjection,
    pub(crate) sidecar_response_hash: CanonicalIdentityProjection,
    pub(crate) chunk_hash: CanonicalIdentityProjection,
    pub(crate) payload_digest: CanonicalIdentityProjection,
    pub(crate) chunk_index: u64,
    pub(crate) chunk_count: u64,
    pub(crate) message_cursor_before: u64,
    pub(crate) message_cursor_after: u64,
    pub(crate) chunk_cursor_before: u64,
    pub(crate) chunk_cursor_after: u64,
    pub(crate) marker_request_id: CanonicalIdentityProjection,
    pub(crate) marker_service_generation: u64,
    pub(crate) marker_stream_epoch: u64,
    pub(crate) marker_semantic_sequence: u64,
    pub(crate) marker_entry_hash: CanonicalIdentityProjection,
    pub(crate) marker_encoded_len: u64,
    pub(crate) marker_epoch_id: u64,
    pub(crate) marker_reference_digest: CanonicalIdentityProjection,
    pub(crate) marker_requester: CanonicalIdentityProjection,
    pub(crate) marker_responder: CanonicalIdentityProjection,
    pub(crate) marker_canonical_response_hash: CanonicalIdentityProjection,
    pub(crate) marker_sidecar_response_hash: CanonicalIdentityProjection,
    pub(crate) marker_chunk_hash: CanonicalIdentityProjection,
    pub(crate) marker_payload_digest: CanonicalIdentityProjection,
    pub(crate) marker_chunk_index: u64,
    pub(crate) marker_chunk_count: u64,
    pub(crate) marker_topic: u8,
    pub(crate) claim_acquired: bool,
    pub(crate) gate_marker_present_before: bool,
    pub(crate) gate_marker_present_after: bool,
    pub(crate) gate_cursor_before: u64,
    pub(crate) gate_cursor_after: u64,
    pub(crate) gate_complete_after: bool,
    pub(crate) gate_attempt_present_after: bool,
    pub(crate) outbound_attempt_present_before: bool,
    pub(crate) outbound_route_bound_before: bool,
    pub(crate) outbound_route_active_before: bool,
    pub(crate) outbound_cursor_before: u64,
    pub(crate) outbound_cursor_after: u64,
    pub(crate) outbound_in_flight_before_present: bool,
    pub(crate) outbound_in_flight_before: u64,
    pub(crate) outbound_queued_before: bool,
    pub(crate) outbound_order_count_before: u64,
    pub(crate) outbound_order_rank_before: u64,
    pub(crate) sibling_order_len_before: u64,
    pub(crate) outbound_attempt_present_after: bool,
    pub(crate) outbound_in_flight_after_present: bool,
    pub(crate) outbound_queued_after: bool,
    pub(crate) outbound_order_count_after: u64,
    pub(crate) outbound_order_rank_after: u64,
    pub(crate) sibling_order_len_after: u64,
    pub(crate) inserted_preserved: bool,
    pub(crate) inserted_equals_now: bool,
    pub(crate) target_gate_residual_records_equal: bool,
    pub(crate) target_gate_residual_before: CanonicalIdentityProjection,
    pub(crate) target_gate_residual_after: CanonicalIdentityProjection,
    pub(crate) target_outbound_residual_records_equal: bool,
    pub(crate) target_outbound_residual_before: CanonicalIdentityProjection,
    pub(crate) target_outbound_residual_after: CanonicalIdentityProjection,
    pub(crate) shared_transfer_present_before: bool,
    pub(crate) shared_transfer_present_after: bool,
    pub(crate) shared_transfer_other_attempts_before: bool,
    pub(crate) shared_transfer_records_equal: bool,
    pub(crate) shared_transfer_state_before: CanonicalIdentityProjection,
    pub(crate) shared_transfer_state_after: CanonicalIdentityProjection,
    pub(crate) sibling_records_equal: bool,
    pub(crate) sibling_state_before: CanonicalIdentityProjection,
    pub(crate) sibling_state_after: CanonicalIdentityProjection,
}

/// Primitive durable application completion returned to the reducer owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionApplicationTraceProjection {
    pub(crate) task_tag: TagProjection,
    pub(crate) owner_tag: TagProjection,
    pub(crate) task_generation: u64,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) context_height: u64,
    pub(crate) commit_qc: ProductionQuorumCertificateIdentityProjection,
    pub(crate) validated_body: ProductionDurableBodyIdentityProjection,
    pub(crate) validated_execution_commitment: CanonicalIdentityProjection,
    pub(crate) proposal_block_hash: CanonicalIdentityProjection,
    pub(crate) proposal_payload_hash: CanonicalIdentityProjection,
    pub(crate) committed_block_hash: CanonicalIdentityProjection,
    pub(crate) executed_block_wire_hash: CanonicalIdentityProjection,
    pub(crate) kura_decision: ProductionDecisionIdentityProjection,
    pub(crate) kura_artifact_hash: CanonicalIdentityProjection,
    pub(crate) artifact_context_id: CanonicalIdentityProjection,
    pub(crate) artifact_height: u64,
    pub(crate) artifact_subject: CanonicalIdentityProjection,
    pub(crate) artifact_block_hash: CanonicalIdentityProjection,
    pub(crate) artifact_commit_qc: ProductionQuorumCertificateIdentityProjection,
    pub(crate) artifact_hash: CanonicalIdentityProjection,
    pub(crate) state_height_after: u64,
    pub(crate) task_work_id: u64,
    pub(crate) completion_work_id: u64,
}

/// Exact applied-height handoff before successor construction begins.
///
/// This projection separates one authenticated application boundary from the
/// subsequent runner-owned successor action.  It intentionally contains no
/// finite-horizon or `MaxHeight` field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductionTerminalApplicationWithoutSuccessorActivationProjection {
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) context_height: u64,
    pub(crate) receipt_context_id: CanonicalIdentityProjection,
    pub(crate) receipt_height: u64,
    pub(crate) receipt_block_hash: CanonicalIdentityProjection,
    pub(crate) receipt_artifact_hash: CanonicalIdentityProjection,
    pub(crate) artifact_context_id: CanonicalIdentityProjection,
    pub(crate) artifact_height: u64,
    pub(crate) artifact_block_hash: CanonicalIdentityProjection,
    pub(crate) artifact_hash: CanonicalIdentityProjection,
    pub(crate) predecessor: ProductionDurablePredecessorIdentityProjection,
    pub(crate) pending_successor_activation_present: bool,
}

/// Primitive durable owner of one exact lane queue reservation.
///
/// The zero identity is permitted only for [`IN_FLIGHT_RESERVATION_STATE_ABSENT`].
/// Prepared/completed owners additionally carry the exact ordered-release
/// barrier digest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightReservationOwnerProjection {
    pub(crate) state: u8,
    pub(crate) reservation_identity: CanonicalIdentityProjection,
    pub(crate) release_identity: CanonicalIdentityProjection,
}

/// One exact reservation-journal action and its primitive owner transition.
///
/// This projection intentionally excludes the queue collection, QueuePlan,
/// Kura, carrier/WSV, and FIFO-order witnesses. It is the checked local
/// identity seam used by the production journal, not a claim of complete
/// forward or reverse adequacy with the first-release TLA+ model. Local
/// ForgetCommit/ForgetRelease both end at `Absent`; the abstract model's
/// distinct forgotten states require the excluded QueuePlan/FIFO evidence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightReservationTransitionProjection {
    pub(crate) action: u8,
    pub(crate) requested_reservation_identity: CanonicalIdentityProjection,
    pub(crate) requested_release_identity: CanonicalIdentityProjection,
    pub(crate) before: ProductionInFlightReservationOwnerProjection,
    pub(crate) after: ProductionInFlightReservationOwnerProjection,
}

/// QueuePlan V4 and reservation journal V5 state in the composed carrier model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseQueueProjection {
    pub(crate) plan_state: u8,
    pub(crate) selected_count: u64,
    pub(crate) reservation_state: u8,
}

/// Durable Kura/input/QC facts in the composed carrier model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseCarrierProjection {
    pub(crate) kura_active: u128,
    pub(crate) execution_input_durable: u128,
    pub(crate) ready_qc_durable: bool,
}

/// Volatile body and READY authorization custody.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseSessionProjection {
    pub(crate) bodies: u128,
    pub(crate) ready_authorized: u128,
    pub(crate) crashed: u128,
    pub(crate) producer_alive: bool,
}

/// Monotonic durable history used to validate crash reconstruction and order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseHistoryProjection {
    pub(crate) ever_queue_plan_v4: bool,
    pub(crate) ever_reservation_v5: bool,
    pub(crate) ever_execution_input_durable: u128,
    pub(crate) ever_ready_authorized: u128,
    pub(crate) ready_signed: u128,
    pub(crate) ever_ready_qc_durable: bool,
    /// Number of canonical ordered reservation keys with durable Commit records.
    pub(crate) reservation_committed_prefix: u64,
    /// Number of canonical ordered QueuePlan V4 keys with durable tombstones.
    pub(crate) queue_plan_tombstoned_prefix: u64,
    /// Number of canonical ordered Commit keys durably forgotten.
    pub(crate) reservation_commit_forgotten_prefix: u64,
    pub(crate) pending_high_water: u64,
    pub(crate) released_high_water: u64,
}

/// Lane decision, canonical WSV application, and mutually exclusive release owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseDecisionProjection {
    pub(crate) lane_commit_scope: CanonicalIdentityProjection,
    pub(crate) release_scope: CanonicalIdentityProjection,
    pub(crate) lane_commit_owner: u128,
    pub(crate) release_owner: u128,
    pub(crate) wsv_committed: bool,
    pub(crate) application_count: u8,
    pub(crate) applied_by: u128,
}

/// Durable four-stage release progress and ordinary FIFO publication.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseReleaseProjection {
    pub(crate) kura_retired: bool,
    pub(crate) pending_prefix: u64,
    pub(crate) released_prefix: u64,
    pub(crate) fifo_restored: bool,
}

/// Total fixed-width safety state for a first-release carrier committee.
///
/// Validator sets use the same 1..=128 canonical-order bitmap geometry as the
/// production height context. The paired TLA+ instance remains deliberately
/// bounded to one producer and two replicas; its states embed into this wider
/// relation. `binding_a` is the exact content-bound FIFO-ordered conjunction
/// of the selected QueuePlan admission preimages. `payload_binding_a`
/// identifies the authenticated committee members whose custody of that
/// complete reservation group is established at this boundary; it is
/// committee-bounded and must include the selected producer, but it does not
/// assert knowledge by every validator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseStateProjection {
    pub(crate) validator_count: u8,
    pub(crate) producer: u128,
    pub(crate) producer_selected_owner: u128,
    pub(crate) replicated_carrier_owners: u128,
    pub(crate) payload_binding_a: u128,
    pub(crate) binding_a: CanonicalIdentityProjection,
    pub(crate) queue: ProductionInFlightFirstReleaseQueueProjection,
    pub(crate) carrier: ProductionInFlightFirstReleaseCarrierProjection,
    pub(crate) session: ProductionInFlightFirstReleaseSessionProjection,
    pub(crate) history: ProductionInFlightFirstReleaseHistoryProjection,
    pub(crate) decision: ProductionInFlightFirstReleaseDecisionProjection,
    pub(crate) release: ProductionInFlightFirstReleaseReleaseProjection,
}

/// One named `Next` action over the total first-release carrier projection.
///
/// `actor` and `target` are zero for actor-free actions, one-hot for ordinary
/// validator actions, and respectively source/target for late-body service.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionInFlightFirstReleaseTransitionProjection {
    pub(crate) action: u8,
    pub(crate) actor: u128,
    pub(crate) target: u128,
    pub(crate) before: ProductionInFlightFirstReleaseStateProjection,
    pub(crate) after: ProductionInFlightFirstReleaseStateProjection,
}

/// Reverse ownership classification for a terminal Commit or release state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)] // Consumed by the verification harness and refinement tests.
pub(crate) struct ProductionInFlightFirstReleaseTerminalOwnerProjection {
    pub(crate) ordinary_fifo_owner: bool,
    pub(crate) canonical_wsv_owner: bool,
    pub(crate) commit_terminal: bool,
    pub(crate) release_terminal: bool,
}

/// Opaque evidence that one production transition gate accepted a projection.
///
/// The field is private so callers cannot manufacture authorization from a
/// projection they already hold. Every constructor below evaluates the
/// executable kernel and returns `None` on rejection; consumers must acquire
/// this token before crossing their state-changing linearization point.
#[must_use = "checked transition evidence must be consumed at the authorized mutation boundary"]
#[derive(Debug, PartialEq, Eq)]
pub struct CheckedProductionTransition<P> {
    projection: P,
}

impl<P> CheckedProductionTransition<P> {
    /// Borrow the exact accepted projection without consuming its authority.
    ///
    /// This supports deterministic composition checks while retaining the
    /// move-only token for the authorized mutation boundary.
    #[must_use]
    pub(crate) const fn accepted_projection(&self) -> &P {
        &self.projection
    }

    /// Consume the checked token and recover the exact accepted projection.
    #[must_use]
    pub fn into_projection(self) -> P {
        self.projection
    }
}

const fn effective_lock_trace_step_is_valid(projection: EffectiveLockTraceProjection) -> bool {
    effective_lock_trace_step_body!(projection)
}

/// Primitive identity of one optional quorum certificate.
///
/// The signer bitmap is indexed by the frozen, canonically ordered voting
/// roster. `evidence_class` is assigned by full concrete
/// [`super::QuorumCertificate`] equality against the local and incoming
/// transition anchors, so signature/evidence changes cannot hide behind an
/// equal stable certificate reference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CertificateIdentityProjection {
    pub(crate) present: bool,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) phase: u8,
    pub(crate) subject: CanonicalIdentityProjection,
    pub(crate) signer_bitmap: u128,
    pub(crate) signer_bitmap_count: u64,
    pub(crate) signer_count: u64,
    pub(crate) voting_power: u64,
    pub(crate) evidence_class: u8,
}

/// Primitive identity of one optional timeout certificate and its selected
/// highest `PrepareQC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeoutIdentityProjection {
    pub(crate) present: bool,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) highest_prepare: CertificateIdentityProjection,
}

/// Exact production projection of a persisted-TC `EnterView` macro-step.
///
/// This relation is deliberately limited to the reducer-owned transition. It
/// proves which lock crosses the serialized view boundary and which immediate
/// recovery fetch names it; asynchronous ownership and scheduler fairness stay
/// separate obligations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnterViewProjection {
    pub(crate) active: bool,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) before_tag: TagProjection,
    pub(crate) after_tag: TagProjection,
    pub(crate) pending_record_kind: u8,
    pub(crate) pending_continuation: u8,
    pub(crate) pending_record_timeout: TimeoutIdentityProjection,
    pub(crate) pending_continuation_timeout: TimeoutIdentityProjection,
    pub(crate) durable_timeout_after: TimeoutIdentityProjection,
    pub(crate) effect_timeout: TimeoutIdentityProjection,
    pub(crate) local_lock_before: CertificateIdentityProjection,
    pub(crate) local_highest_before: CertificateIdentityProjection,
    pub(crate) incoming_highest_for_control: CertificateIdentityProjection,
    pub(crate) durable_lock_after: CertificateIdentityProjection,
    pub(crate) durable_highest_after: CertificateIdentityProjection,
    pub(crate) prepare_control_slot_present_after: bool,
    pub(crate) retained_prepare_qc_after: CertificateIdentityProjection,
    pub(crate) effect_protected_lock: CertificateIdentityProjection,
    pub(crate) following_fetch_lock: CertificateIdentityProjection,
    pub(crate) enter_count: u64,
    pub(crate) fetch_count: u64,
    pub(crate) enter_index: u8,
    pub(crate) following_fetch_index: u8,
}

/// Primitive optional validator identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValidatorProjection {
    pub(crate) present: bool,
    pub(crate) id: ValidatorId,
}

/// Primitive optional subject identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubjectProjection {
    pub(crate) present: bool,
    pub(crate) subject: Subject,
}

/// Concrete invariant violations extracted from one reducer state.
///
/// These are counts, not caller-provided truth values.  The verified kernel
/// accepts a state only when every independently computed class is empty.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SafetyProjection {
    pub(crate) durable_identity_mismatches: u64,
    pub(crate) asynchronous_fence_conflicts: u64,
    pub(crate) invalid_highest_prepare: u64,
    pub(crate) invalid_lock: u64,
    pub(crate) invalid_timeout: u64,
    pub(crate) invalid_decision: u64,
    pub(crate) invalid_pending_append: u64,
    pub(crate) unauthorized_signables: u64,
    pub(crate) invalid_application: u64,
}

/// Safety identity of one pending WAL append and its continuation.
///
/// `record_kind == WAL_RECORD_NONE` is the sole absent value.  The remaining
/// fields are canonical zeroes in that case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PendingProjection {
    pub(crate) record_kind: u8,
    pub(crate) continuation: u8,
    pub(crate) persistence_id: u64,
    pub(crate) context_id: CanonicalIdentityProjection,
    pub(crate) height: u64,
    pub(crate) view: u64,
    /// Whether the WAL record authenticates a proposal-body origin.
    pub(crate) proposal_present: bool,
    /// Immutable proposal-origin height, or zero when absent.
    pub(crate) proposal_height: u64,
    /// Immutable proposal-origin view, or zero when absent.
    pub(crate) proposal_view: u64,
    pub(crate) subject: CanonicalIdentityProjection,
}

/// Concrete capability key used for one reducer effect.
///
/// The key contains only fixed-width, safety-relevant primitives.  Signature
/// bytes are deliberately outside the reducer proof boundary; their checked
/// message identity is represented by context, round, phase, subject, actor,
/// WAL id, and auxiliary certificate/manifest identity below.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectCapabilityKey {
    pub(crate) kind: u8,
    pub(crate) tag: TagProjection,
    pub(crate) context_id: ContextId,
    pub(crate) height: u64,
    pub(crate) view: u64,
    pub(crate) proposal_height: u64,
    pub(crate) proposal_view: u64,
    pub(crate) phase: u8,
    pub(crate) subject: Subject,
    pub(crate) actor: ValidatorId,
    pub(crate) persistence_id: u64,
    pub(crate) record_kind: u8,
    pub(crate) auxiliary_present: bool,
    pub(crate) auxiliary_context_id: ContextId,
    pub(crate) auxiliary_height: u64,
    pub(crate) auxiliary_view: u64,
    pub(crate) auxiliary_proposal_height: u64,
    pub(crate) auxiliary_proposal_view: u64,
    pub(crate) auxiliary_phase: u8,
    pub(crate) auxiliary_subject: Subject,
    pub(crate) manifest_payload: Digest,
    pub(crate) manifest_chunks: Digest,
    pub(crate) manifest_len: u64,
    pub(crate) manifest_count: u64,
}

impl EffectCapabilityKey {
    /// Canonical absent key.
    pub(crate) const fn none() -> Self {
        Self {
            kind: EFFECT_NONE,
            tag: TagProjection {
                height: 0,
                view: 0,
                generation: 0,
            },
            context_id: ContextId::repeat(0),
            height: 0,
            view: 0,
            proposal_height: 0,
            proposal_view: 0,
            phase: 0,
            subject: Subject::repeat(0),
            actor: ValidatorId::repeat(0),
            persistence_id: 0,
            record_kind: WAL_RECORD_NONE,
            auxiliary_present: false,
            auxiliary_context_id: ContextId::repeat(0),
            auxiliary_height: 0,
            auxiliary_view: 0,
            auxiliary_proposal_height: 0,
            auxiliary_proposal_view: 0,
            auxiliary_phase: 0,
            auxiliary_subject: Subject::repeat(0),
            manifest_payload: Digest::repeat(0),
            manifest_chunks: Digest::repeat(0),
            manifest_len: 0,
            manifest_count: 0,
        }
    }
}

/// Exact requested/granted capability pair for one effect vector slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectSlotProjection {
    pub(crate) kind: u8,
    pub(crate) requested: EffectCapabilityKey,
    pub(crate) granted: EffectCapabilityKey,
}

/// One exact item in the durable replay FIFO.
///
/// `kind` uses the [`REPLAY_EFFECT_*`] discriminants.  Proposal, Prepare,
/// Commit, and timeout items carry an [`EFFECT_SIGN`] capability; a Decision
/// carries the one [`EFFECT_FETCH`] frontier reconstructed on recovery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayPlanSlotProjection {
    pub(crate) kind: u8,
    pub(crate) capability: EffectCapabilityKey,
}

impl ReplayPlanSlotProjection {
    /// Canonical absent replay-plan slot.
    pub(crate) const fn none() -> Self {
        Self {
            kind: REPLAY_EFFECT_NONE,
            capability: EffectCapabilityKey::none(),
        }
    }
}

/// Fixed projection of the complete reducer-owned replay FIFO.
///
/// Recovery can reconstruct at most three signable intents, in this order:
/// current Timeout-or-Proposal, current Prepare, and the exact locked Commit.
/// A durable Decision is instead represented by one body-fetch frontier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayPlanProjection {
    pub(crate) len: u8,
    pub(crate) slot0: ReplayPlanSlotProjection,
    pub(crate) slot1: ReplayPlanSlotProjection,
    pub(crate) slot2: ReplayPlanSlotProjection,
}

impl ReplayPlanProjection {
    /// Construct an empty canonical replay plan.
    pub(crate) const fn empty() -> Self {
        Self {
            len: 0,
            slot0: ReplayPlanSlotProjection::none(),
            slot1: ReplayPlanSlotProjection::none(),
            slot2: ReplayPlanSlotProjection::none(),
        }
    }

    /// Append one exact replay item, failing closed beyond the protocol bound.
    pub(crate) fn push(&mut self, kind: u8, capability: EffectCapabilityKey) -> bool {
        let slot = ReplayPlanSlotProjection { kind, capability };
        match self.len {
            0 => self.slot0 = slot,
            1 => self.slot1 = slot,
            2 => self.slot2 = slot,
            _ => return false,
        }
        self.len += 1;
        true
    }
}

/// Primitive identity of a durable-boundary action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoundaryCapabilityKey {
    pub(crate) kind: u8,
    pub(crate) record_kind: u8,
    pub(crate) continuation: u8,
    pub(crate) replay_effect_kind: u8,
    pub(crate) persistence_id: u64,
    pub(crate) context_id: ContextId,
    pub(crate) context_identity: CanonicalIdentityProjection,
    pub(crate) tag: TagProjection,
    pub(crate) subject: SubjectProjection,
    pub(crate) subject_identity: CanonicalIdentityProjection,
    /// Whether the primary WAL identity authenticates a proposal-body origin.
    pub(crate) proposal_present: bool,
    /// Immutable primary proposal-origin height, or zero when absent.
    pub(crate) proposal_height: u64,
    /// Immutable primary proposal-origin view, or zero when absent.
    pub(crate) proposal_view: u64,
    /// Whether the WAL record carries an auxiliary certificate reference.
    pub(crate) auxiliary_present: bool,
    pub(crate) auxiliary_context_id: ContextId,
    pub(crate) auxiliary_height: u64,
    pub(crate) auxiliary_view: u64,
    pub(crate) auxiliary_proposal_height: u64,
    pub(crate) auxiliary_proposal_view: u64,
    pub(crate) auxiliary_phase: u8,
    pub(crate) auxiliary_subject: Subject,
    pub(crate) replay_plan: ReplayPlanProjection,
}

impl BoundaryCapabilityKey {
    /// Canonical absent boundary capability.
    pub(crate) const fn none() -> Self {
        Self {
            kind: BOUNDARY_NONE,
            record_kind: WAL_RECORD_NONE,
            continuation: CONTINUATION_NONE,
            replay_effect_kind: REPLAY_EFFECT_NONE,
            persistence_id: 0,
            context_id: ContextId::repeat(0),
            context_identity: CanonicalIdentityProjection::zero(),
            tag: TagProjection {
                height: 0,
                view: 0,
                generation: 0,
            },
            subject: SubjectProjection {
                present: false,
                subject: Subject::repeat(0),
            },
            subject_identity: CanonicalIdentityProjection::zero(),
            proposal_present: false,
            proposal_height: 0,
            proposal_view: 0,
            auxiliary_present: false,
            auxiliary_context_id: ContextId::repeat(0),
            auxiliary_height: 0,
            auxiliary_view: 0,
            auxiliary_proposal_height: 0,
            auxiliary_proposal_view: 0,
            auxiliary_phase: 0,
            auxiliary_subject: Subject::repeat(0),
            replay_plan: ReplayPlanProjection::empty(),
        }
    }
}

/// Exact cardinality projection of the reducer's volatile collections.
///
/// This summary is not a replacement for verifying the collection extraction
/// itself.  It makes the production gate fail closed if an accepted candidate
/// exceeds a protocol-derived bound, and gives Verus the same fixed-width
/// state shape used by the executable check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VolatileSummary {
    pub(crate) candidate_present: bool,
    pub(crate) body_work: u64,
    pub(crate) pending_prepare: u64,
    pub(crate) known_prepare: u64,
    pub(crate) vote_pools: u64,
    pub(crate) vote_entries: u64,
    pub(crate) timeout_vote_pools: u64,
    pub(crate) timeout_vote_entries: u64,
    pub(crate) formed_certificates: u64,
    pub(crate) formed_timeouts: u64,
    pub(crate) outbound_control: u64,
    pub(crate) signature_queue: u64,
    pub(crate) awaiting_signature: bool,
    /// Upper bound derived from durable proposal/Prepare/Commit/timeout
    /// intents that may be resumed for signing.
    pub(crate) durable_signable_limit: u64,
    pub(crate) replay_resumed: bool,
}

/// Fixed, exact projection of one reducer effect vector.
///
/// Each active slot carries a capability requested by the concrete effect and
/// an independently reconstructed capability granted by the candidate state.
/// The kernel, rather than the reducer extractor, decides authorization by
/// requiring the complete fixed-width keys to match.  Slots at or beyond
/// `len` must be canonical zeroes.  A fixed representation keeps complete
/// vector order visible to Verus without trusting an iterator implementation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectTrace {
    pub(crate) len: u8,
    pub(crate) slot0: EffectSlotProjection,
    pub(crate) slot1: EffectSlotProjection,
    pub(crate) slot2: EffectSlotProjection,
    pub(crate) slot3: EffectSlotProjection,
    pub(crate) slot4: EffectSlotProjection,
    pub(crate) slot5: EffectSlotProjection,
    pub(crate) slot6: EffectSlotProjection,
    pub(crate) slot7: EffectSlotProjection,
}

impl EffectTrace {
    /// Construct an empty canonical trace.
    pub(crate) const fn empty() -> Self {
        Self {
            len: 0,
            slot0: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot1: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot2: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot3: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot4: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot5: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot6: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
            slot7: EffectSlotProjection {
                kind: EFFECT_NONE,
                requested: EffectCapabilityKey::none(),
                granted: EffectCapabilityKey::none(),
            },
        }
    }

    /// Append one exact effect projection.
    pub(crate) fn push(
        &mut self,
        requested: EffectCapabilityKey,
        granted: EffectCapabilityKey,
    ) -> bool {
        let index = usize::from(self.len);
        if index >= MAX_EFFECTS_PER_STEP {
            return false;
        }
        let slot = EffectSlotProjection {
            kind: requested.kind,
            requested,
            granted,
        };
        match index {
            0 => self.slot0 = slot,
            1 => self.slot1 = slot,
            2 => self.slot2 = slot,
            3 => self.slot3 = slot,
            4 => self.slot4 = slot,
            5 => self.slot5 = slot,
            6 => self.slot6 = slot,
            7 => self.slot7 = slot,
            _ => return false,
        }
        self.len += 1;
        true
    }
}

/// Concrete primitive projection consumed by the verified transition kernel.
///
/// Exact reducer and durable-state references are compared directly by the
/// executable kernel.  The Verus instantiation represents those identities as
/// mathematical integers, while sharing the same derivation expression.
#[derive(Clone, Copy, Debug)]
pub struct TransitionProjection<'a> {
    pub(crate) before_state: &'a Reducer,
    pub(crate) after_state: &'a Reducer,
    pub(crate) durable_before: &'a DurableState,
    pub(crate) durable_after: &'a DurableState,
    pub(crate) safety_before: SafetyProjection,
    pub(crate) safety_after: SafetyProjection,
    pub(crate) context_before: &'a HeightContext,
    pub(crate) context_after: &'a HeightContext,
    pub(crate) local_before: ValidatorProjection,
    pub(crate) local_after: ValidatorProjection,
    pub(crate) event_tag: TagProjection,
    pub(crate) height_before: u64,
    pub(crate) view_before: u64,
    pub(crate) generation_before: u64,
    pub(crate) generation_after: u64,
    pub(crate) pending_state_before: Option<&'a PendingPersistence>,
    pub(crate) pending_state_after: Option<&'a PendingPersistence>,
    pub(crate) pending_before: PendingProjection,
    pub(crate) awaiting_before: bool,
    pub(crate) replay_before: bool,
    pub(crate) application_before: SubjectProjection,
    pub(crate) application_after: SubjectProjection,
    pub(crate) event_kind: u8,
    pub(crate) awaiting_message_kind: u8,
    pub(crate) validator_count: u64,
    pub(crate) volatile_before: VolatileSummary,
    pub(crate) volatile_after: VolatileSummary,
    pub(crate) timeout_votes_before: &'a BTreeMap<Round, BTreeMap<ValidatorId, SignedTimeoutVote>>,
    pub(crate) timeout_votes_after: &'a BTreeMap<Round, BTreeMap<ValidatorId, SignedTimeoutVote>>,
    pub(crate) formed_timeouts_before: &'a BTreeSet<Round>,
    pub(crate) formed_timeouts_after: &'a BTreeSet<Round>,
    pub(crate) timeout_evidence_after_outside_installed_window: u64,
    pub(crate) timeout_control_before: Option<&'a ConsensusMessageV2>,
    pub(crate) timeout_control_after: Option<&'a ConsensusMessageV2>,
    pub(crate) boundary_claimed: BoundaryCapabilityKey,
    pub(crate) boundary_granted: BoundaryCapabilityKey,
    pub(crate) enter_view: EnterViewProjection,
    pub(crate) effects: EffectTrace,
}

/// Internal facts derived by the kernel from one primitive transition.
///
/// This type never crosses the module boundary; in particular, the reducer
/// cannot supply any of its boolean fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct TransitionFacts {
    before_invariant: bool,
    after_invariant: bool,
    context_unchanged: bool,
    whole_state_unchanged: bool,
    tag_matches: bool,
    busy_fence_open: bool,
    event_kind: u8,
    action_kind: u8,
    wal_record_kind: u8,
    signed_message_kind: u8,
    replay_effect_kind: u8,
    validator_count: u64,
    volatile_before: VolatileSummary,
    volatile_after: VolatileSummary,
    durable_unchanged: bool,
    pending_unchanged: bool,
    generation_unchanged: bool,
    application_unchanged: bool,
    begin_persist_exact: bool,
    acknowledge_persist_exact: bool,
    application_transition_exact: bool,
    acknowledgement_continuation: u8,
    install_view_unchanged: bool,
    timeout_vote_pool_unchanged: bool,
    formed_timeouts_unchanged: bool,
    timeout_evidence_after_in_installed_window: bool,
    timeout_control_unchanged: bool,
    timeout_control_after_absent: bool,
    enter_view_exact: bool,
    effects: EffectTrace,
}

/// Classification fields derived independently of durable/effect deltas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct TransitionClassificationFacts {
    before_invariant: bool,
    after_invariant: bool,
    context_unchanged: bool,
    whole_state_unchanged: bool,
    tag_matches: bool,
    busy_fence_open: bool,
    event_kind: u8,
    action_kind: u8,
    wal_record_kind: u8,
    signed_message_kind: u8,
    replay_effect_kind: u8,
    validator_count: u64,
}

/// Durable, volatile, capability, and effect fields derived independently of
/// transition classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct TransitionDeltaFacts {
    volatile_before: VolatileSummary,
    volatile_after: VolatileSummary,
    durable_unchanged: bool,
    pending_unchanged: bool,
    generation_unchanged: bool,
    application_unchanged: bool,
    begin_persist_exact: bool,
    acknowledge_persist_exact: bool,
    application_transition_exact: bool,
    acknowledgement_continuation: u8,
    install_view_unchanged: bool,
    timeout_vote_pool_unchanged: bool,
    formed_timeouts_unchanged: bool,
    timeout_evidence_after_in_installed_window: bool,
    timeout_control_unchanged: bool,
    timeout_control_after_absent: bool,
    replay_boundary_exact: bool,
    effects: EffectTrace,
}

// Keep this expression free of calls into production code.  It is expanded
// both below and inside `verus!`, so Verus proves the actual decision logic
// used by the production reducer rather than a separately transcribed checker.
macro_rules! effect_count_body {
    ($trace:expr, $kind:expr) => {{
        (if $trace.len > 0 && $trace.slot0.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 1 && $trace.slot1.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 2 && $trace.slot2.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 3 && $trace.slot3.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 4 && $trace.slot4.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 5 && $trace.slot5.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 6 && $trace.slot6.kind == $kind {
            1u64
        } else {
            0u64
        }) + (if $trace.len > 7 && $trace.slot7.kind == $kind {
            1u64
        } else {
            0u64
        })
    }};
}

macro_rules! capability_key_equal_body {
    ($left:expr, $right:expr) => {{
        $left.kind == $right.kind
            && $left.tag.height == $right.tag.height
            && $left.tag.view == $right.tag.view
            && $left.tag.generation == $right.tag.generation
            && $left.context_id == $right.context_id
            && $left.height == $right.height
            && $left.view == $right.view
            && $left.proposal_height == $right.proposal_height
            && $left.proposal_view == $right.proposal_view
            && $left.phase == $right.phase
            && $left.subject == $right.subject
            && $left.actor == $right.actor
            && $left.persistence_id == $right.persistence_id
            && $left.record_kind == $right.record_kind
            && $left.auxiliary_present == $right.auxiliary_present
            && $left.auxiliary_context_id == $right.auxiliary_context_id
            && $left.auxiliary_height == $right.auxiliary_height
            && $left.auxiliary_view == $right.auxiliary_view
            && $left.auxiliary_proposal_height == $right.auxiliary_proposal_height
            && $left.auxiliary_proposal_view == $right.auxiliary_proposal_view
            && $left.auxiliary_phase == $right.auxiliary_phase
            && $left.auxiliary_subject == $right.auxiliary_subject
            && $left.manifest_payload == $right.manifest_payload
            && $left.manifest_chunks == $right.manifest_chunks
            && $left.manifest_len == $right.manifest_len
            && $left.manifest_count == $right.manifest_count
    }};
}

macro_rules! capability_key_is_none_body {
    ($key:expr) => {{ $key.kind == 0u8 }};
}

macro_rules! active_effect_slot_body {
    ($slot:expr) => {{
        $slot.kind >= 1u8
            && $slot.kind <= 9u8
            && $slot.requested.kind == $slot.kind
            && $slot.granted.kind == $slot.kind
            && capability_key_equal_body!($slot.requested, $slot.granted)
    }};
}

macro_rules! inactive_effect_slot_body {
    ($slot:expr) => {{
        $slot.kind == 0u8
            && capability_key_is_none_body!($slot.requested)
            && capability_key_is_none_body!($slot.granted)
    }};
}

macro_rules! effect_slots_authorized_body {
    ($trace:expr) => {{
        $trace.len <= 8u8
            && (if $trace.len > 0 {
                active_effect_slot_body!($trace.slot0)
            } else {
                inactive_effect_slot_body!($trace.slot0)
            })
            && (if $trace.len > 1 {
                active_effect_slot_body!($trace.slot1)
            } else {
                inactive_effect_slot_body!($trace.slot1)
            })
            && (if $trace.len > 2 {
                active_effect_slot_body!($trace.slot2)
            } else {
                inactive_effect_slot_body!($trace.slot2)
            })
            && (if $trace.len > 3 {
                active_effect_slot_body!($trace.slot3)
            } else {
                inactive_effect_slot_body!($trace.slot3)
            })
            && (if $trace.len > 4 {
                active_effect_slot_body!($trace.slot4)
            } else {
                inactive_effect_slot_body!($trace.slot4)
            })
            && (if $trace.len > 5 {
                active_effect_slot_body!($trace.slot5)
            } else {
                inactive_effect_slot_body!($trace.slot5)
            })
            && (if $trace.len > 6 {
                active_effect_slot_body!($trace.slot6)
            } else {
                inactive_effect_slot_body!($trace.slot6)
            })
            && (if $trace.len > 7 {
                active_effect_slot_body!($trace.slot7)
            } else {
                inactive_effect_slot_body!($trace.slot7)
            })
    }};
}

macro_rules! final_effect_kind_body {
    ($trace:expr, $kind:expr) => {{
        match $trace.len {
            1 => $trace.slot0.kind == $kind,
            2 => $trace.slot1.kind == $kind,
            3 => $trace.slot2.kind == $kind,
            4 => $trace.slot3.kind == $kind,
            5 => $trace.slot4.kind == $kind,
            6 => $trace.slot5.kind == $kind,
            7 => $trace.slot6.kind == $kind,
            8 => $trace.slot7.kind == $kind,
            _ => false,
        }
    }};
}

macro_rules! effect_order_constraints_body {
    (
        $trace:expr,
        $event_kind:expr,
        $persist_count:expr,
        $fetch_count:expr,
        $store_count:expr,
        $validate_count:expr,
        $sign_count:expr,
        $apply_count:expr,
        $enter_count:expr $(,)?
    ) => {{
        $persist_count <= 1u64
            && $store_count <= 1u64
            && $validate_count <= 1u64
            && $sign_count <= 1u64
            && $apply_count <= 1u64
            && $enter_count <= 1u64
            // A signing request is always the final effect.  This makes any
            // preceding view-entry, decision handling, fetch, or broadcast
            // observable before the next asynchronous signing completion.
            && ($sign_count == 0u64
                || match $trace.len {
                    1 => $trace.slot0.kind == 5u8,
                    2 => $trace.slot1.kind == 5u8,
                    3 => $trace.slot2.kind == 5u8,
                    4 => $trace.slot3.kind == 5u8,
                    5 => $trace.slot4.kind == 5u8,
                    6 => $trace.slot5.kind == 5u8,
                    7 => $trace.slot6.kind == 5u8,
                    8 => $trace.slot7.kind == 5u8,
                    _ => false,
                })
            // A persistence request is a fence: the same transition cannot
            // sign, apply, enter a view, or advance the body pipeline.
            && ($persist_count == 0u64
                || ($sign_count == 0u64
                    && $apply_count == 0u64
                    && $enter_count == 0u64
                    && $store_count == 0u64
                    && $validate_count == 0u64))
            // Body storage and validation acknowledgements are serialized
            // one-effect transitions in their exact pipeline order. A
            // retransmission tick may reconstruct the one lost Decision-stage
            // owner after retransmitting retained control messages; in that
            // case the reconstructed body effect is last and cannot coexist
            // with another body-stage, fetch, or report effect.
            && ($store_count == 0u64
                || (($trace.len == 1u8 && $event_kind == 8u8)
                    || $event_kind == 7u8))
            && ($validate_count == 0u64
                || (($trace.len == 1u8 && $event_kind == 9u8)
                    || $event_kind == 7u8))
            && ($enter_count == 0u64
                || ($persist_count == 0u64
                    && $apply_count == 0u64
                    && $store_count == 0u64
                    && $validate_count == 0u64))
            && ($apply_count == 0u64
                || ($persist_count == 0u64
                    && $enter_count == 0u64
                    && $store_count == 0u64
                    && $validate_count == 0u64))
            // `fetch_count` is intentionally not bounded: retransmission may
            // evolve to request several equivalent certified sources in other
            // transitions, but all exact slots still require concrete
            // authorization above. The current Decision retransmission path is
            // deliberately narrower: retained broadcasts followed by at most
            // one exact pipeline-stage owner.
            && $fetch_count <= 8u64
            && ($event_kind != 7u8
                || (effect_count_body!($trace, 9u8) == 0u64
                    && (($fetch_count == 0u64
                        && $store_count == 0u64
                        && $validate_count == 0u64
                        && $apply_count == 0u64)
                        || ($fetch_count == 1u64
                            && $store_count == 0u64
                            && $validate_count == 0u64
                            && $apply_count == 0u64
                            && final_effect_kind_body!($trace, 2u8))
                        || ($fetch_count == 0u64
                            && $store_count == 1u64
                            && $validate_count == 0u64
                            && $apply_count == 0u64
                            && final_effect_kind_body!($trace, 3u8))
                        || ($fetch_count == 0u64
                            && $store_count == 0u64
                            && $validate_count == 1u64
                            && $apply_count == 0u64
                            && final_effect_kind_body!($trace, 4u8))
                        || ($fetch_count == 0u64
                            && $store_count == 0u64
                            && $validate_count == 0u64
                            && $apply_count == 1u64
                            && final_effect_kind_body!($trace, 7u8)))))
    }};
}

macro_rules! effect_ordering_gate_body {
    ($trace:expr, $event_kind:expr, $count_gate:ident, $constraints_gate:ident $(,)?) => {{
        let persist_count = $count_gate($trace, 1u8);
        let fetch_count = $count_gate($trace, 2u8);
        let store_count = $count_gate($trace, 3u8);
        let validate_count = $count_gate($trace, 4u8);
        let sign_count = $count_gate($trace, 5u8);
        let apply_count = $count_gate($trace, 7u8);
        let enter_count = $count_gate($trace, 8u8);
        $constraints_gate(
            $trace,
            $event_kind,
            persist_count,
            fetch_count,
            store_count,
            validate_count,
            sign_count,
            apply_count,
            enter_count,
        )
    }};
}

macro_rules! effect_trace_gate_body {
    ($trace:expr, $event_kind:expr, $slot_gate:ident, $order_gate:ident $(,)?) => {{ $slot_gate($trace) && $order_gate($trace, $event_kind) }};
}

macro_rules! volatile_summary_well_formed_body {
    ($summary:expr, $validator_count:expr) => {{
        $validator_count > 0u64
            && $validator_count <= u64::MAX / 2u64
            // At most two active phase pools are kept: current Prepare plus
            // either its same-round Commit or a durable same-round Commit
            // retained from an older lock round. A newly durable lock retires
            // the superseded older pool before its Commit signature completes.
            && $summary.vote_pools <= 2u64
            && $summary.vote_entries >= $summary.vote_pools
            && $summary.vote_entries <= $validator_count * 2u64
            // The current timeout pool plus exactly one adjacent future pool
            // are retained so staggered honest validators can form the TC
            // which resynchronizes the pacemaker.
            && $summary.timeout_vote_pools <= 2u64
            && $summary.timeout_vote_entries >= $summary.timeout_vote_pools
            && $summary.timeout_vote_entries <= $validator_count * 2u64
            // At most one locally formed certificate per phase and one TC per
            // retained timeout round.
            && $summary.formed_certificates <= 2u64
            && $summary.formed_timeouts <= 2u64
            // `OutboundControlClass` has seven exhaustive variants.
            && $summary.outbound_control <= 7u64
            // Every pending PrepareQC is also known.  Recovery and a view
            // reset may additionally retain highest and locked (at most two).
            && $summary.pending_prepare <= $summary.known_prepare
            && $summary.known_prepare - $summary.pending_prepare <= 2u64
            // Body work is sourced by a candidate, a pending certified body,
            // or the sole durable decision.  Two spare identities cover the
            // candidate/decision cases without trusting subject equality.
            && ($summary.body_work <= $summary.pending_prepare
                || $summary.body_work - $summary.pending_prepare <= 2u64)
            // The FIFO plus its sole in-flight element is bounded by durable
            // intents eligible for replay; no unsigned item may be invented.
            && $summary.signature_queue <= $summary.durable_signable_limit
            && (!$summary.awaiting_signature
                || $summary.signature_queue < $summary.durable_signable_limit)
    }};
}

macro_rules! volatile_summaries_equal_body {
    ($before:expr, $after:expr) => {{
        $before.candidate_present == $after.candidate_present
            && $before.body_work == $after.body_work
            && $before.pending_prepare == $after.pending_prepare
            && $before.known_prepare == $after.known_prepare
            && $before.vote_pools == $after.vote_pools
            && $before.vote_entries == $after.vote_entries
            && $before.timeout_vote_pools == $after.timeout_vote_pools
            && $before.timeout_vote_entries == $after.timeout_vote_entries
            && $before.formed_certificates == $after.formed_certificates
            && $before.formed_timeouts == $after.formed_timeouts
            && $before.outbound_control == $after.outbound_control
            && $before.signature_queue == $after.signature_queue
            && $before.awaiting_signature == $after.awaiting_signature
            && $before.durable_signable_limit == $after.durable_signable_limit
            && $before.replay_resumed == $after.replay_resumed
    }};
}

macro_rules! safety_projection_accepts_body {
    ($safety:expr) => {{
        $safety.durable_identity_mismatches == 0u64
            && $safety.asynchronous_fence_conflicts == 0u64
            && $safety.invalid_highest_prepare == 0u64
            && $safety.invalid_lock == 0u64
            && $safety.invalid_timeout == 0u64
            && $safety.invalid_decision == 0u64
            && $safety.invalid_pending_append == 0u64
            && $safety.unauthorized_signables == 0u64
            && $safety.invalid_application == 0u64
    }};
}

macro_rules! validator_projection_equal_body {
    ($left:expr, $right:expr) => {{ $left.present == $right.present && (!$left.present || $left.id == $right.id) }};
}

macro_rules! subject_projection_equal_body {
    ($left:expr, $right:expr) => {{ $left.present == $right.present && (!$left.present || $left.subject == $right.subject) }};
}

macro_rules! replay_plan_slot_well_formed_body {
    ($slot:expr, $active:expr) => {{
        if $active {
            $slot.kind >= 1u8
                && $slot.kind <= 5u8
                && $slot.capability.kind == if $slot.kind == 5u8 { 2u8 } else { 5u8 }
        } else {
            $slot.kind == 0u8 && $slot.capability.kind == 0u8
        }
    }};
}

macro_rules! replay_plan_well_formed_body {
    ($plan:expr, $effect_kind:expr) => {{
        $plan.len <= 3u8
            && replay_plan_slot_well_formed_body!($plan.slot0, $plan.len > 0u8)
            && replay_plan_slot_well_formed_body!($plan.slot1, $plan.len > 1u8)
            && replay_plan_slot_well_formed_body!($plan.slot2, $plan.len > 2u8)
            && (if $plan.len == 0u8 {
                $effect_kind == 0u8
            } else {
                $effect_kind == $plan.slot0.kind
            })
    }};
}

macro_rules! replay_plan_equal_body {
    ($left:expr, $right:expr) => {{
        $left.len == $right.len
            && $left.slot0.kind == $right.slot0.kind
            && capability_key_equal_body!($left.slot0.capability, $right.slot0.capability)
            && $left.slot1.kind == $right.slot1.kind
            && capability_key_equal_body!($left.slot1.capability, $right.slot1.capability)
            && $left.slot2.kind == $right.slot2.kind
            && capability_key_equal_body!($left.slot2.capability, $right.slot2.capability)
    }};
}

macro_rules! boundary_capability_equal_body {
    ($left:expr, $right:expr) => {{
        $left.kind == $right.kind
            && $left.record_kind == $right.record_kind
            && $left.continuation == $right.continuation
            && $left.replay_effect_kind == $right.replay_effect_kind
            && $left.persistence_id == $right.persistence_id
            && $left.context_id == $right.context_id
            && canonical_identity_equal_body!($left.context_identity, $right.context_identity)
            && $left.tag.height == $right.tag.height
            && $left.tag.view == $right.tag.view
            && $left.tag.generation == $right.tag.generation
            && subject_projection_equal_body!($left.subject, $right.subject)
            && canonical_identity_equal_body!($left.subject_identity, $right.subject_identity)
            && $left.proposal_present == $right.proposal_present
            && $left.proposal_height == $right.proposal_height
            && $left.proposal_view == $right.proposal_view
            && $left.auxiliary_present == $right.auxiliary_present
            && $left.auxiliary_context_id == $right.auxiliary_context_id
            && $left.auxiliary_height == $right.auxiliary_height
            && $left.auxiliary_view == $right.auxiliary_view
            && $left.auxiliary_proposal_height == $right.auxiliary_proposal_height
            && $left.auxiliary_proposal_view == $right.auxiliary_proposal_view
            && $left.auxiliary_phase == $right.auxiliary_phase
            && $left.auxiliary_subject == $right.auxiliary_subject
            && replay_plan_well_formed_body!($left.replay_plan, $left.replay_effect_kind)
            && replay_plan_well_formed_body!($right.replay_plan, $right.replay_effect_kind)
            && replay_plan_equal_body!($left.replay_plan, $right.replay_plan)
    }};
}

macro_rules! boundary_capability_is_absent_body {
    ($boundary:expr) => {{
        $boundary.kind == refinement_tag_value!(BOUNDARY_NONE)
            && $boundary.record_kind == refinement_tag_value!(WAL_RECORD_NONE)
            && $boundary.continuation == refinement_tag_value!(CONTINUATION_NONE)
            && $boundary.replay_effect_kind == refinement_tag_value!(REPLAY_EFFECT_NONE)
            && $boundary.persistence_id == 0u64
            && canonical_identity_is_zero_body!($boundary.context_identity)
            && $boundary.tag.height == 0u64
            && $boundary.tag.view == 0u64
            && $boundary.tag.generation == 0u64
            && !$boundary.subject.present
            && canonical_identity_is_zero_body!($boundary.subject_identity)
            && !$boundary.proposal_present
            && $boundary.proposal_height == 0u64
            && $boundary.proposal_view == 0u64
            && !$boundary.auxiliary_present
            && $boundary.replay_plan.len == 0u8
            && replay_plan_well_formed_body!(
                $boundary.replay_plan,
                refinement_tag_value!(REPLAY_EFFECT_NONE)
            )
    }};
}

macro_rules! boundary_identity_is_canonical_body {
    ($boundary:expr) => {{
        canonical_identity_is_typed_body!(
            $boundary.context_identity,
            refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
            refinement_tag_value!(IDENTITY_KIND_CONSENSUS_CONTEXT)
        ) && (if $boundary.subject.present {
            canonical_identity_is_typed_body!(
                $boundary.subject_identity,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_CONSENSUS_SUBJECT)
            )
        } else {
            canonical_identity_is_zero_body!($boundary.subject_identity)
        }) && (if $boundary.proposal_present {
            $boundary.proposal_height > 0u64
        } else {
            $boundary.proposal_height == 0u64 && $boundary.proposal_view == 0u64
        }) && (if $boundary.auxiliary_present {
            $boundary.auxiliary_height > 0u64
                && ($boundary.auxiliary_phase == 1u8 || $boundary.auxiliary_phase == 2u8)
        } else {
            true
        })
    }};
}

macro_rules! certificate_identity_is_canonical_body {
    ($certificate:expr) => {{
        if !$certificate.present {
            canonical_identity_is_zero_body!($certificate.context_id)
                && $certificate.height == 0u64
                && $certificate.view == 0u64
                && $certificate.phase == 0u8
                && canonical_identity_is_zero_body!($certificate.subject)
                && $certificate.signer_bitmap == 0u128
                && $certificate.signer_bitmap_count == 0u64
                && $certificate.signer_count == 0u64
                && $certificate.voting_power == 0u64
                && $certificate.evidence_class == refinement_tag_value!(CERTIFICATE_EVIDENCE_ABSENT)
        } else {
            canonical_identity_is_typed_body!(
                $certificate.context_id,
                refinement_tag_value!(IDENTITY_DOMAIN_CONTEXT),
                refinement_tag_value!(IDENTITY_KIND_CONSENSUS_CONTEXT)
            ) && canonical_identity_is_typed_body!(
                $certificate.subject,
                refinement_tag_value!(IDENTITY_DOMAIN_SUBJECT),
                refinement_tag_value!(IDENTITY_KIND_CONSENSUS_SUBJECT)
            ) && $certificate.signer_bitmap != 0u128
                && $certificate.signer_bitmap_count > 0u64
                && $certificate.signer_bitmap_count == $certificate.signer_count
                && $certificate.signer_count > 0u64
                && $certificate.signer_count <= 128u64
                && $certificate.voting_power > 0u64
                && ($certificate.evidence_class
                    == refinement_tag_value!(CERTIFICATE_EVIDENCE_LOCAL)
                    || $certificate.evidence_class
                        == refinement_tag_value!(CERTIFICATE_EVIDENCE_INCOMING))
        }
    }};
}

macro_rules! certificate_identity_equal_body {
    ($left:expr, $right:expr) => {{
        certificate_identity_is_canonical_body!($left)
            && certificate_identity_is_canonical_body!($right)
            && $left.present == $right.present
            && (!$left.present
                || (canonical_identity_equal_body!($left.context_id, $right.context_id)
                    && $left.height == $right.height
                    && $left.view == $right.view
                    && $left.phase == $right.phase
                    && canonical_identity_equal_body!($left.subject, $right.subject)
                    && $left.signer_bitmap == $right.signer_bitmap
                    && $left.signer_bitmap_count == $right.signer_bitmap_count
                    && $left.signer_count == $right.signer_count
                    && $left.voting_power == $right.voting_power
                    && $left.evidence_class == $right.evidence_class))
    }};
}

// Compare the complete fixed-width certificate material while deliberately
// ignoring `evidence_class`. The same concrete incoming certificate can be
// labelled INCOMING in the lock projection and LOCAL in the high-QC
// projection when it is also the pre-install durable high. All signer,
// quorum, phase, context, and subject fields must still agree.
macro_rules! certificate_identity_same_material_body {
    ($left:expr, $right:expr) => {{
        certificate_identity_is_canonical_body!($left)
            && certificate_identity_is_canonical_body!($right)
            && $left.present == $right.present
            && (!$left.present
                || (canonical_identity_equal_body!($left.context_id, $right.context_id)
                    && $left.height == $right.height
                    && $left.view == $right.view
                    && $left.phase == $right.phase
                    && canonical_identity_equal_body!($left.subject, $right.subject)
                    && $left.signer_bitmap == $right.signer_bitmap
                    && $left.signer_bitmap_count == $right.signer_bitmap_count
                    && $left.signer_count == $right.signer_count
                    && $left.voting_power == $right.voting_power))
    }};
}

macro_rules! timeout_identity_equal_body {
    ($left:expr, $right:expr) => {{
        certificate_identity_is_canonical_body!($left.highest_prepare)
            && certificate_identity_is_canonical_body!($right.highest_prepare)
            && $left.present == $right.present
            && ($left.present || !$left.highest_prepare.present)
            && ($right.present || !$right.highest_prepare.present)
            && (!$left.present
                || (canonical_identity_equal_body!($left.context_id, $right.context_id)
                    && $left.height == $right.height
                    && $left.view == $right.view
                    && certificate_identity_equal_body!(
                        $left.highest_prepare,
                        $right.highest_prepare
                    )))
    }};
}

macro_rules! prepare_identity_in_context_body {
    ($certificate:expr, $context_id:expr, $height:expr, $maximum_view:expr) => {{
        certificate_identity_is_canonical_body!($certificate)
            && (!$certificate.present
                || (canonical_identity_equal_body!($certificate.context_id, $context_id)
                    && $certificate.height == $height
                    && $certificate.phase == 1u8
                    && $certificate.view <= $maximum_view))
    }};
}

// Preserve the reducer's one exact durable-high PrepareQC control owner across
// the post-WAL InstallTimeout seam. Highest and lock are intentionally
// separate: an observed PrepareQC may be strictly above the current lock.
// Every compared position is projected from a concrete certificate, so
// deleting the reseed, retaining a stale QC, or substituting equal-reference
// evidence fails this shared production/Verus relation.
macro_rules! enter_view_high_prepare_qc_control_identity_body {
    ($projection:expr) => {{
        let timeout_high = $projection.pending_record_timeout.highest_prepare;
        let local = $projection.local_highest_before;
        let incoming = $projection.incoming_highest_for_control;
        let selected = if !local.present {
            incoming
        } else if !incoming.present || incoming.view <= local.view {
            local
        } else {
            incoming
        };
        certificate_identity_is_canonical_body!(local)
            && (!local.present
                || local.evidence_class == refinement_tag_value!(CERTIFICATE_EVIDENCE_LOCAL))
            && certificate_identity_same_material_body!(incoming, timeout_high)
            && (!incoming.present
                || incoming.evidence_class == refinement_tag_value!(CERTIFICATE_EVIDENCE_INCOMING)
                || (incoming.evidence_class == refinement_tag_value!(CERTIFICATE_EVIDENCE_LOCAL)
                    && local.present
                    && certificate_identity_same_material_body!(incoming, local)))
            && certificate_identity_equal_body!($projection.durable_highest_after, selected)
            && $projection.prepare_control_slot_present_after == selected.present
            && certificate_identity_equal_body!(
                $projection.retained_prepare_qc_after,
                $projection.durable_highest_after
            )
    }};
}

// Preserve every PrepareQC position through the post-WAL EnterView seam. The
// local and incoming anchors are independently authenticated by the concrete
// reducer. Their evidence classes are then copied through pending,
// continuation, durable, effect, and immediate-fetch positions. In
// particular, no FOREIGN class is canonical and no equality can omit signer
// set, quorum totals, phase, or full concrete evidence identity.
macro_rules! enter_view_locked_prepare_qc_identity_body {
    ($projection:expr) => {{
        let timeout = $projection.pending_record_timeout;
        let local = $projection.local_lock_before;
        let incoming = timeout.highest_prepare;
        let selected = if !local.present {
            incoming
        } else if !incoming.present || incoming.view <= local.view {
            local
        } else {
            incoming
        };
        certificate_identity_is_canonical_body!(local)
            && (!local.present
                || local.evidence_class == refinement_tag_value!(CERTIFICATE_EVIDENCE_LOCAL))
            && certificate_identity_is_canonical_body!(incoming)
            && timeout_identity_equal_body!(timeout, $projection.pending_continuation_timeout)
            && timeout_identity_equal_body!(timeout, $projection.durable_timeout_after)
            && timeout_identity_equal_body!(timeout, $projection.effect_timeout)
            && certificate_identity_equal_body!($projection.durable_lock_after, selected)
            && certificate_identity_equal_body!(
                $projection.effect_protected_lock,
                $projection.durable_lock_after
            )
            && (if selected.present {
                certificate_identity_equal_body!($projection.following_fetch_lock, selected)
            } else {
                certificate_identity_is_canonical_body!($projection.following_fetch_lock)
                    && !$projection.following_fetch_lock.present
            })
    }};
}

// This expression is instantiated both by the concrete reducer and Verus. It
// describes the exact post-install lock choice, without claiming anything
// about asynchronous executor ownership or temporal fairness.
macro_rules! enter_view_projection_gate_body {
    ($projection:expr) => {{
        if !$projection.active {
            $projection.enter_count == 0u64
        } else {
            let timeout = $projection.pending_record_timeout;
            let local = $projection.local_lock_before;
            let incoming = timeout.highest_prepare;
            let selected = if !local.present {
                incoming
            } else if !incoming.present || incoming.view <= local.view {
                local
            } else {
                incoming
            };
            $projection.enter_count == 1u64
                && enter_view_locked_prepare_qc_identity_body!($projection)
                && enter_view_high_prepare_qc_control_identity_body!($projection)
                && $projection.pending_record_kind == 6u8
                && $projection.pending_continuation == 2u8
                && timeout.present
                && canonical_identity_equal_body!(timeout.context_id, $projection.context_id)
                && timeout.height == $projection.before_tag.height
                && $projection.after_tag.height == $projection.before_tag.height
                && timeout.view < u64::MAX
                && $projection.after_tag.view == timeout.view + 1u64
                // The exact WAL-application boundary separately invokes the
                // shared strict-upgrade kernel. This effect relation therefore
                // checks only the resulting monotonic view, never a second
                // transcription of the admission predicate.
                && $projection.before_tag.view <= $projection.after_tag.view
                && tag_projection_strictly_advances_body!(
                    $projection.after_tag,
                    $projection.before_tag
                )
                && (if $projection.after_tag.view == $projection.before_tag.view {
                    $projection.before_tag.generation < u64::MAX
                        && $projection.after_tag.generation
                            == $projection.before_tag.generation + 1u64
                } else {
                    $projection.after_tag.generation == 0u64
                })
                && prepare_identity_in_context_body!(
                    local,
                    $projection.context_id,
                    $projection.before_tag.height,
                    $projection.before_tag.view
                )
                && prepare_identity_in_context_body!(
                    incoming,
                    $projection.context_id,
                    $projection.before_tag.height,
                    timeout.view
                )
                && prepare_identity_in_context_body!(
                    $projection.local_highest_before,
                    $projection.context_id,
                    $projection.before_tag.height,
                    $projection.before_tag.view
                )
                && prepare_identity_in_context_body!(
                    $projection.incoming_highest_for_control,
                    $projection.context_id,
                    $projection.before_tag.height,
                    timeout.view
                )
                && (!(local.present && incoming.present && local.view == incoming.view)
                    || canonical_identity_equal_body!(local.subject, incoming.subject))
                && (if selected.present {
                    $projection.fetch_count == 1u64
                        && $projection.enter_index < 254u8
                        && $projection.following_fetch_index == $projection.enter_index + 1u8
                } else {
                    $projection.fetch_count == 0u64 && !$projection.following_fetch_lock.present
                })
        }
    }};
}

// Derive the durable, volatile, capability, and effect portion of the checked
// transition facts. Production and Verus instantiate this exact expression
// with their corresponding primitive projection types.
macro_rules! transition_delta_facts_from_projection_body {
    ($projection:expr, $delta_type:ident) => {{
        let boundary_exact = $projection.boundary_claimed.kind != 0u8
            && boundary_capability_equal_body!(
                $projection.boundary_claimed,
                $projection.boundary_granted
            );
        let begin_persist_exact = boundary_exact && $projection.boundary_claimed.kind == 1u8;
        let acknowledge_persist_exact = boundary_exact && $projection.boundary_claimed.kind == 2u8;
        let application_boundary_exact = boundary_exact && $projection.boundary_claimed.kind == 3u8;
        let replay_boundary_exact = boundary_exact && $projection.boundary_claimed.kind == 4u8;
        let durable_unchanged = $projection.durable_before == $projection.durable_after;
        let pending_unchanged = $projection.pending_state_before == $projection.pending_state_after;
        let generation_unchanged = $projection.generation_before == $projection.generation_after;
        let application_unchanged = subject_projection_equal_body!(
            $projection.application_before,
            $projection.application_after
        );
        let acknowledgement_continuation = if acknowledge_persist_exact {
            $projection.boundary_claimed.continuation
        } else {
            0u8
        };
        let install_view_unchanged = acknowledge_persist_exact
            && acknowledgement_continuation == refinement_tag_value!(CONTINUATION_INSTALL_TIMEOUT)
            && $projection.enter_view.active
            && $projection.enter_view.before_tag.view == $projection.enter_view.after_tag.view;
        let timeout_vote_pool_unchanged =
            $projection.timeout_votes_before == $projection.timeout_votes_after;
        let formed_timeouts_unchanged =
            $projection.formed_timeouts_before == $projection.formed_timeouts_after;
        let timeout_evidence_after_in_installed_window =
            $projection.timeout_evidence_after_outside_installed_window == 0u64;
        let timeout_control_unchanged =
            $projection.timeout_control_before == $projection.timeout_control_after;
        let timeout_control_after_absent = $projection.timeout_control_after.is_none();
        $delta_type {
            volatile_before: $projection.volatile_before,
            volatile_after: $projection.volatile_after,
            durable_unchanged,
            pending_unchanged,
            generation_unchanged,
            application_unchanged,
            begin_persist_exact,
            acknowledge_persist_exact,
            application_transition_exact: application_unchanged || application_boundary_exact,
            acknowledgement_continuation,
            install_view_unchanged,
            timeout_vote_pool_unchanged,
            formed_timeouts_unchanged,
            timeout_evidence_after_in_installed_window,
            timeout_control_unchanged,
            timeout_control_after_absent,
            replay_boundary_exact,
            effects: $projection.effects,
        }
    }};
}

// Derive invariant, fence, and action-classification fields from primitive
// state plus the exact delta facts above.
macro_rules! transition_classification_facts_from_projection_body {
    ($projection:expr, $delta:expr, $classification_type:ident) => {{
        let state_unchanged = $projection.before_state == $projection.after_state;
        let action_kind = if $delta.begin_persist_exact {
            1u8
        } else if $delta.acknowledge_persist_exact {
            2u8
        } else if state_unchanged && $projection.effects.len == 0u8 {
            0u8
        } else if !$delta.application_unchanged {
            5u8
        } else if $delta.replay_boundary_exact {
            6u8
        } else if $projection.event_kind >= 8u8 && $projection.event_kind <= 10u8 {
            3u8
        } else {
            4u8
        };
        let replay_duplicate = $projection.replay_before && $projection.event_kind == 15u8;
        let recovery_fence_open = $projection.replay_before || $projection.event_kind == 15u8;
        let pending_completion = $projection.event_kind == refinement_tag_value!(EVENT_PERSISTED)
            || $projection.event_kind == refinement_tag_value!(EVENT_PERSISTENCE_FAILED);
        let signing_completion = $projection.event_kind == 13u8;
        let pending_present = $projection.pending_before.record_kind != 0u8;
        let busy_fence_open = recovery_fence_open
            && (!pending_present || pending_completion || replay_duplicate)
            && (!$projection.awaiting_before || signing_completion || replay_duplicate);
        let tag_matches = $projection.event_tag.height == $projection.height_before
            && $projection.event_tag.view == $projection.view_before
            && $projection.event_tag.generation == $projection.generation_before;
        let wal_record_kind = if $delta.begin_persist_exact || $delta.acknowledge_persist_exact {
            $projection.boundary_claimed.record_kind
        } else {
            0u8
        };
        let signed_message_kind = if action_kind == 0u8 || $projection.event_kind != 13u8 {
            0u8
        } else {
            $projection.awaiting_message_kind
        };
        let replay_effect_kind = if $delta.replay_boundary_exact {
            $projection.boundary_claimed.replay_effect_kind
        } else {
            0u8
        };
        $classification_type {
            before_invariant: safety_projection_accepts_body!($projection.safety_before),
            after_invariant: safety_projection_accepts_body!($projection.safety_after),
            context_unchanged: $projection.context_before == $projection.context_after
                && validator_projection_equal_body!(
                    $projection.local_before,
                    $projection.local_after
                ),
            whole_state_unchanged: state_unchanged,
            tag_matches,
            busy_fence_open,
            event_kind: $projection.event_kind,
            action_kind,
            wal_record_kind,
            signed_message_kind,
            replay_effect_kind,
            validator_count: $projection.validator_count,
        }
    }};
}

// Compose independently derived facts without recomputing any predicate.
macro_rules! transition_facts_from_components_body {
    ($classification:expr, $delta:expr, $enter_view_exact:expr, $facts_type:ident) => {{
        $facts_type {
            before_invariant: $classification.before_invariant,
            after_invariant: $classification.after_invariant,
            context_unchanged: $classification.context_unchanged,
            whole_state_unchanged: $classification.whole_state_unchanged,
            tag_matches: $classification.tag_matches,
            busy_fence_open: $classification.busy_fence_open,
            event_kind: $classification.event_kind,
            action_kind: $classification.action_kind,
            wal_record_kind: $classification.wal_record_kind,
            signed_message_kind: $classification.signed_message_kind,
            replay_effect_kind: $classification.replay_effect_kind,
            validator_count: $classification.validator_count,
            volatile_before: $delta.volatile_before,
            volatile_after: $delta.volatile_after,
            durable_unchanged: $delta.durable_unchanged,
            pending_unchanged: $delta.pending_unchanged,
            generation_unchanged: $delta.generation_unchanged,
            application_unchanged: $delta.application_unchanged,
            begin_persist_exact: $delta.begin_persist_exact,
            acknowledge_persist_exact: $delta.acknowledge_persist_exact,
            application_transition_exact: $delta.application_transition_exact,
            acknowledgement_continuation: $delta.acknowledgement_continuation,
            install_view_unchanged: $delta.install_view_unchanged,
            timeout_vote_pool_unchanged: $delta.timeout_vote_pool_unchanged,
            formed_timeouts_unchanged: $delta.formed_timeouts_unchanged,
            timeout_evidence_after_in_installed_window: $delta
                .timeout_evidence_after_in_installed_window,
            timeout_control_unchanged: $delta.timeout_control_unchanged,
            timeout_control_after_absent: $delta.timeout_control_after_absent,
            enter_view_exact: $enter_view_exact,
            effects: $delta.effects,
        }
    }};
}

// Derive every safety/action fact consumed by the production relation from
// concrete primitive state, boundary, and capability projections.
macro_rules! transition_facts_from_projection_body {
    (
        $projection:expr,
        $facts_type:ident,
        $classification_type:ident,
        $delta_type:ident $(,)?
    ) => {{
        let delta = transition_delta_facts_from_projection_body!($projection, $delta_type);
        let classification = transition_classification_facts_from_projection_body!(
            $projection,
            delta,
            $classification_type
        );
        let enter_view_exact = enter_view_projection_gate_body!($projection.enter_view)
            && $projection.enter_view.enter_count == effect_count_body!($projection.effects, 8u8)
            && $projection.enter_view.fetch_count == effect_count_body!($projection.effects, 2u8);
        transition_facts_from_components_body!(classification, delta, enter_view_exact, $facts_type)
    }};
}

macro_rules! volatile_replay_unchanged_body {
    ($before:expr, $after:expr) => {{
        $before.candidate_present == $after.candidate_present
            && $before.pending_prepare == $after.pending_prepare
            && $before.known_prepare == $after.known_prepare
            && $before.vote_pools == $after.vote_pools
            && $before.vote_entries == $after.vote_entries
            && $before.timeout_vote_pools == $after.timeout_vote_pools
            && $before.timeout_vote_entries == $after.timeout_vote_entries
            && $before.formed_certificates == $after.formed_certificates
            && $before.formed_timeouts == $after.formed_timeouts
            && $before.outbound_control == $after.outbound_control
            && $before.durable_signable_limit == $after.durable_signable_limit
    }};
}

macro_rules! acknowledgement_record_matches_body {
    ($record_kind:expr, $continuation:expr) => {{
        match $continuation {
            0 => $record_kind == 3u8,
            1 => matches!($record_kind, 1u8 | 2u8 | 4u8 | 5u8),
            2 => $record_kind == 6u8,
            3 => $record_kind == 7u8,
            _ => false,
        }
    }};
}

macro_rules! signed_message_class_body {
    ($facts:expr) => {{
        ($facts.signed_message_kind == 0u8
            && ($facts.event_kind != 13u8 || $facts.action_kind == 0u8))
            || ($facts.event_kind == 13u8
                && $facts.action_kind != 0u8
                && $facts.signed_message_kind >= 1u8
                && $facts.signed_message_kind <= 4u8)
    }};
}

macro_rules! stutter_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && $facts.application_unchanged
            && $facts.whole_state_unchanged
            && volatile_summaries_equal_body!($facts.volatile_before, $facts.volatile_after)
            && $facts.effects.len == 0u8
    }};
}

macro_rules! begin_wal_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind >= 1u8
            && $facts.wal_record_kind <= 7u8
            && $facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
    }};
}

macro_rules! acknowledge_wal_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind >= 1u8
            && $facts.wal_record_kind <= 7u8
            && !$facts.begin_persist_exact
            && $facts.acknowledge_persist_exact
            && $facts.event_kind == 11u8
            && acknowledgement_record_matches_body!(
                $facts.wal_record_kind,
                $facts.acknowledgement_continuation
            )
            && effect_count_body!($facts.effects, 1u8) == 0u64
            && effect_count_body!($facts.effects, 3u8) == 0u64
            && effect_count_body!($facts.effects, 4u8) == 0u64
    }};
}

macro_rules! validation_completed_action_body {
    ($facts:expr, $count_gate:ident $(,)?) => {{
        $count_gate($facts.effects, 1u8) == 0u64
            && $count_gate($facts.effects, 2u8) == 0u64
            && $count_gate($facts.effects, 3u8) == 0u64
            && $count_gate($facts.effects, 4u8) == 0u64
            && $count_gate($facts.effects, 5u8) == 0u64
            && $count_gate($facts.effects, 6u8) == 0u64
            && $count_gate($facts.effects, 8u8) == 0u64
    }};
}

macro_rules! body_progress_action_body {
    ($facts:expr, $validation_gate:ident $(,)?) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.event_kind >= 8u8
            && $facts.event_kind <= 10u8
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && $facts.application_unchanged
            && (match $facts.event_kind {
                8 => $facts.effects.len == 1u8 && $facts.effects.slot0.kind == 3u8,
                9 => $facts.effects.len == 1u8 && $facts.effects.slot0.kind == 4u8,
                10 => $validation_gate($facts),
                _ => false,
            })
    }};
}

macro_rules! volatile_protocol_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && $facts.application_unchanged
            && effect_count_body!($facts.effects, 1u8) == 0u64
            && ($facts.event_kind == 7u8 || effect_count_body!($facts.effects, 3u8) == 0u64)
            && ($facts.event_kind == 7u8 || effect_count_body!($facts.effects, 4u8) == 0u64)
            && effect_count_body!($facts.effects, 8u8) == 0u64
    }};
}

macro_rules! complete_application_action_body {
    ($facts:expr) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.event_kind == 14u8
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && !$facts.application_unchanged
            && $facts.application_transition_exact
            && $facts.effects.len == 0u8
    }};
}

macro_rules! resume_after_replay_action_body {
    ($facts:expr, $count_gate:ident $(,)?) => {{
        $facts.wal_record_kind == 0u8
            && !$facts.begin_persist_exact
            && !$facts.acknowledge_persist_exact
            && $facts.event_kind == 15u8
            && $facts.durable_unchanged
            && $facts.pending_unchanged
            && $facts.generation_unchanged
            && $facts.application_unchanged
            && !$facts.volatile_before.replay_resumed
            && $facts.volatile_after.replay_resumed
            && !$facts.volatile_before.awaiting_signature
            && $facts.volatile_before.signature_queue == 0u64
            && volatile_replay_unchanged_body!($facts.volatile_before, $facts.volatile_after)
            && $count_gate($facts.effects, 1u8) == 0u64
            && $count_gate($facts.effects, 3u8) == 0u64
            && $count_gate($facts.effects, 4u8) == 0u64
            && $count_gate($facts.effects, 6u8) == 0u64
            && $count_gate($facts.effects, 8u8) == 0u64
            && $count_gate($facts.effects, 9u8) == 0u64
            && match $facts.replay_effect_kind {
                0 => {
                    $facts.effects.len == 0u8
                        && $facts.volatile_after.body_work == $facts.volatile_before.body_work
                        && !$facts.volatile_after.awaiting_signature
                        && $facts.volatile_after.signature_queue == 0u64
                }
                1..=4 => {
                    $facts.effects.len == 1u8
                        && $facts.effects.slot0.kind == 5u8
                        && $facts.volatile_after.body_work == $facts.volatile_before.body_work
                        && $facts.volatile_after.awaiting_signature
                }
                5 => {
                    $facts.effects.len == 1u8
                        && $facts.effects.slot0.kind == 2u8
                        && $facts.volatile_before.body_work < u64::MAX
                        && $facts.volatile_after.body_work
                            == $facts.volatile_before.body_work + 1u64
                        && !$facts.volatile_after.awaiting_signature
                        && $facts.volatile_after.signature_queue == 0u64
                }
                _ => false,
            }
    }};
}

macro_rules! action_kind_relation_body {
    (
        $facts:expr,
        $stutter_gate:ident,
        $begin_wal_gate:ident,
        $acknowledge_wal_gate:ident,
        $body_progress_gate:ident,
        $volatile_protocol_gate:ident,
        $complete_application_gate:ident,
        $resume_after_replay_gate:ident $(,)?
    ) => {{
        match $facts.action_kind {
            0 => $stutter_gate($facts),
            1 => $begin_wal_gate($facts),
            2 => $acknowledge_wal_gate($facts),
            3 => $body_progress_gate($facts),
            4 => $volatile_protocol_gate($facts),
            5 => $complete_application_gate($facts),
            6 => $resume_after_replay_gate($facts),
            _ => false,
        }
    }};
}

macro_rules! production_action_relation_body {
    ($facts:expr, $signed_gate:ident, $action_gate:ident $(,)?) => {{
        $signed_gate($facts)
            && ($facts.action_kind == 6u8 || $facts.replay_effect_kind == 0u8)
            // A current, serviceable first ResumeAfterReplay event may not be
            // reclassified as an ordinary volatile action when its exact
            // boundary grant fails. Stale tags and duplicate resume events
            // remain ordinary empty stutters.
            && ($facts.event_kind != 15u8
                || !$facts.tag_matches
                || !$facts.busy_fence_open
                || (if $facts.volatile_before.replay_resumed {
                    $facts.action_kind == 0u8
                } else {
                    $facts.action_kind == 6u8
                }))
            && $action_gate($facts)
    }};
}

macro_rules! transition_branch_constraints_body {
    (
        $facts:expr,
        $persist_count:expr,
        $fetch_count:expr,
        $sign_count:expr,
        $apply_count:expr,
        $enter_count:expr $(,)?
    ) => {{
        if !$facts.tag_matches || !$facts.busy_fence_open {
            $facts.whole_state_unchanged
                && $facts.durable_unchanged
                && $facts.pending_unchanged
                && $facts.generation_unchanged
                && $facts.application_unchanged
                && volatile_summaries_equal_body!($facts.volatile_before, $facts.volatile_after)
                && $facts.effects.len == 0u8
                && !$facts.begin_persist_exact
                && !$facts.acknowledge_persist_exact
        } else if $facts.begin_persist_exact {
            !$facts.acknowledge_persist_exact
                && $facts.durable_unchanged
                && !$facts.pending_unchanged
                && $facts.generation_unchanged
                && $facts.application_unchanged
                && $persist_count == 1u64
                && $sign_count == 0u64
                && $apply_count == 0u64
                && $enter_count == 0u64
        } else if $facts.acknowledge_persist_exact {
            !$facts.begin_persist_exact
                && !$facts.pending_unchanged
                && $facts.application_unchanged
                && $persist_count == 0u64
                && (match $facts.acknowledgement_continuation {
                    0 => {
                        $facts.generation_unchanged && $apply_count == 0u64 && $enter_count == 0u64
                    }
                    1 => {
                        $facts.generation_unchanged
                            && $sign_count == 1u64
                            && $apply_count == 0u64
                            && $enter_count == 0u64
                    }
                    2 => {
                        (!$facts.install_view_unchanged || !$facts.generation_unchanged)
                            && $enter_count == 1u64
                            && $apply_count == 0u64
                            && !$facts.volatile_after.candidate_present
                            // A TC-selected lock carries its full PrepareQC.
                            // Installation therefore starts at most one
                            // certified body fetch in the successor view.
                            && $facts.volatile_after.body_work <= 1u64
                            && $fetch_count == $facts.volatile_after.body_work
                            && $facts.volatile_after.pending_prepare == 0u64
                            && $facts.volatile_after.vote_pools == 0u64
                            && $facts.volatile_after.vote_entries == 0u64
                            && (if $facts.install_view_unchanged {
                                $facts.timeout_vote_pool_unchanged
                                    && $facts.volatile_after.timeout_vote_pools
                                    == $facts.volatile_before.timeout_vote_pools
                                    && $facts.volatile_after.timeout_vote_entries
                                        == $facts.volatile_before.timeout_vote_entries
                            } else {
                                // A view advance retires stale shares but may
                                // preserve already authenticated shares for
                                // the installed view and its adjacent future
                                // catch-up round. A persistence acknowledgement
                                // cannot invent either pools or entries.
                                $facts.timeout_evidence_after_in_installed_window
                                    && $facts.volatile_after.timeout_vote_pools
                                    <= $facts.volatile_before.timeout_vote_pools
                                    && $facts.volatile_after.timeout_vote_entries
                                        <= $facts.volatile_before.timeout_vote_entries
                            })
                            && $facts.volatile_after.formed_certificates == 0u64
                            && (if $facts.install_view_unchanged {
                                $facts.formed_timeouts_unchanged
                                    && $facts.volatile_after.formed_timeouts
                                    == $facts.volatile_before.formed_timeouts
                            } else {
                                $facts.volatile_after.formed_timeouts
                                    <= $facts.volatile_before.formed_timeouts
                            })
                            && (if $facts.install_view_unchanged {
                                $facts.timeout_control_unchanged
                            } else {
                                $facts.timeout_control_after_absent
                            })
                            && $facts.volatile_after.known_prepare <= 2u64
                            // Install retains/reseeds one exact durable
                            // PrepareQC beside the CommitVote, installed TC,
                            // and (for a strict same-round upgrade) the
                            // current TimeoutVote.
                            && $facts.volatile_after.outbound_control <= 4u64
                    }
                    3 => {
                        $facts.generation_unchanged
                            && $sign_count == 0u64
                            && $enter_count == 0u64
                            // A durable Decision is the sole terminal owner.
                            // Its acknowledgement retires every speculative
                            // proposal, vote, timeout, and signature owner
                            // while preserving (or creating) exactly one body
                            // pipeline for the certified decision.
                            && !$facts.volatile_after.candidate_present
                            && $facts.volatile_after.body_work == 1u64
                            && $facts.volatile_after.pending_prepare == 0u64
                            && $facts.volatile_after.known_prepare == 0u64
                            && $facts.volatile_after.vote_pools == 0u64
                            && $facts.volatile_after.vote_entries == 0u64
                            && $facts.volatile_after.timeout_vote_pools == 0u64
                            && $facts.volatile_after.timeout_vote_entries == 0u64
                            && $facts.volatile_after.formed_certificates == 0u64
                            && $facts.volatile_after.formed_timeouts == 0u64
                            && $facts.volatile_after.outbound_control == 1u64
                            && $facts.volatile_after.signature_queue == 0u64
                            && !$facts.volatile_after.awaiting_signature
                            && $facts.volatile_after.durable_signable_limit == 0u64
                            && (($apply_count == 1u64 && $fetch_count == 0u64)
                                || ($apply_count == 0u64 && $fetch_count == 1u64)
                                // A CommitQC may arrive after the exact body
                                // has entered StoreBody or validation.  That
                                // exact generation-tagged entry remains the
                                // sole continuation after every competing
                                // entry is retired.
                                || ($apply_count == 0u64
                                    && $fetch_count == 0u64
                                    && $facts.volatile_before.body_work > 0u64
                                    && $facts.volatile_after.body_work
                                        <= $facts.volatile_before.body_work))
                    }
                    _ => false,
                })
        } else if $facts.action_kind == 6u8 {
            $facts.event_kind == 15u8
                && $facts.durable_unchanged
                && $facts.pending_unchanged
                && $facts.generation_unchanged
                && $facts.application_unchanged
                && $persist_count == 0u64
                && $apply_count == 0u64
                && $enter_count == 0u64
                && ($sign_count == 0u64 || $sign_count == 1u64)
                && ($fetch_count == 0u64 || $fetch_count == 1u64)
        } else {
            $facts.durable_unchanged
                && $facts.pending_unchanged
                && $facts.generation_unchanged
                && $facts.application_transition_exact
                && $persist_count == 0u64
                && $enter_count == 0u64
                && ($sign_count == 0u64 || $facts.event_kind == 13u8)
                // LocalProposalReady may recover Apply only when the concrete
                // capability grant binds its exact trusted manifest to the
                // durable decision.  The branch gate admits the event class;
                // requested/granted key equality rejects every non-exact use.
                && ($apply_count == 0u64
                    || $facts.event_kind == 0u8
                    || $facts.event_kind == 7u8
                    || $facts.event_kind == 10u8)
        }
    }};
}

macro_rules! transition_branch_gate_body {
    ($facts:expr, $count_gate:ident, $constraints_gate:ident $(,)?) => {{
        let persist_count = $count_gate($facts.effects, 1u8);
        let fetch_count = $count_gate($facts.effects, 2u8);
        let sign_count = $count_gate($facts.effects, 5u8);
        let apply_count = $count_gate($facts.effects, 7u8);
        let enter_count = $count_gate($facts.effects, 8u8);
        $constraints_gate(
            $facts,
            persist_count,
            fetch_count,
            sign_count,
            apply_count,
            enter_count,
        )
    }};
}

macro_rules! production_transition_gate_body {
    (
        $facts:expr,
        $volatile_gate:ident,
        $action_gate:ident,
        $trace_gate:ident,
        $branch_gate:ident $(,)?
    ) => {{
        $facts.before_invariant
            && $facts.after_invariant
            && $facts.context_unchanged
            && $volatile_gate($facts.volatile_before, $facts.validator_count)
            && $volatile_gate($facts.volatile_after, $facts.validator_count)
            && $action_gate($facts)
            && $trace_gate($facts.effects, $facts.event_kind)
            && $facts.enter_view_exact
            && $branch_gate($facts)
    }};
}

/// Validate the exact post-WAL effective-lock selection trace.
pub(crate) const fn production_enter_view_uses_post_install_effective_lock_kernel(
    trace: EffectiveLockTraceProjection,
    enter_view: EnterViewProjection,
) -> bool {
    effective_lock_trace_claim_body!(trace, 1u8)
        && enter_view_locked_prepare_qc_identity_body!(enter_view)
        && enter_view_high_prepare_qc_control_identity_body!(enter_view)
}

/// Validate monotonic exact body-pipeline ownership.
pub(crate) const fn production_body_ownership_preserves_effective_lock_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 2u8)
}

/// Validate exact byte/capacity retirement for superseded body ownership.
pub(crate) const fn production_body_capacity_retirement_preserves_effective_lock_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 3u8)
}

/// Validate one exact bounded fair-service selection.
pub(crate) const fn production_body_service_refines_async_fairness_kernel(
    projection: EffectiveLockTraceProjection,
) -> bool {
    effective_lock_trace_claim_body!(projection, 4u8)
}

/// Check the exact post-WAL effective-lock selection before reducer commit.
#[must_use]
pub(crate) fn check_production_enter_view_effective_lock_transition(
    trace: EffectiveLockTraceProjection,
    enter_view: EnterViewProjection,
) -> Option<CheckedProductionTransition<(EffectiveLockTraceProjection, EnterViewProjection)>> {
    if production_enter_view_uses_post_install_effective_lock_kernel(trace, enter_view) {
        Some(CheckedProductionTransition {
            projection: (trace, enter_view),
        })
    } else {
        None
    }
}

/// Check one exact body-pipeline ownership transition before map mutation.
#[must_use]
pub(crate) fn check_production_body_ownership_effective_lock_transition(
    projection: EffectiveLockTraceProjection,
) -> Option<CheckedProductionTransition<EffectiveLockTraceProjection>> {
    if production_body_ownership_preserves_effective_lock_kernel(projection) {
        Some(CheckedProductionTransition { projection })
    } else {
        None
    }
}

/// Check exact body-capacity retirement before retiring its live owners.
#[must_use]
pub(crate) fn check_production_body_capacity_retirement_effective_lock_transition(
    projection: EffectiveLockTraceProjection,
) -> Option<CheckedProductionTransition<EffectiveLockTraceProjection>> {
    if production_body_capacity_retirement_preserves_effective_lock_kernel(projection) {
        Some(CheckedProductionTransition { projection })
    } else {
        None
    }
}

/// Check one bounded fair-service selection before queue mutation.
#[must_use]
pub(crate) fn check_production_body_service_effective_lock_transition(
    projection: EffectiveLockTraceProjection,
) -> Option<CheckedProductionTransition<EffectiveLockTraceProjection>> {
    if production_body_service_refines_async_fairness_kernel(projection) {
        Some(CheckedProductionTransition { projection })
    } else {
        None
    }
}

/// Validate exact predecessor ownership returned by successor construction.
pub(crate) const fn production_durable_predecessor_identity_kernel(
    projection: ProductionDurablePredecessorIdentityProjection,
) -> bool {
    durable_predecessor_is_canonical_body!(projection)
}

/// Validate exact predecessor ownership returned by successor construction.
pub(crate) const fn production_successor_predecessor_binding_kernel(
    projection: ProductionSuccessorPredecessorBindingProjection,
) -> bool {
    production_successor_predecessor_binding_body!(projection)
}

/// Validate applied-predecessor activation through the exact prepared status.
pub(crate) const fn production_applied_successor_trace_refines_indexed_activation_kernel(
    projection: ProductionAppliedSuccessorTraceProjection,
) -> bool {
    production_applied_successor_trace_body!(projection)
}

/// Validate complete-tip or distinct snapshot recovery activation.
pub(crate) const fn production_recovered_successor_trace_refines_indexed_activation_kernel(
    projection: ProductionRecoveredSuccessorTraceProjection,
) -> bool {
    production_recovered_successor_trace_body!(projection)
}

/// Validate begin, fail-closed, and authenticated restart lifecycle steps.
pub(crate) const fn production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(
    projection: ProductionSuccessorStartupLifecycleProjection,
) -> bool {
    production_startup_failure_and_restart_trace_body!(projection)
}

/// Validate one authenticated, internally same-round historical CommitQC's
/// exact reducer admission. Historical refers only to the local reducer view.
pub(crate) const fn production_historical_certificate_trace_refines_indexed_async_kernel(
    projection: ProductionHistoricalCertificateTraceProjection,
) -> bool {
    production_historical_certificate_trace_body!(projection)
}

/// Validate one authenticated historical body entering its exact body pipeline.
pub(crate) const fn production_historical_body_pipeline_trace_refines_indexed_async_kernel(
    projection: ProductionHistoricalBodyPipelineTraceProjection,
) -> bool {
    production_historical_body_pipeline_trace_body!(projection)
}

/// Validate one reducer step's durable intent owner and WAL cursor movement.
pub(crate) fn production_durable_intent_trace_refines_progress_witness_kernel(
    projection: ProductionDurableIntentTraceProjection,
) -> bool {
    production_durable_intent_trace_body!(projection)
}

/// Validate startup reconstruction of one durable pending Decision.
pub(crate) const fn production_decision_trace_refines_recovery_witness_kernel(
    projection: ProductionDecisionRecoveryTraceProjection,
) -> bool {
    production_decision_recovery_trace_body!(projection)
}

/// Validate exact scheduler selection and the resulting FIFO debt owner.
pub(crate) const fn production_scheduler_trace_refines_protected_ownership_kernel(
    projection: ProductionSchedulerTraceProjection,
) -> bool {
    production_scheduler_trace_body!(projection)
}

/// Validate ingress identity preservation and its admitted service class.
pub(crate) const fn production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
    projection: ProductionIngressIdentityAndClassTraceProjection,
) -> bool {
    production_ingress_identity_and_class_trace_body!(projection)
}

/// Validate exact, occupancy-preserving materialization of one reserved
/// ingress owner.
pub(crate) const fn production_ingress_reservation_materialization_refines_protected_ownership_kernel(
    projection: ProductionIngressReservationMaterializationTraceProjection,
) -> bool {
    production_ingress_reservation_materialization_trace_body!(projection)
}

/// Validate one exact durable leader-wire admission before persistence.
pub(crate) const fn production_leader_wire_admission_refines_lifecycle_ownership_kernel(
    projection: ProductionLeaderWireAdmissionTraceProjection,
) -> bool {
    production_leader_wire_admission_trace_body!(projection)
}

/// Validate that an exact inner-ingress retry rotates both its authenticated
/// source and its item to the respective fair-service tails.
pub const fn production_two_stage_relay_retry_trace_refines_source_fairness_kernel(
    projection: ProductionTwoStageRelayRetryTraceProjection,
) -> bool {
    production_two_stage_relay_retry_trace_body!(projection)
}

/// Validate that sidecar delivery ownership moves only after exact flush.
pub(crate) const fn production_reliable_flush_trace_refines_outbound_ownership_kernel(
    projection: ProductionReliableFlushTraceProjection,
) -> bool {
    production_reliable_flush_trace_body!(projection)
}

/// Validate the exact one-shot lane mutation caused by a writer flush.
pub(crate) const fn production_reliable_flush_application_refines_source_lane_kernel(
    projection: ProductionReliableFlushApplicationProjection,
) -> bool {
    production_reliable_flush_application_body!(projection)
}

/// Validate that worker confirmation and lane application name one occurrence.
pub(crate) const fn production_reliable_flush_two_phase_link_kernel(
    worker: ProductionReliableFlushTraceProjection,
    application: ProductionReliableFlushApplicationProjection,
) -> bool {
    production_reliable_flush_two_phase_link_body!(worker, application)
}

/// Validate the exact durable application completion exposed to the reducer.
pub(crate) const fn production_application_trace_refines_decision_completion_kernel(
    projection: ProductionApplicationTraceProjection,
) -> bool {
    production_application_trace_body!(projection)
}

/// Validate one exact application handoff before successor construction.
pub(crate) const fn production_terminal_application_without_successor_activation_kernel(
    projection: ProductionTerminalApplicationWithoutSuccessorActivationProjection,
) -> bool {
    production_terminal_application_without_successor_activation_body!(projection)
}

/// Validate one exact primitive reservation-journal owner transition.
///
/// This kernel does not establish the surrounding QueuePlan, Kura, carrier,
/// WSV, FIFO, or lifecycle action-order obligations.
pub(crate) const fn production_in_flight_reservation_transition_kernel(
    projection: ProductionInFlightReservationTransitionProjection,
) -> bool {
    production_in_flight_reservation_transition_body!(projection)
}

/// Validate one complete bounded first-release carrier safety state.
pub(crate) const fn production_in_flight_first_release_state_kernel(
    projection: ProductionInFlightFirstReleaseStateProjection,
) -> bool {
    production_in_flight_first_release_state_body!(projection)
}

/// Validate one named action of the complete bounded first-release carrier.
pub(crate) const fn production_in_flight_first_release_transition_kernel(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
) -> bool {
    production_in_flight_first_release_transition_body!(projection)
}

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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition {
            projection: (worker, application),
        })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
        Some(CheckedProductionTransition { projection })
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
#[allow(dead_code)] // Awaiting concrete trace-extraction call sites; exercised by the harness.
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
#[allow(dead_code)] // Awaiting concrete trace-extraction call sites; exercised by the harness.
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
#[allow(dead_code)] // Awaiting startup lifecycle trace extraction; exercised by the harness.
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

#[cfg(test)]
include!("refinement_constructor_test_helpers.rs");

fn volatile_summary_is_well_formed(summary: VolatileSummary, validator_count: u64) -> bool {
    volatile_summary_well_formed_body!(summary, validator_count)
}

fn signed_message_class_is_valid(facts: TransitionFacts) -> bool {
    signed_message_class_body!(facts)
}

fn stutter_action_is_valid(facts: TransitionFacts) -> bool {
    stutter_action_body!(facts)
}

fn begin_wal_action_is_valid(facts: TransitionFacts) -> bool {
    begin_wal_action_body!(facts)
}

fn acknowledge_wal_action_is_valid(facts: TransitionFacts) -> bool {
    acknowledge_wal_action_body!(facts)
}

fn validation_completed_action_is_valid(facts: TransitionFacts) -> bool {
    validation_completed_action_body!(facts, effect_count)
}

fn body_progress_action_is_valid(facts: TransitionFacts) -> bool {
    body_progress_action_body!(facts, validation_completed_action_is_valid)
}

fn volatile_protocol_action_is_valid(facts: TransitionFacts) -> bool {
    volatile_protocol_action_body!(facts)
}

fn complete_application_action_is_valid(facts: TransitionFacts) -> bool {
    complete_application_action_body!(facts)
}

fn resume_after_replay_action_is_valid(facts: TransitionFacts) -> bool {
    resume_after_replay_action_body!(facts, effect_count)
}

fn action_kind_is_valid(facts: TransitionFacts) -> bool {
    action_kind_relation_body!(
        facts,
        stutter_action_is_valid,
        begin_wal_action_is_valid,
        acknowledge_wal_action_is_valid,
        body_progress_action_is_valid,
        volatile_protocol_action_is_valid,
        complete_application_action_is_valid,
        resume_after_replay_action_is_valid,
    )
}

fn named_action_is_valid(facts: TransitionFacts) -> bool {
    production_action_relation_body!(facts, signed_message_class_is_valid, action_kind_is_valid,)
}

fn effect_slots_are_authorized(trace: EffectTrace) -> bool {
    effect_slots_authorized_body!(trace)
}

fn effect_count(trace: EffectTrace, kind: u8) -> u64 {
    effect_count_body!(trace, kind)
}

#[allow(clippy::too_many_arguments)]
fn effect_order_constraints(
    trace: EffectTrace,
    event_kind: u8,
    persist_count: u64,
    fetch_count: u64,
    store_count: u64,
    validate_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> bool {
    effect_order_constraints_body!(
        trace,
        event_kind,
        persist_count,
        fetch_count,
        store_count,
        validate_count,
        sign_count,
        apply_count,
        enter_count,
    )
}

fn effect_order_is_valid(trace: EffectTrace, event_kind: u8) -> bool {
    effect_ordering_gate_body!(trace, event_kind, effect_count, effect_order_constraints)
}

fn effect_trace_accepts(trace: EffectTrace, event_kind: u8) -> bool {
    effect_trace_gate_body!(
        trace,
        event_kind,
        effect_slots_are_authorized,
        effect_order_is_valid,
    )
}

#[allow(clippy::too_many_arguments)]
fn transition_branch_constraints(
    facts: TransitionFacts,
    persist_count: u64,
    fetch_count: u64,
    sign_count: u64,
    apply_count: u64,
    enter_count: u64,
) -> bool {
    transition_branch_constraints_body!(
        facts,
        persist_count,
        fetch_count,
        sign_count,
        apply_count,
        enter_count,
    )
}

fn transition_branch_accepts(facts: TransitionFacts) -> bool {
    transition_branch_gate_body!(facts, effect_count, transition_branch_constraints)
}

fn accepts_facts(facts: TransitionFacts) -> bool {
    production_transition_gate_body!(
        facts,
        volatile_summary_is_well_formed,
        named_action_is_valid,
        effect_trace_accepts,
        transition_branch_accepts,
    )
}

/// Derive the complete checked relation from concrete primitive projections.
fn transition_facts(projection: TransitionProjection<'_>) -> TransitionFacts {
    transition_facts_from_projection_body!(
        projection,
        TransitionFacts,
        TransitionClassificationFacts,
        TransitionDeltaFacts,
    )
}

/// Predicate-level evidence for a transition rejected by [`accepts`].
///
/// This is diagnostic-only: every field is derived from the same primitive
/// projection and production predicates as the commit gate. It neither grants
/// capabilities nor participates in acceptance.
#[derive(Clone, Copy)]
pub(crate) struct TransitionDiagnostic {
    facts: TransitionFacts,
    safety_before: SafetyProjection,
    safety_after: SafetyProjection,
    volatile_before_well_formed: bool,
    volatile_after_well_formed: bool,
    signed_message_class_valid: bool,
    selected_action_valid: bool,
    effect_slots_authorized: bool,
    effect_order_valid: bool,
    transition_branch_valid: bool,
    enter_view_effective_lock_valid: bool,
}

impl fmt::Debug for TransitionDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransitionDiagnostic")
            .field("facts", &self.facts)
            .field("safety_before", &self.safety_before)
            .field("safety_after", &self.safety_after)
            .field(
                "volatile_before_well_formed",
                &self.volatile_before_well_formed,
            )
            .field(
                "volatile_after_well_formed",
                &self.volatile_after_well_formed,
            )
            .field(
                "signed_message_class_valid",
                &self.signed_message_class_valid,
            )
            .field("selected_action_valid", &self.selected_action_valid)
            .field("effect_slots_authorized", &self.effect_slots_authorized)
            .field("effect_order_valid", &self.effect_order_valid)
            .field("transition_branch_valid", &self.transition_branch_valid)
            .field(
                "enter_view_effective_lock_valid",
                &self.enter_view_effective_lock_valid,
            )
            .finish()
    }
}

/// Derive predicate-level diagnostics without weakening the production gate.
#[must_use]
pub(crate) fn diagnose(projection: TransitionProjection<'_>) -> TransitionDiagnostic {
    let facts = transition_facts(projection);
    let enter_view_effective_lock_valid = if projection.enter_view.active {
        let enter_view = projection.enter_view;
        let protected_after = u64::from(enter_view.durable_lock_after.present);
        let ownership_after = u64::from(enter_view.following_fetch_lock.present);
        let trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_ENTER_VIEW,
            relation_exact: enter_view_projection_gate_body!(enter_view)
                && enter_view.enter_count == effect_count_body!(projection.effects, 8u8)
                && enter_view.fetch_count == effect_count_body!(projection.effects, 2u8),
            protected_before: protected_after,
            protected_after: u64::from(enter_view.effect_protected_lock.present),
            owner_before: enter_view.fetch_count,
            owner_after: ownership_after,
            owner_reused: false,
            ready_before: 0,
            retired_retained: 0,
            retired_ready: 0,
            ready_after: 0,
            store_before: 0,
            retired_store: 0,
            store_after: 0,
            cursor_before: 0,
            completion_ready: false,
            progress_ready: false,
            normal_ready: false,
            selected: 0,
            cursor_after: 0,
        };
        production_enter_view_uses_post_install_effective_lock_kernel(trace, enter_view)
    } else {
        true
    };
    TransitionDiagnostic {
        facts,
        safety_before: projection.safety_before,
        safety_after: projection.safety_after,
        volatile_before_well_formed: volatile_summary_is_well_formed(
            facts.volatile_before,
            facts.validator_count,
        ),
        volatile_after_well_formed: volatile_summary_is_well_formed(
            facts.volatile_after,
            facts.validator_count,
        ),
        signed_message_class_valid: signed_message_class_is_valid(facts),
        selected_action_valid: action_kind_is_valid(facts),
        effect_slots_authorized: effect_slots_are_authorized(facts.effects),
        effect_order_valid: effect_order_is_valid(facts.effects, facts.event_kind),
        transition_branch_valid: transition_branch_accepts(facts),
        enter_view_effective_lock_valid,
    }
}

/// Execute the verified transition kernel used as the production commit gate.
///
/// No caller-provided authorization or action-exactness boolean crosses this
/// boundary.  The kernel derives them from requested/granted capability keys,
/// exact pre/post state identities, event tags, and invariant violation counts.
#[must_use = "checked reducer evidence must be consumed before installing candidate state"]
pub(crate) fn check(
    projection: TransitionProjection<'_>,
) -> Option<CheckedProductionTransition<TransitionProjection<'_>>> {
    if projection.enter_view.active {
        let enter_view = projection.enter_view;
        let protected_after = u64::from(enter_view.durable_lock_after.present);
        let ownership_after = u64::from(enter_view.following_fetch_lock.present);
        let trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_ENTER_VIEW,
            relation_exact: enter_view_projection_gate_body!(enter_view)
                && enter_view.enter_count == effect_count_body!(projection.effects, 8u8)
                && enter_view.fetch_count == effect_count_body!(projection.effects, 2u8),
            protected_before: protected_after,
            protected_after: u64::from(enter_view.effect_protected_lock.present),
            owner_before: enter_view.fetch_count,
            owner_after: ownership_after,
            owner_reused: false,
            ready_before: 0,
            retired_retained: 0,
            retired_ready: 0,
            ready_after: 0,
            store_before: 0,
            retired_store: 0,
            store_after: 0,
            cursor_before: 0,
            completion_ready: false,
            progress_ready: false,
            normal_ready: false,
            selected: 0,
            cursor_after: 0,
        };
        // The inner evidence establishes the Verus-checked effective-lock
        // claim. Consuming it here is safe because the returned outer token
        // retains the complete borrowed transition and is the only evidence
        // production can consume at the reducer's state-install boundary.
        let checked_effective_lock =
            check_production_enter_view_effective_lock_transition(trace, enter_view)?;
        let _authorized_effective_lock = checked_effective_lock.into_projection();
    }
    if accepts_facts(transition_facts(projection)) {
        Some(CheckedProductionTransition { projection })
    } else {
        None
    }
}

/// Report whether the complete production transition relation accepts.
///
/// Production uses [`check`] and consumes its opaque evidence at state
/// installation. This boolean facade remains for pure diagnostics and
/// mutation-focused unit tests which do not cross a mutation boundary.
#[must_use]
pub fn accepts(projection: TransitionProjection<'_>) -> bool {
    check(projection).is_some()
}

#[cfg(test)]
#[path = "refinement_cases.rs"]
mod tests;
