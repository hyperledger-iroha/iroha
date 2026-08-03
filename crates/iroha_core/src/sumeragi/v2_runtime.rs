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
//! Every admitted owner freezes both its receiver-local physical predecessor
//! cut and its logical lifecycle ordinal. A replay admitted at or after that
//! cut cannot overtake the owner even when the replay retains an older logical
//! identity; logical minima govern only the finite pre-cut predecessor set.
//! Within the exact eligible set, a small deterministic arbiter and cyclic
//! class service prevent a saturated normal prefix from starving a locked
//! Commit vote or trusted local completion.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[cfg(test)]
use super::v2_core::check_production_effect_to_candidate_transition;
use super::v2_core::{
    CanonicalIdentityProjection, EFFECTIVE_LOCK_TRACE_SERVICE, EffectiveLockTraceProjection,
    EventTag, ExactBodyCompletionOwnership, IDENTITY_DOMAIN_PROCESS_LOCAL,
    IDENTITY_KIND_RUNTIME_CANDIDATE_SEMANTIC, IDENTITY_KIND_RUNTIME_CAUSAL_CANDIDATE,
    IDENTITY_KIND_RUNTIME_EFFECT, IDENTITY_KIND_RUNTIME_LIFECYCLE_OWNER,
    MAX_CAUSAL_SUCCESSORS_PER_COMMAND, MAX_EFFECTS_PER_STEP,
    ProductionEffectToCandidateTraceProjection, ProductionIngressIdentityAndClassTraceProjection,
    ProductionIngressReservationMaterializationTraceProjection, RUNTIME_CANDIDATE_KIND_APPLY,
    RUNTIME_CANDIDATE_KIND_FETCH_BODY, RUNTIME_CANDIDATE_KIND_NONE,
    RUNTIME_CANDIDATE_KIND_SIGN_PROPOSAL, RUNTIME_CANDIDATE_KIND_SIGN_TIMEOUT,
    RUNTIME_CANDIDATE_KIND_SIGN_VOTE, RUNTIME_CANDIDATE_KIND_STORE_BODY,
    RUNTIME_CANDIDATE_KIND_VALIDATE_BODY, RUNTIME_EFFECT_CAUSALITY_FRESH,
    RUNTIME_EFFECT_CAUSALITY_INHERIT, RUNTIME_EFFECT_KIND_APPLY, RUNTIME_EFFECT_KIND_BROADCAST,
    RUNTIME_EFFECT_KIND_ENTER_VIEW, RUNTIME_EFFECT_KIND_FETCH_BODY,
    RUNTIME_EFFECT_KIND_OPAQUE_TEST, RUNTIME_EFFECT_KIND_REPORT_EQUIVOCATION,
    RUNTIME_EFFECT_KIND_REPORT_INVALID_CERTIFIED_BODY, RUNTIME_EFFECT_KIND_SIGN_PROPOSAL,
    RUNTIME_EFFECT_KIND_SIGN_TIMEOUT, RUNTIME_EFFECT_KIND_SIGN_VOTE,
    RUNTIME_EFFECT_KIND_STORE_BODY, RUNTIME_EFFECT_KIND_VALIDATE_BODY, SERVICE_CLASS_COMPLETION,
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
        DecisionLocalProposalDisposition, DeferredAdmissionOrdinalSource,
        DeferredOccurrenceOwnershipEvidence, DeferredRuntimeOwnershipSeal, DeferredServiceEvidence,
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

/// Immutable receiver-local position of one runtime causal root.
///
/// This sidecar is local scheduling evidence. It is never serialized or
/// exposed on the wire. The cut is frozen by checked fair-ingress dequeue and
/// follows every causal successor of that occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeIngressPhysicalOwnership {
    source_ordinal: u64,
    physical_cut: u128,
}

impl RuntimeIngressPhysicalOwnership {
    fn validate_exact(self) -> bool {
        self.source_ordinal != 0 && u128::from(self.source_ordinal) < self.physical_cut
    }
}

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
        let mut saw_unbound_token = false;
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
                    None if saw_untagged || saw_unbound_token => {
                        return Err(RuntimeIngressMergeError::Conflict);
                    }
                    None => exact = Some(receipt),
                },
                (Some(_), None) if exact.is_some() || saw_untagged => {
                    return Err(RuntimeIngressMergeError::Conflict);
                }
                (Some(_), None) => saw_unbound_token = true,
                (None, None) if exact.is_some() || saw_unbound_token => {
                    return Err(RuntimeIngressMergeError::Conflict);
                }
                (None, None) => saw_untagged = true,
                (None, Some(_)) | (Some(_), Some(_)) => {
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

    /// Exact physical occurrence and dequeue-time predecessor cut for a
    /// productive leader-wire carrier.
    fn leader_wire_physical_carrier(
        &self,
    ) -> Result<Option<(u64, u128)>, RuntimeIngressMergeError> {
        let mut exact = None;
        for carrier in self
            .direct
            .iter()
            .chain(self.commit_certificate_response.iter())
        {
            if carrier.leader_wire_token().is_none() {
                continue;
            }
            let physical_ordinal = carrier
                .physical_admission_ordinal()
                .ok_or(RuntimeIngressMergeError::Conflict)?;
            let physical_cut = carrier
                .runtime_physical_cut()
                .ok_or(RuntimeIngressMergeError::Conflict)?;
            let candidate = (physical_ordinal, physical_cut);
            match exact {
                Some(retained) if retained != candidate => {
                    return Err(RuntimeIngressMergeError::Conflict);
                }
                Some(_) => {}
                None => exact = Some(candidate),
            }
        }
        Ok(exact)
    }

    /// Earliest receiver-local physical occurrence retained by this exact
    /// runtime command, paired with the cut frozen for that same carrier.
    /// Aggregate certificates may carry several independent authenticated
    /// deliveries; the first of them is the physical root of the one
    /// coalesced downstream continuation.
    fn earliest_physical_carrier(
        &self,
    ) -> Result<Option<RuntimeIngressPhysicalOwnership>, RuntimeIngressMergeError> {
        self.direct
            .iter()
            .chain(self.commit_certificate_response.iter())
            .try_fold(
                None::<RuntimeIngressPhysicalOwnership>,
                |earliest, carrier| {
                    let source_ordinal = carrier
                        .physical_admission_ordinal()
                        .ok_or(RuntimeIngressMergeError::Conflict)?;
                    let physical_cut = carrier
                        .runtime_physical_cut()
                        .ok_or(RuntimeIngressMergeError::Conflict)?;
                    let candidate = RuntimeIngressPhysicalOwnership {
                        source_ordinal,
                        physical_cut,
                    };
                    if !candidate.validate_exact() {
                        return Err(RuntimeIngressMergeError::Conflict);
                    }
                    Ok(Some(match earliest {
                        Some(current) if current.source_ordinal < candidate.source_ordinal => {
                            current
                        }
                        Some(current) if current.source_ordinal == candidate.source_ordinal => {
                            if current != candidate {
                                return Err(RuntimeIngressMergeError::Conflict);
                            }
                            current
                        }
                        Some(_) | None => candidate,
                    }))
                },
            )
    }

    fn contains_physical_carrier(
        &self,
        expected: RuntimeIngressPhysicalOwnership,
    ) -> Result<bool, RuntimeIngressMergeError> {
        if !expected.validate_exact() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        self.direct
            .iter()
            .chain(self.commit_certificate_response.iter())
            .try_fold(false, |found, carrier| {
                let candidate = RuntimeIngressPhysicalOwnership {
                    source_ordinal: carrier
                        .physical_admission_ordinal()
                        .ok_or(RuntimeIngressMergeError::Conflict)?,
                    physical_cut: carrier
                        .runtime_physical_cut()
                        .ok_or(RuntimeIngressMergeError::Conflict)?,
                };
                if !candidate.validate_exact() {
                    return Err(RuntimeIngressMergeError::Conflict);
                }
                Ok(found || candidate == expected)
            })
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

    /// Validate the physical occurrence/cut pair after checked fair-ingress
    /// dequeue has committed. The general identity validator deliberately
    /// permits an absent cut because the read-only capacity predicate runs
    /// before dequeue freezes it.
    fn validate_frozen_physical(&self) -> bool {
        self.validate_exact()
            && matches!(self.earliest_physical_carrier(), Ok(Some(_)))
            && matches!(
                (
                    self.leader_wire_token(),
                    self.leader_wire_physical_carrier(),
                    self.leader_wire_runtime_receipt()
                ),
                (Ok(None), Ok(None), Ok(None)) | (Ok(Some(_)), Ok(Some(_)), Ok(Some(_)))
            )
    }

    fn matches_authenticated(&self, authenticated: &AuthenticatedConsensusMessage) -> bool {
        self.validate_frozen_physical()
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
            (Ok(None), Ok(None)) | (Ok(Some(_)), Ok(None)) | (Ok(Some(_)), Ok(Some(_)))
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
    /// Physical occurrence and predecessor cut frozen with the root ingress
    /// carrier. Causal successors retain this pair even after the outer fair
    /// queue advances. Fresh local roots deliberately carry `None`.
    root_ingress_physical_ownership: Option<RuntimeIngressPhysicalOwnership>,
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
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-causal-origin:v2");
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
    match origin.root_ingress_physical_ownership {
        None => projection.push(0),
        Some(ownership) => {
            projection.push(1);
            append_runtime_identity_u64(&mut projection, ownership.source_ordinal);
            append_runtime_identity_field(&mut projection, &ownership.physical_cut.to_le_bytes());
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
            root_ingress_physical_ownership: ingress_ownership
                .and_then(|ownership| ownership.earliest_physical_carrier().ok().flatten()),
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
            root_ingress_physical_ownership: None,
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
        if admission_ordinal == 0
            || ingress_ownership.is_some_and(|ownership| !ownership.validate_frozen_physical())
        {
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
            && (self.root_ingress_identity.is_some()
                == self.root_ingress_physical_ownership.is_some())
            && self
                .root_ingress_physical_ownership
                .is_none_or(RuntimeIngressPhysicalOwnership::validate_exact)
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
    #[cfg(test)]
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

    /// Whether this causal lifecycle descends from receiver ingress admitted
    /// at or after `physical_cut`. Local successors keep the root pair even
    /// after they no longer carry the authenticated envelope itself.
    fn is_post_physical_cut(&self, physical_cut: u128) -> bool {
        self.causal_origin
            .root_ingress_physical_ownership
            .is_some_and(|ownership| u128::from(ownership.source_ordinal) >= physical_cut)
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

fn runtime_effect_causality_code(causality: RuntimeEffectCausality) -> u8 {
    match causality {
        RuntimeEffectCausality::Inherit => RUNTIME_EFFECT_CAUSALITY_INHERIT,
        RuntimeEffectCausality::Fresh(_) => RUNTIME_EFFECT_CAUSALITY_FRESH,
    }
}

fn runtime_effect_fresh_root_code(causality: RuntimeEffectCausality) -> u8 {
    match causality {
        RuntimeEffectCausality::Inherit => 0,
        RuntimeEffectCausality::Fresh(kind) => kind.code(),
    }
}

fn runtime_effect_identity_hash(effect_kind: u8, semantic_identity: &[u8]) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-effect:v1");
    projection.push(effect_kind);
    append_runtime_identity_field(&mut projection, semantic_identity);
    iroha_crypto::Hash::new(projection)
}

fn runtime_effect_candidate_semantic_hash(
    candidate_kind: u8,
    semantic_identity: &[u8],
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-candidate-semantic:v1");
    projection.push(candidate_kind);
    append_runtime_identity_field(&mut projection, semantic_identity);
    iroha_crypto::Hash::new(projection)
}

fn runtime_effect_candidate_identity_hash(
    owner: &RuntimeLifecycleOwner,
    candidate_kind: u8,
    semantic_identity: &iroha_crypto::Hash,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-causal-candidate:v1");
    append_runtime_identity_field(
        &mut projection,
        owner.causal_origin().lifecycle_key.as_ref(),
    );
    projection.push(candidate_kind);
    append_runtime_identity_field(&mut projection, semantic_identity.as_ref());
    iroha_crypto::Hash::new(projection)
}

/// Complete route-neutral statement retained by one internal candidate.
///
/// This is process-local refinement evidence. It is never serialized, and it
/// deliberately remains separate from the concrete effect identity so route
/// or aggregate-carrier changes cannot rewrite the abstract lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeCandidateSemanticStatement {
    context_id: wire::HeightContextId,
    round: wire::ConsensusRound,
    proposal_round: wire::ConsensusRound,
    subject: Option<wire::BlockSubject>,
    phase: Option<wire::GlobalPhase>,
    execution_commitment: Option<wire::ExecutionCommitment>,
}

impl RuntimeCandidateSemanticStatement {
    fn new(
        round: wire::ConsensusRound,
        proposal_round: wire::ConsensusRound,
        subject: Option<wire::BlockSubject>,
        phase: Option<wire::GlobalPhase>,
        execution_commitment: Option<wire::ExecutionCommitment>,
    ) -> Self {
        Self {
            context_id: round.context_id,
            round,
            proposal_round,
            subject,
            phase,
            execution_commitment,
        }
    }

    fn validate_exact(self) -> bool {
        self.context_id == self.round.context_id
            && self.context_id == self.proposal_round.context_id
            && self.round.height == self.proposal_round.height
            && self.phase.is_some() == self.execution_commitment.is_some()
            && self.phase.is_none_or(|_| self.subject.is_some())
            && self
                .execution_commitment
                .is_none_or(|_| self.subject.is_some())
    }

    /// Classify the only authority refinement allowed within an immutable
    /// body owner. A local/ordinary body starts without quorum authority, and
    /// a Prepare-certified body can later acquire the exact durable CommitQC.
    /// Once Commit authority is installed, every coordinate is immutable.
    fn commit_refinement_to(self, successor: Self) -> Option<RuntimeCandidateAuthorityRefinement> {
        if !self.validate_exact()
            || !successor.validate_exact()
            || successor.phase != Some(wire::GlobalPhase::Commit)
            || self.context_id != successor.context_id
            || self.round != successor.round
            || self.proposal_round != successor.proposal_round
            || self.subject != successor.subject
        {
            return None;
        }
        match (self.phase, self.execution_commitment) {
            (None, None) => Some(RuntimeCandidateAuthorityRefinement::AcquireCommit),
            (Some(wire::GlobalPhase::Prepare), Some(commitment))
                if successor.execution_commitment == Some(commitment) =>
            {
                Some(RuntimeCandidateAuthorityRefinement::PromotePrepare)
            }
            (Some(wire::GlobalPhase::Commit), Some(_)) if self == successor => {
                Some(RuntimeCandidateAuthorityRefinement::RetainCommit)
            }
            _ => None,
        }
    }

    fn semantic_identity(self) -> Vec<u8> {
        let mut identity = Vec::new();
        identity.extend_from_slice(b"iroha:sumeragi:v2:tla-candidate-semantic:v2");
        append_runtime_identity_field(&mut identity, &self.context_id.encode());
        append_runtime_identity_field(&mut identity, &self.round.encode());
        append_runtime_identity_field(&mut identity, &self.proposal_round.encode());
        append_optional_runtime_identity_bytes(
            &mut identity,
            self.subject.map(|subject| subject.encode()),
        );
        append_optional_runtime_identity_bytes(
            &mut identity,
            self.phase.map(|phase| phase.encode()),
        );
        append_optional_runtime_identity_bytes(
            &mut identity,
            self.execution_commitment
                .map(|commitment| commitment.encode()),
        );
        identity
    }
}

/// Provenance-sensitive authority transition under one immutable local owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeCandidateAuthorityRefinement {
    /// A validated local or ordinary body acquired its first quorum authority.
    AcquireCommit,
    /// A Prepare-certified body acquired the matching durable CommitQC.
    PromotePrepare,
    /// An exact Commit-authorized retry retained all six coordinates.
    RetainCommit,
}

/// Candidate bytes plus the independently retained typed statement which
/// produced them. Synthetic runtime drivers may omit the production-only
/// statement; every production candidate kind must carry it.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeEffectCandidateSemantic {
    kind: u8,
    semantic_identity: Vec<u8>,
    statement: Option<RuntimeCandidateSemanticStatement>,
}

fn runtime_candidate_kind_requires_statement(kind: u8) -> bool {
    matches!(
        kind,
        RUNTIME_CANDIDATE_KIND_SIGN_PROPOSAL
            | RUNTIME_CANDIDATE_KIND_SIGN_VOTE
            | RUNTIME_CANDIDATE_KIND_SIGN_TIMEOUT
            | RUNTIME_CANDIDATE_KIND_FETCH_BODY
            | RUNTIME_CANDIDATE_KIND_STORE_BODY
            | RUNTIME_CANDIDATE_KIND_VALIDATE_BODY
            | RUNTIME_CANDIDATE_KIND_APPLY
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeEffectCandidateBinding {
    owner_projection_hash: iroha_crypto::Hash,
    parent_owner_projection_hash: Option<iroha_crypto::Hash>,
    effect_kind: u8,
    effect_identity: iroha_crypto::Hash,
    candidate_kind: u8,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    candidate_semantic_identity: Option<iroha_crypto::Hash>,
    candidate_identity: Option<iroha_crypto::Hash>,
    effect_position: u8,
    effect_count: u8,
    candidate_position: u8,
    candidate_count: u8,
    projection_hash: iroha_crypto::Hash,
}

fn append_optional_runtime_hash(projection: &mut Vec<u8>, value: Option<&iroha_crypto::Hash>) {
    match value {
        None => projection.push(0),
        Some(value) => {
            projection.push(1);
            append_runtime_identity_field(projection, value.as_ref());
        }
    }
}

fn runtime_effect_candidate_binding_projection_hash(
    owner: &RuntimeLifecycleOwner,
    causality: RuntimeEffectCausality,
    binding: &RuntimeEffectCandidateBinding,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-effect-binding:v1");
    append_runtime_identity_field(&mut projection, owner.projection_hash.as_ref());
    projection.push(runtime_effect_causality_code(causality));
    projection.push(runtime_effect_fresh_root_code(causality));
    append_runtime_identity_field(&mut projection, binding.owner_projection_hash.as_ref());
    append_optional_runtime_hash(
        &mut projection,
        binding.parent_owner_projection_hash.as_ref(),
    );
    projection.push(binding.effect_kind);
    append_runtime_identity_field(&mut projection, binding.effect_identity.as_ref());
    projection.push(binding.candidate_kind);
    match binding.candidate_statement {
        None => projection.push(0),
        Some(statement) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, &statement.semantic_identity());
        }
    }
    append_optional_runtime_hash(
        &mut projection,
        binding.candidate_semantic_identity.as_ref(),
    );
    append_optional_runtime_hash(&mut projection, binding.candidate_identity.as_ref());
    projection.push(binding.effect_position);
    projection.push(binding.effect_count);
    projection.push(binding.candidate_position);
    projection.push(binding.candidate_count);
    iroha_crypto::Hash::new(projection)
}

impl RuntimeEffectCandidateBinding {
    #[allow(clippy::too_many_arguments)]
    fn new(
        owner: &RuntimeLifecycleOwner,
        causality: RuntimeEffectCausality,
        parent: Option<&RuntimeLifecycleOwner>,
        effect_kind: u8,
        effect_semantic_identity: &[u8],
        candidate: Option<&RuntimeEffectCandidateSemantic>,
        effect_position: u8,
        effect_count: u8,
        candidate_position: u8,
        candidate_count: u8,
    ) -> Result<Self, EnqueueError> {
        let exact_parent = match causality {
            RuntimeEffectCausality::Inherit => parent == Some(owner),
            RuntimeEffectCausality::Fresh(_) => parent.is_none(),
        };
        if !owner.validate_exact()
            || !exact_parent
            || effect_kind == 0
            || effect_count == 0
            || usize::from(effect_count) > MAX_EFFECTS_PER_STEP
            || effect_position == 0
            || effect_position > effect_count
            || usize::from(candidate_count) > MAX_CAUSAL_SUCCESSORS_PER_COMMAND
        {
            return Err(EnqueueError::FailClosed);
        }
        let effect_identity = runtime_effect_identity_hash(effect_kind, effect_semantic_identity);
        let (candidate_kind, candidate_statement, candidate_semantic_identity, candidate_identity) =
            match candidate {
                None if candidate_position == 0 => (RUNTIME_CANDIDATE_KIND_NONE, None, None, None),
                Some(candidate)
                    if candidate.kind != RUNTIME_CANDIDATE_KIND_NONE
                        && candidate_count != 0
                        && candidate_position != 0
                        && candidate_position <= candidate_count
                        && candidate.statement.is_none_or(|statement| {
                            statement.validate_exact()
                                && statement.semantic_identity().as_slice()
                                    == candidate.semantic_identity.as_slice()
                        })
                        && (!runtime_candidate_kind_requires_statement(candidate.kind)
                            || candidate.statement.is_some()) =>
                {
                    let semantic_identity = runtime_effect_candidate_semantic_hash(
                        candidate.kind,
                        &candidate.semantic_identity,
                    );
                    let candidate_identity = runtime_effect_candidate_identity_hash(
                        owner,
                        candidate.kind,
                        &semantic_identity,
                    );
                    (
                        candidate.kind,
                        candidate.statement,
                        Some(semantic_identity),
                        Some(candidate_identity),
                    )
                }
                _ => return Err(EnqueueError::FailClosed),
            };
        let parent_owner_projection_hash = parent.map(|owner| owner.projection_hash);
        let mut binding = Self {
            owner_projection_hash: owner.projection_hash,
            parent_owner_projection_hash,
            effect_kind,
            effect_identity,
            candidate_kind,
            candidate_statement,
            candidate_semantic_identity,
            candidate_identity,
            effect_position,
            effect_count,
            candidate_position,
            candidate_count,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        binding.projection_hash =
            runtime_effect_candidate_binding_projection_hash(owner, causality, &binding);
        binding
            .validate_exact(owner, causality)
            .then_some(binding)
            .ok_or(EnqueueError::FailClosed)
    }

    fn validate_exact(
        &self,
        owner: &RuntimeLifecycleOwner,
        causality: RuntimeEffectCausality,
    ) -> bool {
        let exact_parent = match causality {
            RuntimeEffectCausality::Inherit => {
                self.parent_owner_projection_hash == Some(owner.projection_hash)
            }
            RuntimeEffectCausality::Fresh(_) => self.parent_owner_projection_hash.is_none(),
        };
        let exact_candidate = match (
            self.candidate_kind,
            self.candidate_statement,
            self.candidate_semantic_identity.as_ref(),
            self.candidate_identity.as_ref(),
        ) {
            (RUNTIME_CANDIDATE_KIND_NONE, None, None, None) => self.candidate_position == 0,
            (
                candidate_kind,
                candidate_statement,
                Some(semantic_identity),
                Some(candidate_identity),
            ) => {
                candidate_kind != RUNTIME_CANDIDATE_KIND_NONE
                    && self.candidate_count != 0
                    && self.candidate_position != 0
                    && self.candidate_position <= self.candidate_count
                    && (!runtime_candidate_kind_requires_statement(candidate_kind)
                        || candidate_statement.is_some())
                    && candidate_statement.is_none_or(|statement| {
                        statement.validate_exact()
                            && *semantic_identity
                                == runtime_effect_candidate_semantic_hash(
                                    candidate_kind,
                                    &statement.semantic_identity(),
                                )
                    })
                    && *candidate_identity
                        == runtime_effect_candidate_identity_hash(
                            owner,
                            candidate_kind,
                            semantic_identity,
                        )
            }
            _ => false,
        };
        owner.validate_exact()
            && self.owner_projection_hash == owner.projection_hash
            && exact_parent
            && self.effect_kind != 0
            && self.effect_count != 0
            && usize::from(self.effect_count) <= MAX_EFFECTS_PER_STEP
            && self.effect_position != 0
            && self.effect_position <= self.effect_count
            && usize::from(self.candidate_count) <= MAX_CAUSAL_SUCCESSORS_PER_COMMAND
            && exact_candidate
            && self.projection_hash
                == runtime_effect_candidate_binding_projection_hash(owner, causality, self)
    }
}

/// Sidecar metadata paired positionally with one reducer effect.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeEffectOwnership {
    owner: RuntimeLifecycleOwner,
    causality: RuntimeEffectCausality,
    binding: Option<RuntimeEffectCandidateBinding>,
}

impl PartialEq for RuntimeEffectOwnership {
    // Equality names the immutable lifecycle owner, not the replaceable
    // positional effect binding. Fetch-route upgrades and later consumer-stage
    // rebinds must retain this equality while recomputing and validating the
    // complete binding before the next asynchronous owner is published.
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner && self.causality == other.causality
    }
}

impl Eq for RuntimeEffectOwnership {}

impl RuntimeEffectOwnership {
    fn inherited(owner: RuntimeLifecycleOwner) -> Self {
        Self {
            owner,
            causality: RuntimeEffectCausality::Inherit,
            binding: None,
        }
    }

    fn fresh(owner: RuntimeLifecycleOwner, kind: RuntimeFreshRootKind) -> Self {
        Self {
            owner,
            causality: RuntimeEffectCausality::Fresh(kind),
            binding: None,
        }
    }

    fn validate_exact(&self) -> bool {
        self.owner.validate_exact()
            && self
                .binding
                .as_ref()
                .is_none_or(|binding| binding.validate_exact(&self.owner, self.causality))
    }

    fn validate_bound_exact(&self) -> bool {
        self.validate_exact() && self.binding.is_some()
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_runtime_effect(
        mut self,
        parent: Option<&RuntimeLifecycleOwner>,
        effect_kind: u8,
        effect_semantic_identity: &[u8],
        candidate: Option<&RuntimeEffectCandidateSemantic>,
        effect_position: u8,
        effect_count: u8,
        candidate_position: u8,
        candidate_count: u8,
    ) -> Result<Self, EnqueueError> {
        if self.binding.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        self.binding = Some(RuntimeEffectCandidateBinding::new(
            &self.owner,
            self.causality,
            parent,
            effect_kind,
            effect_semantic_identity,
            candidate,
            effect_position,
            effect_count,
            candidate_position,
            candidate_count,
        )?);
        self.validate_bound_exact()
            .then_some(self)
            .ok_or(EnqueueError::FailClosed)
    }

    fn binding(&self) -> Option<&RuntimeEffectCandidateBinding> {
        self.binding.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn candidate_identity(&self) -> Option<iroha_crypto::Hash> {
        self.binding
            .as_ref()
            .and_then(|binding| binding.candidate_identity)
    }

    /// Route-neutral semantic lifecycle used by the single-owner admission gate.
    pub(crate) fn candidate_semantic_identity(&self) -> Option<iroha_crypto::Hash> {
        self.binding
            .as_ref()
            .and_then(|binding| binding.candidate_semantic_identity)
    }

    /// Typed semantic statement retained through causal completion handoffs.
    fn candidate_semantic_statement(&self) -> Option<RuntimeCandidateSemanticStatement> {
        self.binding
            .as_ref()
            .and_then(|binding| binding.candidate_statement)
    }

    /// Immutable owner carried into an asynchronous task or completion.
    pub(crate) const fn owner(&self) -> &RuntimeLifecycleOwner {
        &self.owner
    }

    /// Frozen inherit/fresh classification. Retries and rebinds retain it.
    #[cfg_attr(not(test), allow(dead_code))]
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

fn append_optional_runtime_identity_bytes(identity: &mut Vec<u8>, value: Option<Vec<u8>>) {
    match value {
        None => identity.push(0),
        Some(value) => {
            identity.push(1);
            append_runtime_identity_field(identity, &value);
        }
    }
}

/// Closed classification of every production adapter effect.
pub(crate) const fn production_adapter_effect_kind(effect: &AdapterEffect) -> u8 {
    match effect {
        AdapterEffect::Sign {
            request: super::v2::SignRequest::Proposal(_),
            ..
        } => RUNTIME_EFFECT_KIND_SIGN_PROPOSAL,
        AdapterEffect::Sign {
            request: super::v2::SignRequest::Vote(_),
            ..
        } => RUNTIME_EFFECT_KIND_SIGN_VOTE,
        AdapterEffect::Sign {
            request: super::v2::SignRequest::TimeoutVote(_),
            ..
        } => RUNTIME_EFFECT_KIND_SIGN_TIMEOUT,
        AdapterEffect::FetchBody { .. } => RUNTIME_EFFECT_KIND_FETCH_BODY,
        AdapterEffect::StoreBody { .. } => RUNTIME_EFFECT_KIND_STORE_BODY,
        AdapterEffect::ValidateBody { .. } => RUNTIME_EFFECT_KIND_VALIDATE_BODY,
        AdapterEffect::Apply { .. } => RUNTIME_EFFECT_KIND_APPLY,
        AdapterEffect::Broadcast(_) => RUNTIME_EFFECT_KIND_BROADCAST,
        AdapterEffect::EnterView { .. } => RUNTIME_EFFECT_KIND_ENTER_VIEW,
        AdapterEffect::ReportEquivocation { .. } => RUNTIME_EFFECT_KIND_REPORT_EQUIVOCATION,
        AdapterEffect::ReportInvalidCertifiedBody { .. } => {
            RUNTIME_EFFECT_KIND_REPORT_INVALID_CERTIFIED_BODY
        }
    }
}

/// Canonical exact identity bytes for every field of one production effect.
///
/// These bytes are internal evidence only. They are never serialized as a wire
/// field and do not introduce a runtime configuration surface.
pub(crate) fn production_adapter_effect_semantic_identity(effect: &AdapterEffect) -> Vec<u8> {
    let mut identity = Vec::new();
    identity.extend_from_slice(b"iroha:sumeragi:v2:adapter-effect-semantic:v1");
    identity.push(production_adapter_effect_kind(effect));
    match effect {
        AdapterEffect::Sign { tag, request } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &request.signature_preimage());
        }
        AdapterEffect::Broadcast(message) => {
            append_runtime_identity_field(&mut identity, &message.encode());
        }
        AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest,
            certified_sources,
            certificate,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &round.encode());
            append_runtime_identity_field(&mut identity, &subject.encode());
            append_optional_runtime_identity_bytes(
                &mut identity,
                manifest.as_ref().map(norito::codec::Encode::encode),
            );
            append_runtime_identity_field(&mut identity, &certified_sources.encode());
            append_optional_runtime_identity_bytes(
                &mut identity,
                certificate.as_ref().map(norito::codec::Encode::encode),
            );
        }
        AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        }
        | AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &round.encode());
            append_runtime_identity_field(&mut identity, &subject.encode());
        }
        AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &subject.encode());
            append_runtime_identity_field(&mut identity, &certificate.encode());
        }
        AdapterEffect::EnterView {
            tag,
            certificate,
            protected_body,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &certificate.encode());
            match protected_body {
                None => identity.push(0),
                Some((round, subject)) => {
                    identity.push(1);
                    append_runtime_identity_field(&mut identity, &round.encode());
                    append_runtime_identity_field(&mut identity, &subject.encode());
                }
            }
        }
        AdapterEffect::ReportEquivocation {
            offender,
            round,
            kind,
        } => {
            append_runtime_identity_field(&mut identity, &offender.encode());
            append_runtime_identity_field(&mut identity, &round.encode());
            identity.push(match kind {
                super::v2_core::EquivocationKind::Vote => 1,
                super::v2_core::EquivocationKind::Timeout => 2,
                super::v2_core::EquivocationKind::Proposal => 3,
            });
        }
        AdapterEffect::ReportInvalidCertifiedBody {
            subject,
            certificate,
        } => {
            append_runtime_identity_field(&mut identity, &subject.encode());
            append_runtime_identity_field(&mut identity, &certificate.encode());
        }
    }
    identity
}

