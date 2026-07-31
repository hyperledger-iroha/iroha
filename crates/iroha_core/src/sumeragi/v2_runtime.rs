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
//! Immutable actor-local lifecycle ordinals freeze the predecessor set of every
//! admission. A later timeout, retransmission, causal successor, or higher
//! service class cannot overtake an older live lifecycle. Within the active
//! lifecycle, a small deterministic arbiter and cyclic class service prevent a
//! saturated normal prefix from starving a locked Commit vote or trusted local
//! completion.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::v2_core::{
    EFFECTIVE_LOCK_TRACE_SERVICE, EffectiveLockTraceProjection, EventTag,
    ExactBodyCompletionOwnership, MAX_EFFECTS_PER_STEP,
    ProductionIngressIdentityAndClassTraceProjection,
    ProductionIngressReservationMaterializationTraceProjection, SERVICE_CLASS_COMPLETION,
    SERVICE_CLASS_NONE, SERVICE_CLASS_NORMAL, SERVICE_CLASS_PROGRESS, ScheduleState, ScheduledWork,
    check_production_body_service_effective_lock_transition,
    check_production_ingress_reservation_materialization_transition,
    check_production_ingress_transition, classify_exact_body_completion_ownership,
    select_bounded_service_class,
};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode as _, Encode as _};

use super::{
    FairV2IngressOwnershipEvidence,
    serviced_candidate_store::{
        LeaderWireLifecycleRuntimeReceipt, ProducerContinuationHandoffToken,
        ProducerContinuationTerminalToken,
    },
    v2::{
        AdapterEffect, AdapterError, AuthenticatedConsensusMessage, BodyPipelineCompletionEvidence,
        DecisionLocalProposalDisposition, DeferredAdmissionOrdinalSource, DeferredServiceEvidence,
        ProducerContinuationHandoffEvidence, SumeragiV2Adapter, classify_decided_local_proposal,
        proposal_is_safe_for_lock,
    },
    v2_body_store::{DurableBodyReceipt, ValidatedBodyReceipt},
};

#[cfg(test)]
use super::v2::DeferredPriority;

const RETRANSMIT_DIVISOR: u32 = 5;
const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// Actor-global source for immutable lifecycle admission ordinals.
///
/// Runtime FIFO admissions, fresh clock/effect roots, and the exact Serve
/// ingress gate share one source for the active height. The source stores the
/// next unused ordinal rather than an event count; a durable Serve waiter can
/// therefore seed a restarted actor past its retained high-watermark before
/// any reconstructed runtime owner is minted.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeLifecycleOrdinalSource {
    next: Arc<Mutex<Option<u128>>>,
}

impl RuntimeLifecycleOrdinalSource {
    /// Construct a source strictly after a durable high-watermark.
    pub(crate) fn after_high_watermark(high_watermark: u128) -> Self {
        Self {
            next: Arc::new(Mutex::new(high_watermark.checked_add(1))),
        }
    }

    fn lock_next(&self) -> Result<std::sync::MutexGuard<'_, Option<u128>>, String> {
        self.next
            .lock()
            .map_err(|_| "Sumeragi v2 lifecycle ordinal source was poisoned".to_owned())
    }

    /// Reserve one globally unique ordinal.
    pub(crate) fn reserve_one(&self) -> Result<u128, String> {
        self.reserve_range(1)?
            .0
            .ok_or_else(|| "Sumeragi v2 lifecycle ordinal source returned no owner".to_owned())
    }

    fn prospective_range(
        next: Option<u128>,
        count: usize,
    ) -> Result<(Option<u128>, Option<u128>), String> {
        if count == 0 {
            return Ok((None, next));
        }
        let first = next.ok_or_else(|| {
            "Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned()
        })?;
        let offset = u128::try_from(count - 1)
            .map_err(|_| "Sumeragi v2 lifecycle admission range is not representable".to_owned())?;
        let last = first.checked_add(offset).ok_or_else(|| {
            "Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned()
        })?;
        let successor = last.checked_add(1).ok_or_else(|| {
            "Sumeragi v2 actor-global lifecycle admission ordinal exhausted".to_owned()
        })?;
        Ok((Some(first), Some(successor)))
    }

    /// Hold the actor-global source while a prospective FIFO owner is fully
    /// checked and committed to its local ingress.
    ///
    /// The source advances only after `commit` returns successfully. Holding
    /// the same mutex across the closure prevents another actor from taking
    /// the prospective range between identity validation and local commit.
    fn with_checked_reservation<T>(
        &self,
        count: usize,
        commit: impl FnOnce(u128, u128) -> Result<T, EnqueueError>,
    ) -> Result<T, EnqueueError> {
        if count == 0 {
            return Err(EnqueueError::FailClosed);
        }
        let mut next = self.lock_next().map_err(|_| EnqueueError::FailClosed)?;
        let (first, successor) =
            Self::prospective_range(*next, count).map_err(|_| EnqueueError::FailClosed)?;
        let first = first.ok_or(EnqueueError::FailClosed)?;
        let successor = successor.ok_or(EnqueueError::FailClosed)?;
        let committed = commit(first, successor)?;
        *next = Some(successor);
        Ok(committed)
    }

    /// Hold the source at one already-minted successor while a reservation is
    /// materialized without allocating another ordinal.
    fn with_checked_current<T>(
        &self,
        commit: impl FnOnce(u128) -> Result<T, EnqueueError>,
    ) -> Result<T, EnqueueError> {
        let next = self.lock_next().map_err(|_| EnqueueError::FailClosed)?;
        let current = (*next).ok_or(EnqueueError::FailClosed)?;
        commit(current)
    }

    /// Return whether two handles share the same actor-global ordinal source.
    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.next, &other.next)
    }

    fn reserve_range(&self, count: usize) -> Result<(Option<u128>, Option<u128>), String> {
        let mut next = self.lock_next()?;
        let reserved = Self::prospective_range(*next, count)?;
        if count != 0 {
            *next = reserved.1;
        }
        Ok(reserved)
    }

    /// Advance a live source past a high-watermark restored by another owner.
    pub(crate) fn advance_past(&self, high_watermark: u128) -> Result<(), String> {
        let mut next = self.lock_next()?;
        if (*next).is_some_and(|candidate| candidate <= high_watermark) {
            *next = high_watermark.checked_add(1);
        }
        Ok(())
    }

    /// Read the next unused ordinal without reserving it.
    ///
    /// Runtime ingress uses this to initialize its diagnostic mirror from the
    /// same actor-global source that owns all lifecycle reservations.
    pub(super) fn next_ordinal(&self) -> Result<Option<u128>, String> {
        self.lock_next().map(|next| *next)
    }

    /// Inspect the next actor-global lifecycle ordinal in tests.
    #[cfg(test)]
    pub(crate) fn next_ordinal_for_test(&self) -> Result<Option<u128>, String> {
        self.next_ordinal()
    }

    fn recognizes_minted(&self, ordinal: u128) -> Result<bool, String> {
        if ordinal == 0 {
            return Ok(false);
        }
        self.lock_next()
            .map(|next| (*next).is_some_and(|next| ordinal < next))
    }

    #[cfg(test)]
    pub(crate) fn exhaust_for_test(&self) {
        *self
            .next
            .lock()
            .expect("test lifecycle ordinal source is not poisoned") = None;
    }
}

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
    /// Startup lifecycle ownership could not be allocated or validated.
    InvalidLifecycleOwnership,
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
            Self::InvalidLifecycleOwnership => formatter.write_str(
                "Sumeragi v2 runtime could not establish exact startup lifecycle ownership",
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
    /// The initial self-leader proposal lifecycle could not be reserved before
    /// the live timeout clock was armed.
    ProducerReservation,
}

impl fmt::Display for RuntimeClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyArmed => formatter.write_str(
                "Sumeragi v2 live pacemaker clocks may be armed only once after startup",
            ),
            Self::ProducerReservation => formatter.write_str(
                "Sumeragi v2 could not reserve the initial view producer before arming clocks",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
    /// Internal startup, timeout, or retransmission lifecycle root.
    /// This value is scheduler metadata only and is never encoded on the wire.
    LifecycleRoot,
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

/// Constant-size identity retained after a physical command carrier leaves
/// runtime ingress.
///
/// The full canonical bytes are validated while the command and its ingress
/// evidence are still present. Causal successors retain only this digest so a
/// large authenticated payload cannot remain pinned by every asynchronous
/// child of the root lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeCommandIdentityDigest {
    /// Exact root command variant.
    pub(crate) kind: RuntimeCommandKind,
    /// Hash of the deeply validated canonical command projection.
    pub(crate) canonical_hash: iroha_crypto::Hash,
    /// Fixed-size integrity projection. This lets scheduler scans reject
    /// corruption without re-encoding or re-hashing the command payload.
    projection_hash: iroha_crypto::Hash,
}

fn runtime_command_identity_digest_projection_hash(
    identity: &RuntimeCommandIdentityDigest,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-command-digest:v1");
    projection.push(identity.kind.code());
    append_runtime_identity_field(&mut projection, identity.canonical_hash.as_ref());
    iroha_crypto::Hash::new(projection)
}

impl RuntimeCommandIdentityDigest {
    fn new(kind: RuntimeCommandKind, canonical_hash: iroha_crypto::Hash) -> Self {
        let mut identity = Self {
            kind,
            canonical_hash,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        identity.projection_hash = runtime_command_identity_digest_projection_hash(&identity);
        identity
    }

    fn validate_exact(&self) -> bool {
        self.projection_hash == runtime_command_identity_digest_projection_hash(self)
    }
}

impl RuntimeCommandIdentity {
    fn validate_exact(&self) -> bool {
        self.canonical_hash == iroha_crypto::Hash::new(self.canonical_bytes.as_ref())
    }

    fn digest(&self) -> RuntimeCommandIdentityDigest {
        RuntimeCommandIdentityDigest::new(self.kind, self.canonical_hash)
    }
}

/// Exact fair-ingress ownership retained for one authenticated runtime
/// command.
///
/// A Commit-certificate discovery response is authenticated as its outer
/// envelope and then projects the enclosed CommitQC into reducer ingress.
/// Direct QC delivery and discovery-response delivery therefore occupy two
/// independent slots while sharing one immutable runtime command. Each slot
/// retains a protocol-bounded set of independently admitted fair-ingress
/// carriers; direct timeout certificates use the direct slot under the same
/// bound. Identical aggregate certificates can legitimately arrive from every
/// voter, so collapsing the slot to one semantic origin would turn a valid
/// duplicate into a fail-closed ownership mismatch. The bound is exact: once
/// every slot is occupied, a new disjoint carrier is rejected rather than
/// summarized without its source identity.
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
    IndependentOccurrence,
}

impl RuntimeIngressOwnershipEvidence {
    fn leader_wire_token(
        &self,
    ) -> Result<Option<&super::FairV2IngressLeaderWireToken>, RuntimeIngressMergeError> {
        let mut exact: Option<&super::FairV2IngressLeaderWireToken> = None;
        let mut saw_untagged = false;
        for carrier in self
            .direct
            .iter()
            .chain(self.commit_certificate_response.iter())
        {
            match carrier.leader_wire_token() {
                Some(token) => match exact {
                    Some(retained) if retained != token => {
                        return Err(RuntimeIngressMergeError::Conflict);
                    }
                    Some(_) => {}
                    None if saw_untagged => return Err(RuntimeIngressMergeError::Conflict),
                    None => exact = Some(token),
                },
                None if exact.is_some() => return Err(RuntimeIngressMergeError::Conflict),
                None => saw_untagged = true,
            }
        }
        Ok(exact)
    }

    fn leader_wire_runtime_receipt(
        &self,
    ) -> Result<Option<&LeaderWireLifecycleRuntimeReceipt>, RuntimeIngressMergeError> {
        let mut exact: Option<&LeaderWireLifecycleRuntimeReceipt> = None;
        let mut saw_untagged = false;
        for carrier in self
            .direct
            .iter()
            .chain(self.commit_certificate_response.iter())
        {
            match (
                carrier.leader_wire_token(),
                carrier.leader_wire_runtime_receipt(),
            ) {
                (Some(token), Some(receipt)) if receipt.token() == token => match exact {
                    Some(retained) if retained != receipt => {
                        return Err(RuntimeIngressMergeError::Conflict);
                    }
                    Some(_) => {}
                    None if saw_untagged => return Err(RuntimeIngressMergeError::Conflict),
                    None => exact = Some(receipt),
                },
                (None, None) if exact.is_some() => {
                    return Err(RuntimeIngressMergeError::Conflict);
                }
                (None, None) => saw_untagged = true,
                (Some(_), None) | (None, Some(_)) | (Some(_), Some(_)) => {
                    return Err(RuntimeIngressMergeError::Conflict);
                }
            }
        }
        Ok(exact)
    }

    fn leader_wire_scheduler_ordinal(&self) -> Result<Option<u128>, RuntimeIngressMergeError> {
        self.leader_wire_token()
            .map(|token| token.map(super::FairV2IngressLeaderWireToken::scheduler_ordinal))
    }

    fn earliest_lifecycle_ordinal(&self) -> Result<Option<u128>, RuntimeIngressMergeError> {
        let mut earliest = None;
        let mut saw_untagged = false;
        for carrier in self
            .direct
            .iter()
            .chain(self.commit_certificate_response.iter())
        {
            match carrier.runtime_lifecycle_ordinal() {
                Some(_) if saw_untagged => {
                    return Err(RuntimeIngressMergeError::Conflict);
                }
                Some(ordinal) => {
                    earliest = Some(earliest.map_or(ordinal, |current: u128| current.min(ordinal)));
                }
                None if earliest.is_some() => {
                    return Err(RuntimeIngressMergeError::Conflict);
                }
                None => saw_untagged = true,
            }
        }
        Ok(earliest)
    }

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
        let retained_lifecycle = self.earliest_lifecycle_ordinal()?;
        let candidate_lifecycle = candidate.earliest_lifecycle_ordinal()?;
        if retained_lifecycle.is_some() != candidate_lifecycle.is_some() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        let mut runtime_cursor = self.runtime_bytes.as_ref();
        let runtime = wire::ConsensusMessageV2::decode(&mut runtime_cursor)
            .map_err(|_| RuntimeIngressMergeError::Conflict)?;
        if !runtime_cursor.is_empty() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        // Distinct semantic origins are independent requests for proposal,
        // vote, timeout-vote, and transport traffic. Untagged aggregate QCs
        // and TCs may retain a bounded set of disjoint source carriers. Once
        // either side carries a durable leader-wire token, only the exact same
        // token may merge; otherwise two generic lifecycles would disappear
        // behind one runtime owner.
        let aggregate_certificate = matches!(
            runtime.payload,
            wire::ConsensusMessageV2Payload::QuorumCertificate(_)
                | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
        );
        let retained_token = self.leader_wire_token()?;
        let candidate_token = candidate.leader_wire_token()?;
        if (retained_token.is_some() || candidate_token.is_some())
            && retained_token != candidate_token
        {
            return Err(RuntimeIngressMergeError::IndependentOccurrence);
        }
        let allow_disjoint_carriers =
            aggregate_certificate && retained_token.is_none() && candidate_token.is_none();
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
        let leader_wire_token_is_exact = self.leader_wire_token().is_ok();
        let lifecycle_ordinal_is_exact = self.earliest_lifecycle_ordinal().is_ok();
        let leader_wire_runtime_receipt_is_exact = matches!(
            (self.leader_wire_token(), self.leader_wire_runtime_receipt()),
            (Ok(None), Ok(None)) | (Ok(Some(_)), Ok(Some(_)))
        );
        direct_exact
            && response_exact
            && carriers_are_pairwise_disjoint
            && leader_wire_token_is_exact
            && lifecycle_ordinal_is_exact
            && leader_wire_runtime_receipt_is_exact
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
            return Err(RuntimeIngressMergeError::IndependentOccurrence);
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

impl ExactRuntimeCommandIdentity for AuthenticatedConsensusMessage {
    fn exact_runtime_command_identity(&self) -> RuntimeCommandIdentity {
        let canonical_bytes = self.canonical_wire_bytes();
        let canonical_hash = iroha_crypto::Hash::new(&canonical_bytes);
        RuntimeCommandIdentity {
            kind: RuntimeCommandKind::Authenticated,
            canonical_bytes: Arc::from(canonical_bytes),
            canonical_hash,
        }
    }
}

/// Immutable scheduler-local identity of the first admitted command in one
/// causal reducer lifecycle.
///
/// Successor commands may replace their exact command bytes, evidence, service
/// class, or reducer incarnation tag, but retain this value unchanged.  The
/// carrier is process-local metadata: it is neither encoded on the wire nor
/// persisted as a Norito field. Retry/replay may reconstruct the diagnostic
/// tag with a later process generation, so generation is intentionally absent
/// from `lifecycle_key`; height and view remain part of the logical root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeCandidateCausalOrigin {
    /// Constant-size digest of the deeply validated root command.
    pub(crate) root_identity: RuntimeCommandIdentityDigest,
    /// Reducer incarnation at first admission.
    pub(crate) root_tag: EventTag,
    /// Service class frozen at first admission.
    pub(crate) root_class: u8,
    /// Stable semantic ingress key, excluding mutable route/cursor history.
    pub(crate) root_ingress_identity: Option<iroha_crypto::Hash>,
    /// Exact durable leader-wire identity, when this root crossed the generic
    /// ingress lifecycle gate. This key is shared verbatim with producer
    /// continuation evidence; it is internal metadata, never a wire field.
    leader_wire_lifecycle_key: Option<iroha_crypto::Hash>,
    /// Restart-stable producer key recovered from validated local admission
    /// metadata. Causal callbacks cannot reconstruct their original parent
    /// command after a process restart, so this field reattaches the exact
    /// persisted lifecycle key without changing its first-admission ordinal.
    /// It is local scheduler metadata, never a wire or configuration field.
    restored_producer_lifecycle_key: Option<iroha_crypto::Hash>,
    /// Ordinal assigned exactly once when the logical root first acquires a
    /// scheduler position. It is part of the integrity projection but not the
    /// semantic lifecycle key used to recognize a retry.
    root_lifecycle_ordinal: Option<u128>,
    /// Logical lifecycle key, deliberately excluding process generation.
    pub(crate) lifecycle_key: iroha_crypto::Hash,
    /// Integrity hash over the complete carrier, including diagnostic tag.
    pub(crate) projection_hash: iroha_crypto::Hash,
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

fn runtime_ingress_causal_origin_projection_hash(
    evidence: &RuntimeIngressOwnershipEvidence,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-causal-ingress:v1");
    let mut cursor = evidence.runtime_bytes.as_ref();
    let decoded = wire::ConsensusMessageV2::decode(&mut cursor)
        .ok()
        .filter(|_| cursor.is_empty());
    match decoded.as_ref().map(|message| &message.payload) {
        Some(wire::ConsensusMessageV2Payload::QuorumCertificate(certificate)) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, &certificate.as_ref().encode());
        }
        Some(wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate)) => {
            // Authentication has already proved the complete grouped signer
            // carrier. Reducer progress, durable coalescing, and safe-value
            // selection identify the logical occurrence only by certified
            // round plus the selected highest PrepareQC. Equal-quorum carrier
            // replacement must therefore retain one lifecycle owner.
            projection.push(2);
            append_runtime_identity_field(&mut projection, &certificate.round.encode());
            match certificate.highest_prepare_qc() {
                None => projection.push(0),
                Some(highest) => {
                    projection.push(1);
                    append_runtime_identity_field(&mut projection, &highest.as_ref().encode());
                }
            }
        }
        _ => {
            projection.push(3);
            append_runtime_identity_field(&mut projection, evidence.runtime_bytes.as_ref());
            for carriers in [&evidence.direct, &evidence.commit_certificate_response] {
                append_runtime_identity_u64(
                    &mut projection,
                    u64::try_from(carriers.len()).expect("bounded ingress carrier count fits u64"),
                );
                for carrier in carriers {
                    let mut semantic = Vec::new();
                    match carrier.first.wire_key.origin.as_ref() {
                        None => semantic.push(0),
                        Some(origin) => {
                            semantic.push(1);
                            append_runtime_identity_field(&mut semantic, &origin.encode());
                        }
                    }
                    append_runtime_identity_field(
                        &mut semantic,
                        carrier.first.wire_key.hash.as_ref(),
                    );
                    append_runtime_identity_field(&mut projection, &semantic);
                }
            }
        }
    }
    iroha_crypto::Hash::new(projection)
}

fn runtime_candidate_causal_origin_projection_hash(
    origin: &RuntimeCandidateCausalOrigin,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-causal-origin:v1");
    projection.push(origin.root_identity.kind.code());
    append_runtime_identity_field(
        &mut projection,
        origin.root_identity.canonical_hash.as_ref(),
    );
    append_runtime_identity_tag(&mut projection, origin.root_tag);
    projection.push(origin.root_class);
    match &origin.root_ingress_identity {
        None => projection.push(0),
        Some(identity) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, identity.as_ref());
        }
    }
    match &origin.leader_wire_lifecycle_key {
        None => projection.push(0),
        Some(identity) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, identity.as_ref());
        }
    }
    match &origin.restored_producer_lifecycle_key {
        None => projection.push(0),
        Some(identity) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, identity.as_ref());
        }
    }
    match origin.root_lifecycle_ordinal {
        None => projection.push(0),
        Some(ordinal) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, &ordinal.to_le_bytes());
        }
    }
    append_runtime_identity_field(&mut projection, origin.lifecycle_key.as_ref());
    iroha_crypto::Hash::new(projection)
}

fn runtime_candidate_causal_origin_lifecycle_key(
    origin: &RuntimeCandidateCausalOrigin,
) -> iroha_crypto::Hash {
    if let Some(identity) = origin.restored_producer_lifecycle_key {
        return identity;
    }
    if let Some(identity) = origin.leader_wire_lifecycle_key {
        return identity;
    }
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-causal-lifecycle:v1");
    projection.push(origin.root_identity.kind.code());
    append_runtime_identity_field(
        &mut projection,
        origin.root_identity.canonical_hash.as_ref(),
    );
    append_runtime_identity_u64(&mut projection, origin.root_tag.height());
    append_runtime_identity_u64(&mut projection, origin.root_tag.view());
    projection.push(origin.root_class);
    match &origin.root_ingress_identity {
        None => projection.push(0),
        Some(identity) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, identity.as_ref());
        }
    }
    iroha_crypto::Hash::new(projection)
}

impl RuntimeCandidateCausalOrigin {
    fn mint<C: ExactRuntimeCommandIdentity>(
        tag: EventTag,
        class: CommandClass,
        command: &C,
        ingress_ownership: Option<&RuntimeIngressOwnershipEvidence>,
    ) -> Self {
        let exact_identity = command.exact_runtime_command_identity();
        debug_assert!(exact_identity.validate_exact());
        let mut root_identity = exact_identity.digest();
        if root_identity.kind == RuntimeCommandKind::Authenticated
            && let Some(ingress_ownership) = ingress_ownership
        {
            let mut semantic = Vec::new();
            semantic.extend_from_slice(b"iroha:sumeragi:v2:authenticated-causal-root:v1");
            append_runtime_identity_field(
                &mut semantic,
                runtime_ingress_causal_origin_projection_hash(ingress_ownership).as_ref(),
            );
            root_identity = RuntimeCommandIdentityDigest::new(
                RuntimeCommandKind::Authenticated,
                iroha_crypto::Hash::new(semantic),
            );
        }
        let mut origin = Self {
            root_identity,
            root_tag: tag,
            root_class: class.service_code(),
            root_ingress_identity: ingress_ownership
                .map(runtime_ingress_causal_origin_projection_hash),
            leader_wire_lifecycle_key: ingress_ownership.and_then(|ownership| {
                ownership
                    .leader_wire_token()
                    .ok()
                    .flatten()
                    .map(super::FairV2IngressLeaderWireToken::identity_hash)
            }),
            restored_producer_lifecycle_key: None,
            root_lifecycle_ordinal: None,
            lifecycle_key: iroha_crypto::Hash::new([]),
            projection_hash: iroha_crypto::Hash::new([]),
        };
        origin.lifecycle_key = runtime_candidate_causal_origin_lifecycle_key(&origin);
        origin.projection_hash = runtime_candidate_causal_origin_projection_hash(&origin);
        origin
    }

