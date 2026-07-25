//! Serialized runtime shell for the authoritative Sumeragi v2 adapter.
//!
//! This module owns scheduling and backpressure, not consensus state. Every
//! admitted command is delivered to [`SumeragiV2Adapter`] by one serialized
//! class-aware arbiter, and all
//! returned [`AdapterEffect`] values are handed to callers unchanged. The only
//! effect inspected here is `EnterView`, because installing a certified view is
//! the sole event allowed to restart the round and retransmission clocks. The
//! round deadline grows linearly with the certified view while retransmission
//! retains its fixed base interval. This deterministic backoff eventually gives
//! a post-GST view enough time for bounded transport and durable body service.
//! A small deterministic arbiter gives the timeout priority while ensuring that
//! periodic retransmission cannot indefinitely exclude already-admitted work.
//! Completion, progress, and normal commands share one bounded allocation but
//! receive cyclic service, so a saturated normal prefix cannot starve a locked
//! Commit vote or a trusted local completion.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use super::v2_core::{
    EFFECTIVE_LOCK_TRACE_SERVICE, EffectiveLockTraceProjection, EventTag,
    ExactBodyCompletionOwnership, ProductionIngressIdentityAndClassTraceProjection,
    SERVICE_CLASS_COMPLETION, SERVICE_CLASS_NONE, SERVICE_CLASS_NORMAL, SERVICE_CLASS_PROGRESS,
    ScheduleState, ScheduledWork, classify_exact_body_completion_ownership,
    production_body_service_refines_async_fairness_kernel,
    production_ingress_identity_and_class_trace_refines_protected_ownership_kernel,
    select_bounded_service_class,
};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode as _, Encode as _};

use super::{
    FairV2IngressOwnershipEvidence,
    v2::{
        AdapterEffect, AdapterError, AuthenticatedConsensusMessage, BodyPipelineCompletionEvidence,
        DecisionLocalProposalDisposition, DeferredAdmissionOrdinalSource, DeferredServiceEvidence,
        SumeragiV2Adapter, classify_decided_local_proposal, proposal_is_safe_for_lock,
    },
    v2_body_store::{DurableBodyReceipt, ValidatedBodyReceipt},
};

#[cfg(test)]
use super::v2::DeferredPriority;

const RETRANSMIT_DIVISOR: u32 = 5;
const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// Derive the deadline for one certified view from the immutable base timeout.
///
/// View zero receives the configured base timeout. Each later view adds one
/// more base interval, so any finite representable post-GST service bound is
/// eventually exceeded. Saturation avoids wraparound at the platform duration
/// limit; the protocol's liveness argument is conditioned on its finite bound
/// being representable by [`Duration`].
fn round_timeout_for_view(base_timeout: Duration, view: u64) -> Duration {
    let multiplier = u128::from(view) + 1;
    let total_nanos = base_timeout.as_nanos().saturating_mul(multiplier);
    let bounded_nanos = total_nanos.min(Duration::MAX.as_nanos());
    let seconds = u64::try_from(bounded_nanos / NANOS_PER_SECOND)
        .expect("duration nanoseconds were bounded by Duration::MAX");
    let nanoseconds = u32::try_from(bounded_nanos % NANOS_PER_SECOND)
        .expect("subsecond nanoseconds are below one billion");
    Duration::new(seconds, nanoseconds)
}

/// Capacity allocation for the single serialized command ingress.
///
/// Normal network traffic may use only the non-reserved prefix. Progress
/// messages (PrepareQCs, CommitQCs, TCs, and authenticated Timeout votes) may
/// additionally use the progress reserve, and trusted asynchronous completions
/// may use the whole queue. This prevents an unbounded proposal/Prepare stream
/// from excluding view-change, CommitQC, or completion work while preserving
/// FIFO order within each service class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeQueueConfig {
    capacity: usize,
    progress_reserve: usize,
    completion_reserve: usize,
}

impl RuntimeQueueConfig {
    /// Construct a bounded class-aware ingress allocation.
    pub(crate) const fn new(
        capacity: usize,
        progress_reserve: usize,
        completion_reserve: usize,
    ) -> Self {
        Self {
            capacity,
            progress_reserve,
            completion_reserve,
        }
    }

    fn validate(self) -> Result<Self, RuntimeConfigError> {
        if self.capacity == 0
            || self.progress_reserve == 0
            || self.completion_reserve == 0
            || self
                .progress_reserve
                .checked_add(self.completion_reserve)
                .is_none_or(|reserved| reserved >= self.capacity)
        {
            return Err(RuntimeConfigError::InvalidQueueAllocation);
        }
        Ok(self)
    }

    const fn normal_limit(self) -> usize {
        self.capacity - self.progress_reserve - self.completion_reserve
    }

    const fn progress_limit(self) -> usize {
        self.capacity - self.completion_reserve
    }
}

impl Default for RuntimeQueueConfig {
    fn default() -> Self {
        Self::new(1024, 128, 256)
    }
}

/// Invalid immutable runtime configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeConfigError {
    /// The round timeout was zero or too small to derive a non-zero fifth.
    InvalidRoundTimeout,
    /// Queue capacity did not leave non-zero normal, progress, and completion
    /// allocations.
    InvalidQueueAllocation,
}

impl fmt::Display for RuntimeConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoundTimeout => formatter.write_str(
                "Sumeragi v2 round timeout must have a non-zero one-fifth retransmit interval",
            ),
            Self::InvalidQueueAllocation => formatter.write_str(
                "Sumeragi v2 runtime queue must reserve non-zero normal, progress, and completion capacity",
            ),
        }
    }
}

impl std::error::Error for RuntimeConfigError {}

/// Invalid activation of the live pacemaker clocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeClockError {
    /// The one-shot post-startup activation already occurred.
    AlreadyArmed,
}

impl fmt::Display for RuntimeClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyArmed => formatter.write_str(
                "Sumeragi v2 live pacemaker clocks may be armed only once after startup",
            ),
        }
    }
}

impl std::error::Error for RuntimeClockError {}

/// Backpressure result from the bounded command ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnqueueError {
    /// Lower-priority traffic reached the boundary of capacity reserved for
    /// protocol progress or trusted completions.
    ReservedCapacity,
    /// The entire command ingress is full.
    Full,
    /// The runtime stopped accepting work after an adapter failure or
    /// process-local admission-ordinal exhaustion.
    FailClosed,
    /// One logical completion stage had conflicting trusted evidence or more
    /// than one serialized owner.
    DuplicateCompletionOwnership,
}

impl fmt::Display for EnqueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedCapacity => {
                formatter.write_str("Sumeragi v2 runtime reserved ingress capacity")
            }
            Self::Full => formatter.write_str("Sumeragi v2 runtime command ingress is full"),
            Self::FailClosed => formatter.write_str("Sumeragi v2 runtime is fail-closed"),
            Self::DuplicateCompletionOwnership => formatter.write_str(
                "Sumeragi v2 body pipeline has conflicting completion evidence or duplicate serialized ownership",
            ),
        }
    }
}

impl std::error::Error for EnqueueError {}

/// Rejection while authenticating or admitting a network message.
#[derive(Debug)]
pub(crate) enum NetworkIngressError {
    /// Signature, structure, version, context, or canonical-manifest admission failed.
    Authentication(AdapterError),
    /// Payload belongs to the body/chunk transport rather than the reducer.
    TransportPayload,
    /// Authenticated input encountered bounded ingress backpressure.
    Backpressure(EnqueueError),
    /// The serialized runtime has already failed closed.
    FailClosed,
}

impl fmt::Display for NetworkIngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authentication(error) => write!(formatter, "{error}"),
            Self::TransportPayload => formatter.write_str(
                "Sumeragi v2 transport payload must use the authenticated body transport",
            ),
            Self::Backpressure(error) => write!(formatter, "{error}"),
            Self::FailClosed => formatter.write_str("Sumeragi v2 runtime is fail-closed"),
        }
    }
}

impl std::error::Error for NetworkIngressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authentication(error) => Some(error),
            Self::Backpressure(error) => Some(error),
            Self::TransportPayload | Self::FailClosed => None,
        }
    }
}

/// Fatal result from executing an already-admitted adapter input.
#[derive(Debug)]
pub(crate) enum RuntimeError<E> {
    /// The adapter rejected an admitted serialized transition.
    Driver(E),
    /// A previous driver failure permanently closed the runtime.
    FailClosed,
    /// The runner attempted live scheduling before startup finished.
    ClocksNotArmed,
    /// Interrupted-tip recovery was attempted after live scheduling began.
    RecoveryAfterClocksArmed,
}

impl<E: fmt::Display> fmt::Display for RuntimeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(error) => write!(formatter, "Sumeragi v2 runtime failed closed: {error}"),
            Self::FailClosed => formatter.write_str("Sumeragi v2 runtime is fail-closed"),
            Self::ClocksNotArmed => {
                formatter.write_str("Sumeragi v2 pacemaker clocks are not armed")
            }
            Self::RecoveryAfterClocksArmed => formatter.write_str(
                "Sumeragi v2 interrupted-tip recovery cannot run after pacemaker clocks are armed",
            ),
        }
    }
}

impl<E> std::error::Error for RuntimeError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Driver(error) => Some(error),
            Self::FailClosed | Self::ClocksNotArmed | Self::RecoveryAfterClocksArmed => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandClass {
    Normal,
    Progress,
    Completion,
}

impl CommandClass {
    const fn service_code(self) -> u8 {
        match self {
            Self::Completion => SERVICE_CLASS_COMPLETION,
            Self::Progress => SERVICE_CLASS_PROGRESS,
            Self::Normal => SERVICE_CLASS_NORMAL,
        }
    }

    const fn from_service_code(code: u8) -> Option<Self> {
        match code {
            SERVICE_CLASS_COMPLETION => Some(Self::Completion),
            SERVICE_CLASS_PROGRESS => Some(Self::Progress),
            SERVICE_CLASS_NORMAL => Some(Self::Normal),
            _ => None,
        }
    }
}

/// Exact production command variant selected from serialized ingress.
///
/// The discriminant is carried separately from the canonical bytes so source
/// refinement can reject either a changed projection or a changed command
/// class without trusting a caller-supplied validity bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeCommandKind {
    /// Cryptographically authenticated consensus envelope.
    Authenticated,
    /// Locally built, durable, and validated proposal body.
    LocalProposalReady,
    /// Reconstructed canonical body became available.
    BodyAvailable,
    /// Exact body-store persistence completed.
    BodyStored,
    /// Deterministic body validation succeeded.
    ValidationSucceeded,
    /// Deterministic body validation failed.
    ValidationFailed,
    /// Consensus signing completed.
    SignatureCompleted,
    /// Decided-body application completed.
    ApplicationCompleted,
    #[cfg(test)]
    /// Deterministic scheduling test command.
    Test,
}

/// Immutable, lossless command identity derived by the command implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeCommandIdentity {
    /// Exact command variant.
    pub(crate) kind: RuntimeCommandKind,
    /// Length-framed canonical fields for the complete command.
    pub(crate) canonical_bytes: Arc<[u8]>,
    /// Hash of `canonical_bytes`, derived alongside the immutable bytes.
    pub(crate) canonical_hash: iroha_crypto::Hash,
}

/// Exact fair-ingress ownership retained for one authenticated runtime
/// command.
///
/// A Commit-certificate discovery response is authenticated as its outer
/// envelope and then projects the enclosed CommitQC into reducer ingress.
/// Direct QC delivery and discovery-response delivery therefore occupy two
/// independent slots while sharing one immutable runtime command. Each slot
/// retains a protocol-bounded set of independently admitted fair-ingress
/// carriers. Identical certificates can legitimately arrive from every voter,
/// so collapsing the slot to one semantic origin would turn a valid duplicate
/// into a fail-closed ownership mismatch. The bound is exact: once every slot
/// is occupied, a new disjoint carrier is rejected rather than summarized
/// without its source identity.
const MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM: usize = wire::MAX_VALIDATORS_PER_HEIGHT;

#[derive(Clone, Debug)]
pub(crate) struct RuntimeIngressOwnershipEvidence {
    runtime_bytes: Arc<[u8]>,
    direct: Vec<FairV2IngressOwnershipEvidence>,
    commit_certificate_response: Vec<FairV2IngressOwnershipEvidence>,
    projection_hash: iroha_crypto::Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeIngressMergeError {
    Capacity,
    Conflict,
}

impl RuntimeIngressOwnershipEvidence {
    fn from_fair_ingress(
        message: &wire::ConsensusMessageV2,
        ownership: FairV2IngressOwnershipEvidence,
    ) -> Option<Self> {
        let outer = ownership.canonical_v2_message()?;
        let (direct, commit_certificate_response) = if outer == *message {
            (vec![ownership], Vec::new())
        } else if matches!(
            (&outer.payload, &message.payload),
            (
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(response),
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
            ) if response.certificate == *certificate
        ) {
            (Vec::new(), vec![ownership])
        } else {
            return None;
        };
        let mut evidence = Self {
            runtime_bytes: Arc::from(message.encode()),
            direct,
            commit_certificate_response,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        evidence.projection_hash = runtime_ingress_ownership_projection_hash(&evidence);
        evidence.validate_exact().then_some(evidence)
    }

    fn merge_downstream(&mut self, candidate: Self) -> Result<(), RuntimeIngressMergeError> {
        if !self.validate_exact()
            || !candidate.validate_exact()
            || self.runtime_bytes != candidate.runtime_bytes
        {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        let mut runtime_cursor = self.runtime_bytes.as_ref();
        let runtime = wire::ConsensusMessageV2::decode(&mut runtime_cursor)
            .map_err(|_| RuntimeIngressMergeError::Conflict)?;
        if !runtime_cursor.is_empty() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        // Distinct semantic origins are independent requests for proposal,
        // vote, timeout, and transport traffic. A QC is an idempotent
        // authenticated fact which can legitimately arrive from every voter,
        // so it alone retains a bounded set of disjoint source carriers in one
        // serialized runtime command.
        let allow_disjoint_carriers = matches!(
            runtime.payload,
            wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        );
        let mut merged = self.clone();
        merge_runtime_ingress_slot(
            &mut merged.direct,
            candidate.direct,
            allow_disjoint_carriers,
        )?;
        merge_runtime_ingress_slot(
            &mut merged.commit_certificate_response,
            candidate.commit_certificate_response,
            allow_disjoint_carriers,
        )?;
        merged.projection_hash = runtime_ingress_ownership_projection_hash(&merged);
        if !merged.validate_exact() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        *self = merged;
        Ok(())
    }

    fn can_merge_downstream(&self, candidate: &Self) -> bool {
        let mut merged = self.clone();
        merged.merge_downstream(candidate.clone()).is_ok()
    }

    fn matches_authenticated(&self, authenticated: &AuthenticatedConsensusMessage) -> bool {
        self.validate_exact()
            && self.runtime_bytes.as_ref() == authenticated.canonical_wire_bytes().as_slice()
    }

    fn validate_exact(&self) -> bool {
        if (self.direct.is_empty() && self.commit_certificate_response.is_empty())
            || self.direct.len() > MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM
            || self.commit_certificate_response.len() > MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM
        {
            return false;
        }
        let mut runtime_cursor = self.runtime_bytes.as_ref();
        let Ok(runtime) = wire::ConsensusMessageV2::decode(&mut runtime_cursor) else {
            return false;
        };
        if !runtime_cursor.is_empty() {
            return false;
        }
        let direct_exact = self.direct.iter().all(|ownership| {
            ownership.validate_exact()
                && ownership
                    .canonical_v2_message()
                    .is_some_and(|message| message == runtime)
        });
        let response_exact = self.commit_certificate_response.iter().all(|ownership| {
            ownership.validate_exact()
                && ownership.canonical_v2_message().is_some_and(|outer| {
                    matches!(
                        (&outer.payload, &runtime.payload),
                        (
                            wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                                response,
                            ),
                            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
                        ) if response.certificate == *certificate
                    )
                })
        });
        let carriers_are_pairwise_disjoint = [&self.direct, &self.commit_certificate_response]
            .into_iter()
            .all(|carriers| {
                carriers.iter().enumerate().all(|(index, carrier)| {
                    carriers[index + 1..]
                        .iter()
                        .all(|other| !carrier.same_semantic_request(other))
                })
            });
        direct_exact
            && response_exact
            && carriers_are_pairwise_disjoint
            && self.projection_hash == runtime_ingress_ownership_projection_hash(self)
    }
}

impl PartialEq for RuntimeIngressOwnershipEvidence {
    fn eq(&self, other: &Self) -> bool {
        self.runtime_bytes == other.runtime_bytes
            && self.projection_hash == other.projection_hash
            && self.direct.len() == other.direct.len()
            && self.commit_certificate_response.len() == other.commit_certificate_response.len()
            && self.validate_exact()
            && other.validate_exact()
    }
}

impl Eq for RuntimeIngressOwnershipEvidence {}

fn merge_runtime_ingress_slot(
    retained: &mut Vec<FairV2IngressOwnershipEvidence>,
    candidates: Vec<FairV2IngressOwnershipEvidence>,
    allow_disjoint_carriers: bool,
) -> Result<(), RuntimeIngressMergeError> {
    for candidate in candidates {
        if !candidate.validate_exact() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        let mut candidate = Some(candidate);
        for existing in retained.iter_mut() {
            if !existing.same_semantic_request(
                candidate
                    .as_ref()
                    .expect("candidate remains present until one merge succeeds"),
            ) {
                continue;
            }
            let mut merged = existing.clone();
            if !merged.merge_downstream(
                candidate
                    .as_ref()
                    .expect("candidate remains present until one merge succeeds")
                    .clone(),
            ) {
                return Err(RuntimeIngressMergeError::Conflict);
            }
            *existing = merged;
            candidate = None;
            break;
        }
        let Some(candidate) = candidate else {
            continue;
        };
        if !allow_disjoint_carriers && !retained.is_empty() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        if retained.len() == MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM {
            return Err(RuntimeIngressMergeError::Capacity);
        }
        retained.push(candidate);
    }
    Ok(())
}

fn runtime_ingress_ownership_projection_hash(
    evidence: &RuntimeIngressOwnershipEvidence,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-ingress-owner:v1");
    append_runtime_identity_field(&mut projection, &evidence.runtime_bytes);
    for carriers in [&evidence.direct, &evidence.commit_certificate_response] {
        append_runtime_identity_u64(
            &mut projection,
            u64::try_from(carriers.len()).expect("bounded ingress carrier count fits u64"),
        );
        for carrier in carriers {
            append_runtime_identity_field(
                &mut projection,
                carrier.process_local_projection_hash().as_ref(),
            );
        }
    }
    iroha_crypto::Hash::new(projection)
}

/// Derive the exact identity of a command rather than accepting an asserted
/// identity from the scheduler's caller.
pub(crate) trait ExactRuntimeCommandIdentity {
    /// Project every command field which can distinguish reducer behavior.
    fn exact_runtime_command_identity(&self) -> RuntimeCommandIdentity;
}

fn append_runtime_identity_field(identity: &mut Vec<u8>, field: &[u8]) {
    let len = u64::try_from(field.len()).expect("runtime command identity field fits u64");
    identity.extend_from_slice(&len.to_le_bytes());
    identity.extend_from_slice(field);
}

fn append_runtime_identity_u64(identity: &mut Vec<u8>, value: u64) {
    append_runtime_identity_field(identity, &value.to_le_bytes());
}

fn append_runtime_identity_tag(identity: &mut Vec<u8>, tag: EventTag) {
    append_runtime_identity_u64(identity, tag.height());
    append_runtime_identity_u64(identity, tag.view());
    append_runtime_identity_u64(identity, tag.generation().get());
}

/// One exact FIFO candidate selected by the class-aware service cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeFifoCandidateOwnership {
    /// Identity derived from the selected command itself.
    pub(crate) identity: RuntimeCommandIdentity,
    /// Redundant explicit kind pinned to the derived identity.
    pub(crate) kind: RuntimeCommandKind,
    /// Frozen service class assigned at admission.
    pub(crate) class: u8,
    /// Exact reducer incarnation tag retained by the queue owner.
    pub(crate) tag: EventTag,
    /// Process-local ordinal minted when this owner entered the runtime queue.
    pub(crate) admission_ordinal: u128,
    /// Exact fair-ingress carrier for authenticated commands. Local trusted
    /// completions and timers never own one.
    pub(crate) ingress_ownership: Option<RuntimeIngressOwnershipEvidence>,
    /// Position in the physical FIFO before class-aware removal.
    pub(crate) fifo_position: u64,
    /// Eligible class skips accumulated before selection.
    pub(crate) eligible_skips_before: u64,
    /// Selection retires the candidate's service debt.
    pub(crate) eligible_skips_after: u64,
    /// Derived integrity hash over every candidate projection field.
    pub(crate) projection_hash: iroha_crypto::Hash,
}

/// Queue rank observed immediately before or after one scheduler decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeQueueOwnershipProjection {
    /// Number of admitted command owners.
    pub(crate) len: u64,
    /// Exact configured queue capacity.
    pub(crate) capacity: u64,
    /// Class-service cursor.
    pub(crate) service_cursor: u8,
    /// Greatest eligible-skip debt retained by any command.
    pub(crate) max_service_debt: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeSchedulerArbitrationInputs {
    live_mode: bool,
    timeout_due: bool,
    periodic_timer_due: bool,
    fifo_ready: bool,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
}

/// Exact source selected for one live or recovery scheduler turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeSelectedOwnerKind {
    /// One older adapter-owned Busy-deferred occurrence.
    Deferred,
    /// Absolute round timeout.
    Timeout,
    /// Periodic retransmission timer.
    PeriodicTimer,
    /// One live class-aware FIFO command.
    Fifo,
    /// No live owner was ready.
    Idle,
    /// One startup-recovery FIFO command.
    RecoveryFifo,
    /// Startup recovery had no ready owner.
    RecoveryIdle,
}

/// Validation result for a retained scheduler ownership carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeSchedulerEvidenceError {
    /// One exact production projection field was altered or inconsistent.
    InvalidProjection,
}

/// Candidate identity carried by a scheduler selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeSelectedCandidateOwnership {
    /// Timer and idle selections do not dispatch a command candidate.
    NotApplicable,
    /// A FIFO or recovery-FIFO selection owns this exact admitted command.
    Exact(RuntimeFifoCandidateOwnership),
    /// An adapter-owned Busy-deferred selection owns this exact occurrence.
    ExactDeferred(RuntimeDeferredCandidateOwnership),
}

/// Exact adapter-deferred occurrence and the fair-ingress carrier which must
/// remain attached until that occurrence crosses the reducer boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeDeferredCandidateOwnership {
    /// Actor-owned deferred service token.
    pub(crate) service: DeferredServiceEvidence,
    /// Authenticated ingress provenance. Trusted local completions and timers
    /// deliberately carry `None`.
    pub(crate) ingress_ownership: Option<RuntimeIngressOwnershipEvidence>,
}

/// Complete scheduler ownership carrier retained at the production step seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeSchedulerOwnershipEvidence {
    /// Selected scheduling branch.
    pub(crate) selected: RuntimeSelectedOwnerKind,
    /// Round tag which owned the scheduling decision before dispatch.
    pub(crate) round_tag: EventTag,
    /// Exact candidate, absence, or typed deferred integration state.
    pub(crate) candidate: RuntimeSelectedCandidateOwnership,
    /// Queue ownership before selection.
    pub(crate) queue_before: RuntimeQueueOwnershipProjection,
    /// Queue ownership after selection.
    pub(crate) queue_after: RuntimeQueueOwnershipProjection,
    /// Whether this occurrence ran the armed live scheduler rather than recovery.
    pub(crate) live_mode: bool,
    /// Whether the absolute round deadline was due before arbitration.
    pub(crate) timeout_due: bool,
    /// Whether the periodic retransmission deadline was due before arbitration.
    pub(crate) periodic_timer_due: bool,
    /// Whether at least one serialized FIFO owner was ready.
    pub(crate) fifo_ready: bool,
    /// Whether the Completion class had an admitted owner.
    pub(crate) completion_ready: bool,
    /// Whether the Progress class had an admitted owner.
    pub(crate) progress_ready: bool,
    /// Whether the Normal class had an admitted owner.
    pub(crate) normal_ready: bool,
    /// Scheduler FIFO debt before selection.
    pub(crate) fifo_owed_before: bool,
    /// Scheduler FIFO debt after selection.
    pub(crate) fifo_owed_after: bool,
    /// Derived integrity hash over the complete selected-owner projection.
    pub(crate) projection_hash: iroha_crypto::Hash,
}

impl RuntimeCommandKind {
    const fn code(self) -> u8 {
        match self {
            Self::Authenticated => 1,
            Self::LocalProposalReady => 2,
            Self::BodyAvailable => 3,
            Self::BodyStored => 4,
            Self::ValidationSucceeded => 5,
            Self::ValidationFailed => 6,
            Self::SignatureCompleted => 7,
            Self::ApplicationCompleted => 8,
            #[cfg(test)]
            Self::Test => 255,
        }
    }
}

impl RuntimeSelectedOwnerKind {
    const fn code(self) -> u8 {
        match self {
            Self::Deferred => 1,
            Self::Timeout => 2,
            Self::PeriodicTimer => 3,
            Self::Fifo => 4,
            Self::Idle => 5,
            Self::RecoveryFifo => 6,
            Self::RecoveryIdle => 7,
        }
    }
}

fn runtime_fifo_candidate_projection_hash(
    candidate: &RuntimeFifoCandidateOwnership,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.push(candidate.kind.code());
    append_runtime_identity_field(&mut projection, candidate.identity.canonical_bytes.as_ref());
    append_runtime_identity_field(&mut projection, candidate.identity.canonical_hash.as_ref());
    projection.push(candidate.class);
    append_runtime_identity_tag(&mut projection, candidate.tag);
    append_runtime_identity_field(&mut projection, &candidate.admission_ordinal.to_le_bytes());
    match &candidate.ingress_ownership {
        None => projection.push(0),
        Some(ownership) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, ownership.projection_hash.as_ref());
        }
    }
    append_runtime_identity_u64(&mut projection, candidate.fifo_position);
    append_runtime_identity_u64(&mut projection, candidate.eligible_skips_before);
    append_runtime_identity_u64(&mut projection, candidate.eligible_skips_after);
    iroha_crypto::Hash::new(projection)
}

fn runtime_scheduler_projection_hash(
    evidence: &RuntimeSchedulerOwnershipEvidence,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.push(evidence.selected.code());
    append_runtime_identity_tag(&mut projection, evidence.round_tag);
    match &evidence.candidate {
        RuntimeSelectedCandidateOwnership::NotApplicable => projection.push(0),
        RuntimeSelectedCandidateOwnership::Exact(candidate) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, candidate.projection_hash.as_ref());
        }
        RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) => {
            projection.push(2);
            append_runtime_identity_field(
                &mut projection,
                candidate.service.projection_hash.as_ref(),
            );
            match &candidate.ingress_ownership {
                None => projection.push(0),
                Some(ownership) => {
                    projection.push(1);
                    append_runtime_identity_field(
                        &mut projection,
                        ownership.projection_hash.as_ref(),
                    );
                }
            }
        }
    }
    for queue in [evidence.queue_before, evidence.queue_after] {
        append_runtime_identity_u64(&mut projection, queue.len);
        append_runtime_identity_u64(&mut projection, queue.capacity);
        projection.push(queue.service_cursor);
        append_runtime_identity_u64(&mut projection, queue.max_service_debt);
    }
    projection.push(u8::from(evidence.timeout_due));
    projection.push(u8::from(evidence.periodic_timer_due));
    projection.push(u8::from(evidence.fifo_ready));
    projection.push(u8::from(evidence.completion_ready));
    projection.push(u8::from(evidence.progress_ready));
    projection.push(u8::from(evidence.normal_ready));
    projection.push(u8::from(evidence.live_mode));
    projection.push(u8::from(evidence.fifo_owed_before));
    projection.push(u8::from(evidence.fifo_owed_after));
    iroha_crypto::Hash::new(projection)
}

impl RuntimeSchedulerOwnershipEvidence {
    /// Validate all locally closed scheduler-rank relations.
    ///
    /// Production deferred evidence is accepted only with the adapter's exact
    /// selected-event token. The typed unavailable result exists solely for a
    /// fake-driver boundary test and is compiled out of production.
    pub(crate) fn validate_exact(&self) -> Result<(), RuntimeSchedulerEvidenceError> {
        if self.projection_hash != runtime_scheduler_projection_hash(self)
            || self.queue_before.capacity != self.queue_after.capacity
            || self.queue_before.len > self.queue_before.capacity
            || self.queue_after.len > self.queue_after.capacity
            || self.fifo_ready != (self.queue_before.len != 0)
            || self.fifo_ready
                != (self.completion_ready || self.progress_ready || self.normal_ready)
            || (!self.live_mode && (self.timeout_due || self.periodic_timer_due))
        {
            return Err(RuntimeSchedulerEvidenceError::InvalidProjection);
        }
        if let (
            RuntimeSelectedOwnerKind::Deferred,
            RuntimeSelectedCandidateOwnership::ExactDeferred(candidate),
        ) = (&self.selected, &self.candidate)
        {
            let ingress_exact = match &candidate.ingress_ownership {
                Some(ownership) => {
                    candidate.service.is_authenticated_ingress()
                        && ownership.validate_exact()
                        && candidate
                            .service
                            .matches_authenticated_runtime_bytes(&ownership.runtime_bytes)
                }
                None => !candidate.service.is_authenticated_ingress(),
            };
            return if candidate.service.validate_exact()
                && candidate.service.service_handoff_is_complete()
                && ingress_exact
                && self.queue_before == self.queue_after
                && self.fifo_owed_before == self.fifo_owed_after
            {
                Ok(())
            } else {
                Err(RuntimeSchedulerEvidenceError::InvalidProjection)
            };
        }
        let schedule_before = ScheduleState {
            fifo_owed: self.fifo_owed_before,
        };
        let (scheduled, schedule_after) =
            schedule_before.select(self.timeout_due, self.periodic_timer_due, self.fifo_ready);
        if schedule_after.fifo_owed != self.fifo_owed_after {
            return Err(RuntimeSchedulerEvidenceError::InvalidProjection);
        }
        match (&self.selected, &self.candidate) {
            (
                RuntimeSelectedOwnerKind::Fifo | RuntimeSelectedOwnerKind::RecoveryFifo,
                RuntimeSelectedCandidateOwnership::Exact(candidate),
            ) => {
                let recovery = self.selected == RuntimeSelectedOwnerKind::RecoveryFifo;
                let service = select_bounded_service_class(
                    self.queue_before.service_cursor,
                    self.completion_ready,
                    self.progress_ready,
                    self.normal_ready,
                );
                let exact = candidate.kind == candidate.identity.kind
                    && candidate.identity.canonical_hash
                        == iroha_crypto::Hash::new(candidate.identity.canonical_bytes.as_ref())
                    && match candidate.kind {
                        RuntimeCommandKind::Authenticated => candidate
                            .ingress_ownership
                            .as_ref()
                            .is_some_and(|ownership| {
                                ownership.validate_exact()
                                    && ownership.runtime_bytes == candidate.identity.canonical_bytes
                            }),
                        _ => candidate.ingress_ownership.is_none(),
                    }
                    && candidate.projection_hash
                        == runtime_fifo_candidate_projection_hash(candidate)
                    && candidate.class != SERVICE_CLASS_NONE
                    && service.selected == candidate.class
                    && service.next == self.queue_after.service_cursor
                    && candidate.fifo_position < self.queue_before.len
                    && candidate.eligible_skips_after == 0
                    && self.queue_after.len.checked_add(1) == Some(self.queue_before.len)
                    && scheduled == ScheduledWork::Fifo
                    && self.live_mode != recovery;
                if exact {
                    Ok(())
                } else {
                    Err(RuntimeSchedulerEvidenceError::InvalidProjection)
                }
            }
            (
                RuntimeSelectedOwnerKind::Timeout,
                RuntimeSelectedCandidateOwnership::NotApplicable,
            ) if self.live_mode
                && self.queue_before == self.queue_after
                && scheduled == ScheduledWork::Timeout =>
            {
                Ok(())
            }
            (
                RuntimeSelectedOwnerKind::PeriodicTimer,
                RuntimeSelectedCandidateOwnership::NotApplicable,
            ) if self.live_mode
                && self.queue_before == self.queue_after
                && scheduled == ScheduledWork::PeriodicTimer =>
            {
                Ok(())
            }
            (RuntimeSelectedOwnerKind::Idle, RuntimeSelectedCandidateOwnership::NotApplicable)
                if self.live_mode
                    && self.queue_before == self.queue_after
                    && scheduled == ScheduledWork::Idle =>
            {
                Ok(())
            }
            (
                RuntimeSelectedOwnerKind::RecoveryIdle,
                RuntimeSelectedCandidateOwnership::NotApplicable,
            ) if !self.live_mode
                && self.queue_before == self.queue_after
                && scheduled == ScheduledWork::Idle =>
            {
                Ok(())
            }
            _ => Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        }
    }
}