fn production_adapter_effect_candidate_statement(
    effect: &AdapterEffect,
) -> Option<(u8, RuntimeCandidateSemanticStatement)> {
    Some(match effect {
        AdapterEffect::Sign {
            request: super::v2::SignRequest::Proposal(proposal),
            ..
        } => (
            RUNTIME_CANDIDATE_KIND_SIGN_PROPOSAL,
            RuntimeCandidateSemanticStatement::new(
                proposal.round,
                proposal.round,
                Some(proposal.subject),
                None,
                None,
            ),
        ),
        AdapterEffect::Sign {
            request: super::v2::SignRequest::Vote(vote),
            ..
        } => (
            RUNTIME_CANDIDATE_KIND_SIGN_VOTE,
            RuntimeCandidateSemanticStatement::new(
                vote.round,
                vote.proposal_round,
                Some(vote.subject),
                Some(vote.phase),
                Some(vote.execution_commitment),
            ),
        ),
        AdapterEffect::Sign {
            request: super::v2::SignRequest::TimeoutVote(vote),
            ..
        } => {
            let highest = vote.highest_prepare_qc.as_ref();
            (
                RUNTIME_CANDIDATE_KIND_SIGN_TIMEOUT,
                RuntimeCandidateSemanticStatement::new(
                    vote.round,
                    highest.map_or(vote.round, |certificate| certificate.proposal_round),
                    highest.map(|certificate| certificate.subject),
                    highest.map(|certificate| certificate.phase),
                    highest.map(|certificate| certificate.execution_commitment),
                ),
            )
        }
        AdapterEffect::FetchBody {
            round,
            subject,
            certificate,
            ..
        } => (
            RUNTIME_CANDIDATE_KIND_FETCH_BODY,
            RuntimeCandidateSemanticStatement::new(
                certificate
                    .as_ref()
                    .map_or(*round, |certificate| certificate.round),
                certificate
                    .as_ref()
                    .map_or(*round, |certificate| certificate.proposal_round),
                Some(*subject),
                certificate.as_ref().map(|certificate| certificate.phase),
                certificate
                    .as_ref()
                    .map(|certificate| certificate.execution_commitment),
            ),
        ),
        AdapterEffect::StoreBody { round, subject, .. } => (
            RUNTIME_CANDIDATE_KIND_STORE_BODY,
            RuntimeCandidateSemanticStatement::new(*round, *round, Some(*subject), None, None),
        ),
        AdapterEffect::ValidateBody { round, subject, .. } => (
            RUNTIME_CANDIDATE_KIND_VALIDATE_BODY,
            RuntimeCandidateSemanticStatement::new(*round, *round, Some(*subject), None, None),
        ),
        AdapterEffect::Apply {
            subject,
            certificate,
            ..
        } => (
            RUNTIME_CANDIDATE_KIND_APPLY,
            RuntimeCandidateSemanticStatement::new(
                certificate.round,
                certificate.proposal_round,
                Some(*subject),
                Some(certificate.phase),
                Some(certificate.execution_commitment),
            ),
        ),
        AdapterEffect::Broadcast(_)
        | AdapterEffect::EnterView { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => return None,
    })
}

/// Bind one production candidate, optionally inheriting the exact body-stage
/// statement carried by its causal parent.
fn production_adapter_effect_candidate_binding(
    effect: &AdapterEffect,
    inherited: Option<&RuntimeCandidateSemanticStatement>,
) -> Result<Option<RuntimeEffectCandidateSemantic>, String> {
    match effect {
        AdapterEffect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if certificate.proposal_round != *round || certificate.subject != *subject => {
            return Err(
                "Sumeragi v2 certified Fetch disagreed with its proposal-round body key".to_owned(),
            );
        }
        AdapterEffect::Apply {
            subject,
            certificate,
            ..
        } if certificate.phase != wire::GlobalPhase::Commit || certificate.subject != *subject => {
            return Err("Sumeragi v2 Apply omitted its exact Commit authority".to_owned());
        }
        _ => {}
    }
    let Some((kind, mut statement)) = production_adapter_effect_candidate_statement(effect) else {
        return Ok(None);
    };
    if !statement.validate_exact() {
        return Err(
            "Sumeragi v2 candidate statement had inconsistent context or height".to_owned(),
        );
    }
    if let Some(parent) = inherited {
        if !parent.validate_exact() {
            return Err("Sumeragi v2 causal parent lost its exact candidate statement".to_owned());
        }
        match effect {
            AdapterEffect::StoreBody { round, subject, .. }
            | AdapterEffect::ValidateBody { round, subject, .. } => {
                if parent.context_id != round.context_id
                    || parent.proposal_round != *round
                    || parent.subject != Some(*subject)
                {
                    return Err(
                        "Sumeragi v2 body successor changed its frozen candidate statement"
                            .to_owned(),
                    );
                }
                // Store and Validate effects intentionally carry only their
                // concrete body key. Their abstract phase and execution
                // commitment remain the exact values frozen by Fetch.
                statement = *parent;
            }
            AdapterEffect::Apply { .. } => {
                if parent.commit_refinement_to(statement).is_none() {
                    return Err(
                        "Sumeragi v2 Apply changed its inherited candidate authority".to_owned(),
                    );
                }
            }
            AdapterEffect::Sign { .. }
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::Broadcast(_)
            | AdapterEffect::EnterView { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => {}
        }
    }
    let semantic_identity = statement.semantic_identity();
    Ok(Some(RuntimeEffectCandidateSemantic {
        kind,
        semantic_identity,
        statement: Some(statement),
    }))
}

/// Route-neutral TLA work kind and semantic payload for candidate-producing
/// effects.
///
/// The normalized payload is exactly the frozen context, round, proposal
/// round, optional subject, optional consensus phase, and optional execution
/// commitment. The candidate kind separately fixes the durable local stage.
/// Signer carriers, aggregate signatures, routes, manifests, and the
/// process-local reducer incarnation remain only in concrete effect identity.
pub(crate) fn production_adapter_effect_candidate_semantic_identity(
    effect: &AdapterEffect,
) -> Option<(u8, Vec<u8>)> {
    let (kind, statement) = production_adapter_effect_candidate_statement(effect)?;
    statement
        .validate_exact()
        .then(|| (kind, statement.semantic_identity()))
}

/// Exact non-serialized ownership outcome for one candidate gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeCandidateAdmissionDisposition {
    /// The semantic lifecycle atomically acquired its sole owner slot (`0 -> 1`).
    FirstAdmission,
    /// An exact retry retained the incumbent owner (`1 -> 1`).
    CoalescedRetry,
    /// The adapter effect creates no TLA candidate owner (`0 -> 0`).
    NonCandidate,
}

impl RuntimeCandidateAdmissionDisposition {
    const fn owner_admitted(self) -> bool {
        matches!(self, Self::FirstAdmission)
    }
}

/// Classify only the three exact owner-count transitions admitted by the model.
pub(crate) fn production_adapter_effect_candidate_admission_disposition(
    effect: &AdapterEffect,
    candidate_owner_count_before: u8,
    candidate_owner_count_after: u8,
) -> Result<RuntimeCandidateAdmissionDisposition, String> {
    match (
        production_adapter_effect_candidate_semantic_identity(effect).is_some(),
        candidate_owner_count_before,
        candidate_owner_count_after,
    ) {
        (true, 0, 1) => Ok(RuntimeCandidateAdmissionDisposition::FirstAdmission),
        (true, 1, 1) => Ok(RuntimeCandidateAdmissionDisposition::CoalescedRetry),
        (false, 0, 0) => Ok(RuntimeCandidateAdmissionDisposition::NonCandidate),
        _ => Err(
            "Sumeragi v2 candidate admission used a non-exact owner-count transition".to_owned(),
        ),
    }
}

fn runtime_identity_projection(
    kind: u8,
    identity: &iroha_crypto::Hash,
) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(IDENTITY_DOMAIN_PROCESS_LOCAL, kind, *identity.as_ref())
}

fn optional_runtime_identity_projection(
    kind: u8,
    identity: Option<&iroha_crypto::Hash>,
) -> CanonicalIdentityProjection {
    identity.map_or_else(CanonicalIdentityProjection::zero, |identity| {
        runtime_identity_projection(kind, identity)
    })
}

/// Bind a complete test/executor effect batch to exact positional candidate
/// identities. Production runtime drivers use the same low-level constructor.
pub(crate) fn bind_adapter_effect_batch_ownership(
    effects: &[AdapterEffect],
    ownership: Vec<RuntimeEffectOwnership>,
) -> Result<Vec<RuntimeEffectOwnership>, String> {
    if effects.is_empty()
        || effects.len() != ownership.len()
        || effects.len() > MAX_EFFECTS_PER_STEP
    {
        return Err("Sumeragi v2 effect batch cannot be bound positionally".to_owned());
    }
    let effect_count = u8::try_from(effects.len())
        .map_err(|_| "Sumeragi v2 effect count is not representable".to_owned())?;
    let candidate_count_usize = effects
        .iter()
        .filter(|effect| production_adapter_effect_candidate_semantic_identity(effect).is_some())
        .count();
    if candidate_count_usize > MAX_CAUSAL_SUCCESSORS_PER_COMMAND {
        return Err("Sumeragi v2 effect batch exceeded the causal-successor bound".to_owned());
    }
    let candidate_count = u8::try_from(candidate_count_usize)
        .map_err(|_| "Sumeragi v2 candidate count is not representable".to_owned())?;
    let mut candidate_position = 0u8;
    effects
        .iter()
        .zip(ownership)
        .enumerate()
        .map(|(index, (effect, ownership))| {
            let effect_position = u8::try_from(index + 1)
                .map_err(|_| "Sumeragi v2 effect position is not representable".to_owned())?;
            let effect_semantic_identity = production_adapter_effect_semantic_identity(effect);
            let candidate = production_adapter_effect_candidate_binding(effect, None)?;
            if candidate.is_some() {
                candidate_position = candidate_position
                    .checked_add(1)
                    .ok_or_else(|| "Sumeragi v2 candidate position overflowed".to_owned())?;
            }
            let ownership = match ownership.causality() {
                RuntimeEffectCausality::Inherit => {
                    RuntimeEffectOwnership::inherited(ownership.owner().clone())
                }
                RuntimeEffectCausality::Fresh(kind) => {
                    RuntimeEffectOwnership::fresh(ownership.owner().clone(), kind)
                }
            };
            let parent = matches!(ownership.causality(), RuntimeEffectCausality::Inherit)
                .then(|| ownership.owner().clone());
            ownership
                .bind_runtime_effect(
                    parent.as_ref(),
                    production_adapter_effect_kind(effect),
                    &effect_semantic_identity,
                    candidate.as_ref(),
                    effect_position,
                    effect_count,
                    candidate.as_ref().map_or(0, |_| candidate_position),
                    candidate_count,
                )
                .map_err(|_| "Sumeragi v2 effect binding failed closed".to_owned())
        })
        .collect()
}