    fn mint_fresh_root(
        tag: EventTag,
        class: CommandClass,
        kind: RuntimeFreshRootKind,
        semantic_identity: &[u8],
    ) -> Self {
        let mut canonical_bytes = Vec::new();
        canonical_bytes.extend_from_slice(b"iroha:sumeragi:v2:fresh-runtime-root:v1");
        canonical_bytes.push(kind.code());
        append_runtime_identity_u64(&mut canonical_bytes, tag.height());
        append_runtime_identity_u64(&mut canonical_bytes, tag.view());
        append_runtime_identity_field(&mut canonical_bytes, semantic_identity);
        let root_identity = RuntimeCommandIdentityDigest::new(
            RuntimeCommandKind::LifecycleRoot,
            iroha_crypto::Hash::new(canonical_bytes),
        );
        let mut origin = Self {
            root_identity,
            root_tag: tag,
            root_class: class.service_code(),
            root_ingress_identity: None,
            leader_wire_lifecycle_key: None,
            restored_producer_lifecycle_key: None,
            root_lifecycle_ordinal: None,
            lifecycle_key: iroha_crypto::Hash::new([]),
            projection_hash: iroha_crypto::Hash::new([]),
        };
        origin.lifecycle_key = runtime_candidate_causal_origin_lifecycle_key(&origin);
        origin.projection_hash = runtime_candidate_causal_origin_projection_hash(&origin);
        origin
    }

    /// Reconstruct an exact persisted producer lifecycle around a replayed
    /// command. The adapter has already matched the complete route-neutral
    /// serviced-candidate identity before returning this key and ordinal.
    fn restore_producer_lifecycle<C: ExactRuntimeCommandIdentity>(
        tag: EventTag,
        class: CommandClass,
        command: &C,
        ingress_ownership: Option<&RuntimeIngressOwnershipEvidence>,
        causal_lifecycle_key: iroha_crypto::Hash,
        admission_ordinal: u128,
    ) -> Result<RuntimeLifecycleOwner, EnqueueError> {
        if admission_ordinal == 0 {
            return Err(EnqueueError::FailClosed);
        }
        let mut origin = Self::mint(tag, class, command, ingress_ownership);
        if origin.restored_producer_lifecycle_key.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        if origin.leader_wire_lifecycle_key.is_some()
            && origin.lifecycle_key != causal_lifecycle_key
        {
            return Err(EnqueueError::FailClosed);
        }
        origin.restored_producer_lifecycle_key = Some(causal_lifecycle_key);
        origin.lifecycle_key = runtime_candidate_causal_origin_lifecycle_key(&origin);
        origin.projection_hash = runtime_candidate_causal_origin_projection_hash(&origin);
        RuntimeLifecycleOwner::new(origin, admission_ordinal)
    }

    fn validate_exact(&self) -> bool {
        self.root_class != SERVICE_CLASS_NONE
            && self.root_identity.validate_exact()
            && self.lifecycle_key == runtime_candidate_causal_origin_lifecycle_key(self)
            && self.projection_hash == runtime_candidate_causal_origin_projection_hash(self)
    }

    fn bind_lifecycle_ordinal(&mut self, lifecycle_ordinal: u128) -> bool {
        match self.root_lifecycle_ordinal {
            Some(existing) if existing != lifecycle_ordinal => return false,
            Some(_) => return self.validate_exact(),
            None => self.root_lifecycle_ordinal = Some(lifecycle_ordinal),
        }
        self.projection_hash = runtime_candidate_causal_origin_projection_hash(self);
        self.validate_exact()
    }

    /// Whether two exact carriers identify one lifecycle despite diagnostic
    /// process-generation retagging.
    pub(crate) fn same_lifecycle(&self, other: &Self) -> bool {
        self.validate_exact() && other.validate_exact() && self.lifecycle_key == other.lifecycle_key
    }
}

/// Existing TLA root constructor mirrored by a process-local scheduler root.
///
/// The value is internal evidence only; it is neither a wire field nor a
/// runtime configuration surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RuntimeFreshRootKind {
    /// `RestartCandidate` reconstructed before live ingress opens.
    StartupRecovery,
    /// `BeginTimeout` frozen when its absolute deadline first becomes due.
    Timeout,
    /// Ordinary `Retransmit` timer episode.
    Retransmit,
    /// `HistoricalLockedRetransmitCandidate` reconstructed from the durable lock.
    HistoricalLockedRetransmit,
    /// `AssembleBody` accepted from the deterministic local proposal builder.
    LocalProposalAdmission,
}

impl RuntimeFreshRootKind {
    const fn code(self) -> u8 {
        match self {
            Self::StartupRecovery => 1,
            Self::Timeout => 2,
            Self::Retransmit => 3,
            Self::HistoricalLockedRetransmit => 4,
            Self::LocalProposalAdmission => 5,
        }
    }
}

/// Immutable logical owner transferred from ingress through every local
/// asynchronous causal successor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeLifecycleOwner {
    causal_origin: RuntimeCandidateCausalOrigin,
    lifecycle_ordinal: u128,
    projection_hash: iroha_crypto::Hash,
}

impl RuntimeLifecycleOwner {
    fn new(
        mut causal_origin: RuntimeCandidateCausalOrigin,
        lifecycle_ordinal: u128,
    ) -> Result<Self, EnqueueError> {
        if !causal_origin.bind_lifecycle_ordinal(lifecycle_ordinal) {
            return Err(EnqueueError::FailClosed);
        }
        let mut owner = Self {
            causal_origin,
            lifecycle_ordinal,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        owner.projection_hash = runtime_lifecycle_owner_projection_hash(&owner);
        owner
            .validate_exact()
            .then_some(owner)
            .ok_or(EnqueueError::FailClosed)
    }

    fn validate_exact(&self) -> bool {
        self.causal_origin.validate_exact()
            && self.causal_origin.root_lifecycle_ordinal == Some(self.lifecycle_ordinal)
            && self.projection_hash == runtime_lifecycle_owner_projection_hash(self)
    }

    /// Immutable first-admission origin.
    pub(crate) const fn causal_origin(&self) -> &RuntimeCandidateCausalOrigin {
        &self.causal_origin
    }

    /// Monotone actor-local lifecycle ordinal.
    pub(crate) const fn lifecycle_ordinal(&self) -> u128 {
        self.lifecycle_ordinal
    }

    fn rebase_deferred_ingress(
        &self,
        lifecycle_ordinal: u128,
        ingress_identity: iroha_crypto::Hash,
    ) -> Result<Self, RuntimeIngressMergeError> {
        if lifecycle_ordinal == 0 || lifecycle_ordinal > self.lifecycle_ordinal {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        if lifecycle_ordinal == self.lifecycle_ordinal {
            return Ok(self.clone());
        }
        if self.causal_origin.restored_producer_lifecycle_key.is_some()
            || self.causal_origin.leader_wire_lifecycle_key.is_some()
            || self.causal_origin.root_ingress_identity != Some(ingress_identity)
        {
            return Err(RuntimeIngressMergeError::IndependentOccurrence);
        }
        let mut causal_origin = self.causal_origin.clone();
        causal_origin.root_lifecycle_ordinal = Some(lifecycle_ordinal);
        causal_origin.projection_hash =
            runtime_candidate_causal_origin_projection_hash(&causal_origin);
        let mut rebased = Self {
            causal_origin,
            lifecycle_ordinal,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        rebased.projection_hash = runtime_lifecycle_owner_projection_hash(&rebased);
        rebased
            .validate_exact()
            .then_some(rebased)
            .ok_or(RuntimeIngressMergeError::Conflict)
    }
}

fn runtime_lifecycle_owner_projection_hash(owner: &RuntimeLifecycleOwner) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-lifecycle-owner:v1");
    append_runtime_identity_field(
        &mut projection,
        owner.causal_origin.projection_hash.as_ref(),
    );
    append_runtime_identity_field(&mut projection, &owner.lifecycle_ordinal.to_le_bytes());
    iroha_crypto::Hash::new(projection)
}

/// How one effect acquired its immutable lifecycle owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeEffectCausality {
    /// The effect is a causal child of the selected command or clock root.
    Inherit,
    /// The effect is an independently reconstructed TLA root.
    Fresh(RuntimeFreshRootKind),
}

/// Sidecar metadata paired positionally with one reducer effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeEffectOwnership {
    owner: RuntimeLifecycleOwner,
    causality: RuntimeEffectCausality,
}

impl RuntimeEffectOwnership {
    fn inherited(owner: RuntimeLifecycleOwner) -> Self {
        Self {
            owner,
            causality: RuntimeEffectCausality::Inherit,
        }
    }

    fn fresh(owner: RuntimeLifecycleOwner, kind: RuntimeFreshRootKind) -> Self {
        Self {
            owner,
            causality: RuntimeEffectCausality::Fresh(kind),
        }
    }

    fn validate_exact(&self) -> bool {
        self.owner.validate_exact()
    }

    /// Immutable owner carried into an asynchronous task or completion.
    pub(crate) const fn owner(&self) -> &RuntimeLifecycleOwner {
        &self.owner
    }

    /// Frozen inherit/fresh classification. Retries and rebinds retain it.
    pub(crate) const fn causality(&self) -> RuntimeEffectCausality {
        self.causality
    }

    #[cfg(test)]
    pub(crate) fn fresh_for_test(tag: EventTag, lifecycle_ordinal: u128) -> Self {
        let kind = RuntimeFreshRootKind::StartupRecovery;
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            tag,
            CommandClass::Progress,
            kind,
            b"test-runtime-effect",
        );
        Self::fresh(
            RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
                .expect("fresh test owner binds its first lifecycle ordinal"),
            kind,
        )
    }
}

/// One exact FIFO candidate selected by the class-aware service cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeFifoCandidateOwnership {
    /// Identity derived from the selected command itself.
    pub(crate) identity: RuntimeCommandIdentityDigest,
    /// Redundant explicit kind pinned to the derived identity.
    pub(crate) kind: RuntimeCommandKind,
    /// Frozen service class assigned at admission.
    pub(crate) class: u8,
    /// Exact reducer incarnation tag retained by the queue owner.
    pub(crate) tag: EventTag,
    /// Process-local ordinal minted when this owner entered the runtime queue.
    pub(crate) admission_ordinal: u128,
    /// Immutable root lifecycle ordinal inherited by every causal successor.
    /// This is distinct from the unique physical FIFO admission ordinal.
    pub(crate) lifecycle_ordinal: u128,
    /// Immutable first-admission root retained across causal successors.
    pub(crate) causal_origin: RuntimeCandidateCausalOrigin,
    /// Constant-size digest of the deeply validated fair-ingress carrier.
    /// Local trusted completions and timers never own one.
    pub(crate) ingress_projection_hash: Option<iroha_crypto::Hash>,
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
    /// A live FIFO command encountered retryable adapter backpressure and was
    /// restored with its immutable admission and lifecycle owner intact.
    FifoRetryRetained,
    /// No live owner was ready.
    Idle,
    /// One startup-recovery FIFO command.
    RecoveryFifo,
    /// A startup-recovery FIFO command encountered retryable adapter
    /// backpressure and was restored without minting a new owner.
    RecoveryFifoRetryRetained,
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
            Self::LifecycleRoot => 9,
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
            Self::FifoRetryRetained => 8,
            Self::RecoveryFifoRetryRetained => 9,
        }
    }
}