pub(crate) struct TaggedCommand<C> {
    tag: EventTag,
    class: CommandClass,
    command: C,
    admitted_at: Instant,
    eligible_skips: u64,
    admission_ordinal: Option<u128>,
    ingress_ownership: Option<RuntimeIngressOwnershipEvidence>,
}

impl<C> TaggedCommand<C> {
    fn new(tag: EventTag, class: CommandClass, command: C, admitted_at: Instant) -> Self {
        Self {
            tag,
            class,
            command,
            admitted_at,
            eligible_skips: 0,
            admission_ordinal: None,
            ingress_ownership: None,
        }
    }

    fn with_ingress_ownership(
        tag: EventTag,
        class: CommandClass,
        command: C,
        admitted_at: Instant,
        ingress_ownership: RuntimeIngressOwnershipEvidence,
    ) -> Self {
        Self {
            tag,
            class,
            command,
            admitted_at,
            eligible_skips: 0,
            admission_ordinal: None,
            ingress_ownership: Some(ingress_ownership),
        }
    }
}

struct BoundedIngress<C> {
    config: RuntimeQueueConfig,
    commands: VecDeque<TaggedCommand<C>>,
    next_class: CommandClass,
    next_admission_ordinal: Option<u128>,
    reserved_body_available: Option<BodyAvailableReservation>,
}

impl<C> BoundedIngress<C> {
    fn new(config: RuntimeQueueConfig) -> Self {
        Self {
            config,
            commands: VecDeque::with_capacity(config.capacity),
            next_class: CommandClass::Completion,
            next_admission_ordinal: Some(0),
            reserved_body_available: None,
        }
    }

    fn enqueue(&mut self, command: TaggedCommand<C>) -> Result<(), EnqueueError> {
        self.enqueue_classified_command(command)
    }

    fn enqueue_classified_command(
        &mut self,
        mut command: TaggedCommand<C>,
    ) -> Result<(), EnqueueError> {
        self.check_capacity(command.class)?;
        command.admission_ordinal = Some(self.claim_admission_ordinal()?);
        let incoming_tag = command.tag;
        let incoming_class = command.class.service_code();
        let queue_len_before = u64::try_from(self.commands.len())
            .expect("bounded runtime ingress length is representable as u64");
        self.commands.push_back(command);
        let stored = self
            .commands
            .back()
            .expect("successful runtime ingress retains the admitted command");
        let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {
            incoming_height: incoming_tag.height(),
            incoming_view: incoming_tag.view(),
            incoming_generation: incoming_tag.generation().get(),
            incoming_class,
            stored_height: stored.tag.height(),
            stored_view: stored.tag.view(),
            stored_generation: stored.tag.generation().get(),
            stored_class: stored.class.service_code(),
            queue_len_before,
            queue_len_after: u64::try_from(self.commands.len())
                .expect("bounded runtime ingress length is representable as u64"),
            queue_capacity: u64::try_from(self.config.capacity)
                .expect("bounded runtime ingress capacity is representable as u64"),
        };
        if !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            ingress_trace,
        ) {
            panic!("Sumeragi v2 ingress changed command identity or service class");
        }
        Ok(())
    }

    fn enqueue_completion_batch(
        &mut self,
        mut commands: Vec<TaggedCommand<C>>,
    ) -> Result<(), EnqueueError> {
        if commands
            .iter()
            .any(|command| command.class != CommandClass::Completion)
        {
            return Err(EnqueueError::FailClosed);
        }
        if commands.len() > self.remaining_capacity() {
            return Err(EnqueueError::Full);
        }
        let first_ordinal = self.claim_admission_ordinal_range(commands.len())?;
        if let Some(first_ordinal) = first_ordinal {
            for (offset, command) in commands.iter_mut().enumerate() {
                let offset = u128::try_from(offset)
                    .expect("bounded runtime batch length is representable as u128");
                command.admission_ordinal = Some(
                    first_ordinal
                        .checked_add(offset)
                        .expect("admission ordinal range was preflighted"),
                );
            }
        }
        for command in commands {
            let incoming_tag = command.tag;
            let incoming_class = command.class.service_code();
            let queue_len_before = u64::try_from(self.commands.len())
                .expect("bounded runtime ingress length is representable as u64");
            self.commands.push_back(command);
            let stored = self
                .commands
                .back()
                .expect("successful runtime batch ingress retains the admitted command");
            let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {
                incoming_height: incoming_tag.height(),
                incoming_view: incoming_tag.view(),
                incoming_generation: incoming_tag.generation().get(),
                incoming_class,
                stored_height: stored.tag.height(),
                stored_view: stored.tag.view(),
                stored_generation: stored.tag.generation().get(),
                stored_class: stored.class.service_code(),
                queue_len_before,
                queue_len_after: u64::try_from(self.commands.len())
                    .expect("bounded runtime ingress length is representable as u64"),
                queue_capacity: u64::try_from(self.config.capacity)
                    .expect("bounded runtime ingress capacity is representable as u64"),
            };
            if !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
                ingress_trace,
            ) {
                panic!("Sumeragi v2 batch ingress changed command identity or service class");
            }
        }
        Ok(())
    }

    fn claim_admission_ordinal(&mut self) -> Result<u128, EnqueueError> {
        self.claim_admission_ordinal_range(1)?
            .ok_or(EnqueueError::FailClosed)
    }

    fn claim_admission_ordinal_range(
        &mut self,
        count: usize,
    ) -> Result<Option<u128>, EnqueueError> {
        if count == 0 {
            return Ok(None);
        }
        let first = self
            .next_admission_ordinal
            .ok_or(EnqueueError::FailClosed)?;
        let offset = u128::try_from(count - 1).map_err(|_| EnqueueError::FailClosed)?;
        let last = first.checked_add(offset).ok_or(EnqueueError::FailClosed)?;
        let successor = last.checked_add(1).ok_or(EnqueueError::FailClosed)?;
        self.next_admission_ordinal = Some(successor);
        Ok(Some(first))
    }

    fn check_capacity(&self, class: CommandClass) -> Result<(), EnqueueError> {
        let limit = match class {
            CommandClass::Normal => self.config.normal_limit(),
            CommandClass::Progress => self.config.progress_limit(),
            CommandClass::Completion => self.config.capacity,
        };
        let occupied = self
            .commands
            .len()
            .saturating_add(usize::from(self.reserved_body_available.is_some()));
        if occupied >= limit {
            return Err(if occupied >= self.config.capacity {
                EnqueueError::Full
            } else {
                EnqueueError::ReservedCapacity
            });
        }
        Ok(())
    }

    fn ownership_projection(&self) -> RuntimeQueueOwnershipProjection {
        RuntimeQueueOwnershipProjection {
            len: u64::try_from(self.commands.len())
                .expect("bounded runtime ingress length is representable as u64"),
            capacity: u64::try_from(self.config.capacity)
                .expect("bounded runtime ingress capacity is representable as u64"),
            service_cursor: self.next_class.service_code(),
            max_service_debt: self
                .commands
                .iter()
                .map(|queued| queued.eligible_skips)
                .max()
                .unwrap_or(0),
        }
    }

    fn class_readiness(&self) -> (bool, bool, bool) {
        let class_ready = |class| self.commands.iter().any(|queued| queued.class == class);
        (
            class_ready(CommandClass::Completion),
            class_ready(CommandClass::Progress),
            class_ready(CommandClass::Normal),
        )
    }

    fn pop_next_with_ownership(
        &mut self,
    ) -> Result<Option<(TaggedCommand<C>, RuntimeFifoCandidateOwnership)>, EnqueueError>
    where
        C: ExactRuntimeCommandIdentity,
    {
        let queue_before = self.ownership_projection();
        let cursor_before = self.next_class.service_code();
        let (completion_ready, progress_ready, normal_ready) = self.class_readiness();
        let selection = select_bounded_service_class(
            cursor_before,
            completion_ready,
            progress_ready,
            normal_ready,
        );
        let service_trace = EffectiveLockTraceProjection {
            kind: EFFECTIVE_LOCK_TRACE_SERVICE,
            relation_exact: select_bounded_service_class(
                cursor_before,
                completion_ready,
                progress_ready,
                normal_ready,
            ) == selection,
            protected_before: 0,
            protected_after: 0,
            owner_before: 0,
            owner_after: 0,
            owner_reused: false,
            ready_before: 0,
            retired_retained: 0,
            retired_ready: 0,
            ready_after: 0,
            store_before: 0,
            retired_store: 0,
            store_after: 0,
            cursor_before,
            completion_ready,
            progress_ready,
            normal_ready,
            selected: selection.selected,
            cursor_after: selection.next,
        };
        if !production_body_service_refines_async_fairness_kernel(service_trace) {
            panic!("Sumeragi v2 bounded service violated the effective-lock trace");
        }
        let Some(next) = CommandClass::from_service_code(selection.next) else {
            return Err(EnqueueError::FailClosed);
        };
        if selection.selected == SERVICE_CLASS_NONE {
            self.next_class = next;
            return Ok(None);
        }
        let Some(class) = CommandClass::from_service_code(selection.selected) else {
            return Err(EnqueueError::FailClosed);
        };
        let Some(index) = self
            .commands
            .iter()
            .position(|queued| queued.class == class)
        else {
            return Err(EnqueueError::FailClosed);
        };
        let selected = self
            .commands
            .get(index)
            .expect("selected runtime FIFO position remains present");
        let admission_ordinal = selected.admission_ordinal.ok_or(EnqueueError::FailClosed)?;
        let identity = selected.command.exact_runtime_command_identity();
        let mut candidate = RuntimeFifoCandidateOwnership {
            kind: identity.kind,
            identity,
            class: selected.class.service_code(),
            tag: selected.tag,
            admission_ordinal,
            ingress_ownership: selected.ingress_ownership.clone(),
            fifo_position: u64::try_from(index)
                .expect("bounded runtime FIFO position is representable as u64"),
            eligible_skips_before: selected.eligible_skips,
            eligible_skips_after: 0,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(&candidate);
        for skipped_class in [
            CommandClass::Completion,
            CommandClass::Progress,
            CommandClass::Normal,
        ] {
            if skipped_class == class {
                continue;
            }
            if self
                .commands
                .iter()
                .find(|queued| queued.class == skipped_class)
                .is_some_and(|oldest| oldest.eligible_skips.checked_add(1).is_none())
            {
                return Err(EnqueueError::FailClosed);
            }
        }
        self.next_class = next;
        for skipped_class in [
            CommandClass::Completion,
            CommandClass::Progress,
            CommandClass::Normal,
        ] {
            if skipped_class == class {
                continue;
            }
            if let Some(oldest) = self
                .commands
                .iter_mut()
                .find(|queued| queued.class == skipped_class)
            {
                oldest.eligible_skips = oldest
                    .eligible_skips
                    .checked_add(1)
                    .expect("service debt overflow was preflighted");
            }
        }
        let command = self
            .commands
            .remove(index)
            .expect("selected runtime FIFO owner remains present");
        debug_assert_eq!(queue_before.len, self.ownership_projection().len + 1);
        Ok(Some((command, candidate)))
    }

    #[cfg(test)]
    fn pop_next(&mut self) -> Option<TaggedCommand<C>>
    where
        C: ExactRuntimeCommandIdentity,
    {
        self.pop_next_with_ownership()
            .expect("test ingress commands always own admission ordinals")
            .map(|(command, _)| command)
    }

    fn len(&self) -> usize {
        self.commands.len()
    }

    fn remaining_capacity(&self) -> usize {
        self.config.capacity.saturating_sub(
            self.commands
                .len()
                .saturating_add(usize::from(self.reserved_body_available.is_some())),
        )
    }

    fn lane_snapshot(&self, class: CommandClass, now: Instant) -> RuntimeQueueLaneSnapshot {
        let mut depth = 0usize;
        let mut oldest_age = None;
        let mut max_service_debt = 0u64;
        for queued in self.commands.iter().filter(|queued| queued.class == class) {
            depth = depth.saturating_add(1);
            let age = now.saturating_duration_since(queued.admitted_at);
            oldest_age = Some(oldest_age.map_or(age, |oldest: Duration| oldest.max(age)));
            max_service_debt = max_service_debt.max(queued.eligible_skips);
        }
        let capacity = match class {
            CommandClass::Normal => self.config.normal_limit(),
            CommandClass::Progress => self.config.progress_limit(),
            CommandClass::Completion => self.config.capacity,
        };
        RuntimeQueueLaneSnapshot {
            depth,
            capacity,
            oldest_age,
            max_service_debt,
        }
    }
}

/// Local operational snapshot for one serialized runtime lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeQueueLaneSnapshot {
    /// Commands currently owned by the lane.
    pub(crate) depth: usize,
    /// Maximum total occupancy at which this class may still be admitted.
    pub(crate) capacity: usize,
    /// Age of the oldest command in this class.
    pub(crate) oldest_age: Option<Duration>,
    /// Eligible dispatches observed by the most-delayed queued command.
    pub(crate) max_service_debt: u64,
}

/// Local operational snapshot for all serialized runtime lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeQueueSnapshot {
    /// Ordinary proposal, vote, and timeout-vote work.
    pub(crate) normal: RuntimeQueueLaneSnapshot,
    /// Certified and exact-lock progress work.
    pub(crate) progress: RuntimeQueueLaneSnapshot,
    /// Trusted local I/O and application completions.
    pub(crate) completion: RuntimeQueueLaneSnapshot,
}

/// Exclusive reservation for one exact `BodyAvailable` runtime handoff.
///
/// A new reservation consumes completion capacity without exposing a reducer
/// command. An exact completion already owned by the serialized runtime yields
/// a coalescing reservation instead. The executor commits this token only
/// after the body-fetch service transfers its exact owner; until then it may
/// abort the token without changing any queued command.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "a body-available reservation must be committed or aborted"]
pub(crate) struct BodyAvailableReservation {
    tag: EventTag,
    manifest: wire::PayloadManifest,
    owns_new_slot: bool,
    admission_ordinal: Option<u128>,
}

impl BodyAvailableReservation {
    /// Construct a runtime-minted token which owns one unpublished completion slot.
    fn reserved_with_admission_ordinal(
        tag: EventTag,
        manifest: wire::PayloadManifest,
        admission_ordinal: u128,
    ) -> Self {
        Self {
            tag,
            manifest,
            owns_new_slot: true,
            admission_ordinal: Some(admission_ordinal),
        }
    }

    /// Construct an ordinal-free reservation for isolated runtime-driver tests.
    ///
    /// Production reservations are minted only by `BoundedIngress`, which
    /// assigns their actor-local admission ordinal before publishing them.
    #[cfg(test)]
    pub(crate) fn reserved(tag: EventTag, manifest: wire::PayloadManifest) -> Self {
        Self {
            tag,
            manifest,
            owns_new_slot: true,
            admission_ordinal: None,
        }
    }

    /// Construct a token which coalesces with one exact existing owner.
    pub(crate) fn coalesced(tag: EventTag, manifest: wire::PayloadManifest) -> Self {
        Self {
            tag,
            manifest,
            owns_new_slot: false,
            admission_ordinal: None,
        }
    }

    /// Reducer incarnation which will consume the completion.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }

    /// Exact canonical manifest carried by the completion.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }

    /// Whether this token reserved a new bounded ingress slot.
    pub(crate) const fn owns_new_slot(&self) -> bool {
        self.owns_new_slot
    }
}

/// Preflight ownership counts for exact decided local-proposal completions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DecisionLocalProposalCounts {
    retainable: usize,
    recovery_only: usize,
    conflicting: usize,
}

impl DecisionLocalProposalCounts {
    /// Record one classified exact Decision owner.
    pub(crate) fn record(&mut self, disposition: DecisionLocalProposalDisposition) {
        let count = match disposition {
            DecisionLocalProposalDisposition::Retain => &mut self.retainable,
            DecisionLocalProposalDisposition::RetireForRecovery => &mut self.recovery_only,
            DecisionLocalProposalDisposition::Conflict => &mut self.conflicting,
        };
        *count = count.saturating_add(1);
    }

    fn merge(self, other: Self) -> Self {
        Self {
            retainable: self.retainable.saturating_add(other.retainable),
            recovery_only: self.recovery_only.saturating_add(other.recovery_only),
            conflicting: self.conflicting.saturating_add(other.conflicting),
        }
    }

    const fn total(self) -> usize {
        self.retainable
            .saturating_add(self.recovery_only)
            .saturating_add(self.conflicting)
    }

    const fn retainable(self) -> usize {
        self.retainable
    }

    const fn recovery_only(self) -> usize {
        self.recovery_only
    }

    const fn conflicting(self) -> usize {
        self.conflicting
    }
}

/// Result of atomically reconciling serialized proposal work with a Decision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DecisionProposalRetirement {
    retained_local_proposal: Option<EventTag>,
    retired_for_recovery: usize,
}

impl DecisionProposalRetirement {
    /// Construct one verified serialized retirement result.
    pub(crate) const fn new(
        retained_local_proposal: Option<EventTag>,
        retired_for_recovery: usize,
    ) -> Self {
        Self {
            retained_local_proposal,
            retired_for_recovery,
        }
    }

    /// Current-tag completion preserved for direct reducer application.
    pub(crate) const fn retained_local_proposal(self) -> Option<EventTag> {
        self.retained_local_proposal
    }

    /// Exact stale completion owners removed so durable reconstruction can proceed.
    pub(crate) const fn retired_for_recovery(self) -> usize {
        self.retired_for_recovery
    }
}

/// Logical completion-stage slots retired when one body pipeline loses ownership.
///
/// Distinct stages may coexist briefly while the serialized adapter is busy.
/// Within one tag/round/subject stage slot, however, the full trusted evidence
/// must agree and exactly one serialized owner may exist across runtime ingress
/// and the adapter's deferred-completion lane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RetiredBodyPipelineCompletions {
    body_available: usize,
    body_stored: usize,
    validation: usize,
    local_proposal: usize,
}

impl RetiredBodyPipelineCompletions {
    /// Record one exact reconstructed-body completion owner.
    pub(crate) fn record_body_available(&mut self) {
        self.body_available = self.body_available.saturating_add(1);
    }

    /// Record one exact durable-store completion owner.
    pub(crate) fn record_body_stored(&mut self) {
        self.body_stored = self.body_stored.saturating_add(1);
    }

    /// Record one exact validation completion owner.
    pub(crate) fn record_validation(&mut self) {
        self.validation = self.validation.saturating_add(1);
    }

    /// Record one exact locally built proposal completion owner.
    pub(crate) fn record_local_proposal(&mut self) {
        self.local_proposal = self.local_proposal.saturating_add(1);
    }

    fn record_matching_command(
        &mut self,
        command: &AdapterCommand,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        match command {
            AdapterCommand::LocalProposalReady { manifest, .. }
                if manifest.round == round && manifest.subject == subject =>
            {
                self.record_local_proposal();
                true
            }
            AdapterCommand::BodyAvailable { manifest }
                if manifest.round == round && manifest.subject == subject =>
            {
                self.record_body_available();
                true
            }
            AdapterCommand::BodyStored {
                round: queued_round,
                subject: queued_subject,
                ..
            } if *queued_round == round && *queued_subject == subject => {
                self.record_body_stored();
                true
            }
            AdapterCommand::ValidationSucceeded {
                round: queued_round,
                subject: queued_subject,
                ..
            }
            | AdapterCommand::ValidationFailed {
                round: queued_round,
                subject: queued_subject,
            } if *queued_round == round && *queued_subject == subject => {
                self.record_validation();
                true
            }
            AdapterCommand::Authenticated(_)
            | AdapterCommand::LocalProposalReady { .. }
            | AdapterCommand::BodyAvailable { .. }
            | AdapterCommand::BodyStored { .. }
            | AdapterCommand::ValidationSucceeded { .. }
            | AdapterCommand::ValidationFailed { .. }
            | AdapterCommand::SignatureCompleted(_)
            | AdapterCommand::ApplicationCompleted(_) => false,
        }
    }

    fn merge(self, other: Self) -> Self {
        Self {
            body_available: self.body_available.saturating_add(other.body_available),
            body_stored: self.body_stored.saturating_add(other.body_stored),
            validation: self.validation.saturating_add(other.validation),
            local_proposal: self.local_proposal.saturating_add(other.local_proposal),
        }
    }

    fn validate_unique(self) -> Result<Self, String> {
        if self.body_available > 1
            || self.body_stored > 1
            || self.validation > 1
            || self.local_proposal > 1
        {
            return Err(
                "Sumeragi v2 body pipeline has duplicate exact serialized completion stages"
                    .to_owned(),
            );
        }
        Ok(self)
    }

    /// Return whether the exact reconstructed-body acknowledgement was retired.
    pub(crate) const fn body_available(self) -> bool {
        self.body_available == 1
    }
}

pub(crate) enum AdapterCommand {
    Authenticated(AuthenticatedConsensusMessage),
    LocalProposalReady {
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    },
    BodyAvailable {
        manifest: wire::PayloadManifest,
    },
    BodyStored {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    },
    ValidationSucceeded {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    },
    ValidationFailed {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    },
    SignatureCompleted(Vec<u8>),
    ApplicationCompleted(wire::BlockSubject),
}

fn manifests_conflict_for_same_body(
    left: &wire::PayloadManifest,
    right: &wire::PayloadManifest,
) -> bool {
    left.round == right.round && left.subject == right.subject && left != right
}