impl RuntimeEffectOwnership {
    /// Replace the positional binding while retaining the same lifecycle root.
    /// This is used only when one completed local async stage atomically creates
    /// its next exact stage without returning through the serialized reducer.
    pub(crate) fn rebind_as_inherited_adapter_effect(
        &self,
        effect: &AdapterEffect,
    ) -> Result<Self, String> {
        let inherited = self.candidate_semantic_statement();
        let candidate = production_adapter_effect_candidate_binding(effect, inherited.as_ref())?;
        let candidate_count = u8::from(candidate.is_some());
        RuntimeEffectOwnership::inherited(self.owner.clone())
            .bind_runtime_effect(
                Some(&self.owner),
                production_adapter_effect_kind(effect),
                &production_adapter_effect_semantic_identity(effect),
                candidate.as_ref(),
                1,
                1,
                candidate_count,
                candidate_count,
            )
            .map_err(|_| "Sumeragi v2 successor effect binding failed closed".to_owned())
    }

    pub(crate) fn rebind_same_adapter_effect(
        &self,
        effect: &AdapterEffect,
    ) -> Result<Self, String> {
        let inherited = self.candidate_semantic_statement();
        let candidate = production_adapter_effect_candidate_binding(effect, inherited.as_ref())?;
        let candidate_count = u8::from(candidate.is_some());
        let ownership = match self.causality {
            RuntimeEffectCausality::Inherit => {
                RuntimeEffectOwnership::inherited(self.owner.clone())
            }
            RuntimeEffectCausality::Fresh(kind) => {
                RuntimeEffectOwnership::fresh(self.owner.clone(), kind)
            }
        };
        let parent = matches!(ownership.causality, RuntimeEffectCausality::Inherit)
            .then(|| ownership.owner.clone());
        ownership
            .bind_runtime_effect(
                parent.as_ref(),
                production_adapter_effect_kind(effect),
                &production_adapter_effect_semantic_identity(effect),
                candidate.as_ref(),
                1,
                1,
                candidate_count,
                candidate_count,
            )
            .map_err(|_| "Sumeragi v2 exact effect rebind failed closed".to_owned())
    }
}

/// Recompute one total effect/candidate gate projection from concrete effect
/// bytes and the independently retained runtime binding.
#[allow(clippy::too_many_arguments)]
pub(crate) fn production_adapter_effect_candidate_trace_projection(
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
    effect_position: u8,
    effect_count: u8,
    candidate_position: u8,
    candidate_count: u8,
    candidate_owner_count_before: u8,
    candidate_owner_count_after: u8,
    producer_episode_retained: bool,
) -> Result<ProductionEffectToCandidateTraceProjection, String> {
    let admission = production_adapter_effect_candidate_admission_disposition(
        effect,
        candidate_owner_count_before,
        candidate_owner_count_after,
    )?;
    let binding = ownership
        .binding()
        .filter(|binding| binding.validate_exact(ownership.owner(), ownership.causality()))
        .ok_or_else(|| "Sumeragi v2 effect omitted its exact candidate binding".to_owned())?;
    let effect_kind = production_adapter_effect_kind(effect);
    let effect_identity = runtime_effect_identity_hash(
        effect_kind,
        &production_adapter_effect_semantic_identity(effect),
    );
    let candidate =
        production_adapter_effect_candidate_binding(effect, binding.candidate_statement.as_ref())?;
    let (candidate_kind, candidate_semantic_identity, candidate_identity) = match candidate {
        None => (RUNTIME_CANDIDATE_KIND_NONE, None, None),
        Some(candidate) => {
            let semantic_identity = runtime_effect_candidate_semantic_hash(
                candidate.kind,
                &candidate.semantic_identity,
            );
            let candidate_identity = runtime_effect_candidate_identity_hash(
                ownership.owner(),
                candidate.kind,
                &semantic_identity,
            );
            (
                candidate.kind,
                Some(semantic_identity),
                Some(candidate_identity),
            )
        }
    };
    Ok(ProductionEffectToCandidateTraceProjection {
        incoming_effect_kind: effect_kind,
        stored_effect_kind: binding.effect_kind,
        incoming_candidate_kind: candidate_kind,
        stored_candidate_kind: binding.candidate_kind,
        causality: runtime_effect_causality_code(ownership.causality()),
        fresh_root_kind: runtime_effect_fresh_root_code(ownership.causality()),
        incoming_effect_position: effect_position,
        stored_effect_position: binding.effect_position,
        incoming_effect_count: effect_count,
        stored_effect_count: binding.effect_count,
        incoming_candidate_position: candidate_position,
        stored_candidate_position: binding.candidate_position,
        incoming_candidate_count: candidate_count,
        stored_candidate_count: binding.candidate_count,
        incoming_lifecycle_ordinal: ownership.owner().lifecycle_ordinal(),
        stored_lifecycle_ordinal: ownership.owner().lifecycle_ordinal(),
        incoming_effect_identity: runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_EFFECT,
            &effect_identity,
        ),
        stored_effect_identity: runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_EFFECT,
            &binding.effect_identity,
        ),
        incoming_owner_identity: runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_LIFECYCLE_OWNER,
            &ownership.owner().projection_hash,
        ),
        stored_owner_identity: runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_LIFECYCLE_OWNER,
            &binding.owner_projection_hash,
        ),
        parent_owner_identity: optional_runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_LIFECYCLE_OWNER,
            binding.parent_owner_projection_hash.as_ref(),
        ),
        incoming_candidate_semantic_identity: optional_runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_CANDIDATE_SEMANTIC,
            candidate_semantic_identity.as_ref(),
        ),
        stored_candidate_semantic_identity: optional_runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_CANDIDATE_SEMANTIC,
            binding.candidate_semantic_identity.as_ref(),
        ),
        incoming_candidate_identity: optional_runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_CAUSAL_CANDIDATE,
            candidate_identity.as_ref(),
        ),
        stored_candidate_identity: optional_runtime_identity_projection(
            IDENTITY_KIND_RUNTIME_CAUSAL_CANDIDATE,
            binding.candidate_identity.as_ref(),
        ),
        candidate_owner_count_before,
        candidate_owner_count_after,
        candidate_owner_admitted: admission.owner_admitted(),
        producer_episode_retained,
    })
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
    /// Complete deeply validated fair-ingress carrier. Local trusted
    /// completions never own one; retaining the bounded process-local object
    /// prevents a same-shape projection hash from replacing authenticated
    /// provenance after selection.
    ingress_ownership: Option<RuntimeIngressOwnershipEvidence>,
    /// Position in the physical FIFO before class-aware removal.
    pub(crate) fifo_position: u64,
    /// Eligible class skips accumulated before selection.
    pub(crate) eligible_skips_before: u64,
    /// Selection retires the candidate's service debt.
    pub(crate) eligible_skips_after: u64,
    /// Derived integrity hash over every candidate projection field.
    pub(crate) projection_hash: iroha_crypto::Hash,
    /// Queue-private, one-shot attestation minted before removal. It binds the
    /// physical position and all class/rank facts to the actual bounded queue,
    /// rather than trusting a caller-rehashed projection.
    selection_seal: RuntimeQueueSelectionSeal,
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

/// Private snapshot of one bounded queue observation.
///
/// The source identity is minted with the queue instance. Callers receive the
/// public projection through scheduler evidence, but cannot manufacture a
/// second snapshot for altered capacity, cursor, or debt fields.
#[derive(Clone, Debug)]
struct RuntimeQueueOwnershipSnapshot {
    source_identity: Arc<()>,
    projection: RuntimeQueueOwnershipProjection,
    minimum_lifecycle_ordinal: Option<u128>,
    completion_at_minimum: u64,
    progress_at_minimum: u64,
    normal_at_minimum: u64,
    projection_hash: iroha_crypto::Hash,
}

impl PartialEq for RuntimeQueueOwnershipSnapshot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && self.projection == other.projection
            && self.minimum_lifecycle_ordinal == other.minimum_lifecycle_ordinal
            && self.completion_at_minimum == other.completion_at_minimum
            && self.progress_at_minimum == other.progress_at_minimum
            && self.normal_at_minimum == other.normal_at_minimum
            && self.projection_hash == other.projection_hash
    }
}

impl Eq for RuntimeQueueOwnershipSnapshot {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeQueueSelectionKind {
    Ordinary,
    FenceCompletion,
}

impl RuntimeQueueSelectionKind {
    const fn code(self) -> u8 {
        match self {
            Self::Ordinary => 1,
            Self::FenceCompletion => 2,
        }
    }
}

/// One-shot queue-issued authority for an exact physical FIFO selection.
///
/// This is an internal ownership capability, not a deductive proof. Its facts
/// are independently checked against the public scheduler relation, while
/// pointer identity and the atomic handoff prevent cloned or caller-rehashed
/// candidates from being installed as a later scheduling occurrence.
#[derive(Clone, Debug)]
struct RuntimeQueueSelectionSeal {
    source_identity: Arc<()>,
    scheduler_handoff_claimed: Arc<AtomicBool>,
    kind: RuntimeQueueSelectionKind,
    queue_before: RuntimeQueueOwnershipProjection,
    oldest_lifecycle_ordinal: u128,
    completion_at_minimum: u64,
    progress_at_minimum: u64,
    normal_at_minimum: u64,
    selected_class: u8,
    selected_position: u64,
    selected_admission_ordinal: u128,
    selected_lifecycle_ordinal: u128,
    selected_eligible_skips: u64,
    selected_identity: RuntimeCommandIdentityDigest,
    selected_tag: EventTag,
    selected_causal_origin_hash: iroha_crypto::Hash,
    selected_ingress_ownership_hash: Option<iroha_crypto::Hash>,
    cursor_after_removal: u8,
    max_debt_after_upper_bound: u64,
    projection_hash: iroha_crypto::Hash,
}

impl PartialEq for RuntimeQueueSelectionSeal {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && Arc::ptr_eq(
                &self.scheduler_handoff_claimed,
                &other.scheduler_handoff_claimed,
            )
            && self.kind == other.kind
            && self.queue_before == other.queue_before
            && self.oldest_lifecycle_ordinal == other.oldest_lifecycle_ordinal
            && self.completion_at_minimum == other.completion_at_minimum
            && self.progress_at_minimum == other.progress_at_minimum
            && self.normal_at_minimum == other.normal_at_minimum
            && self.selected_class == other.selected_class
            && self.selected_position == other.selected_position
            && self.selected_admission_ordinal == other.selected_admission_ordinal
            && self.selected_lifecycle_ordinal == other.selected_lifecycle_ordinal
            && self.selected_eligible_skips == other.selected_eligible_skips
            && self.selected_identity == other.selected_identity
            && self.selected_tag == other.selected_tag
            && self.selected_causal_origin_hash == other.selected_causal_origin_hash
            && self.selected_ingress_ownership_hash == other.selected_ingress_ownership_hash
            && self.cursor_after_removal == other.cursor_after_removal
            && self.max_debt_after_upper_bound == other.max_debt_after_upper_bound
            && self.projection_hash == other.projection_hash
    }
}

impl Eq for RuntimeQueueSelectionSeal {}

fn append_runtime_queue_projection(
    projection: &mut Vec<u8>,
    queue: RuntimeQueueOwnershipProjection,
) {
    append_runtime_identity_u64(projection, queue.len);
    append_runtime_identity_u64(projection, queue.capacity);
    projection.push(queue.service_cursor);
    append_runtime_identity_u64(projection, queue.max_service_debt);
}

fn runtime_queue_ownership_snapshot_projection_hash(
    snapshot: &RuntimeQueueOwnershipSnapshot,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-queue-snapshot:v1");
    append_runtime_identity_field(
        &mut projection,
        &(Arc::as_ptr(&snapshot.source_identity) as usize).to_le_bytes(),
    );
    append_runtime_queue_projection(&mut projection, snapshot.projection);
    match snapshot.minimum_lifecycle_ordinal {
        None => projection.push(0),
        Some(ordinal) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, &ordinal.to_le_bytes());
        }
    }
    append_runtime_identity_u64(&mut projection, snapshot.completion_at_minimum);
    append_runtime_identity_u64(&mut projection, snapshot.progress_at_minimum);
    append_runtime_identity_u64(&mut projection, snapshot.normal_at_minimum);
    iroha_crypto::Hash::new(projection)
}

impl RuntimeQueueOwnershipSnapshot {
    fn validate_identity(&self) -> bool {
        let minimum_count = self
            .completion_at_minimum
            .checked_add(self.progress_at_minimum)
            .and_then(|count| count.checked_add(self.normal_at_minimum));
        self.projection_hash == runtime_queue_ownership_snapshot_projection_hash(self)
            && self.projection.len <= self.projection.capacity
            && CommandClass::from_service_code(self.projection.service_cursor).is_some()
            && (self.projection.len != 0 || self.projection.max_service_debt == 0)
            && match (self.minimum_lifecycle_ordinal, minimum_count) {
                (None, Some(0)) => self.projection.len == 0,
                (Some(ordinal), Some(count)) => {
                    ordinal != 0 && count != 0 && count <= self.projection.len
                }
                _ => false,
            }
    }

    fn class_ready_at_minimum(&self) -> (bool, bool, bool) {
        (
            self.completion_at_minimum != 0,
            self.progress_at_minimum != 0,
            self.normal_at_minimum != 0,
        )
    }
}

fn runtime_queue_selection_seal_projection_hash(
    seal: &RuntimeQueueSelectionSeal,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-queue-selection:v1");
    append_runtime_identity_field(
        &mut projection,
        &(Arc::as_ptr(&seal.source_identity) as usize).to_le_bytes(),
    );
    append_runtime_identity_field(
        &mut projection,
        &(Arc::as_ptr(&seal.scheduler_handoff_claimed) as usize).to_le_bytes(),
    );
    projection.push(seal.kind.code());
    append_runtime_queue_projection(&mut projection, seal.queue_before);
    append_runtime_identity_field(
        &mut projection,
        &seal.oldest_lifecycle_ordinal.to_le_bytes(),
    );
    append_runtime_identity_u64(&mut projection, seal.completion_at_minimum);
    append_runtime_identity_u64(&mut projection, seal.progress_at_minimum);
    append_runtime_identity_u64(&mut projection, seal.normal_at_minimum);
    projection.push(seal.selected_class);
    append_runtime_identity_u64(&mut projection, seal.selected_position);
    append_runtime_identity_field(
        &mut projection,
        &seal.selected_admission_ordinal.to_le_bytes(),
    );
    append_runtime_identity_field(
        &mut projection,
        &seal.selected_lifecycle_ordinal.to_le_bytes(),
    );
    append_runtime_identity_u64(&mut projection, seal.selected_eligible_skips);
    append_runtime_identity_field(
        &mut projection,
        seal.selected_identity.projection_hash.as_ref(),
    );
    append_runtime_identity_tag(&mut projection, seal.selected_tag);
    append_runtime_identity_field(&mut projection, seal.selected_causal_origin_hash.as_ref());
    match seal.selected_ingress_ownership_hash {
        None => projection.push(0),
        Some(hash) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, hash.as_ref());
        }
    }
    projection.push(seal.cursor_after_removal);
    append_runtime_identity_u64(&mut projection, seal.max_debt_after_upper_bound);
    iroha_crypto::Hash::new(projection)
}

impl RuntimeQueueSelectionSeal {
    fn validate_identity(&self) -> bool {
        let minimum_count = self
            .completion_at_minimum
            .checked_add(self.progress_at_minimum)
            .and_then(|count| count.checked_add(self.normal_at_minimum));
        let selected_by_ordinary_cursor = select_bounded_service_class(
            self.queue_before.service_cursor,
            self.completion_at_minimum != 0,
            self.progress_at_minimum != 0,
            self.normal_at_minimum != 0,
        );
        self.projection_hash == runtime_queue_selection_seal_projection_hash(self)
            && self.queue_before.len != 0
            && self.queue_before.len <= self.queue_before.capacity
            && CommandClass::from_service_code(self.queue_before.service_cursor).is_some()
            && self.oldest_lifecycle_ordinal != 0
            && minimum_count.is_some_and(|count| count != 0 && count <= self.queue_before.len)
            && self.selected_class != SERVICE_CLASS_NONE
            && self.selected_position < self.queue_before.len
            && self.selected_admission_ordinal != 0
            && self.selected_lifecycle_ordinal != 0
            && self.selected_lifecycle_ordinal <= self.selected_admission_ordinal
            && self.selected_eligible_skips <= self.queue_before.max_service_debt
            && self.selected_identity.validate_exact()
            && match self.kind {
                RuntimeQueueSelectionKind::Ordinary => {
                    self.selected_lifecycle_ordinal == self.oldest_lifecycle_ordinal
                        && selected_by_ordinary_cursor.selected == self.selected_class
                        && selected_by_ordinary_cursor.next == self.cursor_after_removal
                        && self.max_debt_after_upper_bound
                            == self.queue_before.max_service_debt.saturating_add(1)
                }
                RuntimeQueueSelectionKind::FenceCompletion => {
                    self.selected_class == SERVICE_CLASS_COMPLETION
                        && self.cursor_after_removal == self.queue_before.service_cursor
                        && self.max_debt_after_upper_bound == self.queue_before.max_service_debt
                }
            }
    }

    fn claim_scheduler_handoff_once(&self) -> bool {
        self.validate_identity()
            && self
                .scheduler_handoff_claimed
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }

    fn scheduler_handoff_is_claimed(&self) -> bool {
        self.scheduler_handoff_claimed.load(Ordering::Acquire)
    }