fn runtime_fifo_candidate_projection_hash(
    candidate: &RuntimeFifoCandidateOwnership,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.push(candidate.kind.code());
    append_runtime_identity_field(&mut projection, candidate.identity.canonical_hash.as_ref());
    projection.push(candidate.class);
    append_runtime_identity_tag(&mut projection, candidate.tag);
    append_runtime_identity_field(&mut projection, &candidate.admission_ordinal.to_le_bytes());
    append_runtime_identity_field(&mut projection, &candidate.lifecycle_ordinal.to_le_bytes());
    append_runtime_identity_field(
        &mut projection,
        candidate.causal_origin.projection_hash.as_ref(),
    );
    match &candidate.ingress_projection_hash {
        None => projection.push(0),
        Some(projection_hash) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, projection_hash.as_ref());
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
            || (self.fifo_ready && self.queue_before.len == 0)
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
                RuntimeSelectedOwnerKind::Fifo
                | RuntimeSelectedOwnerKind::FifoRetryRetained
                | RuntimeSelectedOwnerKind::RecoveryFifo
                | RuntimeSelectedOwnerKind::RecoveryFifoRetryRetained,
                RuntimeSelectedCandidateOwnership::Exact(candidate),
            ) => {
                let (recovery, retry_retained) = match self.selected {
                    RuntimeSelectedOwnerKind::Fifo => (false, false),
                    RuntimeSelectedOwnerKind::FifoRetryRetained => (false, true),
                    RuntimeSelectedOwnerKind::RecoveryFifo => (true, false),
                    RuntimeSelectedOwnerKind::RecoveryFifoRetryRetained => (true, true),
                    _ => unreachable!("match arm contains only FIFO selections"),
                };
                let service = select_bounded_service_class(
                    self.queue_before.service_cursor,
                    self.completion_ready,
                    self.progress_ready,
                    self.normal_ready,
                );
                let exact = candidate.identity.validate_exact()
                    && candidate.kind == candidate.identity.kind
                    && match candidate.kind {
                        RuntimeCommandKind::Authenticated => {
                            candidate.ingress_projection_hash.is_some()
                        }
                        _ => candidate.ingress_projection_hash.is_none(),
                    }
                    && candidate.projection_hash
                        == runtime_fifo_candidate_projection_hash(candidate)
                    && candidate.causal_origin.validate_exact()
                    && candidate.class != SERVICE_CLASS_NONE
                    && service.selected == candidate.class
                    && service.next == self.queue_after.service_cursor
                    && candidate.fifo_position < self.queue_before.len
                    && candidate.eligible_skips_after == 0
                    && if retry_retained {
                        self.queue_after.len == self.queue_before.len
                    } else {
                        self.queue_after.len.checked_add(1) == Some(self.queue_before.len)
                    }
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

#[derive(Clone)]
pub(crate) struct TaggedCommand<C> {
    tag: EventTag,
    class: CommandClass,
    command: C,
    identity: RuntimeCommandIdentityDigest,
    identity_deep_validated: bool,
    causal_origin: RuntimeCandidateCausalOrigin,
    admitted_at: Instant,
    eligible_skips: u64,
    admission_ordinal: Option<u128>,
    lifecycle_ordinal: Option<u128>,
    restored_producer_stage: Option<u8>,
    ingress_ownership: Option<RuntimeIngressOwnershipEvidence>,
}

impl<C: ExactRuntimeCommandIdentity> TaggedCommand<C> {
    fn new(tag: EventTag, class: CommandClass, command: C, admitted_at: Instant) -> Self {
        let exact_identity = command.exact_runtime_command_identity();
        let identity_deep_validated = exact_identity.validate_exact();
        let identity = exact_identity.digest();
        let causal_origin = RuntimeCandidateCausalOrigin::mint(tag, class, &command, None);
        Self {
            tag,
            class,
            command,
            identity,
            identity_deep_validated,
            causal_origin,
            admitted_at,
            eligible_skips: 0,
            admission_ordinal: None,
            lifecycle_ordinal: None,
            restored_producer_stage: None,
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
        let exact_identity = command.exact_runtime_command_identity();
        let mut identity_deep_validated = exact_identity.validate_exact()
            && ingress_ownership.validate_exact()
            && ingress_ownership.runtime_bytes.as_ref() == exact_identity.canonical_bytes.as_ref();
        let identity = exact_identity.digest();
        let mut causal_origin =
            RuntimeCandidateCausalOrigin::mint(tag, class, &command, Some(&ingress_ownership));
        let lifecycle_ordinal = match ingress_ownership.earliest_lifecycle_ordinal() {
            Ok(Some(ordinal)) if causal_origin.bind_lifecycle_ordinal(ordinal) => Some(ordinal),
            Ok(Some(_)) | Err(_) => {
                identity_deep_validated = false;
                None
            }
            Ok(None) => None,
        };
        Self {
            tag,
            class,
            command,
            identity,
            identity_deep_validated,
            causal_origin,
            admitted_at,
            eligible_skips: 0,
            admission_ordinal: None,
            lifecycle_ordinal,
            restored_producer_stage: None,
            ingress_ownership: Some(ingress_ownership),
        }
    }

    /// Construct a causal successor while retaining the first-admission root.
    ///
    /// The successor may deliberately rewrite command evidence, class, or
    /// reducer view/generation; none of those mutable work coordinates can
    /// replace its immutable lifecycle key.
    fn with_causal_origin(
        tag: EventTag,
        class: CommandClass,
        command: C,
        admitted_at: Instant,
        mut causal_origin: RuntimeCandidateCausalOrigin,
        lifecycle_ordinal: u128,
    ) -> Result<Self, EnqueueError> {
        let exact_identity = command.exact_runtime_command_identity();
        let identity_deep_validated = exact_identity.validate_exact();
        let identity = exact_identity.digest();
        if !causal_origin.bind_lifecycle_ordinal(lifecycle_ordinal) {
            return Err(EnqueueError::FailClosed);
        }
        Ok(Self {
            tag,
            class,
            command,
            identity,
            identity_deep_validated,
            causal_origin,
            admitted_at,
            eligible_skips: 0,
            admission_ordinal: None,
            lifecycle_ordinal: Some(lifecycle_ordinal),
            restored_producer_stage: None,
            ingress_ownership: None,
        })
    }

    fn lifecycle_owner(&self) -> Result<RuntimeLifecycleOwner, EnqueueError> {
        let lifecycle_ordinal = self.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
        RuntimeLifecycleOwner::new(self.causal_origin.clone(), lifecycle_ordinal)
    }

    /// Install a newly reconciled ingress carrier set before this queued
    /// command dispatches.
    ///
    /// Aggregate certificates may be admitted from a later fair-ingress lane
    /// before an older frozen carrier becomes downstream-admissible. In that
    /// case the queued command atomically adopts the older exact lifecycle
    /// root. Same-semantic retries retain the already-owned root.
    fn install_merged_ingress_ownership(
        &mut self,
        ingress_ownership: RuntimeIngressOwnershipEvidence,
    ) -> Result<(), RuntimeIngressMergeError> {
        if self.identity.kind != RuntimeCommandKind::Authenticated
            || !self.validate_admission_identity()
            || !ingress_ownership.validate_exact()
        {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        let retained_lifecycle = self
            .lifecycle_ordinal
            .ok_or(RuntimeIngressMergeError::Conflict)?;
        let lifecycle_ordinal = ingress_ownership
            .earliest_lifecycle_ordinal()?
            .unwrap_or(retained_lifecycle);
        if lifecycle_ordinal > retained_lifecycle {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        if lifecycle_ordinal < retained_lifecycle
            && (self.restored_producer_stage.is_some()
                || ingress_ownership.leader_wire_token()?.is_some())
        {
            return Err(RuntimeIngressMergeError::IndependentOccurrence);
        }
        let mut causal_origin = RuntimeCandidateCausalOrigin::mint(
            self.tag,
            self.class,
            &self.command,
            Some(&ingress_ownership),
        );
        if !causal_origin.bind_lifecycle_ordinal(lifecycle_ordinal) {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        if causal_origin.lifecycle_key != self.causal_origin.lifecycle_key {
            return Err(RuntimeIngressMergeError::IndependentOccurrence);
        }
        if lifecycle_ordinal == retained_lifecycle && causal_origin != self.causal_origin {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        self.causal_origin = causal_origin;
        self.lifecycle_ordinal = Some(lifecycle_ordinal);
        self.ingress_ownership = Some(ingress_ownership);
        debug_assert!(self.validate_admission_identity());
        Ok(())
    }

    fn validate_admission_identity(&self) -> bool {
        self.identity_deep_validated
            && self.identity.validate_exact()
            && self.restored_producer_stage.is_none_or(|_| {
                self.lifecycle_ordinal.is_some()
                    && self.causal_origin.restored_producer_lifecycle_key.is_some()
            })
            && match self.identity.kind {
                RuntimeCommandKind::Authenticated => {
                    self.ingress_ownership.as_ref().is_some_and(|ownership| {
                        ownership.validate_exact()
                            && match ownership.earliest_lifecycle_ordinal() {
                                Ok(Some(ordinal)) => self.lifecycle_ordinal == Some(ordinal),
                                Ok(None) => true,
                                Err(_) => false,
                            }
                    })
                }
                _ => self.ingress_ownership.is_none(),
            }
    }
}

struct BoundedIngress<C> {
    config: RuntimeQueueConfig,
    commands: VecDeque<TaggedCommand<C>>,
    /// Restart-restored Local stages which already consume their eventual
    /// physical FIFO position. Each exact replay atomically replaces one entry.
    dormant_local_fifo_reservations: BTreeSet<RuntimeDormantLocalFifoReservation>,
    next_class: CommandClass,
    lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    /// Diagnostic mirror of the shared source after this ingress's last mint.
    /// Other actor owners may advance the source between runtime admissions.
    next_admission_ordinal: Option<u128>,
    reserved_body_available: Option<BodyAvailableReservation>,
}

impl<C: ExactRuntimeCommandIdentity> BoundedIngress<C> {
    fn new(config: RuntimeQueueConfig) -> Self {
        Self::with_lifecycle_ordinals(
            config,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
    }

    fn with_lifecycle_ordinals(
        config: RuntimeQueueConfig,
        lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    ) -> Self {
        let next_admission_ordinal = lifecycle_ordinals
            .next_ordinal()
            .expect("new lifecycle ordinal source is not poisoned");
        Self {
            config,
            commands: VecDeque::with_capacity(config.capacity),
            dormant_local_fifo_reservations: BTreeSet::new(),
            next_class: CommandClass::Completion,
            lifecycle_ordinals,
            next_admission_ordinal,
            reserved_body_available: None,
        }
    }

    fn enqueue(&mut self, command: TaggedCommand<C>) -> Result<(), EnqueueError> {
        self.enqueue_classified_command(command)
    }

    fn install_dormant_local_fifo_reservations(
        &mut self,
        reservations: Vec<RuntimeDormantLocalFifoReservation>,
    ) -> Result<(), EnqueueError> {
        if !self.commands.is_empty()
            || self.reserved_body_available.is_some()
            || !self.dormant_local_fifo_reservations.is_empty()
        {
            return Err(EnqueueError::FailClosed);
        }
        let reservation_count = reservations.len();
        let reservations = reservations
            .into_iter()
            .collect::<BTreeSet<RuntimeDormantLocalFifoReservation>>();
        if reservations.len() != reservation_count
            || reservations.iter().any(|reservation| {
                reservation.admission_ordinal == 0
                    || reservation.class != CommandClass::Completion
                    || !RuntimeDormantLocalFifoReservation::is_local_fifo_stage(
                        reservation.producer_stage,
                    )
                    || !self
                        .lifecycle_ordinals
                        .recognizes_minted(reservation.admission_ordinal)
                        .unwrap_or(false)
            })
        {
            return Err(EnqueueError::FailClosed);
        }
        let total = reservations.len();
        let progress = reservations
            .iter()
            .filter(|reservation| reservation.class == CommandClass::Progress)
            .count();
        let normal = reservations
            .iter()
            .filter(|reservation| reservation.class == CommandClass::Normal)
            .count();
        if normal > self.config.normal_limit()
            || normal.saturating_add(progress) > self.config.progress_limit()
            || total > self.config.capacity
        {
            return Err(EnqueueError::FailClosed);
        }
        self.dormant_local_fifo_reservations = reservations;
        Ok(())
    }

    fn restored_producer_alias_in<'a>(
        command: &TaggedCommand<C>,
        queued: impl Iterator<Item = &'a TaggedCommand<C>>,
    ) -> Result<bool, EnqueueError>
    where
        C: 'a,
    {
        let Some(producer_stage) = command.restored_producer_stage else {
            return Ok(false);
        };
        let lifecycle_ordinal = command.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
        let causal_lifecycle_key = command
            .causal_origin
            .restored_producer_lifecycle_key
            .ok_or(EnqueueError::FailClosed)?;
        let mut coalesced = false;
        for existing in queued.filter(|existing| {
            existing.causal_origin.restored_producer_lifecycle_key == Some(causal_lifecycle_key)
        }) {
            // The ordinal belongs to the whole restored lifecycle, while the
            // stage distinguishes causal successors within that lifecycle.
            if existing.lifecycle_ordinal != Some(lifecycle_ordinal) {
                return Err(EnqueueError::FailClosed);
            }
            if existing.restored_producer_stage != Some(producer_stage) {
                continue;
            }
            if existing.tag != command.tag
                || existing.class != command.class
                || existing.identity != command.identity
                || existing.causal_origin != command.causal_origin
                || existing.ingress_ownership != command.ingress_ownership
            {
                return Err(EnqueueError::FailClosed);
            }
            coalesced = true;
        }
        Ok(coalesced)
    }

    fn enqueue_classified_command(
        &mut self,
        mut command: TaggedCommand<C>,
    ) -> Result<(), EnqueueError> {
        if !command.validate_admission_identity() {
            return Err(EnqueueError::FailClosed);
        }
        if Self::restored_producer_alias_in(&command, self.commands.iter())? {
            return Ok(());
        }
        let dormant_replacement = self.dormant_local_fifo_replacement(&command)?;
        self.validate_preassigned_lifecycle_owner(&command, &[])?;
        self.check_capacity_change(command.class, usize::from(dormant_replacement.is_some()), 1)?;
        self.with_checked_admission_ordinal_range(
            1,
            move |ingress, admission_ordinal, successor| {
                command.admission_ordinal = Some(admission_ordinal);
                if command
                    .lifecycle_ordinal
                    .is_some_and(|ordinal| ordinal >= admission_ordinal)
                {
                    return Err(EnqueueError::FailClosed);
                }
                if command.lifecycle_ordinal.is_none() {
                    command.lifecycle_ordinal = Some(admission_ordinal);
                }
                let lifecycle_ordinal =
                    command.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
                if !command
                    .causal_origin
                    .bind_lifecycle_ordinal(lifecycle_ordinal)
                {
                    return Err(EnqueueError::FailClosed);
                }
                let incoming_tag = command.tag;
                let incoming_class = command.class.service_code();
                let occupied_before = ingress
                    .commands
                    .len()
                    .checked_add(usize::from(ingress.reserved_body_available.is_some()))
                    .ok_or(EnqueueError::FailClosed)?;
                let queue_len_before = u64::try_from(occupied_before)
                    .expect("bounded runtime ingress length is representable as u64");
                let queue_len_after = queue_len_before
                    .checked_add(1)
                    .ok_or(EnqueueError::FailClosed)?;
                let dormant_reservations_before =
                    u64::try_from(ingress.active_dormant_local_fifo_reservation_count()?)
                        .map_err(|_| EnqueueError::FailClosed)?;
                let (dormant_reservations_after, dormant_owner_ordinal) =
                    if let Some(reservation) = dormant_replacement.as_ref() {
                        if !ingress
                            .dormant_local_fifo_reservations
                            .contains(reservation)
                        {
                            return Err(EnqueueError::FailClosed);
                        }
                        (
                            dormant_reservations_before
                                .checked_sub(1)
                                .ok_or(EnqueueError::FailClosed)?,
                            reservation.admission_ordinal,
                        )
                    } else {
                        (dormant_reservations_before, 0)
                    };
                let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {
                    incoming_height: incoming_tag.height(),
                    incoming_view: incoming_tag.view(),
                    incoming_generation: incoming_tag.generation().get(),
                    incoming_class,
                    stored_height: command.tag.height(),
                    stored_view: command.tag.view(),
                    stored_generation: command.tag.generation().get(),
                    stored_class: command.class.service_code(),
                    queue_len_before,
                    queue_len_after,
                    queue_capacity: u64::try_from(ingress.config.capacity)
                        .expect("bounded runtime ingress capacity is representable as u64"),
                    ordinal_source_before: admission_ordinal,
                    physical_admission_ordinal: admission_ordinal,
                    lifecycle_ordinal,
                    ordinal_source_after: successor,
                    dormant_reservations_before,
                    dormant_reservations_after,
                    dormant_owner_ordinal,
                    ordinal_minted: true,
                };
                let checked_transition = check_production_ingress_transition(ingress_trace)
                    .ok_or(EnqueueError::FailClosed)?;
                let _authorized_transition = checked_transition.into_projection();

                // Infallible commit tail: the source mutex remains held until the
                // exact dormant replacement and queue publication both complete.
                if let Some(reservation) = dormant_replacement.as_ref() {
                    let removed = ingress.dormant_local_fifo_reservations.remove(reservation);
                    debug_assert!(removed);
                }
                ingress.commands.push_back(command);
                Ok(())
            },
        )
    }

    fn enqueue_completion_batch(
        &mut self,
        commands: Vec<TaggedCommand<C>>,
    ) -> Result<(), EnqueueError> {
        if commands.iter().any(|command| {
            command.class != CommandClass::Completion || !command.validate_admission_identity()
        }) {
            return Err(EnqueueError::FailClosed);
        }
        let mut deduplicated = Vec::with_capacity(commands.len());
        for command in commands {
            if Self::restored_producer_alias_in(&command, self.commands.iter())?
                || Self::restored_producer_alias_in(&command, deduplicated.iter())?
            {
                continue;
            }
            deduplicated.push(command);
        }
        let mut commands = deduplicated;
        let dormant_replacements = commands
            .iter()
            .map(|command| self.dormant_local_fifo_replacement(command))
            .collect::<Result<Vec<_>, _>>()?;
        let unique_dormant_replacements = dormant_replacements
            .iter()
            .flatten()
            .cloned()
            .collect::<BTreeSet<_>>();
        if unique_dormant_replacements.len() != dormant_replacements.iter().flatten().count() {
            return Err(EnqueueError::FailClosed);
        }
        for (index, command) in commands.iter().enumerate() {
            self.validate_preassigned_lifecycle_owner(command, &commands[..index])?;
        }
        self.check_capacity_change(
            CommandClass::Completion,
            unique_dormant_replacements.len(),
            commands.len(),
        )?;
        if commands.is_empty() {
            return Ok(());
        }
        let command_count = commands.len();
        self.with_checked_admission_ordinal_range(
            command_count,
            move |ingress, first_ordinal, ordinal_successor| {
                for (offset, command) in commands.iter_mut().enumerate() {
                    let offset = u128::try_from(offset).map_err(|_| EnqueueError::FailClosed)?;
                    let physical_ordinal = first_ordinal
                        .checked_add(offset)
                        .ok_or(EnqueueError::FailClosed)?;
                    command.admission_ordinal = Some(physical_ordinal);
                    if command
                        .lifecycle_ordinal
                        .is_some_and(|ordinal| ordinal >= physical_ordinal)
                    {
                        return Err(EnqueueError::FailClosed);
                    }
                    if command.lifecycle_ordinal.is_none() {
                        command.lifecycle_ordinal = Some(physical_ordinal);
                    }
                    let lifecycle_ordinal =
                        command.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
                    if !command
                        .causal_origin
                        .bind_lifecycle_ordinal(lifecycle_ordinal)
                    {
                        return Err(EnqueueError::FailClosed);
                    }
                }

                let occupied_at_start = ingress
                    .commands
                    .len()
                    .checked_add(usize::from(ingress.reserved_body_available.is_some()))
                    .ok_or(EnqueueError::FailClosed)?;
                let queue_len_at_start = u64::try_from(occupied_at_start)
                    .expect("bounded runtime ingress length is representable as u64");
                let dormant_count_at_start =
                    u64::try_from(ingress.active_dormant_local_fifo_reservation_count()?)
                        .map_err(|_| EnqueueError::FailClosed)?;
                let mut checked_transitions = Vec::with_capacity(commands.len());
                let mut removed_dormant = 0_u64;
                for (offset, (command, dormant_replacement)) in
                    commands.iter().zip(dormant_replacements.iter()).enumerate()
                {
                    let incoming_tag = command.tag;
                    let incoming_class = command.class.service_code();
                    let queue_offset =
                        u64::try_from(offset).map_err(|_| EnqueueError::FailClosed)?;
                    let ordinal_offset =
                        u128::try_from(offset).map_err(|_| EnqueueError::FailClosed)?;
                    let physical_ordinal = first_ordinal
                        .checked_add(ordinal_offset)
                        .ok_or(EnqueueError::FailClosed)?;
                    let source_after = physical_ordinal
                        .checked_add(1)
                        .ok_or(EnqueueError::FailClosed)?;
                    let queue_len_before = queue_len_at_start
                        .checked_add(queue_offset)
                        .ok_or(EnqueueError::FailClosed)?;
                    let queue_len_after = queue_len_before
                        .checked_add(1)
                        .ok_or(EnqueueError::FailClosed)?;
                    let dormant_reservations_before = dormant_count_at_start
                        .checked_sub(removed_dormant)
                        .ok_or(EnqueueError::FailClosed)?;
                    let (dormant_reservations_after, dormant_owner_ordinal) =
                        if let Some(reservation) = dormant_replacement {
                            if !ingress
                                .dormant_local_fifo_reservations
                                .contains(reservation)
                            {
                                return Err(EnqueueError::FailClosed);
                            }
                            removed_dormant = removed_dormant
                                .checked_add(1)
                                .ok_or(EnqueueError::FailClosed)?;
                            (
                                dormant_reservations_before
                                    .checked_sub(1)
                                    .ok_or(EnqueueError::FailClosed)?,
                                reservation.admission_ordinal,
                            )
                        } else {
                            (dormant_reservations_before, 0)
                        };
                    let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {
                        incoming_height: incoming_tag.height(),
                        incoming_view: incoming_tag.view(),
                        incoming_generation: incoming_tag.generation().get(),
                        incoming_class,
                        stored_height: command.tag.height(),
                        stored_view: command.tag.view(),
                        stored_generation: command.tag.generation().get(),
                        stored_class: command.class.service_code(),
                        queue_len_before,
                        queue_len_after,
                        queue_capacity: u64::try_from(ingress.config.capacity)
                            .expect("bounded runtime ingress capacity is representable as u64"),
                        ordinal_source_before: physical_ordinal,
                        physical_admission_ordinal: physical_ordinal,
                        lifecycle_ordinal: command
                            .lifecycle_ordinal
                            .ok_or(EnqueueError::FailClosed)?,
                        ordinal_source_after: source_after,
                        dormant_reservations_before,
                        dormant_reservations_after,
                        dormant_owner_ordinal,
                        ordinal_minted: true,
                    };
                    checked_transitions.push(
                        check_production_ingress_transition(ingress_trace)
                            .ok_or(EnqueueError::FailClosed)?
                            .into_projection(),
                    );
                }
                if ordinal_successor
                    != first_ordinal
                        .checked_add(
                            u128::try_from(command_count).map_err(|_| EnqueueError::FailClosed)?,
                        )
                        .ok_or(EnqueueError::FailClosed)?
                    || unique_dormant_replacements.iter().any(|reservation| {
                        !ingress
                            .dormant_local_fifo_reservations
                            .contains(reservation)
                    })
                {
                    return Err(EnqueueError::FailClosed);
                }
                drop(checked_transitions);

                // Infallible commit tail under the ordinal-source mutex.
                for reservation in &unique_dormant_replacements {
                    let removed = ingress.dormant_local_fifo_reservations.remove(reservation);
                    debug_assert!(removed);
                }
                ingress.commands.extend(commands);
                Ok(())
            },
        )
    }

    fn reserve_admission_ordinal_range(
        &mut self,
        count: usize,
    ) -> Result<(Option<u128>, Option<u128>), EnqueueError> {
        let reserved = self
            .lifecycle_ordinals
            .reserve_range(count)
            .map_err(|_| EnqueueError::FailClosed)?;
        self.next_admission_ordinal = reserved.1;
        Ok(reserved)
    }

    /// Atomically validate and publish one or more physical FIFO owners.
    ///
    /// The closure runs while the shared ordinal source is locked. It must put
    /// every fallible check before its local-state commit tail; only a
    /// successful return advances both the diagnostic mirror and the source.
    fn with_checked_admission_ordinal_range<T>(
        &mut self,
        count: usize,
        commit: impl FnOnce(&mut Self, u128, u128) -> Result<T, EnqueueError>,
    ) -> Result<T, EnqueueError> {
        let lifecycle_ordinals = self.lifecycle_ordinals.clone();
        lifecycle_ordinals.with_checked_reservation(count, |first, successor| {
            let committed = commit(self, first, successor)?;
            self.next_admission_ordinal = Some(successor);
            Ok(committed)
        })
    }

    /// Reserve one actor-global ordinal for a non-FIFO lifecycle root.
    ///
    /// Physical FIFO admissions and clock/startup roots deliberately share
    /// this source, so lifecycle age comparisons cannot collide or depend on
    /// a second unbounded counter. A skipped physical ordinal is diagnostic
    /// evidence that the owner entered through a non-FIFO root.
    fn mint_non_fifo_lifecycle_ordinal(&mut self) -> Result<u128, EnqueueError> {
        let (ordinal, successor) = self.reserve_admission_ordinal_range(1)?;
        let ordinal = ordinal.ok_or(EnqueueError::FailClosed)?;
        self.next_admission_ordinal = successor;
        Ok(ordinal)
    }

    fn dormant_local_fifo_replacement(
        &self,
        command: &TaggedCommand<C>,
    ) -> Result<Option<RuntimeDormantLocalFifoReservation>, EnqueueError> {
        self.dormant_local_fifo_replacement_inner(command, false)
    }

    fn dormant_local_fifo_replacement_inner(
        &self,
        command: &TaggedCommand<C>,
        allow_reserved_body_alias: bool,
    ) -> Result<Option<RuntimeDormantLocalFifoReservation>, EnqueueError> {
        let Some(producer_stage) = command.restored_producer_stage else {
            return Ok(None);
        };
        let admission_ordinal = command.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
        let causal_lifecycle_key = command
            .causal_origin
            .restored_producer_lifecycle_key
            .ok_or(EnqueueError::FailClosed)?;
        if !RuntimeDormantLocalFifoReservation::is_known_stage(producer_stage) {
            return Err(EnqueueError::FailClosed);
        }
        let expected = RuntimeDormantLocalFifoReservation {
            causal_lifecycle_key,
            admission_ordinal,
            producer_stage,
            class: command.class,
        };
        if self.dormant_local_fifo_reservations.contains(&expected) {
            if !allow_reserved_body_alias
                && self
                    .reserved_body_available
                    .as_ref()
                    .and_then(|reservation| reservation.dormant_replacement.as_ref())
                    == Some(&expected)
            {
                // The unpublished body token is already the physical alias of
                // this dormant slot. Only its exact materialization may remove
                // the backing record.
                return Err(EnqueueError::FailClosed);
            }
            return Ok(Some(expected));
        }
        if self
            .dormant_local_fifo_reservations
            .iter()
            .any(|reservation| reservation.causal_lifecycle_key == causal_lifecycle_key)
        {
            return Err(EnqueueError::FailClosed);
        }
        if RuntimeDormantLocalFifoReservation::is_local_fifo_stage(producer_stage) {
            // A Local replay may replace only the exact slot installed from
            // its adjacent durable snapshot. Once removed, the lifecycle must
            // coalesce with its physical owner or terminal tombstone.
            return Err(EnqueueError::FailClosed);
        }
        if producer_stage != RuntimeDormantLocalFifoReservation::TIMEOUT_ELAPSED_STAGE {
            // Transport-conditional and pre-store body stages cannot own a
            // restart-dormant continuation at all.
            return Err(EnqueueError::FailClosed);
        }
        // Non-Local restored producer classes retain their separate transport
        // ownership and therefore have no latent FIFO charge. Timeout is the
        // sole locally reconstructible non-FIFO producer stage.
        Ok(None)
    }

    /// Validate a lifecycle position carried in from another actor-owned
    /// stage before this FIFO spends a fresh physical admission position.
    ///
    /// A carried ordinal must have been minted by this exact shared source.
    /// Causal siblings may reuse it only with the identical immutable root;
    /// an unrelated queued, reserved, or restart-dormant owner cannot alias
    /// the position.
    fn validate_preassigned_lifecycle_owner(
        &self,
        command: &TaggedCommand<C>,
        staged: &[TaggedCommand<C>],
    ) -> Result<(), EnqueueError> {
        let Some(lifecycle_ordinal) = command.lifecycle_ordinal else {
            return Ok(());
        };
        if !self
            .lifecycle_ordinals
            .recognizes_minted(lifecycle_ordinal)
            .map_err(|_| EnqueueError::FailClosed)?
            || !command.causal_origin.validate_exact()
            || command.causal_origin.root_lifecycle_ordinal != Some(lifecycle_ordinal)
        {
            return Err(EnqueueError::FailClosed);
        }
        for existing in self.commands.iter().chain(staged.iter()) {
            if existing.lifecycle_ordinal == Some(lifecycle_ordinal)
                && existing.causal_origin != command.causal_origin
            {
                return Err(EnqueueError::FailClosed);
            }
        }
        if self
            .dormant_local_fifo_reservations
            .iter()
            .any(|reservation| {
                reservation.admission_ordinal == lifecycle_ordinal
                    && reservation.causal_lifecycle_key != command.causal_origin.lifecycle_key
            })
        {
            return Err(EnqueueError::FailClosed);
        }
        if let Some(reservation) = &self.reserved_body_available
            && let Some(owner) = reservation.lifecycle_owner()
            && owner.lifecycle_ordinal() == lifecycle_ordinal
            && owner.causal_origin() != &command.causal_origin
        {
            return Err(EnqueueError::FailClosed);
        }
        Ok(())
    }

    fn occupied_with_dormant_reservations(&self) -> Result<usize, EnqueueError> {
        let dormant = self.active_dormant_local_fifo_reservation_count()?;
        self.commands
            .len()
            .checked_add(usize::from(self.reserved_body_available.is_some()))
            .and_then(|occupied| occupied.checked_add(dormant))
            .ok_or(EnqueueError::FailClosed)
    }

    /// Count dormant FIFO owners which are not already represented by the
    /// exact unpublished body token. The aliased backing record remains in the
    /// set for retry identity, but cannot consume a second capacity slot.
    fn active_dormant_local_fifo_reservation_count(&self) -> Result<usize, EnqueueError> {
        let aliased = self
            .reserved_body_available
            .as_ref()
            .and_then(|reservation| reservation.dormant_replacement.as_ref());
        if let Some(aliased) = aliased
            && !self.dormant_local_fifo_reservations.contains(aliased)
        {
            return Err(EnqueueError::FailClosed);
        }
        self.dormant_local_fifo_reservations
            .len()
            .checked_sub(usize::from(aliased.is_some()))
            .ok_or(EnqueueError::FailClosed)
    }

    fn check_capacity_change(
        &self,
        class: CommandClass,
        dormant_replacements: usize,
        additions: usize,
    ) -> Result<(), EnqueueError> {
        let limit = match class {
            CommandClass::Normal => self.config.normal_limit(),
            CommandClass::Progress => self.config.progress_limit(),
            CommandClass::Completion => self.config.capacity,
        };
        let occupied = self.occupied_with_dormant_reservations()?;
        let occupied_after = occupied
            .checked_sub(dormant_replacements)
            .and_then(|occupied| occupied.checked_add(additions))
            .ok_or(EnqueueError::FailClosed)?;
        if occupied_after > limit {
            return Err(if occupied_after > self.config.capacity {
                EnqueueError::Full
            } else {
                EnqueueError::ReservedCapacity
            });
        }
        Ok(())
    }

    fn check_capacity(&self, class: CommandClass) -> Result<(), EnqueueError> {
        self.check_capacity_change(class, 0, 1)
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

    fn oldest_lifecycle_ordinal(&self) -> Result<Option<u128>, EnqueueError> {
        self.commands
            .iter()
            .map(|queued| {
                let ordinal = queued.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
                (queued.identity_deep_validated
                    && queued.identity.validate_exact()
                    && queued.causal_origin.validate_exact()
                    && queued.causal_origin.root_lifecycle_ordinal == Some(ordinal))
                .then_some(ordinal)
                .ok_or(EnqueueError::FailClosed)
            })
            .try_fold(None, |oldest, ordinal| {
                let ordinal = ordinal?;
                Ok(Some(
                    oldest.map_or(ordinal, |oldest: u128| oldest.min(ordinal)),
                ))
            })
    }

    fn oldest_active_lifecycle_ordinal(&self) -> Result<Option<u128>, EnqueueError> {
        let command_minimum = self.oldest_lifecycle_ordinal()?;
        self.dormant_local_fifo_reservations.iter().try_fold(
            command_minimum,
            |minimum, reservation| {
                if reservation.admission_ordinal == 0
                    || !self
                        .lifecycle_ordinals
                        .recognizes_minted(reservation.admission_ordinal)
                        .map_err(|_| EnqueueError::FailClosed)?
                {
                    return Err(EnqueueError::FailClosed);
                }
                Ok(Some(
                    minimum.map_or(reservation.admission_ordinal, |ordinal| {
                        ordinal.min(reservation.admission_ordinal)
                    }),
                ))
            },
        )
    }

    fn uses_lifecycle_ordinal(&self, lifecycle_ordinal: u128) -> Result<bool, EnqueueError> {
        if self
            .dormant_local_fifo_reservations
            .iter()
            .any(|reservation| reservation.admission_ordinal == lifecycle_ordinal)
        {
            return Ok(true);
        }
        for queued in &self.commands {
            let ordinal = queued.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
            if !queued.identity_deep_validated
                || !queued.identity.validate_exact()
                || !queued.causal_origin.validate_exact()
                || queued.causal_origin.root_lifecycle_ordinal != Some(ordinal)
            {
                return Err(EnqueueError::FailClosed);
            }
            if ordinal == lifecycle_ordinal {
                return Ok(true);
            }
        }
        if let Some(reservation) = &self.reserved_body_available {
            let owner = reservation
                .lifecycle_owner()
                .ok_or(EnqueueError::FailClosed)?;
            if owner.lifecycle_ordinal() == lifecycle_ordinal {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn class_readiness_at_lifecycle(&self, lifecycle_ordinal: u128) -> (bool, bool, bool) {
        let class_ready = |class| {
            self.commands.iter().any(|queued| {
                queued.class == class && queued.lifecycle_ordinal == Some(lifecycle_ordinal)
            })
        };
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
        let Some(oldest_lifecycle_ordinal) = self.oldest_lifecycle_ordinal()? else {
            return Ok(None);
        };
        let (completion_ready, progress_ready, normal_ready) =
            self.class_readiness_at_lifecycle(oldest_lifecycle_ordinal);
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
        let Some(checked_service) =
            check_production_body_service_effective_lock_transition(service_trace)
        else {
            panic!("Sumeragi v2 bounded service violated the effective-lock trace");
        };
        let Some(next) = CommandClass::from_service_code(selection.next) else {
            return Err(EnqueueError::FailClosed);
        };
        if selection.selected == SERVICE_CLASS_NONE {
            let _authorized_service = checked_service.into_projection();
            self.next_class = next;
            return Ok(None);
        }
        let Some(class) = CommandClass::from_service_code(selection.selected) else {
            return Err(EnqueueError::FailClosed);
        };
        let Some(index) = self.commands.iter().position(|queued| {
            queued.class == class && queued.lifecycle_ordinal == Some(oldest_lifecycle_ordinal)
        }) else {
            return Err(EnqueueError::FailClosed);
        };
        let selected = self
            .commands
            .get(index)
            .expect("selected runtime FIFO position remains present");
        let admission_ordinal = selected.admission_ordinal.ok_or(EnqueueError::FailClosed)?;
        let lifecycle_ordinal = selected.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
        let identity = selected.identity;
        let ingress_exact = match identity.kind {
            RuntimeCommandKind::Authenticated => selected.ingress_ownership.is_some(),
            _ => selected.ingress_ownership.is_none(),
        };
        if !selected.identity_deep_validated
            || !identity.validate_exact()
            || !ingress_exact
            || !selected.causal_origin.validate_exact()
            || selected.causal_origin.root_lifecycle_ordinal != Some(lifecycle_ordinal)
        {
            return Err(EnqueueError::FailClosed);
        }
        let mut candidate = RuntimeFifoCandidateOwnership {
            kind: identity.kind,
            identity,
            class: selected.class.service_code(),
            tag: selected.tag,
            admission_ordinal,
            lifecycle_ordinal,
            causal_origin: selected.causal_origin.clone(),
            ingress_projection_hash: selected
                .ingress_ownership
                .as_ref()
                .map(|ownership| ownership.projection_hash),
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
                .find(|queued| {
                    queued.class == skipped_class
                        && queued.lifecycle_ordinal == Some(oldest_lifecycle_ordinal)
                })
                .is_some_and(|oldest| oldest.eligible_skips.checked_add(1).is_none())
            {
                return Err(EnqueueError::FailClosed);
            }
        }
        let _authorized_service = checked_service.into_projection();
        self.next_class = next;
        for skipped_class in [
            CommandClass::Completion,
            CommandClass::Progress,
            CommandClass::Normal,
        ] {
            if skipped_class == class {
                continue;
            }
            if let Some(oldest) = self.commands.iter_mut().find(|queued| {
                queued.class == skipped_class
                    && queued.lifecycle_ordinal == Some(oldest_lifecycle_ordinal)
            }) {
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

    /// Restore an exact command whose reducer transition reported retryable
    /// backpressure before acquiring adapter-deferred ownership.
    ///
    /// This is not a new admission: the original physical position,
    /// admission ordinal, lifecycle ordinal, causal root, and authenticated
    /// ingress carrier must all match the scheduler's selected candidate.
    /// Only the finite class-service debt is retired by the attempted turn.
    fn restore_selected_command(
        &mut self,
        mut command: TaggedCommand<C>,
        candidate: &RuntimeFifoCandidateOwnership,
    ) -> Result<(), EnqueueError> {
        let position =
            usize::try_from(candidate.fifo_position).map_err(|_| EnqueueError::FailClosed)?;
        let ingress_projection_hash = command
            .ingress_ownership
            .as_ref()
            .map(|ownership| ownership.projection_hash);
        if !command.validate_admission_identity()
            || !candidate.identity.validate_exact()
            || candidate.projection_hash != runtime_fifo_candidate_projection_hash(candidate)
            || command.identity != candidate.identity
            || command.identity.kind != candidate.kind
            || command.class.service_code() != candidate.class
            || command.tag != candidate.tag
            || command.admission_ordinal != Some(candidate.admission_ordinal)
            || command.lifecycle_ordinal != Some(candidate.lifecycle_ordinal)
            || command.causal_origin != candidate.causal_origin
            || command.causal_origin.root_lifecycle_ordinal != Some(candidate.lifecycle_ordinal)
            || ingress_projection_hash != candidate.ingress_projection_hash
            || command.eligible_skips != candidate.eligible_skips_before
            || candidate.eligible_skips_after != 0
            || position > self.commands.len()
            || self
                .commands
                .iter()
                .any(|queued| queued.admission_ordinal == Some(candidate.admission_ordinal))
        {
            return Err(EnqueueError::FailClosed);
        }
        command.eligible_skips = 0;
        self.commands.insert(position, command);
        Ok(())
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
            self.occupied_with_dormant_reservations()
                .unwrap_or(usize::MAX),
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
/// abort its attempt without changing or releasing the process-local token.
/// Restart reconstructs the lifecycle only from the existing durable dormant
/// or effect owner; the unpublished token itself is not persisted.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "a body-available reservation must be committed or aborted"]
pub(crate) struct BodyAvailableReservation {
    tag: EventTag,
    manifest: wire::PayloadManifest,
    owns_new_slot: bool,
    admission_ordinal: Option<u128>,
    lifecycle_ordinal: Option<u128>,
    causal_origin: Option<RuntimeCandidateCausalOrigin>,
    restored_producer_stage: Option<u8>,
    /// Exact restart-dormant capacity backing aliased by this unpublished
    /// token. It remains installed until materialization so an ordinary abort
    /// cannot orphan or recreate the old producer stage.
    dormant_replacement: Option<RuntimeDormantLocalFifoReservation>,
}

impl BodyAvailableReservation {
    /// Construct a runtime-minted token which owns one unpublished completion slot.
    fn reserved_with_admission_ordinal(
        tag: EventTag,
        manifest: wire::PayloadManifest,
        admission_ordinal: u128,
    ) -> Result<Self, EnqueueError> {
        let command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let mut causal_origin =
            RuntimeCandidateCausalOrigin::mint(tag, CommandClass::Completion, &command, None);
        if !causal_origin.bind_lifecycle_ordinal(admission_ordinal) {
            return Err(EnqueueError::FailClosed);
        }
        Ok(Self {
            tag,
            manifest,
            owns_new_slot: true,
            admission_ordinal: Some(admission_ordinal),
            lifecycle_ordinal: Some(admission_ordinal),
            causal_origin: Some(causal_origin),
            restored_producer_stage: None,
            dormant_replacement: None,
        })
    }

    /// Construct an ordinal-free reservation for isolated runtime-driver tests.
    ///
    /// Production reservations are minted only by `BoundedIngress`, which
    /// assigns their actor-local admission ordinal before publishing them.
    #[cfg(test)]
    pub(crate) fn reserved(tag: EventTag, manifest: wire::PayloadManifest) -> Self {
        let command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        Self {
            tag,
            manifest,
            owns_new_slot: true,
            admission_ordinal: None,
            lifecycle_ordinal: None,
            causal_origin: Some(RuntimeCandidateCausalOrigin::mint(
                tag,
                CommandClass::Completion,
                &command,
                None,
            )),
            restored_producer_stage: None,
            dormant_replacement: None,
        }
    }

    /// Construct a token which coalesces with one exact existing owner.
    pub(crate) fn coalesced(tag: EventTag, manifest: wire::PayloadManifest) -> Self {
        Self {
            tag,
            manifest,
            owns_new_slot: false,
            admission_ordinal: None,
            lifecycle_ordinal: None,
            causal_origin: None,
            restored_producer_stage: None,
            dormant_replacement: None,
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

    fn lifecycle_owner(&self) -> Option<RuntimeLifecycleOwner> {
        RuntimeLifecycleOwner::new(self.causal_origin.clone()?, self.lifecycle_ordinal?).ok()
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

#[derive(Clone)]
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
            Self::Authenticated(authenticated) => {
                return authenticated.exact_runtime_command_identity();
            }
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
        let counts = self
            .commands
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
            });
        let Some(reservation) = self
            .reserved_body_available
            .as_ref()
            .filter(|reservation| reservation.tag == tag)
        else {
            return counts;
        };
        let BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: candidate_manifest,
        } = candidate
        else {
            return counts;
        };
        if reservation.manifest.round != candidate_manifest.round
            || reservation.manifest.subject != candidate_manifest.subject
        {
            return counts;
        }
        (
            counts.0.saturating_add(1),
            counts
                .1
                .saturating_add(usize::from(&reservation.manifest == candidate_manifest)),
        )
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

    fn authenticated_wire_merge_preflight(
        &self,
        message: &wire::ConsensusMessageV2,
        ownership: &RuntimeIngressOwnershipEvidence,
    ) -> Option<Result<(), RuntimeIngressMergeError>> {
        let mut matching_envelope = false;
        for queued in &self.commands {
            if !queued.command.matches_wire_envelope(message) {
                continue;
            }
            matching_envelope = true;
            let Some(retained) = queued.ingress_ownership.as_ref() else {
                return Some(Err(RuntimeIngressMergeError::Conflict));
            };
            let mut preview = retained.clone();
            match preview.merge_downstream(ownership.clone()) {
                Ok(()) => return Some(Ok(())),
                Err(RuntimeIngressMergeError::IndependentOccurrence) => {}
                Err(error) => return Some(Err(error)),
            }
        }
        matching_envelope.then_some(Err(RuntimeIngressMergeError::IndependentOccurrence))
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
        if let Some(preflight) = self.authenticated_wire_merge_preflight(message, ownership) {
            match preflight {
                Ok(()) => return Ok(()),
                Err(RuntimeIngressMergeError::Capacity) => return Err(EnqueueError::Full),
                // Authentication remains mandatory before a conflicting
                // process-local carrier can latch the runtime fail-closed.
                Err(RuntimeIngressMergeError::Conflict) => return Ok(()),
                Err(RuntimeIngressMergeError::IndependentOccurrence) => {}
            }
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
        self.enqueue_authenticated_with_ingress_ownership_and_owner(
            tag,
            class,
            authenticated,
            ingress_ownership,
            None,
        )
    }

    fn enqueue_authenticated_with_ingress_ownership_and_owner(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        authenticated: AuthenticatedConsensusMessage,
        ingress_ownership: RuntimeIngressOwnershipEvidence,
        restored_owner: Option<(&RuntimeLifecycleOwner, u8)>,
    ) -> Result<EventTag, EnqueueError> {
        if !ingress_ownership.matches_authenticated(&authenticated) {
            return Err(EnqueueError::FailClosed);
        }
        let mut tagged = TaggedCommand::with_ingress_ownership(
            tag,
            class,
            AdapterCommand::Authenticated(authenticated.clone()),
            Instant::now(),
            ingress_ownership.clone(),
        );
        if let Some((owner, producer_stage)) = restored_owner {
            if !owner.validate_exact()
                || tagged
                    .lifecycle_ordinal
                    .is_some_and(|ordinal| ordinal != owner.lifecycle_ordinal())
            {
                return Err(EnqueueError::FailClosed);
            }
            tagged.causal_origin = owner.causal_origin().clone();
            tagged.lifecycle_ordinal = Some(owner.lifecycle_ordinal());
            tagged.restored_producer_stage = Some(producer_stage);
        }
        self.validate_preassigned_lifecycle_owner(&tagged, &[])?;
        let matching_indices = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, queued)| {
                queued
                    .command
                    .is_same_authenticated_envelope(&authenticated)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        for index in matching_indices {
            let queued = self
                .commands
                .get_mut(index)
                .expect("located authenticated runtime owner remains present");
            let Some(retained) = queued.ingress_ownership.as_ref() else {
                return Err(EnqueueError::FailClosed);
            };
            let mut merged = retained.clone();
            match merged.merge_downstream(ingress_ownership.clone()) {
                Ok(()) => match queued.install_merged_ingress_ownership(merged) {
                    Ok(()) => return Ok(queued.tag),
                    Err(RuntimeIngressMergeError::Capacity) => {
                        return Err(EnqueueError::Full);
                    }
                    Err(RuntimeIngressMergeError::Conflict) => {
                        return Err(EnqueueError::FailClosed);
                    }
                    Err(RuntimeIngressMergeError::IndependentOccurrence) => {}
                },
                Err(RuntimeIngressMergeError::Capacity) => return Err(EnqueueError::Full),
                Err(RuntimeIngressMergeError::Conflict) => {
                    return Err(EnqueueError::FailClosed);
                }
                Err(RuntimeIngressMergeError::IndependentOccurrence) => {}
            }
        }
        self.enqueue(tagged)?;
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
        self.commit_canonical_body_available(reservation)
    }

    fn reserve_canonical_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        self.reserve_canonical_body_available_internal(tag, manifest, None, None)
    }

    fn reserve_canonical_body_available_with_owner(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        owner: &RuntimeLifecycleOwner,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        self.reserve_canonical_body_available_internal(tag, manifest, Some(owner), None)
    }

    fn reserve_canonical_body_available_internal(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        owner: Option<&RuntimeLifecycleOwner>,
        restored_producer_stage: Option<u8>,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        if restored_producer_stage.is_some() && owner.is_none() {
            return Err(EnqueueError::FailClosed);
        }
        let body_command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let mut prospective = if let Some(owner) = owner {
            if !owner.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            TaggedCommand::with_causal_origin(
                tag,
                CommandClass::Completion,
                body_command,
                Instant::now(),
                owner.causal_origin().clone(),
                owner.lifecycle_ordinal(),
            )?
        } else {
            TaggedCommand::new(tag, CommandClass::Completion, body_command, Instant::now())
        };
        prospective.restored_producer_stage = restored_producer_stage;
        if owner.is_some() {
            self.validate_preassigned_lifecycle_owner(&prospective, &[])?;
        }
        let dormant_replacement = self.dormant_local_fifo_replacement_inner(&prospective, true)?;
        if restored_producer_stage
            .is_some_and(RuntimeDormantLocalFifoReservation::is_local_fifo_stage)
            && dormant_replacement.is_none()
        {
            return Err(EnqueueError::FailClosed);
        }

        if let Some(existing) = &self.reserved_body_available {
            let physical_admission_ordinal =
                existing.admission_ordinal.ok_or(EnqueueError::FailClosed)?;
            let expected_lifecycle_ordinal = prospective.lifecycle_ordinal;
            let expected_causal_origin = if expected_lifecycle_ordinal.is_some() {
                Some(prospective.causal_origin.clone())
            } else {
                let retained_origin = existing
                    .causal_origin
                    .as_ref()
                    .ok_or(EnqueueError::FailClosed)?;
                if tag != retained_origin.root_tag
                    && !tag.strictly_advances(retained_origin.root_tag)
                {
                    return Err(EnqueueError::DuplicateCompletionOwnership);
                }
                let mut causal_origin = RuntimeCandidateCausalOrigin::mint(
                    retained_origin.root_tag,
                    CommandClass::Completion,
                    &prospective.command,
                    None,
                );
                if !causal_origin.bind_lifecycle_ordinal(physical_admission_ordinal) {
                    return Err(EnqueueError::FailClosed);
                }
                Some(causal_origin)
            };
            let expected_lifecycle_ordinal =
                expected_lifecycle_ordinal.or(Some(physical_admission_ordinal));
            let exact_retry = existing.tag == tag
                && existing.manifest == manifest
                && existing.owns_new_slot
                && existing.lifecycle_ordinal == expected_lifecycle_ordinal
                && existing.causal_origin == expected_causal_origin
                && existing.restored_producer_stage == restored_producer_stage
                && existing.dormant_replacement == dormant_replacement
                && existing
                    .lifecycle_owner()
                    .is_some_and(|retained| retained.validate_exact())
                && self
                    .lifecycle_ordinals
                    .recognizes_minted(physical_admission_ordinal)
                    .map_err(|_| EnqueueError::FailClosed)?
                && self
                    .lifecycle_ordinals
                    .recognizes_minted(expected_lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?)
                    .map_err(|_| EnqueueError::FailClosed)?
                && dormant_replacement.as_ref().is_none_or(|replacement| {
                    self.dormant_local_fifo_reservations.contains(replacement)
                });
            if exact_retry {
                return Ok(existing.clone());
            }
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
        let retained_commands = self.commands.len().saturating_sub(conflicting);
        let dormant_reservations_before = self.active_dormant_local_fifo_reservation_count()?;
        let dormant_reservations_after = dormant_reservations_before
            .checked_sub(usize::from(dormant_replacement.is_some()))
            .ok_or(EnqueueError::FailClosed)?;
        let occupied_after_commit = retained_commands
            .checked_add(dormant_reservations_after)
            .and_then(|occupied| occupied.checked_add(1))
            .ok_or(EnqueueError::FailClosed)?;
        if occupied_after_commit > self.config.capacity {
            return Err(EnqueueError::Full);
        }
        let queue_len_before =
            u64::try_from(retained_commands).map_err(|_| EnqueueError::FailClosed)?;
        let queue_len_after = queue_len_before
            .checked_add(1)
            .ok_or(EnqueueError::FailClosed)?;
        let queue_capacity =
            u64::try_from(self.config.capacity).map_err(|_| EnqueueError::FailClosed)?;
        let dormant_reservations_before =
            u64::try_from(dormant_reservations_before).map_err(|_| EnqueueError::FailClosed)?;
        let dormant_reservations_after =
            u64::try_from(dormant_reservations_after).map_err(|_| EnqueueError::FailClosed)?;
        self.with_checked_admission_ordinal_range(
            1,
            move |ingress, admission_ordinal, ordinal_successor| {
                if prospective
                    .lifecycle_ordinal
                    .is_some_and(|ordinal| ordinal >= admission_ordinal)
                {
                    return Err(EnqueueError::FailClosed);
                }
                let mut reservation = BodyAvailableReservation::reserved_with_admission_ordinal(
                    tag,
                    manifest,
                    admission_ordinal,
                )?;
                if let Some(lifecycle_ordinal) = prospective.lifecycle_ordinal {
                    reservation.lifecycle_ordinal = Some(lifecycle_ordinal);
                    reservation.causal_origin = Some(prospective.causal_origin.clone());
                }
                reservation.restored_producer_stage = restored_producer_stage;
                reservation.dormant_replacement = dormant_replacement;
                let lifecycle_ordinal = reservation
                    .lifecycle_ordinal
                    .ok_or(EnqueueError::FailClosed)?;
                if reservation
                    .dormant_replacement
                    .as_ref()
                    .is_some_and(|replacement| {
                        !ingress
                            .dormant_local_fifo_reservations
                            .contains(replacement)
                    })
                {
                    return Err(EnqueueError::FailClosed);
                }
                let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {
                    incoming_height: tag.height(),
                    incoming_view: tag.view(),
                    incoming_generation: tag.generation().get(),
                    incoming_class: CommandClass::Completion.service_code(),
                    stored_height: tag.height(),
                    stored_view: tag.view(),
                    stored_generation: tag.generation().get(),
                    stored_class: CommandClass::Completion.service_code(),
                    queue_len_before,
                    queue_len_after,
                    queue_capacity,
                    ordinal_source_before: admission_ordinal,
                    physical_admission_ordinal: admission_ordinal,
                    lifecycle_ordinal,
                    ordinal_source_after: ordinal_successor,
                    dormant_reservations_before,
                    dormant_reservations_after,
                    dormant_owner_ordinal: reservation
                        .dormant_replacement
                        .as_ref()
                        .map_or(0, |replacement| replacement.admission_ordinal),
                    ordinal_minted: true,
                };
                let checked_transition = check_production_ingress_transition(ingress_trace)
                    .ok_or(EnqueueError::FailClosed)?;
                let _authorized_transition = checked_transition.into_projection();

                // Infallible reservation commit while the source remains
                // locked; a rejected owner or gate cannot burn an ordinal.
                ingress.reserved_body_available = Some(reservation.clone());
                Ok(reservation)
            },
        )
    }

    fn commit_canonical_body_available(
        &mut self,
        reservation: BodyAvailableReservation,
    ) -> Result<(), EnqueueError> {
        if !reservation.owns_new_slot() {
            return Ok(());
        }
        if self.reserved_body_available.as_ref() != Some(&reservation) {
            return Err(EnqueueError::FailClosed);
        }
        let mut command = TaggedCommand::new(
            reservation.tag(),
            CommandClass::Completion,
            AdapterCommand::BodyAvailable {
                manifest: reservation.manifest.clone(),
            },
            Instant::now(),
        );
        command.admission_ordinal = reservation.admission_ordinal;
        command.lifecycle_ordinal = reservation.lifecycle_ordinal;
        command.restored_producer_stage = reservation.restored_producer_stage;
        command.causal_origin = reservation
            .causal_origin
            .clone()
            .ok_or(EnqueueError::FailClosed)?;
        if !command.validate_admission_identity() {
            return Err(EnqueueError::FailClosed);
        }
        self.validate_preassigned_lifecycle_owner(&command, &[])?;
        let dormant_replacement = self.dormant_local_fifo_replacement_inner(&command, true)?;
        if dormant_replacement != reservation.dormant_replacement {
            return Err(EnqueueError::FailClosed);
        }
        let lifecycle_ordinals = self.lifecycle_ordinals.clone();
        lifecycle_ordinals.with_checked_current(move |source_current| {
            if self.reserved_body_available.as_ref() != Some(&reservation)
                || dormant_replacement.as_ref().is_some_and(|replacement| {
                    !self.dormant_local_fifo_reservations.contains(replacement)
                })
            {
                return Err(EnqueueError::FailClosed);
            }
            let physical_admission_ordinal = reservation
                .admission_ordinal
                .ok_or(EnqueueError::FailClosed)?;
            let lifecycle_ordinal = reservation
                .lifecycle_ordinal
                .ok_or(EnqueueError::FailClosed)?;
            let incoming_tag = command.tag;
            let incoming_class = command.class.service_code();
            let retained_len = self
                .commands
                .iter()
                .filter(|queued| {
                    !queued
                        .command
                        .is_authenticated_proposal_conflicting_with(reservation.manifest())
                })
                .count();
            let queue_len_before =
                u64::try_from(retained_len).map_err(|_| EnqueueError::FailClosed)?;
            let queue_len_after = queue_len_before
                .checked_add(1)
                .ok_or(EnqueueError::FailClosed)?;
            let dormant_reservations_before =
                u64::try_from(self.dormant_local_fifo_reservations.len())
                    .map_err(|_| EnqueueError::FailClosed)?;
            let dormant_reservations_after = dormant_reservations_before
                .checked_sub(u64::from(dormant_replacement.is_some()))
                .ok_or(EnqueueError::FailClosed)?;
            let ingress_trace = ProductionIngressReservationMaterializationTraceProjection {
                incoming_height: incoming_tag.height(),
                incoming_view: incoming_tag.view(),
                incoming_generation: incoming_tag.generation().get(),
                incoming_class,
                stored_height: command.tag.height(),
                stored_view: command.tag.view(),
                stored_generation: command.tag.generation().get(),
                stored_class: command.class.service_code(),
                queue_len_before,
                queue_len_after,
                reserved_slots_before: 1,
                reserved_slots_after: 0,
                queue_capacity: u64::try_from(self.config.capacity)
                    .expect("bounded runtime ingress capacity is representable as u64"),
                ordinal_source_before: source_current,
                physical_admission_ordinal,
                lifecycle_ordinal,
                ordinal_source_after: source_current,
                dormant_reservations_before,
                dormant_reservations_after,
                dormant_owner_ordinal: dormant_replacement
                    .as_ref()
                    .map_or(0, |replacement| replacement.admission_ordinal),
            };
            let checked_transition =
                check_production_ingress_reservation_materialization_transition(ingress_trace)
                    .ok_or(EnqueueError::FailClosed)?;
            let _authorized_transition = checked_transition.into_projection();

            // Materialization does not mint another owner; keep the source
            // locked through the infallible reserved-slot replacement.
            if let Some(replacement) = dormant_replacement.as_ref() {
                let removed = self.dormant_local_fifo_reservations.remove(replacement);
                debug_assert!(removed);
            }
            self.reserved_body_available = None;
            self.discard_proposals_conflicting_with(reservation.manifest());
            self.commands.push_back(command);
            Ok(())
        })
    }

    fn abort_canonical_body_available(&mut self, reservation: BodyAvailableReservation) {
        // Rejection by a later service boundary is retryable ownership, not a
        // terminal event. Retain the entire token (including its ordinal and
        // dormant backing) so the exact retry reclaims it without reminting.
        // Abort carries no retirement authority: a stale or mismatched token
        // is therefore the same intentional no-op and cannot clear the exact
        // retained owner.
        let _retained_exact_owner = !reservation.owns_new_slot()
            || self.reserved_body_available.as_ref() == Some(&reservation);
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
        if let Some(reservation) = &mut self.reserved_body_available
            && reservation.tag == previous
            && reservation.manifest == *manifest
        {
            reservation.tag = rebound;
            rebound_count = rebound_count.saturating_add(1);
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
        let mut retired = before.saturating_sub(self.commands.len());
        if self
            .reserved_body_available
            .as_ref()
            .is_some_and(|reservation| reservation.tag == tag && reservation.manifest == *manifest)
        {
            let reservation = self
                .reserved_body_available
                .take()
                .expect("matched body reservation remains present");
            if let Some(replacement) = reservation.dormant_replacement {
                let removed = self.dormant_local_fifo_reservations.remove(&replacement);
                debug_assert!(removed);
            }
            retired = retired.saturating_add(1);
        }
        retired
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
        if self
            .reserved_body_available
            .as_ref()
            .is_some_and(|reservation| {
                reservation.tag == tag
                    && reservation.manifest.round == round
                    && reservation.manifest.subject == subject
            })
        {
            let reservation = self
                .reserved_body_available
                .take()
                .expect("matched body reservation remains present");
            if let Some(replacement) = reservation.dormant_replacement {
                let removed = self.dormant_local_fifo_reservations.remove(&replacement);
                debug_assert!(removed);
            }
            retired.record_body_available();
        }
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
        if self
            .reserved_body_available
            .as_ref()
            .is_some_and(|reservation| {
                reservation.tag == tag
                    && reservation.manifest.round == round
                    && reservation.manifest.subject == subject
            })
        {
            counts.record_body_available();
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
        self.reserved_body_available
            .as_ref()
            .is_some_and(|reservation| {
                manifests_conflict_for_same_body(&proposal.manifest, reservation.manifest())
            })
            || self.commands.iter().any(|queued| {
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
    deferred_ordinal: Option<u128>,
    retry_unadmitted: bool,
    producer_handoff: Option<ProducerContinuationHandoffToken>,
}

/// Read-only admission decision made before a command can consume a runtime
/// ordinal or physical FIFO slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeCommandAdmissionPreflight {
    /// The exact command still has a live reducer stage to consume.
    Admit,
    /// A restart-restored exact internal-completion producer stage must reuse
    /// its immutable lifecycle key, first-admission ordinal, and completion
    /// service class. Authenticated ingress retains its separate leader-wire
    /// lifecycle gate and cannot enter this branch.
    ReuseDormant {
        /// Persisted causal lifecycle key for the exact retry.
        causal_lifecycle_key: iroha_crypto::Hash,
        /// Persisted immutable first-admission ordinal.
        admission_ordinal: u128,
        /// Closed reducer service stage at the persisted bounded address.
        producer_stage: u8,
    },
    /// A phase-specific monotone reducer fact or exact durable terminal record
    /// already suppresses this lifecycle occurrence.
    Coalesce,
    /// The internal command is malformed or conflicts with frozen authority.
    Reject,
}

/// Read-only lookup result for a deterministic fresh root reconstructed after
/// restart. Multiple exact stage records may share one lifecycle, but they
/// must all retain the same immutable ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeDormantProducerLifecycle {
    /// No dormant record owns this causal key.
    Absent,
    /// The exact persisted lifecycle owns this immutable ordinal.
    Exact { admission_ordinal: u128 },
    /// Dormant metadata disagreed about status, durability, or ordinal.
    Conflict,
}

/// Restart-dormant local producer stage which already owns one latent FIFO slot.
///
/// The adjacent producer-continuation snapshot carries only internal admission
/// metadata, never a command payload or wire field.  The runtime installs this
/// projection before admitting any live work, charges it against the existing
/// class-aware queue allocation, and removes it only when the exact restored
/// lifecycle/stage becomes a physical FIFO command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RuntimeDormantLocalFifoReservation {
    causal_lifecycle_key: iroha_crypto::Hash,
    admission_ordinal: u128,
    producer_stage: u8,
    class: CommandClass,
}

impl RuntimeDormantLocalFifoReservation {
    const TIMEOUT_ELAPSED_STAGE: u8 = 6;

    const fn is_known_stage(producer_stage: u8) -> bool {
        producer_stage <= 10
    }

    const fn is_local_fifo_stage(producer_stage: u8) -> bool {
        matches!(producer_stage, 0 | 8 | 9 | 10)
    }

    /// Bind one locally replayable producer stage to the trusted completion lane.
    pub(crate) const fn completion(
        causal_lifecycle_key: iroha_crypto::Hash,
        admission_ordinal: u128,
        producer_stage: u8,
    ) -> Self {
        Self {
            causal_lifecycle_key,
            admission_ordinal,
            producer_stage,
            class: CommandClass::Completion,
        }
    }
}

impl<E> RuntimeDriverDispatch<E> {
    fn completed(effects: Vec<E>) -> Self {
        Self {
            effects,
            deferred_ingress: None,
            deferred_ordinal: None,
            retry_unadmitted: false,
            producer_handoff: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeEffectSource {
    Startup,
    Fifo,
    Deferred,
    Timeout,
    Retransmit,
}

pub(crate) trait RuntimeDriver {
    /// Command payload consumed by the driver.
    type Command: ExactRuntimeCommandIdentity + Clone;
    /// Effect emitted unchanged to asynchronous adapters.
    type Effect;
    /// Fatal transition error.
    type Error: fmt::Display;

    /// Current authoritative reducer tag.
    fn current_tag(&self) -> EventTag;
    /// Classify an exact command without mutating reducer, registry, queue, or
    /// ordinal state. Authenticated wire ingress is always admitted here and
    /// remains governed by its dedicated authentication/equivocation seam.
    fn preflight_command_admission(
        &self,
        _tag: EventTag,
        _command: &Self::Command,
    ) -> RuntimeCommandAdmissionPreflight {
        RuntimeCommandAdmissionPreflight::Admit
    }
    /// Look up a restart-dormant deterministic root by its recomputed causal
    /// lifecycle key without mutating adapter or scheduler state.
    fn dormant_producer_lifecycle(
        &self,
        _causal_lifecycle_key: &iroha_crypto::Hash,
    ) -> RuntimeDormantProducerLifecycle {
        RuntimeDormantProducerLifecycle::Absent
    }
    /// Enumerate every restart-dormant Local stage whose deterministic replay
    /// will enter the serialized FIFO. Non-FIFO timeout roots and
    /// transport-conditional work are deliberately absent.
    fn dormant_local_fifo_reservations(
        &self,
    ) -> Result<Vec<RuntimeDormantLocalFifoReservation>, String> {
        Ok(Vec::new())
    }
    /// Deliver one admitted command with its original tag.
    fn dispatch(
        &mut self,
        command: TaggedCommand<Self::Command>,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error>;
    /// Bind one scheduler-validated lifecycle to a timer transition whose
    /// compact driver method otherwise carries only the reducer tag.
    fn bind_selected_producer_lifecycle(
        &mut self,
        _owner: &RuntimeLifecycleOwner,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Clear a lifecycle binding after the driver transition returns.
    fn clear_selected_producer_lifecycle(&mut self) {}
    /// Classify the exact producer replacement already retained by this
    /// dispatch. Production must distinguish durable, concrete-successor, and
    /// process-local volatile terminals; effect-count inference alone is not
    /// sufficient.
    fn producer_handoff_evidence(
        &self,
        _token: ProducerContinuationHandoffToken,
        _has_concrete_successor: bool,
    ) -> Result<ProducerContinuationHandoffEvidence, Self::Error> {
        unreachable!("a synthetic driver cannot classify producer handoff tokens")
    }
    /// Acknowledge an exact producer only after the runtime installed its
    /// concrete successor sidecar or retained exact durable terminal evidence.
    fn acknowledge_producer_handoff(
        &mut self,
        _token: ProducerContinuationHandoffToken,
        _evidence: ProducerContinuationHandoffEvidence,
    ) -> Result<ProducerContinuationTerminalToken, Self::Error> {
        unreachable!("a synthetic driver cannot mint producer handoff tokens")
    }
    /// Deliver the absolute round-timeout event.
    fn timeout_elapsed(
        &mut self,
        tag: EventTag,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error>;
    /// Deliver one derived retransmission tick.
    fn retransmit_elapsed(
        &mut self,
        tag: EventTag,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error>;
    /// Return whether older adapter-owned Busy-deferred work can cross the
    /// reducer boundary without spinning behind a persistence/signing fence.
    fn deferred_work_is_serviceable(&self) -> bool;
    /// Actor-global source which minted deferred ownership capabilities.
    fn deferred_admission_ordinal_source(&self) -> &DeferredAdmissionOrdinalSource;
    /// Actor-global ordinals of every authenticated occurrence still retained
    /// by the adapter's Busy-deferred queues.
    fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128>;
    /// Actor-global ordinals of every occurrence retained by any Busy lane.
    fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128>;
    /// Test-driver seam for deferred owners created outside production
    /// ingress. Production adapters must return `None` and use the runtime
    /// handoff map populated by `dispatch`.
    #[cfg(test)]
    fn synthetic_deferred_lifecycle_owner(
        &self,
        _evidence: &DeferredServiceEvidence,
    ) -> Option<RuntimeLifecycleOwner> {
        None
    }
    /// Deliver exactly one serviceable adapter-owned deferred transition and
    /// its exact selected-occurrence token. `eligible` is the non-empty set of
    /// adapter admission ordinals whose immutable lifecycle ordinal equals the
    /// global active minimum.
    fn dispatch_deferred(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<
        Option<(
            Vec<Self::Effect>,
            DeferredServiceEvidence,
            Option<ProducerContinuationHandoffToken>,
        )>,
        Self::Error,
    >;
    /// Identify only the effect which authorizes timer restart.
    fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag>;
    /// Classify the exceptional effects which are new TLA roots rather than
    /// causal children of the selected scheduler owner.
    fn effect_causality(
        _effect: &Self::Effect,
        _source: RuntimeEffectSource,
    ) -> RuntimeEffectCausality {
        RuntimeEffectCausality::Inherit
    }
    /// Route-neutral semantic identity for a new TLA effect root. Diagnostic
    /// generation and local admission ordinals must not appear here.
    fn fresh_effect_semantic_identity(
        _effect: &Self::Effect,
        kind: RuntimeFreshRootKind,
    ) -> Vec<u8> {
        vec![kind.code()]
    }
    /// Reducer tag carried by an effect which may become a fresh root.
    fn effect_root_tag(_effect: &Self::Effect) -> Option<EventTag> {
        None
    }
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

    fn preflight_command_admission(
        &self,
        tag: EventTag,
        command: &Self::Command,
    ) -> RuntimeCommandAdmissionPreflight {
        self.preflight_runtime_command_admission(tag, command)
    }

    fn dormant_producer_lifecycle(
        &self,
        causal_lifecycle_key: &iroha_crypto::Hash,
    ) -> RuntimeDormantProducerLifecycle {
        SumeragiV2Adapter::dormant_producer_lifecycle(self, causal_lifecycle_key)
    }

    fn dormant_local_fifo_reservations(
        &self,
    ) -> Result<Vec<RuntimeDormantLocalFifoReservation>, String> {
        SumeragiV2Adapter::dormant_local_fifo_reservations(self)
    }

    fn dispatch(
        &mut self,
        tagged: TaggedCommand<Self::Command>,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
        let lifecycle_ordinal = tagged
            .lifecycle_ordinal
            .ok_or(AdapterError::RuntimeIngressOwnershipViolation)?;
        if !tagged.causal_origin.validate_exact()
            || tagged.causal_origin.root_lifecycle_ordinal != Some(lifecycle_ordinal)
        {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        let authenticated = matches!(&tagged.command, AdapterCommand::Authenticated(_));
        if authenticated != tagged.ingress_ownership.is_some() {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        self.bind_selected_producer_lifecycle(
            tagged.causal_origin.lifecycle_key.clone(),
            lifecycle_ordinal,
        )?;
        let tag = tagged.tag;
        let ingress_ownership = tagged.ingress_ownership;
        let outcome = (|| {
            match tagged.command {
                AdapterCommand::Authenticated(message) => {
                    let ownership = ingress_ownership
                        .as_ref()
                        .ok_or(AdapterError::RuntimeIngressOwnershipViolation)?;
                    if !ownership.matches_authenticated(&message) {
                        return Err(AdapterError::RuntimeIngressOwnershipViolation);
                    }
                    // Authenticated network ingress is deliberately retagged by the
                    // adapter if it waited behind a certified view transition.
                    // Asynchronous completion variants below retain `tag` exactly.
                    self.receive_authenticated(message)
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
            }
        })();
        self.clear_selected_producer_lifecycle();
        let outcome = outcome?;
        let deferred_ordinal = outcome.deferred_admission_ordinal();
        let retry_unadmitted = outcome.requires_runtime_retry();
        let producer_handoff = outcome.producer_handoff();
        let deferred_ingress = match (deferred_ordinal, ingress_ownership) {
            (Some(ordinal), Some(ownership)) => Some((ordinal, ownership)),
            (Some(_), None) | (None, None) => None,
            (None, Some(_)) => None,
        };
        Ok(RuntimeDriverDispatch {
            effects: outcome.into_effects(),
            deferred_ingress,
            deferred_ordinal,
            retry_unadmitted,
            producer_handoff,
        })
    }

    fn bind_selected_producer_lifecycle(
        &mut self,
        owner: &RuntimeLifecycleOwner,
    ) -> Result<(), Self::Error> {
        if !owner.validate_exact() {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        SumeragiV2Adapter::bind_selected_producer_lifecycle(
            self,
            owner.causal_origin().lifecycle_key.clone(),
            owner.lifecycle_ordinal(),
        )
    }

    fn clear_selected_producer_lifecycle(&mut self) {
        SumeragiV2Adapter::clear_selected_producer_lifecycle(self);
    }

    fn producer_handoff_evidence(
        &self,
        token: ProducerContinuationHandoffToken,
        has_concrete_successor: bool,
    ) -> Result<ProducerContinuationHandoffEvidence, Self::Error> {
        SumeragiV2Adapter::producer_handoff_evidence(self, token, has_concrete_successor)
    }

    fn acknowledge_producer_handoff(
        &mut self,
        token: ProducerContinuationHandoffToken,
        evidence: ProducerContinuationHandoffEvidence,
    ) -> Result<ProducerContinuationTerminalToken, Self::Error> {
        SumeragiV2Adapter::acknowledge_producer_handoff(self, token, evidence)
    }

    fn timeout_elapsed(
        &mut self,
        tag: EventTag,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
        SumeragiV2Adapter::timeout_elapsed(self, tag).map(|outcome| {
            let deferred_ordinal = outcome.deferred_admission_ordinal();
            let retry_unadmitted = outcome.requires_runtime_retry();
            let producer_handoff = outcome.producer_handoff();
            RuntimeDriverDispatch {
                effects: outcome.into_effects(),
                deferred_ingress: None,
                deferred_ordinal,
                retry_unadmitted,
                producer_handoff,
            }
        })
    }

    fn retransmit_elapsed(
        &mut self,
        tag: EventTag,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
        SumeragiV2Adapter::retransmit_elapsed(self, tag).map(|outcome| {
            let deferred_ordinal = outcome.deferred_admission_ordinal();
            let retry_unadmitted = outcome.requires_runtime_retry();
            let producer_handoff = outcome.producer_handoff();
            RuntimeDriverDispatch {
                effects: outcome.into_effects(),
                deferred_ingress: None,
                deferred_ordinal,
                retry_unadmitted,
                producer_handoff,
            }
        })
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

    fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
        SumeragiV2Adapter::all_deferred_admission_ordinals(self)
    }

    fn dispatch_deferred(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<
        Option<(
            Vec<Self::Effect>,
            DeferredServiceEvidence,
            Option<ProducerContinuationHandoffToken>,
        )>,
        Self::Error,
    > {
        SumeragiV2Adapter::drain_deferred_with_handoff_for_ordinals(self, eligible)
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

    fn effect_causality(
        effect: &Self::Effect,
        source: RuntimeEffectSource,
    ) -> RuntimeEffectCausality {
        if source == RuntimeEffectSource::Retransmit
            && matches!(effect, AdapterEffect::FetchBody { .. })
        {
            // The periodic durable-lock recovery fetch is the TLA
            // HistoricalLockedRetransmitCandidate root. Retrying or rebinding
            // the resulting task must preserve this classification.
            RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::HistoricalLockedRetransmit)
        } else {
            RuntimeEffectCausality::Inherit
        }
    }

    fn fresh_effect_semantic_identity(
        effect: &Self::Effect,
        kind: RuntimeFreshRootKind,
    ) -> Vec<u8> {
        let mut identity = vec![kind.code()];
        match effect {
            AdapterEffect::Sign { request, .. } => {
                identity.push(1);
                append_runtime_identity_field(&mut identity, &request.signature_preimage());
            }
            AdapterEffect::Broadcast(message) => {
                identity.push(2);
                append_runtime_identity_field(&mut identity, &message.encode());
            }
            AdapterEffect::FetchBody {
                round,
                subject,
                manifest,
                certificate,
                ..
            } => {
                identity.push(3);
                append_runtime_identity_field(&mut identity, &round.encode());
                append_runtime_identity_field(&mut identity, &subject.encode());
                if let Some(manifest) = manifest {
                    append_runtime_identity_field(&mut identity, &manifest.encode());
                }
                if let Some(certificate) = certificate {
                    // Exclude mutable transport sources. The certified semantic
                    // authority is its round/phase/subject/commitment tuple;
                    // aggregate carrier bytes do not create a new root.
                    append_runtime_identity_field(&mut identity, &certificate.round.encode());
                    append_runtime_identity_field(
                        &mut identity,
                        &certificate.proposal_round.encode(),
                    );
                    append_runtime_identity_field(&mut identity, &certificate.subject.encode());
                    identity.push(certificate.phase as u8);
                    append_runtime_identity_field(
                        &mut identity,
                        &certificate.execution_commitment.encode(),
                    );
                }
            }
            AdapterEffect::StoreBody { round, subject, .. }
            | AdapterEffect::ValidateBody { round, subject, .. } => {
                identity.push(4);
                append_runtime_identity_field(&mut identity, &round.encode());
                append_runtime_identity_field(&mut identity, &subject.encode());
            }
            AdapterEffect::Apply {
                subject,
                certificate,
                ..
            } => {
                identity.push(5);
                append_runtime_identity_field(&mut identity, &subject.encode());
                append_runtime_identity_field(&mut identity, &certificate.round.encode());
            }
            AdapterEffect::EnterView {
                certificate,
                protected_body,
                ..
            } => {
                identity.push(6);
                append_runtime_identity_field(&mut identity, &certificate.round.encode());
                if let Some((round, subject)) = protected_body {
                    append_runtime_identity_field(&mut identity, &round.encode());
                    append_runtime_identity_field(&mut identity, &subject.encode());
                }
            }
            AdapterEffect::ReportEquivocation {
                offender,
                round,
                kind: equivocation_kind,
            } => {
                identity.push(7);
                append_runtime_identity_field(&mut identity, &offender.encode());
                append_runtime_identity_field(&mut identity, &round.encode());
                identity.push(match equivocation_kind {
                    super::v2_core::EquivocationKind::Vote => 1,
                    super::v2_core::EquivocationKind::Timeout => 2,
                    super::v2_core::EquivocationKind::Proposal => 3,
                });
            }
            AdapterEffect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => {
                identity.push(8);
                append_runtime_identity_field(&mut identity, &subject.encode());
                append_runtime_identity_field(&mut identity, &certificate.round.encode());
            }
        }
        identity
    }

    fn effect_root_tag(effect: &Self::Effect) -> Option<EventTag> {
        match effect {
            AdapterEffect::Sign { tag, .. }
            | AdapterEffect::FetchBody { tag, .. }
            | AdapterEffect::StoreBody { tag, .. }
            | AdapterEffect::ValidateBody { tag, .. }
            | AdapterEffect::Apply { tag, .. }
            | AdapterEffect::EnterView { tag, .. } => Some(*tag),
            AdapterEffect::Broadcast(_)
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
    /// No timer, deferred owner, or FIFO lifecycle was globally eligible.
    Idle,
    /// One timer or command was delivered; effects remain in adapter order.
    Advanced(Vec<E>),
}

/// Exact last-consumer outcome for one generic productive-wire lifecycle.
///
/// The serialized runtime emits this only after it has either retained every
/// concrete successor sidecar or installed the adapter's typed durable
/// producer terminal. The effect executor then commits the corresponding
/// generic gate transition before another outer ingress turn can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LeaderWireRuntimeTerminal {
    /// The consumer is complete for this process generation but must reopen on
    /// crash because no independent durable terminal exists.
    Volatile(LeaderWireLifecycleRuntimeReceipt),
    /// The adapter published an exact restart-stable producer terminal.
    Producer {
        /// Durable ingress-to-runtime ownership being retired.
        runtime: LeaderWireLifecycleRuntimeReceipt,
        /// Independently persisted producer continuation evidence.
        terminal: ProducerContinuationTerminalToken,
    },
}

/// Scheduler-visible ownership of the local proposal producer for one active
/// view. The first live view mints this owner before clocks are armed; a later
/// `EnterView` inherits the exact positional owner of the persisted install
/// transition. It is not a queued command, so the runner must either hand it
/// to exact local Store admission or release it for a non-leader.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveViewProducerReservation {
    tag: EventTag,
    ownership: RuntimeEffectOwnership,
}

/// One-owner, class-aware scheduling shell for Sumeragi v2.
pub(crate) struct SerializedV2Runtime<D: RuntimeDriver = SumeragiV2Adapter> {
    driver: D,
    ingress: BoundedIngress<D::Command>,
    deferred_ingress_ownership: BTreeMap<u128, RuntimeIngressOwnershipEvidence>,
    deferred_lifecycle_ownership: BTreeMap<u128, RuntimeLifecycleOwner>,
    leader_wire_runtime_receipts: BTreeMap<u128, LeaderWireLifecycleRuntimeReceipt>,
    pending_leader_wire_terminals: VecDeque<LeaderWireRuntimeTerminal>,
    active_view_producer: Option<ActiveViewProducerReservation>,
    base_round_timeout: Duration,
    retransmit_interval: Duration,
    round_started_at: Instant,
    retransmit_started_at: Instant,
    round_tag: EventTag,
    clocks_armed: bool,
    timeout_emitted: bool,
    timeout_owner: Option<RuntimeLifecycleOwner>,
    retransmit_owner: Option<RuntimeLifecycleOwner>,
    dormant_fresh_lifecycle_owners:
        BTreeMap<(RuntimeFreshRootKind, iroha_crypto::Hash), RuntimeLifecycleOwner>,
    pending_effect_ownership: Option<Vec<RuntimeEffectOwnership>>,
    external_lifecycle_owners: Vec<RuntimeLifecycleOwner>,
    external_lifecycle_owner_capacity: usize,
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
        Self::with_driver_and_lifecycle_ordinals(
            driver,
            started_at,
            round_timeout,
            queue_config,
            startup_effects,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        )
    }

    fn with_driver_and_lifecycle_ordinals(
        driver: D,
        started_at: Instant,
        round_timeout: Duration,
        queue_config: RuntimeQueueConfig,
        startup_effects: Vec<D::Effect>,
        lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    ) -> Result<(Self, Vec<D::Effect>), RuntimeConfigError> {
        let retransmit_interval = round_timeout
            .checked_div(RETRANSMIT_DIVISOR)
            .filter(|interval| !interval.is_zero())
            .ok_or(RuntimeConfigError::InvalidRoundTimeout)?;
        let queue_config = queue_config.validate()?;
        let round_tag = driver.current_tag();
        let dormant_local_fifo_reservations = driver
            .dormant_local_fifo_reservations()
            .map_err(|_| RuntimeConfigError::InvalidLifecycleOwnership)?;
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(queue_config, lifecycle_ordinals);
        ingress
            .install_dormant_local_fifo_reservations(dormant_local_fifo_reservations)
            .map_err(|_| RuntimeConfigError::InvalidLifecycleOwnership)?;
        let mut runtime = Self {
            driver,
            ingress,
            deferred_ingress_ownership: BTreeMap::new(),
            deferred_lifecycle_ownership: BTreeMap::new(),
            leader_wire_runtime_receipts: BTreeMap::new(),
            pending_leader_wire_terminals: VecDeque::new(),
            active_view_producer: None,
            base_round_timeout: round_timeout,
            retransmit_interval,
            round_started_at: started_at,
            retransmit_started_at: started_at,
            round_tag,
            clocks_armed: false,
            timeout_emitted: false,
            timeout_owner: None,
            retransmit_owner: None,
            dormant_fresh_lifecycle_owners: BTreeMap::new(),
            pending_effect_ownership: None,
            external_lifecycle_owners: Vec::new(),
            // Before the effect executor installs its configured pending-work
            // bound, only the one bounded startup batch can exist externally.
            external_lifecycle_owner_capacity: MAX_EFFECTS_PER_STEP,
            schedule: ScheduleState::default(),
            last_scheduler_ownership: None,
            fail_closed: false,
            fail_closed_reason: None,
        };
        runtime
            .retain_effect_ownership(
                RuntimeEffectSource::Startup,
                None,
                startup_effects.as_slice(),
            )
            .map_err(|_| RuntimeConfigError::InvalidLifecycleOwnership)?;
        runtime
            .observe_effects(started_at, &startup_effects)
            .map_err(|_| RuntimeConfigError::InvalidLifecycleOwnership)?;
        Ok((runtime, startup_effects))
    }

    fn mint_fresh_lifecycle_owner(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        kind: RuntimeFreshRootKind,
        semantic_identity: &[u8],
    ) -> Result<RuntimeLifecycleOwner, EnqueueError> {
        let causal_origin =
            RuntimeCandidateCausalOrigin::mint_fresh_root(tag, class, kind, semantic_identity);
        let cache_key = (kind, causal_origin.lifecycle_key);
        if let Some(existing) = self.dormant_fresh_lifecycle_owners.get(&cache_key) {
            return existing
                .validate_exact()
                .then(|| existing.clone())
                .ok_or(EnqueueError::FailClosed);
        }
        match self
            .driver
            .dormant_producer_lifecycle(&causal_origin.lifecycle_key)
        {
            RuntimeDormantProducerLifecycle::Exact { admission_ordinal } => {
                if !self
                    .ingress
                    .lifecycle_ordinals
                    .recognizes_minted(admission_ordinal)
                    .map_err(|_| EnqueueError::FailClosed)?
                {
                    return Err(EnqueueError::FailClosed);
                }
                let owner = RuntimeLifecycleOwner::new(causal_origin, admission_ordinal)?;
                self.dormant_fresh_lifecycle_owners
                    .insert(cache_key, owner.clone());
                return Ok(owner);
            }
            RuntimeDormantProducerLifecycle::Conflict => {
                return Err(EnqueueError::FailClosed);
            }
            RuntimeDormantProducerLifecycle::Absent => {}
        }
        let capacity = self
            .ingress
            .config
            .capacity
            .checked_add(MAX_EFFECTS_PER_STEP)
            .ok_or(EnqueueError::FailClosed)?;
        if self.dormant_fresh_lifecycle_owners.len() >= capacity {
            return Err(EnqueueError::Full);
        }
        let lifecycle_ordinal = self.ingress.mint_non_fifo_lifecycle_ordinal()?;
        let owner = RuntimeLifecycleOwner::new(causal_origin, lifecycle_ordinal)?;
        self.dormant_fresh_lifecycle_owners
            .insert(cache_key, owner.clone());
        Ok(owner)
    }

    fn retain_fresh_lifecycle_alias(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        kind: RuntimeFreshRootKind,
        semantic_identity: &[u8],
        owner: &RuntimeLifecycleOwner,
    ) -> Result<(), EnqueueError> {
        if !owner.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        let alias =
            RuntimeCandidateCausalOrigin::mint_fresh_root(tag, class, kind, semantic_identity);
        let key = (kind, alias.lifecycle_key);
        match self.dormant_fresh_lifecycle_owners.get(&key) {
            Some(existing) if existing != owner => Err(EnqueueError::FailClosed),
            Some(_) => Ok(()),
            None => {
                let capacity = self
                    .ingress
                    .config
                    .capacity
                    .checked_add(MAX_EFFECTS_PER_STEP)
                    .ok_or(EnqueueError::FailClosed)?;
                if self.dormant_fresh_lifecycle_owners.len() >= capacity {
                    return Err(EnqueueError::Full);
                }
                self.dormant_fresh_lifecycle_owners
                    .insert(key, owner.clone());
                Ok(())
            }
        }
    }

    fn retain_effect_ownership(
        &mut self,
        source: RuntimeEffectSource,
        parent: Option<&RuntimeLifecycleOwner>,
        effects: &[D::Effect],
    ) -> Result<(), EnqueueError> {
        if effects.is_empty() {
            return Ok(());
        }
        if self.pending_effect_ownership.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        let mut ownership = Vec::with_capacity(effects.len());
        for effect in effects {
            let causality = if source == RuntimeEffectSource::Startup {
                RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::StartupRecovery)
            } else {
                D::effect_causality(effect, source)
            };
            let evidence = match causality {
                RuntimeEffectCausality::Inherit => {
                    let owner = parent.cloned().ok_or(EnqueueError::FailClosed)?;
                    RuntimeEffectOwnership::inherited(owner)
                }
                RuntimeEffectCausality::Fresh(kind) => {
                    let tag = D::effect_root_tag(effect).unwrap_or(self.round_tag);
                    let owner = self.mint_fresh_lifecycle_owner(
                        tag,
                        CommandClass::Progress,
                        kind,
                        &D::fresh_effect_semantic_identity(effect, kind),
                    )?;
                    RuntimeEffectOwnership::fresh(owner, kind)
                }
            };
            if !evidence.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            // Startup is fenced before live ingress. Its deterministic effect
            // order remaps restart-stable work to an order-isomorphic ordinal
            // prefix. If periodic durable-lock reconstruction can later
            // reproduce the same effect, alias that semantic producer key to
            // this startup owner so retries do not mint a younger lifecycle.
            if source == RuntimeEffectSource::Startup
                && let RuntimeEffectCausality::Fresh(kind) =
                    D::effect_causality(effect, RuntimeEffectSource::Retransmit)
            {
                let tag = D::effect_root_tag(effect).unwrap_or(self.round_tag);
                self.retain_fresh_lifecycle_alias(
                    tag,
                    CommandClass::Progress,
                    kind,
                    &D::fresh_effect_semantic_identity(effect, kind),
                    evidence.owner(),
                )?;
            }
            ownership.push(evidence);
        }
        self.pending_effect_ownership = Some(ownership);
        Ok(())
    }

    /// Consume the positional lifecycle sidecar for one returned effect batch.
    /// A second runtime step cannot overwrite this owner before the executor
    /// has accepted it.
    pub(crate) fn take_effect_ownership(
        &mut self,
        effect_count: usize,
    ) -> Result<Vec<RuntimeEffectOwnership>, String> {
        if effect_count == 0 && self.pending_effect_ownership.is_none() {
            return Ok(Vec::new());
        }
        let ownership = self
            .pending_effect_ownership
            .take()
            .ok_or_else(|| "Sumeragi v2 effect batch omitted its lifecycle ownership".to_owned())?;
        if ownership.len() != effect_count
            || ownership.iter().any(|evidence| !evidence.validate_exact())
        {
            self.latch_fail_closed("effect lifecycle ownership did not match its batch");
            return Err("Sumeragi v2 effect lifecycle ownership was invalid".to_owned());
        }
        Ok(ownership)
    }

    /// Replace the bounded set of owners currently held by retained executor
    /// effects or asynchronous Sign/Fetch/Store/Validate/Apply tasks.
    ///
    /// The executor derives this set from its existing bounded maps before
    /// each runtime step. Supplying a forged carrier or exceeding the existing
    /// pending-work plus one retained-batch bound fails closed.
    pub(crate) fn set_external_lifecycle_owners(
        &mut self,
        owners: Vec<RuntimeLifecycleOwner>,
    ) -> Result<(), String> {
        if owners.len() > self.external_lifecycle_owner_capacity
            || owners.iter().any(|owner| !owner.validate_exact())
        {
            self.latch_fail_closed("external lifecycle ownership was invalid or unbounded");
            return Err("Sumeragi v2 external lifecycle ownership was invalid".to_owned());
        }
        self.external_lifecycle_owners = owners;
        Ok(())
    }

    /// Bind external lifecycle capacity to the effect executor's existing
    /// pending-work limit plus one retained reducer-effect batch.
    ///
    /// Runtime ingress and asynchronous effect work have independent bounded
    /// configurations.  Keeping this relation explicit avoids rejecting a
    /// legitimate executor with a small ingress FIFO and a larger task bound.
    pub(crate) fn configure_external_lifecycle_owner_capacity(
        &mut self,
        max_pending_work: usize,
    ) -> Result<(), String> {
        let capacity = max_pending_work
            .checked_add(MAX_EFFECTS_PER_STEP)
            .ok_or_else(|| "external lifecycle-owner capacity overflowed".to_owned())?;
        if max_pending_work == 0 || self.external_lifecycle_owners.len() > capacity {
            self.latch_fail_closed("external lifecycle-owner capacity was invalid");
            return Err("Sumeragi v2 external lifecycle-owner capacity was invalid".to_owned());
        }
        self.external_lifecycle_owner_capacity = capacity;
        Ok(())
    }

    /// Mint the `AssembleBody` root when a deterministic local proposal is
    /// accepted into the bounded asynchronous Store -> Validate pipeline.
    pub(crate) fn mint_local_proposal_effect_ownership(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<RuntimeEffectOwnership, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        if let Some(reservation) = self.active_view_producer.as_ref() {
            if reservation.tag != tag
                || tag != self.round_tag
                || !reservation.ownership.validate_exact()
            {
                self.latch_fail_closed(
                    "local proposal changed its active-view producer reservation",
                );
                return Err(
                    "Sumeragi v2 local proposal changed its active-view producer".to_owned(),
                );
            }
            let ownership = reservation.ownership.clone();
            self.retain_fresh_lifecycle_alias(
                tag,
                CommandClass::Normal,
                RuntimeFreshRootKind::LocalProposalAdmission,
                &manifest.encode(),
                ownership.owner(),
            )
            .map_err(|error| error.to_string())?;
            return Ok(RuntimeEffectOwnership::inherited(ownership.owner().clone()));
        }
        if self.clocks_armed && tag == self.round_tag {
            self.latch_fail_closed(
                "armed local proposal admission had no active-view producer reservation",
            );
            return Err(
                "Sumeragi v2 local proposal omitted its active-view producer reservation"
                    .to_owned(),
            );
        }
        let owner = self
            .mint_fresh_lifecycle_owner(
                tag,
                CommandClass::Normal,
                RuntimeFreshRootKind::LocalProposalAdmission,
                &manifest.encode(),
            )
            .map_err(|error| error.to_string())?;
        Ok(RuntimeEffectOwnership::fresh(
            owner,
            RuntimeFreshRootKind::LocalProposalAdmission,
        ))
    }

    /// Retain or release the scheduler-visible producer for the authoritative
    /// active view. `retain = true` reuses an inherited `EnterView` carrier when
    /// present and otherwise mints the startup owner before live clocks arm.
    pub(crate) fn reconcile_active_view_producer(
        &mut self,
        tag: EventTag,
        retain: bool,
    ) -> Result<(), String> {
        if self.fail_closed || tag != self.round_tag {
            self.latch_fail_closed(
                "active-view producer reconciliation changed the authoritative tag",
            );
            return Err("Sumeragi v2 active-view producer tag was invalid".to_owned());
        }
        if !retain {
            if self
                .active_view_producer
                .as_ref()
                .is_some_and(|reservation| reservation.tag != tag)
            {
                self.latch_fail_closed(
                    "non-leader release targeted a different active-view producer",
                );
                return Err("Sumeragi v2 active-view producer release was invalid".to_owned());
            }
            self.active_view_producer = None;
            return Ok(());
        }
        if let Some(reservation) = self.active_view_producer.as_ref() {
            if reservation.tag == tag && reservation.ownership.validate_exact() {
                return Ok(());
            }
            self.latch_fail_closed(
                "active-view producer reservation changed before reconciliation",
            );
            return Err("Sumeragi v2 active-view producer reservation was invalid".to_owned());
        }
        let owner = self
            .mint_fresh_lifecycle_owner(
                tag,
                CommandClass::Normal,
                RuntimeFreshRootKind::LocalProposalAdmission,
                b"active-view-producer-reservation",
            )
            .map_err(|error| error.to_string())?;
        self.active_view_producer = Some(ActiveViewProducerReservation {
            tag,
            ownership: RuntimeEffectOwnership::fresh(
                owner,
                RuntimeFreshRootKind::LocalProposalAdmission,
            ),
        });
        Ok(())
    }

    /// Retire the exact active-view producer after its signed Proposal and
    /// canonical chunks have atomically entered guarded remote fanout.
    ///
    /// Store, validation, and signing only transfer this owner into causal
    /// successors.  They are not terminal: releasing the fence at any of
    /// those boundaries would let an already-due timeout overtake the final
    /// source-side transport admission.  A retransmission after the original
    /// fanout observes no active reservation and is an idempotent no-op.
    pub(crate) fn complete_active_view_producer_after_proposal_fanout(
        &mut self,
        proposal_round: wire::ConsensusRound,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), String> {
        let Some(reservation) = self.active_view_producer.as_ref() else {
            return Ok(());
        };
        if reservation.tag != self.round_tag
            || proposal_round.height != reservation.tag.height()
            || proposal_round.view != reservation.tag.view()
            || !reservation.ownership.validate_exact()
            || !ownership.validate_exact()
            || reservation.ownership.owner() != ownership.owner()
        {
            self.latch_fail_closed("Proposal fanout changed its active-view producer");
            return Err("Sumeragi v2 Proposal fanout changed producer ownership".to_owned());
        }
        self.active_view_producer = None;
        Ok(())
    }

    fn command_admission_preflight(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        command: &D::Command,
    ) -> Result<RuntimeCommandAdmissionPreflight, EnqueueError> {
        match self.driver.preflight_command_admission(tag, command) {
            RuntimeCommandAdmissionPreflight::ReuseDormant { .. }
                if class != CommandClass::Completion =>
            {
                self.latch_fail_closed(
                    "restart-dormant producer changed its frozen completion service class",
                );
                Err(EnqueueError::FailClosed)
            }
            preflight @ (RuntimeCommandAdmissionPreflight::Admit
            | RuntimeCommandAdmissionPreflight::ReuseDormant { .. }
            | RuntimeCommandAdmissionPreflight::Coalesce) => Ok(preflight),
            RuntimeCommandAdmissionPreflight::Reject => {
                self.latch_fail_closed(
                    "runtime command admission conflicted with frozen reducer authority",
                );
                Err(EnqueueError::FailClosed)
            }
        }
    }

    fn reject_authenticated_preflight_coalescence(
        &mut self,
        preflight: RuntimeCommandAdmissionPreflight,
    ) -> Result<RuntimeCommandAdmissionPreflight, NetworkIngressError> {
        if preflight == RuntimeCommandAdmissionPreflight::Coalesce {
            // Production authenticated ingress is always `Admit` at the adapter
            // preflight seam. Queue and Busy-deferred coalescing happen only
            // through their exact ownership carriers before this point. A
            // successful semantic-only Coalesce here would drop the fresh
            // leader-wire lifecycle without installing a physical runtime
            // owner, so reject it before registering any runtime receipt.
            self.latch_fail_closed(
                "authenticated network ingress attempted semantic-only preflight coalescence",
            );
            return Err(NetworkIngressError::FailClosed);
        }
        Ok(preflight)
    }

    fn restored_command_owner(
        &self,
        tag: EventTag,
        class: CommandClass,
        command: &D::Command,
        ingress_ownership: Option<&RuntimeIngressOwnershipEvidence>,
        causal_lifecycle_key: iroha_crypto::Hash,
        admission_ordinal: u128,
    ) -> Result<RuntimeLifecycleOwner, EnqueueError> {
        if !self
            .ingress
            .lifecycle_ordinals
            .recognizes_minted(admission_ordinal)
            .map_err(|_| EnqueueError::FailClosed)?
        {
            return Err(EnqueueError::FailClosed);
        }
        RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
            tag,
            class,
            command,
            ingress_ownership,
            causal_lifecycle_key,
            admission_ordinal,
        )
    }

    fn restored_tagged_command(
        &self,
        tag: EventTag,
        class: CommandClass,
        command: D::Command,
        admitted_at: Instant,
        causal_lifecycle_key: iroha_crypto::Hash,
        admission_ordinal: u128,
        producer_stage: u8,
    ) -> Result<TaggedCommand<D::Command>, EnqueueError> {
        let owner = self.restored_command_owner(
            tag,
            class,
            &command,
            None,
            causal_lifecycle_key,
            admission_ordinal,
        )?;
        let mut tagged = TaggedCommand::with_causal_origin(
            tag,
            class,
            command,
            admitted_at,
            owner.causal_origin().clone(),
            owner.lifecycle_ordinal(),
        )?;
        tagged.restored_producer_stage = Some(producer_stage);
        Ok(tagged)
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
        let preflight = self.command_admission_preflight(tag, class, &command)?;
        let tagged = match preflight {
            RuntimeCommandAdmissionPreflight::Coalesce => return Ok(()),
            RuntimeCommandAdmissionPreflight::Admit => {
                TaggedCommand::new(tag, class, command, Instant::now())
            }
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => self.restored_tagged_command(
                tag,
                class,
                command,
                Instant::now(),
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            )?,
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        let result = self.ingress.enqueue(tagged);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("runtime ingress exact ownership validation failed");
        }
        result
    }

    fn enqueue_with_lifecycle_owner(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        command: D::Command,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        if self.fail_closed || !ownership.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        let preflight = self.command_admission_preflight(tag, class, &command)?;
        if preflight == RuntimeCommandAdmissionPreflight::Coalesce {
            return Ok(());
        }
        let tagged = match preflight {
            RuntimeCommandAdmissionPreflight::Admit => TaggedCommand::with_causal_origin(
                tag,
                class,
                command,
                Instant::now(),
                ownership.owner().causal_origin().clone(),
                ownership.owner().lifecycle_ordinal(),
            )?,
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => self.restored_tagged_command(
                tag,
                class,
                command,
                Instant::now(),
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            )?,
            RuntimeCommandAdmissionPreflight::Coalesce => unreachable!("handled above"),
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        let result = self.ingress.enqueue(tagged);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("causal-successor ingress ownership validation failed");
        }
        result
    }

    fn reconcile_deferred_ingress_ownership(
        &mut self,
        handoff: Option<(u128, RuntimeIngressOwnershipEvidence)>,
    ) -> Result<(), RuntimeIngressMergeError> {
        let active = self.driver.authenticated_deferred_admission_ordinals();
        let mut retained = self.deferred_ingress_ownership.clone();
        let mut lifecycle_ownership = self.deferred_lifecycle_ownership.clone();
        if let Some((ordinal, candidate)) = handoff {
            if !active.contains(&ordinal) || !candidate.validate_exact() {
                return Err(RuntimeIngressMergeError::Conflict);
            }
            match retained.get_mut(&ordinal) {
                Some(existing) => {
                    let previous_lifecycle = existing.earliest_lifecycle_ordinal()?;
                    existing.merge_downstream(candidate)?;
                    let merged_lifecycle = existing.earliest_lifecycle_ordinal()?;
                    if matches!(
                        (previous_lifecycle, merged_lifecycle),
                        (Some(previous), Some(merged)) if merged < previous
                    ) {
                        let merged_lifecycle = merged_lifecycle.expect("matched tagged lifecycle");
                        if self
                            .active_lifecycle_uses_ordinal(merged_lifecycle)
                            .map_err(|_| RuntimeIngressMergeError::Conflict)?
                        {
                            return Err(RuntimeIngressMergeError::Conflict);
                        }
                        let ingress_identity =
                            runtime_ingress_causal_origin_projection_hash(existing);
                        let owner = lifecycle_ownership
                            .get(&ordinal)
                            .ok_or(RuntimeIngressMergeError::Conflict)?
                            .rebase_deferred_ingress(merged_lifecycle, ingress_identity)?;
                        lifecycle_ownership.insert(ordinal, owner);
                    } else if previous_lifecycle.is_some() != merged_lifecycle.is_some() {
                        return Err(RuntimeIngressMergeError::Conflict);
                    }
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
        self.deferred_lifecycle_ownership = lifecycle_ownership;
        Ok(())
    }

    fn reconcile_deferred_lifecycle_ownership_after_retirement(
        &mut self,
    ) -> Result<(), RuntimeIngressMergeError> {
        let active = self.driver.all_deferred_admission_ordinals();
        let mut retained = self.deferred_lifecycle_ownership.clone();
        retained.retain(|ordinal, _| active.contains(ordinal));
        if retained.len() != active.len()
            || retained.values().any(|owner| !owner.validate_exact())
            || !active.iter().all(|ordinal| retained.contains_key(ordinal))
        {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        self.deferred_lifecycle_ownership = retained;
        Ok(())
    }

    fn active_leader_wire_runtime_ordinals(
        &self,
    ) -> Result<BTreeSet<u128>, RuntimeIngressMergeError> {
        let mut active = BTreeSet::new();
        for queued in &self.ingress.commands {
            let Some(ownership) = queued.ingress_ownership.as_ref() else {
                continue;
            };
            let Some(ordinal) = ownership.leader_wire_scheduler_ordinal()? else {
                continue;
            };
            if queued.lifecycle_ordinal != Some(ordinal) || !active.insert(ordinal) {
                return Err(RuntimeIngressMergeError::Conflict);
            }
        }
        for ownership in self.deferred_ingress_ownership.values() {
            let Some(ordinal) = ownership.leader_wire_scheduler_ordinal()? else {
                continue;
            };
            if !active.insert(ordinal) {
                return Err(RuntimeIngressMergeError::Conflict);
            }
        }
        Ok(active)
    }

    fn retire_orphaned_leader_wire_runtime_receipts(
        &mut self,
    ) -> Result<(), RuntimeIngressMergeError> {
        let active = self.active_leader_wire_runtime_ordinals()?;
        let retired = self
            .leader_wire_runtime_receipts
            .keys()
            .filter(|ordinal| !active.contains(ordinal))
            .copied()
            .collect::<Vec<_>>();
        for ordinal in retired {
            let receipt = self
                .leader_wire_runtime_receipts
                .remove(&ordinal)
                .ok_or(RuntimeIngressMergeError::Conflict)?;
            self.pending_leader_wire_terminals
                .push_back(LeaderWireRuntimeTerminal::Volatile(receipt));
        }
        Ok(())
    }

    fn register_leader_wire_runtime_receipt(
        &mut self,
        ownership: &RuntimeIngressOwnershipEvidence,
    ) -> Result<(), RuntimeIngressMergeError> {
        let Some(receipt) = ownership.leader_wire_runtime_receipt()?.cloned() else {
            return Ok(());
        };
        let ordinal = receipt.owner().admission_ordinal();
        if ordinal == 0 || ordinal != receipt.token().scheduler_ordinal() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        match self.leader_wire_runtime_receipts.get(&ordinal) {
            Some(existing) if existing != &receipt => Err(RuntimeIngressMergeError::Conflict),
            Some(_) => Ok(()),
            None => {
                self.leader_wire_runtime_receipts.insert(ordinal, receipt);
                Ok(())
            }
        }
    }

    fn complete_leader_wire_runtime_owner(
        &mut self,
        parent: &RuntimeLifecycleOwner,
        handoff: Option<(
            ProducerContinuationHandoffEvidence,
            ProducerContinuationTerminalToken,
        )>,
    ) -> Result<(), RuntimeError<D::Error>> {
        let Some(receipt) = self
            .leader_wire_runtime_receipts
            .get(&parent.lifecycle_ordinal())
            .cloned()
        else {
            return Ok(());
        };
        if !parent.validate_exact()
            || receipt.owner().admission_ordinal() != parent.lifecycle_ordinal()
            || receipt.owner().causal_lifecycle_key() != parent.causal_origin().lifecycle_key
        {
            self.latch_fail_closed("leader-wire terminal changed its runtime lifecycle owner");
            return Err(RuntimeError::FailClosed);
        }
        let event = match handoff {
            Some((ProducerContinuationHandoffEvidence::DurableTerminal, terminal)) => {
                LeaderWireRuntimeTerminal::Producer {
                    runtime: receipt,
                    terminal,
                }
            }
            Some((
                ProducerContinuationHandoffEvidence::ConcreteSuccessor
                | ProducerContinuationHandoffEvidence::VolatileTerminal,
                _,
            ))
            | None => LeaderWireRuntimeTerminal::Volatile(receipt),
        };
        self.leader_wire_runtime_receipts
            .remove(&parent.lifecycle_ordinal());
        self.pending_leader_wire_terminals.push_back(event);
        Ok(())
    }

    /// Move the bounded terminal sidecar into the effect executor. A later
    /// scheduler turn is forbidden until this exact batch is consumed.
    pub(crate) fn take_leader_wire_runtime_terminals(&mut self) -> Vec<LeaderWireRuntimeTerminal> {
        self.pending_leader_wire_terminals.drain(..).collect()
    }

    fn accept_driver_dispatch(
        &mut self,
        dispatch: RuntimeDriverDispatch<D::Effect>,
        parent: &RuntimeLifecycleOwner,
    ) -> Result<
        (
            Vec<D::Effect>,
            bool,
            Option<ProducerContinuationHandoffToken>,
            bool,
        ),
        RuntimeError<D::Error>,
    > {
        let RuntimeDriverDispatch {
            effects,
            deferred_ingress,
            deferred_ordinal,
            retry_unadmitted,
            producer_handoff,
        } = dispatch;
        let retained_deferred_ingress = deferred_ingress.is_some();
        if retry_unadmitted
            && (deferred_ingress.is_some()
                || deferred_ordinal.is_some()
                || !effects.is_empty()
                || producer_handoff.is_some())
        {
            self.latch_fail_closed("retryable driver backpressure changed downstream ownership");
            return Err(RuntimeError::FailClosed);
        }
        if self
            .reconcile_deferred_ingress_ownership(deferred_ingress)
            .is_err()
        {
            self.latch_fail_closed("driver dispatch lost deferred ingress ownership");
            return Err(RuntimeError::FailClosed);
        }
        let active = self.driver.all_deferred_admission_ordinals();
        let mut retained = self.deferred_lifecycle_ownership.clone();
        if let Some(ordinal) = deferred_ordinal {
            if !active.contains(&ordinal) || !parent.validate_exact() {
                self.latch_fail_closed("driver dispatch forged deferred lifecycle ownership");
                return Err(RuntimeError::FailClosed);
            }
            match retained.get(&ordinal) {
                Some(existing) if existing != parent => {
                    self.latch_fail_closed("deferred ordinal changed its lifecycle owner");
                    return Err(RuntimeError::FailClosed);
                }
                Some(_) => {}
                None => {
                    retained.insert(ordinal, parent.clone());
                }
            }
        }
        retained.retain(|ordinal, _| active.contains(ordinal));
        if retained.len() != active.len()
            || retained.values().any(|owner| !owner.validate_exact())
            || !active.iter().all(|ordinal| retained.contains_key(ordinal))
        {
            self.latch_fail_closed("driver dispatch lost deferred lifecycle ownership");
            return Err(RuntimeError::FailClosed);
        }
        self.deferred_lifecycle_ownership = retained;
        Ok((
            effects,
            retry_unadmitted,
            producer_handoff,
            retained_deferred_ingress,
        ))
    }

    fn freeze_due_clock_owners(&mut self, now: Instant) -> Result<(), EnqueueError> {
        // Validate every active owner before a clock can bypass it.
        let _ = self.minimum_active_lifecycle_ordinal()?;
        if !self.clocks_armed {
            return Ok(());
        }
        let raw_timeout_due = !self.timeout_emitted
            && now.saturating_duration_since(self.round_started_at)
                >= round_timeout_for_view(self.base_round_timeout, self.round_tag.view());
        if raw_timeout_due && self.timeout_owner.is_none() {
            self.timeout_owner = Some(self.mint_fresh_lifecycle_owner(
                self.round_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::Timeout,
                b"begin-timeout",
            )?);
        }
        let raw_retransmit_due =
            now.saturating_duration_since(self.retransmit_started_at) >= self.retransmit_interval;
        // A retransmission owner which was already frozen before the absolute
        // deadline may finish its one bounded episode. Once timeout is due,
        // however, do not replenish that lower-ordinal producer: otherwise
        // the dormant retry alias could be resurrected ahead of the timeout on
        // every call and form a proofless scheduling lasso.
        if raw_retransmit_due && !raw_timeout_due && self.retransmit_owner.is_none() {
            self.retransmit_owner = Some(self.mint_fresh_lifecycle_owner(
                self.round_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::Retransmit,
                b"periodic-retransmit",
            )?);
        }
        Ok(())
    }

    fn minimum_active_lifecycle_ordinal(&self) -> Result<Option<u128>, EnqueueError> {
        let mut minimum = self.ingress.oldest_active_lifecycle_ordinal()?;
        let mut observe = |owner: &RuntimeLifecycleOwner| -> Result<(), EnqueueError> {
            if !owner.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            minimum = Some(minimum.map_or(owner.lifecycle_ordinal(), |ordinal| {
                ordinal.min(owner.lifecycle_ordinal())
            }));
            Ok(())
        };
        for owner in self.deferred_lifecycle_ownership.values() {
            observe(owner)?;
        }
        if let Some(owner) = &self.timeout_owner {
            observe(owner)?;
        }
        if let Some(owner) = &self.retransmit_owner {
            observe(owner)?;
        }
        if let Some(reservation) = &self.active_view_producer {
            if reservation.tag != self.round_tag || !reservation.ownership.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            observe(reservation.ownership.owner())?;
        }
        for owner in &self.external_lifecycle_owners {
            observe(owner)?;
        }
        if let Some(ownership) = &self.pending_effect_ownership {
            for effect in ownership {
                observe(effect.owner())?;
            }
        }
        if let Some(reservation) = &self.ingress.reserved_body_available {
            let owner = reservation
                .lifecycle_owner()
                .ok_or(EnqueueError::FailClosed)?;
            observe(&owner)?;
        }
        Ok(minimum)
    }

    fn active_lifecycle_uses_ordinal(&self, lifecycle_ordinal: u128) -> Result<bool, EnqueueError> {
        if self.ingress.uses_lifecycle_ordinal(lifecycle_ordinal)? {
            return Ok(true);
        }
        let owner_matches = |owner: &RuntimeLifecycleOwner| {
            owner.validate_exact() && owner.lifecycle_ordinal() == lifecycle_ordinal
        };
        if self
            .deferred_lifecycle_ownership
            .values()
            .any(|owner| owner_matches(owner))
            || self
                .timeout_owner
                .as_ref()
                .is_some_and(|owner| owner_matches(owner))
            || self
                .retransmit_owner
                .as_ref()
                .is_some_and(|owner| owner_matches(owner))
            || self
                .active_view_producer
                .as_ref()
                .is_some_and(|reservation| owner_matches(reservation.ownership.owner()))
            || self
                .external_lifecycle_owners
                .iter()
                .any(|owner| owner_matches(owner))
            || self
                .pending_effect_ownership
                .iter()
                .flatten()
                .any(|effect| owner_matches(effect.owner()))
        {
            return Ok(true);
        }
        Ok(false)
    }

    /// Freeze every due clock and compare the complete runtime owner set with
    /// one exact Serve ingress ticket from the shared ordinal source.
    ///
    /// The caller publishes executor-retained owners immediately before this
    /// query. Returning `true` authorizes one bounded producer episode; it does
    /// not authorize a loop. A runtime owner equal to the external ticket is a
    /// source-uniqueness violation and latches fail-closed.
    pub(crate) fn older_lifecycle_predates_exact_serve(
        &mut self,
        now: Instant,
        serve_lifecycle_ordinal: u128,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let recognized = match self
            .ingress
            .lifecycle_ordinals
            .recognizes_minted(serve_lifecycle_ordinal)
        {
            Ok(recognized) => recognized,
            Err(reason) => {
                self.latch_fail_closed(reason.clone());
                return Err(reason);
            }
        };
        if !recognized {
            self.latch_fail_closed("exact Serve barrier used an unminted lifecycle ordinal");
            return Err("Sumeragi v2 exact Serve barrier ordinal was invalid".to_owned());
        }
        if self.freeze_due_clock_owners(now).is_err() {
            self.latch_fail_closed("clock lifecycle ownership could not be frozen for Serve");
            return Err("Sumeragi v2 clock lifecycle ownership could not be frozen".to_owned());
        }
        let collision = match self.active_lifecycle_uses_ordinal(serve_lifecycle_ordinal) {
            Ok(collision) => collision,
            Err(_) => {
                self.latch_fail_closed("runtime lifecycle ownership was invalid for Serve");
                return Err("Sumeragi v2 runtime lifecycle ownership was invalid".to_owned());
            }
        };
        if collision {
            self.latch_fail_closed("runtime and Serve claimed one lifecycle ordinal");
            return Err("Sumeragi v2 lifecycle ordinal ownership collided".to_owned());
        }
        match self.minimum_active_lifecycle_ordinal() {
            Ok(minimum) => Ok(minimum.is_some_and(|ordinal| ordinal < serve_lifecycle_ordinal)),
            Err(_) => {
                self.latch_fail_closed("runtime lifecycle ownership was invalid for Serve");
                Err("Sumeragi v2 runtime lifecycle ownership was invalid".to_owned())
            }
        }
    }

    fn older_active_lifecycle_blocks(&self, owner: &RuntimeLifecycleOwner) -> bool {
        self.minimum_active_lifecycle_ordinal()
            .map_or(true, |minimum| {
                minimum.is_some_and(|ordinal| ordinal < owner.lifecycle_ordinal())
            })
    }

    fn scheduler_arbitration_inputs(
        &self,
        now: Instant,
    ) -> Result<RuntimeSchedulerArbitrationInputs, EnqueueError> {
        let global_minimum = self.minimum_active_lifecycle_ordinal()?;
        let fifo_minimum = self.ingress.oldest_lifecycle_ordinal()?;
        let fifo_ready = fifo_minimum.is_some() && fifo_minimum == global_minimum;
        let (completion_ready, progress_ready, normal_ready) =
            if let Some(ordinal) = fifo_minimum.filter(|_| fifo_ready) {
                self.ingress.class_readiness_at_lifecycle(ordinal)
            } else {
                (false, false, false)
            };
        let timers_enabled = self.clocks_armed;
        let raw_timeout_due = timers_enabled
            && !self.timeout_emitted
            && now.saturating_duration_since(self.round_started_at)
                >= round_timeout_for_view(self.base_round_timeout, self.round_tag.view());
        let timeout_due = raw_timeout_due
            && self
                .timeout_owner
                .as_ref()
                .is_some_and(|owner| !self.older_active_lifecycle_blocks(owner));
        let raw_periodic_timer_due = timers_enabled
            && now.saturating_duration_since(self.retransmit_started_at)
                >= self.retransmit_interval;
        let periodic_timer_due = raw_periodic_timer_due
            && !timeout_due
            && self
                .retransmit_owner
                .as_ref()
                .is_some_and(|owner| !self.older_active_lifecycle_blocks(owner));
        Ok(RuntimeSchedulerArbitrationInputs {
            live_mode: timers_enabled,
            timeout_due,
            periodic_timer_due,
            fifo_ready,
            completion_ready,
            progress_ready,
            normal_ready,
        })
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
    /// Older serviceable adapter debt runs first. Every FIFO, deferred, timer,
    /// reservation, effect, and external owner then competes by its immutable
    /// lifecycle ordinal, so a newly due clock cannot overtake a previously
    /// admitted lifecycle and later FIFO churn cannot overtake a frozen timer.
    /// Among simultaneously eligible clocks, timeout wins and is emitted at
    /// most once for the installed view. Retransmission runs at most once per
    /// call and advances from the actual service time, avoiding an unbounded
    /// catch-up burst after a paused process. Neither clock is changed by an
    /// arbitrary message or by any effect other than `EnterView`.
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
        if self.pending_effect_ownership.is_some() {
            self.latch_fail_closed("live scheduling overtook an unconsumed effect owner");
            return Err(RuntimeError::FailClosed);
        }
        if !self.pending_leader_wire_terminals.is_empty() {
            self.latch_fail_closed(
                "live scheduling overtook an unconsumed leader-wire terminal owner",
            );
            return Err(RuntimeError::FailClosed);
        }
        if !self.clocks_armed {
            return Err(RuntimeError::ClocksNotArmed);
        }
        if self.freeze_due_clock_owners(now).is_err() {
            self.latch_fail_closed("clock lifecycle ownership could not be frozen");
            return Err(RuntimeError::FailClosed);
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
        let arbitration = self.scheduler_arbitration_inputs(now).map_err(|_| {
            self.latch_fail_closed("scheduler lifecycle ownership was invalid");
            RuntimeError::FailClosed
        })?;
        let (work, next_schedule) = self.schedule.select(
            arbitration.timeout_due,
            arbitration.periodic_timer_due,
            arbitration.fifo_ready,
        );
        self.schedule = next_schedule;

        let (effects, effect_source, effect_parent, producer_handoff, retained_deferred_ingress) =
            match work {
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
                    let owner = self.timeout_owner.clone().ok_or_else(|| {
                        self.latch_fail_closed("due timeout had no frozen lifecycle owner");
                        RuntimeError::FailClosed
                    })?;
                    if let Err(error) = self.driver.bind_selected_producer_lifecycle(&owner) {
                        return Err(self.close(error));
                    }
                    let dispatch = self.driver.timeout_elapsed(self.round_tag);
                    self.driver.clear_selected_producer_lifecycle();
                    let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
                        match dispatch {
                            Ok(dispatch) => self.accept_driver_dispatch(dispatch, &owner)?,
                            Err(error) => return Err(self.close(error)),
                        };
                    if retry_unadmitted {
                        self.latch_fail_closed(
                            "timeout backpressure had no physical command owner to retain",
                        );
                        return Err(RuntimeError::FailClosed);
                    }
                    if self.timeout_owner.as_ref() != Some(&owner) {
                        self.latch_fail_closed(
                            "timeout lifecycle reservation changed before transfer",
                        );
                        return Err(RuntimeError::FailClosed);
                    }
                    self.timeout_owner = None;
                    (
                        effects,
                        RuntimeEffectSource::Timeout,
                        owner,
                        producer_handoff,
                        retained_deferred_ingress,
                    )
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
                    let owner = self.retransmit_owner.clone().ok_or_else(|| {
                        self.latch_fail_closed("due retransmission had no frozen lifecycle owner");
                        RuntimeError::FailClosed
                    })?;
                    if let Err(error) = self.driver.bind_selected_producer_lifecycle(&owner) {
                        return Err(self.close(error));
                    }
                    let dispatch = self.driver.retransmit_elapsed(self.round_tag);
                    self.driver.clear_selected_producer_lifecycle();
                    let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
                        match dispatch {
                            Ok(dispatch) => self.accept_driver_dispatch(dispatch, &owner)?,
                            Err(error) => return Err(self.close(error)),
                        };
                    if retry_unadmitted {
                        self.latch_fail_closed(
                            "retransmission backpressure had no physical command owner to retain",
                        );
                        return Err(RuntimeError::FailClosed);
                    }
                    if self.retransmit_owner.as_ref() != Some(&owner) {
                        self.latch_fail_closed(
                            "retransmission lifecycle reservation changed before transfer",
                        );
                        return Err(RuntimeError::FailClosed);
                    }
                    self.retransmit_owner = None;
                    (
                        effects,
                        RuntimeEffectSource::Retransmit,
                        owner,
                        producer_handoff,
                        retained_deferred_ingress,
                    )
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
                    let owner = match command.lifecycle_owner() {
                        Ok(owner)
                            if owner.lifecycle_ordinal() == candidate.lifecycle_ordinal
                                && owner.causal_origin() == &candidate.causal_origin =>
                        {
                            owner
                        }
                        Ok(_) | Err(_) => {
                            self.latch_fail_closed(
                                "selected FIFO lifecycle owner was inconsistent",
                            );
                            return Err(RuntimeError::FailClosed);
                        }
                    };
                    let retry_command = command.clone();
                    let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
                        match self.driver.dispatch(command) {
                            Ok(dispatch) => self.accept_driver_dispatch(dispatch, &owner)?,
                            Err(error) => return Err(self.close(error)),
                        };
                    if retry_unadmitted {
                        if self
                            .ingress
                            .restore_selected_command(retry_command, &candidate)
                            .is_err()
                        {
                            self.latch_fail_closed(
                                "retryable FIFO backpressure could not restore its exact owner",
                            );
                            return Err(RuntimeError::FailClosed);
                        }
                        let queue_after = self.ingress.ownership_projection();
                        self.retain_scheduler_ownership(
                            RuntimeSelectedOwnerKind::FifoRetryRetained,
                            selected_round_tag,
                            RuntimeSelectedCandidateOwnership::Exact(candidate),
                            queue_before,
                            queue_after,
                            arbitration,
                            schedule_before,
                            next_schedule,
                        )?;
                        return Ok(RuntimeStep::Advanced(Vec::new()));
                    }
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
                    (
                        effects,
                        RuntimeEffectSource::Fifo,
                        owner,
                        producer_handoff,
                        retained_deferred_ingress,
                    )
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
        if self
            .retain_effect_ownership(effect_source, Some(&effect_parent), &effects)
            .is_err()
        {
            match effect_source {
                RuntimeEffectSource::Timeout => {
                    self.timeout_owner = Some(effect_parent.clone());
                }
                RuntimeEffectSource::Retransmit => {
                    self.retransmit_owner = Some(effect_parent.clone());
                }
                RuntimeEffectSource::Startup
                | RuntimeEffectSource::Fifo
                | RuntimeEffectSource::Deferred => {}
            }
            self.latch_fail_closed("effect lifecycle ownership could not be retained");
            return Err(RuntimeError::FailClosed);
        }
        let mut completed_producer_handoff = None;
        if let Some(token) = producer_handoff {
            if token.identity().admission_ordinal() != effect_parent.lifecycle_ordinal()
                || token.identity().causal_lifecycle_key()
                    != effect_parent.causal_origin().lifecycle_key
            {
                self.latch_fail_closed("producer handoff changed its selected lifecycle identity");
                return Err(RuntimeError::FailClosed);
            }
            let evidence = match self
                .driver
                .producer_handoff_evidence(token, !effects.is_empty())
            {
                Ok(evidence) => evidence,
                Err(error) => {
                    self.latch_fail_closed(format!(
                        "producer handoff evidence failed after successor retention: {error}"
                    ));
                    return Err(RuntimeError::FailClosed);
                }
            };
            let terminal = match self.driver.acknowledge_producer_handoff(token, evidence) {
                Ok(terminal) => terminal,
                Err(error) => {
                    self.latch_fail_closed(format!(
                        "producer handoff acknowledgement failed after successor retention: {error}"
                    ));
                    return Err(RuntimeError::FailClosed);
                }
            };
            completed_producer_handoff = Some((evidence, terminal));
        }
        if !retained_deferred_ingress {
            self.complete_leader_wire_runtime_owner(&effect_parent, completed_producer_handoff)?;
        }
        if self.observe_effects(now, &effects).is_err() {
            self.latch_fail_closed("effect observation lost active-view producer ownership");
            return Err(RuntimeError::FailClosed);
        }
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
        if self.pending_effect_ownership.is_some() {
            self.latch_fail_closed("recovery overtook an unconsumed effect owner");
            return Err(RuntimeError::FailClosed);
        }
        if !self.pending_leader_wire_terminals.is_empty() {
            self.latch_fail_closed("recovery overtook an unconsumed leader-wire terminal owner");
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
        let arbitration = self.scheduler_arbitration_inputs(now).map_err(|_| {
            self.latch_fail_closed("recovery scheduler lifecycle ownership was invalid");
            RuntimeError::FailClosed
        })?;
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
        let owner = match command.lifecycle_owner() {
            Ok(owner)
                if owner.lifecycle_ordinal() == candidate.lifecycle_ordinal
                    && owner.causal_origin() == &candidate.causal_origin =>
            {
                owner
            }
            Ok(_) | Err(_) => {
                self.latch_fail_closed("recovery FIFO lifecycle owner was inconsistent");
                return Err(RuntimeError::FailClosed);
            }
        };
        let retry_command = command.clone();
        let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
            match self.driver.dispatch(command) {
                Ok(dispatch) => self.accept_driver_dispatch(dispatch, &owner)?,
                Err(error) => return Err(self.close(error)),
            };
        if retry_unadmitted {
            if self
                .ingress
                .restore_selected_command(retry_command, &candidate)
                .is_err()
            {
                self.latch_fail_closed(
                    "retryable recovery FIFO backpressure could not restore its exact owner",
                );
                return Err(RuntimeError::FailClosed);
            }
            let queue_after = self.ingress.ownership_projection();
            self.retain_scheduler_ownership(
                RuntimeSelectedOwnerKind::RecoveryFifoRetryRetained,
                round_tag,
                RuntimeSelectedCandidateOwnership::Exact(candidate),
                queue_before,
                queue_after,
                arbitration,
                schedule_before,
                schedule_after,
            )?;
            return Ok(RuntimeStep::Advanced(Vec::new()));
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
        if self
            .retain_effect_ownership(RuntimeEffectSource::Fifo, Some(&owner), &effects)
            .is_err()
        {
            self.latch_fail_closed("recovery effect lifecycle ownership could not be retained");
            return Err(RuntimeError::FailClosed);
        }
        let mut completed_producer_handoff = None;
        if let Some(token) = producer_handoff {
            if token.identity().admission_ordinal() != owner.lifecycle_ordinal()
                || token.identity().causal_lifecycle_key() != owner.causal_origin().lifecycle_key
            {
                self.latch_fail_closed(
                    "recovery producer handoff changed its selected lifecycle identity",
                );
                return Err(RuntimeError::FailClosed);
            }
            let evidence = match self
                .driver
                .producer_handoff_evidence(token, !effects.is_empty())
            {
                Ok(evidence) => evidence,
                Err(error) => {
                    self.latch_fail_closed(format!(
                        "recovery producer handoff evidence failed: {error}"
                    ));
                    return Err(RuntimeError::FailClosed);
                }
            };
            let terminal = match self.driver.acknowledge_producer_handoff(token, evidence) {
                Ok(terminal) => terminal,
                Err(error) => {
                    self.latch_fail_closed(format!(
                        "recovery producer handoff acknowledgement failed: {error}"
                    ));
                    return Err(RuntimeError::FailClosed);
                }
            };
            completed_producer_handoff = Some((evidence, terminal));
        }
        if !retained_deferred_ingress {
            self.complete_leader_wire_runtime_owner(&owner, completed_producer_handoff)?;
        }
        if self.observe_effects(now, &effects).is_err() {
            self.latch_fail_closed(
                "recovery effect observation lost active-view producer ownership",
            );
            return Err(RuntimeError::FailClosed);
        }
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
        let active_deferred = self.driver.all_deferred_admission_ordinals();
        let deferred_minimum = self
            .deferred_lifecycle_ownership
            .values()
            .map(|owner| {
                owner
                    .validate_exact()
                    .then_some(owner.lifecycle_ordinal())
                    .ok_or(EnqueueError::FailClosed)
            })
            .try_fold(None, |minimum, ordinal| {
                let ordinal = ordinal?;
                Ok::<_, EnqueueError>(Some(
                    minimum.map_or(ordinal, |minimum: u128| minimum.min(ordinal)),
                ))
            })
            .map_err(|_| {
                self.latch_fail_closed("deferred lifecycle ownership was invalid");
                RuntimeError::FailClosed
            })?;
        if self.deferred_lifecycle_ownership.len() != active_deferred.len()
            || !active_deferred
                .iter()
                .all(|ordinal| self.deferred_lifecycle_ownership.contains_key(ordinal))
        {
            #[cfg(not(test))]
            {
                self.latch_fail_closed("serviceable deferred work lost its lifecycle owner");
                return Err(RuntimeError::FailClosed);
            }
            #[cfg(test)]
            if !self.deferred_lifecycle_ownership.is_empty() || !active_deferred.is_empty() {
                self.latch_fail_closed("serviceable deferred work lost its lifecycle owner");
                return Err(RuntimeError::FailClosed);
            }
        }
        let eligible = match deferred_minimum {
            Some(deferred_minimum) => {
                let global_minimum = self.minimum_active_lifecycle_ordinal().map_err(|_| {
                    self.latch_fail_closed("global lifecycle ownership was invalid");
                    RuntimeError::FailClosed
                })?;
                if global_minimum != Some(deferred_minimum) {
                    return Ok(None);
                }
                self.deferred_lifecycle_ownership
                    .iter()
                    .filter_map(|(ordinal, owner)| {
                        (owner.lifecycle_ordinal() == deferred_minimum).then_some(*ordinal)
                    })
                    .collect::<BTreeSet<_>>()
            }
            #[cfg(test)]
            None => BTreeSet::new(),
            #[cfg(not(test))]
            None => {
                self.latch_fail_closed("serviceable deferred work had no lifecycle owner");
                return Err(RuntimeError::FailClosed);
            }
        };
        let round_tag = self.round_tag;
        let queue_before = self.ingress.ownership_projection();
        let schedule = self.schedule;
        let arbitration = self.scheduler_arbitration_inputs(now).map_err(|_| {
            self.latch_fail_closed("deferred scheduler lifecycle ownership was invalid");
            RuntimeError::FailClosed
        })?;
        let queue_after = self.ingress.ownership_projection();
        let dispatch = match self.driver.dispatch_deferred(&eligible) {
            Ok(dispatch) => dispatch,
            Err(error) => return Err(self.close(error)),
        };
        let Some((effects, evidence, producer_handoff)) = dispatch else {
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
        let deferred_ordinal = evidence.admission_ordinal;
        let lifecycle_owner = self.deferred_lifecycle_ownership.remove(&deferred_ordinal);
        #[cfg(test)]
        let lifecycle_owner =
            lifecycle_owner.or_else(|| self.driver.synthetic_deferred_lifecycle_owner(&evidence));
        let Some(lifecycle_owner) = lifecycle_owner else {
            self.latch_fail_closed("deferred service had no lifecycle owner");
            return Err(RuntimeError::FailClosed);
        };
        let active_deferred = self.driver.all_deferred_admission_ordinals();
        self.deferred_lifecycle_ownership
            .retain(|ordinal, _| active_deferred.contains(ordinal));
        if self.deferred_lifecycle_ownership.len() != active_deferred.len()
            || self
                .deferred_lifecycle_ownership
                .values()
                .any(|owner| !owner.validate_exact())
        {
            self.latch_fail_closed("deferred service changed lifecycle ownership");
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
        if self
            .retain_effect_ownership(
                RuntimeEffectSource::Deferred,
                Some(&lifecycle_owner),
                &effects,
            )
            .is_err()
        {
            self.latch_fail_closed("deferred effect lifecycle ownership could not be retained");
            return Err(RuntimeError::FailClosed);
        }
        let mut completed_producer_handoff = None;
        if let Some(token) = producer_handoff {
            if token.identity().admission_ordinal() != lifecycle_owner.lifecycle_ordinal()
                || token.identity().causal_lifecycle_key()
                    != lifecycle_owner.causal_origin().lifecycle_key
            {
                self.latch_fail_closed(
                    "deferred producer handoff changed its selected lifecycle identity",
                );
                return Err(RuntimeError::FailClosed);
            }
            let handoff_evidence = match self
                .driver
                .producer_handoff_evidence(token, !effects.is_empty())
            {
                Ok(evidence) => evidence,
                Err(error) => {
                    self.latch_fail_closed(format!(
                        "deferred producer handoff evidence failed: {error}"
                    ));
                    return Err(RuntimeError::FailClosed);
                }
            };
            let terminal = match self
                .driver
                .acknowledge_producer_handoff(token, handoff_evidence)
            {
                Ok(terminal) => terminal,
                Err(error) => {
                    self.latch_fail_closed(format!(
                        "deferred producer handoff acknowledgement failed: {error}"
                    ));
                    return Err(RuntimeError::FailClosed);
                }
            };
            completed_producer_handoff = Some((handoff_evidence, terminal));
        }
        self.complete_leader_wire_runtime_owner(&lifecycle_owner, completed_producer_handoff)?;
        if self.observe_effects(now, &effects).is_err() {
            self.latch_fail_closed(
                "deferred effect observation lost active-view producer ownership",
            );
            return Err(RuntimeError::FailClosed);
        }
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
        if let Ok(step) = &result {
            self.take_last_scheduler_ownership()
                .expect("every successful live scheduler turn retains exact ownership");
            if let RuntimeStep::Advanced(effects) = step {
                self.take_effect_ownership(effects.len())
                    .expect("test executor consumes the exact live effect sidecar");
            }
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
        if let Ok(step) = &result {
            self.take_last_scheduler_ownership()
                .expect("every successful recovery scheduler turn retains exact ownership");
            if let RuntimeStep::Advanced(effects) = step {
                self.take_effect_ownership(effects.len())
                    .expect("test executor consumes the exact recovery effect sidecar");
            }
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

    /// Freeze and return the deterministic timeout owner used by restart
    /// lifecycle tests before dispatch mutates the adapter.
    #[cfg(test)]
    pub(crate) fn frozen_timeout_owner_for_test(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeLifecycleOwner, String> {
        self.freeze_due_clock_owners(now)
            .map_err(|error| error.to_string())?;
        self.timeout_owner
            .clone()
            .ok_or_else(|| "timeout lifecycle owner was not due".to_owned())
    }

    fn observe_effects(&mut self, now: Instant, effects: &[D::Effect]) -> Result<(), EnqueueError> {
        for (index, effect) in effects.iter().enumerate() {
            if let Some(tag) = D::enter_view_tag(effect) {
                let ownership = self
                    .pending_effect_ownership
                    .as_ref()
                    .and_then(|ownership| ownership.get(index))
                    .cloned()
                    .filter(RuntimeEffectOwnership::validate_exact)
                    .ok_or(EnqueueError::FailClosed)?;
                self.round_tag = tag;
                self.round_started_at = now;
                self.retransmit_started_at = now;
                self.timeout_emitted = false;
                self.timeout_owner = None;
                self.retransmit_owner = None;
                self.dormant_fresh_lifecycle_owners.retain(|_, owner| {
                    let root = owner.causal_origin().root_tag;
                    root.height() == tag.height() && root.view() == tag.view()
                });
                self.active_view_producer = Some(ActiveViewProducerReservation { tag, ownership });
                self.schedule = ScheduleState::default();
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn observe_effects_with_test_ownership(
        &mut self,
        now: Instant,
        effects: &[D::Effect],
    ) -> Result<(), EnqueueError> {
        if self.pending_effect_ownership.is_some() {
            return self.observe_effects(now, effects);
        }
        let owner = self
            .dormant_fresh_lifecycle_owners
            .values()
            .next()
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| {
                self.mint_fresh_lifecycle_owner(
                    self.round_tag,
                    CommandClass::Progress,
                    RuntimeFreshRootKind::StartupRecovery,
                    b"direct-test-enter-view-owner",
                )
            })?;
        self.pending_effect_ownership = Some(
            effects
                .iter()
                .map(|_| RuntimeEffectOwnership::inherited(owner.clone()))
                .collect(),
        );
        let result = self.observe_effects(now, effects);
        self.pending_effect_ownership = None;
        result
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

    fn enqueue_body_pipeline_completion_with_owner(
        &mut self,
        tag: EventTag,
        evidence: BodyPipelineCompletionEvidence,
        command: AdapterCommand,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        if self.body_pipeline_completion_is_owned(tag, &evidence)? {
            return Ok(());
        }
        self.enqueue_with_lifecycle_owner(tag, CommandClass::Completion, command, ownership)
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

    /// Open a runtime whose FIFO and fresh roots share the active height's
    /// actor-global source with exact Serve ingress reservations.
    pub(crate) fn new_with_lifecycle_ordinals(
        adapter: SumeragiV2Adapter,
        startup_effects: Vec<AdapterEffect>,
        started_at: Instant,
        round_timeout: Duration,
        queue_config: RuntimeQueueConfig,
        lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), RuntimeConfigError> {
        Self::with_driver_and_lifecycle_ordinals(
            adapter,
            started_at,
            round_timeout,
            queue_config,
            startup_effects,
            lifecycle_ordinals,
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
    /// Busy-deferred aggregate certificate is cryptographically authenticated
    /// and then checked against canonical authority. Rejections do not poison
    /// the runtime. Once admitted, any adapter transition failure is fatal when
    /// the serialized command is executed.
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
        match ingress_ownership.earliest_lifecycle_ordinal() {
            Ok(Some(ordinal))
                if self
                    .ingress
                    .lifecycle_ordinals
                    .recognizes_minted(ordinal)
                    .unwrap_or(false) => {}
            Ok(None) => {}
            Ok(Some(_)) | Err(_) => {
                self.latch_fail_closed(
                    "network ingress carried an unminted actor-global lifecycle ordinal",
                );
                return Err(NetworkIngressError::FailClosed);
            }
        }
        // Registration is committed only after the authenticated command, or
        // its exact Busy-deferred owner, has retained this carrier. Keeping a
        // clone here avoids publishing a runtime terminal obligation for an
        // authentication or capacity rejection.
        let leader_wire_registration = ingress_ownership.clone();
        let default_class = classify_reducer_network_ingress(self.fail_closed, &message.payload)?;
        let deferred_owner = self.driver.deferred_authenticated_message_owner(&message);
        if let wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) = &message.payload {
            let projected_owner_tag = self
                .driver
                .deferred_quorum_certificate_owner_tag(certificate);
            if projected_owner_tag != deferred_owner.map(|(tag, _)| tag) {
                self.latch_fail_closed(
                    "deferred certificate owner projection disagreed with its exact owner",
                );
                return Err(NetworkIngressError::FailClosed);
            }
        }
        // An exact queued retransmission may always spend authentication work
        // so it can release its ingress occurrence. An exact Busy-deferred
        // aggregate certificate may likewise spend authentication work without
        // claiming a second queue slot. Otherwise, only the adapter's exact
        // active-lock match may proceed after the normal prefix fills.
        // Authentication below remains mandatory before either form of
        // coalescing.
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
                "network authentication changed deferred certificate ownership classification",
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
                Err(
                    RuntimeIngressMergeError::Conflict
                    | RuntimeIngressMergeError::IndependentOccurrence,
                ) => {
                    self.latch_fail_closed(
                        "deferred certificate admission lost authenticated ingress ownership",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
            }
            if self
                .register_leader_wire_runtime_receipt(&leader_wire_registration)
                .is_err()
            {
                self.latch_fail_closed(
                    "deferred certificate admission changed its leader-wire runtime receipt",
                );
                return Err(NetworkIngressError::FailClosed);
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
        let command = AdapterCommand::Authenticated(authenticated.clone());
        let preflight = self
            .command_admission_preflight(tag, class, &command)
            .map_err(NetworkIngressError::Backpressure)?;
        let preflight = self.reject_authenticated_preflight_coalescence(preflight)?;
        let restored_owner = match preflight {
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => Some((
                self.restored_command_owner(
                    tag,
                    class,
                    &command,
                    Some(&ingress_ownership),
                    causal_lifecycle_key,
                    admission_ordinal,
                )
                .map_err(NetworkIngressError::Backpressure)?,
                producer_stage,
            )),
            RuntimeCommandAdmissionPreflight::Admit => None,
            RuntimeCommandAdmissionPreflight::Coalesce => unreachable!("handled above"),
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        match self
            .ingress
            .enqueue_authenticated_with_ingress_ownership_and_owner(
                tag,
                class,
                authenticated,
                ingress_ownership,
                restored_owner
                    .as_ref()
                    .map(|(owner, producer_stage)| (owner, *producer_stage)),
            ) {
            Ok(owner) => {
                if self
                    .register_leader_wire_runtime_receipt(&leader_wire_registration)
                    .is_err()
                {
                    self.latch_fail_closed(
                        "authenticated admission changed its leader-wire runtime receipt",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
                Ok(owner)
            }
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
        if matches!(
            ownership.earliest_lifecycle_ordinal(),
            Ok(Some(ordinal))
                if !self
                    .ingress
                    .lifecycle_ordinals
                    .recognizes_minted(ordinal)
                    .unwrap_or(false)
        ) {
            // As with malformed ownership, let the mutating seam consume and
            // fail closed instead of pinning a corrupt fair-ingress head.
            return true;
        }
        if self.fail_closed {
            return false;
        }
        if let Some((round, _)) = self
            .driver
            .wire_ingress_missing_execution_commitment(&runtime_message.payload)
            && round.height == self.round_tag.height()
            && round.view == self.round_tag.view()
        {
            // The fair-ingress occurrence is the only durable process-local
            // owner at this boundary. Retain a current-view direct vote until
            // proposal validation binds its execution commitment. Proposal
            // and body traffic may arrive through independent source lanes
            // after the vote, and periodic retransmission remains best effort.
            // Fair ingress is bounded per source and bypasses blocked entries,
            // so an unknown subject cannot globally block later traffic. A
            // future-view vote has no certified local transition authority and
            // must drain normally; once the local view advances, an unmatched
            // current-view vote likewise drains for bounded rejection.
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

    /// Enqueue a completed local proposal without changing the immutable
    /// `AssembleBody` lifecycle owner minted when the proposal entered the
    /// asynchronous Store -> Validate pipeline.
    pub(crate) fn enqueue_local_proposal_with_owner(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            validated_receipt: validated_receipt.clone(),
        };
        self.enqueue_body_pipeline_completion_with_owner(
            tag,
            evidence,
            AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            },
            ownership,
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
        self.commit_body_available(reservation)
    }

    /// Reserve exact runtime ownership for a reconstructed body completion.
    ///
    /// Capacity and conflicting queued proposals are evaluated without
    /// exposing a reducer command. The returned token exclusively owns any
    /// claimed completion slot until committed or terminally retired. An
    /// executor abort retains this exact unpublished owner for retry.
    pub(crate) fn reserve_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let already_owned = self.body_pipeline_completion_is_owned(tag, &evidence)?;
        if already_owned && self.ingress.reserved_body_available.is_none() {
            return Ok(BodyAvailableReservation::coalesced(tag, manifest));
        }
        let command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let preflight =
            self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
        let restored_owner = match preflight {
            RuntimeCommandAdmissionPreflight::Coalesce => {
                return Ok(BodyAvailableReservation::coalesced(tag, manifest));
            }
            RuntimeCommandAdmissionPreflight::Admit => None,
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => Some((
                self.restored_command_owner(
                    tag,
                    CommandClass::Completion,
                    &command,
                    None,
                    causal_lifecycle_key,
                    admission_ordinal,
                )?,
                producer_stage,
            )),
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        let result = self.ingress.reserve_canonical_body_available_internal(
            tag,
            manifest,
            restored_owner.as_ref().map(|(owner, _)| owner),
            restored_owner
                .as_ref()
                .map(|(_, producer_stage)| *producer_stage),
        );
        if matches!(
            result,
            Err(EnqueueError::FailClosed | EnqueueError::DuplicateCompletionOwnership)
        ) {
            self.latch_fail_closed("body-available reservation ownership validation failed");
        }
        result
    }

    /// Reserve a reconstructed-body successor while retaining the FetchBody
    /// lifecycle owner.
    pub(crate) fn reserve_body_available_with_owner(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        if self.fail_closed || !ownership.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let already_owned = self.body_pipeline_completion_is_owned(tag, &evidence)?;
        if already_owned && self.ingress.reserved_body_available.is_none() {
            return Ok(BodyAvailableReservation::coalesced(tag, manifest));
        }
        let command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let preflight =
            self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
        let restored_owner = match preflight {
            RuntimeCommandAdmissionPreflight::Coalesce => {
                return Ok(BodyAvailableReservation::coalesced(tag, manifest));
            }
            RuntimeCommandAdmissionPreflight::Admit => None,
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => Some((
                self.restored_command_owner(
                    tag,
                    CommandClass::Completion,
                    &command,
                    None,
                    causal_lifecycle_key,
                    admission_ordinal,
                )?,
                producer_stage,
            )),
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        let owner = restored_owner
            .as_ref()
            .map_or_else(|| ownership.owner(), |(owner, _)| owner);
        let result = self.ingress.reserve_canonical_body_available_internal(
            tag,
            manifest,
            Some(owner),
            restored_owner
                .as_ref()
                .map(|(_, producer_stage)| *producer_stage),
        );
        if matches!(
            result,
            Err(EnqueueError::FailClosed | EnqueueError::DuplicateCompletionOwnership)
        ) {
            self.latch_fail_closed("owned body-available reservation validation failed");
        }
        result
    }

    /// Publish one previously reserved completion without another capacity
    /// check. A stale or mismatched token is an internal ownership violation
    /// and permanently closes the serialized runtime.
    pub(crate) fn commit_body_available(
        &mut self,
        reservation: BodyAvailableReservation,
    ) -> Result<(), EnqueueError> {
        let result = self.ingress.commit_canonical_body_available(reservation);
        if result.is_err() {
            self.latch_fail_closed("body-available reservation commit token did not match");
        }
        result
    }

    /// Retain an unpublished completion reservation after an all-or-error
    /// service transfer rejected the operation. The exact retry reclaims the
    /// same token and ordinal; this is not a terminal release. A stale or
    /// mismatched token is an intentional no-op because abort carries no
    /// authority to clear the retained owner.
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
        if self
            .reconcile_deferred_lifecycle_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("decided proposal retirement lost lifecycle ownership");
            return Err(
                "Sumeragi v2 deferred proposal retirement lost lifecycle ownership".to_owned(),
            );
        }
        if self.retire_orphaned_leader_wire_runtime_receipts().is_err() {
            self.latch_fail_closed("decided proposal retirement changed leader-wire ownership");
            return Err(
                "Sumeragi v2 decided proposal retirement changed leader-wire ownership".to_owned(),
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
        // Decision is the other terminal arm for the active-view producer.
        // The exact durable certificate already owns recovery/application, so
        // retaining a proposal fence here would resurrect work finality that
        // the retirement above has deliberately closed.
        self.active_view_producer = None;
        Ok(DecisionProposalRetirement::new(
            (expected.retainable() == 1).then_some(decision_tag),
            expected.recovery_only(),
        ))
    }

    /// Retire authenticated proposals which a newly installed lock makes unsafe.
    ///
    /// The exact locked subject may remain queued for unchanged reproposal.
    /// A competing subject survives only with the strictly higher matching
    /// PrepareQC required by the shared safe-value rule.
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
        if self
            .reconcile_deferred_lifecycle_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("unsafe proposal retirement lost lifecycle ownership");
            return Err(
                "Sumeragi v2 unsafe-proposal retirement lost lifecycle ownership".to_owned(),
            );
        }
        if self.retire_orphaned_leader_wire_runtime_receipts().is_err() {
            self.latch_fail_closed("unsafe proposal retirement changed leader-wire ownership");
            return Err(
                "Sumeragi v2 unsafe-proposal retirement changed leader-wire ownership".to_owned(),
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

    pub(crate) fn enqueue_body_stored_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::BodyStored {
            round,
            subject,
            receipt: receipt.clone(),
        };
        self.enqueue_body_pipeline_completion_with_owner(
            tag,
            evidence,
            AdapterCommand::BodyStored {
                round,
                subject,
                receipt,
            },
            ownership,
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

    pub(crate) fn enqueue_validation_succeeded_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::ValidationSucceeded {
            round,
            subject,
            receipt: receipt.clone(),
        };
        self.enqueue_body_pipeline_completion_with_owner(
            tag,
            evidence,
            AdapterCommand::ValidationSucceeded {
                round,
                subject,
                receipt,
            },
            ownership,
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

    pub(crate) fn enqueue_validation_failed_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::ValidationFailed { round, subject };
        self.enqueue_body_pipeline_completion_with_owner(
            tag,
            evidence,
            AdapterCommand::ValidationFailed { round, subject },
            ownership,
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
            let command = AdapterCommand::ValidationFailed { round, subject };
            let preflight =
                self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
            let tagged = match preflight {
                RuntimeCommandAdmissionPreflight::Coalesce => continue,
                RuntimeCommandAdmissionPreflight::Admit => {
                    TaggedCommand::new(tag, CommandClass::Completion, command, admitted_at)
                }
                RuntimeCommandAdmissionPreflight::ReuseDormant {
                    causal_lifecycle_key,
                    admission_ordinal,
                    producer_stage,
                } => self.restored_tagged_command(
                    tag,
                    CommandClass::Completion,
                    command,
                    admitted_at,
                    causal_lifecycle_key,
                    admission_ordinal,
                    producer_stage,
                )?,
                RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
            };
            commands.push(tagged);
        }
        let result = self.ingress.enqueue_completion_batch(commands);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("validation failure batch ownership validation failed");
        }
        result
    }

    /// Atomically enqueue validation rejections while preserving the exact
    /// lifecycle owner of every independently admitted validation task.
    pub(crate) fn enqueue_validation_failures_atomically_with_owners(
        &mut self,
        failures: &[(
            EventTag,
            wire::ConsensusRound,
            wire::BlockSubject,
            RuntimeEffectOwnership,
        )],
    ) -> Result<(), EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        let mut keys = BTreeSet::new();
        let mut commands = Vec::with_capacity(failures.len());
        let admitted_at = Instant::now();
        for (tag, round, subject, ownership) in failures {
            if !ownership.validate_exact() {
                self.latch_fail_closed(
                    "validation failure batch contained invalid lifecycle ownership",
                );
                return Err(EnqueueError::FailClosed);
            }
            if !keys.insert((*round, *subject)) {
                self.latch_fail_closed("validation failure batch contained duplicate body owners");
                return Err(EnqueueError::DuplicateCompletionOwnership);
            }
            let evidence = BodyPipelineCompletionEvidence::ValidationFailed {
                round: *round,
                subject: *subject,
            };
            if self.body_pipeline_completion_is_owned(*tag, &evidence)? {
                continue;
            }
            let command = AdapterCommand::ValidationFailed {
                round: *round,
                subject: *subject,
            };
            let preflight =
                self.command_admission_preflight(*tag, CommandClass::Completion, &command)?;
            if preflight == RuntimeCommandAdmissionPreflight::Coalesce {
                continue;
            }
            let restored_owner = match preflight {
                RuntimeCommandAdmissionPreflight::ReuseDormant {
                    causal_lifecycle_key,
                    admission_ordinal,
                    producer_stage,
                } => Some((
                    self.restored_command_owner(
                        *tag,
                        CommandClass::Completion,
                        &command,
                        None,
                        causal_lifecycle_key,
                        admission_ordinal,
                    )?,
                    producer_stage,
                )),
                RuntimeCommandAdmissionPreflight::Admit => None,
                RuntimeCommandAdmissionPreflight::Coalesce => unreachable!("handled above"),
                RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
            };
            let owner = restored_owner
                .as_ref()
                .map_or_else(|| ownership.owner(), |(owner, _)| owner);
            let mut tagged = TaggedCommand::with_causal_origin(
                *tag,
                CommandClass::Completion,
                command,
                admitted_at,
                owner.causal_origin().clone(),
                owner.lifecycle_ordinal(),
            )?;
            tagged.restored_producer_stage =
                restored_owner.map(|(_, producer_stage)| producer_stage);
            commands.push(tagged);
        }
        let result = self.ingress.enqueue_completion_batch(commands);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("owned validation failure batch validation failed");
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

    pub(crate) fn enqueue_signature_with_owner(
        &mut self,
        tag: EventTag,
        signature: Vec<u8>,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_with_lifecycle_owner(
            tag,
            CommandClass::Completion,
            AdapterCommand::SignatureCompleted(signature),
            ownership,
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

    pub(crate) fn enqueue_application_completed_with_owner(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_with_lifecycle_owner(
            tag,
            CommandClass::Completion,
            AdapterCommand::ApplicationCompleted(subject),
            ownership,
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
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => None,
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
    use iroha_p2p::network::{
        NetworkReplyRoute, NetworkReplyRouteError, NetworkReplyRouteTestFixture,
    };
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

    include!("tests/v2_runtime_unsealed_00.rs");
    include!("tests/v2_runtime_unsealed_01.rs");
    include!("tests/v2_runtime_unsealed_02.rs");
    include!("tests/v2_runtime_unsealed_03.rs");
    include!("tests/v2_runtime_unsealed_04.rs");
    include!("tests/v2_runtime_unsealed_05.rs");
    include!("tests/v2_runtime_unsealed_06.rs");
}