impl AdapterCommand {
    /// Return whether this command owns the candidate's logical stage slot
    /// and, if so, whether its full trusted evidence is exact.
    fn body_pipeline_completion_ownership(
        &self,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> Option<bool> {
        match (self, candidate) {
            (
                Self::LocalProposalReady {
                    manifest,
                    durable_receipt,
                    validated_receipt,
                },
                BodyPipelineCompletionEvidence::LocalProposalReady {
                    manifest: candidate_manifest,
                    durable_receipt: candidate_durable,
                    validated_receipt: candidate_validated,
                },
            ) if manifest.round == candidate_manifest.round
                && manifest.subject == candidate_manifest.subject =>
            {
                Some(
                    manifest == candidate_manifest
                        && durable_receipt == candidate_durable
                        && validated_receipt == candidate_validated,
                )
            }
            (
                Self::BodyAvailable { manifest },
                BodyPipelineCompletionEvidence::BodyAvailable {
                    manifest: candidate_manifest,
                },
            ) if manifest.round == candidate_manifest.round
                && manifest.subject == candidate_manifest.subject =>
            {
                Some(manifest == candidate_manifest)
            }
            (
                Self::BodyStored {
                    round,
                    subject,
                    receipt,
                },
                BodyPipelineCompletionEvidence::BodyStored {
                    round: candidate_round,
                    subject: candidate_subject,
                    receipt: candidate_receipt,
                },
            ) if round == candidate_round && subject == candidate_subject => {
                Some(receipt == candidate_receipt)
            }
            (
                Self::ValidationSucceeded {
                    round,
                    subject,
                    receipt,
                },
                BodyPipelineCompletionEvidence::ValidationSucceeded {
                    round: candidate_round,
                    subject: candidate_subject,
                    receipt: candidate_receipt,
                },
            ) if round == candidate_round && subject == candidate_subject => {
                Some(receipt == candidate_receipt)
            }
            (
                Self::ValidationFailed { round, subject },
                BodyPipelineCompletionEvidence::ValidationFailed {
                    round: candidate_round,
                    subject: candidate_subject,
                },
            ) if round == candidate_round && subject == candidate_subject => Some(true),
            (
                Self::ValidationSucceeded { round, subject, .. },
                BodyPipelineCompletionEvidence::ValidationFailed {
                    round: candidate_round,
                    subject: candidate_subject,
                },
            )
            | (
                Self::ValidationFailed { round, subject },
                BodyPipelineCompletionEvidence::ValidationSucceeded {
                    round: candidate_round,
                    subject: candidate_subject,
                    ..
                },
            ) if round == candidate_round && subject == candidate_subject => Some(false),
            _ => None,
        }
    }

    fn is_same_authenticated_envelope(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> bool {
        matches!(
            self,
            Self::Authenticated(queued)
                if queued.same_wire_envelope(authenticated)
        )
    }

    fn matches_wire_envelope(&self, message: &wire::ConsensusMessageV2) -> bool {
        matches!(
            self,
            Self::Authenticated(queued) if queued.matches_wire_envelope(message)
        )
    }

    fn is_authenticated_proposal_conflicting_with(
        &self,
        canonical: &wire::PayloadManifest,
    ) -> bool {
        let Self::Authenticated(message) = self else {
            return false;
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload() else {
            return false;
        };
        manifests_conflict_for_same_body(&proposal.manifest, canonical)
    }
}

fn append_durable_receipt_identity(identity: &mut Vec<u8>, receipt: &DurableBodyReceipt) {
    append_runtime_identity_field(identity, &receipt.context_id().encode());
    append_runtime_identity_field(identity, &receipt.round().encode());
    append_runtime_identity_field(identity, &receipt.subject().encode());
    append_runtime_identity_field(identity, &receipt.manifest_hash().encode());
    append_runtime_identity_field(identity, &receipt.frame_hash().encode());
}

fn append_validated_receipt_identity(identity: &mut Vec<u8>, receipt: &ValidatedBodyReceipt) {
    let mut durable = Vec::new();
    append_durable_receipt_identity(&mut durable, receipt.durable());
    append_runtime_identity_field(identity, &durable);
    append_runtime_identity_field(identity, &receipt.execution_commitment().encode());
}

impl ExactRuntimeCommandIdentity for AdapterCommand {
    fn exact_runtime_command_identity(&self) -> RuntimeCommandIdentity {
        let (kind, canonical_bytes) = match self {
            Self::Authenticated(authenticated) => (
                RuntimeCommandKind::Authenticated,
                authenticated.canonical_wire_bytes(),
            ),
            Self::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            } => {
                let mut identity = Vec::new();
                append_runtime_identity_field(&mut identity, &manifest.encode());
                let mut durable = Vec::new();
                append_durable_receipt_identity(&mut durable, durable_receipt);
                append_runtime_identity_field(&mut identity, &durable);
                let mut validated = Vec::new();
                append_validated_receipt_identity(&mut validated, validated_receipt);
                append_runtime_identity_field(&mut identity, &validated);
                (RuntimeCommandKind::LocalProposalReady, identity)
            }
            Self::BodyAvailable { manifest } => {
                let mut identity = Vec::new();
                append_runtime_identity_field(&mut identity, &manifest.encode());
                (RuntimeCommandKind::BodyAvailable, identity)
            }
            Self::BodyStored {
                round,
                subject,
                receipt,
            } => {
                let mut identity = Vec::new();
                append_runtime_identity_field(&mut identity, &round.encode());
                append_runtime_identity_field(&mut identity, &subject.encode());
                let mut receipt_identity = Vec::new();
                append_durable_receipt_identity(&mut receipt_identity, receipt);
                append_runtime_identity_field(&mut identity, &receipt_identity);
                (RuntimeCommandKind::BodyStored, identity)
            }
            Self::ValidationSucceeded {
                round,
                subject,
                receipt,
            } => {
                let mut identity = Vec::new();
                append_runtime_identity_field(&mut identity, &round.encode());
                append_runtime_identity_field(&mut identity, &subject.encode());
                let mut receipt_identity = Vec::new();
                append_validated_receipt_identity(&mut receipt_identity, receipt);
                append_runtime_identity_field(&mut identity, &receipt_identity);
                (RuntimeCommandKind::ValidationSucceeded, identity)
            }
            Self::ValidationFailed { round, subject } => {
                let mut identity = Vec::new();
                append_runtime_identity_field(&mut identity, &round.encode());
                append_runtime_identity_field(&mut identity, &subject.encode());
                (RuntimeCommandKind::ValidationFailed, identity)
            }
            Self::SignatureCompleted(signature) => {
                let mut identity = Vec::new();
                append_runtime_identity_field(&mut identity, signature);
                (RuntimeCommandKind::SignatureCompleted, identity)
            }
            Self::ApplicationCompleted(subject) => {
                let mut identity = Vec::new();
                append_runtime_identity_field(&mut identity, &subject.encode());
                (RuntimeCommandKind::ApplicationCompleted, identity)
            }
        };
        let canonical_hash = iroha_crypto::Hash::new(&canonical_bytes);
        RuntimeCommandIdentity {
            kind,
            canonical_bytes: Arc::from(canonical_bytes),
            canonical_hash,
        }
    }
}

impl BoundedIngress<AdapterCommand> {
    fn body_pipeline_completion_ownership(
        &self,
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> (usize, usize) {
        self.commands
            .iter()
            .filter(|queued| queued.tag == tag)
            .fold((0usize, 0usize), |(owners, exact), queued| {
                let Some(is_exact) = queued.command.body_pipeline_completion_ownership(candidate)
                else {
                    return (owners, exact);
                };
                (
                    owners.saturating_add(1),
                    exact.saturating_add(usize::from(is_exact)),
                )
            })
    }

    #[cfg(test)]
    fn authenticated_wire_tag(&self, message: &wire::ConsensusMessageV2) -> Option<EventTag> {
        self.commands.iter().find_map(|queued| {
            queued
                .command
                .matches_wire_envelope(message)
                .then_some(queued.tag)
        })
    }

    fn compatible_authenticated_wire_tag(
        &self,
        message: &wire::ConsensusMessageV2,
        ownership: &RuntimeIngressOwnershipEvidence,
    ) -> Option<EventTag> {
        self.commands.iter().find_map(|queued| {
            (queued.command.matches_wire_envelope(message)
                && queued
                    .ingress_ownership
                    .as_ref()
                    .is_some_and(|retained| retained.can_merge_downstream(ownership)))
            .then_some(queued.tag)
        })
    }

    /// Check whether an independently authenticated form of `message` can
    /// either claim a new slot or coalesce with an exact queued owner.
    ///
    /// Raw equality is only a permission to spend authentication work while a
    /// prefix is saturated.  [`Self::enqueue_authenticated`] repeats equality
    /// on the resulting authenticated token before it coalesces anything.
    fn check_authenticated_wire_capacity_with_ownership(
        &self,
        message: &wire::ConsensusMessageV2,
        ownership: &RuntimeIngressOwnershipEvidence,
        default_class: CommandClass,
        may_use_progress: bool,
    ) -> Result<(), EnqueueError> {
        if self
            .compatible_authenticated_wire_tag(message, ownership)
            .is_some()
        {
            return Ok(());
        }
        match self.check_capacity(default_class) {
            Ok(()) => Ok(()),
            Err(_) if may_use_progress => self.check_capacity(CommandClass::Progress),
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    fn check_authenticated_wire_capacity(
        &self,
        message: &wire::ConsensusMessageV2,
        default_class: CommandClass,
        may_use_progress: bool,
    ) -> Result<(), EnqueueError> {
        if self.authenticated_wire_tag(message).is_some() {
            return Ok(());
        }
        match self.check_capacity(default_class) {
            Ok(()) => Ok(()),
            Err(_) if may_use_progress => self.check_capacity(CommandClass::Progress),
            Err(error) => Err(error),
        }
    }

    /// Enqueue one independently authenticated envelope unless its exact wire
    /// value is already owned by this serialized queue.
    ///
    /// This is deliberately queue-scoped rather than height-long semantic
    /// suppression. Once the queued occurrence leaves, a later retransmission
    /// may be admitted and checked against the adapter's generation-aware
    /// delivery records in the usual way.
    fn enqueue_authenticated_with_ingress_ownership(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        authenticated: AuthenticatedConsensusMessage,
        ingress_ownership: RuntimeIngressOwnershipEvidence,
    ) -> Result<EventTag, EnqueueError> {
        if !ingress_ownership.matches_authenticated(&authenticated) {
            return Err(EnqueueError::FailClosed);
        }
        for index in 0..self.commands.len() {
            if !self.commands[index]
                .command
                .is_same_authenticated_envelope(&authenticated)
            {
                continue;
            }
            let Some(retained) = self.commands[index].ingress_ownership.as_ref() else {
                return Err(EnqueueError::FailClosed);
            };
            let mut merged = retained.clone();
            match merged.merge_downstream(ingress_ownership.clone()) {
                Ok(()) => {}
                Err(RuntimeIngressMergeError::Capacity) => return Err(EnqueueError::Full),
                Err(RuntimeIngressMergeError::Conflict) => continue,
            }
            let queued = self
                .commands
                .get_mut(index)
                .expect("located authenticated runtime owner remains present");
            let Some(retained) = queued.ingress_ownership.as_mut() else {
                return Err(EnqueueError::FailClosed);
            };
            *retained = merged;
            return Ok(queued.tag);
        }
        self.enqueue(TaggedCommand::with_ingress_ownership(
            tag,
            class,
            AdapterCommand::Authenticated(authenticated),
            Instant::now(),
            ingress_ownership,
        ))?;
        Ok(tag)
    }

    #[cfg(test)]
    fn enqueue_authenticated(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        authenticated: AuthenticatedConsensusMessage,
    ) -> Result<EventTag, EnqueueError> {
        let message = authenticated.wire_envelope_for_test();
        let mut admitted = super::fair_v2_ingress_admit_for_test(super::InboundBlockMessage::new(
            super::message::BlockMessage::V2(message.clone()),
            None,
        ));
        let ownership = admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact ownership");
        let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, ownership)
            .expect("direct test runtime ingress projection is exact");
        self.enqueue_authenticated_with_ingress_ownership(tag, class, authenticated, ownership)
    }

    #[cfg(test)]
    fn enqueue_canonical_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        let reservation = self.reserve_canonical_body_available(tag, manifest)?;
        self.commit_canonical_body_available(reservation);
        Ok(())
    }

    fn reserve_canonical_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        if self.reserved_body_available.is_some() {
            return Err(EnqueueError::DuplicateCompletionOwnership);
        }
        let conflicting = self
            .commands
            .iter()
            .filter(|queued| {
                queued
                    .command
                    .is_authenticated_proposal_conflicting_with(&manifest)
            })
            .count();
        let occupied_after_commit = self
            .commands
            .len()
            .saturating_sub(conflicting)
            .saturating_add(1);
        if occupied_after_commit > self.config.capacity {
            return Err(EnqueueError::Full);
        }
        let admission_ordinal = self.claim_admission_ordinal()?;
        let reservation = BodyAvailableReservation::reserved_with_admission_ordinal(
            tag,
            manifest,
            admission_ordinal,
        );
        self.reserved_body_available = Some(reservation.clone());
        Ok(reservation)
    }

    fn commit_canonical_body_available(&mut self, reservation: BodyAvailableReservation) {
        if !reservation.owns_new_slot() {
            return;
        }
        if self.reserved_body_available.as_ref() != Some(&reservation) {
            return;
        }
        self.reserved_body_available = None;
        self.discard_proposals_conflicting_with(reservation.manifest());
        let mut command = TaggedCommand::new(
            reservation.tag(),
            CommandClass::Completion,
            AdapterCommand::BodyAvailable {
                manifest: reservation.manifest,
            },
            Instant::now(),
        );
        command.admission_ordinal = reservation.admission_ordinal;
        let incoming_tag = command.tag;
        let incoming_class = command.class.service_code();
        let queue_len_before = u64::try_from(self.commands.len())
            .expect("bounded runtime ingress length is representable as u64");
        self.commands.push_back(command);
        let stored = self
            .commands
            .back()
            .expect("canonical body commit retains the admitted completion");
        let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {
            incoming_height: incoming_tag.height(),
            incoming_view: incoming_tag.view(),
            incoming_generation: incoming_tag.generation().get(),
            incoming_class,
            stored_height: stored.tag.height(),
            stored_view: stored.tag.view(),
            stored_generation: stored.tag.generation().get(),
            stored_class: stored.class.service_code(),
            queue_len_before,
            queue_len_after: u64::try_from(self.commands.len())
                .expect("bounded runtime ingress length is representable as u64"),
            queue_capacity: u64::try_from(self.config.capacity)
                .expect("bounded runtime ingress capacity is representable as u64"),
        };
        if !production_ingress_identity_and_class_trace_refines_protected_ownership_kernel(
            ingress_trace,
        ) {
            panic!("Sumeragi v2 canonical body ingress changed completion identity or class");
        }
    }

    fn abort_canonical_body_available(&mut self, reservation: BodyAvailableReservation) {
        if reservation.owns_new_slot()
            && self.reserved_body_available.as_ref() == Some(&reservation)
        {
            self.reserved_body_available = None;
        }
    }

    fn discard_proposals_conflicting_with(&mut self, manifest: &wire::PayloadManifest) {
        self.commands.retain(|queued| {
            !queued
                .command
                .is_authenticated_proposal_conflicting_with(manifest)
        });
    }

    fn rebind_canonical_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> usize {
        let mut rebound_count = 0usize;
        for queued in &mut self.commands {
            if queued.tag == previous
                && matches!(
                    &queued.command,
                    AdapterCommand::BodyAvailable { manifest: queued_manifest }
                        if queued_manifest == manifest
                )
            {
                queued.tag = rebound;
                rebound_count = rebound_count.saturating_add(1);
            }
        }
        rebound_count
    }

    fn retire_canonical_body_available(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> usize {
        let before = self.commands.len();
        self.commands.retain(|queued| {
            !(queued.tag == tag
                && matches!(
                    &queued.command,
                    AdapterCommand::BodyAvailable { manifest: queued_manifest }
                        if queued_manifest == manifest
                ))
        });
        before.saturating_sub(self.commands.len())
    }

    fn retire_body_pipeline_completions(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> RetiredBodyPipelineCompletions {
        let mut retired = RetiredBodyPipelineCompletions::default();
        self.commands.retain(|queued| {
            if queued.tag != tag {
                return true;
            }
            !retired.record_matching_command(&queued.command, round, subject)
        });
        retired
    }

    fn body_pipeline_completion_counts(
        &self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> RetiredBodyPipelineCompletions {
        let mut counts = RetiredBodyPipelineCompletions::default();
        for queued in self.commands.iter().filter(|queued| queued.tag == tag) {
            counts.record_matching_command(&queued.command, round, subject);
        }
        counts
    }

    fn decided_local_proposal_counts(
        &self,
        decision_tag: EventTag,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> DecisionLocalProposalCounts {
        let mut counts = DecisionLocalProposalCounts::default();
        for queued in &self.commands {
            if let AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            } = &queued.command
                && let Some(disposition) = classify_decided_local_proposal(
                    queued.tag,
                    manifest,
                    durable_receipt,
                    validated_receipt,
                    decision_tag,
                    decision_round,
                    decision_subject,
                    decision_commitment,
                )
            {
                counts.record(disposition);
            }
        }
        counts
    }

    fn retire_proposal_work_after_decision(
        &mut self,
        decision_tag: EventTag,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) {
        self.commands.retain(|queued| {
            let remove = match &queued.command {
                AdapterCommand::Authenticated(authenticated)
                    if matches!(
                        authenticated.payload(),
                        wire::ConsensusMessageV2Payload::Proposal(proposal)
                            if proposal.round.height == decision_round.height
                    ) =>
                {
                    true
                }
                AdapterCommand::LocalProposalReady {
                    manifest,
                    durable_receipt,
                    validated_receipt,
                } if manifest.round.height == decision_round.height => !matches!(
                    classify_decided_local_proposal(
                        queued.tag,
                        manifest,
                        durable_receipt,
                        validated_receipt,
                        decision_tag,
                        decision_round,
                        decision_subject,
                        decision_commitment,
                    ),
                    Some(DecisionLocalProposalDisposition::Retain)
                ),
                AdapterCommand::Authenticated(_)
                | AdapterCommand::LocalProposalReady { .. }
                | AdapterCommand::BodyAvailable { .. }
                | AdapterCommand::BodyStored { .. }
                | AdapterCommand::ValidationSucceeded { .. }
                | AdapterCommand::ValidationFailed { .. }
                | AdapterCommand::SignatureCompleted(_)
                | AdapterCommand::ApplicationCompleted(_) => false,
            };
            !remove
        });
    }

    fn retire_unsafe_proposals_for_lock(
        &mut self,
        locked_round: wire::ConsensusRound,
        locked_subject: wire::BlockSubject,
    ) -> usize {
        let before = self.commands.len();
        self.commands.retain(|queued| {
            !matches!(
                &queued.command,
                AdapterCommand::Authenticated(authenticated)
                    if matches!(
                        authenticated.payload(),
                        wire::ConsensusMessageV2Payload::Proposal(proposal)
                            if proposal.round.context_id == locked_round.context_id
                                && proposal.round.height == locked_round.height
                                && !proposal_is_safe_for_lock(
                                    proposal,
                                    locked_round,
                                    locked_subject,
                                )
                    )
            )
        });
        before.saturating_sub(self.commands.len())
    }

    fn conflicts_with_pending_body_available(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> bool {
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = authenticated.payload() else {
            return false;
        };
        self.commands.iter().any(|queued| {
            let AdapterCommand::BodyAvailable { manifest } = &queued.command else {
                return false;
            };
            manifests_conflict_for_same_body(&proposal.manifest, manifest)
        })
    }
}

/// Minimal scheduling seam around the sole production adapter.
///
/// The generic parameter exists so clock and queue behavior can be tested
/// deterministically without constructing cryptographic contexts or a WAL.
/// Production uses the implementation for [`SumeragiV2Adapter`] below.
pub(crate) struct RuntimeDriverDispatch<E> {
    effects: Vec<E>,
    deferred_ingress: Option<(u128, RuntimeIngressOwnershipEvidence)>,
}

impl<E> RuntimeDriverDispatch<E> {
    fn completed(effects: Vec<E>) -> Self {
        Self {
            effects,
            deferred_ingress: None,
        }
    }
}

pub(crate) trait RuntimeDriver {
    /// Command payload consumed by the driver.
    type Command: ExactRuntimeCommandIdentity;
    /// Effect emitted unchanged to asynchronous adapters.
    type Effect;
    /// Fatal transition error.
    type Error: fmt::Display;

    /// Current authoritative reducer tag.
    fn current_tag(&self) -> EventTag;
    /// Deliver one admitted command with its original tag.
    fn dispatch(
        &mut self,
        command: TaggedCommand<Self::Command>,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error>;
    /// Deliver the absolute round-timeout event.
    fn timeout_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error>;
    /// Deliver one derived retransmission tick.
    fn retransmit_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error>;
    /// Return whether older adapter-owned Busy-deferred work can cross the
    /// reducer boundary without spinning behind a persistence/signing fence.
    fn deferred_work_is_serviceable(&self) -> bool;
    /// Actor-global source which minted deferred ownership capabilities.
    fn deferred_admission_ordinal_source(&self) -> &DeferredAdmissionOrdinalSource;
    /// Actor-global ordinals of every authenticated occurrence still retained
    /// by the adapter's Busy-deferred queues.
    fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128>;
    /// Deliver exactly one serviceable adapter-owned deferred transition and
    /// its exact selected-occurrence token.
    fn dispatch_deferred(
        &mut self,
    ) -> Result<Option<(Vec<Self::Effect>, DeferredServiceEvidence)>, Self::Error>;
    /// Identify only the effect which authorizes timer restart.
    fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag>;
    /// Return whether the unauthenticated wire shape could match a protected
    /// active-lock item after authentication.
    #[cfg(test)]
    fn wire_ingress_may_use_progress(&self, payload: &wire::ConsensusMessageV2Payload) -> bool;
}

impl RuntimeDriver for SumeragiV2Adapter {
    type Command = AdapterCommand;
    type Effect = AdapterEffect;
    type Error = AdapterError;

    fn current_tag(&self) -> EventTag {
        SumeragiV2Adapter::current_tag(self)
    }

    fn dispatch(
        &mut self,
        tagged: TaggedCommand<Self::Command>,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
        let tag = tagged.tag;
        let authenticated = matches!(&tagged.command, AdapterCommand::Authenticated(_));
        if authenticated != tagged.ingress_ownership.is_some() {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        let ingress_ownership = tagged.ingress_ownership;
        let outcome = match tagged.command {
            AdapterCommand::Authenticated(message) => {
                let ownership =
                    ingress_ownership.ok_or(AdapterError::RuntimeIngressOwnershipViolation)?;
                if !ownership.matches_authenticated(&message) {
                    return Err(AdapterError::RuntimeIngressOwnershipViolation);
                }
                // Authenticated network ingress is deliberately retagged by the
                // adapter if it waited behind a certified view transition.
                // Asynchronous completion variants below retain `tag` exactly.
                let outcome = self.receive_authenticated(message)?;
                let deferred_ingress = outcome
                    .deferred_admission_ordinal()
                    .map(|ordinal| (ordinal, ownership));
                return Ok(RuntimeDriverDispatch {
                    effects: outcome.into_effects(),
                    deferred_ingress,
                });
            }
            AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            } => self.local_proposal_ready(tag, manifest, &durable_receipt, &validated_receipt),
            AdapterCommand::BodyAvailable { manifest } => self.body_available(tag, manifest),
            AdapterCommand::BodyStored {
                round,
                subject,
                receipt,
            } => self.body_stored(tag, round, subject, &receipt),
            AdapterCommand::ValidationSucceeded {
                round,
                subject,
                receipt,
            } => self.validation_succeeded(tag, round, subject, &receipt),
            AdapterCommand::ValidationFailed { round, subject } => {
                self.validation_failed(tag, round, subject)
            }
            AdapterCommand::SignatureCompleted(signature) => {
                self.signature_completed(tag, signature)
            }
            AdapterCommand::ApplicationCompleted(subject) => {
                self.application_completed(tag, subject)
            }
        }?;
        Ok(RuntimeDriverDispatch::completed(outcome.into_effects()))
    }

    fn timeout_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error> {
        SumeragiV2Adapter::timeout_elapsed(self, tag).map(|outcome| outcome.into_effects())
    }

    fn retransmit_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error> {
        SumeragiV2Adapter::retransmit_elapsed(self, tag).map(|outcome| outcome.into_effects())
    }

    fn deferred_work_is_serviceable(&self) -> bool {
        SumeragiV2Adapter::deferred_work_is_serviceable(self)
    }

    fn deferred_admission_ordinal_source(&self) -> &DeferredAdmissionOrdinalSource {
        SumeragiV2Adapter::deferred_admission_ordinal_source(self)
    }

    fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
        SumeragiV2Adapter::authenticated_deferred_admission_ordinals(self)
    }

    fn dispatch_deferred(
        &mut self,
    ) -> Result<Option<(Vec<Self::Effect>, DeferredServiceEvidence)>, Self::Error> {
        SumeragiV2Adapter::drain_deferred_with_evidence(self)
    }

    fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag> {
        match effect {
            AdapterEffect::EnterView { tag, .. } => Some(*tag),
            AdapterEffect::Sign { .. }
            | AdapterEffect::Broadcast(_)
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::StoreBody { .. }
            | AdapterEffect::ValidateBody { .. }
            | AdapterEffect::Apply { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => None,
        }
    }

    #[cfg(test)]
    fn wire_ingress_may_use_progress(&self, payload: &wire::ConsensusMessageV2Payload) -> bool {
        SumeragiV2Adapter::wire_ingress_may_use_progress(self, payload)
    }
}

/// Result of one serialized scheduling step.
///
/// A step invokes the adapter at most once. Consequently, if that invocation
/// fails, no effects from a preceding invocation can be hidden by the error.
#[derive(Debug)]
pub(crate) enum RuntimeStep<E> {
    /// No timer was due and the command ingress was empty.
    Idle,
    /// One timer or command was delivered; effects remain in adapter order.
    Advanced(Vec<E>),
}

/// One-owner, class-aware scheduling shell for Sumeragi v2.
pub(crate) struct SerializedV2Runtime<D: RuntimeDriver = SumeragiV2Adapter> {
    driver: D,
    ingress: BoundedIngress<D::Command>,
    deferred_ingress_ownership: BTreeMap<u128, RuntimeIngressOwnershipEvidence>,
    base_round_timeout: Duration,
    retransmit_interval: Duration,
    round_started_at: Instant,
    retransmit_started_at: Instant,
    round_tag: EventTag,
    clocks_armed: bool,
    timeout_emitted: bool,
    schedule: ScheduleState,
    last_scheduler_ownership: Option<RuntimeSchedulerOwnershipEvidence>,
    fail_closed: bool,
    fail_closed_reason: Option<String>,
}

impl<D: RuntimeDriver> SerializedV2Runtime<D> {
    fn latch_fail_closed(&mut self, reason: impl Into<String>) {
        if self.fail_closed_reason.is_none() {
            let reason = reason.into();
            let tag = self.driver.current_tag();
            iroha_logger::error!(
                reason = reason.as_str(),
                height = tag.height(),
                view = tag.view(),
                generation = tag.generation().get(),
                "Sumeragi v2 serialized runtime failed closed"
            );
            self.fail_closed_reason = Some(reason);
        }
        self.fail_closed = true;
    }

    fn with_driver(
        driver: D,
        started_at: Instant,
        round_timeout: Duration,
        queue_config: RuntimeQueueConfig,
        startup_effects: Vec<D::Effect>,
    ) -> Result<(Self, Vec<D::Effect>), RuntimeConfigError> {
        let retransmit_interval = round_timeout
            .checked_div(RETRANSMIT_DIVISOR)
            .filter(|interval| !interval.is_zero())
            .ok_or(RuntimeConfigError::InvalidRoundTimeout)?;
        let queue_config = queue_config.validate()?;
        let round_tag = driver.current_tag();
        let mut runtime = Self {
            driver,
            ingress: BoundedIngress::new(queue_config),
            deferred_ingress_ownership: BTreeMap::new(),
            base_round_timeout: round_timeout,
            retransmit_interval,
            round_started_at: started_at,
            retransmit_started_at: started_at,
            round_tag,
            clocks_armed: false,
            timeout_emitted: false,
            schedule: ScheduleState::default(),
            last_scheduler_ownership: None,
            fail_closed: false,
            fail_closed_reason: None,
        };
        runtime.observe_effects(started_at, &startup_effects);
        Ok((runtime, startup_effects))
    }

    fn enqueue(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        command: D::Command,
    ) -> Result<(), EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        let result = self
            .ingress
            .enqueue(TaggedCommand::new(tag, class, command, Instant::now()));
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("runtime ingress exact ownership validation failed");
        }
        result
    }

    fn reconcile_deferred_ingress_ownership(
        &mut self,
        handoff: Option<(u128, RuntimeIngressOwnershipEvidence)>,
    ) -> Result<(), RuntimeIngressMergeError> {
        let active = self.driver.authenticated_deferred_admission_ordinals();
        let mut retained = self.deferred_ingress_ownership.clone();
        if let Some((ordinal, candidate)) = handoff {
            if !active.contains(&ordinal) || !candidate.validate_exact() {
                return Err(RuntimeIngressMergeError::Conflict);
            }
            match retained.get_mut(&ordinal) {
                Some(existing) => {
                    existing.merge_downstream(candidate)?;
                }
                None => {
                    retained.insert(ordinal, candidate);
                }
            }
        }
        retained.retain(|ordinal, _| active.contains(ordinal));
        if retained.len() != active.len()
            || !active.iter().all(|ordinal| retained.contains_key(ordinal))
        {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        self.deferred_ingress_ownership = retained;
        Ok(())
    }

    fn accept_driver_dispatch(
        &mut self,
        dispatch: RuntimeDriverDispatch<D::Effect>,
    ) -> Result<Vec<D::Effect>, RuntimeError<D::Error>> {
        if self
            .reconcile_deferred_ingress_ownership(dispatch.deferred_ingress)
            .is_err()
        {
            self.latch_fail_closed("driver dispatch lost deferred ingress ownership");
            return Err(RuntimeError::FailClosed);
        }
        Ok(dispatch.effects)
    }

    fn scheduler_arbitration_inputs(&self, now: Instant) -> RuntimeSchedulerArbitrationInputs {
        let (completion_ready, progress_ready, normal_ready) = self.ingress.class_readiness();
        let fifo_ready = completion_ready || progress_ready || normal_ready;
        let timers_enabled = self.clocks_armed;
        let timeout_due = timers_enabled
            && !self.timeout_emitted
            && now.saturating_duration_since(self.round_started_at)
                >= round_timeout_for_view(self.base_round_timeout, self.round_tag.view());
        let periodic_timer_due = timers_enabled
            && now.saturating_duration_since(self.retransmit_started_at)
                >= self.retransmit_interval;
        RuntimeSchedulerArbitrationInputs {
            live_mode: timers_enabled,
            timeout_due,
            periodic_timer_due,
            fifo_ready,
            completion_ready,
            progress_ready,
            normal_ready,
        }
    }

    fn retain_scheduler_ownership(
        &mut self,
        selected: RuntimeSelectedOwnerKind,
        round_tag: EventTag,
        candidate: RuntimeSelectedCandidateOwnership,
        queue_before: RuntimeQueueOwnershipProjection,
        queue_after: RuntimeQueueOwnershipProjection,
        arbitration: RuntimeSchedulerArbitrationInputs,
        schedule_before: ScheduleState,
        schedule_after: ScheduleState,
    ) -> Result<(), RuntimeError<D::Error>> {
        if self.last_scheduler_ownership.is_some() {
            self.latch_fail_closed("a prior scheduler owner was not consumed");
            return Err(RuntimeError::FailClosed);
        }
        let mut evidence = RuntimeSchedulerOwnershipEvidence {
            selected,
            round_tag,
            candidate,
            queue_before,
            queue_after,
            live_mode: arbitration.live_mode,
            timeout_due: arbitration.timeout_due,
            periodic_timer_due: arbitration.periodic_timer_due,
            fifo_ready: arbitration.fifo_ready,
            completion_ready: arbitration.completion_ready,
            progress_ready: arbitration.progress_ready,
            normal_ready: arbitration.normal_ready,
            fifo_owed_before: schedule_before.fifo_owed,
            fifo_owed_after: schedule_after.fifo_owed,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        evidence.projection_hash = runtime_scheduler_projection_hash(&evidence);
        if evidence.validate_exact().is_err() {
            self.latch_fail_closed("scheduler ownership evidence failed exact validation");
            return Err(RuntimeError::FailClosed);
        }
        self.last_scheduler_ownership = Some(evidence);
        Ok(())
    }

    /// Run at most one adapter-deferred transition, timer, or admitted command.
    ///
    /// Older serviceable adapter debt runs first. Once that debt is empty,
    /// timeout wins when both clocks are due and is emitted at most once for
    /// the installed view. A non-timeout timer may precede queued work once;
    /// the pure scheduler then owes admitted work the next slot. Retransmission
    /// runs at most once per call and advances from the actual service time,
    /// avoiding an unbounded catch-up burst after a paused process. Neither
    /// clock is changed by an arbitrary message or by any effect other than
    /// `EnterView`.
    pub(crate) fn step(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<D::Effect>, RuntimeError<D::Error>> {
        if self.fail_closed {
            return Err(RuntimeError::FailClosed);
        }
        if self.last_scheduler_ownership.is_some() {
            self.latch_fail_closed("live scheduling began with an unconsumed scheduler owner");
            return Err(RuntimeError::FailClosed);
        }
        if !self.clocks_armed {
            return Err(RuntimeError::ClocksNotArmed);
        }

        // Work which already crossed runtime ingress and acquired the
        // adapter's Busy-deferred ownership predates every still-queued
        // command. Once its WAL/signing fence opens, give exactly one such
        // transition a serialized turn. The finite queue rank decreases on
        // every call, and each returned effect batch still represents only one
        // reducer macro-step.
        if let Some(step) = self.dispatch_one_adapter_deferred(now)? {
            return Ok(step);
        }

        let selected_round_tag = self.round_tag;
        let schedule_before = self.schedule;
        let queue_before = self.ingress.ownership_projection();
        let arbitration = self.scheduler_arbitration_inputs(now);
        let (work, next_schedule) = self.schedule.select(
            arbitration.timeout_due,
            arbitration.periodic_timer_due,
            arbitration.fifo_ready,
        );
        self.schedule = next_schedule;

        let effects = match work {
            ScheduledWork::Timeout => {
                let queue_after = self.ingress.ownership_projection();
                self.retain_scheduler_ownership(
                    RuntimeSelectedOwnerKind::Timeout,
                    selected_round_tag,
                    RuntimeSelectedCandidateOwnership::NotApplicable,
                    queue_before,
                    queue_after,
                    arbitration,
                    schedule_before,
                    next_schedule,
                )?;
                self.timeout_emitted = true;
                let effects = match self.driver.timeout_elapsed(self.round_tag) {
                    Ok(effects) => effects,
                    Err(error) => return Err(self.close(error)),
                };
                if self.reconcile_deferred_ingress_ownership(None).is_err() {
                    self.latch_fail_closed(
                        "timeout service lost authenticated deferred ingress ownership",
                    );
                    return Err(RuntimeError::FailClosed);
                }
                effects
            }
            ScheduledWork::PeriodicTimer => {
                let queue_after = self.ingress.ownership_projection();
                self.retain_scheduler_ownership(
                    RuntimeSelectedOwnerKind::PeriodicTimer,
                    selected_round_tag,
                    RuntimeSelectedCandidateOwnership::NotApplicable,
                    queue_before,
                    queue_after,
                    arbitration,
                    schedule_before,
                    next_schedule,
                )?;
                self.retransmit_started_at = now;
                let effects = match self.driver.retransmit_elapsed(self.round_tag) {
                    Ok(effects) => effects,
                    Err(error) => return Err(self.close(error)),
                };
                if self.reconcile_deferred_ingress_ownership(None).is_err() {
                    self.latch_fail_closed(
                        "retransmission service lost authenticated deferred ingress ownership",
                    );
                    return Err(RuntimeError::FailClosed);
                }
                effects
            }
            ScheduledWork::Fifo => {
                let (command, candidate) = match self.ingress.pop_next_with_ownership() {
                    Ok(Some(selected)) => selected,
                    Ok(None) | Err(_) => {
                        self.latch_fail_closed(
                            "FIFO arbitration selected no exact ingress candidate",
                        );
                        return Err(RuntimeError::FailClosed);
                    }
                };
                let queue_after = self.ingress.ownership_projection();
                self.retain_scheduler_ownership(
                    RuntimeSelectedOwnerKind::Fifo,
                    selected_round_tag,
                    RuntimeSelectedCandidateOwnership::Exact(candidate),
                    queue_before,
                    queue_after,
                    arbitration,
                    schedule_before,
                    next_schedule,
                )?;
                match self.driver.dispatch(command) {
                    Ok(dispatch) => self.accept_driver_dispatch(dispatch)?,
                    Err(error) => return Err(self.close(error)),
                }
            }
            ScheduledWork::Idle => {
                let queue_after = self.ingress.ownership_projection();
                self.retain_scheduler_ownership(
                    RuntimeSelectedOwnerKind::Idle,
                    selected_round_tag,
                    RuntimeSelectedCandidateOwnership::NotApplicable,
                    queue_before,
                    queue_after,
                    arbitration,
                    schedule_before,
                    next_schedule,
                )?;
                return Ok(RuntimeStep::Idle);
            }
        };
        self.observe_effects(now, &effects);
        Ok(RuntimeStep::Advanced(effects))
    }

    /// Drain at most one adapter-deferred transition or startup-recovery
    /// command without running live timers.
    ///
    /// An interrupted canonical Kura tip is already decided and can require a
    /// slow local WSV/checkpoint/fsync replay before the height is retired. It
    /// must therefore keep the pacemaker unarmed: no peer can help this local
    /// operation, and elapsed wall time must not manufacture a timeout vote or
    /// retransmission. The runner consumes this runtime after finalization and
    /// constructs a fresh, normally armed successor-height runtime.
    pub(crate) fn step_recovery(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<D::Effect>, RuntimeError<D::Error>> {
        if self.fail_closed {
            return Err(RuntimeError::FailClosed);
        }
        if self.last_scheduler_ownership.is_some() {
            self.latch_fail_closed("recovery began with an unconsumed scheduler owner");
            return Err(RuntimeError::FailClosed);
        }
        if self.clocks_armed {
            return Err(RuntimeError::RecoveryAfterClocksArmed);
        }
        if let Some(step) = self.dispatch_one_adapter_deferred(now)? {
            return Ok(step);
        }
        let round_tag = self.round_tag;
        let queue_before = self.ingress.ownership_projection();
        let schedule_before = self.schedule;
        let arbitration = self.scheduler_arbitration_inputs(now);
        let (scheduled, schedule_after) = schedule_before.select(
            arbitration.timeout_due,
            arbitration.periodic_timer_due,
            arbitration.fifo_ready,
        );
        self.schedule = schedule_after;
        let selected = match self.ingress.pop_next_with_ownership() {
            Ok(selected) => selected,
            Err(_) => {
                self.latch_fail_closed("recovery ingress ownership validation failed");
                return Err(RuntimeError::FailClosed);
            }
        };
        let Some((command, candidate)) = selected else {
            if scheduled != ScheduledWork::Idle {
                self.latch_fail_closed("recovery arbitration selected work without a candidate");
                return Err(RuntimeError::FailClosed);
            }
            let queue_after = self.ingress.ownership_projection();
            self.retain_scheduler_ownership(
                RuntimeSelectedOwnerKind::RecoveryIdle,
                round_tag,
                RuntimeSelectedCandidateOwnership::NotApplicable,
                queue_before,
                queue_after,
                arbitration,
                schedule_before,
                schedule_after,
            )?;
            return Ok(RuntimeStep::Idle);
        };
        if scheduled != ScheduledWork::Fifo {
            self.latch_fail_closed("recovery candidate disagreed with FIFO arbitration");
            return Err(RuntimeError::FailClosed);
        }
        let queue_after = self.ingress.ownership_projection();
        self.retain_scheduler_ownership(
            RuntimeSelectedOwnerKind::RecoveryFifo,
            round_tag,
            RuntimeSelectedCandidateOwnership::Exact(candidate),
            queue_before,
            queue_after,
            arbitration,
            schedule_before,
            schedule_after,
        )?;
        let effects = match self.driver.dispatch(command) {
            Ok(dispatch) => self.accept_driver_dispatch(dispatch)?,
            Err(error) => return Err(self.close(error)),
        };
        self.observe_effects(now, &effects);
        Ok(RuntimeStep::Advanced(effects))
    }

    /// Dispatch one older adapter-owned transition without concatenating it
    /// with a timer or runtime-ingress command.
    ///
    /// Returning `None` means either no adapter debt exists or its reducer
    /// persistence/signature fence still needs an ordinary completion command.
    /// Returning `Some` always represents exactly one reducer macro-step.
    fn dispatch_one_adapter_deferred(
        &mut self,
        now: Instant,
    ) -> Result<Option<RuntimeStep<D::Effect>>, RuntimeError<D::Error>> {
        if !self.driver.deferred_work_is_serviceable() {
            return Ok(None);
        }
        let round_tag = self.round_tag;
        let queue_before = self.ingress.ownership_projection();
        let schedule = self.schedule;
        let arbitration = self.scheduler_arbitration_inputs(now);
        let queue_after = self.ingress.ownership_projection();
        let dispatch = match self.driver.dispatch_deferred() {
            Ok(dispatch) => dispatch,
            Err(error) => return Err(self.close(error)),
        };
        let Some((effects, evidence)) = dispatch else {
            self.latch_fail_closed("serviceable deferred work had no selected owner");
            return Err(RuntimeError::FailClosed);
        };
        if !evidence.belongs_to(self.driver.deferred_admission_ordinal_source())
            || !evidence.adapter_service_is_claimed()
            || !evidence.claim_runtime_handoff_once()
        {
            self.latch_fail_closed("deferred service evidence failed ownership handoff");
            return Err(RuntimeError::FailClosed);
        }
        let ingress_ownership = self
            .deferred_ingress_ownership
            .remove(&evidence.admission_ordinal);
        if evidence.is_authenticated_ingress() != ingress_ownership.is_some()
            || self.reconcile_deferred_ingress_ownership(None).is_err()
        {
            self.latch_fail_closed("deferred service lost authenticated ingress ownership");
            return Err(RuntimeError::FailClosed);
        }
        self.retain_scheduler_ownership(
            RuntimeSelectedOwnerKind::Deferred,
            round_tag,
            RuntimeSelectedCandidateOwnership::ExactDeferred(RuntimeDeferredCandidateOwnership {
                service: evidence,
                ingress_ownership,
            }),
            queue_before,
            queue_after,
            arbitration,
            schedule,
            schedule,
        )?;
        self.observe_effects(now, &effects);
        Ok(Some(RuntimeStep::Advanced(effects)))
    }

    /// Last exact scheduling ownership carrier produced by `step` or
    /// `step_recovery`.
    #[cfg(test)]
    pub(crate) const fn last_scheduler_ownership(
        &self,
    ) -> Option<&RuntimeSchedulerOwnershipEvidence> {
        self.last_scheduler_ownership.as_ref()
    }

    /// Move the most recent exact scheduler carrier into the runner bridge.
    ///
    /// Taking the carrier before the next scheduling call prevents a later
    /// branch from overwriting the production occurrence being handed to the
    /// worker or recovery owner.
    pub(crate) fn take_last_scheduler_ownership(
        &mut self,
    ) -> Option<RuntimeSchedulerOwnershipEvidence> {
        self.last_scheduler_ownership.take()
    }

    /// Advance one live scheduler turn and model the production runner taking
    /// its exact ownership carrier before another turn can enter.
    #[cfg(test)]
    fn step_and_take_scheduler_ownership_for_test(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<D::Effect>, RuntimeError<D::Error>> {
        let result = self.step(now);
        if result.is_ok() {
            self.take_last_scheduler_ownership()
                .expect("every successful live scheduler turn retains exact ownership");
        }
        result
    }

    /// Advance one recovery scheduler turn and model the production runner
    /// taking its exact ownership carrier before another turn can enter.
    #[cfg(test)]
    fn step_recovery_and_take_scheduler_ownership_for_test(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<D::Effect>, RuntimeError<D::Error>> {
        let result = self.step_recovery(now);
        if result.is_ok() {
            self.take_last_scheduler_ownership()
                .expect("every successful recovery scheduler turn retains exact ownership");
        }
        result
    }

    /// Number of admitted commands awaiting serialized delivery.
    pub(crate) fn queued_commands(&self) -> usize {
        self.ingress.len()
    }

    /// Per-class queue ownership, age, and service debt for diagnostics.
    pub(crate) fn queue_snapshot(&self, now: Instant) -> RuntimeQueueSnapshot {
        RuntimeQueueSnapshot {
            normal: self.ingress.lane_snapshot(CommandClass::Normal, now),
            progress: self.ingress.lane_snapshot(CommandClass::Progress, now),
            completion: self.ingress.lane_snapshot(CommandClass::Completion, now),
        }
    }

    /// View-aware diagnostic deadline for declaring a no-progress interval.
    ///
    /// The watchdog allows the complete current-view round deadline plus one
    /// fixed retransmission interval. Both values come from the configured
    /// pacemaker; saturation preserves a conservative diagnostic at the
    /// platform duration limit.
    pub(crate) fn watchdog_threshold(&self) -> Duration {
        round_timeout_for_view(self.base_round_timeout, self.round_tag.view())
            .checked_add(self.retransmit_interval)
            .unwrap_or(Duration::MAX)
    }

    /// Slots into which trusted asynchronous completions can be admitted now.
    ///
    /// Completion producers must consult this bound before removing work from
    /// their own bounded queues. Unlike normal and progress traffic,
    /// completions may use the entire ingress, so this is the exact free
    /// capacity.
    pub(crate) fn remaining_completion_capacity(&self) -> usize {
        self.ingress.remaining_capacity()
    }

    /// Return whether removing this network head can be coupled to immediate
    /// runtime admission.
    ///
    /// Reducer-directed traffic is checked against its exact Normal or
    /// Progress prefix in the single total-length ingress. Transport payloads
    /// do not enter this queue and therefore impose no runtime admission
    /// condition.
    #[cfg(test)]
    pub(crate) fn can_admit_network_payload(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        let class = if self.driver.wire_ingress_may_use_progress(payload) {
            Some(CommandClass::Progress)
        } else {
            network_admission_class(payload)
        };
        class.is_none_or(|class| self.ingress.check_capacity(class).is_ok())
    }

    /// Tag of the view which owns the absolute clocks.
    pub(crate) const fn round_tag(&self) -> EventTag {
        self.round_tag
    }

    /// Arm the live clocks after all height constructors and startup effects.
    ///
    /// This one-shot boundary prevents WAL replay, body-store recovery, worker
    /// startup, and lane-work recovery from consuming the first live view's
    /// deadline. Once armed, only a certified `EnterView` effect may restart
    /// either clock.
    pub(crate) fn arm_live_clocks(&mut self, now: Instant) -> Result<(), RuntimeClockError> {
        if self.clocks_armed {
            return Err(RuntimeClockError::AlreadyArmed);
        }
        self.round_started_at = now;
        self.retransmit_started_at = now;
        self.timeout_emitted = false;
        self.schedule = ScheduleState::default();
        self.clocks_armed = true;
        Ok(())
    }

    /// View-indexed deadline currently owned by the runtime clock.
    #[cfg(test)]
    pub(crate) fn round_timeout(&self) -> Duration {
        round_timeout_for_view(self.base_round_timeout, self.round_tag.view())
    }

    /// Constant retransmission interval derived from the configured timeout.
    #[cfg(test)]
    pub(crate) const fn retransmit_interval(&self) -> Duration {
        self.retransmit_interval
    }

    /// Borrow the sole reducer driver without transferring ownership.
    pub(crate) const fn driver(&self) -> &D {
        &self.driver
    }

    /// Consume the shell and recover ownership of the adapter.
    pub(crate) fn into_driver(self) -> D {
        self.driver
    }

    fn observe_effects(&mut self, now: Instant, effects: &[D::Effect]) {
        for effect in effects {
            if let Some(tag) = D::enter_view_tag(effect) {
                self.round_tag = tag;
                self.round_started_at = now;
                self.retransmit_started_at = now;
                self.timeout_emitted = false;
                self.schedule = ScheduleState::default();
            }
        }
    }

    fn close(&mut self, error: D::Error) -> RuntimeError<D::Error> {
        self.latch_fail_closed(format!(
            "runtime driver rejected a serialized transition: {error}"
        ));
        RuntimeError::Driver(error)
    }
}

impl SerializedV2Runtime<SumeragiV2Adapter> {
    /// Build the reducer-owned status which the runner will publish at the
    /// one-shot live-height activation boundary.
    ///
    /// The snapshot is unavailable until [`Self::arm_live_clocks`] succeeds,
    /// so caller ordering alone cannot publish an unarmed successor.
    pub(crate) fn successor_activation_status_snapshot(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        if !self.clocks_armed {
            return Err(AdapterError::SuccessorClocksNotArmed);
        }
        self.driver.successor_activation_status()
    }

    fn body_pipeline_completion_is_owned(
        &mut self,
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> Result<bool, EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        let (ingress_owners, ingress_exact) = self
            .ingress
            .body_pipeline_completion_ownership(tag, candidate);
        let (deferred_owners, deferred_exact) = self
            .driver
            .deferred_body_pipeline_completion_ownership(tag, candidate);
        match classify_exact_body_completion_ownership(
            ingress_owners,
            ingress_exact,
            deferred_owners,
            deferred_exact,
        ) {
            ExactBodyCompletionOwnership::Vacant => Ok(false),
            ExactBodyCompletionOwnership::Exact => Ok(true),
            ExactBodyCompletionOwnership::Invalid => {
                self.latch_fail_closed(
                    "body completion had conflicting evidence or duplicate serialized owners",
                );
                Err(EnqueueError::DuplicateCompletionOwnership)
            }
        }
    }

    fn enqueue_body_pipeline_completion(
        &mut self,
        tag: EventTag,
        evidence: BodyPipelineCompletionEvidence,
        command: AdapterCommand,
    ) -> Result<(), EnqueueError> {
        if self.body_pipeline_completion_is_owned(tag, &evidence)? {
            return Ok(());
        }
        self.enqueue(tag, CommandClass::Completion, command)
    }

    fn body_available_is_uniquely_owned(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        match self.body_pipeline_completion_is_owned(tag, &evidence) {
            Ok(owned) => Ok(owned),
            Err(EnqueueError::DuplicateCompletionOwnership) => Err(
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
                    .to_owned(),
            ),
            Err(EnqueueError::FailClosed) => {
                Err("Sumeragi v2 runtime is fail-closed".to_owned())
            }
            Err(
                error @ (EnqueueError::ReservedCapacity
                | EnqueueError::Full),
            ) => {
                Err(error.to_string())
            }
        }
    }

    /// Take exclusive ownership of an opened adapter and preserve its recovery
    /// effects for immediate asynchronous dispatch.
    pub(crate) fn new(
        adapter: SumeragiV2Adapter,
        startup_effects: Vec<AdapterEffect>,
        started_at: Instant,
        round_timeout: Duration,
        queue_config: RuntimeQueueConfig,
    ) -> Result<(Self, Vec<AdapterEffect>), RuntimeConfigError> {
        Self::with_driver(
            adapter,
            started_at,
            round_timeout,
            queue_config,
            startup_effects,
        )
    }

    /// Read the reducer-owned proposal constraint without exposing mutable
    /// access to the authoritative adapter.
    pub(crate) fn local_proposal_directive(
        &self,
    ) -> Result<super::v2::LocalProposalDirective, AdapterError> {
        self.driver.local_proposal_directive()
    }

    /// Return the exact Decision key reconstructed by safety-WAL replay.
    pub(crate) fn replayed_decision_key(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        AdapterError,
    > {
        self.driver.replayed_decision_key()
    }

    /// Rebind one independently durable validation marker before replayed
    /// startup effects are dispatched.
    pub(crate) fn recover_validated_body(
        &mut self,
        manifest: &wire::PayloadManifest,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<(), AdapterError> {
        if self.fail_closed {
            return Err(AdapterError::FailClosed);
        }
        self.driver
            .recover_validated_body(manifest, validated_receipt)
    }

    /// Authenticate and enqueue one reducer-directed network message.
    ///
    /// Traffic which passes the bounded capacity check, exactly matches an
    /// already-owned authenticated envelope, or exactly matches a
    /// Busy-deferred QC is cryptographically authenticated and then checked
    /// against canonical authority. Rejections do not poison the runtime.
    /// Once admitted, any adapter transition failure is fatal when the
    /// serialized command is executed.
    pub(crate) fn enqueue_network_with_ingress_ownership(
        &mut self,
        message: wire::ConsensusMessageV2,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<EventTag, NetworkIngressError> {
        let ingress_ownership =
            RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, ingress_ownership)
                .ok_or_else(|| {
                    self.latch_fail_closed(
                        "network ingress changed its authenticated fair-queue ownership",
                    );
                    NetworkIngressError::FailClosed
                })?;
        let default_class = classify_reducer_network_ingress(self.fail_closed, &message.payload)?;
        let deferred_owner = self.driver.deferred_authenticated_message_owner(&message);
        // An exact queued retransmission may always spend authentication work
        // so it can release its ingress occurrence. An exact Busy-deferred QC
        // may likewise spend authentication work without claiming a second
        // queue slot. Otherwise, only the adapter's exact active-lock match
        // may proceed after the normal prefix fills. Authentication below
        // remains mandatory before either form of coalescing.
        let may_be_exact_locked_commit =
            self.driver.wire_ingress_may_use_progress(&message.payload);
        if deferred_owner.is_none() {
            self.ingress
                .check_authenticated_wire_capacity_with_ownership(
                    &message,
                    &ingress_ownership,
                    default_class,
                    may_be_exact_locked_commit,
                )
                .map_err(NetworkIngressError::Backpressure)?;
        }
        let authenticated = match self.driver.authenticate(message) {
            Ok(authenticated) => authenticated,
            Err(AdapterError::FailClosed | AdapterError::ReplayNotComplete) => {
                self.latch_fail_closed("network authentication observed a closed adapter");
                return Err(NetworkIngressError::FailClosed);
            }
            Err(error) => return Err(NetworkIngressError::Authentication(error)),
        };
        let authenticated_deferred_owner = self
            .driver
            .deferred_authenticated_message_owner(authenticated.wire_envelope());
        if authenticated_deferred_owner != deferred_owner {
            // Authentication does not mutate the adapter or envelope. Any
            // disagreement would invalidate the raw-capacity hint rather than
            // authorizing an unchecked queue insertion.
            self.latch_fail_closed(
                "network authentication changed deferred QC ownership classification",
            );
            return Err(NetworkIngressError::FailClosed);
        }
        if let Some((owner_tag, admission_ordinal)) = authenticated_deferred_owner {
            match self
                .reconcile_deferred_ingress_ownership(Some((admission_ordinal, ingress_ownership)))
            {
                Ok(()) => {}
                Err(RuntimeIngressMergeError::Capacity) => {
                    return Err(NetworkIngressError::Backpressure(EnqueueError::Full));
                }
                Err(RuntimeIngressMergeError::Conflict) => {
                    self.latch_fail_closed(
                        "deferred QC admission lost authenticated ingress ownership",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
            }
            return Ok(owner_tag);
        }
        let class = if self
            .driver
            .authenticated_ingress_is_progress(&authenticated)
        {
            CommandClass::Progress
        } else {
            default_class
        };
        if self
            .ingress
            .conflicts_with_pending_body_available(&authenticated)
        {
            return Err(NetworkIngressError::Authentication(
                AdapterError::ConflictingManifest,
            ));
        }
        let tag = self.driver.current_tag();
        match self.ingress.enqueue_authenticated_with_ingress_ownership(
            tag,
            class,
            authenticated,
            ingress_ownership,
        ) {
            Ok(owner) => Ok(owner),
            Err(EnqueueError::FailClosed) => {
                self.latch_fail_closed("authenticated ingress exact ownership validation failed");
                Err(NetworkIngressError::FailClosed)
            }
            Err(error) => Err(NetworkIngressError::Backpressure(error)),
        }
    }

    /// Test-only direct ingress helper. Production callers must preserve the
    /// fair-ingress carrier obtained from the authenticated network boundary.
    #[cfg(test)]
    pub(crate) fn enqueue_network(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<EventTag, NetworkIngressError> {
        let mut admitted = super::fair_v2_ingress_admit_for_test(super::InboundBlockMessage::new(
            super::message::BlockMessage::V2(message.clone()),
            None,
        ));
        let ingress_ownership = admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact ownership");
        self.enqueue_network_with_ingress_ownership(message, ingress_ownership)
    }

    /// Return whether the fair-ingress head can reach authentication and then
    /// either claim its exact runtime prefix or coalesce with an exact queued
    /// authenticated owner.
    pub(crate) fn can_admit_network_message_with_ingress_ownership(
        &self,
        message: &wire::ConsensusMessageV2,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        let (runtime_message, default_class) = match &message.payload {
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => (
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    response.certificate.clone(),
                )),
                CommandClass::Progress,
            ),
            payload => {
                let Some(class) = network_command_class(payload) else {
                    // Body/chunk transport does not enter the reducer FIFO.
                    return true;
                };
                (message.clone(), class)
            }
        };
        let Some(ownership) = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &runtime_message,
            ingress_ownership.clone(),
        ) else {
            // Drain malformed process-local ownership so the mutating seam can
            // fail closed instead of leaving the fair queue permanently stuck.
            return true;
        };
        if self.fail_closed {
            return false;
        }
        if let Some((_, ordinal)) = self
            .driver
            .deferred_authenticated_message_owner(&runtime_message)
        {
            return self
                .deferred_ingress_ownership
                .get(&ordinal)
                .is_some_and(|retained| retained.can_merge_downstream(&ownership));
        }
        let may_be_exact_locked_commit = self
            .driver
            .wire_ingress_may_use_progress(&runtime_message.payload);
        self.ingress
            .check_authenticated_wire_capacity_with_ownership(
                &runtime_message,
                &ownership,
                default_class,
                may_be_exact_locked_commit,
            )
            .is_ok()
    }

    #[cfg(test)]
    pub(crate) fn can_admit_network_message(&self, message: &wire::ConsensusMessageV2) -> bool {
        let mut admitted = super::fair_v2_ingress_admit_for_test(super::InboundBlockMessage::new(
            super::message::BlockMessage::V2(message.clone()),
            None,
        ));
        let ownership = admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact ownership");
        self.can_admit_network_message_with_ingress_ownership(message, &ownership)
    }

    /// Enqueue a completed local proposal build with its original reducer tag.
    pub(crate) fn enqueue_local_proposal(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            validated_receipt: validated_receipt.clone(),
        };
        self.enqueue_body_pipeline_completion(
            tag,
            evidence,
            AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            },
        )
    }

    /// Enqueue successful canonical reconstruction with the exact fetch tag.
    ///
    /// Authenticated proposals already waiting in the FIFO are discarded only
    /// when they advertise a different manifest for this exact round and
    /// subject. Every retained command keeps its original relative order, and
    /// the completion is appended normally.
    pub(crate) fn enqueue_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        let reservation = self.reserve_body_available(tag, manifest)?;
        self.commit_body_available(reservation);
        Ok(())
    }

    /// Reserve exact runtime ownership for a reconstructed body completion.
    ///
    /// Capacity and conflicting queued proposals are evaluated without
    /// exposing a reducer command. The returned token exclusively owns any
    /// claimed completion slot until committed or aborted by the executor.
    pub(crate) fn reserve_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        if self.body_pipeline_completion_is_owned(tag, &evidence)? {
            return Ok(BodyAvailableReservation::coalesced(tag, manifest));
        }
        let result = self.ingress.reserve_canonical_body_available(tag, manifest);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("body-available reservation ownership validation failed");
        }
        result
    }

    /// Publish one previously reserved completion without another fallible
    /// capacity or ownership check.
    pub(crate) fn commit_body_available(&mut self, reservation: BodyAvailableReservation) {
        self.ingress.commit_canonical_body_available(reservation);
    }

    /// Release an unpublished completion reservation after an all-or-error
    /// service transfer rejected the operation.
    pub(crate) fn abort_body_available(&mut self, reservation: BodyAvailableReservation) {
        self.ingress.abort_canonical_body_available(reservation);
    }

    /// Transfer one already admitted exact-body completion to a certified later incarnation.
    ///
    /// The completion can be waiting either in runtime ingress or in the adapter's Busy-deferred
    /// completion lane. `rebound` must be the runtime's installed incarnation,
    /// and source and destination slots are both checked before either queue is
    /// mutated. A single exact destination owner coalesces the transfer by
    /// retiring the unique source; conflicting evidence or duplicate ownership
    /// at either tag fails closed without mutation. Success leaves exactly one
    /// full-evidence owner at `rebound`.
    pub(crate) fn rebind_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        if !rebound.strictly_advances(previous) {
            return Err(
                "Sumeragi v2 body completion rebind did not advance its incarnation".to_owned(),
            );
        }
        if rebound != self.round_tag {
            return Err(
                "Sumeragi v2 body completion rebind target is not the installed runtime incarnation"
                    .to_owned(),
            );
        }
        let source_owned = self.body_available_is_uniquely_owned(previous, manifest)?;
        let destination_owned = self.body_available_is_uniquely_owned(rebound, manifest)?;
        if !source_owned {
            return Ok(false);
        }

        let transferred = if destination_owned {
            let ingress = self
                .ingress
                .retire_canonical_body_available(previous, manifest);
            let deferred = self
                .driver
                .retire_deferred_body_available(previous, manifest);
            ingress.saturating_add(deferred)
        } else {
            let ingress = self
                .ingress
                .rebind_canonical_body_available(previous, rebound, manifest);
            let deferred = self
                .driver
                .rebind_deferred_body_available(previous, rebound, manifest);
            ingress.saturating_add(deferred)
        };
        if transferred != 1 {
            self.latch_fail_closed("body completion rebind changed its serialized owner count");
            return Err(
                "Sumeragi v2 body completion ownership changed during serialized rebind".to_owned(),
            );
        }

        match self.body_available_is_uniquely_owned(rebound, manifest) {
            Ok(true) => {}
            Ok(false) => {
                self.latch_fail_closed("body completion rebind left no destination owner");
                return Err(
                    "Sumeragi v2 body completion rebind did not leave one destination owner"
                        .to_owned(),
                );
            }
            Err(error) => return Err(error),
        }
        Ok(true)
    }

    /// Retire one superseded exact-body completion from its serialized owner.
    ///
    /// The completion may still be waiting in runtime ingress or may already
    /// have crossed into the adapter's Busy-deferred completion lane. Exactly
    /// one owner with the exact manifest evidence is permitted across both
    /// queues, and ownership is checked before either queue is mutated.
    pub(crate) fn retire_body_available(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        if !self.body_available_is_uniquely_owned(tag, manifest)? {
            return Ok(false);
        }
        let ingress = self.ingress.retire_canonical_body_available(tag, manifest);
        let deferred = self.driver.retire_deferred_body_available(tag, manifest);
        let total = ingress.saturating_add(deferred);
        if total != 1 {
            self.latch_fail_closed("body completion retirement changed its owner count");
            return Err(
                "Sumeragi v2 body completion ownership changed during serialized retirement"
                    .to_owned(),
            );
        }
        Ok(true)
    }

    /// Retire every queued completion stage for one exact superseded body pipeline.
    ///
    /// The command may still be in runtime ingress or may have crossed into
    /// the adapter's Busy-deferred completion lane. Different stage slots can
    /// coexist, but each slot must have only one serialized owner. Both lanes
    /// are counted before mutation, so duplicate ownership fails closed while
    /// every occurrence remains available for diagnosis.
    pub(crate) fn retire_body_pipeline_completions(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<RetiredBodyPipelineCompletions, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let expected = self
            .ingress
            .body_pipeline_completion_counts(tag, round, subject)
            .merge(
                self.driver
                    .deferred_body_pipeline_completion_counts(tag, round, subject),
            );
        let expected = match expected.validate_unique() {
            Ok(expected) => expected,
            Err(error) => {
                self.latch_fail_closed(
                    "body pipeline completion retirement found duplicate owners",
                );
                return Err(error);
            }
        };
        let ingress = self
            .ingress
            .retire_body_pipeline_completions(tag, round, subject);
        let deferred = self
            .driver
            .retire_deferred_body_pipeline_completions(tag, round, subject);
        let retired = ingress.merge(deferred);
        let remaining = self
            .ingress
            .body_pipeline_completion_counts(tag, round, subject)
            .merge(
                self.driver
                    .deferred_body_pipeline_completion_counts(tag, round, subject),
            );
        if retired != expected || remaining != RetiredBodyPipelineCompletions::default() {
            self.latch_fail_closed("body pipeline completion retirement changed ownership");
            return Err(
                "Sumeragi v2 body pipeline ownership changed during serialized retirement"
                    .to_owned(),
            );
        }
        Ok(retired)
    }

    /// Retire proposal work made terminal by an exact durable decision.
    ///
    /// Every authenticated proposal and nonmatching local proposal completion
    /// at the decided height is removed from both serialized owners. One exact
    /// current-tag completion remains queued only when its full manifest,
    /// durable receipt, validation receipt, and execution commitment match the
    /// Decision. `decision_round` identifies the selected durable body origin;
    /// it may precede the CommitQC round. Stale exact work is removed for
    /// ordinary durable recovery.
    /// Duplicate or conflicting exact owners fail closed before mutation.
    pub(crate) fn retire_proposal_work_after_decision(
        &mut self,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> Result<DecisionProposalRetirement, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let decision_tag = self.round_tag;
        let expected = self
            .ingress
            .decided_local_proposal_counts(
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            )
            .merge(self.driver.deferred_decided_local_proposal_counts(
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            ));
        if expected.conflicting() != 0 {
            self.latch_fail_closed("decided local proposal evidence conflicted with Decision");
            return Err(
                "Sumeragi v2 decided local proposal evidence conflicts with the durable Decision"
                    .to_owned(),
            );
        }
        if expected.total() > 1 {
            self.latch_fail_closed("decided local proposal had duplicate serialized owners");
            return Err(
                "Sumeragi v2 decided local proposal completion has duplicate serialized owners"
                    .to_owned(),
            );
        }
        self.ingress.retire_proposal_work_after_decision(
            decision_tag,
            decision_round,
            decision_subject,
            decision_commitment,
        );
        self.driver.retire_deferred_proposal_work_after_decision(
            decision_tag,
            decision_round,
            decision_subject,
            decision_commitment,
        );
        if self.reconcile_deferred_ingress_ownership(None).is_err() {
            self.latch_fail_closed(
                "decided proposal retirement lost authenticated ingress ownership",
            );
            return Err(
                "Sumeragi v2 deferred proposal retirement lost authenticated ingress ownership"
                    .to_owned(),
            );
        }
        let remaining = self
            .ingress
            .decided_local_proposal_counts(
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            )
            .merge(self.driver.deferred_decided_local_proposal_counts(
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            ));
        if remaining.conflicting() != 0
            || remaining.recovery_only() != 0
            || remaining.retainable() != expected.retainable()
            || remaining.total() != expected.retainable()
        {
            self.latch_fail_closed("decided proposal retirement changed ownership");
            return Err(
                "Sumeragi v2 decided local proposal ownership changed during serialized retirement"
                    .to_owned(),
            );
        }
        Ok(DecisionProposalRetirement::new(
            (expected.retainable() == 1).then_some(decision_tag),
            expected.recovery_only(),
        ))
    }

    /// Retire authenticated proposals which a newly installed lock makes unsafe.
    ///
    /// Only the exact locked subject at its authenticated proposal origin
    /// remains queued. Prepared-value authority is installed and committed
    /// directly; it never authorizes a competing proposal origin.
    pub(crate) fn retire_unsafe_proposals_for_lock(
        &mut self,
        locked_round: wire::ConsensusRound,
        locked_subject: wire::BlockSubject,
    ) -> Result<usize, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let ingress = self
            .ingress
            .retire_unsafe_proposals_for_lock(locked_round, locked_subject);
        let deferred = self
            .driver
            .retire_deferred_unsafe_proposals_for_lock(locked_round, locked_subject);
        if self.reconcile_deferred_ingress_ownership(None).is_err() {
            self.latch_fail_closed(
                "unsafe proposal retirement lost authenticated ingress ownership",
            );
            return Err(
                "Sumeragi v2 unsafe-proposal retirement lost authenticated ingress ownership"
                    .to_owned(),
            );
        }
        Ok(ingress.saturating_add(deferred))
    }

    /// Enqueue the durable body-store acknowledgement with its exact tag.
    pub(crate) fn enqueue_body_stored(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::BodyStored {
            round,
            subject,
            receipt: receipt.clone(),
        };
        self.enqueue_body_pipeline_completion(
            tag,
            evidence,
            AdapterCommand::BodyStored {
                round,
                subject,
                receipt,
            },
        )
    }

    /// Enqueue successful deterministic validation with its non-forgeable
    /// receipt and the tag of its currently attached reducer consumer.
    pub(crate) fn enqueue_validation_succeeded(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::ValidationSucceeded {
            round,
            subject,
            receipt: receipt.clone(),
        };
        self.enqueue_body_pipeline_completion(
            tag,
            evidence,
            AdapterCommand::ValidationSucceeded {
                round,
                subject,
                receipt,
            },
        )
    }

    /// Enqueue deterministic validation rejection for its currently attached
    /// reducer consumer.
    pub(crate) fn enqueue_validation_failed(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::ValidationFailed { round, subject };
        self.enqueue_body_pipeline_completion(
            tag,
            evidence,
            AdapterCommand::ValidationFailed { round, subject },
        )
    }

    /// Atomically enqueue a set of deterministic validation rejections.
    ///
    /// Exact pre-existing owners coalesce. Every vacant owner and the complete
    /// completion-capacity requirement are checked before any command becomes
    /// visible to the reducer.
    pub(crate) fn enqueue_validation_failures_atomically(
        &mut self,
        failures: &[(EventTag, wire::ConsensusRound, wire::BlockSubject)],
    ) -> Result<(), EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        let mut keys = BTreeSet::new();
        let mut commands = Vec::with_capacity(failures.len());
        let admitted_at = Instant::now();
        for (tag, round, subject) in failures.iter().copied() {
            if !keys.insert((round, subject)) {
                self.latch_fail_closed("validation failure batch contained duplicate body owners");
                return Err(EnqueueError::DuplicateCompletionOwnership);
            }
            let evidence = BodyPipelineCompletionEvidence::ValidationFailed { round, subject };
            if self.body_pipeline_completion_is_owned(tag, &evidence)? {
                continue;
            }
            commands.push(TaggedCommand::new(
                tag,
                CommandClass::Completion,
                AdapterCommand::ValidationFailed { round, subject },
                admitted_at,
            ));
        }
        let result = self.ingress.enqueue_completion_batch(commands);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("validation failure batch ownership validation failed");
        }
        result
    }

    /// Enqueue a signer completion without retagging it to the current view.
    pub(crate) fn enqueue_signature(
        &mut self,
        tag: EventTag,
        signature: Vec<u8>,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::SignatureCompleted(signature),
        )
    }

    /// Enqueue an application completion without retagging it.
    pub(crate) fn enqueue_application_completed(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::ApplicationCompleted(subject),
        )
    }
}

fn network_command_class(payload: &wire::ConsensusMessageV2Payload) -> Option<CommandClass> {
    match payload {
        wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_) => Some(CommandClass::Progress),
        wire::ConsensusMessageV2Payload::Proposal(_) | wire::ConsensusMessageV2Payload::Vote(_) => {
            Some(CommandClass::Normal)
        }
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => None,
    }
}

fn classify_reducer_network_ingress(
    fail_closed: bool,
    payload: &wire::ConsensusMessageV2Payload,
) -> Result<CommandClass, NetworkIngressError> {
    if fail_closed {
        return Err(NetworkIngressError::FailClosed);
    }
    network_command_class(payload).ok_or(NetworkIngressError::TransportPayload)
}

#[cfg(test)]
fn network_admission_class(payload: &wire::ConsensusMessageV2Payload) -> Option<CommandClass> {
    match payload {
        // The transport wrapper is authenticated against an outstanding
        // request, then unwrapped into the embedded CommitQC and admitted to
        // the same Progress prefix before discovery state is retired.
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
            Some(CommandClass::Progress)
        }
        _ => network_command_class(payload),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use crate::sumeragi::v2_core::Generation;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::peer::PeerId;
    use iroha_p2p::network::{NetworkReplyRoute, NetworkReplyRouteTestFixture};
    use tempfile::TempDir;

    use super::*;
    use crate::sumeragi::{
        InboundBlockMessage,
        message::BlockMessage,
        v2::{
            AdapterFingerprints, DeferredBodyPipelineStageForTest, SignRequest,
            VerifiedHeightContext,
        },
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeCommand {
        record: Option<u8>,
        enter_view: Option<EventTag>,
        fail: bool,
    }

    impl FakeCommand {
        const fn record(value: u8) -> Self {
            Self {
                record: Some(value),
                enter_view: None,
                fail: false,
            }
        }

        const fn enter_view(tag: EventTag) -> Self {
            Self {
                record: None,
                enter_view: Some(tag),
                fail: false,
            }
        }

        const fn fail() -> Self {
            Self {
                record: None,
                enter_view: None,
                fail: true,
            }
        }
    }

    impl ExactRuntimeCommandIdentity for FakeCommand {
        fn exact_runtime_command_identity(&self) -> RuntimeCommandIdentity {
            let mut identity = Vec::new();
            match self.record {
                Some(value) => {
                    identity.push(1);
                    identity.push(value);
                }
                None => identity.push(0),
            }
            match self.enter_view {
                Some(tag) => {
                    identity.push(1);
                    append_runtime_identity_tag(&mut identity, tag);
                }
                None => identity.push(0),
            }
            identity.push(u8::from(self.fail));
            let canonical_hash = iroha_crypto::Hash::new(&identity);
            RuntimeCommandIdentity {
                kind: RuntimeCommandKind::Test,
                canonical_bytes: Arc::from(identity),
                canonical_hash,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeEffect {
        enter_view: Option<EventTag>,
    }

    impl FakeEffect {
        const fn other() -> Self {
            Self { enter_view: None }
        }

        const fn enter_view(tag: EventTag) -> Self {
            Self {
                enter_view: Some(tag),
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeError;

    impl fmt::Display for FakeError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("fake driver failure")
        }
    }

    impl std::error::Error for FakeError {}

    struct FakeDriver {
        current_tag: EventTag,
        delivered: Vec<(EventTag, u8)>,
        timeouts: Vec<EventTag>,
        retransmits: Vec<EventTag>,
        timer_effects: VecDeque<Vec<FakeEffect>>,
        deferred_effects: VecDeque<Vec<FakeEffect>>,
        deferred_dispatches: usize,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
        deferred_service_cursor: DeferredPriority,
        deferred_identity_unavailable: bool,
        deferred_evidence_overrides: VecDeque<DeferredServiceEvidence>,
        protected_commit: Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    }

    impl FakeDriver {
        fn new(tag: EventTag) -> Self {
            Self {
                current_tag: tag,
                delivered: Vec::new(),
                timeouts: Vec::new(),
                retransmits: Vec::new(),
                timer_effects: VecDeque::new(),
                deferred_effects: VecDeque::new(),
                deferred_dispatches: 0,
                deferred_admission_ordinals: DeferredAdmissionOrdinalSource::new(0),
                deferred_service_cursor: DeferredPriority::Completion,
                deferred_identity_unavailable: false,
                deferred_evidence_overrides: VecDeque::new(),
                protected_commit: None,
            }
        }
    }

    impl RuntimeDriver for FakeDriver {
        type Command = FakeCommand;
        type Effect = FakeEffect;
        type Error = FakeError;

        fn current_tag(&self) -> EventTag {
            self.current_tag
        }

        fn dispatch(
            &mut self,
            tagged: TaggedCommand<Self::Command>,
        ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
            if tagged.command.fail {
                return Err(FakeError);
            }
            if let Some(tag) = tagged.command.enter_view {
                self.current_tag = tag;
                return Ok(RuntimeDriverDispatch::completed(vec![
                    FakeEffect::enter_view(tag),
                ]));
            }
            let value = tagged.command.record.expect("well-formed fake command");
            self.delivered.push((tagged.tag, value));
            Ok(RuntimeDriverDispatch::completed(vec![FakeEffect::other()]))
        }

        fn timeout_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error> {
            self.timeouts.push(tag);
            Ok(self.timer_effects.pop_front().unwrap_or_default())
        }

        fn retransmit_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error> {
            self.retransmits.push(tag);
            Ok(self.timer_effects.pop_front().unwrap_or_default())
        }

        fn deferred_work_is_serviceable(&self) -> bool {
            !self.deferred_effects.is_empty()
        }

        fn deferred_admission_ordinal_source(&self) -> &DeferredAdmissionOrdinalSource {
            &self.deferred_admission_ordinals
        }

        fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
            BTreeSet::new()
        }

        fn dispatch_deferred(
            &mut self,
        ) -> Result<Option<(Vec<Self::Effect>, DeferredServiceEvidence)>, Self::Error> {
            self.deferred_dispatches = self.deferred_dispatches.saturating_add(1);
            let before = u64::try_from(self.deferred_effects.len())
                .expect("bounded fake deferred queue length fits u64");
            let effects = self.deferred_effects.pop_front().unwrap_or_default();
            if self.deferred_identity_unavailable {
                return Ok(None);
            }
            let evidence = match self.deferred_evidence_overrides.pop_front() {
                Some(evidence) => evidence,
                None => {
                    let evidence = DeferredServiceEvidence::completion_for_test(
                        &self.deferred_admission_ordinals,
                        self.current_tag,
                        before,
                        self.deferred_service_cursor,
                    );
                    assert!(evidence.claim_adapter_service_for_test());
                    evidence
                }
            };
            self.deferred_service_cursor = evidence.service_cursor_after;
            Ok(Some((effects, evidence)))
        }

        fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag> {
            effect.enter_view
        }

        fn wire_ingress_may_use_progress(&self, payload: &wire::ConsensusMessageV2Payload) -> bool {
            matches!(
                (payload, self.protected_commit),
                (
                    wire::ConsensusMessageV2Payload::Vote(vote),
                    Some((round, subject, execution_commitment))
                ) if vote.phase == wire::GlobalPhase::Commit
                    && vote.round == round
                    && vote.subject == subject
                    && vote.execution_commitment == execution_commitment
            )
        }
    }

    fn tag(view: u64) -> EventTag {
        EventTag::new(7, view, Generation::new(view + 11))
    }

    fn authenticated_proposal_for_test(
        manifest: wire::PayloadManifest,
    ) -> AuthenticatedConsensusMessage {
        AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
                round: manifest.round,
                proposer: 0,
                subject: manifest.subject,
                manifest,
                justification: wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate: None },
                ),
                signature: vec![1],
            }),
        ))
    }

    fn authenticated_runtime_context() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic runtime ingress key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id: "sumeragi-v2-runtime-ingress-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("runtime fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"runtime ingress nexus context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1024 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0x5A; 32],
        };
        (context, keys)
    }

    fn signed_runtime_proposal(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        marker: u8,
    ) -> wire::ConsensusMessageV2 {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
            payload_hash: Hash::new([marker, 2]),
        };
        let body = vec![marker; 4];
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("small runtime fixture body"),
            &[body],
        )
        .expect("valid runtime fixture manifest");
        let proposer = context.leader(round.view);
        let mut proposal = wire::Proposal {
            round,
            proposer,
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
            &proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal))
    }

    fn fair_runtime_ownership(
        message: &wire::ConsensusMessageV2,
        semantic_origin: PeerId,
        authenticated_via: PeerId,
    ) -> FairV2IngressOwnershipEvidence {
        let mut inbound =
            super::super::fair_v2_ingress_admit_for_test(InboundBlockMessage::from_transport(
                BlockMessage::V2(message.clone()),
                semantic_origin,
                authenticated_via,
            ));
        inbound
            .take_ingress_ownership()
            .expect("real fair ingress attaches exact ownership")
    }

    fn fair_runtime_ownership_with_reply_route(
        message: &wire::ConsensusMessageV2,
        semantic_origin: PeerId,
        authenticated_via: PeerId,
        reply_route: NetworkReplyRoute,
    ) -> FairV2IngressOwnershipEvidence {
        let mut inbound = super::super::fair_v2_ingress_admit_for_test(
            InboundBlockMessage::try_from_transport_with_reply_route(
                BlockMessage::V2(message.clone()),
                semantic_origin,
                authenticated_via,
                reply_route,
            )
            .expect("test transport identities bind the reply capability"),
        );
        inbound
            .take_ingress_ownership()
            .expect("real fair ingress attaches route ownership")
    }

    fn signed_runtime_quorum_certificate(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        marker: u8,
    ) -> wire::QuorumCertificate {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 5])),
            payload_hash: Hash::new([marker, 6]),
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new([marker, 7]),
            Hash::new([marker, 8]),
            Hash::new([marker, 9]),
            Hash::new([marker, 10]),
        );
        let signers = vec![0, 1, 2];
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate runtime fixture certificate"),
        }
    }

    fn runtime_manifest(context: &wire::HeightContext, marker: u8) -> wire::PayloadManifest {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 3])),
            payload_hash: Hash::new([marker, 4]),
        };
        let body = vec![marker; 4];
        wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("small runtime manifest body"),
            &[body],
        )
        .expect("valid runtime manifest")
    }

    fn observe_enter_view_for_test(
        runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) {
        assert_eq!(runtime.round_tag(), previous);
        runtime.observe_effects(
            Instant::now(),
            &[AdapterEffect::EnterView {
                tag: rebound,
                certificate: wire::TimeoutCertificate {
                    round: wire::ConsensusRound {
                        view: rebound
                            .view()
                            .checked_sub(1)
                            .expect("test EnterView target has a predecessor"),
                        ..manifest.round
                    },
                    groups: vec![wire::TimeoutVoteGroup {
                        highest_prepare_qc: None,
                        signers: vec![0, 1, 2],
                        aggregate_signature: vec![0xA5; 96],
                    }],
                },
                protected_body: Some((manifest.round, manifest.subject)),
            }],
        );
        assert_eq!(runtime.round_tag(), rebound);
    }

    #[test]
    fn body_available_rebind_accepts_same_view_higher_generation() {
        let directory = TempDir::new().expect("temporary same-view rebind directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let initial = runtime.round_tag();
        let view_one = EventTag::new(
            initial.height(),
            1,
            Generation::new(initial.generation().get() + 1),
        );
        let manifest = runtime_manifest(&context, 0x8A);
        observe_enter_view_for_test(&mut runtime, initial, view_one, &manifest);

        runtime
            .enqueue_body_available(view_one, manifest.clone())
            .expect("enqueue the unique old-generation owner");
        let rebound = EventTag::new(
            view_one.height(),
            view_one.view(),
            Generation::new(view_one.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut runtime, view_one, rebound, &manifest);

        assert!(
            runtime
                .rebind_body_available(view_one, rebound, &manifest)
                .expect("same-view generation supersession transfers the exact owner")
        );
        assert_eq!(runtime.queued_commands(), 1);
        assert!(matches!(
            runtime.ingress.commands.front(),
            Some(TaggedCommand {
                tag,
                command: AdapterCommand::BodyAvailable {
                    manifest: queued_manifest,
                },
                ..
            }) if *tag == rebound && queued_manifest == &manifest
        ));
        assert!(!runtime.fail_closed);
    }

    fn authenticated_network_runtime(
        directory: &TempDir,
        queue: RuntimeQueueConfig,
    ) -> (
        SerializedV2Runtime<SumeragiV2Adapter>,
        wire::HeightContext,
        Vec<KeyPair>,
    ) {
        authenticated_network_runtime_with_local_validator(directory, queue, None)
    }

    fn authenticated_network_runtime_with_local_validator(
        directory: &TempDir,
        queue: RuntimeQueueConfig,
        local_validator: Option<wire::ValidatorIndex>,
    ) -> (
        SerializedV2Runtime<SumeragiV2Adapter>,
        wire::HeightContext,
        Vec<KeyPair>,
    ) {
        let (context, keys) = authenticated_runtime_context();
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("runtime fixture proof of possession")
            })
            .collect();
        let verified =
            VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified fixture");
        let (adapter, startup) = SumeragiV2Adapter::open(
            directory.path().join("runtime-ingress-safety.wal"),
            verified,
            local_validator,
            Generation::new(1),
            [0x31; 32],
            AdapterFingerprints {
                node: Hash::new(b"runtime ingress node"),
                build: Hash::new(b"runtime ingress build"),
                config: Hash::new(b"runtime ingress config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open authenticated network runtime adapter");
        assert!(startup.is_empty());
        let runtime = SerializedV2Runtime::new(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            queue,
        )
        .expect("valid authenticated network runtime")
        .0;
        (runtime, context, keys)
    }

    fn fair_network_ownership(
        message: &wire::ConsensusMessageV2,
        sender: PeerId,
    ) -> FairV2IngressOwnershipEvidence {
        let mut admitted =
            super::super::fair_v2_ingress_admit_for_test(super::super::InboundBlockMessage::new(
                super::super::message::BlockMessage::V2(message.clone()),
                Some(sender),
            ));
        admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact source ownership")
    }

    fn fair_network_ownership_with_route(
        message: &wire::ConsensusMessageV2,
        semantic_origin: PeerId,
        authenticated_via: PeerId,
        route: NetworkReplyRoute,
    ) -> FairV2IngressOwnershipEvidence {
        let inbound = super::super::InboundBlockMessage::try_from_transport_with_reply_route(
            super::super::message::BlockMessage::V2(message.clone()),
            semantic_origin,
            authenticated_via,
            route,
        )
        .expect("test reply route binds the semantic origin and authenticated source");
        let mut admitted = super::super::fair_v2_ingress_admit_for_test(inbound);
        admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact routed ownership")
    }

    fn runtime(
        driver: FakeDriver,
        start: Instant,
        queue: RuntimeQueueConfig,
    ) -> SerializedV2Runtime<FakeDriver> {
        let mut runtime = SerializedV2Runtime::with_driver(
            driver,
            start,
            Duration::from_secs(10),
            queue,
            Vec::new(),
        )
        .expect("valid fake runtime")
        .0;
        runtime
            .arm_live_clocks(start)
            .expect("arm fake runtime after startup");
        runtime
    }

    fn enqueue_fake(
        runtime: &mut SerializedV2Runtime<FakeDriver>,
        tag: EventTag,
        class: CommandClass,
        command: FakeCommand,
    ) -> Result<(), EnqueueError> {
        runtime.enqueue(tag, class, command)
    }

    #[test]
    fn successor_activation_snapshot_requires_armed_live_clocks() {
        let directory = TempDir::new().expect("temporary successor-clock directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));

        assert!(matches!(
            runtime.successor_activation_status_snapshot(),
            Err(AdapterError::SuccessorClocksNotArmed)
        ));

        runtime
            .arm_live_clocks(Instant::now())
            .expect("arm clocks after all startup work");
        let status = runtime
            .successor_activation_status_snapshot()
            .expect("armed runtime may produce its activation snapshot");
        assert_eq!(status.height_context_id, context.id());
        assert_eq!(status.height, context.height);
        assert!(matches!(
            status.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
    }

    #[test]
    fn absolute_timeout_fires_once_and_messages_never_reset_it() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        assert_eq!(runtime.remaining_completion_capacity(), 8);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(10));
        assert_eq!(runtime.retransmit_interval(), Duration::from_secs(2));
        assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(12));

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("enqueue message");
        assert!(matches!(
            runtime.step(start + Duration::from_secs(1)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("message dispatch publishes scheduler ownership")
                .validate_exact(),
            Ok(())
        );

        assert!(matches!(
            runtime.step(start + Duration::from_secs(2)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("retransmit dispatch publishes scheduler ownership")
                .validate_exact(),
            Ok(())
        );
        assert_eq!(runtime.driver.retransmits, vec![initial]);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(2),
        )
        .expect("enqueue second message");
        runtime
            .step(start + Duration::from_secs(9))
            .expect("second message dispatch succeeds");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("second message dispatch publishes scheduler ownership")
                .validate_exact(),
            Ok(())
        );
        runtime
            .step(start + Duration::from_secs(10))
            .expect("absolute timeout dispatch succeeds");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("timeout dispatch publishes scheduler ownership")
                .validate_exact(),
            Ok(())
        );
        runtime
            .step(start + Duration::from_secs(20))
            .expect("post-timeout scheduling succeeds");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("post-timeout scheduling publishes scheduler ownership")
                .validate_exact(),
            Ok(())
        );
        assert_eq!(runtime.driver.timeouts, vec![initial]);
    }

    #[test]
    fn serviceable_adapter_debt_drains_one_macro_step_before_new_work() {
        let start = Instant::now();
        let initial = tag(0);
        let mut driver = FakeDriver::new(initial);
        driver
            .deferred_effects
            .push_back(vec![FakeEffect::other(), FakeEffect::other()]);
        driver.deferred_effects.push_back(vec![FakeEffect::other()]);
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(8, 2, 2));
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .expect("enqueue newer runtime work");

        let due = start + Duration::from_secs(10);
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 2
        ));
        assert_eq!(runtime.driver.deferred_dispatches, 1);
        assert_eq!(runtime.queued_commands(), 1);
        assert!(runtime.driver.timeouts.is_empty());

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.deferred_dispatches, 2);
        assert_eq!(runtime.queued_commands(), 1);
        assert!(runtime.driver.timeouts.is_empty());

        // The finite debt is now empty. The already-due absolute timeout keeps
        // its normal precedence, proving deferred service delays but cannot
        // erase the timer or overtake more than its decreasing queue rank.
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        assert_eq!(runtime.driver.timeouts, vec![initial]);
        assert_eq!(runtime.queued_commands(), 1);
    }

    #[test]
    fn serviceable_adapter_debt_runs_without_runtime_ingress() {
        let start = Instant::now();
        let initial = tag(0);
        let mut driver = FakeDriver::new(initial);
        driver.deferred_effects.push_back(vec![FakeEffect::other()]);
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(8, 2, 2));

        assert_eq!(runtime.queued_commands(), 0);
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.deferred_dispatches, 1);
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Idle)
        ));
    }

    #[test]
    fn real_adapter_signature_completion_precedes_deferred_timeout_and_newer_ingress() {
        let directory = TempDir::new().expect("temporary real-adapter ordering directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let start = Instant::now();
        runtime
            .arm_live_clocks(start)
            .expect("arm runtime after adapter startup");

        // Refresh the derived clock before the signer becomes busy. This keeps
        // the absolute deadline and retransmission deadline independent in the
        // ordering trace below.
        let before_timeout = start + Duration::from_secs(9);
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("service pre-fence retransmission"),
            RuntimeStep::Advanced(_)
        ));

        let proposal = signed_runtime_proposal(&context, &keys, 0xE1);
        runtime
            .enqueue_network(proposal.clone())
            .expect("enqueue authenticated proposal");
        let proposal_effects = match runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch authenticated proposal")
        {
            RuntimeStep::Advanced(effects) => effects,
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
        let (tag, manifest) = match proposal_effects.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };

        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("enqueue reconstructed body");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("dispatch reconstructed body"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
        ));
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("enqueue durable-body completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("dispatch durable-body completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));
        runtime
            .enqueue_validation_succeeded(
                tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            )
            .expect("enqueue validated-body completion");
        let (prepare_sign_tag, prepare_signature_preimage) = match runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch validated-body completion")
        {
            RuntimeStep::Advanced(effects) => match effects.as_slice() {
                [
                    AdapterEffect::Sign {
                        tag,
                        request: SignRequest::Vote(vote),
                    },
                ] if vote.phase == wire::GlobalPhase::Prepare
                    && vote.round == manifest.round
                    && vote.subject == manifest.subject =>
                {
                    (*tag, vote.signature_preimage())
                }
                effects => panic!("unexpected validation effects: {effects:?}"),
            },
            RuntimeStep::Idle => panic!("validation dispatch unexpectedly idle"),
        };

        // The body pipeline leaves the fair-ingress cursor at Progress. An
        // exact authenticated retransmission is consumed below the reducer
        // fence and advances that cursor normally, so Completion owns the
        // first slot once the signature and newer ingress arrive together.
        runtime
            .enqueue_network(proposal)
            .expect("enqueue exact authenticated retransmission");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("coalesce exact authenticated retransmission"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.ingress.next_class, CommandClass::Completion);

        let deadline = start + runtime.round_timeout();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("deliver absolute timeout through the real adapter"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert!(
            !runtime.driver().deferred_work_is_serviceable(),
            "the exact Prepare signature still fences the Busy-deferred timeout"
        );

        let prepare_signature = Signature::new(keys[0].private_key(), &prepare_signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(prepare_sign_tag, prepare_signature)
            .expect("enqueue exact Prepare signature completion");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0xE2))
            .expect("enqueue newer authenticated ingress");
        assert_eq!(runtime.queued_commands(), 2);

        let prepare_broadcast = runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("signature completion owns the first serialized turn");
        assert!(matches!(
            prepare_broadcast,
            RuntimeStep::Advanced(ref effects)
                if matches!(
                    effects.as_slice(),
                    [AdapterEffect::Broadcast(message)]
                        if matches!(
                            &message.payload,
                            wire::ConsensusMessageV2Payload::Vote(vote)
                                if vote.phase == wire::GlobalPhase::Prepare
                                    && vote.round == manifest.round
                                    && vote.subject == manifest.subject
                        )
                )
        ));
        assert_eq!(
            runtime.queued_commands(),
            1,
            "newer ingress remains owned after signature completion"
        );

        let timeout_macro_step = runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("service exactly one older Busy-deferred timeout transition");
        assert!(matches!(
            timeout_macro_step,
            RuntimeStep::Advanced(ref effects)
                if matches!(
                    effects.as_slice(),
                    [AdapterEffect::Sign {
                        request: SignRequest::TimeoutVote(vote),
                        ..
                    }] if vote.round == manifest.round
                )
        ));
        assert_eq!(
            runtime.queued_commands(),
            1,
            "one deferred macro-step cannot concatenate newer ingress"
        );

        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("dispatch newer ingress"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ReportEquivocation { .. }])
        ));
        assert_eq!(runtime.queued_commands(), 0);

        let next_retransmission = before_timeout + runtime.retransmit_interval();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(next_retransmission)
                .expect("make the next periodic scheduling decision"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.retransmit_started_at, next_retransmission);
    }

    #[test]
    fn round_timeout_grows_linearly_by_view_without_wrapping() {
        let base = Duration::from_secs(10);
        assert_eq!(round_timeout_for_view(base, 0), base);
        assert_eq!(round_timeout_for_view(base, 1), Duration::from_secs(20));
        assert_eq!(round_timeout_for_view(base, 7), Duration::from_secs(80));
        assert_eq!(
            round_timeout_for_view(Duration::new(1, 500_000_000), 1),
            Duration::from_secs(3),
        );

        assert_eq!(
            round_timeout_for_view(Duration::from_secs(1), u64::MAX - 1),
            Duration::from_secs(u64::MAX)
        );
        assert_eq!(
            round_timeout_for_view(Duration::from_secs(1), u64::MAX),
            Duration::MAX
        );
        assert_eq!(round_timeout_for_view(Duration::MAX, 1), Duration::MAX);
    }

    #[test]
    fn recovered_nonzero_view_uses_scaled_timeout_from_live_arm() {
        let constructed_at = Instant::now();
        let armed_at = constructed_at + Duration::from_secs(500);
        let recovered = tag(4);
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(recovered),
            constructed_at,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("open recovered runtime");

        runtime
            .arm_live_clocks(armed_at)
            .expect("arm after recovered startup");
        assert_eq!(runtime.round_timeout(), Duration::from_secs(50));
        let _ =
            runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(49));
        assert!(runtime.driver.timeouts.is_empty());
        let _ =
            runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(50));
        assert_eq!(runtime.driver.timeouts, vec![recovered]);
    }

    #[test]
    fn class_aware_ingress_is_bounded_and_reserves_progress_and_completion_slots() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(4, 1, 1),
        );
        assert_eq!(runtime.remaining_completion_capacity(), 4);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .unwrap();
        assert_eq!(runtime.remaining_completion_capacity(), 3);
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(2),
        )
        .unwrap();
        assert_eq!(runtime.remaining_completion_capacity(), 2);
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(99)
            ),
            Err(EnqueueError::ReservedCapacity)
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("reserved progress slot");
        assert_eq!(runtime.remaining_completion_capacity(), 1);
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(4),
        )
        .expect("reserved completion slot");
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        assert_eq!(runtime.queued_commands(), 4);
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Completion,
                FakeCommand::record(5)
            ),
            Err(EnqueueError::Full)
        );

        for offset in 0..4 {
            let _ = runtime
                .step_and_take_scheduler_ownership_for_test(start + Duration::from_millis(offset));
        }
        assert_eq!(
            runtime.driver.delivered,
            vec![(initial, 4), (initial, 3), (initial, 1), (initial, 2)]
        );
    }

    #[test]
    fn scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("normal owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(9),
        )
        .expect("progress owner fits");

        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Advanced(_))));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("FIFO dispatch retains exact scheduler ownership")
            .clone();
        assert_eq!(evidence.selected, RuntimeSelectedOwnerKind::Fifo);
        assert_eq!(evidence.round_tag, owner_tag);
        assert_eq!(evidence.queue_before.len, 2);
        assert_eq!(evidence.queue_after.len, 1);
        assert_eq!(
            evidence.queue_before.service_cursor,
            SERVICE_CLASS_COMPLETION
        );
        assert_eq!(evidence.queue_after.service_cursor, SERVICE_CLASS_NORMAL);
        assert_eq!(evidence.queue_before.max_service_debt, 0);
        assert_eq!(evidence.queue_after.max_service_debt, 1);
        assert!(evidence.live_mode);
        assert!(!evidence.timeout_due);
        assert!(!evidence.periodic_timer_due);
        assert!(evidence.fifo_ready);
        assert!(!evidence.completion_ready);
        assert!(evidence.progress_ready);
        assert!(evidence.normal_ready);
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &evidence.candidate else {
            panic!("FIFO dispatch must carry one exact command candidate");
        };
        assert_eq!(
            candidate.identity,
            FakeCommand::record(9).exact_runtime_command_identity()
        );
        assert_eq!(candidate.kind, RuntimeCommandKind::Test);
        assert_eq!(candidate.class, SERVICE_CLASS_PROGRESS);
        assert_eq!(candidate.tag, owner_tag);
        assert_eq!(candidate.admission_ordinal, 1);
        assert_eq!(candidate.fifo_position, 1);
        assert_eq!(candidate.eligible_skips_before, 0);
        assert_eq!(candidate.eligible_skips_after, 0);
        assert_eq!(evidence.validate_exact(), Ok(()));

        let rejected = |mutated: RuntimeSchedulerOwnershipEvidence| {
            assert_eq!(
                mutated.validate_exact(),
                Err(RuntimeSchedulerEvidenceError::InvalidProjection)
            );
        };

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.identity.canonical_bytes = Arc::<[u8]>::from(vec![0xFF]);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.kind = RuntimeCommandKind::Authenticated;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.class = SERVICE_CLASS_NORMAL;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.tag = tag(99);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.admission_ordinal = 0;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.fifo_position = 0;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_after.service_cursor = SERVICE_CLASS_COMPLETION;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_after.max_service_debt = 0;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.timeout_due = true;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.progress_ready = false;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.fifo_owed_after = true;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence;
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.eligible_skips_before = 1;
        rejected(mutated);
    }

    #[test]
    fn adapter_command_identity_is_derived_from_exact_immutable_payload() {
        let owner_tag = tag(4);
        let command = AdapterCommand::SignatureCompleted(vec![0x11, 0x22, 0x33]);
        let expected = command.exact_runtime_command_identity();
        let shared = expected.clone();
        assert!(Arc::ptr_eq(
            &expected.canonical_bytes,
            &shared.canonical_bytes
        ));
        assert_ne!(
            expected,
            AdapterCommand::SignatureCompleted(vec![0x11, 0x22, 0x34])
                .exact_runtime_command_identity()
        );

        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));
        ingress
            .enqueue(TaggedCommand::new(
                owner_tag,
                CommandClass::Completion,
                command,
                Instant::now(),
            ))
            .expect("exact adapter command fits completion capacity");
        let (_, candidate) = ingress
            .pop_next_with_ownership()
            .expect("adapter command retains its admission ordinal")
            .expect("adapter command owns the selected FIFO occurrence");
        assert_eq!(candidate.identity, expected);
        assert_eq!(candidate.kind, RuntimeCommandKind::SignatureCompleted);
        assert_eq!(candidate.class, SERVICE_CLASS_COMPLETION);
        assert_eq!(candidate.tag, owner_tag);
        assert_eq!(candidate.admission_ordinal, 0);
        assert_eq!(candidate.fifo_position, 0);
    }

    #[test]
    fn scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches() {
        let start = Instant::now();
        let owner_tag = tag(0);

        let mut idle = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        assert!(matches!(idle.step(start), Ok(RuntimeStep::Idle)));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::Idle)
        );
        assert!(idle.take_last_scheduler_ownership().is_some());

        assert!(matches!(
            idle.step(start + Duration::from_secs(2)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::PeriodicTimer)
        );
        assert!(idle.take_last_scheduler_ownership().is_some());
        assert!(matches!(
            idle.step(start + Duration::from_secs(10)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::Timeout)
        );

        let (mut recovery, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(owner_tag),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(6, 2, 1),
            Vec::new(),
        )
        .expect("construct unarmed recovery runtime");
        enqueue_fake(
            &mut recovery,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("recovery FIFO owner fits");
        assert!(matches!(
            recovery.step_recovery(start),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::RecoveryFifo)
        );
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .expect("recovery FIFO retains evidence")
                .validate_exact(),
            Ok(())
        );
        assert!(
            !recovery
                .last_scheduler_ownership()
                .expect("recovery FIFO retains evidence")
                .live_mode
        );
        assert!(recovery.take_last_scheduler_ownership().is_some());
        assert!(matches!(
            recovery.step_recovery(start),
            Ok(RuntimeStep::Idle)
        ));
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::RecoveryIdle)
        );
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .expect("recovery idle retains evidence")
                .validate_exact(),
            Ok(())
        );

        let mut deferred_driver = FakeDriver::new(owner_tag);
        deferred_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut deferred = runtime(deferred_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(deferred.step(start), Ok(RuntimeStep::Advanced(_))));
        let evidence = deferred
            .last_scheduler_ownership()
            .expect("deferred dispatch retains its typed occurrence");
        assert_eq!(evidence.selected, RuntimeSelectedOwnerKind::Deferred);
        assert_eq!(evidence.validate_exact(), Ok(()));
        assert!(matches!(
            &evidence.candidate,
            RuntimeSelectedCandidateOwnership::ExactDeferred(candidate)
                if candidate.service.admission_ordinal == 0
                    && candidate.service.validate_exact()
                    && candidate.ingress_ownership.is_none()
        ));

        let mut unavailable_driver = FakeDriver::new(owner_tag);
        unavailable_driver.deferred_identity_unavailable = true;
        unavailable_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut unavailable = runtime(unavailable_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(
            unavailable.step(start),
            Err(RuntimeError::FailClosed)
        ));
        assert!(unavailable.last_scheduler_ownership().is_none());
    }

    #[test]
    fn runtime_rejects_replayed_foreign_and_mutated_deferred_tokens() {
        let start = Instant::now();
        let owner_tag = tag(0);

        let mut replay_driver = FakeDriver::new(owner_tag);
        replay_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        replay_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let replayed = DeferredServiceEvidence::completion_for_test(
            &replay_driver.deferred_admission_ordinals,
            owner_tag,
            2,
            DeferredPriority::Completion,
        );
        assert!(replayed.claim_adapter_service_for_test());
        replay_driver
            .deferred_evidence_overrides
            .push_back(replayed.clone());
        replay_driver
            .deferred_evidence_overrides
            .push_back(replayed);
        let mut replay = runtime(replay_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(replay.step(start), Ok(RuntimeStep::Advanced(_))));
        assert!(replay.take_last_scheduler_ownership().is_some());
        assert!(matches!(replay.step(start), Err(RuntimeError::FailClosed)));

        let mut foreign_driver = FakeDriver::new(owner_tag);
        foreign_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let foreign_source = DeferredAdmissionOrdinalSource::new(0);
        let foreign_evidence = DeferredServiceEvidence::completion_for_test(
            &foreign_source,
            owner_tag,
            1,
            DeferredPriority::Completion,
        );
        assert!(foreign_evidence.claim_adapter_service_for_test());
        foreign_driver
            .deferred_evidence_overrides
            .push_back(foreign_evidence);
        let mut foreign = runtime(foreign_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(foreign.step(start), Err(RuntimeError::FailClosed)));

        let mut mutated_driver = FakeDriver::new(owner_tag);
        mutated_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut mutated = DeferredServiceEvidence::completion_for_test(
            &mutated_driver.deferred_admission_ordinals,
            owner_tag,
            1,
            DeferredPriority::Completion,
        );
        assert!(mutated.claim_adapter_service_for_test());
        mutated.protected_progress = true;
        mutated_driver
            .deferred_evidence_overrides
            .push_back(mutated);
        let mut mutated = runtime(mutated_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(mutated.step(start), Err(RuntimeError::FailClosed)));
    }

    #[test]
    fn scheduler_owner_must_be_taken_before_a_later_step_can_enter() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut blocked_runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );

        assert!(matches!(blocked_runtime.step(start), Ok(RuntimeStep::Idle)));
        let first_projection_hash = blocked_runtime
            .last_scheduler_ownership()
            .expect("first idle selection retains a carrier")
            .projection_hash;

        let periodic_at = start + blocked_runtime.retransmit_interval();
        assert!(matches!(
            blocked_runtime.step(periodic_at),
            Err(RuntimeError::FailClosed)
        ));
        assert_eq!(
            blocked_runtime.fail_closed_reason.as_deref(),
            Some("live scheduling began with an unconsumed scheduler owner")
        );
        blocked_runtime.latch_fail_closed("a later generic failure");
        assert_eq!(
            blocked_runtime.fail_closed_reason.as_deref(),
            Some("live scheduling began with an unconsumed scheduler owner"),
            "fail-closed diagnostics retain the first invariant violation"
        );
        let retained = blocked_runtime
            .last_scheduler_ownership()
            .expect("failed re-entry preserves the first unconsumed carrier");
        assert_eq!(retained.selected, RuntimeSelectedOwnerKind::Idle);
        assert_eq!(retained.projection_hash, first_projection_hash);

        let mut runtime = self::runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Idle)));

        let taken = runtime
            .take_last_scheduler_ownership()
            .expect("effect boundary takes the exact first occurrence");
        assert_eq!(taken.selected, RuntimeSelectedOwnerKind::Idle);
        assert_eq!(taken.validate_exact(), Ok(()));
        assert!(runtime.last_scheduler_ownership().is_none());

        assert!(matches!(
            runtime.step(periodic_at),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::PeriodicTimer)
        );
        assert!(runtime.last_scheduler_ownership().is_none());
    }

    #[test]
    fn admission_ordinal_exhaustion_fails_runtime_closed() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        runtime.ingress.next_admission_ordinal = Some(u128::MAX - 1);
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("the last ordinal with a representable successor is valid");
        assert_eq!(
            runtime.ingress.commands[0].admission_ordinal,
            Some(u128::MAX - 1)
        );
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(2),
            ),
            Err(EnqueueError::FailClosed)
        );
        assert!(runtime.fail_closed);
    }

    #[test]
    fn selected_owner_without_a_runtime_minted_ordinal_fails_closed() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        runtime.ingress.commands.push_back(TaggedCommand::new(
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
            start,
        ));

        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert!(runtime.fail_closed);
        assert!(runtime.last_scheduler_ownership().is_none());
    }

    #[test]
    fn class_cursor_advances_from_the_served_class_after_empty_classes() {
        let admitted_at = Instant::now();
        let initial = tag(0);
        let queued = |class, value| {
            TaggedCommand::new(initial, class, FakeCommand::record(value), admitted_at)
        };
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));

        ingress
            .enqueue(queued(CommandClass::Normal, 1))
            .expect("normal command fits the bounded ingress");
        let first = ingress.pop_next().expect("normal class is reachable");
        assert_eq!(first.command.record, Some(1));
        assert_eq!(ingress.next_class, CommandClass::Completion);

        ingress
            .enqueue(queued(CommandClass::Normal, 2))
            .expect("second normal command fits the bounded ingress");
        ingress
            .enqueue(queued(CommandClass::Completion, 3))
            .expect("completion reserve remains available");
        let second = ingress.pop_next().expect("completion class is selected");
        assert_eq!(second.command.record, Some(3));
        assert_eq!(ingress.next_class, CommandClass::Progress);

        let third = ingress
            .pop_next()
            .expect("empty progress class is skipped to normal");
        assert_eq!(third.command.record, Some(2));
        assert_eq!(ingress.next_class, CommandClass::Completion);
    }

    #[test]
    fn production_ingress_pop_uses_shared_selector_for_every_ready_mask() {
        let admitted_at = Instant::now();
        let initial = tag(0);
        for cursor in [
            CommandClass::Completion,
            CommandClass::Progress,
            CommandClass::Normal,
        ] {
            for ready_mask in 0u8..8 {
                let completion_ready = ready_mask & 0b001 != 0;
                let progress_ready = ready_mask & 0b010 != 0;
                let normal_ready = ready_mask & 0b100 != 0;
                let expected = select_bounded_service_class(
                    cursor.service_code(),
                    completion_ready,
                    progress_ready,
                    normal_ready,
                );
                let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
                ingress.next_class = cursor;
                for (class, ready) in [
                    (CommandClass::Normal, normal_ready),
                    (CommandClass::Progress, progress_ready),
                    (CommandClass::Completion, completion_ready),
                ] {
                    if ready {
                        ingress
                            .enqueue(TaggedCommand::new(
                                initial,
                                class,
                                FakeCommand::record(class.service_code()),
                                admitted_at,
                            ))
                            .expect("one command per ready class fits reserved ingress");
                    }
                }

                let selected = ingress.pop_next();
                assert_eq!(
                    selected.as_ref().and_then(|queued| queued.command.record),
                    (expected.selected != SERVICE_CLASS_NONE).then_some(expected.selected),
                );
                assert_eq!(ingress.next_class.service_code(), expected.next);
            }
        }
    }

    #[test]
    fn healthy_same_class_fifo_depth_does_not_accrue_service_debt() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        for id in 0..4 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(id),
            )
            .expect("enqueue same-class work");
        }

        let _ = runtime.step(start);
        let queue = runtime.queue_snapshot(start);
        assert_eq!(queue.normal.depth, 3);
        assert_eq!(queue.normal.max_service_debt, 0);
    }

    #[test]
    fn canonical_body_completion_prunes_only_conflicting_queued_proposals() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"queued-body-context",
            ))),
            height: 7,
            view: 2,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"queued-body-block")),
            payload_hash: Hash::new(b"queued-body-payload"),
        };
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let canonical = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout,
            chunk_hashes: vec![Hash::new(b"canonical chunk")],
            chunk_root: Hash::new(b"canonical root"),
        };
        let conflicting = wire::PayloadManifest {
            chunk_hashes: vec![Hash::new(b"conflicting chunk")],
            chunk_root: Hash::new(b"conflicting root"),
            ..canonical.clone()
        };
        let other_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"other queued block")),
            payload_hash: Hash::new(b"other queued payload"),
            ..subject
        };
        let other = wire::PayloadManifest {
            subject: other_subject,
            ..conflicting.clone()
        };

        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 1, 1));
        for (command_tag, manifest) in [
            (tag(0), conflicting.clone()),
            (tag(1), canonical.clone()),
            (tag(2), other.clone()),
        ] {
            ingress
                .enqueue(TaggedCommand::new(
                    command_tag,
                    CommandClass::Normal,
                    AdapterCommand::Authenticated(authenticated_proposal_for_test(manifest)),
                    Instant::now(),
                ))
                .expect("queue authenticated proposal");
        }

        ingress
            .enqueue_canonical_body_available(tag(3), canonical.clone())
            .expect("trusted completion prunes its conflicting proposal and appends in FIFO order");
        assert_eq!(ingress.len(), 3);
        assert!(
            ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(
                conflicting
            ))
        );
        assert!(
            !ingress
                .conflicts_with_pending_body_available(&authenticated_proposal_for_test(canonical))
        );
        assert!(
            !ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(other))
        );

        let retained_tags = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(retained_tags, vec![tag(1), tag(2), tag(3)]);
        let committed = ingress
            .commands
            .back()
            .expect("canonical completion remains at the queue tail");
        assert_eq!(committed.tag, tag(3));
        assert_eq!(committed.class, CommandClass::Completion);
        assert_eq!(committed.admission_ordinal, Some(3));
        assert!(matches!(
            ingress.commands.back().map(|queued| &queued.command),
            Some(AdapterCommand::BodyAvailable { manifest }) if manifest.subject == subject
        ));
    }

    #[test]
    fn retiring_exact_body_completion_releases_a_capacity_one_ingress_slot() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"retired-body-context",
            ))),
            height: 11,
            view: 4,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"retired-body-block")),
            payload_hash: Hash::new(b"retired-body-payload"),
        };
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let original = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout,
            chunk_hashes: vec![Hash::new(b"retired chunk")],
            chunk_root: Hash::new(b"retired root"),
        };
        let replacement = wire::PayloadManifest {
            round: wire::ConsensusRound {
                view: round.view + 1,
                ..round
            },
            chunk_hashes: vec![Hash::new(b"replacement chunk")],
            chunk_root: Hash::new(b"replacement root"),
            ..original.clone()
        };
        let original_tag = tag(4);
        let replacement_tag = tag(5);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(1, 0, 0));

        ingress
            .enqueue_canonical_body_available(original_tag, original.clone())
            .expect("the original completion claims the sole slot");
        assert_eq!(
            ingress.enqueue_canonical_body_available(replacement_tag, replacement.clone()),
            Err(EnqueueError::Full)
        );
        assert_eq!(
            ingress.retire_canonical_body_available(original_tag, &original),
            1
        );
        assert_eq!(ingress.remaining_capacity(), 1);
        ingress
            .enqueue_canonical_body_available(replacement_tag, replacement.clone())
            .expect("retirement releases the sole completion slot");
        assert_eq!(ingress.len(), 1);
        assert!(matches!(
            ingress.commands.front(),
            Some(TaggedCommand {
                tag,
                command: AdapterCommand::BodyAvailable { manifest },
                ..
            }) if *tag == replacement_tag && manifest == &replacement
        ));
    }

    #[test]
    fn exact_authenticated_progress_retransmission_is_queue_coalesced() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"coalesced-progress-context",
            ))),
            height: 7,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"coalesced-progress-block")),
            payload_hash: Hash::new(b"coalesced-progress-payload"),
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"coalesced parent state"),
            Hash::new(b"coalesced post state"),
            Hash::new(b"coalesced ordinary writes"),
            Hash::new(b"coalesced executed block wire"),
        );
        let payload = wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        });
        let authenticated = || {
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload.clone()))
        };
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));

        assert_eq!(
            ingress
                .enqueue_authenticated(tag(0), CommandClass::Progress, authenticated())
                .expect("first authenticated CommitQC owns one queue slot"),
            tag(0)
        );
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(1), CommandClass::Progress, authenticated())
                .expect("equal authenticated retransmission is coalesced"),
            tag(0),
            "a coalesced retransmission returns the original queue owner's tag"
        );
        assert_eq!(ingress.len(), 1);

        let dispatched = ingress
            .pop_next()
            .expect("the sole queued CommitQC is dispatchable");
        assert_eq!(dispatched.class, CommandClass::Progress);
        assert!(matches!(
            dispatched.command,
            AdapterCommand::Authenticated(_)
        ));
        assert_eq!(ingress.len(), 0);

        assert_eq!(
            ingress
                .enqueue_authenticated(tag(2), CommandClass::Progress, authenticated())
                .expect("a later retransmission starts a new ownership interval"),
            tag(2)
        );
        assert_eq!(ingress.len(), 1);
    }

    #[test]
    fn runtime_merges_alternate_sources_for_one_semantic_request() {
        let directory = TempDir::new().expect("temporary alternate-source runtime directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
        let message = signed_runtime_proposal(&context, &keys, 0x76);
        let semantic_origin = PeerId::new(keys[0].public_key().clone());
        let source_a = PeerId::new(keys[1].public_key().clone());
        let source_b = PeerId::new(keys[2].public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(source_a.clone(), 2);
        let route_a = routes.mint_via(semantic_origin.clone(), source_a.clone());
        let route_b = routes.mint_via(semantic_origin.clone(), source_b.clone());
        let ownership_a = fair_runtime_ownership_with_reply_route(
            &message,
            semantic_origin.clone(),
            source_a,
            route_a.clone(),
        );
        let ownership_b = fair_runtime_ownership_with_reply_route(
            &message,
            semantic_origin,
            source_b,
            route_b.clone(),
        );

        let owner_tag = runtime
            .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
            .expect("first source admits the semantic request");
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(message, ownership_b)
                .expect("alternate source attaches to the retained request"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 1);
        let ownership = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("coalesced runtime command retains exact source ownership");
        assert!(ownership.validate_exact());
        let projection_hash = ownership.projection_hash;
        let direct = ownership
            .direct
            .first()
            .expect("proposal retains direct fair-ingress ownership");
        assert_eq!(
            direct
                .current_reply_routes()
                .expect("route-aware fair ownership")
                .len(),
            2
        );
        assert!(routes.retire(&route_a));
        let ownership = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("queued ownership survives a normal source disconnect");
        assert!(ownership.validate_exact());
        assert_eq!(
            ownership.projection_hash, projection_hash,
            "connection liveness is not part of immutable runtime ownership identity"
        );
        assert!(
            ownership
                .direct
                .first()
                .and_then(FairV2IngressOwnershipEvidence::current_reply_routes)
                .is_some_and(|owned| {
                    owned.iter().any(|route| route.same_delivery(&route_a))
                        && owned.iter().any(|route| route.same_delivery(&route_b))
                }),
            "retirement is applied only by an authoritative prune receipt"
        );
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent() {
        let directory = TempDir::new().expect("temporary distinct-origin runtime directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
        let message = signed_runtime_proposal(&context, &keys, 0x77);
        let origin_a = PeerId::new(keys[0].public_key().clone());
        let origin_b = PeerId::new(keys[1].public_key().clone());
        let source = PeerId::new(keys[2].public_key().clone());
        let ownership_a = fair_runtime_ownership(&message, origin_a, source.clone());
        let ownership_b = fair_runtime_ownership(&message, origin_b, source);

        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
            .expect("first semantic origin owns one runtime occurrence");
        runtime
            .enqueue_network_with_ingress_ownership(message, ownership_b)
            .expect("distinct semantic origin retains an independent occurrence");
        assert_eq!(runtime.queued_commands(), 2);
        assert!(runtime.ingress.commands.iter().all(|queued| {
            queued
                .ingress_ownership
                .as_ref()
                .is_some_and(RuntimeIngressOwnershipEvidence::validate_exact)
        }));
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn busy_deferred_request_merges_alternate_source_and_services_exact_carrier() {
        let directory = TempDir::new().expect("temporary Busy-deferred ownership directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 2, 2),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before authenticated ingress");
        let round_tag = runtime.round_tag();
        let timeout_effects = runtime
            .driver
            .timeout_elapsed(round_tag)
            .expect("install a local signing fence")
            .into_effects();
        let (signature_tag, signature_preimage) = match timeout_effects.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(vote),
                },
            ] => (*tag, vote.signature_preimage()),
            effects => panic!("unexpected timeout effects: {effects:?}"),
        };

        let message = signed_runtime_proposal(&context, &keys, 0x78);
        let semantic_origin = PeerId::new(keys[0].public_key().clone());
        let ownership_a = fair_runtime_ownership(
            &message,
            semantic_origin.clone(),
            PeerId::new(keys[1].public_key().clone()),
        );
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), ownership_a)
            .expect("first source enters runtime ingress");
        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let queued_owner = runtime
            .take_last_scheduler_ownership()
            .expect("Busy dispatch retains its exact queue owner");
        assert!(queued_owner.validate_exact().is_ok());
        assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
        let admission_ordinal = *runtime
            .deferred_ingress_ownership
            .keys()
            .next()
            .expect("authenticated Busy owner has an actor-global ordinal");
        let projection_before_alternate =
            runtime.deferred_ingress_ownership[&admission_ordinal].projection_hash;

        let ownership_b = fair_runtime_ownership(
            &message,
            semantic_origin,
            PeerId::new(keys[2].public_key().clone()),
        );
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(message, ownership_b)
                .expect("alternate source attaches to the Busy owner"),
            round_tag
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert_ne!(
            runtime.deferred_ingress_ownership[&admission_ordinal].projection_hash,
            projection_before_alternate,
            "alternate ownership history must change the exact runtime projection"
        );

        let signature = Signature::new(keys[0].private_key(), &signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(signature_tag, signature)
            .expect("enqueue the exact signing completion");
        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects))
                if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
        ));
        assert!(runtime.take_last_scheduler_ownership().is_some());

        let deferred_effects = match runtime.step(now) {
            Ok(RuntimeStep::Advanced(effects)) => effects,
            other => panic!("deferred owner did not receive its service turn: {other:?}"),
        };
        assert!(
            deferred_effects.is_empty()
                || matches!(
                    deferred_effects.as_slice(),
                    [AdapterEffect::FetchBody { .. }]
                ),
            "the timeout intent may obsolete the proposal, but no unrelated effect may replace it: {deferred_effects:?}"
        );
        let deferred_owner = runtime
            .take_last_scheduler_ownership()
            .expect("deferred service hands off its exact owner");
        let RuntimeSelectedCandidateOwnership::ExactDeferred(deferred) = &deferred_owner.candidate
        else {
            panic!("expected exact deferred scheduler ownership")
        };
        assert!(
            deferred
                .ingress_ownership
                .as_ref()
                .is_some_and(RuntimeIngressOwnershipEvidence::validate_exact)
        );
        assert!(runtime.deferred_ingress_ownership.is_empty());
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn exact_authenticated_qc_from_distinct_sources_coalesces_in_one_runtime_slot() {
        let directory = TempDir::new().expect("temporary multi-source QC directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let owner_tag = runtime.round_tag();
        let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC7);
        let message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
        );
        let first_source = PeerId::new(keys[0].public_key().clone());
        let second_source = PeerId::new(keys[1].public_key().clone());

        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, first_source),
                )
                .expect("the first authenticated carrier owns the runtime command"),
            owner_tag
        );
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, second_source),
                )
                .expect("an exact QC from another source coalesces"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 1);

        let retained = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the queued QC retains fair-ingress ownership");
        assert!(retained.validate_exact());
        assert_eq!(retained.direct.len(), 2);
        assert!(retained.commit_certificate_response.is_empty());
        assert_ne!(
            retained.direct[0].process_local_projection_hash(),
            retained.direct[1].process_local_projection_hash(),
            "route-free carrier projections must retain their distinct source identities"
        );

        let mut source_substituted = retained.clone();
        let substituted_source = PeerId::from(KeyPair::random().public_key().clone());
        source_substituted.direct[0].first.wire_key.origin = Some(substituted_source.clone());
        source_substituted.direct[0].first.semantic_origin = Some(substituted_source.clone());
        source_substituted.direct[0].first.authenticated_via = Some(substituted_source.clone());
        source_substituted.direct[0].first.authenticated_source =
            super::super::FairV2IngressSource::Validator(substituted_source.clone());
        source_substituted.direct[0].first.semantic_owner_source =
            super::super::FairV2IngressSource::Validator(substituted_source.clone());
        source_substituted.direct[0].latest.wire_key.origin = Some(substituted_source.clone());
        source_substituted.direct[0].latest.semantic_origin = Some(substituted_source.clone());
        source_substituted.direct[0].latest.authenticated_via = Some(substituted_source.clone());
        source_substituted.direct[0].latest.authenticated_source =
            super::super::FairV2IngressSource::Validator(substituted_source.clone());
        source_substituted.direct[0].latest.semantic_owner_source =
            super::super::FairV2IngressSource::Validator(substituted_source);
        assert!(source_substituted.direct[0].validate_exact());
        assert!(
            !source_substituted.validate_exact(),
            "the retained runtime projection must reject an otherwise exact source substitution"
        );

        let mut reordered = retained.clone();
        reordered.direct.reverse();
        assert!(
            !reordered.validate_exact(),
            "the retained runtime projection must reject carrier-order mutation"
        );
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn same_semantic_qc_with_conflicting_route_authority_fails_closed_atomically() {
        let directory = TempDir::new().expect("temporary conflicting route directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC8);
        let message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
        );
        let source = PeerId::new(keys[0].public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(source.clone());
        let first_route = routes.mint(source.clone());
        let conflicting_route = routes
            .forge_equal_ordinal_different_tenure(&first_route, source.clone(), source.clone())
            .expect("fixture owns the conflicting route authority");

        let first_ownership = fair_network_ownership_with_route(
            &message,
            source.clone(),
            source.clone(),
            first_route.clone(),
        );
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), first_ownership.clone())
            .expect("the first exact route owns the authenticated QC");
        let retained_before = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the queued QC retains its first route")
            .clone();

        let mut conflicting_ownership = first_ownership;
        conflicting_ownership
            .latest
            .attempts_after
            .first_mut()
            .expect("routed ownership retains one reply attempt")
            .route = conflicting_route;
        assert!(!conflicting_ownership.validate_exact());
        assert!(matches!(
            runtime.enqueue_network_with_ingress_ownership(message.clone(), conflicting_ownership),
            Err(NetworkIngressError::FailClosed)
        ));
        let retained_after = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("failed merge preserves the first exact route");
        assert_eq!(retained_after, &retained_before);
        assert_eq!(retained_after.direct.len(), 1);
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("network ingress changed its authenticated fair-queue ownership")
        );
    }

    #[test]
    fn runtime_ingress_carrier_capacity_returns_backpressure_atomically() {
        let directory = TempDir::new().expect("temporary carrier-capacity directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC9);
        let message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
        );
        let carrier = || {
            let source = PeerId::from(KeyPair::random().public_key().clone());
            fair_network_ownership(&message, source)
        };
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), carrier())
            .expect("the first disjoint carrier owns the authenticated QC");
        for _ in 1..MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM {
            let candidate = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, carrier())
                .expect("independent fair-ingress carrier is exact");
            runtime
                .ingress
                .commands
                .front_mut()
                .and_then(|queued| queued.ingress_ownership.as_mut())
                .expect("the queued QC retains its carrier set")
                .merge_downstream(candidate)
                .expect("every protocol-bounded carrier remains exact");
        }
        let retained = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the queued QC retains the full carrier set");
        assert_eq!(retained.direct.len(), MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM);
        let retained_before = retained.clone();
        let excess_carrier = carrier();

        assert!(matches!(
            runtime.enqueue_network_with_ingress_ownership(message, excess_carrier),
            Err(NetworkIngressError::Backpressure(EnqueueError::Full))
        ));
        let retained_after = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("backpressure preserves the full exact carrier set");
        assert_eq!(retained_after, &retained_before);
        assert!(retained_after.validate_exact());
        assert!(!runtime.fail_closed);
        assert!(runtime.fail_closed_reason.is_none());
    }

    #[test]
    fn exact_authenticated_retransmission_preserves_capacity_fifo_and_cursor() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"coalesced-capacity-context",
            ))),
            height: 9,
            view: 4,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"coalesced-capacity-block")),
            payload_hash: Hash::new(b"coalesced-capacity-payload"),
        };
        let payload = |signature| {
            wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: wire::ExecutionCommitment::without_topups(
                    Hash::new(b"capacity parent state"),
                    Hash::new(b"capacity post state"),
                    Hash::new(b"capacity ordinary writes"),
                    Hash::new(b"capacity executed block wire"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![signature],
            })
        };
        let authenticated = |signature| {
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload(
                signature,
            )))
        };
        let queued_wire = wire::ConsensusMessageV2::new(payload(1));
        let transport = wire::ConsensusMessageV2Payload::PayloadManifest(wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1,
                max_chunk_count: 1,
            },
            chunk_hashes: vec![Hash::new(b"coalesced capacity chunk")],
            chunk_root: Hash::new(b"coalesced capacity root"),
        });
        assert!(matches!(
            classify_reducer_network_ingress(false, &queued_wire.payload),
            Ok(CommandClass::Progress)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(false, &transport),
            Err(NetworkIngressError::TransportPayload)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(true, &queued_wire.payload),
            Err(NetworkIngressError::FailClosed)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(true, &transport),
            Err(NetworkIngressError::FailClosed)
        ));
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));

        assert_eq!(
            ingress
                .enqueue_authenticated(tag(0), CommandClass::Normal, authenticated(1))
                .expect("first wire value enters below the normal boundary"),
            tag(0)
        );
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(1), CommandClass::Normal, authenticated(2))
                .expect("a non-identical wire value uses ordinary capacity"),
            tag(1)
        );
        assert_eq!(
            ingress.check_capacity(CommandClass::Normal),
            Err(EnqueueError::ReservedCapacity)
        );

        let cursor_before = ingress.next_class;
        let tags_before = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(8), CommandClass::Normal, authenticated(1))
                .expect("an exact duplicate coalesces at reserved capacity"),
            tag(0),
            "coalescing deterministically returns the original admission tag"
        );
        assert_eq!(ingress.next_class, cursor_before);
        assert_eq!(
            ingress
                .commands
                .iter()
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            tags_before,
            "coalescing changes neither FIFO ownership nor its tags"
        );
        assert_eq!(
            ingress.enqueue_authenticated(tag(9), CommandClass::Normal, authenticated(3)),
            Err(EnqueueError::ReservedCapacity),
            "a non-identical envelope still obeys the normal boundary"
        );

        ingress
            .enqueue_authenticated(tag(2), CommandClass::Progress, authenticated(3))
            .expect("progress reserve remains independent");
        ingress
            .enqueue_authenticated(tag(3), CommandClass::Completion, authenticated(4))
            .expect("completion reserve fills the final slot");
        assert_eq!(ingress.len(), 4);
        assert_eq!(
            ingress.check_capacity(CommandClass::Completion),
            Err(EnqueueError::Full)
        );
        assert_eq!(ingress.authenticated_wire_tag(&queued_wire), Some(tag(0)));
        assert!(
            ingress
                .check_authenticated_wire_capacity(&queued_wire, CommandClass::Normal, false,)
                .is_ok(),
            "raw equality only opens the authentication attempt at full capacity"
        );
        assert_eq!(
            ingress.check_authenticated_wire_capacity(
                &wire::ConsensusMessageV2::new(payload(5)),
                CommandClass::Normal,
                false,
            ),
            Err(EnqueueError::Full)
        );

        let full_tags = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(10), CommandClass::Normal, authenticated(1))
                .expect("the exact envelope coalesces even when every slot is owned"),
            tag(0)
        );
        assert_eq!(ingress.next_class, cursor_before);
        assert_eq!(
            ingress
                .commands
                .iter()
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            full_tags
        );
        assert!(
            ingress
                .commands
                .iter()
                .all(|queued| queued.eligible_skips == 0)
        );
        assert_eq!(
            ingress.enqueue_authenticated(tag(11), CommandClass::Progress, authenticated(5)),
            Err(EnqueueError::Full),
            "wire inequality cannot inherit the duplicate's full-queue exception"
        );
    }

    #[test]
    fn completion_retries_coalesce_across_ingress_and_busy_deferred_ownership() {
        let directory = TempDir::new().expect("temporary completion-coalescing directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let receipts = |manifest: &wire::PayloadManifest| {
            let durable = DurableBodyReceipt::for_test(
                context.id(),
                manifest.round,
                manifest.subject,
                HashOf::new(manifest),
            );
            let validated = ValidatedBodyReceipt::for_test(durable.clone());
            (durable, validated)
        };

        let ingress_manifest = runtime_manifest(&context, 0x91);
        let (durable, _) = receipts(&ingress_manifest);
        runtime
            .enqueue_body_stored(
                owner_tag,
                ingress_manifest.round,
                ingress_manifest.subject,
                durable.clone(),
            )
            .expect("enqueue the first durable-store completion");
        runtime
            .enqueue_body_stored(
                owner_tag,
                ingress_manifest.round,
                ingress_manifest.subject,
                durable,
            )
            .expect("an exact retransmission coalesces in runtime ingress");
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    ingress_manifest.round,
                    ingress_manifest.subject,
                )
                .expect("retire the one coalesced ingress owner"),
            RetiredBodyPipelineCompletions {
                body_available: 0,
                body_stored: 1,
                validation: 0,
                local_proposal: 0,
            }
        );

        let deferred_store = runtime_manifest(&context, 0x92);
        let (durable, _) = receipts(&deferred_store);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_store,
                DeferredBodyPipelineStageForTest::BodyStored,
            )
            .expect("stage a Busy-deferred durable-store completion");
        runtime
            .enqueue_body_stored(
                owner_tag,
                deferred_store.round,
                deferred_store.subject,
                durable,
            )
            .expect("a retransmit coalesces with the Busy-deferred store owner");
        assert_eq!(runtime.queued_commands(), 0);

        let deferred_validation = runtime_manifest(&context, 0x93);
        let (_, validated) = receipts(&deferred_validation);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_validation,
                DeferredBodyPipelineStageForTest::ValidationSucceeded,
            )
            .expect("stage a Busy-deferred validation completion");
        runtime
            .enqueue_validation_succeeded(
                owner_tag,
                deferred_validation.round,
                deferred_validation.subject,
                validated,
            )
            .expect("a retransmit coalesces with the Busy-deferred validation owner");
        assert_eq!(runtime.queued_commands(), 0);

        let deferred_proposal = runtime_manifest(&context, 0x94);
        let (durable, validated) = receipts(&deferred_proposal);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_proposal,
                DeferredBodyPipelineStageForTest::LocalProposalReady,
            )
            .expect("stage a Busy-deferred local-proposal completion");
        runtime
            .enqueue_local_proposal(owner_tag, deferred_proposal.clone(), durable, validated)
            .expect("a retransmit coalesces with the Busy-deferred proposal owner");
        assert_eq!(runtime.queued_commands(), 0);

        for manifest in [deferred_store, deferred_validation, deferred_proposal] {
            runtime
                .retire_body_pipeline_completions(owner_tag, manifest.round, manifest.subject)
                .expect("each coalesced Busy-deferred pipeline has one exact owner");
        }
    }

    #[test]
    fn body_available_rebind_rejects_uninstalled_destination_without_mutation() {
        let directory = TempDir::new().expect("temporary uninstalled-rebind directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let source_tag = runtime.round_tag();
        let fabricated = EventTag::new(
            source_tag.height(),
            source_tag.view() + 1,
            Generation::new(source_tag.generation().get() + 1),
        );
        let manifest = runtime_manifest(&context, 0x8B);
        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue unique source owner");

        assert_eq!(
            runtime
                .rebind_body_available(source_tag, fabricated, &manifest)
                .expect_err("an uninstalled destination tag must be rejected"),
            "Sumeragi v2 body completion rebind target is not the installed runtime incarnation"
        );
        assert!(
            !runtime.fail_closed,
            "caller contract rejection is recoverable"
        );
        assert_eq!(runtime.round_tag(), source_tag);
        assert_eq!(runtime.queued_commands(), 1);
        assert!(matches!(
            runtime.ingress.commands.front(),
            Some(TaggedCommand {
                tag,
                command: AdapterCommand::BodyAvailable {
                    manifest: queued_manifest,
                },
                ..
            }) if *tag == source_tag && queued_manifest == &manifest
        ));
        assert!(
            runtime
                .retire_body_available(source_tag, &manifest)
                .expect("the untouched source owner remains retireable")
        );
        assert_eq!(runtime.queued_commands(), 0);
    }

    #[test]
    fn body_available_rebind_coalesces_exact_busy_deferred_destination_owner() {
        let directory = TempDir::new().expect("temporary destination-coalescing directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for production dispatch");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0x8C))
            .expect("enqueue authenticated proposal");
        let proposal_effects = match runtime.step(now).expect("dispatch proposal") {
            RuntimeStep::Advanced(effects) => effects,
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("proposal dispatch publishes exact scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
        let (source_tag, manifest) = match proposal_effects.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };

        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue body reconstruction completion");
        assert!(matches!(
            runtime.step(now).expect("dispatch body reconstruction"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("body reconstruction publishes exact scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        runtime
            .enqueue_body_stored(
                source_tag,
                manifest.round,
                manifest.subject,
                durable.clone(),
            )
            .expect("enqueue durable-store completion");
        assert!(matches!(
            runtime.step(now).expect("dispatch durable-store completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("durable-store completion publishes exact scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
        runtime
            .enqueue_validation_succeeded(
                source_tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            )
            .expect("enqueue validation completion");
        assert!(matches!(
            runtime.step(now).expect("dispatch validation completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::Sign { .. }])
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("validation completion publishes exact scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );

        let rebound = EventTag::new(
            source_tag.height(),
            source_tag.view() + 1,
            Generation::new(source_tag.generation().get() + 1),
        );
        assert!(
            runtime
                .driver
                .body_available(source_tag, manifest.clone())
                .expect("stage exact completion behind the signer fence")
                .into_effects()
                .is_empty()
        );
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        assert_eq!(
            runtime
                .driver
                .deferred_body_pipeline_completion_ownership(source_tag, &evidence),
            (1, 1),
            "the current tag owns the real Busy-deferred completion"
        );
        observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
        assert_eq!(
            runtime
                .driver
                .rebind_deferred_body_available(source_tag, rebound, &manifest),
            1,
            "the seam models an exact destination owner already transferred by another path"
        );
        assert_eq!(
            runtime
                .driver
                .deferred_body_pipeline_completion_ownership(rebound, &evidence),
            (1, 1),
            "the destination must be owned by the real Busy-deferred lane"
        );
        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue the unique source owner in runtime ingress");
        assert_eq!(runtime.queued_commands(), 1);

        assert!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect("exact destination ownership coalesces the source")
        );
        assert!(!runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 0, "the source owner was retired");
        assert_eq!(
            runtime
                .driver
                .deferred_body_pipeline_completion_ownership(rebound, &evidence),
            (1, 1),
            "coalescing retains exactly one destination owner"
        );
        assert!(
            !runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect("an idempotent retry finds no remaining source owner")
        );
        let same_view_rebound = EventTag::new(
            rebound.height(),
            rebound.view(),
            Generation::new(rebound.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut runtime, rebound, same_view_rebound, &manifest);
        assert!(
            runtime
                .rebind_body_available(rebound, same_view_rebound, &manifest)
                .expect("same-view generation supersession transfers the Busy-deferred owner")
        );
        assert_eq!(
            runtime
                .driver
                .deferred_body_pipeline_completion_ownership(same_view_rebound, &evidence),
            (1, 1),
            "same-view rebinding leaves exactly one Busy-deferred destination"
        );
        assert!(
            runtime
                .retire_body_available(same_view_rebound, &manifest)
                .expect("the unique destination owner remains retireable")
        );
    }

    #[test]
    fn body_available_rebind_destination_conflicts_and_duplicates_fail_closed_before_mutation() {
        {
            let directory = TempDir::new().expect("temporary destination-conflict directory");
            let (mut runtime, context, _keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let source_tag = runtime.round_tag();
            let rebound = EventTag::new(
                source_tag.height(),
                source_tag.view() + 1,
                Generation::new(source_tag.generation().get() + 1),
            );
            let manifest = runtime_manifest(&context, 0x8D);
            observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
            let mut conflicting = manifest.clone();
            conflicting.chunk_hashes[0] = Hash::new(b"conflicting rebound chunk");
            conflicting.chunk_root = Hash::new(b"conflicting rebound root");
            runtime
                .enqueue_body_available(source_tag, manifest.clone())
                .expect("enqueue unique source owner");
            runtime
                .ingress
                .enqueue_canonical_body_available(rebound, conflicting.clone())
                .expect("test seam stages conflicting destination evidence");

            assert_eq!(
                runtime
                    .rebind_body_available(source_tag, rebound, &manifest)
                    .expect_err("conflicting destination evidence must fail closed"),
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
            );
            assert!(runtime.fail_closed);
            assert_eq!(runtime.queued_commands(), 2);
            assert!(runtime.ingress.commands.iter().any(|queued| matches!(
                &queued.command,
                AdapterCommand::BodyAvailable { manifest: queued_manifest }
                    if queued.tag == source_tag && queued_manifest == &manifest
            )));
            assert!(runtime.ingress.commands.iter().any(|queued| matches!(
                &queued.command,
                AdapterCommand::BodyAvailable { manifest: queued_manifest }
                    if queued.tag == rebound && queued_manifest == &conflicting
            )));
            assert_eq!(
                runtime
                    .rebind_body_available(source_tag, rebound, &manifest)
                    .expect_err("fail-closed runtime rejects a second conflicting rebind"),
                "Sumeragi v2 runtime is fail-closed"
            );
            assert_eq!(
                runtime.enqueue_application_completed(source_tag, manifest.subject),
                Err(EnqueueError::FailClosed)
            );
            assert!(matches!(
                runtime.step(Instant::now()),
                Err(RuntimeError::FailClosed)
            ));
        }

        {
            let directory = TempDir::new().expect("temporary destination-duplicate directory");
            let (mut runtime, context, _keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let source_tag = runtime.round_tag();
            let rebound = EventTag::new(
                source_tag.height(),
                source_tag.view() + 1,
                Generation::new(source_tag.generation().get() + 1),
            );
            let manifest = runtime_manifest(&context, 0x8E);
            observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
            runtime
                .enqueue_body_available(source_tag, manifest.clone())
                .expect("enqueue unique source owner");
            for _ in 0..2 {
                runtime
                    .ingress
                    .enqueue_canonical_body_available(rebound, manifest.clone())
                    .expect("test seam creates duplicate destination ownership");
            }

            assert_eq!(
                runtime
                    .rebind_body_available(source_tag, rebound, &manifest)
                    .expect_err("duplicate destination ownership must fail closed"),
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
            );
            assert!(runtime.fail_closed);
            assert_eq!(runtime.queued_commands(), 3);
            assert_eq!(
                runtime
                    .ingress
                    .commands
                    .iter()
                    .filter(|queued| queued.tag == source_tag)
                    .count(),
                1,
                "destination preflight must retain the source owner"
            );
            assert_eq!(
                runtime
                    .ingress
                    .commands
                    .iter()
                    .filter(|queued| queued.tag == rebound)
                    .count(),
                2,
                "destination preflight must not mutate duplicate owners"
            );
            assert_eq!(
                runtime
                    .rebind_body_available(source_tag, rebound, &manifest)
                    .expect_err("fail-closed runtime rejects a second duplicate rebind"),
                "Sumeragi v2 runtime is fail-closed"
            );
            assert_eq!(
                runtime.enqueue_application_completed(source_tag, manifest.subject),
                Err(EnqueueError::FailClosed)
            );
            assert!(matches!(
                runtime.step(Instant::now()),
                Err(RuntimeError::FailClosed)
            ));
        }
    }

    #[test]
    fn duplicate_body_available_rebind_and_retirement_fail_closed_before_mutation() {
        {
            let directory = TempDir::new().expect("temporary duplicate-rebind directory");
            let (mut runtime, context, _keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let owner_tag = runtime.round_tag();
            let manifest = runtime_manifest(&context, 0x8E);
            for _ in 0..2 {
                runtime
                    .ingress
                    .enqueue_canonical_body_available(owner_tag, manifest.clone())
                    .expect("test seam creates duplicate ingress ownership");
            }
            let rebound = EventTag::new(
                owner_tag.height(),
                owner_tag.view() + 1,
                Generation::new(owner_tag.generation().get() + 1),
            );
            observe_enter_view_for_test(&mut runtime, owner_tag, rebound, &manifest);

            assert_eq!(
                runtime
                    .rebind_body_available(owner_tag, rebound, &manifest)
                    .expect_err("duplicate ownership must prevent rebind"),
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
            );
            assert!(runtime.fail_closed);
            assert_eq!(runtime.queued_commands(), 2);
            assert!(
                runtime
                    .ingress
                    .commands
                    .iter()
                    .all(|queued| queued.tag == owner_tag),
                "preflight must leave every duplicate owner at its original tag"
            );
            assert_eq!(
                runtime
                    .rebind_body_available(owner_tag, rebound, &manifest)
                    .expect_err("fail-closed runtime must reject a second rebind"),
                "Sumeragi v2 runtime is fail-closed"
            );
            assert_eq!(
                runtime.enqueue_application_completed(owner_tag, manifest.subject),
                Err(EnqueueError::FailClosed)
            );
            assert!(matches!(
                runtime.step(Instant::now()),
                Err(RuntimeError::FailClosed)
            ));
        }

        {
            let directory = TempDir::new().expect("temporary duplicate-retirement directory");
            let (mut runtime, context, _keys) =
                authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
            let owner_tag = runtime.round_tag();
            let manifest = runtime_manifest(&context, 0x8F);
            for _ in 0..2 {
                runtime
                    .ingress
                    .enqueue_canonical_body_available(owner_tag, manifest.clone())
                    .expect("test seam creates duplicate ingress ownership");
            }

            assert_eq!(
                runtime
                    .retire_body_available(owner_tag, &manifest)
                    .expect_err("duplicate ownership must prevent retirement"),
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
            );
            assert!(runtime.fail_closed);
            assert_eq!(
                runtime.queued_commands(),
                2,
                "preflight must not mutate duplicate serialized owners"
            );
            assert_eq!(
                runtime
                    .retire_body_available(owner_tag, &manifest)
                    .expect_err("fail-closed runtime must reject a second retirement"),
                "Sumeragi v2 runtime is fail-closed"
            );
            assert_eq!(
                runtime.enqueue_application_completed(owner_tag, manifest.subject),
                Err(EnqueueError::FailClosed)
            );
            assert!(matches!(
                runtime.step(Instant::now()),
                Err(RuntimeError::FailClosed)
            ));
        }
    }

    #[test]
    fn conflicting_body_pipeline_evidence_fails_closed_before_body_available_pruning() {
        let body_directory = TempDir::new().expect("temporary body evidence directory");
        let (mut body_runtime, context, keys) =
            authenticated_network_runtime(&body_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = body_runtime.round_tag();
        let proposal = signed_runtime_proposal(&context, &keys, 0x95);
        let manifest = match &proposal.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => proposal.manifest.clone(),
            _ => unreachable!("fixture is a proposal"),
        };
        body_runtime
            .enqueue_network(proposal)
            .expect("enqueue the exact authenticated proposal");
        body_runtime
            .enqueue_body_available(owner_tag, manifest.clone())
            .expect("enqueue the first canonical body completion");
        assert_eq!(body_runtime.queued_commands(), 2);

        let mut conflicting_manifest = manifest.clone();
        conflicting_manifest.chunk_hashes[0] = Hash::new(b"conflicting completion chunk");
        conflicting_manifest.chunk_root = Hash::new(b"conflicting completion root");
        assert_eq!(
            body_runtime.enqueue_body_available(owner_tag, conflicting_manifest),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert!(body_runtime.fail_closed);
        assert_eq!(
            body_runtime.queued_commands(),
            2,
            "ownership must fail before a conflicting completion prunes the exact proposal"
        );
        assert!(body_runtime.ingress.commands.iter().any(|queued| matches!(
            &queued.command,
            AdapterCommand::Authenticated(authenticated)
                if matches!(
                    authenticated.payload(),
                    wire::ConsensusMessageV2Payload::Proposal(proposal)
                        if proposal.manifest == manifest
                )
        )));
        assert_eq!(
            body_runtime.enqueue_body_available(owner_tag, manifest),
            Err(EnqueueError::FailClosed)
        );

        let stored_directory = TempDir::new().expect("temporary durable evidence directory");
        let (mut stored_runtime, context, _keys) =
            authenticated_network_runtime(&stored_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = stored_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x96);
        let exact_receipt = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let mut other_manifest = manifest.clone();
        other_manifest.chunk_hashes[0] = Hash::new(b"different durable receipt chunk");
        other_manifest.chunk_root = Hash::new(b"different durable receipt root");
        let conflicting_receipt = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&other_manifest),
        );
        stored_runtime
            .enqueue_body_stored(owner_tag, manifest.round, manifest.subject, exact_receipt)
            .expect("enqueue exact durable receipt");
        assert_eq!(
            stored_runtime.enqueue_body_stored(
                owner_tag,
                manifest.round,
                manifest.subject,
                conflicting_receipt,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert!(stored_runtime.fail_closed);

        let validation_directory = TempDir::new().expect("temporary validation polarity directory");
        let (mut validation_runtime, context, _keys) =
            authenticated_network_runtime(&validation_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = validation_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x97);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        validation_runtime
            .enqueue_validation_succeeded(
                owner_tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            )
            .expect("enqueue validation success");
        assert_eq!(
            validation_runtime.enqueue_validation_failed(
                owner_tag,
                manifest.round,
                manifest.subject,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership),
            "opposite validation polarity is conflicting evidence"
        );
        assert!(validation_runtime.fail_closed);

        let deferred_failure_directory =
            TempDir::new().expect("temporary deferred validation-failure directory");
        let (mut deferred_failure_runtime, context, _keys) = authenticated_network_runtime(
            &deferred_failure_directory,
            RuntimeQueueConfig::new(8, 1, 1),
        );
        let owner_tag = deferred_failure_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x9B);
        deferred_failure_runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &manifest,
                DeferredBodyPipelineStageForTest::ValidationFailed,
            )
            .expect("stage Busy-deferred validation failure");
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        assert_eq!(
            deferred_failure_runtime.enqueue_validation_succeeded(
                owner_tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            ),
            Err(EnqueueError::DuplicateCompletionOwnership),
            "Busy-deferred failure cannot coalesce an incoming success"
        );
        assert!(deferred_failure_runtime.fail_closed);

        let deferred_success_directory =
            TempDir::new().expect("temporary deferred validation-success directory");
        let (mut deferred_success_runtime, context, _keys) = authenticated_network_runtime(
            &deferred_success_directory,
            RuntimeQueueConfig::new(8, 1, 1),
        );
        let owner_tag = deferred_success_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x9C);
        deferred_success_runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &manifest,
                DeferredBodyPipelineStageForTest::ValidationSucceeded,
            )
            .expect("stage Busy-deferred validation success");
        assert_eq!(
            deferred_success_runtime.enqueue_validation_failed(
                owner_tag,
                manifest.round,
                manifest.subject,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership),
            "Busy-deferred success cannot coalesce an incoming failure"
        );
        assert!(deferred_success_runtime.fail_closed);

        let atomic_directory = TempDir::new().expect("temporary atomic validation directory");
        let (mut atomic_runtime, context, _keys) =
            authenticated_network_runtime(&atomic_directory, RuntimeQueueConfig::new(3, 1, 1));
        let owner_tag = atomic_runtime.round_tag();
        let manifests = [0x9D, 0x9E, 0x9F, 0xA0].map(|seed| runtime_manifest(&context, seed));
        let failures = manifests
            .iter()
            .enumerate()
            .map(|(index, manifest)| {
                let offset = u64::try_from(index).expect("small failure batch");
                (
                    EventTag::new(
                        owner_tag.height(),
                        owner_tag.view().saturating_add(offset),
                        Generation::new(owner_tag.generation().get().saturating_add(offset)),
                    ),
                    manifest.round,
                    manifest.subject,
                )
            })
            .collect::<Vec<_>>();
        let next_ordinal_before_wrong_class = atomic_runtime.ingress.next_admission_ordinal;
        let (wrong_tag, wrong_round, wrong_subject) = failures[0];
        assert_eq!(
            atomic_runtime
                .ingress
                .enqueue_completion_batch(vec![TaggedCommand::new(
                    wrong_tag,
                    CommandClass::Normal,
                    AdapterCommand::ValidationFailed {
                        round: wrong_round,
                        subject: wrong_subject,
                    },
                    Instant::now(),
                )]),
            Err(EnqueueError::FailClosed),
            "a batch API cannot relabel non-completion traffic as trusted completion work"
        );
        assert_eq!(atomic_runtime.queued_commands(), 0);
        assert_eq!(
            atomic_runtime.ingress.next_admission_ordinal, next_ordinal_before_wrong_class,
            "rejected batch traffic cannot spend an admission ordinal"
        );
        assert_eq!(
            atomic_runtime.enqueue_validation_failures_atomically(&failures),
            Err(EnqueueError::Full)
        );
        assert_eq!(
            atomic_runtime.queued_commands(),
            0,
            "a capacity failure cannot publish an earlier member of the batch"
        );
        atomic_runtime
            .enqueue_validation_failures_atomically(&failures[..3])
            .expect("the complete fitting batch is admitted atomically");
        assert_eq!(atomic_runtime.queued_commands(), 3);
        for (queued, (tag, round, subject)) in atomic_runtime
            .ingress
            .commands
            .iter()
            .zip(failures.iter().copied())
        {
            assert_eq!(queued.tag, tag);
            assert!(matches!(
                &queued.command,
                AdapterCommand::ValidationFailed {
                    round: queued_round,
                    subject: queued_subject,
                } if *queued_round == round && *queued_subject == subject
            ));
        }
        atomic_runtime
            .enqueue_validation_failures_atomically(&failures[..3])
            .expect("exact pre-owned rows coalesce without spending capacity");
        assert_eq!(atomic_runtime.queued_commands(), 3);

        let conflict_directory =
            TempDir::new().expect("temporary conflicting atomic validation directory");
        let (mut conflict_runtime, conflict_context, _keys) =
            authenticated_network_runtime(&conflict_directory, RuntimeQueueConfig::new(4, 1, 1));
        let conflict_tag = conflict_runtime.round_tag();
        let vacant = runtime_manifest(&conflict_context, 0xA1);
        let conflicting = runtime_manifest(&conflict_context, 0xA2);
        let durable = DurableBodyReceipt::for_test(
            conflict_context.id(),
            conflicting.round,
            conflicting.subject,
            HashOf::new(&conflicting),
        );
        conflict_runtime
            .enqueue_validation_succeeded(
                conflict_tag,
                conflicting.round,
                conflicting.subject,
                ValidatedBodyReceipt::for_test(durable),
            )
            .expect("stage conflicting positive validation evidence");
        assert_eq!(
            conflict_runtime.enqueue_validation_failures_atomically(&[
                (conflict_tag, vacant.round, vacant.subject),
                (conflict_tag, conflicting.round, conflicting.subject),
            ]),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert_eq!(
            conflict_runtime.queued_commands(),
            1,
            "the vacant prefix cannot become visible before a later conflict"
        );
        assert!(conflict_runtime.fail_closed);
    }

    #[test]
    fn conflicting_local_and_validated_receipts_do_not_coalesce() {
        let validation_directory =
            TempDir::new().expect("temporary execution commitment directory");
        let (mut validation_runtime, context, _keys) =
            authenticated_network_runtime(&validation_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = validation_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x98);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let exact_validated = ValidatedBodyReceipt::for_test(durable.clone());
        let conflicting_validated = ValidatedBodyReceipt::for_test_with_commitment(
            durable,
            wire::ExecutionCommitment::without_topups(
                Hash::new(b"conflicting parent state"),
                Hash::new(b"conflicting post state"),
                Hash::new(b"conflicting ordinary writes"),
                Hash::new(b"conflicting executed body"),
            ),
        );
        validation_runtime
            .enqueue_validation_succeeded(
                owner_tag,
                manifest.round,
                manifest.subject,
                exact_validated,
            )
            .expect("enqueue exact validated receipt");
        assert_eq!(
            validation_runtime.enqueue_validation_succeeded(
                owner_tag,
                manifest.round,
                manifest.subject,
                conflicting_validated,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert!(validation_runtime.fail_closed);

        let proposal_directory = TempDir::new().expect("temporary local proposal directory");
        let (mut proposal_runtime, context, _keys) =
            authenticated_network_runtime(&proposal_directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = proposal_runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x99);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        proposal_runtime
            .enqueue_local_proposal(owner_tag, manifest.clone(), durable, validated)
            .expect("enqueue exact local proposal completion");

        let mut conflicting_manifest = manifest.clone();
        conflicting_manifest.chunk_hashes[0] = Hash::new(b"conflicting local proposal chunk");
        conflicting_manifest.chunk_root = Hash::new(b"conflicting local proposal root");
        let conflicting_durable = DurableBodyReceipt::for_test(
            context.id(),
            conflicting_manifest.round,
            conflicting_manifest.subject,
            HashOf::new(&conflicting_manifest),
        );
        let conflicting_validated = ValidatedBodyReceipt::for_test(conflicting_durable.clone());
        assert_eq!(
            proposal_runtime.enqueue_local_proposal(
                owner_tag,
                conflicting_manifest,
                conflicting_durable,
                conflicting_validated,
            ),
            Err(EnqueueError::DuplicateCompletionOwnership)
        );
        assert!(proposal_runtime.fail_closed);
    }

    #[test]
    fn production_busy_transfer_retains_exact_validation_evidence_for_retry_and_cleanup() {
        let directory = TempDir::new().expect("temporary production Busy-transfer directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for production dispatch");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0x9A))
            .expect("enqueue authenticated proposal");
        let proposal_effects = match runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch proposal")
        {
            RuntimeStep::Advanced(effects) => effects,
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
        let (tag, manifest) = match proposal_effects.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };

        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("enqueue body reconstruction completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch body reconstruction"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
        ));
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("enqueue durable-store completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch durable-store completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));
        let validated = ValidatedBodyReceipt::for_test(durable);
        runtime
            .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated.clone())
            .expect("enqueue validation completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch validation completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::Sign { .. }])
        ));

        runtime
            .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated.clone())
            .expect("enqueue validation retry behind the signer fence");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("transfer retry to Busy-deferred ownership"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.queued_commands(), 0);
        runtime
            .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated)
            .expect("exact retry coalesces with real Busy-deferred evidence");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(tag, manifest.round, manifest.subject)
                .expect("decision cleanup sees one exact Busy-deferred owner"),
            RetiredBodyPipelineCompletions {
                body_available: 0,
                body_stored: 0,
                validation: 1,
                local_proposal: 0,
            }
        );
    }

    #[test]
    fn body_pipeline_retirement_spans_ingress_and_busy_deferred_owners_and_rejects_duplicates() {
        let directory = TempDir::new().expect("temporary body-pipeline retirement directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let receipts = |manifest: &wire::PayloadManifest| {
            let durable = DurableBodyReceipt::for_test(
                context.id(),
                manifest.round,
                manifest.subject,
                HashOf::new(manifest),
            );
            let validated = ValidatedBodyReceipt::for_test(durable.clone());
            (durable, validated)
        };
        let three_stages = RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 1,
            validation: 1,
            local_proposal: 1,
        };
        let validation_only = RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 0,
            validation: 1,
            local_proposal: 0,
        };

        let ingress_manifest = runtime_manifest(&context, 0xA1);
        let (durable, validated) = receipts(&ingress_manifest);
        runtime
            .enqueue_body_stored(
                owner_tag,
                ingress_manifest.round,
                ingress_manifest.subject,
                durable.clone(),
            )
            .expect("enqueue ingress BodyStored owner");
        runtime
            .enqueue_validation_succeeded(
                owner_tag,
                ingress_manifest.round,
                ingress_manifest.subject,
                validated.clone(),
            )
            .expect("enqueue ingress validation-success owner");
        runtime
            .enqueue_local_proposal(owner_tag, ingress_manifest.clone(), durable, validated)
            .expect("enqueue ingress LocalProposalReady owner");
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    ingress_manifest.round,
                    ingress_manifest.subject,
                )
                .expect("retire ingress body pipeline"),
            three_stages
        );

        let ingress_failure_manifest = runtime_manifest(&context, 0xA2);
        runtime
            .enqueue_validation_failed(
                owner_tag,
                ingress_failure_manifest.round,
                ingress_failure_manifest.subject,
            )
            .expect("enqueue ingress validation-failure owner");
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    ingress_failure_manifest.round,
                    ingress_failure_manifest.subject,
                )
                .expect("retire ingress validation failure"),
            validation_only
        );

        let deferred_manifest = runtime_manifest(&context, 0xB1);
        for stage in [
            DeferredBodyPipelineStageForTest::BodyStored,
            DeferredBodyPipelineStageForTest::ValidationSucceeded,
            DeferredBodyPipelineStageForTest::LocalProposalReady,
        ] {
            runtime
                .driver
                .defer_body_pipeline_stage_for_test(owner_tag, &deferred_manifest, stage)
                .expect("stage Busy-deferred body completion");
        }
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    deferred_manifest.round,
                    deferred_manifest.subject,
                )
                .expect("retire Busy-deferred body pipeline"),
            three_stages
        );

        let deferred_failure_manifest = runtime_manifest(&context, 0xB2);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_failure_manifest,
                DeferredBodyPipelineStageForTest::ValidationFailed,
            )
            .expect("stage Busy-deferred validation failure");
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    deferred_failure_manifest.round,
                    deferred_failure_manifest.subject,
                )
                .expect("retire Busy-deferred validation failure"),
            validation_only
        );

        let duplicate_body_stored = runtime_manifest(&context, 0xC1);
        let (durable, _) = receipts(&duplicate_body_stored);
        runtime
            .enqueue_body_stored(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
                durable,
            )
            .expect("enqueue duplicate ingress BodyStored owner");
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &duplicate_body_stored,
                DeferredBodyPipelineStageForTest::BodyStored,
            )
            .expect("stage duplicate deferred BodyStored owner");
        let stored_only = RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 1,
            validation: 0,
            local_proposal: 0,
        };
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime.ingress.body_pipeline_completion_counts(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            ),
            stored_only
        );
        assert_eq!(
            runtime.driver.deferred_body_pipeline_completion_counts(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            ),
            stored_only
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    duplicate_body_stored.round,
                    duplicate_body_stored.subject,
                )
                .expect_err("duplicate BodyStored ownership must fail"),
            "Sumeragi v2 body pipeline has duplicate exact serialized completion stages"
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime.ingress.body_pipeline_completion_counts(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            ),
            stored_only,
            "preflight must retain the ingress owner"
        );
        assert_eq!(
            runtime.driver.deferred_body_pipeline_completion_counts(
                owner_tag,
                duplicate_body_stored.round,
                duplicate_body_stored.subject,
            ),
            stored_only,
            "preflight must retain the Busy-deferred owner"
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    duplicate_body_stored.round,
                    duplicate_body_stored.subject,
                )
                .expect_err("fail-closed runtime must reject a second pipeline retirement"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(owner_tag, duplicate_body_stored.subject,),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }

    #[test]
    fn decision_retires_proposal_owners_but_preserves_body_and_application_completions() {
        let directory = TempDir::new().expect("temporary decision-retirement directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(12, 1, 1));
        let owner_tag = runtime.round_tag();
        let receipts = |manifest: &wire::PayloadManifest| {
            let durable = DurableBodyReceipt::for_test(
                context.id(),
                manifest.round,
                manifest.subject,
                HashOf::new(manifest),
            );
            let validated = ValidatedBodyReceipt::for_test(durable.clone());
            (durable, validated)
        };

        let decision_manifest = runtime_manifest(&context, 0xD0);
        let (decision_durable, decision_validated) = receipts(&decision_manifest);
        let decision_commitment = decision_validated.execution_commitment();
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0xD1))
            .expect("enqueue authenticated proposal at decided height");
        runtime
            .enqueue_local_proposal(
                owner_tag,
                decision_manifest.clone(),
                decision_durable.clone(),
                decision_validated,
            )
            .expect("enqueue exact decided LocalProposalReady");
        let other_local_manifest = runtime_manifest(&context, 0xD2);
        let (other_durable, other_validated) = receipts(&other_local_manifest);
        runtime
            .enqueue_local_proposal(
                owner_tag,
                other_local_manifest.clone(),
                other_durable,
                other_validated,
            )
            .expect("enqueue another local proposal at decided height");
        runtime
            .enqueue_body_available(owner_tag, decision_manifest.clone())
            .expect("enqueue body-recovery completion");
        runtime
            .enqueue_body_stored(
                owner_tag,
                decision_manifest.round,
                decision_manifest.subject,
                decision_durable,
            )
            .expect("enqueue body-store completion");
        runtime
            .enqueue_application_completed(owner_tag, decision_manifest.subject)
            .expect("enqueue application completion");

        let deferred_proposal = match signed_runtime_proposal(&context, &keys, 0xD3).payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => proposal,
            _ => unreachable!("fixture is a proposal"),
        };
        runtime
            .driver
            .defer_authenticated_proposal_for_test(owner_tag, &deferred_proposal)
            .expect("stage Busy-deferred authenticated proposal");
        let deferred_local_manifest = runtime_manifest(&context, 0xD4);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_local_manifest,
                DeferredBodyPipelineStageForTest::LocalProposalReady,
            )
            .expect("stage Busy-deferred LocalProposalReady");
        let deferred_body_manifest = runtime_manifest(&context, 0xD5);
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_body_manifest,
                DeferredBodyPipelineStageForTest::BodyStored,
            )
            .expect("stage Busy-deferred body-store completion");
        assert_eq!(
            runtime
                .driver
                .status()
                .expect("status before decision retirement")
                .liveness
                .work
                .candidate,
            wire::SumeragiV2LocalWorkStage::Complete
        );

        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(
                    decision_manifest.round,
                    decision_manifest.subject,
                    decision_commitment,
                )
                .expect("retire proposal work after decision"),
            DecisionProposalRetirement::new(Some(owner_tag), 0),
            "the exact current-tag LocalProposalReady owner must remain queued"
        );
        assert_eq!(runtime.queued_commands(), 4);
        assert!(runtime.ingress.commands.iter().all(|queued| !matches!(
            &queued.command,
            AdapterCommand::Authenticated(authenticated)
                if matches!(
                    authenticated.payload(),
                    wire::ConsensusMessageV2Payload::Proposal(_)
                )
        )));
        assert!(runtime.ingress.commands.iter().any(|queued| matches!(
            &queued.command,
            AdapterCommand::LocalProposalReady { manifest, .. }
                if manifest == &decision_manifest
        )));
        assert!(
            runtime
                .ingress
                .commands
                .iter()
                .any(|queued| matches!(&queued.command, AdapterCommand::BodyAvailable { .. }))
        );
        assert!(
            runtime
                .ingress
                .commands
                .iter()
                .any(|queued| matches!(&queued.command, AdapterCommand::BodyStored { .. }))
        );
        assert!(
            runtime
                .ingress
                .commands
                .iter()
                .any(|queued| matches!(&queued.command, AdapterCommand::ApplicationCompleted(_)))
        );
        assert_eq!(
            runtime
                .driver
                .status()
                .expect("status after decision retirement")
                .liveness
                .work
                .candidate,
            wire::SumeragiV2LocalWorkStage::Idle,
            "decision retirement clears stale active proposal state"
        );
        let deferred_local_commitment = receipts(&deferred_local_manifest).1.execution_commitment();
        assert_eq!(
            runtime
                .ingress
                .decided_local_proposal_counts(
                    owner_tag,
                    deferred_local_manifest.round,
                    deferred_local_manifest.subject,
                    deferred_local_commitment,
                )
                .merge(runtime.driver.deferred_decided_local_proposal_counts(
                    owner_tag,
                    deferred_local_manifest.round,
                    deferred_local_manifest.subject,
                    deferred_local_commitment,
                )),
            DecisionLocalProposalCounts::default(),
            "all nonmatching local proposal completions were retired"
        );

        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    decision_manifest.round,
                    decision_manifest.subject,
                )
                .expect("body recovery remains queued after decision"),
            RetiredBodyPipelineCompletions {
                body_available: 1,
                body_stored: 1,
                validation: 0,
                local_proposal: 1,
            }
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    deferred_body_manifest.round,
                    deferred_body_manifest.subject,
                )
                .expect("Busy-deferred body store remains queued after decision"),
            RetiredBodyPipelineCompletions {
                body_available: 0,
                body_stored: 1,
                validation: 0,
                local_proposal: 0,
            }
        );
        assert_eq!(runtime.queued_commands(), 1);
        assert!(matches!(
            runtime.ingress.commands.front().map(|queued| &queued.command),
            Some(AdapterCommand::ApplicationCompleted(subject))
                if *subject == decision_manifest.subject
        ));

        let duplicate_manifest = runtime_manifest(&context, 0xD6);
        let (duplicate_durable, duplicate_validated) = receipts(&duplicate_manifest);
        let duplicate_commitment = duplicate_validated.execution_commitment();
        runtime
            .enqueue_local_proposal(
                owner_tag,
                duplicate_manifest.clone(),
                duplicate_durable,
                duplicate_validated,
            )
            .expect("enqueue exact local completion in runtime ingress");
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &duplicate_manifest,
                DeferredBodyPipelineStageForTest::LocalProposalReady,
            )
            .expect("stage duplicate exact local completion in Busy-deferred lane");
        assert_eq!(runtime.queued_commands(), 2);
        assert_eq!(
            runtime
                .ingress
                .decided_local_proposal_counts(
                    owner_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .retainable(),
            1,
        );
        assert_eq!(
            runtime
                .driver
                .deferred_decided_local_proposal_counts(
                    owner_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .retainable(),
            1,
        );
        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .expect_err("duplicate exact local completion ownership must fail"),
            "Sumeragi v2 decided local proposal completion has duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.queued_commands(),
            2,
            "preflight must retain the application and ingress proposal owners"
        );
        assert_eq!(
            runtime
                .ingress
                .decided_local_proposal_counts(
                    owner_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .retainable(),
            1,
        );
        assert_eq!(
            runtime
                .driver
                .deferred_decided_local_proposal_counts(
                    owner_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .retainable(),
            1,
            "preflight must retain the Busy-deferred proposal owner"
        );
        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                    duplicate_commitment,
                )
                .expect_err("fail-closed runtime must reject a second proposal retirement"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_signature(owner_tag, vec![0xD6]),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }

    #[test]
    fn decision_retires_stale_local_completion_for_durable_recovery() {
        let directory = TempDir::new().expect("temporary stale-decision directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let stale_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xD7);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        let commitment = validated.execution_commitment();
        runtime
            .enqueue_local_proposal(stale_tag, manifest.clone(), durable, validated)
            .expect("enqueue the old reducer incarnation's completion");

        runtime.round_tag = EventTag::new(
            stale_tag.height(),
            stale_tag.view().saturating_add(1),
            Generation::new(stale_tag.generation().get().saturating_add(1)),
        );
        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
                .expect("retire stale exact completion after certified view change"),
            DecisionProposalRetirement::new(None, 1)
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert!(!runtime.fail_closed);
        runtime
            .enqueue_body_available(runtime.round_tag(), manifest)
            .expect("durable reconstruction can claim the current reducer tag");
    }

    #[test]
    fn progress_cursor_decision_preserves_outer_ingress_completion_until_apply() {
        let directory = TempDir::new().expect("temporary Decision-race directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xD9);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        let commitment = validated.execution_commitment();
        runtime
            .enqueue_local_proposal(
                owner_tag,
                manifest.clone(),
                durable.clone(),
                validated.clone(),
            )
            .expect("enqueue trusted completion in the outer runtime ingress");
        runtime
            .enqueue_local_proposal(owner_tag, manifest.clone(), durable, validated)
            .expect("an exact trusted retry coalesces with its existing owner");
        assert_eq!(runtime.queued_commands(), 1);
        let decision = wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: manifest.subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD9; 96],
        };
        runtime
            .ingress
            .enqueue_authenticated(
                owner_tag,
                CommandClass::Progress,
                AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(decision.clone()),
                )),
            )
            .expect("enqueue the CommitQC progress item");
        runtime.ingress.next_class = CommandClass::Progress;
        let now = Instant::now();
        runtime.arm_live_clocks(now).expect("arm runtime clocks");

        let RuntimeStep::Advanced(decision_effects) = runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("Progress cursor installs Decision")
        else {
            panic!("queued CommitQC must advance the reducer")
        };
        assert!(matches!(
            decision_effects.as_slice(),
            [AdapterEffect::FetchBody {
                subject,
                certificate: Some(certificate),
                ..
            }] if *subject == manifest.subject && certificate == &decision
        ));
        assert_eq!(runtime.queued_commands(), 1);

        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
                .expect("Decision cleanup preserves the exact completion"),
            DecisionProposalRetirement::new(Some(owner_tag), 0)
        );
        let RuntimeStep::Advanced(completion_effects) = runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("fair completion service reaches the reducer")
        else {
            panic!("retained completion must advance the reducer")
        };
        assert!(matches!(
            completion_effects.as_slice(),
            [AdapterEffect::Apply {
                subject,
                certificate,
                ..
            }] if *subject == manifest.subject && certificate == &decision
        ));
        assert!(!completion_effects.iter().any(|effect| matches!(
            effect,
            AdapterEffect::FetchBody { .. } | AdapterEffect::StoreBody { .. }
        )));
        assert_eq!(runtime.queued_commands(), 0);
    }

    #[test]
    fn decision_cleanup_preserves_unique_busy_deferred_completion() {
        let directory = TempDir::new().expect("temporary Busy-deferred Decision directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xDA);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &manifest,
                DeferredBodyPipelineStageForTest::LocalProposalReady,
            )
            .expect("stage exact Busy-deferred completion");

        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(manifest.round, manifest.subject, commitment,)
                .expect("retain exact Busy-deferred completion"),
            DecisionProposalRetirement::new(Some(owner_tag), 0)
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .driver
                .deferred_decided_local_proposal_counts(
                    owner_tag,
                    manifest.round,
                    manifest.subject,
                    commitment,
                )
                .retainable(),
            1
        );
    }

    #[test]
    fn decision_commitment_mismatch_fails_closed_before_retirement() {
        let directory = TempDir::new().expect("temporary mismatched-decision directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xD8);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        let conflicting_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"decision mismatch parent state"),
            Hash::new(b"decision mismatch post state"),
            Hash::new(b"decision mismatch ordinary writes"),
            Hash::new(b"decision mismatch executed block"),
        );
        assert_ne!(validated.execution_commitment(), conflicting_commitment);
        runtime
            .enqueue_local_proposal(owner_tag, manifest.clone(), durable, validated)
            .expect("enqueue exact trusted completion");

        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(
                    manifest.round,
                    manifest.subject,
                    conflicting_commitment,
                )
                .expect_err("Decision commitment drift must fail closed"),
            "Sumeragi v2 decided local proposal evidence conflicts with the durable Decision"
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.queued_commands(),
            1,
            "conflict preflight must preserve the original evidence for diagnosis"
        );
        assert!(matches!(
            runtime.ingress.commands.front().map(|queued| &queued.command),
            Some(AdapterCommand::LocalProposalReady {
                manifest: queued,
                ..
            }) if queued == &manifest
        ));
    }

    #[test]
    fn unbound_direct_vote_authentication_is_recoverable_and_becomes_admissible_after_validation() {
        let directory = TempDir::new().expect("temporary unbound-vote directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let manifest = runtime_manifest(&context, 0xD7);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable);
        let mut vote = wire::Vote {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Prepare,
            subject: manifest.subject,
            execution_commitment: validated.execution_commitment(),
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(vote.signer).expect("small signer index")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        let signed_vote =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote));

        assert!(matches!(
            runtime.enqueue_network(signed_vote.clone()),
            Err(NetworkIngressError::Authentication(
                AdapterError::MissingExecutionCommitment
            ))
        ));
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            !runtime.fail_closed,
            "recoverable authentication rejection must not poison the runtime"
        );

        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0xD8))
            .expect("a subsequent valid proposal remains admissible");
        assert_eq!(runtime.queued_commands(), 1);
        assert!(!runtime.fail_closed);

        runtime
            .recover_validated_body(&manifest, &validated)
            .expect("local validation establishes canonical commitment authority");
        runtime
            .enqueue_network(signed_vote)
            .expect("the same signed canonical vote becomes admissible after validation");
        assert_eq!(runtime.queued_commands(), 2);
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn exact_authenticated_network_retransmission_obeys_runtime_boundaries() {
        let directory = TempDir::new().expect("temporary runtime ingress directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let original = signed_runtime_proposal(&context, &keys, 1);
        let second = signed_runtime_proposal(&context, &keys, 2);
        let third = signed_runtime_proposal(&context, &keys, 3);
        let transport = match &original.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadManifest(proposal.manifest.clone()),
            ),
            _ => unreachable!("fixture is a proposal"),
        };

        let owner_tag = runtime
            .enqueue_network(original.clone())
            .expect("first authenticated proposal owns one normal slot");
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact duplicate coalesces below the normal boundary"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 1);

        let mut invalid = third.clone();
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut invalid.payload else {
            unreachable!("fixture is a proposal")
        };
        proposal.signature[0] ^= 0x80;
        assert!(matches!(
            runtime.enqueue_network(invalid),
            Err(NetworkIngressError::Authentication(_))
        ));
        assert_eq!(runtime.queued_commands(), 1);

        runtime
            .enqueue_network(second.clone())
            .expect("non-identical authenticated proposal uses ordinary capacity");
        assert_eq!(runtime.queued_commands(), 2);
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact duplicate coalesces at reserved capacity"),
            owner_tag
        );
        assert!(matches!(
            runtime.enqueue_network(third.clone()),
            Err(NetworkIngressError::Backpressure(
                EnqueueError::ReservedCapacity
            ))
        ));

        let cursor_before = runtime.ingress.next_class;
        let tags_before = runtime
            .ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        runtime
            .enqueue_signature(owner_tag, vec![4])
            .expect("completion reserve admits the third slot");
        runtime
            .enqueue_signature(owner_tag, vec![5])
            .expect("completion traffic may fill the fourth slot");
        assert_eq!(runtime.queued_commands(), 4);
        assert!(runtime.can_admit_network_message(&original));
        assert!(!runtime.can_admit_network_message(&third));
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact authenticated duplicate coalesces at full capacity"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 4);
        assert_eq!(runtime.ingress.next_class, cursor_before);
        assert_eq!(
            runtime
                .ingress
                .commands
                .iter()
                .take(tags_before.len())
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            tags_before
        );
        assert!(matches!(
            runtime.enqueue_network(third),
            Err(NetworkIngressError::Backpressure(EnqueueError::Full))
        ));

        runtime.fail_closed = true;
        assert!(matches!(
            runtime.enqueue_network(original.clone()),
            Err(NetworkIngressError::FailClosed)
        ));
        assert!(matches!(
            runtime.enqueue_network(transport.clone()),
            Err(NetworkIngressError::FailClosed)
        ));
        runtime.fail_closed = false;
        assert!(matches!(
            runtime.enqueue_network(transport),
            Err(NetworkIngressError::TransportPayload)
        ));
    }

    #[test]
    fn commit_certificate_response_waits_for_embedded_qc_progress_capacity() {
        let directory = TempDir::new().expect("temporary runtime ingress directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"response-capacity-block")),
            payload_hash: Hash::new(b"response-capacity-payload"),
        };
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"response capacity parent state"),
                Hash::new(b"response capacity post state"),
                Hash::new(b"response capacity ordinary writes"),
                Hash::new(b"response capacity executed block wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let response = |certificate| {
            wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                    wire::CommitCertificateResponse {
                        request_hash: HashOf::from_untyped_unchecked(Hash::new(
                            b"response capacity request",
                        )),
                        certificate,
                        responder: PeerId::new(keys[0].public_key().clone()),
                        signature: vec![1],
                    },
                ),
            )
        };
        let exact_response = response(certificate.clone());
        let mut distinct_certificate = certificate.clone();
        distinct_certificate.aggregate_signature = vec![2];
        let distinct_response = response(distinct_certificate);
        let owner_tag = runtime.round_tag();

        runtime
            .enqueue_signature(owner_tag, vec![3])
            .expect("first completion occupies shared capacity");
        runtime
            .enqueue_signature(owner_tag, vec![4])
            .expect("second completion occupies shared capacity");
        runtime
            .ingress
            .enqueue_authenticated(
                owner_tag,
                CommandClass::Progress,
                AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
                )),
            )
            .expect("authenticated CommitQC fills the Progress prefix");
        assert_eq!(runtime.queued_commands(), 3);

        assert!(
            !runtime.can_admit_network_message(&distinct_response),
            "a distinct response remains in outer ingress while inner Progress is full"
        );
        assert!(
            runtime.can_admit_network_message(&exact_response),
            "an exact embedded CommitQC can coalesce with its queued owner"
        );

        let released = runtime
            .ingress
            .pop_next()
            .expect("release one shared-capacity owner");
        assert_eq!(released.class, CommandClass::Completion);
        assert!(
            runtime.can_admit_network_message(&distinct_response),
            "the retained response can drain after Progress capacity returns"
        );
    }

    #[test]
    fn commit_certificate_response_coalesces_with_exact_busy_deferred_qc() {
        let directory = TempDir::new().expect("temporary deferred-QC runtime directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(4, 1, 1),
            Some(0),
        );
        let owner_tag = runtime.round_tag();
        let exact_certificate = signed_runtime_quorum_certificate(&context, &keys, 0xE1);
        let distinct_certificate = signed_runtime_quorum_certificate(&context, &keys, 0xE2);
        let exact_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(exact_certificate.clone()),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm the production runtime before dispatch");

        let timeout = runtime
            .driver
            .timeout_elapsed(owner_tag)
            .expect("open a signer fence before CommitQC dispatch");
        assert!(
            matches!(
                timeout.effects(),
                [AdapterEffect::Sign {
                    request: SignRequest::TimeoutVote(_),
                    ..
                }]
            ),
            "unexpected timeout effects: {:?}",
            timeout.effects()
        );
        runtime
            .enqueue_network_with_ingress_ownership(
                exact_message.clone(),
                fair_network_ownership(&exact_message, PeerId::new(keys[0].public_key().clone())),
            )
            .expect("enqueue the authenticated CommitQC before the fence is observed");
        assert!(matches!(
            runtime
                .step(now)
                .expect("move the Busy CommitQC into adapter ownership"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .driver
                .deferred_authenticated_message_owner(&exact_message)
                .map(|(tag, _)| tag),
            Some(owner_tag)
        );
        let distinct_message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(distinct_certificate.clone()),
        );
        assert_eq!(
            runtime
                .driver
                .deferred_authenticated_message_owner(&distinct_message)
                .map(|(tag, _)| tag),
            None
        );
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&distinct_certificate),
            None
        );
        let mut reordered_signers = exact_certificate.clone();
        reordered_signers.signers.reverse();
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&reordered_signers),
            None,
            "canonical signer order is part of the deferred QC identity"
        );
        let mut altered_aggregate = exact_certificate.clone();
        altered_aggregate.aggregate_signature.push(0xFF);
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&altered_aggregate),
            None,
            "the aggregate signature is part of the deferred QC identity"
        );
        let mut altered_proposal_round = exact_certificate.clone();
        altered_proposal_round.proposal_round.view =
            altered_proposal_round.proposal_round.view.saturating_add(1);
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&altered_proposal_round),
            None,
            "the proposal round is part of the deferred QC identity"
        );

        for signature in [vec![3], vec![4], vec![5]] {
            runtime
                .enqueue_signature(owner_tag, signature)
                .expect("completion traffic saturates the shared Progress prefix");
        }
        assert_eq!(runtime.queued_commands(), 3);

        let response = |certificate| {
            wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                    wire::CommitCertificateResponse {
                        request_hash: HashOf::from_untyped_unchecked(Hash::new(
                            b"deferred-QC coalescing request",
                        )),
                        certificate,
                        responder: PeerId::new(keys[0].public_key().clone()),
                        signature: vec![1],
                    },
                ),
            )
        };
        assert!(
            runtime.can_admit_network_message(&response(exact_certificate.clone())),
            "an exact response can reach authentication through its Busy-deferred owner"
        );
        assert!(
            !runtime.can_admit_network_message(&response(distinct_certificate.clone())),
            "a distinct response remains blocked while the Progress prefix is saturated"
        );

        let queued_before = runtime.queued_commands();
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    exact_message.clone(),
                    fair_network_ownership(
                        &exact_message,
                        PeerId::new(keys[1].public_key().clone()),
                    ),
                )
                .expect("an exact QC from another source coalesces with adapter ownership"),
            owner_tag
        );
        let exact_response = response(exact_certificate.clone());
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    exact_message.clone(),
                    fair_network_ownership(
                        &exact_response,
                        PeerId::new(keys[2].public_key().clone()),
                    ),
                )
                .expect("the authenticated discovery response coalesces with adapter ownership"),
            owner_tag
        );
        assert_eq!(
            runtime.queued_commands(),
            queued_before,
            "authenticated coalescing must not create a runtime-queued duplicate"
        );
        assert_eq!(
            runtime
                .driver
                .deferred_authenticated_message_owner(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(exact_certificate.clone(),),
                ))
                .map(|(tag, _)| tag),
            Some(owner_tag),
            "request completion leaves the sole Busy-deferred owner intact"
        );
        let retained = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy-deferred QC retains its ingress carriers");
        assert!(retained.validate_exact());
        assert_eq!(retained.direct.len(), 2);
        assert_eq!(retained.commit_certificate_response.len(), 1);
        assert!(!runtime.fail_closed);

        for _ in 2..MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM {
            let source = PeerId::from(KeyPair::random().public_key().clone());
            let candidate = RuntimeIngressOwnershipEvidence::from_fair_ingress(
                &exact_message,
                fair_network_ownership(&exact_message, source),
            )
            .expect("independent Busy-deferred carrier is exact");
            runtime
                .deferred_ingress_ownership
                .values_mut()
                .next()
                .expect("the Busy-deferred QC retains its ingress carriers")
                .merge_downstream(candidate)
                .expect("every protocol-bounded Busy-deferred carrier remains exact");
        }
        let deferred_owner_before = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy-deferred carrier set is full")
            .clone();
        let excess_source = PeerId::from(KeyPair::random().public_key().clone());
        assert!(matches!(
            runtime.enqueue_network_with_ingress_ownership(
                exact_message.clone(),
                fair_network_ownership(&exact_message, excess_source),
            ),
            Err(NetworkIngressError::Backpressure(EnqueueError::Full))
        ));
        assert_eq!(
            runtime
                .deferred_ingress_ownership
                .values()
                .next()
                .expect("backpressure preserves the full Busy-deferred carrier set"),
            &deferred_owner_before
        );
        assert!(!runtime.fail_closed);
        assert!(runtime.fail_closed_reason.is_none());
        assert!(matches!(
            runtime.enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(distinct_certificate),
            )),
            Err(NetworkIngressError::Backpressure(
                EnqueueError::ReservedCapacity
            ))
        ));
        assert_eq!(runtime.queued_commands(), queued_before);
    }

    #[test]
    fn progress_is_not_starved_by_a_normal_traffic_flood() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        for value in 0..3 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .unwrap();
        }
        for value in 100..140 {
            assert_eq!(
                enqueue_fake(
                    &mut runtime,
                    initial,
                    CommandClass::Normal,
                    FakeCommand::record(value)
                ),
                Err(EnqueueError::ReservedCapacity)
            );
        }
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(200),
        )
        .expect("CommitQC/progress reserve remains available");

        let _ = runtime.step(start);
        assert_eq!(runtime.driver.delivered, vec![(initial, 200)]);
        let queue = runtime.queue_snapshot(start);
        assert_eq!(queue.normal.depth, 3);
        assert_eq!(queue.normal.capacity, 3);
        assert_eq!(queue.normal.max_service_debt, 1);
        assert_eq!(queue.progress.depth, 0);
        assert_eq!(queue.completion.depth, 0);
    }

    #[test]
    fn periodic_retransmit_cannot_starve_admitted_work_when_every_step_arrives_late() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        for value in 1..=2 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .unwrap();
        }

        for seconds in [2, 4, 6, 8] {
            let _ = runtime
                .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(seconds));
        }

        assert_eq!(runtime.driver.retransmits, vec![initial, initial]);
        assert_eq!(runtime.driver.delivered, vec![(initial, 1), (initial, 2)]);
    }

    #[test]
    fn absolute_timeout_preempts_admitted_work_owed_by_periodic_timer() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(7),
        )
        .unwrap();

        runtime
            .step(start + Duration::from_secs(2))
            .expect("periodic retransmit dispatch succeeds");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("periodic retransmit publishes scheduler ownership")
                .validate_exact(),
            Ok(())
        );
        runtime
            .step(start + Duration::from_secs(10))
            .expect("absolute timeout dispatch succeeds");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("absolute timeout publishes scheduler ownership")
                .validate_exact(),
            Ok(())
        );
        assert_eq!(runtime.driver.timeouts, vec![initial]);
        assert!(runtime.driver.delivered.is_empty());

        runtime
            .step(start + Duration::from_secs(12))
            .expect("admitted work dispatch succeeds after the timeout");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("admitted work dispatch publishes scheduler ownership")
                .validate_exact(),
            Ok(())
        );
        assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
    }

    #[test]
    fn network_admission_uses_exact_normal_and_progress_reservations() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(4, 1, 1),
        );
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"runtime-test-context",
            ))),
            height: 7,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime-test-block")),
            payload_hash: Hash::new(b"runtime-test-payload"),
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"runtime parent state"),
            Hash::new(b"runtime post state"),
            Hash::new(b"runtime ordinary writes"),
            Hash::new(b"runtime executed block wire"),
        );
        let vote = wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signer: 0,
            signature: vec![1],
        });
        let locked_commit_vote = match &vote {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let mut vote = vote.clone();
                vote.phase = wire::GlobalPhase::Commit;
                wire::ConsensusMessageV2Payload::Vote(vote)
            }
            _ => unreachable!("fixture is a vote"),
        };
        runtime.driver.protected_commit = Some((round, subject, execution_commitment));
        let mismatched_commit_vote = match &locked_commit_vote {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let mut vote = vote.clone();
                vote.subject.payload_hash = Hash::new(b"mismatched runtime commit vote");
                wire::ConsensusMessageV2Payload::Vote(vote)
            }
            _ => unreachable!("fixture is a vote"),
        };
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let commit_qc = wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone());
        let timeout_vote = wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: 0,
            signature: vec![1],
        });
        let commit_response = wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime commit request")),
                certificate,
                responder: PeerId::new(KeyPair::random().public_key().clone()),
                signature: vec![1],
            },
        );
        assert_eq!(network_command_class(&vote), Some(CommandClass::Normal));
        assert_eq!(
            network_command_class(&commit_qc),
            Some(CommandClass::Progress)
        );
        assert_eq!(
            network_command_class(&timeout_vote),
            Some(CommandClass::Progress),
            "authenticated TimeoutVote traffic owns the protected progress prefix"
        );
        assert_eq!(network_command_class(&commit_response), None);
        assert_eq!(
            network_admission_class(&commit_response),
            Some(CommandClass::Progress)
        );
        assert!(runtime.can_admit_network_payload(&vote));
        assert!(runtime.can_admit_network_payload(&commit_qc));
        assert!(runtime.can_admit_network_payload(&timeout_vote));
        assert!(runtime.can_admit_network_payload(&commit_response));

        for value in [1, 2] {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .expect("fill the normal prefix");
        }
        assert!(!runtime.can_admit_network_payload(&vote));
        assert!(
            !runtime.can_admit_network_payload(&mismatched_commit_vote),
            "a merely Commit-shaped vote must stop at pre-authentication backpressure"
        );
        assert!(
            runtime.can_admit_network_payload(&locked_commit_vote),
            "the exact locked Commit vote can reach authentication through the progress reserve"
        );
        assert!(
            runtime.can_admit_network_payload(&commit_qc),
            "CommitQC can use the reserved progress slot"
        );
        assert!(
            runtime.can_admit_network_payload(&timeout_vote),
            "TimeoutVote can use the reserved progress slot"
        );
        assert!(runtime.can_admit_network_payload(&commit_response));

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("fill the progress prefix");
        assert!(!runtime.can_admit_network_payload(&vote));
        assert!(!runtime.can_admit_network_payload(&mismatched_commit_vote));
        assert!(!runtime.can_admit_network_payload(&locked_commit_vote));
        assert!(!runtime.can_admit_network_payload(&commit_qc));
        assert!(!runtime.can_admit_network_payload(&timeout_vote));
        assert!(!runtime.can_admit_network_payload(&commit_response));

        let transport = wire::ConsensusMessageV2Payload::PayloadManifest(wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1,
                max_chunk_count: 1,
            },
            chunk_hashes: vec![Hash::new([0_u8])],
            chunk_root: Hash::new(b"runtime transport root"),
        });
        assert!(runtime.can_admit_network_payload(&transport));
    }

    #[test]
    fn stale_completion_tag_is_delivered_after_due_retransmit_without_retagging() {
        let start = Instant::now();
        let current = tag(4);
        let stale = tag(2);
        let mut runtime = runtime(
            FakeDriver::new(current),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            stale,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .unwrap();
        runtime
            .step(start + Duration::from_secs(2))
            .expect("the due retransmit owns the first turn");
        assert_eq!(runtime.driver.retransmits, vec![current]);
        assert!(runtime.driver.delivered.is_empty());
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("the retransmit publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::PeriodicTimer
        );

        // Even though the clock remains retransmit-due, the admitted
        // completion is owed this slot and retains its original tag.
        runtime
            .step(start + Duration::from_secs(4))
            .expect("the owed completion owns the next turn");
        assert_eq!(runtime.driver.delivered, vec![(stale, 9)]);
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("the completion publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
    }

    #[test]
    fn only_enter_view_effect_restarts_both_clocks() {
        let start = Instant::now();
        let initial = tag(0);
        let next = tag(1);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .unwrap();
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(1));
        assert_eq!(runtime.round_tag(), initial);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::enter_view(next),
        )
        .unwrap();
        // Service the retransmission tick due at t=9, then the queued TC-like
        // progress command at the same monotonic instant.
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9));
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9));
        assert_eq!(runtime.round_tag(), next);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
        assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(22));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10)),
            Ok(RuntimeStep::Idle)
        ));
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(11));
        assert_eq!(runtime.driver.retransmits, vec![initial, next]);
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(19));
        assert!(runtime.driver.timeouts.is_empty());
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(29));
        assert_eq!(runtime.driver.timeouts, vec![next]);
    }

    #[test]
    fn startup_enter_view_effect_restarts_clocks_and_is_returned_unchanged() {
        let start = Instant::now();
        let initial = tag(0);
        let next = tag(1);
        let (mut runtime, effects) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            vec![FakeEffect::enter_view(next), FakeEffect::other()],
        )
        .unwrap();
        assert_eq!(runtime.round_tag(), next);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
        assert_eq!(
            effects,
            vec![FakeEffect::enter_view(next), FakeEffect::other()]
        );
        assert!(matches!(
            runtime.step(start + Duration::from_secs(100)),
            Err(RuntimeError::ClocksNotArmed)
        ));
        runtime
            .arm_live_clocks(start + Duration::from_secs(100))
            .expect("arm after startup effects are dispatched");
        assert_eq!(
            runtime.arm_live_clocks(start + Duration::from_secs(101)),
            Err(RuntimeClockError::AlreadyArmed)
        );
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(119)),
            Ok(RuntimeStep::Advanced(_)) | Ok(RuntimeStep::Idle)
        ));
        assert!(runtime.driver.timeouts.is_empty());
        let _ =
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(120));
        assert_eq!(runtime.driver.timeouts, vec![next]);
    }

    #[test]
    fn interrupted_tip_recovery_drains_ingress_without_arming_live_timers() {
        let start = Instant::now();
        let initial = tag(0);
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("open unarmed recovery runtime");
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("queue local recovery completion");

        assert!(matches!(
            runtime.step_recovery_and_take_scheduler_ownership_for_test(
                start + Duration::from_secs(1_000)
            ),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
        assert!(runtime.driver.timeouts.is_empty());
        assert!(runtime.driver.retransmits.is_empty());
        assert!(matches!(
            runtime.step_recovery_and_take_scheduler_ownership_for_test(
                start + Duration::from_secs(2_000)
            ),
            Ok(RuntimeStep::Idle)
        ));
    }

    #[test]
    fn interrupted_tip_recovery_is_rejected_after_live_clock_arm() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );

        assert!(matches!(
            runtime.step_recovery(start),
            Err(RuntimeError::RecoveryAfterClocksArmed)
        ));
    }

    #[test]
    fn adapter_failure_closes_runtime_permanently() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::fail(),
        )
        .unwrap();
        assert!(matches!(
            runtime.step(start),
            Err(RuntimeError::Driver(FakeError))
        ));
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("runtime driver rejected a serialized transition: fake driver failure")
        );
        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("runtime driver rejected a serialized transition: fake driver failure"),
            "the generic closed guard cannot replace the driver root cause"
        );
    }

    #[test]
    fn invalid_configuration_is_rejected() {
        let start = Instant::now();
        let initial = tag(0);
        let result = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::ZERO,
            RuntimeQueueConfig::new(4, 1, 1),
            Vec::<FakeEffect>::new(),
        );
        assert!(matches!(
            result,
            Err(RuntimeConfigError::InvalidRoundTimeout)
        ));

        let invalid_queue = RuntimeQueueConfig::new(2, 1, 1).validate();
        assert_eq!(
            invalid_queue,
            Err(RuntimeConfigError::InvalidQueueAllocation)
        );
    }
}