    fn matches_scheduler_occurrence(
        &self,
        candidate: &RuntimeFifoCandidateOwnership,
        before: &RuntimeQueueOwnershipSnapshot,
        after: &RuntimeQueueOwnershipSnapshot,
        expected_kind: RuntimeQueueSelectionKind,
        retry_retained: bool,
    ) -> bool {
        self.validate_identity()
            && self.scheduler_handoff_is_claimed()
            && before.validate_identity()
            && after.validate_identity()
            && Arc::ptr_eq(&self.source_identity, &before.source_identity)
            && Arc::ptr_eq(&self.source_identity, &after.source_identity)
            && self.kind == expected_kind
            && self.queue_before == before.projection
            && self.oldest_lifecycle_ordinal == before.minimum_lifecycle_ordinal.unwrap_or(0)
            && self.completion_at_minimum == before.completion_at_minimum
            && self.progress_at_minimum == before.progress_at_minimum
            && self.normal_at_minimum == before.normal_at_minimum
            && self.selected_class == candidate.class
            && self.selected_position == candidate.fifo_position
            && self.selected_admission_ordinal == candidate.admission_ordinal
            && self.selected_lifecycle_ordinal == candidate.lifecycle_ordinal
            && self.selected_eligible_skips == candidate.eligible_skips_before
            && self.selected_identity == candidate.identity
            && self.selected_tag == candidate.tag
            && self.selected_causal_origin_hash == candidate.causal_origin.projection_hash
            && self.selected_ingress_ownership_hash
                == candidate
                    .ingress_ownership
                    .as_ref()
                    .map(|ownership| ownership.projection_hash)
            && self.cursor_after_removal == after.projection.service_cursor
            && after.projection.max_service_debt <= self.max_debt_after_upper_bound
            && if retry_retained {
                after.projection.len == before.projection.len
            } else {
                after.projection.len.checked_add(1) == Some(before.projection.len)
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeSchedulerArbitrationInputs {
    live_mode: bool,
    timeout_due: bool,
    periodic_timer_due: bool,
    fifo_ready: bool,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
    fence_completion_bypass: bool,
    fence_predecessor_lifecycle_ordinal: Option<u128>,
    fence_predecessor_ownership: Option<RuntimeDeferredLifecycleOwnership>,
    fence_predecessor_ingress_ownership: Option<RuntimeIngressOwnershipEvidence>,
    fence_predecessor_occurrence_ownership: Option<DeferredOccurrenceOwnershipEvidence>,
}

/// Exact source selected for one live or recovery scheduler turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeSelectedOwnerKind {
    /// One older adapter-owned Busy-deferred occurrence.
    Deferred,
    /// The exact causally owned signature completion which opens an active
    /// reducer fence for strictly older unserviceable adapter debt.
    FenceCompletion,
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
    /// Immutable logical owner plus the source occurrence/cut which authorized
    /// this candidate under target-relative physical precedence.
    lifecycle_ownership: RuntimeDeferredLifecycleOwnership,
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
    /// Queue-issued source snapshot for the exact pre-selection observation.
    queue_before_snapshot: RuntimeQueueOwnershipSnapshot,
    /// Queue-issued source snapshot for the exact post-selection observation.
    queue_after_snapshot: RuntimeQueueOwnershipSnapshot,
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
    /// Whether this turn used the narrow dependency edge from older
    /// unserviceable adapter debt to its exact signing completion.
    pub(crate) fence_completion_bypass: bool,
    /// Oldest exact adapter-deferred lifecycle which depended on the selected
    /// fence completion. Present only for the exceptional dependency edge.
    pub(crate) fence_predecessor_lifecycle_ordinal: Option<u128>,
    /// Exact deferred target whose frozen physical cut authorized the narrow
    /// fence-completion dependency edge.
    fence_predecessor_ownership: Option<RuntimeDeferredLifecycleOwnership>,
    /// Current authenticated carrier for that target, when the deferred
    /// occurrence itself came directly from network ingress. A local causal
    /// successor deliberately retains `None` while its wrapper keeps the root
    /// physical pair.
    fence_predecessor_ingress_ownership: Option<RuntimeIngressOwnershipEvidence>,
    /// Adapter-issued unclaimed Busy capability which independently binds the
    /// target ordinal and its direct-authenticated vs local/causal provenance.
    fence_predecessor_occurrence_ownership: Option<DeferredOccurrenceOwnershipEvidence>,
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
            Self::FenceCompletion => 10,
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
    append_runtime_identity_field(
        &mut projection,
        candidate.selection_seal.projection_hash.as_ref(),
    );
    iroha_crypto::Hash::new(projection)
}

fn runtime_fifo_candidate_ingress_is_exact(candidate: &RuntimeFifoCandidateOwnership) -> bool {
    match (&candidate.ingress_ownership, candidate.kind) {
        (None, kind) => kind != RuntimeCommandKind::Authenticated,
        (Some(ownership), RuntimeCommandKind::Authenticated) => {
            ownership.validate_frozen_physical()
                && iroha_crypto::Hash::new(ownership.runtime_bytes.as_ref())
                    == candidate.identity.canonical_hash
                && candidate.causal_origin.root_ingress_identity
                    == Some(runtime_ingress_causal_origin_projection_hash(ownership))
                && candidate.causal_origin.root_ingress_physical_ownership
                    == ownership.earliest_physical_carrier().ok().flatten()
                && match ownership.earliest_lifecycle_ordinal() {
                    Ok(Some(ordinal)) => ordinal == candidate.lifecycle_ordinal,
                    Ok(None) => matches!(ownership.leader_wire_token(), Ok(None)),
                    Err(_) => false,
                }
        }
        (Some(_), _) => false,
    }
}

fn append_runtime_deferred_lifecycle_ownership(
    projection: &mut Vec<u8>,
    ownership: &RuntimeDeferredLifecycleOwnership,
) {
    append_runtime_identity_field(projection, ownership.owner.projection_hash.as_ref());
    match ownership.candidate_semantic_statement {
        None => projection.push(0),
        Some(statement) => {
            projection.push(1);
            append_runtime_identity_field(projection, &statement.semantic_identity());
        }
    }
    append_runtime_identity_field(
        projection,
        &ownership.deferred_admission_ordinal.to_le_bytes(),
    );
    projection.push(ownership.current_ingress.code());
    match ownership.source_physical_ordinal {
        None => projection.push(0),
        Some(ordinal) => {
            projection.push(1);
            append_runtime_identity_u64(projection, ordinal);
        }
    }
    append_runtime_identity_field(projection, &ownership.physical_cut.to_le_bytes());
    append_runtime_identity_field(
        projection,
        ownership.runtime_seal.projection_hash().as_ref(),
    );
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
            append_runtime_deferred_lifecycle_ownership(
                &mut projection,
                &candidate.lifecycle_ownership,
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
        append_runtime_queue_projection(&mut projection, queue);
    }
    append_runtime_identity_field(
        &mut projection,
        evidence.queue_before_snapshot.projection_hash.as_ref(),
    );
    append_runtime_identity_field(
        &mut projection,
        evidence.queue_after_snapshot.projection_hash.as_ref(),
    );
    projection.push(u8::from(evidence.timeout_due));
    projection.push(u8::from(evidence.periodic_timer_due));
    projection.push(u8::from(evidence.fifo_ready));
    projection.push(u8::from(evidence.completion_ready));
    projection.push(u8::from(evidence.progress_ready));
    projection.push(u8::from(evidence.normal_ready));
    projection.push(u8::from(evidence.fence_completion_bypass));
    match evidence.fence_predecessor_lifecycle_ordinal {
        None => projection.push(0),
        Some(ordinal) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, &ordinal.to_le_bytes());
        }
    }
    match &evidence.fence_predecessor_ownership {
        None => projection.push(0),
        Some(ownership) => {
            projection.push(1);
            append_runtime_deferred_lifecycle_ownership(&mut projection, ownership);
        }
    }
    match &evidence.fence_predecessor_ingress_ownership {
        None => projection.push(0),
        Some(ownership) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, ownership.projection_hash.as_ref());
        }
    }
    match &evidence.fence_predecessor_occurrence_ownership {
        None => projection.push(0),
        Some(ownership) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, ownership.projection_hash().as_ref());
        }
    }
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
        let queue_snapshots_are_exact = self.queue_before_snapshot.validate_identity()
            && self.queue_after_snapshot.validate_identity()
            && Arc::ptr_eq(
                &self.queue_before_snapshot.source_identity,
                &self.queue_after_snapshot.source_identity,
            )
            && self.queue_before_snapshot.projection == self.queue_before
            && self.queue_after_snapshot.projection == self.queue_after;
        let ready_classes_match_snapshot = !self.fifo_ready
            || self.queue_before_snapshot.class_ready_at_minimum()
                == (
                    self.completion_ready,
                    self.progress_ready,
                    self.normal_ready,
                );
        let fence_predecessor_is_exact = match (
            &self.fence_predecessor_ownership,
            &self.fence_predecessor_ingress_ownership,
            &self.fence_predecessor_occurrence_ownership,
        ) {
            (None, None, None) => self.fence_predecessor_lifecycle_ordinal.is_none(),
            (Some(ownership), ingress, Some(occurrence)) => {
                let ingress_is_exact = match ingress {
                    Some(ingress) => {
                        ownership.current_ingress == RuntimeDispatchIngress::DirectAuthenticated
                            && ingress.validate_frozen_physical()
                            && ownership
                                .owner()
                                .causal_origin()
                                .root_ingress_physical_ownership
                                .is_some_and(|physical| {
                                    ingress.contains_physical_carrier(physical) == Ok(true)
                                })
                            && ownership.owner().causal_origin().root_ingress_identity
                                == Some(runtime_ingress_causal_origin_projection_hash(ingress))
                    }
                    None => ownership.current_ingress == RuntimeDispatchIngress::LocalOrCausal,
                };
                ownership.validate_exact()
                    && occurrence.validate_exact()
                    && ownership.validate_against_ingress(ingress.as_ref())
                    && occurrence.matches_runtime_ownership_seal(&ownership.runtime_seal)
                    && occurrence.admission_ordinal() == ownership.deferred_admission_ordinal
                    && occurrence.is_authenticated_ingress()
                        == (ownership.current_ingress
                            == RuntimeDispatchIngress::DirectAuthenticated)
                    && self.fence_predecessor_lifecycle_ordinal
                        == Some(ownership.owner().lifecycle_ordinal())
                    && ingress_is_exact
            }
            _ => false,
        };
        if self.projection_hash != runtime_scheduler_projection_hash(self)
            || !queue_snapshots_are_exact
            || !ready_classes_match_snapshot
            || self.queue_before.capacity != self.queue_after.capacity
            || self.queue_before.len > self.queue_before.capacity
            || self.queue_after.len > self.queue_after.capacity
            || (self.queue_before.len == 0 && self.queue_before.max_service_debt != 0)
            || (self.queue_after.len == 0 && self.queue_after.max_service_debt != 0)
            || CommandClass::from_service_code(self.queue_before.service_cursor).is_none()
            || CommandClass::from_service_code(self.queue_after.service_cursor).is_none()
            || (self.fifo_ready && self.queue_before.len == 0)
            || self.fifo_ready
                != (self.completion_ready || self.progress_ready || self.normal_ready)
            || (!self.live_mode && (self.timeout_due || self.periodic_timer_due))
            || (self.fence_completion_bypass
                != matches!(self.selected, RuntimeSelectedOwnerKind::FenceCompletion))
            || (self.fence_completion_bypass != self.fence_predecessor_ownership.is_some())
            || !fence_predecessor_is_exact
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
                        && candidate.lifecycle_ownership.current_ingress
                            == RuntimeDispatchIngress::DirectAuthenticated
                        && ownership.validate_frozen_physical()
                        && candidate
                            .service
                            .matches_authenticated_runtime_bytes(&ownership.runtime_bytes)
                        && match ownership.earliest_lifecycle_ordinal() {
                            Ok(Some(ordinal)) => {
                                ordinal == candidate.lifecycle_ownership.owner().lifecycle_ordinal()
                            }
                            // Generic authenticated ingress receives its
                            // logical ordinal only at runtime admission. It is
                            // therefore exact without a leader-wire tag, while
                            // a tagged carrier may never lose that tag.
                            Ok(None) => matches!(ownership.leader_wire_token(), Ok(None)),
                            Err(_) => false,
                        }
                        && candidate
                            .lifecycle_ownership
                            .owner()
                            .causal_origin()
                            .root_ingress_identity
                            == Some(runtime_ingress_causal_origin_projection_hash(ownership))
                        && candidate
                            .lifecycle_ownership
                            .owner()
                            .causal_origin()
                            .root_ingress_physical_ownership
                            .is_some_and(|physical| {
                                ownership.contains_physical_carrier(physical) == Ok(true)
                            })
                }
                None => {
                    !candidate.service.is_authenticated_ingress()
                        && candidate.lifecycle_ownership.current_ingress
                            == RuntimeDispatchIngress::LocalOrCausal
                }
            };
            return if candidate.service.validate_exact()
                && candidate.service.service_handoff_is_complete()
                && candidate.lifecycle_ownership.validate_exact()
                && candidate.service.admission_ordinal
                    == candidate.lifecycle_ownership.deferred_admission_ordinal
                && candidate
                    .service
                    .matches_runtime_ownership_seal(&candidate.lifecycle_ownership.runtime_seal)
                && candidate
                    .lifecycle_ownership
                    .validate_against_ingress(candidate.ingress_ownership.as_ref())
                && ingress_exact
                && self.queue_before == self.queue_after
                && self.fifo_owed_before == self.fifo_owed_after
            {
                Ok(())
            } else {
                Err(RuntimeSchedulerEvidenceError::InvalidProjection)
            };
        }
        if let (
            RuntimeSelectedOwnerKind::FenceCompletion,
            RuntimeSelectedCandidateOwnership::Exact(candidate),
        ) = (&self.selected, &self.candidate)
        {
            let exact = self.live_mode
                && self.fence_completion_bypass
                && self.round_tag == candidate.tag
                // Fence completion is an explicit dependency branch, not an
                // ordinary FIFO or clock selection. Its carrier must not
                // claim that normal arbitration was simultaneously ready.
                && !self.timeout_due
                && !self.periodic_timer_due
                && !self.fifo_ready
                && !self.completion_ready
                && !self.progress_ready
                && !self.normal_ready
                && candidate.identity.validate_exact()
                && candidate.kind == RuntimeCommandKind::SignatureCompleted
                && candidate.kind == candidate.identity.kind
                && candidate.class == SERVICE_CLASS_COMPLETION
                && candidate.admission_ordinal != 0
                && candidate.lifecycle_ordinal != 0
                && candidate.lifecycle_ordinal <= candidate.admission_ordinal
                && runtime_fifo_candidate_ingress_is_exact(candidate)
                && candidate.projection_hash == runtime_fifo_candidate_projection_hash(candidate)
                && candidate.causal_origin.validate_exact()
                && candidate.causal_origin.root_lifecycle_ordinal
                    == Some(candidate.lifecycle_ordinal)
                // A callback minted as an independent SignatureCompleted root
                // did not inherit the Sign effect and cannot bypass lifecycle
                // order even if its bytes happen to clear the reducer fence.
                && candidate.causal_origin.root_identity.kind
                    != RuntimeCommandKind::SignatureCompleted
                && self
                    .fence_predecessor_lifecycle_ordinal
                    .is_some_and(|predecessor| predecessor < candidate.lifecycle_ordinal)
                && candidate.fifo_position < self.queue_before.len
                && candidate.eligible_skips_before <= self.queue_before.max_service_debt
                && candidate.eligible_skips_after == 0
                && self.queue_before.service_cursor == self.queue_after.service_cursor
                && self.queue_after.max_service_debt <= self.queue_before.max_service_debt
                && self.fifo_owed_before == self.fifo_owed_after
                && self.queue_after.len.checked_add(1) == Some(self.queue_before.len)
                && candidate.selection_seal.matches_scheduler_occurrence(
                    candidate,
                    &self.queue_before_snapshot,
                    &self.queue_after_snapshot,
                    RuntimeQueueSelectionKind::FenceCompletion,
                    false,
                );
            return exact
                .then_some(())
                .ok_or(RuntimeSchedulerEvidenceError::InvalidProjection);
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
                    && candidate.admission_ordinal != 0
                    && candidate.lifecycle_ordinal != 0
                    && candidate.lifecycle_ordinal <= candidate.admission_ordinal
                    && runtime_fifo_candidate_ingress_is_exact(candidate)
                    && candidate.projection_hash
                        == runtime_fifo_candidate_projection_hash(candidate)
                    && candidate.causal_origin.validate_exact()
                    && candidate.causal_origin.root_lifecycle_ordinal
                        == Some(candidate.lifecycle_ordinal)
                    && candidate.class != SERVICE_CLASS_NONE
                    && service.selected == candidate.class
                    && service.next == self.queue_after.service_cursor
                    && candidate.fifo_position < self.queue_before.len
                    && candidate.eligible_skips_before <= self.queue_before.max_service_debt
                    && candidate.eligible_skips_after == 0
                    && self.queue_after.max_service_debt
                        <= self.queue_before.max_service_debt.saturating_add(1)
                    && if retry_retained {
                        self.queue_after.len == self.queue_before.len
                    } else {
                        self.queue_after.len.checked_add(1) == Some(self.queue_before.len)
                    }
                    && scheduled == ScheduledWork::Fifo
                    && self.live_mode != recovery
                    && candidate.selection_seal.matches_scheduler_occurrence(
                        candidate,
                        &self.queue_before_snapshot,
                        &self.queue_after_snapshot,
                        RuntimeQueueSelectionKind::Ordinary,
                        retry_retained,
                    );
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
    candidate_semantic_statement: Option<RuntimeCandidateSemanticStatement>,
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
            candidate_semantic_statement: None,
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
            && ingress_ownership.validate_frozen_physical()
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
            candidate_semantic_statement: None,
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
            candidate_semantic_statement: None,
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
            || !ingress_ownership.validate_frozen_physical()
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

    /// Validate the constant-size admission certificate retained by an
    /// immutable queued command.
    ///
    /// Full command and ingress ownership validation happens before queue
    /// publication (and again before dispatch). Scheduler rank scans only
    /// need the cached result plus the fixed-size structural projection; in
    /// particular they must not repeatedly decode and authenticate the same
    /// retained network envelope while an exact Serve barrier is polling.
    fn validate_cached_admission_identity(&self) -> bool {
        self.identity_deep_validated
            && self.identity.validate_exact()
            && self.candidate_semantic_statement.is_none_or(|statement| {
                statement.validate_exact() && statement.round.height == self.tag.height()
            })
            && self.restored_producer_stage.is_none_or(|_| {
                self.lifecycle_ordinal.is_some()
                    && self.causal_origin.restored_producer_lifecycle_key.is_some()
            })
            && match (&self.ingress_ownership, self.identity.kind) {
                (Some(_), RuntimeCommandKind::Authenticated) => true,
                (None, RuntimeCommandKind::Authenticated) | (Some(_), _) => false,
                (None, _) => true,
            }
    }

    fn validate_admission_identity(&self) -> bool {
        self.validate_cached_admission_identity()
            && match self.identity.kind {
                RuntimeCommandKind::Authenticated => {
                    self.ingress_ownership.as_ref().is_some_and(|ownership| {
                        ownership.validate_frozen_physical()
                            && match ownership.earliest_lifecycle_ordinal() {
                                Ok(Some(ordinal)) => self.lifecycle_ordinal == Some(ordinal),
                                Ok(None) => true,
                                Err(_) => false,
                            }
                    })
                }
                _ => true,
            }
    }
}

struct BoundedIngress<C> {
    config: RuntimeQueueConfig,
    commands: VecDeque<TaggedCommand<C>>,
    /// Process-local identity which authorizes queue observation and selection
    /// seals. It is never serialized or exposed as runtime configuration.
    selection_source_identity: Arc<()>,
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
    #[cfg(test)]
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
            selection_source_identity: Arc::new(()),
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
    /// Ordinary causal siblings may reuse it only with the identical immutable
    /// root. Distinct restored producer stages may share it only when their
    /// durable lifecycle key and every frozen ingress/root field agree; their
    /// stage-specific command identity remains distinct. An unrelated queued,
    /// reserved, or restart-dormant owner cannot alias the position.
    fn restored_successor_shares_lifecycle(
        existing: &TaggedCommand<C>,
        candidate: &TaggedCommand<C>,
    ) -> bool {
        let (Some(existing_stage), Some(candidate_stage)) = (
            existing.restored_producer_stage,
            candidate.restored_producer_stage,
        ) else {
            return false;
        };
        let existing_origin = &existing.causal_origin;
        let candidate_origin = &candidate.causal_origin;
        existing_stage != candidate_stage
            && existing.tag == candidate.tag
            && existing.class == candidate.class
            && existing.ingress_ownership == candidate.ingress_ownership
            && existing_origin.validate_exact()
            && candidate_origin.validate_exact()
            && existing_origin.restored_producer_lifecycle_key.is_some()
            && existing_origin.restored_producer_lifecycle_key
                == candidate_origin.restored_producer_lifecycle_key
            && existing_origin.lifecycle_key == candidate_origin.lifecycle_key
            && existing_origin.root_tag == candidate_origin.root_tag
            && existing_origin.root_class == candidate_origin.root_class
            && existing_origin.root_ingress_identity == candidate_origin.root_ingress_identity
            && existing_origin.root_ingress_physical_ownership
                == candidate_origin.root_ingress_physical_ownership
            && existing_origin.leader_wire_lifecycle_key
                == candidate_origin.leader_wire_lifecycle_key
            && existing_origin.root_lifecycle_ordinal == candidate_origin.root_lifecycle_ordinal
    }

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
                && !Self::restored_successor_shares_lifecycle(existing, command)
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
        if dormant_replacements != 0 && class != CommandClass::Completion {
            return Err(EnqueueError::FailClosed);
        }
        let occupied = self.occupied_with_dormant_reservations()?;
        let occupied_after = occupied
            .checked_sub(dormant_replacements)
            .and_then(|occupied| occupied.checked_add(additions))
            .ok_or(EnqueueError::FailClosed)?;
        if occupied_after > self.config.capacity {
            return Err(EnqueueError::Full);
        }

        // Reservations are class allocations, not arrival-order allocations.
        // A Completion which arrived first cannot consume a Normal or Progress
        // prefix, although every class still consumes one position in the
        // common physical bound.  Counting total occupancy against each lower
        // class limit made the same multiset admissible or inadmissible solely
        // according to enqueue order.
        let normal_before = self
            .commands
            .iter()
            .filter(|queued| queued.class == CommandClass::Normal)
            .count();
        let progress_before = self
            .commands
            .iter()
            .filter(|queued| queued.class == CommandClass::Progress)
            .count();
        let normal_after = normal_before
            .checked_add(usize::from(class == CommandClass::Normal) * additions)
            .ok_or(EnqueueError::FailClosed)?;
        let progress_after = progress_before
            .checked_add(usize::from(class == CommandClass::Progress) * additions)
            .ok_or(EnqueueError::FailClosed)?;
        let noncompletion_after = normal_after
            .checked_add(progress_after)
            .ok_or(EnqueueError::FailClosed)?;
        if normal_after > self.config.normal_limit()
            || noncompletion_after > self.config.progress_limit()
        {
            return Err(EnqueueError::ReservedCapacity);
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

    fn class_counts_at_lifecycle(&self, lifecycle_ordinal: u128) -> (u64, u64, u64) {
        let count = |class| {
            u64::try_from(
                self.commands
                    .iter()
                    .filter(|queued| {
                        queued.class == class && queued.lifecycle_ordinal == Some(lifecycle_ordinal)
                    })
                    .count(),
            )
            .expect("bounded runtime class count is representable as u64")
        };
        (
            count(CommandClass::Completion),
            count(CommandClass::Progress),
            count(CommandClass::Normal),
        )
    }

    fn ownership_snapshot(&self) -> RuntimeQueueOwnershipSnapshot {
        let minimum_lifecycle_ordinal = self
            .commands
            .iter()
            .map(|queued| queued.lifecycle_ordinal)
            .min()
            .unwrap_or(None);
        let (completion_at_minimum, progress_at_minimum, normal_at_minimum) =
            minimum_lifecycle_ordinal
                .map_or((0, 0, 0), |ordinal| self.class_counts_at_lifecycle(ordinal));
        let mut snapshot = RuntimeQueueOwnershipSnapshot {
            source_identity: Arc::clone(&self.selection_source_identity),
            projection: self.ownership_projection(),
            minimum_lifecycle_ordinal,
            completion_at_minimum,
            progress_at_minimum,
            normal_at_minimum,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        snapshot.projection_hash = runtime_queue_ownership_snapshot_projection_hash(&snapshot);
        snapshot
    }

    #[allow(clippy::too_many_arguments)]
    fn mint_selection_seal(
        &self,
        kind: RuntimeQueueSelectionKind,
        queue_before: &RuntimeQueueOwnershipSnapshot,
        selected_class: u8,
        selected_position: u64,
        selected_admission_ordinal: u128,
        selected_lifecycle_ordinal: u128,
        selected_eligible_skips: u64,
        selected_identity: RuntimeCommandIdentityDigest,
        selected_tag: EventTag,
        selected_causal_origin_hash: iroha_crypto::Hash,
        selected_ingress_ownership_hash: Option<iroha_crypto::Hash>,
        cursor_after_removal: u8,
    ) -> Result<RuntimeQueueSelectionSeal, EnqueueError> {
        if !queue_before.validate_identity()
            || !Arc::ptr_eq(
                &queue_before.source_identity,
                &self.selection_source_identity,
            )
        {
            return Err(EnqueueError::FailClosed);
        }
        let oldest_lifecycle_ordinal = queue_before
            .minimum_lifecycle_ordinal
            .ok_or(EnqueueError::FailClosed)?;
        let max_debt_after_upper_bound = match kind {
            RuntimeQueueSelectionKind::Ordinary => {
                queue_before.projection.max_service_debt.saturating_add(1)
            }
            RuntimeQueueSelectionKind::FenceCompletion => queue_before.projection.max_service_debt,
        };
        let mut seal = RuntimeQueueSelectionSeal {
            source_identity: Arc::clone(&self.selection_source_identity),
            scheduler_handoff_claimed: Arc::new(AtomicBool::new(false)),
            kind,
            queue_before: queue_before.projection,
            oldest_lifecycle_ordinal,
            completion_at_minimum: queue_before.completion_at_minimum,
            progress_at_minimum: queue_before.progress_at_minimum,
            normal_at_minimum: queue_before.normal_at_minimum,
            selected_class,
            selected_position,
            selected_admission_ordinal,
            selected_lifecycle_ordinal,
            selected_eligible_skips,
            selected_identity,
            selected_tag,
            selected_causal_origin_hash,
            selected_ingress_ownership_hash,
            cursor_after_removal,
            max_debt_after_upper_bound,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        seal.projection_hash = runtime_queue_selection_seal_projection_hash(&seal);
        seal.validate_identity()
            .then_some(seal)
            .ok_or(EnqueueError::FailClosed)
    }

    fn oldest_lifecycle_ordinal(&self) -> Result<Option<u128>, EnqueueError> {
        self.commands
            .iter()
            .map(|queued| {
                let ordinal = queued.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
                (queued.validate_cached_admission_identity()
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

    /// Oldest owner which is allowed to precede an already-admitted producer
    /// continuation at `physical_cut`.
    ///
    /// A leader-wire replay admitted at or after the cut retains its logical
    /// scheduler ordinal for identity, but its fresh physical carrier is
    /// behind the continuation and must not become its runner blocker.
    fn oldest_active_lifecycle_ordinal_before_physical_cut_excluding(
        &self,
        physical_cut: u128,
        excluded: &[RuntimeLifecycleOwner],
    ) -> Result<Option<u128>, EnqueueError> {
        if physical_cut == 0 {
            return Err(EnqueueError::FailClosed);
        }
        let command_minimum = self.commands.iter().try_fold(
            None,
            |minimum, queued| -> Result<Option<u128>, EnqueueError> {
                let ordinal = queued.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
                if !queued.validate_admission_identity()
                    || !queued.causal_origin.validate_exact()
                    || queued.causal_origin.root_lifecycle_ordinal != Some(ordinal)
                {
                    return Err(EnqueueError::FailClosed);
                }
                let owner = queued.lifecycle_owner()?;
                if excluded.iter().any(|excluded| excluded == &owner) {
                    return Ok(minimum);
                }
                let post_cut_ingress = match (
                    queued.causal_origin.root_ingress_physical_ownership,
                    queued.ingress_ownership.as_ref(),
                ) {
                    (Some(root), Some(ownership))
                        if ownership.contains_physical_carrier(root) == Ok(true) =>
                    {
                        u128::from(root.source_ordinal) >= physical_cut
                    }
                    (Some(root), None) => u128::from(root.source_ordinal) >= physical_cut,
                    (None, None) => false,
                    (Some(_), Some(_)) | (None, Some(_)) => {
                        return Err(EnqueueError::FailClosed);
                    }
                };
                if post_cut_ingress {
                    return Ok(minimum);
                }
                Ok(Some(
                    minimum.map_or(ordinal, |current: u128| current.min(ordinal)),
                ))
            },
        )?;
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

    /// Remove the first exact Completion command which satisfies a
    /// runtime-validated dependency predicate without changing class-service
    /// cursor or debt. This is reserved for the one callback which opens a
    /// signing fence for strictly older, otherwise unserviceable adapter debt.
    fn pop_fence_completion_with_ownership(
        &mut self,
        mut matches_fence: impl FnMut(&TaggedCommand<C>) -> bool,
    ) -> Result<Option<(TaggedCommand<C>, RuntimeFifoCandidateOwnership)>, EnqueueError> {
        // Validate the complete retained set before a dependency edge can
        // bypass its ordinary minimum-lifecycle selection.
        let _ = self.oldest_lifecycle_ordinal()?;
        let queue_before = self.ownership_snapshot();
        let Some(index) = self
            .commands
            .iter()
            .position(|queued| queued.class == CommandClass::Completion && matches_fence(queued))
        else {
            return Ok(None);
        };
        let selected = self
            .commands
            .get(index)
            .expect("selected fence completion remains present");
        let admission_ordinal = selected.admission_ordinal.ok_or(EnqueueError::FailClosed)?;
        let lifecycle_ordinal = selected.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
        let identity = selected.identity;
        if !selected.identity_deep_validated
            || !identity.validate_exact()
            || identity.kind != RuntimeCommandKind::SignatureCompleted
            || selected.ingress_ownership.is_some()
            || !selected.causal_origin.validate_exact()
            || selected.causal_origin.root_lifecycle_ordinal != Some(lifecycle_ordinal)
        {
            return Err(EnqueueError::FailClosed);
        }
        let fifo_position =
            u64::try_from(index).expect("bounded runtime FIFO position is representable as u64");
        let selection_seal = self.mint_selection_seal(
            RuntimeQueueSelectionKind::FenceCompletion,
            &queue_before,
            selected.class.service_code(),
            fifo_position,
            admission_ordinal,
            lifecycle_ordinal,
            selected.eligible_skips,
            identity,
            selected.tag,
            selected.causal_origin.projection_hash,
            None,
            queue_before.projection.service_cursor,
        )?;
        let mut candidate = RuntimeFifoCandidateOwnership {
            kind: identity.kind,
            identity,
            class: selected.class.service_code(),
            tag: selected.tag,
            admission_ordinal,
            lifecycle_ordinal,
            causal_origin: selected.causal_origin.clone(),
            ingress_ownership: None,
            fifo_position,
            eligible_skips_before: selected.eligible_skips,
            eligible_skips_after: 0,
            projection_hash: iroha_crypto::Hash::new([]),
            selection_seal,
        };
        if !runtime_fifo_candidate_ingress_is_exact(&candidate) {
            return Err(EnqueueError::FailClosed);
        }
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(&candidate);
        let command = self
            .commands
            .remove(index)
            .expect("selected fence completion remains present");
        debug_assert_eq!(
            queue_before.projection.len,
            self.ownership_projection().len + 1
        );
        Ok(Some((command, candidate)))
    }

    /// Return exact owners of queued commands which the driver proves cannot
    /// cross its active reducer fence. The caller may remove only these aliases
    /// from one dependency-minimum calculation; no queue item is consumed or
    /// reordered here.
    fn fence_blocked_lifecycle_owners(
        &self,
        mut is_blocked: impl FnMut(&TaggedCommand<C>) -> bool,
    ) -> Result<Vec<RuntimeLifecycleOwner>, EnqueueError> {
        // Validate the complete queue before any owner can be excluded from a
        // lifecycle comparison, including entries which do not match.
        let _ = self.oldest_lifecycle_ordinal()?;
        self.commands
            .iter()
            .filter(|queued| is_blocked(queued))
            .map(|queued| queued.lifecycle_owner())
            .collect()
    }

    fn pop_next_with_ownership(
        &mut self,
    ) -> Result<Option<(TaggedCommand<C>, RuntimeFifoCandidateOwnership)>, EnqueueError>
    where
        C: ExactRuntimeCommandIdentity,
    {
        let queue_before = self.ownership_snapshot();
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
        let fifo_position =
            u64::try_from(index).expect("bounded runtime FIFO position is representable as u64");
        let selection_seal = self.mint_selection_seal(
            RuntimeQueueSelectionKind::Ordinary,
            &queue_before,
            selected.class.service_code(),
            fifo_position,
            admission_ordinal,
            lifecycle_ordinal,
            selected.eligible_skips,
            identity,
            selected.tag,
            selected.causal_origin.projection_hash,
            selected
                .ingress_ownership
                .as_ref()
                .map(|ownership| ownership.projection_hash),
            selection.next,
        )?;
        let mut candidate = RuntimeFifoCandidateOwnership {
            kind: identity.kind,
            identity,
            class: selected.class.service_code(),
            tag: selected.tag,
            admission_ordinal,
            lifecycle_ordinal,
            causal_origin: selected.causal_origin.clone(),
            ingress_ownership: selected.ingress_ownership.clone(),
            fifo_position,
            eligible_skips_before: selected.eligible_skips,
            eligible_skips_after: 0,
            projection_hash: iroha_crypto::Hash::new([]),
            selection_seal,
        };
        if !runtime_fifo_candidate_ingress_is_exact(&candidate) {
            return Err(EnqueueError::FailClosed);
        }
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
        debug_assert_eq!(
            queue_before.projection.len,
            self.ownership_projection().len + 1
        );
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
        if !command.validate_admission_identity()
            || !candidate.identity.validate_exact()
            || !candidate.selection_seal.validate_identity()
            || candidate.selection_seal.scheduler_handoff_is_claimed()
            || candidate.projection_hash != runtime_fifo_candidate_projection_hash(candidate)
            || !runtime_fifo_candidate_ingress_is_exact(candidate)
            || command.identity != candidate.identity
            || command.identity.kind != candidate.kind
            || command.class.service_code() != candidate.class
            || command.tag != candidate.tag
            || command.admission_ordinal != Some(candidate.admission_ordinal)
            || command.lifecycle_ordinal != Some(candidate.lifecycle_ordinal)
            || command.causal_origin != candidate.causal_origin
            || command.causal_origin.root_lifecycle_ordinal != Some(candidate.lifecycle_ordinal)
            || command.ingress_ownership != candidate.ingress_ownership
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
    candidate_semantic_statement: Option<RuntimeCandidateSemanticStatement>,
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
            candidate_semantic_statement: None,
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
            candidate_semantic_statement: None,
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
            candidate_semantic_statement: None,
            restored_producer_stage: None,
            dormant_replacement: None,
        }
    }

    fn coalesced_with_owner(
        tag: EventTag,
        manifest: wire::PayloadManifest,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<Self, EnqueueError> {
        if !ownership.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        let mut reservation = Self::coalesced(tag, manifest);
        reservation.lifecycle_ordinal = Some(ownership.owner().lifecycle_ordinal());
        reservation.causal_origin = Some(ownership.owner().causal_origin().clone());
        reservation.candidate_semantic_statement = ownership.candidate_semantic_statement();
        Ok(reservation)
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

    fn exact_body_pipeline_completion_owners(
        &self,
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> Result<Vec<RuntimeLifecycleOwner>, EnqueueError> {
        let mut owners = self
            .commands
            .iter()
            .filter(|queued| {
                queued.tag == tag
                    && queued.command.body_pipeline_completion_ownership(candidate) == Some(true)
            })
            .map(TaggedCommand::lifecycle_owner)
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(reservation) = self
            .reserved_body_available
            .as_ref()
            .filter(|reservation| reservation.tag == tag)
            && let BodyPipelineCompletionEvidence::BodyAvailable {
                manifest: candidate_manifest,
            } = candidate
            && reservation.manifest == *candidate_manifest
        {
            owners.push(
                reservation
                    .lifecycle_owner()
                    .ok_or(EnqueueError::FailClosed)?,
            );
        }
        Ok(owners)
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
    #[cfg(test)]
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

    #[cfg(test)]
    fn reserve_canonical_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        self.reserve_canonical_body_available_internal(tag, manifest, None, None, None)
    }

    fn reserve_canonical_body_available_internal(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        owner: Option<&RuntimeLifecycleOwner>,
        candidate_semantic_statement: Option<RuntimeCandidateSemanticStatement>,
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
        prospective.candidate_semantic_statement = candidate_semantic_statement;
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
                && existing.candidate_semantic_statement == candidate_semantic_statement
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
                reservation.candidate_semantic_statement = candidate_semantic_statement;
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
            let owner_is_exact = match (
                reservation.causal_origin.is_some(),
                reservation.lifecycle_ordinal.is_some(),
            ) {
                (false, false) => reservation.candidate_semantic_statement.is_none(),
                (true, true) => {
                    reservation.lifecycle_owner().is_some()
                        && reservation
                            .candidate_semantic_statement
                            .is_none_or(RuntimeCandidateSemanticStatement::validate_exact)
                }
                (false, true) | (true, false) => false,
            };
            return (reservation.admission_ordinal.is_none()
                && reservation.restored_producer_stage.is_none()
                && reservation.dormant_replacement.is_none()
                && owner_is_exact)
                .then_some(())
                .ok_or(EnqueueError::FailClosed);
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
        command.candidate_semantic_statement = reservation.candidate_semantic_statement;
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

/// Current command carrier at the runtime-to-adapter dispatch seam.
///
/// Causal successors retain their root ingress physical pair but do not carry
/// the original authenticated envelope. Keeping this distinction explicit
/// prevents a direct network command from dropping its current ingress proof
/// while still allowing an internally generated successor to inherit the
/// frozen root pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeDispatchIngress {
    /// Locally originated work or a causal successor of an older root.
    LocalOrCausal,
    /// The current command directly carries authenticated receiver ingress.
    DirectAuthenticated,
}

impl RuntimeDispatchIngress {
    const fn code(self) -> u8 {
        match self {
            Self::LocalOrCausal => 0,
            Self::DirectAuthenticated => 1,
        }
    }
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
    /// An exact live or terminal producer record suppresses this occurrence
    /// only for the immutable owner which originally admitted it.
    CoalesceOwned {
        /// Persisted causal lifecycle key retained beside the service marker.
        causal_lifecycle_key: iroha_crypto::Hash,
        /// Persisted immutable first-admission ordinal.
        admission_ordinal: u128,
    },
    /// The internal command is malformed or conflicts with frozen authority.
    Reject,
}

impl RuntimeCommandAdmissionPreflight {
    const fn is_coalescence(self) -> bool {
        matches!(self, Self::Coalesce | Self::CoalesceOwned { .. })
    }
}

include!("v2_runtime/dormant_producer_ownership.rs");

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

    fn completion_unblocks_deferred_fence(&self, tag: EventTag, command: &Self::Command) -> bool {
        SumeragiV2Adapter::completion_unblocks_deferred_fence(self, tag, command)
    }

    fn command_is_blocked_by_deferred_fence(&self, tag: EventTag, command: &Self::Command) -> bool {
        SumeragiV2Adapter::command_is_blocked_by_deferred_fence(self, tag, command)
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

    fn deferred_occurrence_ownership(
        &self,
        admission_ordinal: u128,
    ) -> Option<DeferredOccurrenceOwnershipEvidence> {
        SumeragiV2Adapter::deferred_occurrence_ownership(self, admission_ordinal)
    }

    fn seal_deferred_runtime_ownership(
        &mut self,
        admission_ordinal: u128,
        owner: &RuntimeLifecycleOwner,
        current_ingress: RuntimeDispatchIngress,
        source_physical_ordinal: Option<u64>,
        physical_cut: u128,
    ) -> Result<DeferredRuntimeOwnershipSeal, Self::Error> {
        if !owner.validate_exact() {
            return Err(AdapterError::RuntimeIngressOwnershipViolation);
        }
        SumeragiV2Adapter::bind_deferred_runtime_ownership(
            self,
            admission_ordinal,
            owner.causal_origin().lifecycle_key.clone(),
            owner.lifecycle_ordinal(),
            current_ingress == RuntimeDispatchIngress::DirectAuthenticated,
            source_physical_ordinal,
            physical_cut,
        )
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

    fn effect_refinement_kind(effect: &Self::Effect) -> u8 {
        production_adapter_effect_kind(effect)
    }

    fn effect_semantic_identity(effect: &Self::Effect) -> Vec<u8> {
        production_adapter_effect_semantic_identity(effect)
    }

    fn effect_candidate_semantic_identity(effect: &Self::Effect) -> Option<(u8, Vec<u8>)> {
        production_adapter_effect_candidate_semantic_identity(effect)
    }

    fn effect_candidate_semantic_binding(
        effect: &Self::Effect,
        inherited: Option<&RuntimeCandidateSemanticStatement>,
    ) -> Result<Option<RuntimeEffectCandidateSemantic>, String> {
        production_adapter_effect_candidate_binding(effect, inherited)
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

/// One adapter-deferred producer together with its immutable receiver-local
/// ingress cut.
///
/// The logical lifecycle may be rebased when an older exact aggregate carrier
/// joins the same request. The physical cut never moves: a leader-wire replay
/// admitted at or after it cannot regain a position ahead of this already
/// admitted continuation.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeDeferredLifecycleOwnership {
    owner: RuntimeLifecycleOwner,
    /// Exact route-neutral statement transferred with a body-pipeline
    /// completion while the adapter retains it in Busy storage.
    candidate_semantic_statement: Option<RuntimeCandidateSemanticStatement>,
    /// Adapter-global Busy occurrence ordinal used as the runtime map key.
    deferred_admission_ordinal: u128,
    /// Whether the command which acquired this Busy owner directly carried an
    /// authenticated envelope or merely inherited an older network root.
    current_ingress: RuntimeDispatchIngress,
    /// Receiver-local occurrence which created this continuation. Local
    /// continuations have no network occurrence and retain `None`.
    source_physical_ordinal: Option<u64>,
    physical_cut: u128,
    /// Adapter-private capability binding this wrapper to the exact Busy
    /// occurrence, causal lifecycle, provenance, and frozen physical cut.
    runtime_seal: DeferredRuntimeOwnershipSeal,
}

impl RuntimeDeferredLifecycleOwnership {
    fn new(
        owner: RuntimeLifecycleOwner,
        deferred_admission_ordinal: u128,
        current_ingress: RuntimeDispatchIngress,
        source_physical_ordinal: Option<u64>,
        physical_cut: u128,
        runtime_seal: DeferredRuntimeOwnershipSeal,
    ) -> Result<Self, EnqueueError> {
        let ownership = Self {
            owner,
            candidate_semantic_statement: None,
            deferred_admission_ordinal,
            current_ingress,
            source_physical_ordinal,
            physical_cut,
            runtime_seal,
        };
        ownership
            .validate_exact()
            .then_some(ownership)
            .ok_or(EnqueueError::FailClosed)
    }

    fn validate_exact(&self) -> bool {
        let source_matches_root = match self.owner.causal_origin().root_ingress_physical_ownership {
            Some(root) => {
                self.source_physical_ordinal == Some(root.source_ordinal)
                    && self.physical_cut == root.physical_cut
            }
            None => self.source_physical_ordinal.is_none(),
        };
        let current_ingress_is_exact = match self.current_ingress {
            RuntimeDispatchIngress::DirectAuthenticated => self
                .owner
                .causal_origin()
                .root_ingress_physical_ownership
                .is_some(),
            RuntimeDispatchIngress::LocalOrCausal => true,
        };
        self.physical_cut != 0
            && self.owner.validate_exact()
            && self
                .candidate_semantic_statement
                .is_none_or(RuntimeCandidateSemanticStatement::validate_exact)
            && self.runtime_seal.admission_ordinal() == self.deferred_admission_ordinal
            && self.runtime_seal.matches_runtime_owner(
                &self.owner.causal_origin().lifecycle_key,
                self.owner.lifecycle_ordinal(),
                self.current_ingress == RuntimeDispatchIngress::DirectAuthenticated,
                self.source_physical_ordinal,
                self.physical_cut,
            )
            && source_matches_root
            && current_ingress_is_exact
            && self
                .source_physical_ordinal
                .is_none_or(|ordinal| u128::from(ordinal) < self.physical_cut)
    }

    fn with_candidate_semantic_statement(
        mut self,
        statement: Option<RuntimeCandidateSemanticStatement>,
    ) -> Result<Self, EnqueueError> {
        self.candidate_semantic_statement = statement;
        self.validate_exact()
            .then_some(self)
            .ok_or(EnqueueError::FailClosed)
    }

    fn validate_against_ingress(&self, ingress: Option<&RuntimeIngressOwnershipEvidence>) -> bool {
        if !self.validate_exact() {
            return false;
        }
        match (self.current_ingress, ingress) {
            (RuntimeDispatchIngress::DirectAuthenticated, Some(ingress)) => {
                let expected_lifecycle = match ingress.earliest_lifecycle_ordinal() {
                    Ok(Some(ordinal)) => ordinal.min(self.runtime_seal.initial_lifecycle_ordinal()),
                    Ok(None) => self.runtime_seal.initial_lifecycle_ordinal(),
                    Err(_) => return false,
                };
                ingress.validate_frozen_physical()
                    && self.owner.lifecycle_ordinal() == expected_lifecycle
                    && self.owner.causal_origin().root_ingress_identity
                        == Some(runtime_ingress_causal_origin_projection_hash(ingress))
                    && self
                        .owner
                        .causal_origin()
                        .root_ingress_physical_ownership
                        .is_some_and(|physical| {
                            ingress.contains_physical_carrier(physical) == Ok(true)
                        })
            }
            (RuntimeDispatchIngress::LocalOrCausal, None) => {
                self.owner.lifecycle_ordinal() == self.runtime_seal.initial_lifecycle_ordinal()
            }
            (RuntimeDispatchIngress::DirectAuthenticated, None)
            | (RuntimeDispatchIngress::LocalOrCausal, Some(_)) => false,
        }
    }

    fn validate_active_against_ingress(
        &self,
        ingress: Option<&RuntimeIngressOwnershipEvidence>,
        source: &DeferredAdmissionOrdinalSource,
    ) -> bool {
        self.runtime_seal.still_retained()
            && self.runtime_seal.belongs_to(source)
            && self.validate_against_ingress(ingress)
    }

    const fn owner(&self) -> &RuntimeLifecycleOwner {
        &self.owner
    }

    fn rebase_deferred_ingress(
        &self,
        lifecycle_ordinal: u128,
        ingress_identity: iroha_crypto::Hash,
    ) -> Result<Self, RuntimeIngressMergeError> {
        let owner = self
            .owner
            .rebase_deferred_ingress(lifecycle_ordinal, ingress_identity)?;
        let rebased = Self {
            owner,
            candidate_semantic_statement: self.candidate_semantic_statement,
            deferred_admission_ordinal: self.deferred_admission_ordinal,
            current_ingress: self.current_ingress,
            source_physical_ordinal: self.source_physical_ordinal,
            physical_cut: self.physical_cut,
            runtime_seal: self.runtime_seal.clone(),
        };
        rebased
            .validate_exact()
            .then_some(rebased)
            .ok_or(RuntimeIngressMergeError::Conflict)
    }
}

impl std::ops::Deref for RuntimeDeferredLifecycleOwnership {
    type Target = RuntimeLifecycleOwner;

    fn deref(&self) -> &Self::Target {
        self.owner()
    }
}

/// The formal producer-continuation partition permits at most one live
/// adapter-deferred occurrence for a logical lifecycle. Executor and FIFO
/// aliases may temporarily share a causal owner, but two distinct Busy
/// ordinals for that owner would make class rotation decide a stage tie which
/// the proved transition relation declares unreachable.
fn deferred_lifecycle_ordinals_are_unique(
    ownership: &BTreeMap<u128, RuntimeDeferredLifecycleOwnership>,
) -> bool {
    let mut logical_ordinals = BTreeSet::new();
    ownership
        .values()
        .all(|owner| logical_ordinals.insert(owner.owner().lifecycle_ordinal()))
}

/// One-owner, class-aware scheduling shell for Sumeragi v2.
pub(crate) struct SerializedV2Runtime<D: RuntimeDriver = SumeragiV2Adapter> {
    driver: D,
    ingress: BoundedIngress<D::Command>,
    deferred_ingress_ownership: BTreeMap<u128, RuntimeIngressOwnershipEvidence>,
    deferred_lifecycle_ownership: BTreeMap<u128, RuntimeDeferredLifecycleOwnership>,
    /// Latest receiver-local physical high-watermark published by the outer
    /// runner immediately before a serialized runtime turn.
    ingress_physical_cut: u128,
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
    /// Receiver-local ingress high-watermark frozen atomically with the
    /// current timeout owner. A dormant wire replay admitted at or beyond
    /// this cut cannot resurrect an older logical ordinal ahead of timeout.
    timeout_owner_physical_cut: Option<u128>,
    retransmit_owner: Option<RuntimeLifecycleOwner>,
    /// Receiver-local ingress high-watermark frozen atomically with the
    /// current periodic owner. Retries of the same clock episode retain this
    /// cut; a later physical replay cannot revive an older logical position
    /// ahead of the already-admitted periodic work.
    retransmit_owner_physical_cut: Option<u128>,
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

    #[cfg(test)]
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
            ingress_physical_cut: 1,
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
            timeout_owner_physical_cut: None,
            retransmit_owner: None,
            retransmit_owner_physical_cut: None,
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
        parent_statement: Option<&RuntimeCandidateSemanticStatement>,
        effects: &[D::Effect],
    ) -> Result<(), EnqueueError> {
        if effects.is_empty() {
            return Ok(());
        }
        if self.pending_effect_ownership.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        if effects.len() > MAX_EFFECTS_PER_STEP {
            return Err(EnqueueError::FailClosed);
        }
        let effect_count = u8::try_from(effects.len()).map_err(|_| EnqueueError::FailClosed)?;
        let candidate_semantics = effects
            .iter()
            .map(|effect| {
                let causality = if source == RuntimeEffectSource::Startup {
                    RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::StartupRecovery)
                } else {
                    D::effect_causality(effect, source)
                };
                D::effect_candidate_semantic_binding(
                    effect,
                    matches!(causality, RuntimeEffectCausality::Inherit)
                        .then_some(parent_statement)
                        .flatten(),
                )
                .map_err(|_| EnqueueError::FailClosed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let candidate_count_usize = candidate_semantics
            .iter()
            .filter(|candidate| candidate.is_some())
            .count();
        if candidate_count_usize > MAX_CAUSAL_SUCCESSORS_PER_COMMAND {
            return Err(EnqueueError::FailClosed);
        }
        let candidate_count =
            u8::try_from(candidate_count_usize).map_err(|_| EnqueueError::FailClosed)?;
        let mut ownership = Vec::with_capacity(effects.len());
        let mut candidate_position = 0u8;
        for (index, (effect, candidate)) in
            effects.iter().zip(candidate_semantics.iter()).enumerate()
        {
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
            if candidate.is_some() {
                candidate_position = candidate_position
                    .checked_add(1)
                    .ok_or(EnqueueError::FailClosed)?;
            }
            let effect_position = u8::try_from(index + 1).map_err(|_| EnqueueError::FailClosed)?;
            let evidence = evidence.bind_runtime_effect(
                matches!(causality, RuntimeEffectCausality::Inherit)
                    .then_some(parent)
                    .flatten(),
                D::effect_refinement_kind(effect),
                &D::effect_semantic_identity(effect),
                candidate.as_ref(),
                effect_position,
                effect_count,
                candidate.as_ref().map_or(0, |_| candidate_position),
                candidate_count,
            )?;
            if !evidence.validate_bound_exact() {
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
        let Some(ownership) = self.pending_effect_ownership.take() else {
            self.latch_fail_closed("effect batch omitted its lifecycle ownership");
            return Err("Sumeragi v2 effect batch omitted its lifecycle ownership".to_owned());
        };
        if ownership.len() != effect_count
            || ownership
                .iter()
                .any(|evidence| !evidence.validate_bound_exact())
        {
            self.latch_fail_closed("effect lifecycle ownership did not match its batch");
            return Err("Sumeragi v2 effect lifecycle ownership was invalid".to_owned());
        }
        Ok(ownership)
    }

    /// Publish the receiver-local physical admission high-watermark before a
    /// serialized runtime turn. The value may only advance; refreshing an
    /// already-created continuation is forbidden because each continuation
    /// copies the current value exactly once at reservation.
    pub(crate) fn set_ingress_physical_cut(&mut self, physical_cut: u128) -> Result<(), String> {
        if physical_cut == 0 || physical_cut < self.ingress_physical_cut {
            self.latch_fail_closed("receiver physical admission cut regressed or was zero");
            return Err("Sumeragi v2 receiver physical admission cut was invalid".to_owned());
        }
        self.ingress_physical_cut = physical_cut;
        Ok(())
    }

    fn validate_clock_owner_physical_cuts(&self) -> Result<(), EnqueueError> {
        let timeout_is_paired =
            self.timeout_owner.is_some() == self.timeout_owner_physical_cut.is_some();
        let retransmit_is_paired =
            self.retransmit_owner.is_some() == self.retransmit_owner_physical_cut.is_some();
        let cuts_are_valid = self
            .timeout_owner_physical_cut
            .into_iter()
            .chain(self.retransmit_owner_physical_cut)
            .all(|cut| cut != 0 && cut <= self.ingress_physical_cut);
        if timeout_is_paired && retransmit_is_paired && cuts_are_valid {
            Ok(())
        } else {
            Err(EnqueueError::FailClosed)
        }
    }

    fn clock_owner_physical_cut_for(
        &self,
        parent: &RuntimeLifecycleOwner,
    ) -> Result<Option<u128>, EnqueueError> {
        self.validate_clock_owner_physical_cuts()?;
        let timeout_matches = self.timeout_owner.as_ref() == Some(parent);
        let retransmit_matches = self.retransmit_owner.as_ref() == Some(parent);
        match (timeout_matches, retransmit_matches) {
            (true, false) => self
                .timeout_owner_physical_cut
                .map(Some)
                .ok_or(EnqueueError::FailClosed),
            (false, true) => self
                .retransmit_owner_physical_cut
                .map(Some)
                .ok_or(EnqueueError::FailClosed),
            (false, false) => Ok(None),
            (true, true) => Err(EnqueueError::FailClosed),
        }
    }

    /// Whether one physically later occurrence would resurrect a logical
    /// position at or ahead of an already-frozen clock owner.
    ///
    /// The occurrence stays in its existing fair-ingress or executor owner
    /// and is retried after the clock episode transfers.  It must not enter
    /// the FIFO, where the persistent `fifo_owed` bit could otherwise let a
    /// different, post-cut identity inherit an earlier command's debt.
    fn clock_owner_reservation_blocks_occurrence(
        &self,
        lifecycle_ordinal: u128,
        source_physical_ordinal: u64,
    ) -> Result<bool, EnqueueError> {
        if lifecycle_ordinal == 0 || source_physical_ordinal == 0 {
            return Err(EnqueueError::FailClosed);
        }
        self.validate_clock_owner_physical_cuts()?;
        let occurrence_is_blocked = |owner: &RuntimeLifecycleOwner, physical_cut: u128| {
            u128::from(source_physical_ordinal) >= physical_cut
                && lifecycle_ordinal <= owner.lifecycle_ordinal()
        };
        Ok(self
            .timeout_owner
            .as_ref()
            .zip(self.timeout_owner_physical_cut)
            .is_some_and(|(owner, physical_cut)| occurrence_is_blocked(owner, physical_cut))
            || self
                .retransmit_owner
                .as_ref()
                .zip(self.retransmit_owner_physical_cut)
                .is_some_and(|(owner, physical_cut)| occurrence_is_blocked(owner, physical_cut)))
    }

    fn clock_owner_reservation_blocks(
        &self,
        owner: &RuntimeLifecycleOwner,
    ) -> Result<bool, EnqueueError> {
        if !owner.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        let Some(physical) = owner.causal_origin().root_ingress_physical_ownership else {
            self.validate_clock_owner_physical_cuts()?;
            return Ok(false);
        };
        self.clock_owner_reservation_blocks_occurrence(
            owner.lifecycle_ordinal(),
            physical.source_ordinal,
        )
    }

    fn enqueue_after_clock_reservation(
        &mut self,
        command: TaggedCommand<D::Command>,
    ) -> Result<(), EnqueueError> {
        // Fresh commands receive their lifecycle ordinal atomically with
        // physical FIFO admission below. Only a restored or inherited owner
        // can carry an older logical position across the frozen clock cut.
        if command.lifecycle_ordinal.is_some() {
            let owner = command.lifecycle_owner()?;
            if self.clock_owner_reservation_blocks(&owner)? {
                return Err(EnqueueError::Full);
            }
        } else {
            // The queue will mint this owner's ordinal, but corrupt clock
            // owner/cut pairing must still fail closed before publication.
            self.validate_clock_owner_physical_cuts()?;
        }
        self.ingress.enqueue(command)
    }

    /// Replace the bounded set of runnable owners currently held by retained
    /// executor effects or asynchronous Sign/Store/Validate/Apply tasks.
    ///
    /// The executor derives this set from its existing bounded maps before
    /// each runtime step. Supplying a forged carrier or exceeding the existing
    /// pending-work plus one retained-batch bound fails closed.
    /// A network-waiting Fetch remains executor-owned but is intentionally
    /// passive here; its exact owner returns with `BodyAvailable` so the wait
    /// itself cannot block the control traffic needed to finish or supersede it.
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

    /// Return the number of runnable external owners published to the runtime.
    #[cfg(test)]
    pub(crate) fn external_lifecycle_owner_count(&self) -> usize {
        self.external_lifecycle_owners.len()
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
            let effect = AdapterEffect::StoreBody {
                tag,
                round: manifest.round,
                subject: manifest.subject,
            };
            return bind_adapter_effect_batch_ownership(
                std::slice::from_ref(&effect),
                vec![RuntimeEffectOwnership::inherited(ownership.owner().clone())],
            )?
            .pop()
            .ok_or_else(|| "local proposal StoreBody binding was empty".to_owned());
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
        let effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh(
                owner,
                RuntimeFreshRootKind::LocalProposalAdmission,
            )],
        )?
        .pop()
        .ok_or_else(|| "local proposal StoreBody binding was empty".to_owned())
    }

    /// Return whether the runner may begin one local proposal for this view.
    ///
    /// An armed leader reservation is deliberately one-shot: guarded Proposal
    /// fanout consumes it, while a later timeout or `EnterView` owns the next
    /// progress transition. A same-view lock update may make the runner's
    /// candidate state eligible again, but it must not turn that scheduling
    /// churn into a second Proposal. Report the consumed reservation as
    /// ordinary backpressure so the pacemaker can advance the view. Corrupt or
    /// mismatched reservations remain fatal, and the actual admission path
    /// below still fails closed if a caller bypasses this preflight.
    pub(crate) fn local_proposal_admission_available(
        &mut self,
        tag: EventTag,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        if tag != self.round_tag {
            self.latch_fail_closed(
                "local proposal admission preflight changed the authoritative tag",
            );
            return Err("Sumeragi v2 local proposal preflight tag was invalid".to_owned());
        }
        let Some(reservation) = self.active_view_producer.as_ref() else {
            return Ok(!self.clocks_armed);
        };
        if reservation.tag != tag || !reservation.ownership.validate_exact() {
            self.latch_fail_closed("local proposal admission preflight changed its producer");
            return Err("Sumeragi v2 local proposal preflight producer was invalid".to_owned());
        }
        Ok(true)
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
        {
            self.latch_fail_closed("Proposal fanout changed its active-view producer");
            return Err("Sumeragi v2 Proposal fanout changed producer ownership".to_owned());
        }

        if reservation.ownership.owner() != ownership.owner() {
            let owner = ownership.owner();
            let fresh_kind = self
                .dormant_fresh_lifecycle_owners
                .iter()
                .find_map(|((kind, _), candidate)| (candidate == owner).then_some(*kind));
            if owner.causal_origin().root_tag != reservation.tag {
                self.latch_fail_closed("Proposal fanout changed its active-view producer");
                return Err("Sumeragi v2 Proposal fanout changed producer ownership".to_owned());
            }
            match fresh_kind {
                // Recovery can restore a durable Proposal signing request before
                // live clocks mint the process-local producer reservation. Its
                // eventual fanout is the original Proposal terminal, not a
                // competing producer, so it consumes the reservation.
                Some(RuntimeFreshRootKind::StartupRecovery) => {}
                // Periodic control retransmission has its own scheduler owner.
                // It neither creates nor completes the active view's one-shot
                // local Proposal producer.
                Some(RuntimeFreshRootKind::Retransmit) => return Ok(()),
                Some(
                    RuntimeFreshRootKind::Timeout
                    | RuntimeFreshRootKind::HistoricalLockedRetransmit
                    | RuntimeFreshRootKind::LocalProposalAdmission,
                )
                | None => {
                    self.latch_fail_closed("Proposal fanout changed its active-view producer");
                    return Err("Sumeragi v2 Proposal fanout changed producer ownership".to_owned());
                }
            }
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
            | RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. }) => Ok(preflight),
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
        if preflight.is_coalescence() {
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

    fn owned_preflight_is_coalesced(
        &mut self,
        tag: EventTag,
        preflight: RuntimeCommandAdmissionPreflight,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<bool, EnqueueError> {
        match preflight {
            RuntimeCommandAdmissionPreflight::CoalesceOwned {
                causal_lifecycle_key,
                admission_ordinal,
            } => {
                let owner = ownership.owner();
                if owner.causal_origin().lifecycle_key != causal_lifecycle_key
                    || owner.lifecycle_ordinal() != admission_ordinal
                {
                    self.latch_fail_closed(
                        "coalesced runtime command changed its retained lifecycle owner",
                    );
                    return Err(EnqueueError::FailClosed);
                }
                Ok(true)
            }
            RuntimeCommandAdmissionPreflight::Coalesce if tag != self.driver.current_tag() => {
                Ok(true)
            }
            RuntimeCommandAdmissionPreflight::Coalesce => {
                self.latch_fail_closed(
                    "owned runtime command reached current-tag coalescence without an exact owner",
                );
                Err(EnqueueError::FailClosed)
            }
            RuntimeCommandAdmissionPreflight::Admit
            | RuntimeCommandAdmissionPreflight::ReuseDormant { .. }
            | RuntimeCommandAdmissionPreflight::Reject => Ok(false),
        }
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
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => return Ok(()),
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
        let result = self.enqueue_after_clock_reservation(tagged);
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
        if self.owned_preflight_is_coalesced(tag, preflight, ownership)? {
            return Ok(());
        }
        let mut tagged = match preflight {
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
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
                unreachable!("handled above")
            }
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        tagged.candidate_semantic_statement = ownership.candidate_semantic_statement();
        if !tagged.validate_admission_identity() {
            self.latch_fail_closed("causal-successor candidate statement was invalid");
            return Err(EnqueueError::FailClosed);
        }
        let result = self.enqueue_after_clock_reservation(tagged);
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
            if !active.contains(&ordinal) || !candidate.validate_frozen_physical() {
                return Err(RuntimeIngressMergeError::Conflict);
            }
            match retained.get_mut(&ordinal) {
                Some(existing) => {
                    let previous_lifecycle = existing.earliest_lifecycle_ordinal()?;
                    existing.merge_downstream(candidate)?;
                    if !existing.validate_frozen_physical() {
                        return Err(RuntimeIngressMergeError::Conflict);
                    }
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
            || retained
                .values()
                .any(|ownership| !ownership.validate_frozen_physical())
            || !active.iter().all(|ordinal| retained.contains_key(ordinal))
        {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        if !deferred_lifecycle_ordinals_are_unique(&lifecycle_ownership)
            || lifecycle_ownership.iter().any(|(ordinal, ownership)| {
                ownership.deferred_admission_ordinal != *ordinal
                    || !ownership.validate_active_against_ingress(
                        retained.get(ordinal),
                        self.driver.deferred_admission_ordinal_source(),
                    )
            })
        {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        self.deferred_ingress_ownership = retained;
        self.deferred_lifecycle_ownership = lifecycle_ownership;
        Ok(())
    }

    /// Atomically prune runtime wrappers whose adapter-owned Busy occurrence
    /// was retired outside ordinary deferred service.
    ///
    /// Test seam helpers may stage an adapter input before installing its
    /// runtime wrapper, so absence is tolerated here. Every wrapper which is
    /// present must still have its exact live adapter ordinal, provenance,
    /// ingress carrier, and private seal. The scheduler's ordinary entry
    /// validation continues to require complete wrapper coverage before any
    /// such staged input can receive service.
    fn reconcile_deferred_runtime_ownership_after_retirement(
        &mut self,
    ) -> Result<(), RuntimeIngressMergeError> {
        let active = self.driver.all_deferred_admission_ordinals();
        let authenticated = self.driver.authenticated_deferred_admission_ordinals();
        if !authenticated.is_subset(&active) {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        let mut lifecycle = self.deferred_lifecycle_ownership.clone();
        lifecycle.retain(|ordinal, _| active.contains(ordinal));
        let mut ingress = self.deferred_ingress_ownership.clone();
        ingress.retain(|ordinal, _| authenticated.contains(ordinal));
        if !deferred_lifecycle_ordinals_are_unique(&lifecycle)
            || ingress.iter().any(|(ordinal, ownership)| {
                !ownership.validate_frozen_physical() || !lifecycle.contains_key(ordinal)
            })
            || lifecycle.iter().any(|(ordinal, owner)| {
                owner.deferred_admission_ordinal != *ordinal
                    || !owner.validate_active_against_ingress(
                        ingress.get(ordinal),
                        self.driver.deferred_admission_ordinal_source(),
                    )
            })
        {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        self.deferred_lifecycle_ownership = lifecycle;
        self.deferred_ingress_ownership = ingress;
        self.retire_orphaned_leader_wire_runtime_receipts()
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
        if !ownership.validate_frozen_physical() {
            return Err(RuntimeIngressMergeError::Conflict);
        }
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

    /// Terminalize the selected leader-wire lifecycle, then any different
    /// lifecycle which the same reducer macro-step retired from Busy storage.
    ///
    /// The selected owner must be classified first: pruning receipts before
    /// its producer handoff is acknowledged would misclassify that live
    /// parent as an unrelated volatile orphan. Adapter-side cleanup can also
    /// remove other deferred occurrences (for example, a conflicting proposal
    /// after `BodyAvailable`), so the second pass closes their durable Runtime
    /// records once their scheduler wrappers have disappeared.
    fn complete_driver_dispatch_leader_wire_owners(
        &mut self,
        parent: &RuntimeLifecycleOwner,
        retained_parent: bool,
        handoff: Option<(
            ProducerContinuationHandoffEvidence,
            ProducerContinuationTerminalToken,
        )>,
    ) -> Result<(), RuntimeError<D::Error>> {
        if !retained_parent {
            self.complete_leader_wire_runtime_owner(parent, handoff)?;
        }
        if self.retire_orphaned_leader_wire_runtime_receipts().is_err() {
            self.latch_fail_closed(
                "driver dispatch left invalid orphaned leader-wire runtime ownership",
            );
            return Err(RuntimeError::FailClosed);
        }
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
        parent_statement: Option<RuntimeCandidateSemanticStatement>,
        current_ingress: RuntimeDispatchIngress,
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
        let clock_physical_cut = match self.clock_owner_physical_cut_for(parent) {
            Ok(cut) => cut,
            Err(_) => {
                self.latch_fail_closed(
                    "driver dispatch observed an unpaired or ambiguous clock physical cut",
                );
                return Err(RuntimeError::FailClosed);
            }
        };
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
                Some(existing)
                    if existing.deferred_admission_ordinal != ordinal
                        || existing.owner() != parent
                        || existing.candidate_semantic_statement != parent_statement =>
                {
                    self.latch_fail_closed("deferred ordinal changed its lifecycle owner");
                    return Err(RuntimeError::FailClosed);
                }
                Some(_) => {}
                None => {
                    if retained.values().any(|existing| {
                        existing.owner().lifecycle_ordinal() == parent.lifecycle_ordinal()
                    }) {
                        self.latch_fail_closed(
                            "a logical lifecycle acquired two active deferred occurrences",
                        );
                        return Err(RuntimeError::FailClosed);
                    }
                    let retained_ingress = self.deferred_ingress_ownership.get(&ordinal);
                    let (source_physical_ordinal, physical_cut) = match (
                        current_ingress,
                        parent.causal_origin().root_ingress_physical_ownership,
                        retained_ingress,
                    ) {
                        (
                            RuntimeDispatchIngress::DirectAuthenticated,
                            Some(ownership),
                            Some(ingress),
                        ) if ingress.contains_physical_carrier(ownership) == Ok(true)
                            && parent.causal_origin().root_ingress_identity
                                == Some(runtime_ingress_causal_origin_projection_hash(ingress)) =>
                        {
                            (Some(ownership.source_ordinal), ownership.physical_cut)
                        }
                        (RuntimeDispatchIngress::LocalOrCausal, Some(ownership), None) => {
                            (Some(ownership.source_ordinal), ownership.physical_cut)
                        }
                        (RuntimeDispatchIngress::LocalOrCausal, None, None) => (
                            None,
                            clock_physical_cut.unwrap_or(self.ingress_physical_cut),
                        ),
                        _ => {
                            self.latch_fail_closed(
                                "deferred ingress lineage changed its current or root physical carrier",
                            );
                            return Err(RuntimeError::FailClosed);
                        }
                    };
                    let runtime_seal = match self.driver.seal_deferred_runtime_ownership(
                        ordinal,
                        parent,
                        current_ingress,
                        source_physical_ordinal,
                        physical_cut,
                    ) {
                        Ok(seal) => seal,
                        Err(error) => return Err(self.close(error)),
                    };
                    let Some(occurrence) = self.driver.deferred_occurrence_ownership(ordinal)
                    else {
                        self.latch_fail_closed(
                            "new deferred owner lost its adapter occurrence after sealing",
                        );
                        return Err(RuntimeError::FailClosed);
                    };
                    if !occurrence.belongs_to(self.driver.deferred_admission_ordinal_source())
                        || !occurrence.matches_retained_runtime_ownership_seal(&runtime_seal)
                    {
                        self.latch_fail_closed(
                            "new deferred owner received a foreign or mismatched runtime seal",
                        );
                        return Err(RuntimeError::FailClosed);
                    }
                    let ownership = RuntimeDeferredLifecycleOwnership::new(
                        parent.clone(),
                        ordinal,
                        current_ingress,
                        source_physical_ordinal,
                        physical_cut,
                        runtime_seal,
                    )
                    .and_then(|ownership| {
                        ownership.with_candidate_semantic_statement(parent_statement)
                    })
                    .map_err(|_| {
                        self.latch_fail_closed(
                            "deferred lifecycle did not retain a valid physical ingress cut",
                        );
                        RuntimeError::FailClosed
                    })?;
                    retained.insert(ordinal, ownership);
                }
            }
        }
        retained.retain(|ordinal, _| active.contains(ordinal));
        if retained.len() != active.len()
            || !deferred_lifecycle_ordinals_are_unique(&retained)
            || retained.iter().any(|(ordinal, owner)| {
                owner.deferred_admission_ordinal != *ordinal
                    || !owner.validate_active_against_ingress(
                        self.deferred_ingress_ownership.get(ordinal),
                        self.driver.deferred_admission_ordinal_source(),
                    )
            })
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
        self.validate_clock_owner_physical_cuts()?;
        if !self.clocks_armed {
            return Ok(());
        }
        let raw_timeout_due = !self.timeout_emitted
            && now.saturating_duration_since(self.round_started_at)
                >= round_timeout_for_view(self.base_round_timeout, self.round_tag.view());
        if raw_timeout_due && self.timeout_owner.is_none() {
            if self.timeout_owner_physical_cut.is_some() {
                return Err(EnqueueError::FailClosed);
            }
            let owner = self.mint_fresh_lifecycle_owner(
                self.round_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::Timeout,
                b"begin-timeout",
            )?;
            self.timeout_owner_physical_cut = Some(self.ingress_physical_cut);
            self.timeout_owner = Some(owner);
        }
        let raw_retransmit_due =
            now.saturating_duration_since(self.retransmit_started_at) >= self.retransmit_interval;
        // A periodic owner frozen before the absolute deadline may complete
        // its one bounded episode. Once that episode drains, do not replenish
        // the cached lower-ordinal root while the one-shot timeout is still
        // waiting to emit: otherwise every late call can recreate the same
        // older owner and starve the frozen timeout forever. `raw_timeout_due`
        // is false immediately after timeout emission, so post-timeout
        // TimeoutVote and decided-body recovery remain fully enabled.
        if raw_retransmit_due && !raw_timeout_due && self.retransmit_owner.is_none() {
            if self.retransmit_owner_physical_cut.is_some() {
                return Err(EnqueueError::FailClosed);
            }
            let retransmit_origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
                self.round_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::Retransmit,
                b"periodic-retransmit",
            );
            let retransmit_cache_key = (
                RuntimeFreshRootKind::Retransmit,
                retransmit_origin.lifecycle_key,
            );
            if let Some(prior_episode) = self
                .dormant_fresh_lifecycle_owners
                .get(&retransmit_cache_key)
                .cloned()
            {
                // The cache coalesces retries only while this exact episode
                // still owns concrete queue/deferred/effect work. Once every
                // alias drains, a later wall-clock tick is a new physical
                // producer episode and must take a fresh actor-global
                // position. Reinstalling `prior_episode` would resurrect its
                // old ordinal ahead of work admitted after the last tick.
                if self.active_lifecycle_uses_ordinal(prior_episode.lifecycle_ordinal())? {
                    return Ok(());
                }
                self.dormant_fresh_lifecycle_owners
                    .remove(&retransmit_cache_key);
            }
            let owner = self.mint_fresh_lifecycle_owner(
                self.round_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::Retransmit,
                b"periodic-retransmit",
            )?;
            // A prior occurrence of this exact cached timer root may already
            // be retained by the adapter's Busy-deferred queue. Installing a
            // second runtime alias would make the same immutable owner compete
            // with itself and keep the nondeferred alias at the global
            // minimum while its signing dependency waits.
            if self.active_lifecycle_uses_ordinal(owner.lifecycle_ordinal())? {
                return Ok(());
            }
            self.retransmit_owner_physical_cut = Some(self.ingress_physical_cut);
            self.retransmit_owner = Some(owner);
        }
        Ok(())
    }

    fn minimum_active_lifecycle_ordinal(&self) -> Result<Option<u128>, EnqueueError> {
        self.minimum_active_lifecycle_ordinal_excluding(&[])
    }

    /// Return the oldest exact active owner after removing only aliases of the
    /// supplied blocked adapter-deferred set.
    fn minimum_active_lifecycle_ordinal_excluding(
        &self,
        excluded: &[RuntimeLifecycleOwner],
    ) -> Result<Option<u128>, EnqueueError> {
        // Deeply validate every physical FIFO owner and every restart-dormant
        // local producer reservation before an exclusion can affect rank.
        // Dormant reservations have no command whose reducer path can be
        // proved fence-blocked, so they remain unconditional predecessors.
        let _ = self.ingress.oldest_active_lifecycle_ordinal()?;
        let mut minimum = self
            .ingress
            .dormant_local_fifo_reservations
            .iter()
            .map(|reservation| reservation.admission_ordinal)
            .min();
        let mut observe = |owner: &RuntimeLifecycleOwner| -> Result<(), EnqueueError> {
            if !owner.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            if excluded.iter().any(|blocked| blocked == owner) {
                return Ok(());
            }
            minimum = Some(minimum.map_or(owner.lifecycle_ordinal(), |ordinal| {
                ordinal.min(owner.lifecycle_ordinal())
            }));
            Ok(())
        };
        for queued in &self.ingress.commands {
            observe(&queued.lifecycle_owner()?)?;
        }
        for (ordinal, owner) in &self.deferred_lifecycle_ownership {
            if owner.deferred_admission_ordinal != *ordinal
                || !owner.validate_active_against_ingress(
                    self.deferred_ingress_ownership.get(ordinal),
                    self.driver.deferred_admission_ordinal_source(),
                )
            {
                return Err(EnqueueError::FailClosed);
            }
            observe(owner.owner())?;
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

    /// Return the oldest owner which was physically present before a frozen
    /// receiver-local cut. Logical ordinals retained by a later wire replay
    /// cannot become predecessors merely because their semantic identity is
    /// old. Local owners have no ingress occurrence and remain ordered solely
    /// by their immutable lifecycle ordinal.
    fn minimum_active_lifecycle_ordinal_before_physical_cut_excluding(
        &self,
        physical_cut: u128,
        excluded: &[RuntimeLifecycleOwner],
    ) -> Result<Option<u128>, EnqueueError> {
        let mut minimum = self
            .ingress
            .oldest_active_lifecycle_ordinal_before_physical_cut_excluding(
                physical_cut,
                excluded,
            )?;
        let mut observe = |owner: &RuntimeLifecycleOwner| -> Result<(), EnqueueError> {
            if !owner.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            if excluded.iter().any(|blocked| blocked == owner)
                || owner.is_post_physical_cut(physical_cut)
            {
                return Ok(());
            }
            minimum = Some(minimum.map_or(owner.lifecycle_ordinal(), |ordinal| {
                ordinal.min(owner.lifecycle_ordinal())
            }));
            Ok(())
        };
        for (ordinal, owner) in &self.deferred_lifecycle_ownership {
            if owner.deferred_admission_ordinal != *ordinal
                || !owner.validate_active_against_ingress(
                    self.deferred_ingress_ownership.get(ordinal),
                    self.driver.deferred_admission_ordinal_source(),
                )
            {
                return Err(EnqueueError::FailClosed);
            }
            observe(owner.owner())?;
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

    /// Global logical minimum as observed by one already-admitted deferred
    /// continuation.
    ///
    /// All ordinary owners retain the established logical ordering. An
    /// authenticated ingress or derived deferred continuation whose source
    /// occurrence is at or after this continuation's frozen cut is physically
    /// later and cannot resurrect an older logical queue position.
    #[cfg(test)]
    fn minimum_active_lifecycle_ordinal_for_deferred(
        &self,
        target: &RuntimeDeferredLifecycleOwnership,
    ) -> Result<Option<u128>, EnqueueError> {
        self.minimum_active_lifecycle_ordinal_for_deferred_excluding(target, &[])
    }

    fn minimum_active_lifecycle_ordinal_for_deferred_excluding(
        &self,
        target: &RuntimeDeferredLifecycleOwnership,
        excluded: &[RuntimeLifecycleOwner],
    ) -> Result<Option<u128>, EnqueueError> {
        if !target.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        let mut minimum = self
            .ingress
            .oldest_active_lifecycle_ordinal_before_physical_cut_excluding(
                target.physical_cut,
                excluded,
            )?;
        let mut observe = |owner: &RuntimeLifecycleOwner| -> Result<(), EnqueueError> {
            if !owner.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            if excluded.iter().any(|excluded| excluded == owner) {
                return Ok(());
            }
            if owner.is_post_physical_cut(target.physical_cut) {
                return Ok(());
            }
            minimum = Some(minimum.map_or(owner.lifecycle_ordinal(), |ordinal| {
                ordinal.min(owner.lifecycle_ordinal())
            }));
            Ok(())
        };
        for (ordinal, owner) in &self.deferred_lifecycle_ownership {
            if owner.deferred_admission_ordinal != *ordinal
                || !owner.validate_active_against_ingress(
                    self.deferred_ingress_ownership.get(ordinal),
                    self.driver.deferred_admission_ordinal_source(),
                )
            {
                return Err(EnqueueError::FailClosed);
            }
            observe(owner.owner())?;
        }
        if !self
            .deferred_lifecycle_ownership
            .values()
            .any(|owner| owner == target)
        {
            observe(target.owner())?;
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

    /// Adapter-deferred occurrences which may own the next runner turn under
    /// every active continuation's immutable physical cut.
    fn eligible_deferred_admission_ordinals(&self) -> Result<BTreeSet<u128>, EnqueueError> {
        // Pairwise target-relative precedence is not transitive when several
        // frozen physical intervals overlap.  First remove every occurrence
        // whose source is physically behind any active target.  Only then may
        // logical lifecycle rank select from the remaining acyclic pool.
        if !deferred_lifecycle_ordinals_are_unique(&self.deferred_lifecycle_ownership)
            || self.validate_clock_owner_physical_cuts().is_err()
        {
            return Err(EnqueueError::FailClosed);
        }
        for (admission_ordinal, candidate) in &self.deferred_lifecycle_ownership {
            if candidate.deferred_admission_ordinal != *admission_ordinal
                || !candidate.validate_active_against_ingress(
                    self.deferred_ingress_ownership.get(admission_ordinal),
                    self.driver.deferred_admission_ordinal_source(),
                )
            {
                return Err(EnqueueError::FailClosed);
            }
        }
        let physically_eligible =
            self.deferred_lifecycle_ownership
                .iter()
                .filter_map(|(admission_ordinal, candidate)| {
                    let physically_behind_an_active_target = candidate
                        .source_physical_ordinal
                        .is_some_and(|source_physical_ordinal| {
                            self.deferred_lifecycle_ownership.iter().any(
                                |(other_ordinal, target)| {
                                    other_ordinal != admission_ordinal
                                        && u128::from(source_physical_ordinal)
                                            >= target.physical_cut
                                },
                            ) || self.timeout_owner_physical_cut.is_some_and(|timeout_cut| {
                                u128::from(source_physical_ordinal) >= timeout_cut
                            }) || self
                                .retransmit_owner_physical_cut
                                .is_some_and(|retransmit_cut| {
                                    u128::from(source_physical_ordinal) >= retransmit_cut
                                })
                        });
                    (!physically_behind_an_active_target).then_some(*admission_ordinal)
                })
                .collect::<BTreeSet<_>>();
        let physically_ineligible_owners = self
            .deferred_lifecycle_ownership
            .iter()
            .filter(|(admission_ordinal, _)| !physically_eligible.contains(admission_ordinal))
            .map(|(_, ownership)| ownership.owner().clone())
            .collect::<Vec<_>>();

        let mut eligible = BTreeSet::new();
        for (admission_ordinal, candidate) in &self.deferred_lifecycle_ownership {
            if !physically_eligible.contains(admission_ordinal) {
                continue;
            }
            if self.minimum_active_lifecycle_ordinal_for_deferred_excluding(
                candidate,
                &physically_ineligible_owners,
            )? == Some(candidate.owner().lifecycle_ordinal())
            {
                eligible.insert(*admission_ordinal);
            }
        }
        Ok(eligible)
    }

    fn active_lifecycle_uses_ordinal(&self, lifecycle_ordinal: u128) -> Result<bool, EnqueueError> {
        if self.ingress.uses_lifecycle_ordinal(lifecycle_ordinal)? {
            return Ok(true);
        }
        let owner_matches = |owner: &RuntimeLifecycleOwner| {
            owner.validate_exact() && owner.lifecycle_ordinal() == lifecycle_ordinal
        };
        for (ordinal, owner) in &self.deferred_lifecycle_ownership {
            if owner.deferred_admission_ordinal != *ordinal
                || !owner.validate_active_against_ingress(
                    self.deferred_ingress_ownership.get(ordinal),
                    self.driver.deferred_admission_ordinal_source(),
                )
            {
                return Err(EnqueueError::FailClosed);
            }
            if owner_matches(owner.owner()) {
                return Ok(true);
            }
        }
        if self
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

    fn scheduler_arbitration_inputs(
        &self,
        now: Instant,
    ) -> Result<RuntimeSchedulerArbitrationInputs, EnqueueError> {
        self.validate_clock_owner_physical_cuts()?;
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
        let timeout_due = if !raw_timeout_due {
            false
        } else if let (Some(owner), Some(physical_cut)) =
            (&self.timeout_owner, self.timeout_owner_physical_cut)
        {
            !self
                .minimum_active_lifecycle_ordinal_before_physical_cut_excluding(
                    physical_cut,
                    std::slice::from_ref(owner),
                )?
                .is_some_and(|ordinal| ordinal < owner.lifecycle_ordinal())
        } else {
            false
        };
        let raw_periodic_timer_due = timers_enabled
            && now.saturating_duration_since(self.retransmit_started_at)
                >= self.retransmit_interval;
        let periodic_timer_due = if !raw_periodic_timer_due || timeout_due {
            false
        } else if let (Some(owner), Some(physical_cut)) =
            (&self.retransmit_owner, self.retransmit_owner_physical_cut)
        {
            !self
                .minimum_active_lifecycle_ordinal_before_physical_cut_excluding(
                    physical_cut,
                    std::slice::from_ref(owner),
                )?
                .is_some_and(|ordinal| ordinal < owner.lifecycle_ordinal())
        } else {
            false
        };
        Ok(RuntimeSchedulerArbitrationInputs {
            live_mode: timers_enabled,
            timeout_due,
            periodic_timer_due,
            fifo_ready,
            completion_ready,
            progress_ready,
            normal_ready,
            fence_completion_bypass: false,
            fence_predecessor_lifecycle_ordinal: None,
            fence_predecessor_ownership: None,
            fence_predecessor_ingress_ownership: None,
            fence_predecessor_occurrence_ownership: None,
        })
    }

    fn retain_scheduler_ownership(
        &mut self,
        selected: RuntimeSelectedOwnerKind,
        round_tag: EventTag,
        candidate: RuntimeSelectedCandidateOwnership,
        queue_before: RuntimeQueueOwnershipSnapshot,
        queue_after: RuntimeQueueOwnershipSnapshot,
        arbitration: RuntimeSchedulerArbitrationInputs,
        schedule_before: ScheduleState,
        schedule_after: ScheduleState,
    ) -> Result<(), RuntimeError<D::Error>> {
        if self.last_scheduler_ownership.is_some() {
            self.latch_fail_closed("a prior scheduler owner was not consumed");
            return Err(RuntimeError::FailClosed);
        }
        if let RuntimeSelectedCandidateOwnership::Exact(candidate) = &candidate
            && !candidate.selection_seal.claim_scheduler_handoff_once()
        {
            self.latch_fail_closed("FIFO selection capability was replayed or invalid");
            return Err(RuntimeError::FailClosed);
        }
        let mut evidence = RuntimeSchedulerOwnershipEvidence {
            selected,
            round_tag,
            candidate,
            queue_before: queue_before.projection,
            queue_after: queue_after.projection,
            queue_before_snapshot: queue_before,
            queue_after_snapshot: queue_after,
            live_mode: arbitration.live_mode,
            timeout_due: arbitration.timeout_due,
            periodic_timer_due: arbitration.periodic_timer_due,
            fifo_ready: arbitration.fifo_ready,
            completion_ready: arbitration.completion_ready,
            progress_ready: arbitration.progress_ready,
            normal_ready: arbitration.normal_ready,
            fence_completion_bypass: arbitration.fence_completion_bypass,
            fence_predecessor_lifecycle_ordinal: arbitration.fence_predecessor_lifecycle_ordinal,
            fence_predecessor_ownership: arbitration.fence_predecessor_ownership,
            fence_predecessor_ingress_ownership: arbitration.fence_predecessor_ingress_ownership,
            fence_predecessor_occurrence_ownership: arbitration
                .fence_predecessor_occurrence_ownership,
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
    /// Serviceable adapter debt is filtered first by each target's immutable
    /// physical cut, then by logical lifecycle rank inside the frozen pre-cut
    /// set. FIFO, timer, reservation, effect, and external owners retain the
    /// same logical ordering, so a newly due clock cannot overtake a previously
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

        // An older timer or ingress occurrence can already belong to the
        // adapter's Busy-deferred set while a later Sign effect owns the only
        // completion which can open that reducer fence. Immutable lifecycle
        // order alone would then select the older occurrence forever, observe
        // Busy again, and starve its dependency. Give only the exact causally
        // owned fence completion one bounded turn; every frozen timer and
        // scheduler debt remains intact for the immediately following call.
        if let Some(step) = self.dispatch_one_fence_dependency(now)? {
            return Ok(step);
        }

        // Work which already crossed runtime ingress and acquired the
        // adapter's Busy-deferred ownership competes by its frozen physical
        // cut and then by logical rank inside that retained predecessor set.
        // Once its WAL/signing fence opens, give exactly one eligible
        // transition a serialized turn. Each returned effect batch still
        // represents only one reducer macro-step.
        if let Some(step) = self.dispatch_one_adapter_deferred(now)? {
            return Ok(step);
        }

        let selected_round_tag = self.round_tag;
        let schedule_before = self.schedule;
        let queue_before = self.ingress.ownership_snapshot();
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

        let (
            effects,
            effect_source,
            effect_parent,
            effect_parent_statement,
            producer_handoff,
            retained_deferred_ingress,
        ) = match work {
            ScheduledWork::Timeout => {
                let queue_after = self.ingress.ownership_snapshot();
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
                        Ok(dispatch) => self.accept_driver_dispatch(
                            dispatch,
                            &owner,
                            None,
                            RuntimeDispatchIngress::LocalOrCausal,
                        )?,
                        Err(error) => return Err(self.close(error)),
                    };
                if retry_unadmitted {
                    self.latch_fail_closed(
                        "timeout backpressure had no physical command owner to retain",
                    );
                    return Err(RuntimeError::FailClosed);
                }
                if self.timeout_owner.as_ref() != Some(&owner) {
                    self.latch_fail_closed("timeout lifecycle reservation changed before transfer");
                    return Err(RuntimeError::FailClosed);
                }
                self.timeout_owner = None;
                (
                    effects,
                    RuntimeEffectSource::Timeout,
                    owner,
                    None,
                    producer_handoff,
                    retained_deferred_ingress,
                )
            }
            ScheduledWork::PeriodicTimer => {
                let queue_after = self.ingress.ownership_snapshot();
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
                let physical_cut = self.retransmit_owner_physical_cut.ok_or_else(|| {
                    self.latch_fail_closed("due retransmission had no frozen ingress physical cut");
                    RuntimeError::FailClosed
                })?;
                if let Err(error) = self.driver.bind_selected_producer_lifecycle(&owner) {
                    return Err(self.close(error));
                }
                let dispatch = self.driver.retransmit_elapsed(self.round_tag);
                self.driver.clear_selected_producer_lifecycle();
                let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
                    match dispatch {
                        Ok(dispatch) => self.accept_driver_dispatch(
                            dispatch,
                            &owner,
                            None,
                            RuntimeDispatchIngress::LocalOrCausal,
                        )?,
                        Err(error) => return Err(self.close(error)),
                    };
                if retry_unadmitted {
                    self.latch_fail_closed(
                        "retransmission backpressure had no physical command owner to retain",
                    );
                    return Err(RuntimeError::FailClosed);
                }
                if self.retransmit_owner.as_ref() != Some(&owner)
                    || self.retransmit_owner_physical_cut != Some(physical_cut)
                {
                    self.latch_fail_closed(
                            "retransmission lifecycle reservation or physical cut changed before transfer",
                        );
                    return Err(RuntimeError::FailClosed);
                }
                self.retransmit_owner = None;
                (
                    effects,
                    RuntimeEffectSource::Retransmit,
                    owner,
                    None,
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
                        self.latch_fail_closed("selected FIFO lifecycle owner was inconsistent");
                        return Err(RuntimeError::FailClosed);
                    }
                };
                let current_ingress = if command.ingress_ownership.is_some() {
                    RuntimeDispatchIngress::DirectAuthenticated
                } else {
                    RuntimeDispatchIngress::LocalOrCausal
                };
                let parent_statement = command.candidate_semantic_statement;
                let retry_command = command.clone();
                let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
                    match self.driver.dispatch(command) {
                        Ok(dispatch) => self.accept_driver_dispatch(
                            dispatch,
                            &owner,
                            parent_statement,
                            current_ingress,
                        )?,
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
                    let queue_after = self.ingress.ownership_snapshot();
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
                let queue_after = self.ingress.ownership_snapshot();
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
                    parent_statement,
                    producer_handoff,
                    retained_deferred_ingress,
                )
            }
            ScheduledWork::Idle => {
                let queue_after = self.ingress.ownership_snapshot();
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
            .retain_effect_ownership(
                effect_source,
                Some(&effect_parent),
                effect_parent_statement.as_ref(),
                &effects,
            )
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
        match effect_source {
            RuntimeEffectSource::Timeout => self.timeout_owner_physical_cut = None,
            RuntimeEffectSource::Retransmit => self.retransmit_owner_physical_cut = None,
            RuntimeEffectSource::Startup
            | RuntimeEffectSource::Fifo
            | RuntimeEffectSource::Deferred => {}
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
        self.complete_driver_dispatch_leader_wire_owners(
            &effect_parent,
            retained_deferred_ingress,
            completed_producer_handoff,
        )?;
        if self.observe_effects(now, &effects).is_err() {
            self.latch_fail_closed("effect observation lost active-view producer ownership");
            return Err(RuntimeError::FailClosed);
        }
        Ok(RuntimeStep::Advanced(effects))
    }

    /// Dispatch the exact owned signature callback required by strictly
    /// older, currently unserviceable adapter debt.
    ///
    /// This is not Completion-class priority in general. The production
    /// driver must prove that the callback matches its active signing fence,
    /// the command must inherit a non-SignatureCompleted causal root, and at
    /// least one exact older deferred lifecycle must remain blocked. The
    /// completion must also be the oldest active lifecycle after excluding
    /// only queued owners which the adapter proves are blocked by this same
    /// fence. External tasks, active producers, reservations, and unrelated
    /// or serviceable FIFO owners cannot be bypassed. The ownership carrier
    /// records the exceptional dependency edge explicitly.
    fn dispatch_one_fence_dependency(
        &mut self,
        now: Instant,
    ) -> Result<Option<RuntimeStep<D::Effect>>, RuntimeError<D::Error>> {
        if self.driver.deferred_work_is_serviceable() {
            return Ok(None);
        }
        let active_deferred = self.driver.all_deferred_admission_ordinals();
        if active_deferred.is_empty() {
            return Ok(None);
        }
        if self.deferred_lifecycle_ownership.len() != active_deferred.len()
            || !active_deferred
                .iter()
                .all(|ordinal| self.deferred_lifecycle_ownership.contains_key(ordinal))
            || self
                .deferred_lifecycle_ownership
                .iter()
                .any(|(ordinal, owner)| {
                    owner.deferred_admission_ordinal != *ordinal
                        || !owner.validate_active_against_ingress(
                            self.deferred_ingress_ownership.get(ordinal),
                            self.driver.deferred_admission_ordinal_source(),
                        )
                })
        {
            self.latch_fail_closed("unserviceable deferred work lost lifecycle ownership");
            return Err(RuntimeError::FailClosed);
        }
        let eligible_deferred = self.eligible_deferred_admission_ordinals().map_err(|_| {
            self.latch_fail_closed("fence-completion deferred physical-cut ownership was invalid");
            RuntimeError::FailClosed
        })?;
        let Some((target_ordinal, target)) = eligible_deferred
            .iter()
            .filter_map(|ordinal| {
                self.deferred_lifecycle_ownership
                    .get(ordinal)
                    .map(|owner| (*ordinal, owner.clone()))
            })
            .min_by_key(|(ordinal, owner)| (owner.owner().lifecycle_ordinal(), *ordinal))
        else {
            return Ok(None);
        };
        let target_ingress_ownership = self
            .deferred_ingress_ownership
            .get(&target_ordinal)
            .cloned();
        let Some(target_occurrence_ownership) =
            self.driver.deferred_occurrence_ownership(target_ordinal)
        else {
            self.latch_fail_closed("fence target lost its adapter-issued occurrence capability");
            return Err(RuntimeError::FailClosed);
        };
        if !target_occurrence_ownership.still_retained()
            || target_occurrence_ownership.admission_ordinal() != target.deferred_admission_ordinal
            || target_occurrence_ownership.is_authenticated_ingress()
                != (target.current_ingress == RuntimeDispatchIngress::DirectAuthenticated)
            || !target_occurrence_ownership
                .matches_retained_runtime_ownership_seal(&target.runtime_seal)
            || !target.validate_active_against_ingress(
                target_ingress_ownership.as_ref(),
                self.driver.deferred_admission_ordinal_source(),
            )
        {
            self.latch_fail_closed("fence target changed its adapter occurrence provenance");
            return Err(RuntimeError::FailClosed);
        }
        let oldest_deferred_lifecycle = target.owner().lifecycle_ordinal();
        let blocked_deferred_owners = self
            .deferred_lifecycle_ownership
            .values()
            .map(|owner| owner.owner().clone())
            .collect::<Vec<_>>();
        let blocked_fifo_owners = self
            .ingress
            .fence_blocked_lifecycle_owners(|queued| {
                self.driver
                    .command_is_blocked_by_deferred_fence(queued.tag, &queued.command)
            })
            .map_err(|_| {
                self.latch_fail_closed("fence-blocked FIFO ownership was invalid");
                RuntimeError::FailClosed
            })?;
        let mut blocked_dependency_owners = blocked_deferred_owners;
        blocked_dependency_owners.extend(blocked_fifo_owners);
        let Some(oldest_unblocked_lifecycle) = self
            .minimum_active_lifecycle_ordinal_for_deferred_excluding(
                &target,
                &blocked_dependency_owners,
            )
            .map_err(|_| {
                self.latch_fail_closed("fence-completion successor ownership was invalid");
                RuntimeError::FailClosed
            })?
        else {
            return Ok(None);
        };

        let round_tag = self.round_tag;
        let queue_before = self.ingress.ownership_snapshot();
        let schedule = self.schedule;
        let mut arbitration = match self.scheduler_arbitration_inputs(now) {
            Ok(arbitration) => arbitration,
            Err(_) => {
                self.latch_fail_closed("fence-completion scheduler ownership was invalid");
                return Err(RuntimeError::FailClosed);
            }
        };
        // This exceptional dependency edge is deliberately outside ordinary
        // FIFO/timer arbitration. Keep those readiness claims closed in the
        // retained carrier so validation cannot confuse the bypass with a
        // normal scheduling result.
        arbitration.timeout_due = false;
        arbitration.periodic_timer_due = false;
        arbitration.fifo_ready = false;
        arbitration.completion_ready = false;
        arbitration.progress_ready = false;
        arbitration.normal_ready = false;
        let selected_result = {
            let driver = &self.driver;
            self.ingress.pop_fence_completion_with_ownership(|queued| {
                queued.lifecycle_ordinal.is_some_and(|ordinal| {
                    ordinal > oldest_deferred_lifecycle && ordinal == oldest_unblocked_lifecycle
                }) && queued.causal_origin.root_identity.kind
                    != RuntimeCommandKind::SignatureCompleted
                    && driver.completion_unblocks_deferred_fence(queued.tag, &queued.command)
            })
        };
        let selected = match selected_result {
            Ok(selected) => selected,
            Err(_) => {
                self.latch_fail_closed("fence completion lost exact FIFO ownership");
                return Err(RuntimeError::FailClosed);
            }
        };
        let Some((command, candidate)) = selected else {
            return Ok(None);
        };
        arbitration.fence_completion_bypass = true;
        arbitration.fence_predecessor_lifecycle_ordinal = Some(oldest_deferred_lifecycle);
        arbitration.fence_predecessor_ownership = Some(target);
        arbitration.fence_predecessor_ingress_ownership = target_ingress_ownership;
        arbitration.fence_predecessor_occurrence_ownership = Some(target_occurrence_ownership);
        let owner = match command.lifecycle_owner() {
            Ok(owner) => owner,
            Err(_) => {
                self.latch_fail_closed("fence completion lost causal lifecycle ownership");
                return Err(RuntimeError::FailClosed);
            }
        };
        if owner.lifecycle_ordinal() != candidate.lifecycle_ordinal
            || owner.causal_origin() != &candidate.causal_origin
        {
            self.latch_fail_closed("selected fence completion changed its lifecycle owner");
            return Err(RuntimeError::FailClosed);
        }
        let parent_statement = command.candidate_semantic_statement;
        let dispatch = match self.driver.dispatch(command) {
            Ok(dispatch) => dispatch,
            Err(error) => return Err(self.close(error)),
        };
        // `completion_unblocks_deferred_fence` is a production adapter
        // contract: matcher-true means this exact callback consumes the active
        // signing fence. Retrying or inserting that callback into another Busy
        // lane would recreate the dependency cycle under a different owner.
        if dispatch.retry_unadmitted
            || dispatch.deferred_ordinal.is_some()
            || dispatch.deferred_ingress.is_some()
        {
            self.latch_fail_closed("matching fence completion retried or became adapter-deferred");
            return Err(RuntimeError::FailClosed);
        }
        let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) = self
            .accept_driver_dispatch(
                dispatch,
                &owner,
                parent_statement,
                RuntimeDispatchIngress::LocalOrCausal,
            )?;
        if retry_unadmitted {
            self.latch_fail_closed("matching fence completion retained retry state");
            return Err(RuntimeError::FailClosed);
        }
        let queue_after = self.ingress.ownership_snapshot();
        self.retain_scheduler_ownership(
            RuntimeSelectedOwnerKind::FenceCompletion,
            round_tag,
            RuntimeSelectedCandidateOwnership::Exact(candidate),
            queue_before,
            queue_after,
            arbitration,
            schedule,
            schedule,
        )?;
        if self
            .retain_effect_ownership(
                RuntimeEffectSource::Fifo,
                Some(&owner),
                parent_statement.as_ref(),
                &effects,
            )
            .is_err()
        {
            self.latch_fail_closed("fence-completion effect ownership could not be retained");
            return Err(RuntimeError::FailClosed);
        }
        let mut completed_producer_handoff = None;
        if let Some(token) = producer_handoff {
            if token.identity().admission_ordinal() != owner.lifecycle_ordinal()
                || token.identity().causal_lifecycle_key() != owner.causal_origin().lifecycle_key
            {
                self.latch_fail_closed("fence completion changed its producer handoff lifecycle");
                return Err(RuntimeError::FailClosed);
            }
            let evidence = match self
                .driver
                .producer_handoff_evidence(token, !effects.is_empty())
            {
                Ok(evidence) => evidence,
                Err(error) => {
                    self.latch_fail_closed(format!(
                        "fence-completion producer handoff evidence failed: {error}"
                    ));
                    return Err(RuntimeError::FailClosed);
                }
            };
            let terminal = match self.driver.acknowledge_producer_handoff(token, evidence) {
                Ok(terminal) => terminal,
                Err(error) => {
                    self.latch_fail_closed(format!(
                        "fence-completion producer handoff acknowledgement failed: {error}"
                    ));
                    return Err(RuntimeError::FailClosed);
                }
            };
            completed_producer_handoff = Some((evidence, terminal));
        }
        self.complete_driver_dispatch_leader_wire_owners(
            &owner,
            retained_deferred_ingress,
            completed_producer_handoff,
        )?;
        if self.observe_effects(now, &effects).is_err() {
            self.latch_fail_closed(
                "fence-completion effect observation lost active-view producer ownership",
            );
            return Err(RuntimeError::FailClosed);
        }
        Ok(Some(RuntimeStep::Advanced(effects)))
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
        let queue_before = self.ingress.ownership_snapshot();
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
            let queue_after = self.ingress.ownership_snapshot();
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
        let current_ingress = if command.ingress_ownership.is_some() {
            RuntimeDispatchIngress::DirectAuthenticated
        } else {
            RuntimeDispatchIngress::LocalOrCausal
        };
        let parent_statement = command.candidate_semantic_statement;
        let retry_command = command.clone();
        let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) = match self
            .driver
            .dispatch(command)
        {
            Ok(dispatch) => {
                self.accept_driver_dispatch(dispatch, &owner, parent_statement, current_ingress)?
            }
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
            let queue_after = self.ingress.ownership_snapshot();
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
        let queue_after = self.ingress.ownership_snapshot();
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
            .retain_effect_ownership(
                RuntimeEffectSource::Fifo,
                Some(&owner),
                parent_statement.as_ref(),
                &effects,
            )
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
        self.complete_driver_dispatch_leader_wire_owners(
            &owner,
            retained_deferred_ingress,
            completed_producer_handoff,
        )?;
        if self.observe_effects(now, &effects).is_err() {
            self.latch_fail_closed(
                "recovery effect observation lost active-view producer ownership",
            );
            return Err(RuntimeError::FailClosed);
        }
        Ok(RuntimeStep::Advanced(effects))
    }

    /// Dispatch one target-relative eligible adapter-owned transition without
    /// concatenating it with a timer or runtime-ingress command.
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
        let eligible = self.eligible_deferred_admission_ordinals().map_err(|_| {
            self.latch_fail_closed("deferred physical-cut lifecycle ownership was invalid");
            RuntimeError::FailClosed
        })?;
        if eligible.is_empty() && !active_deferred.is_empty() {
            return Ok(None);
        }
        let round_tag = self.round_tag;
        let queue_before = self.ingress.ownership_snapshot();
        let schedule = self.schedule;
        let arbitration = self.scheduler_arbitration_inputs(now).map_err(|_| {
            self.latch_fail_closed("deferred scheduler lifecycle ownership was invalid");
            RuntimeError::FailClosed
        })?;
        let queue_after = self.ingress.ownership_snapshot();
        let dispatch = match self.driver.dispatch_deferred(&eligible) {
            Ok(dispatch) => dispatch,
            Err(error) => return Err(self.close(error)),
        };
        let Some((effects, evidence, producer_handoff)) = dispatch else {
            self.latch_fail_closed("serviceable deferred work had no selected owner");
            return Err(RuntimeError::FailClosed);
        };
        #[cfg(test)]
        let mut evidence = evidence;
        let selected_owner_is_eligible = eligible.contains(&evidence.admission_ordinal);
        #[cfg(test)]
        let selected_owner_is_eligible = selected_owner_is_eligible
            || (active_deferred.is_empty() && self.deferred_lifecycle_ownership.is_empty());
        if !selected_owner_is_eligible {
            self.latch_fail_closed("deferred driver selected an ineligible admission owner");
            return Err(RuntimeError::FailClosed);
        }
        let selected_eligible_set_is_exact =
            evidence.matches_eligible_admission_ordinals(&eligible);
        #[cfg(test)]
        let selected_eligible_set_is_exact = selected_eligible_set_is_exact
            || (active_deferred.is_empty() && self.deferred_lifecycle_ownership.is_empty());
        if !selected_eligible_set_is_exact
            || !evidence.belongs_to(self.driver.deferred_admission_ordinal_source())
            || !evidence.adapter_service_is_claimed()
            || !evidence.claim_runtime_handoff_once()
        {
            self.latch_fail_closed("deferred service evidence failed ownership handoff");
            return Err(RuntimeError::FailClosed);
        }
        let deferred_ordinal = evidence.admission_ordinal;
        let lifecycle_ownership = self.deferred_lifecycle_ownership.remove(&deferred_ordinal);
        #[cfg(test)]
        let lifecycle_ownership = lifecycle_ownership.or_else(|| {
            self.driver
                .synthetic_deferred_lifecycle_owner(&evidence)
                .and_then(|owner| {
                    let runtime_seal = evidence.bind_runtime_ownership_for_test(
                        owner.causal_origin().lifecycle_key.clone(),
                        owner.lifecycle_ordinal(),
                        None,
                        self.ingress_physical_cut,
                    )?;
                    RuntimeDeferredLifecycleOwnership::new(
                        owner,
                        evidence.admission_ordinal,
                        RuntimeDispatchIngress::LocalOrCausal,
                        None,
                        self.ingress_physical_cut,
                        runtime_seal,
                    )
                    .ok()
                })
        });
        let Some(lifecycle_ownership) = lifecycle_ownership else {
            self.latch_fail_closed("deferred service had no lifecycle owner");
            return Err(RuntimeError::FailClosed);
        };
        if lifecycle_ownership.deferred_admission_ordinal != deferred_ordinal {
            self.latch_fail_closed("deferred service changed its adapter admission ordinal");
            return Err(RuntimeError::FailClosed);
        }
        let lifecycle_owner = lifecycle_ownership.owner().clone();
        let parent_statement = lifecycle_ownership.candidate_semantic_statement;
        let active_deferred = self.driver.all_deferred_admission_ordinals();
        self.deferred_lifecycle_ownership
            .retain(|ordinal, _| active_deferred.contains(ordinal));
        if self.deferred_lifecycle_ownership.len() != active_deferred.len()
            || self
                .deferred_lifecycle_ownership
                .iter()
                .any(|(ordinal, owner)| {
                    owner.deferred_admission_ordinal != *ordinal
                        || !owner.validate_active_against_ingress(
                            self.deferred_ingress_ownership.get(ordinal),
                            self.driver.deferred_admission_ordinal_source(),
                        )
                })
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
                lifecycle_ownership,
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
                parent_statement.as_ref(),
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
        self.complete_driver_dispatch_leader_wire_owners(
            &lifecycle_owner,
            false,
            completed_producer_handoff,
        )?;
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
                self.timeout_owner_physical_cut = None;
                self.retransmit_owner = None;
                self.retransmit_owner_physical_cut = None;
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

include!("v2_runtime/adapter_runtime.rs");

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
    include!("tests/v2_runtime_upstream_exact_ownership.rs");
}
