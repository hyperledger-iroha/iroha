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
                        ownership.validate_frozen_physical()
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
                (queued.validate_admission_identity()
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
            .map(TaggedCommand::lifecycle_owner)
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
    /// Return whether this exact causally owned completion is the sole command
    /// which can open the adapter's current Busy-deferred signing fence.
    ///
    /// The runtime uses this only when strictly older adapter debt is present
    /// but unserviceable. Ordinary completions and stale or independent
    /// signature callbacks remain governed by immutable FIFO lifecycle order.
    /// Returning `true` also promises that dispatch consumes the signing fence;
    /// retry or insertion into another deferred lane is a contract failure.
    fn completion_unblocks_deferred_fence(&self, _tag: EventTag, _command: &Self::Command) -> bool {
        false
    }
    /// Return whether this exact queued command is demonstrably blocked by the
    /// same active fence as [`Self::completion_unblocks_deferred_fence`].
    ///
    /// The runtime uses this proof only to ignore that command's queue alias
    /// while locating the exact causal completion. External tasks, producer
    /// reservations, timers, and commands which can terminate before the
    /// reducer remain ordered blockers.
    fn command_is_blocked_by_deferred_fence(
        &self,
        _tag: EventTag,
        _command: &Self::Command,
    ) -> bool {
        false
    }
    /// Return whether adapter-owned Busy-deferred work can cross the reducer
    /// boundary without spinning behind a persistence/signing fence.
    ///
    /// This is an actor-global predicate: when it is true, every retained
    /// deferred owner is past the same reducer fences. A driver with per-owner
    /// readiness must expose an exact ordinal set instead of implementing this
    /// boolean approximately.
    fn deferred_work_is_serviceable(&self) -> bool;
    /// Actor-global source which minted deferred ownership capabilities.
    fn deferred_admission_ordinal_source(&self) -> &DeferredAdmissionOrdinalSource;
    /// Actor-global ordinals of every authenticated occurrence still retained
    /// by the adapter's Busy-deferred queues.
    fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128>;
    /// Actor-global ordinals of every occurrence retained by any Busy lane.
    fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128>;
    /// Private adapter-issued identity of one retained Busy occurrence,
    /// sampled without claiming its service turn.
    fn deferred_occurrence_ownership(
        &self,
        _admission_ordinal: u128,
    ) -> Option<DeferredOccurrenceOwnershipEvidence> {
        None
    }
    /// Seal one newly admitted Busy occurrence to the exact runtime owner and
    /// frozen physical cut before the runtime retains its wrapper.
    fn seal_deferred_runtime_ownership(
        &mut self,
        _admission_ordinal: u128,
        _owner: &RuntimeLifecycleOwner,
        _current_ingress: RuntimeDispatchIngress,
        _source_physical_ordinal: Option<u64>,
        _physical_cut: u128,
    ) -> Result<DeferredRuntimeOwnershipSeal, Self::Error> {
        unreachable!("a synthetic driver cannot admit production Busy ownership")
    }
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
    /// adapter admission ordinals selected by the runtime's target-relative
    /// physical-cut relation and then by logical minimum inside each retained
    /// predecessor set.
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
                        || existing.owner() != parent =>
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
                        (RuntimeDispatchIngress::LocalOrCausal, None, None) => {
                            (None, self.ingress_physical_cut)
                        }
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
        // A periodic owner frozen before the absolute deadline may complete
        // its one bounded episode. Once that episode drains, do not replenish
        // the cached lower-ordinal root while the one-shot timeout is still
        // waiting to emit: otherwise every late call can recreate the same
        // older owner and starve the frozen timeout forever. `raw_timeout_due`
        // is false immediately after timeout emission, so post-timeout
        // TimeoutVote and decided-body recovery remain fully enabled.
        if raw_retransmit_due && !raw_timeout_due && self.retransmit_owner.is_none() {
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

    /// Global logical minimum as observed by one already-admitted deferred
    /// continuation.
    ///
    /// All ordinary owners retain the established logical ordering. An
    /// authenticated ingress or derived deferred continuation whose source
    /// occurrence is at or after this continuation's frozen cut is physically
    /// later and cannot resurrect an older logical queue position.
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
        if !deferred_lifecycle_ordinals_are_unique(&self.deferred_lifecycle_ownership) {
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
        let physically_eligible = self
            .deferred_lifecycle_ownership
            .iter()
            .filter_map(|(admission_ordinal, candidate)| {
                let physically_behind_an_active_target = candidate
                    .source_physical_ordinal
                    .is_some_and(|source_physical_ordinal| {
                        self.deferred_lifecycle_ownership
                            .iter()
                            .any(|(other_ordinal, target)| {
                                other_ordinal != admission_ordinal
                                    && u128::from(source_physical_ordinal) >= target.physical_cut
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

        let (effects, effect_source, effect_parent, producer_handoff, retained_deferred_ingress) =
            match work {
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
                    let current_ingress = if command.ingress_ownership.is_some() {
                        RuntimeDispatchIngress::DirectAuthenticated
                    } else {
                        RuntimeDispatchIngress::LocalOrCausal
                    };
                    let retry_command = command.clone();
                    let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
                        match self.driver.dispatch(command) {
                            Ok(dispatch) => {
                                self.accept_driver_dispatch(dispatch, &owner, current_ingress)?
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
        let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
            self.accept_driver_dispatch(dispatch, &owner, RuntimeDispatchIngress::LocalOrCausal)?;
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
            .retain_effect_ownership(RuntimeEffectSource::Fifo, Some(&owner), &effects)
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
        let retry_command = command.clone();
        let (effects, retry_unadmitted, producer_handoff, retained_deferred_ingress) =
            match self.driver.dispatch(command) {
                Ok(dispatch) => self.accept_driver_dispatch(dispatch, &owner, current_ingress)?,
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

    /// Bind one live, independently durable validation marker without
    /// delivering an obsolete reducer event.
    ///
    /// Effect completions call this inside the same serialized actor turn as
    /// their catalog update. The registry mutation is exact and monotone; it
    /// does not retag or otherwise revive a retired reducer consumer.
    pub(crate) fn bind_validated_body(
        &mut self,
        manifest: &wire::PayloadManifest,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<(), AdapterError> {
        if self.fail_closed {
            return Err(AdapterError::FailClosed);
        }
        self.driver.bind_validated_body(manifest, validated_receipt)
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
        let observed_physical_cut = ingress_ownership.runtime_physical_cut().ok_or_else(|| {
            self.latch_fail_closed(
                "network ingress omitted its checked receiver physical admission cut",
            );
            NetworkIngressError::FailClosed
        })?;
        self.ingress_physical_cut = self.ingress_physical_cut.max(observed_physical_cut);
        let ingress_ownership =
            RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, ingress_ownership)
                .ok_or_else(|| {
                    self.latch_fail_closed(
                        "network ingress changed its authenticated fair-queue ownership",
                    );
                    NetworkIngressError::FailClosed
                })?;
        if !ingress_ownership.validate_frozen_physical() {
            self.latch_fail_closed(
                "network ingress changed its checked receiver physical ownership",
            );
            return Err(NetworkIngressError::FailClosed);
        }
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
    fn can_admit_pre_runtime_leader_wire(
        &self,
        outer_message: &wire::ConsensusMessageV2,
        runtime_message: &wire::ConsensusMessageV2,
        default_class: CommandClass,
        ownership: &FairV2IngressOwnershipEvidence,
    ) -> Option<bool> {
        let token = ownership.leader_wire_token()?;
        if ownership.leader_wire_runtime_receipt().is_some() {
            return None;
        }

        // Productive fair ingress owns the durable Ingress token while the
        // packet is still physically queued. Its Runtime receipt can only be
        // minted by the atomic dequeue immediately after this read-only
        // predicate succeeds. Validate that exact pre-handoff state here;
        // never weaken RuntimeIngressOwnershipEvidence, whose token+receipt
        // pairing remains the post-dequeue proof boundary.
        let outer = super::message::BlockMessage::V2(outer_message.clone());
        if !ownership.validate_exact()
            || !ownership.matches_message(&outer)
            || ownership.runtime_physical_cut().is_some()
            || ownership.runtime_lifecycle_ordinal() != Some(token.scheduler_ordinal())
            || !self
                .ingress
                .lifecycle_ordinals
                .recognizes_minted(token.scheduler_ordinal())
                .unwrap_or(false)
        {
            // Drain malformed process-local ownership so the mutating seam
            // reports the exact invariant failure instead of pinning a fair
            // lane forever.
            return Some(true);
        }

        if let Some((_, admission_ordinal)) = self
            .driver
            .deferred_authenticated_message_owner(runtime_message)
        {
            // A Busy-deferred aggregate already owns its sole serialized
            // occurrence. An exact restart retry may rejoin that lifecycle;
            // a distinct productive token must remain in fair ingress until
            // the deferred owner retires and a real FIFO slot is available.
            let same_token = self
                .deferred_ingress_ownership
                .get(&admission_ordinal)
                .and_then(|retained| retained.leader_wire_token().ok().flatten())
                == Some(token);
            return Some(same_token);
        }

        for queued in &self.ingress.commands {
            if !queued.command.matches_wire_envelope(runtime_message) {
                continue;
            }
            let Some(retained) = queued.ingress_ownership.as_ref() else {
                // Let the mutating seam expose a corrupt authenticated owner.
                return Some(true);
            };
            match retained.leader_wire_token() {
                Ok(Some(retained_token)) if retained_token == token => return Some(true),
                Ok(_) => {}
                Err(_) => return Some(true),
            }
        }

        let may_use_progress = self
            .driver
            .wire_ingress_may_use_progress(&runtime_message.payload);
        let capacity = match self.ingress.check_capacity(default_class) {
            Ok(()) => Ok(()),
            Err(_) if may_use_progress => self.ingress.check_capacity(CommandClass::Progress),
            Err(error) => Err(error),
        };
        Some(capacity.is_ok())
    }

    pub(crate) fn can_admit_network_message_with_ingress_ownership(
        &self,
        message: &wire::ConsensusMessageV2,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        let outer_message = message;
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
            if let Some(admissible) = self.can_admit_pre_runtime_leader_wire(
                outer_message,
                &runtime_message,
                default_class,
                ingress_ownership,
            ) {
                return admissible;
            }
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
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("body completion rebind lost deferred runtime ownership");
            return Err(
                "Sumeragi v2 body completion rebind lost deferred runtime ownership".to_owned(),
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
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("body completion retirement lost deferred runtime ownership");
            return Err(
                "Sumeragi v2 body completion retirement lost deferred runtime ownership".to_owned(),
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
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed(
                "body pipeline completion retirement lost deferred runtime ownership",
            );
            return Err(
                "Sumeragi v2 body pipeline retirement lost deferred runtime ownership".to_owned(),
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
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("decided proposal retirement lost deferred runtime ownership");
            return Err(
                "Sumeragi v2 deferred proposal retirement lost runtime ownership".to_owned(),
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
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("unsafe proposal retirement lost deferred runtime ownership");
            return Err("Sumeragi v2 unsafe-proposal retirement lost runtime ownership".to_owned());
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
        fresh: Option<RuntimeFreshRootKind>,
        semantic: u8,
    }

    impl FakeEffect {
        const fn other() -> Self {
            Self {
                enter_view: None,
                fresh: None,
                semantic: 0,
            }
        }

        const fn enter_view(tag: EventTag) -> Self {
            Self {
                enter_view: Some(tag),
                fresh: None,
                semantic: 0,
            }
        }

        const fn historical(semantic: u8) -> Self {
            Self {
                enter_view: None,
                fresh: Some(RuntimeFreshRootKind::HistoricalLockedRetransmit),
                semantic,
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
        retry_once: BTreeSet<u8>,
        timer_effects: VecDeque<Vec<FakeEffect>>,
        deferred_effects: VecDeque<Vec<FakeEffect>>,
        deferred_dispatches: usize,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
        deferred_active_ordinals: BTreeSet<u128>,
        deferred_service_cursor: DeferredPriority,
        deferred_identity_unavailable: bool,
        deferred_evidence_overrides: VecDeque<DeferredServiceEvidence>,
        admission_preflight_override: Option<RuntimeCommandAdmissionPreflight>,
        dormant_local_fifo_reservations: Vec<RuntimeDormantLocalFifoReservation>,
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
                retry_once: BTreeSet::new(),
                timer_effects: VecDeque::new(),
                deferred_effects: VecDeque::new(),
                deferred_dispatches: 0,
                deferred_admission_ordinals: DeferredAdmissionOrdinalSource::new(0),
                deferred_active_ordinals: BTreeSet::new(),
                deferred_service_cursor: DeferredPriority::Completion,
                deferred_identity_unavailable: false,
                deferred_evidence_overrides: VecDeque::new(),
                admission_preflight_override: None,
                dormant_local_fifo_reservations: Vec::new(),
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

        fn preflight_command_admission(
            &self,
            _tag: EventTag,
            _command: &Self::Command,
        ) -> RuntimeCommandAdmissionPreflight {
            self.admission_preflight_override
                .unwrap_or(RuntimeCommandAdmissionPreflight::Admit)
        }

        fn dormant_local_fifo_reservations(
            &self,
        ) -> Result<Vec<RuntimeDormantLocalFifoReservation>, String> {
            Ok(self.dormant_local_fifo_reservations.clone())
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
            if self.retry_once.remove(&value) {
                return Ok(RuntimeDriverDispatch {
                    effects: Vec::new(),
                    deferred_ingress: None,
                    deferred_ordinal: None,
                    retry_unadmitted: true,
                    producer_handoff: None,
                });
            }
            self.delivered.push((tagged.tag, value));
            Ok(RuntimeDriverDispatch::completed(vec![FakeEffect::other()]))
        }

        fn timeout_elapsed(
            &mut self,
            tag: EventTag,
        ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
            self.timeouts.push(tag);
            Ok(RuntimeDriverDispatch::completed(
                self.timer_effects.pop_front().unwrap_or_default(),
            ))
        }

        fn retransmit_elapsed(
            &mut self,
            tag: EventTag,
        ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
            self.retransmits.push(tag);
            Ok(RuntimeDriverDispatch::completed(
                self.timer_effects.pop_front().unwrap_or_default(),
            ))
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

        fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
            self.deferred_active_ordinals.clone()
        }

        fn synthetic_deferred_lifecycle_owner(
            &self,
            evidence: &DeferredServiceEvidence,
        ) -> Option<RuntimeLifecycleOwner> {
            let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
                evidence.original_tag,
                CommandClass::Completion,
                RuntimeFreshRootKind::StartupRecovery,
                b"fake-deferred-owner",
            );
            let lifecycle_ordinal = evidence.admission_ordinal.checked_add(1)?;
            RuntimeLifecycleOwner::new(origin, lifecycle_ordinal).ok()
        }

        fn dispatch_deferred(
            &mut self,
            _eligible: &BTreeSet<u128>,
        ) -> Result<
            Option<(
                Vec<Self::Effect>,
                DeferredServiceEvidence,
                Option<ProducerContinuationHandoffToken>,
            )>,
            Self::Error,
        > {
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
            Ok(Some((effects, evidence, None)))
        }

        fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag> {
            effect.enter_view
        }

        fn effect_causality(
            effect: &Self::Effect,
            _source: RuntimeEffectSource,
        ) -> RuntimeEffectCausality {
            effect.fresh.map_or(
                RuntimeEffectCausality::Inherit,
                RuntimeEffectCausality::Fresh,
            )
        }

        fn fresh_effect_semantic_identity(
            effect: &Self::Effect,
            kind: RuntimeFreshRootKind,
        ) -> Vec<u8> {
            vec![kind.code(), effect.semantic]
        }

        fn effect_root_tag(_effect: &Self::Effect) -> Option<EventTag> {
            None
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
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
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

    fn signed_runtime_vote(
        keys: &[KeyPair],
        round: wire::ConsensusRound,
        phase: wire::GlobalPhase,
        subject: wire::BlockSubject,
        execution_commitment: wire::ExecutionCommitment,
    ) -> wire::ConsensusMessageV2 {
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(vote.signer).expect("small signer index")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote))
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

    fn fair_runtime_ownership_at_lifecycle(
        mut ownership: FairV2IngressOwnershipEvidence,
        lifecycle_ordinal: u128,
    ) -> FairV2IngressOwnershipEvidence {
        ownership.first.lifecycle_ordinal = Some(lifecycle_ordinal);
        ownership.latest.lifecycle_ordinal = Some(lifecycle_ordinal);
        assert!(
            ownership.validate_exact(),
            "test lifecycle projection must preserve exact fair ownership"
        );
        ownership
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

    fn signed_runtime_timeout_certificate(
        context: &wire::HeightContext,
        keys: &[KeyPair],
    ) -> wire::TimeoutCertificate {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let signers = vec![0, 1, 2];
        let preimage = wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
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
        wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers,
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                    .expect("aggregate runtime fixture timeout certificate"),
            }],
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
        runtime
            .observe_effects_with_test_ownership(
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
            )
            .expect("test EnterView retains positional producer ownership");
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

        stage_completion_for_queue_test(
            &mut runtime,
            view_one,
            AdapterCommand::BodyAvailable {
                manifest: manifest.clone(),
            },
        );
        let causal_origin = runtime.ingress.commands[0].causal_origin.clone();
        let lifecycle_ordinal = runtime.ingress.commands[0].lifecycle_ordinal;
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
        assert_eq!(runtime.ingress.commands[0].causal_origin, causal_origin);
        assert_eq!(
            runtime.ingress.commands[0].lifecycle_ordinal, lifecycle_ordinal,
            "view/generation rebinding retains the logical lifecycle owner"
        );
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner() {
        let directory = TempDir::new().expect("temporary reserved-body rebind directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let initial = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x8B);
        let reservation = runtime
            .reserve_body_available(initial, manifest.clone())
            .expect("reserve an unpublished body completion");
        let source_after_reserve = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect ordinal source after reservation");
        let rebound = EventTag::new(
            initial.height(),
            initial.view() + 1,
            Generation::new(initial.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut runtime, initial, rebound, &manifest);

        assert!(
            runtime
                .rebind_body_available(initial, rebound, &manifest)
                .expect("the unpublished token is a serialized body owner")
        );
        let mut rebound_reservation = reservation;
        rebound_reservation.tag = rebound;
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&rebound_reservation),
        );
        let retry = runtime
            .reserve_body_available(rebound, manifest.clone())
            .expect("rebound exact retry reclaims the immutable root token");
        assert_eq!(retry, rebound_reservation);
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect source after rebound retry"),
            source_after_reserve,
            "rebind and retry cannot remint the token",
        );

        assert!(
            runtime
                .retire_body_available(rebound, &manifest)
                .expect("terminal supersession retires the exact unpublished owner")
        );
        assert!(runtime.ingress.reserved_body_available.is_none());
        assert_eq!(runtime.queued_commands(), 0);
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

    /// Stage an exact completion directly in the bounded FIFO for tests of
    /// queue ownership itself. Production tests use the public enqueue seams,
    /// whose reducer preflight correctly rejects callbacks without a live
    /// phase or exact terminal lifecycle.
    fn stage_completion_for_queue_test(
        runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
        tag: EventTag,
        command: AdapterCommand,
    ) {
        runtime
            .ingress
            .enqueue(TaggedCommand::new(
                tag,
                CommandClass::Completion,
                command,
                Instant::now(),
            ))
            .expect("queue-ownership fixture stages an exact completion");
    }

    /// Attach the same private local/causal runtime wrapper that production
    /// dispatch installs around one exact adapter-owned Busy occurrence.
    fn bind_local_deferred_lifecycle_for_test(
        runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
        deferred_admission_ordinal: u128,
        semantic_identity: &[u8],
    ) -> RuntimeLifecycleOwner {
        let lifecycle_ordinal = runtime
            .ingress
            .lifecycle_ordinals
            .reserve_one()
            .expect("reserve one exact local lifecycle ordinal");
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            runtime.round_tag(),
            CommandClass::Completion,
            RuntimeFreshRootKind::StartupRecovery,
            semantic_identity,
        );
        let owner = RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
            .expect("bind the local deferred lifecycle ordinal");
        let physical_cut = runtime.ingress_physical_cut;
        let runtime_seal = runtime
            .driver
            .bind_deferred_runtime_ownership(
                deferred_admission_ordinal,
                owner.causal_origin().lifecycle_key.clone(),
                owner.lifecycle_ordinal(),
                false,
                None,
                physical_cut,
            )
            .expect("seal the exact local Busy occurrence");
        let deferred = RuntimeDeferredLifecycleOwnership::new(
            owner.clone(),
            deferred_admission_ordinal,
            RuntimeDispatchIngress::LocalOrCausal,
            None,
            physical_cut,
            runtime_seal,
        )
        .expect("freeze the exact local Busy occurrence");
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(deferred_admission_ordinal, deferred)
                .is_none(),
            "the fixture cannot replace an existing runtime wrapper"
        );
        owner
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

    fn preowned_leader_wire_ownerships(
        context: &wire::HeightContext,
        messages: &[(wire::ConsensusMessageV2, PeerId)],
        lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    ) -> (
        TempDir,
        Arc<super::super::FairV2Ingress>,
        Vec<FairV2IngressOwnershipEvidence>,
    ) {
        let directory = TempDir::new().expect("temporary preowned leader-wire directory");
        let ingress = Arc::new(super::super::FairV2Ingress::new(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        ));
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        ingress
            .configure_roster_for_context(roster.clone(), &context.chain_id, context.da_layout)
            .expect("preowned leader-wire geometry");
        ingress.require_leader_wire_lifecycle_gate();
        let capacity =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
                roster.len(),
                context.da_layout.max_chunk_count,
            )
            .expect("finite preowned leader-wire capacity");
        let recovery_authority =
            super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                context.id(),
                context.height,
                [0xE7; 32],
                0,
                false,
            );
        let (gate, restore) =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
                &directory.path().join("leader-wire-preowned.wal"),
                context.id(),
                context.height,
                [0xE7; 32],
                roster.iter().cloned().collect(),
                capacity,
                context.da_layout.max_chunk_count,
                recovery_authority,
                &[],
                &[],
            )
            .expect("open preowned leader-wire gate");
        ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&gate),
                restore,
                lifecycle_ordinals,
                context.id(),
                context.height,
            )
            .expect("bind preowned leader-wire gate");
        ingress.open().expect("open preowned fair ingress");

        let ownerships = messages
            .iter()
            .map(|(message, semantic_origin)| {
                assert!(matches!(
                    ingress.try_push(InboundBlockMessage::new(
                        BlockMessage::V2(message.clone()),
                        Some(semantic_origin.clone()),
                    )),
                    Ok(super::super::FairV2IngressPushDisposition::Enqueued)
                ));
                let mut admitted = ingress
                    .try_recv()
                    .expect("drain preowned leader-wire occurrence");
                let mut ownership = admitted
                    .take_ingress_ownership()
                    .expect("preowned leader wire retains fair ownership");
                assert!(
                    ownership.leader_wire_runtime_receipt().is_some(),
                    "checked dequeue atomically installs the durable runtime handoff"
                );
                let token = ownership
                    .leader_wire_token()
                    .expect("productive dequeue retains its immutable leader-wire token");
                assert_eq!(
                    gate.ingress_scheduler_ordinals()
                        .expect("read durable owners after atomic handoff"),
                    std::collections::BTreeSet::new(),
                    "atomic handoff removes the owner from the durable Ingress selector"
                );
                {
                    let state = ingress.state.lock();
                    let record = state
                        .leader_wire_lifecycles
                        .get(&token.slot)
                        .expect("atomic handoff retains the exact lifecycle record");
                    assert_eq!(
                        record.status,
                        super::super::FairV2IngressLeaderWireStatus::Runtime,
                        "atomic handoff publishes the in-memory Runtime owner"
                    );
                }
                ingress
                    .bind_leader_wire_runtime_ownership(&mut ownership)
                    .expect("repeated preowned leader-wire bind is idempotent");
                assert!(matches!(
                    ingress.try_push(InboundBlockMessage::new(
                        BlockMessage::V2(message.clone()),
                        Some(semantic_origin.clone()),
                    )),
                    Ok(super::super::FairV2IngressPushDisposition::Coalesced)
                ));
                ownership
            })
            .collect();
        (directory, ingress, ownerships)
    }

    struct LeaderWireProposalFixture {
        ingress: Arc<super::super::FairV2Ingress>,
        gate: Arc<super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate>,
        message: wire::ConsensusMessageV2,
        ownership: FairV2IngressOwnershipEvidence,
        receipt: LeaderWireLifecycleRuntimeReceipt,
    }

    fn leader_wire_proposal_fixture(
        directory: &TempDir,
        context: &wire::HeightContext,
        keys: &[KeyPair],
        marker: u8,
        lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    ) -> LeaderWireProposalFixture {
        let message = signed_runtime_proposal(context, keys, marker);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            unreachable!("signed runtime proposal fixture carries Proposal")
        };
        let ingress = Arc::new(super::super::FairV2Ingress::new(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        ));
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        ingress
            .configure_roster_for_context(roster.clone(), &context.chain_id, context.da_layout)
            .expect("leader-wire runtime fixture geometry");
        ingress.require_leader_wire_lifecycle_gate();
        let capacity =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
                roster.len(),
                context.da_layout.max_chunk_count,
            )
            .expect("finite leader-wire runtime fixture capacity");
        let owner = [marker; 32];
        let recovery_authority =
            super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                context.id(),
                context.height,
                owner,
                proposal.round.view,
                false,
            );
        let (gate, restore) =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
                &directory
                    .path()
                    .join(format!("leader-wire-runtime-{marker}.wal")),
                context.id(),
                context.height,
                owner,
                roster.iter().cloned().collect(),
                capacity,
                context.da_layout.max_chunk_count,
                recovery_authority,
                &[],
                &[],
            )
            .expect("open leader-wire runtime fixture gate");
        ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&gate),
                restore,
                lifecycle_ordinals,
                context.id(),
                context.height,
            )
            .expect("bind leader-wire runtime fixture gate");
        ingress.open().expect("open leader-wire runtime fixture");
        let semantic_origin = context.roster
            [usize::try_from(proposal.proposer).expect("small fixture proposer")]
        .validator
        .clone();
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(message.clone()),
                Some(semantic_origin),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ));
        let mut saw_predequeue_owner = false;
        assert!(
            ingress
                .try_recv_if_checked(|inbound| {
                    let ownership = inbound
                        .ingress_ownership()
                        .expect("queued leader-wire message retains fair ownership");
                    assert!(ownership.runtime_physical_cut().is_none());
                    assert!(ownership.leader_wire_token().is_some());
                    assert!(ownership.leader_wire_runtime_receipt().is_none());
                    let projected = RuntimeIngressOwnershipEvidence::from_fair_ingress(
                        &message,
                        ownership.clone(),
                    )
                    .expect("pre-dequeue identity remains valid for the capacity probe");
                    assert!(projected.validate_exact());
                    assert!(!projected.validate_frozen_physical());
                    saw_predequeue_owner = true;
                    false
                })
                .expect("rejected pre-dequeue probe preserves the queued owner")
                .is_none()
        );
        assert!(saw_predequeue_owner);
        let mut admitted = ingress
            .try_recv()
            .expect("drain exact leader-wire proposal fixture");
        let mut ownership = admitted
            .take_ingress_ownership()
            .expect("leader-wire proposal retains fair-ingress ownership");
        ingress
            .bind_leader_wire_runtime_ownership(&mut ownership)
            .expect("bind exact leader-wire runtime receipt");
        let receipt = ownership
            .leader_wire_runtime_receipt()
            .expect("productive proposal carries runtime receipt")
            .clone();
        LeaderWireProposalFixture {
            ingress,
            gate,
            message,
            ownership,
            receipt,
        }
    }

    fn assert_volatile_leader_wire_release(
        fixture: &LeaderWireProposalFixture,
        receipt: &LeaderWireLifecycleRuntimeReceipt,
    ) {
        assert_eq!(receipt, &fixture.receipt);
        fixture
            .ingress
            .mark_leader_wire_volatile_terminal(receipt)
            .expect("publish process-local leader-wire retirement");
        assert_eq!(
            fixture
                .gate
                .earliest_ingress_scheduler_ordinal()
                .expect("read durable leader-wire minimum"),
            None,
            "a retired runtime owner cannot remain an active scheduler predecessor"
        );
        let semantic_origin = fixture.receipt.token().identity.semantic_origin.clone();
        assert!(matches!(
            fixture.ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(fixture.message.clone()),
                Some(semantic_origin),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Coalesced)
        ));
    }

    fn bind_authenticated_deferred_proposal_for_test(
        runtime: &mut SerializedV2Runtime,
        fixture: &LeaderWireProposalFixture,
    ) -> (wire::Proposal, u128) {
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &fixture.message.payload else {
            unreachable!("leader-wire fixture carries Proposal")
        };
        let proposal = proposal.clone();
        let ingress_ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &fixture.message,
            fixture.ownership.clone(),
        )
        .expect("project exact leader-wire ownership into runtime");
        let tagged = TaggedCommand::with_ingress_ownership(
            runtime.round_tag(),
            CommandClass::Normal,
            AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
                fixture.message.clone(),
            )),
            Instant::now(),
            ingress_ownership.clone(),
        );
        let lifecycle_ordinal = tagged
            .lifecycle_ordinal
            .expect("leader-wire command carries its scheduler ordinal");
        let lifecycle_owner =
            RuntimeLifecycleOwner::new(tagged.causal_origin.clone(), lifecycle_ordinal)
                .expect("construct exact deferred lifecycle owner");
        let (source_physical_ordinal, physical_cut) = ingress_ownership
            .leader_wire_physical_carrier()
            .expect("leader-wire carrier set is exact")
            .expect("leader-wire carrier exposes its checked physical cut");
        runtime
            .driver
            .defer_authenticated_proposal_for_test(runtime.round_tag(), &proposal)
            .expect("stage Busy-deferred proposal");
        let (_, deferred_ordinal) = runtime
            .driver
            .deferred_authenticated_message_owner(&fixture.message)
            .expect("deferred proposal exposes its adapter ordinal");
        let runtime_seal = runtime
            .driver
            .bind_deferred_runtime_ownership(
                deferred_ordinal,
                lifecycle_owner.causal_origin().lifecycle_key.clone(),
                lifecycle_owner.lifecycle_ordinal(),
                true,
                Some(source_physical_ordinal),
                physical_cut,
            )
            .expect("seal the exact deferred adapter occurrence");
        let lifecycle_owner = RuntimeDeferredLifecycleOwnership::new(
            lifecycle_owner,
            deferred_ordinal,
            RuntimeDispatchIngress::DirectAuthenticated,
            Some(source_physical_ordinal),
            physical_cut,
            runtime_seal,
        )
        .expect("freeze the exact deferred physical cut");
        assert!(
            runtime
                .deferred_ingress_ownership
                .insert(deferred_ordinal, ingress_ownership.clone())
                .is_none()
        );
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(deferred_ordinal, lifecycle_owner)
                .is_none()
        );
        runtime
            .register_leader_wire_runtime_receipt(&ingress_ownership)
            .expect("register deferred leader-wire receipt");
        (proposal, deferred_ordinal)
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

    fn deferred_lifecycle_ownership_for_test(
        owner: RuntimeLifecycleOwner,
        deferred_admission_ordinal: u128,
        current_ingress: RuntimeDispatchIngress,
        source_physical_ordinal: Option<u64>,
        physical_cut: u128,
    ) -> Result<RuntimeDeferredLifecycleOwnership, EnqueueError> {
        let runtime_seal = DeferredRuntimeOwnershipSeal::for_test(
            deferred_admission_ordinal,
            owner.causal_origin().lifecycle_key.clone(),
            owner.lifecycle_ordinal(),
            current_ingress == RuntimeDispatchIngress::DirectAuthenticated,
            source_physical_ordinal,
            physical_cut,
        );
        RuntimeDeferredLifecycleOwnership::new(
            owner,
            deferred_admission_ordinal,
            current_ingress,
            source_physical_ordinal,
            physical_cut,
            runtime_seal,
        )
    }

    fn enqueue_fake(
        runtime: &mut SerializedV2Runtime<FakeDriver>,
        tag: EventTag,
        class: CommandClass,
        command: FakeCommand,
    ) -> Result<(), EnqueueError> {
        runtime.enqueue(tag, class, command)
    }

    fn restored_fake_command(
        tag: EventTag,
        class: CommandClass,
        command: FakeCommand,
        causal_lifecycle_key: Hash,
        lifecycle_ordinal: u128,
        producer_stage: u8,
    ) -> TaggedCommand<FakeCommand> {
        let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
            tag,
            class,
            &command,
            None,
            causal_lifecycle_key,
            lifecycle_ordinal,
        )
        .expect("validated dormant metadata reconstructs one exact owner");
        let mut tagged = TaggedCommand::with_causal_origin(
            tag,
            class,
            command,
            Instant::now(),
            owner.causal_origin().clone(),
            owner.lifecycle_ordinal(),
        )
        .expect("restored command binds its persisted ordinal");
        tagged.restored_producer_stage = Some(producer_stage);
        tagged
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
    fn active_view_producer_fences_timeout_until_exact_proposal_fanout() {
        let (context, keys) = authenticated_runtime_context();
        let message = signed_runtime_proposal(&context, &keys, 0xA7);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
            panic!("runtime fixture must produce a Proposal")
        };
        let initial = EventTag::new(context.height, 0, Generation::new(1));
        let start = Instant::now();
        let (mut runtime, startup) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("construct unarmed active-view producer runtime");
        assert!(startup.is_empty());
        runtime
            .reconcile_active_view_producer(initial, true)
            .expect("reserve the leader producer before clocks arm");
        let reserved = runtime
            .active_view_producer
            .as_ref()
            .expect("leader producer reservation")
            .ownership
            .clone();
        runtime
            .arm_live_clocks(start)
            .expect("arm clocks after producer reservation");
        assert!(
            runtime
                .local_proposal_admission_available(initial)
                .expect("armed reservation is eligible")
        );

        let ownership = runtime
            .mint_local_proposal_effect_ownership(initial, &proposal.manifest)
            .expect("local Store aliases the active producer");
        assert_eq!(ownership.owner(), reserved.owner());
        assert!(runtime.active_view_producer.is_some());

        let deadline = start + Duration::from_secs(10);
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(deadline),
            Ok(RuntimeStep::Idle)
        ));
        assert!(runtime.driver.timeouts.is_empty());
        assert!(runtime.active_view_producer.is_some());

        runtime
            .complete_active_view_producer_after_proposal_fanout(proposal.round, &ownership)
            .expect("guarded fanout retires the inherited producer");
        assert!(runtime.active_view_producer.is_none());
        assert!(
            !runtime
                .local_proposal_admission_available(initial)
                .expect("consumed same-view reservation becomes retryable backpressure")
        );
        assert!(
            !runtime.fail_closed,
            "same-view scheduling churn must leave timeout recovery live"
        );
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(deadline),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        assert_eq!(runtime.driver.timeouts, vec![initial]);
    }

    #[test]
    fn armed_proposal_admission_cannot_bypass_the_active_view_reservation() {
        let (context, keys) = authenticated_runtime_context();
        let message = signed_runtime_proposal(&context, &keys, 0xA9);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
            panic!("runtime fixture must produce a Proposal")
        };
        let initial = EventTag::new(context.height, 0, Generation::new(1));
        let start = Instant::now();
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("construct unarmed runtime");
        runtime
            .reconcile_active_view_producer(initial, false)
            .expect("nonleader has no proposal reservation");
        runtime
            .arm_live_clocks(start)
            .expect("arm runtime without a producer reservation");

        assert!(
            !runtime
                .local_proposal_admission_available(initial)
                .expect("scheduler observes an unavailable one-shot producer")
        );
        assert!(
            runtime
                .mint_local_proposal_effect_ownership(initial, &proposal.manifest)
                .is_err(),
            "the admission invariant remains fail-closed if preflight is bypassed"
        );
        assert!(runtime.fail_closed);
    }

    #[test]
    fn proposal_fanout_cannot_replace_active_view_producer_owner() {
        let (context, keys) = authenticated_runtime_context();
        let message = signed_runtime_proposal(&context, &keys, 0xA8);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
            panic!("runtime fixture must produce a Proposal")
        };
        let initial = EventTag::new(context.height, 0, Generation::new(1));
        let start = Instant::now();
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("construct active-view producer runtime");
        runtime
            .reconcile_active_view_producer(initial, true)
            .expect("reserve exact active producer");
        runtime
            .arm_live_clocks(start)
            .expect("arm after producer reservation");
        let foreign = RuntimeEffectOwnership::fresh_for_test(initial, 999);

        assert!(
            runtime
                .complete_active_view_producer_after_proposal_fanout(proposal.round, &foreign)
                .is_err()
        );
        assert!(runtime.fail_closed);
        assert!(runtime.active_view_producer.is_some());
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
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(1)),
            Ok(RuntimeStep::Advanced(_))
        ));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(runtime.driver.retransmits, vec![initial]);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(2),
        )
        .expect("enqueue second message");
        let second_lifecycle_ordinal = runtime
            .ingress
            .commands
            .back()
            .and_then(|queued| queued.lifecycle_ordinal)
            .expect("the second message owns its immutable lifecycle ordinal");
        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9))
            .expect("the admitted message precedes the fresh periodic episode");
        assert_eq!(runtime.driver.retransmits, vec![initial]);
        assert_eq!(runtime.driver.delivered, vec![(initial, 1), (initial, 2)]);
        assert!(
            runtime
                .retransmit_owner
                .as_ref()
                .is_some_and(|owner| owner.lifecycle_ordinal() > second_lifecycle_ordinal),
            "the later runner freeze must mint a fresh periodic position after admitted work"
        );

        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("the retained periodic episode drains before the later timeout owner");
        assert_eq!(runtime.driver.retransmits, vec![initial, initial]);
        assert!(runtime.driver.timeouts.is_empty());

        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("absolute timeout dispatch succeeds after the finite prefix");
        assert_eq!(runtime.driver.timeouts, vec![initial]);

        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(20))
            .expect("post-timeout scheduling succeeds");
        assert_eq!(
            runtime.driver.retransmits,
            vec![initial, initial, initial],
            "ordinary ingress never resets either clock"
        );
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

        // The finite debt is now empty. The admitted FIFO lifecycle predates
        // the frozen timeout owner and therefore drains first.
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert!(runtime.driver.timeouts.is_empty());
        assert_eq!(runtime.driver.delivered, vec![(initial, 9)]);
        assert_eq!(runtime.queued_commands(), 0);

        // The retained timeout owner then runs without any replenished
        // periodic producer ahead of it.
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        assert_eq!(runtime.driver.timeouts, vec![initial]);
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
    fn real_adapter_fence_completion_bypasses_only_preowned_fenced_fifo() {
        let directory = TempDir::new().expect("temporary preowned-fence runtime directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let first =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0xD8),
            ));
        let second = first.clone();
        let source_one = context.roster[1].validator.clone();
        let source_two = context.roster[2].validator.clone();
        let (_leader_wire_directory, _leader_wire_ingress, ownerships) =
            preowned_leader_wire_ownerships(
                &context,
                &[(first.clone(), source_one), (second.clone(), source_two)],
                runtime.ingress.lifecycle_ordinals.clone(),
            );
        let [first_ownership, second_ownership]: [FairV2IngressOwnershipEvidence; 2] = ownerships
            .try_into()
            .expect("fixture creates two exact pre-timeout owners");
        let first_token = first_ownership
            .leader_wire_token()
            .expect("first aggregate owns its origin-specific token")
            .clone();
        let second_token = second_ownership
            .leader_wire_token()
            .expect("second aggregate owns its origin-specific token")
            .clone();
        let first_receipt = first_ownership
            .leader_wire_runtime_receipt()
            .expect("first aggregate owns its runtime receipt")
            .clone();
        let second_receipt = second_ownership
            .leader_wire_runtime_receipt()
            .expect("second aggregate owns its runtime receipt")
            .clone();
        assert_ne!(first_token, second_token);
        assert_ne!(first_receipt, second_receipt);
        assert_ne!(
            first_ownership
                .physical_admission_ordinal()
                .expect("first aggregate owns its physical occurrence"),
            second_ownership
                .physical_admission_ordinal()
                .expect("second aggregate owns its physical occurrence")
        );

        let start = Instant::now();
        runtime
            .arm_live_clocks(start)
            .expect("arm runtime after preowning peer ingress");
        let deadline = start + runtime.round_timeout();
        let timeout_step = runtime
            .step(deadline)
            .expect("absolute deadline opens TimeoutVote signing");
        runtime
            .take_last_scheduler_ownership()
            .expect("timeout retains exact scheduler ownership");
        let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
            panic!("absolute deadline unexpectedly idled")
        };
        let timeout_ownership = runtime
            .take_effect_ownership(timeout_effects.len())
            .expect("TimeoutVote Sign retains its timeout root");
        let [timeout_ownership] = timeout_ownership.as_slice() else {
            panic!("TimeoutVote Sign has one exact owner")
        };
        let (sign_tag, signature_preimage) = match timeout_effects.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(vote),
                },
            ] => (*tag, vote.signature_preimage()),
            effects => panic!("unexpected timeout effects: {effects:?}"),
        };
        runtime
            .set_external_lifecycle_owners(vec![timeout_ownership.owner().clone()])
            .expect("publish pending TimeoutVote signer owner");

        let first_physical_ordinal = first_ownership
            .physical_admission_ordinal()
            .expect("checked target owns one receiver-local occurrence");
        let first_physical_cut = first_ownership
            .runtime_physical_cut()
            .expect("checked target freezes its predecessor cut");
        runtime
            .enqueue_network_with_ingress_ownership(first, first_ownership)
            .expect("admit first pre-timeout peer owner after signing begins");
        runtime
            .enqueue_network_with_ingress_ownership(second, second_ownership)
            .expect("admit the distinct-origin duplicate before either aggregate dispatches");
        assert_eq!(runtime.queued_commands(), 2);
        assert_eq!(
            runtime
                .active_leader_wire_runtime_ordinals()
                .expect("both durable aggregate owners remain active"),
            BTreeSet::from([
                first_token.scheduler_ordinal(),
                second_token.scheduler_ordinal(),
            ])
        );
        assert_eq!(runtime.leader_wire_runtime_receipts.len(), 2);
        runtime
            .set_ingress_physical_cut(
                first_physical_cut
                    .checked_add(2)
                    .expect("small test cut can advance"),
            )
            .expect("later receiver activity advances only the global high-watermark");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("move first peer owner into Busy-deferred state"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert!(!runtime.driver().deferred_work_is_serviceable());
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
        assert_eq!(runtime.deferred_lifecycle_ownership.len(), 1);
        let (&deferred_ordinal, deferred_target) = runtime
            .deferred_lifecycle_ownership
            .iter()
            .next()
            .expect("Busy target retains exact lifecycle ownership");
        let deferred_target = deferred_target.clone();
        assert_eq!(
            deferred_target.source_physical_ordinal,
            Some(first_physical_ordinal)
        );
        assert_eq!(
            deferred_target.physical_cut, first_physical_cut,
            "a later global receiver high-watermark cannot refresh the target cut"
        );
        assert_eq!(
            runtime.deferred_ingress_ownership[&deferred_ordinal].leader_wire_token(),
            Ok(Some(&first_token)),
            "the Busy occurrence owns only the selected origin-specific lifecycle"
        );
        assert!(runtime.take_leader_wire_runtime_terminals().is_empty());

        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("the later duplicate cannot cross the active signing fence"),
            RuntimeStep::Idle
        ));
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime.deferred_lifecycle_ownership[&deferred_ordinal], deferred_target,
            "an idle fenced turn cannot replace the Busy ordinal, seal, or frozen cut"
        );
        assert_eq!(runtime.leader_wire_runtime_receipts.len(), 2);
        assert!(runtime.take_leader_wire_runtime_terminals().is_empty());

        let signature = Signature::new(keys[0].private_key(), &signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature_with_owner(sign_tag, signature, timeout_ownership)
            .expect("enqueue exact owned TimeoutVote completion");
        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("retire pending signer after completion enqueue");

        let completion_step = runtime
            .step(deadline)
            .expect("exact completion crosses preowned fenced FIFO debt");
        let scheduling = runtime
            .take_last_scheduler_ownership()
            .expect("dependency bypass retains scheduler evidence");
        assert_eq!(
            scheduling.selected,
            RuntimeSelectedOwnerKind::FenceCompletion
        );
        assert!(scheduling.fence_completion_bypass);
        assert!(scheduling.validate_exact().is_ok());
        assert!(
            scheduling
                .fence_predecessor_ingress_ownership
                .as_ref()
                .is_some_and(RuntimeIngressOwnershipEvidence::validate_frozen_physical),
            "an authenticated fence target retains its checked ingress carrier"
        );
        assert_eq!(
            scheduling
                .fence_predecessor_ingress_ownership
                .as_ref()
                .expect("fence target retains ingress ownership")
                .leader_wire_token(),
            Ok(Some(&first_token)),
            "the dependency bypass names the Busy aggregate, never its later duplicate"
        );
        let mut weakened_fence = scheduling.clone();
        weakened_fence
            .fence_predecessor_ownership
            .as_mut()
            .expect("fence evidence carries its exact deferred target")
            .physical_cut = first_physical_cut
            .checked_add(1)
            .expect("small test cut can be mutated");
        weakened_fence.projection_hash = runtime_scheduler_projection_hash(&weakened_fence);
        assert_eq!(
            weakened_fence.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "rehashing cannot hide a fence-target physical-cut mutation"
        );
        let mut replenished_fence_debt = scheduling.clone();
        replenished_fence_debt.queue_after.max_service_debt = replenished_fence_debt
            .queue_before
            .max_service_debt
            .saturating_add(1);
        replenished_fence_debt.projection_hash =
            runtime_scheduler_projection_hash(&replenished_fence_debt);
        assert_eq!(
            replenished_fence_debt.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the dependency-only fence branch cannot replenish scheduler debt"
        );
        let mut coherently_weakened_fence = scheduling.clone();
        let mutated_cut = first_physical_cut
            .checked_add(1)
            .expect("small test cut can be mutated");
        let predecessor = coherently_weakened_fence
            .fence_predecessor_ownership
            .as_mut()
            .expect("fence evidence carries its exact deferred target");
        predecessor.physical_cut = mutated_cut;
        predecessor
            .owner
            .causal_origin
            .root_ingress_physical_ownership
            .as_mut()
            .expect("network-rooted target carries its physical pair")
            .physical_cut = mutated_cut;
        predecessor.owner.causal_origin.projection_hash =
            runtime_candidate_causal_origin_projection_hash(&predecessor.owner.causal_origin);
        predecessor.owner.projection_hash =
            runtime_lifecycle_owner_projection_hash(&predecessor.owner);
        coherently_weakened_fence.projection_hash =
            runtime_scheduler_projection_hash(&coherently_weakened_fence);
        assert_eq!(
            coherently_weakened_fence.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the retained fair-ingress carrier rejects a coherently rehashed wrapper/root cut mutation"
        );
        let mut deleted_fence_ingress = scheduling.clone();
        deleted_fence_ingress.fence_predecessor_ingress_ownership = None;
        deleted_fence_ingress.projection_hash =
            runtime_scheduler_projection_hash(&deleted_fence_ingress);
        assert_eq!(
            deleted_fence_ingress.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "direct-authenticated provenance rejects deletion of the rehashed fence carrier"
        );
        let mut reclassified_fence = scheduling.clone();
        reclassified_fence.fence_predecessor_ingress_ownership = None;
        reclassified_fence
            .fence_predecessor_ownership
            .as_mut()
            .expect("fence evidence carries its exact deferred target")
            .current_ingress = RuntimeDispatchIngress::LocalOrCausal;
        reclassified_fence.projection_hash = runtime_scheduler_projection_hash(&reclassified_fence);
        assert_eq!(
            reclassified_fence.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the adapter-issued occurrence capability rejects a coherent provenance flip"
        );
        let RuntimeStep::Advanced(effects) = completion_step else {
            panic!("exact TimeoutVote completion unexpectedly idled")
        };
        assert!(effects.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(message)
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                        if vote.round.height == context.height && vote.round.view == 0
                )
        )));
        runtime
            .take_effect_ownership(effects.len())
            .expect("consume TimeoutVote broadcast ownership");

        let deferred_step = runtime
            .step(deadline)
            .expect("the physically frozen Busy target owns the next turn");
        let deferred_scheduling = runtime
            .take_last_scheduler_ownership()
            .expect("deferred turn retains scheduler evidence");
        let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
            &deferred_scheduling.candidate
        else {
            panic!("expected exact deferred scheduler ownership")
        };
        assert_eq!(candidate.service.admission_ordinal, deferred_ordinal);
        assert_eq!(candidate.lifecycle_ownership, deferred_target);
        assert_eq!(
            candidate
                .ingress_ownership
                .as_ref()
                .expect("deferred aggregate retains its authenticated carrier")
                .leader_wire_token(),
            Ok(Some(&first_token))
        );
        assert_eq!(
            candidate.lifecycle_ownership.source_physical_ordinal,
            Some(first_physical_ordinal)
        );
        assert_eq!(
            candidate.lifecycle_ownership.physical_cut,
            first_physical_cut
        );
        assert_eq!(deferred_scheduling.validate_exact(), Ok(()));
        let mut weakened_deferred = deferred_scheduling.clone();
        let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
            &mut weakened_deferred.candidate
        else {
            unreachable!("cloned deferred evidence retains its variant")
        };
        candidate.lifecycle_ownership.physical_cut = first_physical_cut
            .checked_add(1)
            .expect("small test cut can be mutated");
        weakened_deferred.projection_hash = runtime_scheduler_projection_hash(&weakened_deferred);
        assert_eq!(
            weakened_deferred.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "rehashing cannot hide a deferred-target physical-cut mutation"
        );
        let mut ordinal_mutation = deferred_scheduling.clone();
        let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
            &mut ordinal_mutation.candidate
        else {
            unreachable!("cloned deferred evidence retains its variant")
        };
        candidate.lifecycle_ownership.deferred_admission_ordinal = candidate
            .lifecycle_ownership
            .deferred_admission_ordinal
            .checked_add(1)
            .expect("small adapter ordinal has a successor");
        ordinal_mutation.projection_hash = runtime_scheduler_projection_hash(&ordinal_mutation);
        assert_eq!(
            ordinal_mutation.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "a rehashed wrapper cannot detach from the selected adapter ordinal"
        );
        let mut nonminimum_rebase = deferred_scheduling.clone();
        let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
            &mut nonminimum_rebase.candidate
        else {
            unreachable!("cloned deferred evidence retains its variant")
        };
        let invalid_lower_rank = candidate
            .lifecycle_ownership
            .owner
            .lifecycle_ordinal
            .checked_sub(1)
            .expect("aggregate fixture has a lower nonminimum rank");
        candidate.lifecycle_ownership.owner.lifecycle_ordinal = invalid_lower_rank;
        candidate
            .lifecycle_ownership
            .owner
            .causal_origin
            .root_lifecycle_ordinal = Some(invalid_lower_rank);
        candidate
            .lifecycle_ownership
            .owner
            .causal_origin
            .projection_hash = runtime_candidate_causal_origin_projection_hash(
            &candidate.lifecycle_ownership.owner.causal_origin,
        );
        candidate.lifecycle_ownership.owner.projection_hash =
            runtime_lifecycle_owner_projection_hash(&candidate.lifecycle_ownership.owner);
        nonminimum_rebase.projection_hash = runtime_scheduler_projection_hash(&nonminimum_rebase);
        assert_eq!(
            nonminimum_rebase.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "aggregate rebasing must equal the retained ingress minimum, not any lower rank"
        );
        let RuntimeStep::Advanced(deferred_effects) = deferred_step else {
            panic!("deferred target unexpectedly idled")
        };
        runtime
            .take_effect_ownership(deferred_effects.len())
            .expect("consume deferred target effect ownership");
        let first_terminals = runtime.take_leader_wire_runtime_terminals();
        let [first_terminal] = first_terminals.as_slice() else {
            panic!("servicing the first aggregate emits exactly its one terminal")
        };
        let first_terminal_receipt = match first_terminal {
            LeaderWireRuntimeTerminal::Volatile(receipt)
            | LeaderWireRuntimeTerminal::Producer {
                runtime: receipt, ..
            } => receipt,
        };
        assert_eq!(first_terminal_receipt, &first_receipt);
        assert_eq!(
            runtime.leader_wire_runtime_receipts,
            BTreeMap::from([(second_token.scheduler_ordinal(), second_receipt.clone(),)]),
            "the first terminal cannot consume the later origin-specific receipt"
        );

        let second_step = runtime
            .step(deadline)
            .expect("the later duplicate runs only after the Busy owner terminalizes");
        let second_scheduling = runtime
            .take_last_scheduler_ownership()
            .expect("the later duplicate retains its independent FIFO owner");
        assert_eq!(second_scheduling.selected, RuntimeSelectedOwnerKind::Fifo);
        let RuntimeSelectedCandidateOwnership::Exact(second_candidate) =
            &second_scheduling.candidate
        else {
            panic!("the later duplicate must remain an independent FIFO lifecycle")
        };
        assert_eq!(
            second_candidate.lifecycle_ordinal,
            second_token.scheduler_ordinal()
        );
        let RuntimeStep::Advanced(second_effects) = second_step else {
            panic!("the later aggregate unexpectedly idled after its predecessor terminalized")
        };
        runtime
            .take_effect_ownership(second_effects.len())
            .expect("consume later aggregate effect ownership");
        let second_terminals = runtime.take_leader_wire_runtime_terminals();
        let [second_terminal] = second_terminals.as_slice() else {
            panic!("the later aggregate emits exactly its own terminal")
        };
        let second_terminal_receipt = match second_terminal {
            LeaderWireRuntimeTerminal::Volatile(receipt)
            | LeaderWireRuntimeTerminal::Producer {
                runtime: receipt, ..
            } => receipt,
        };
        assert_eq!(second_terminal_receipt, &second_receipt);
        assert!(runtime.leader_wire_runtime_receipts.is_empty());
        assert!(runtime.deferred_ingress_ownership.is_empty());
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
        assert_eq!(runtime.queued_commands(), 0);
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn post_cut_old_logical_replay_cannot_overtake_fenced_busy_deferred_target() {
        let directory = TempDir::new().expect("temporary post-cut replay runtime directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let replay = signed_runtime_proposal(&context, &keys, 0xDA);
        let wire::ConsensusMessageV2Payload::Proposal(replay_proposal) = &replay.payload else {
            unreachable!("replay fixture carries Proposal")
        };
        let replay_origin = context.roster
            [usize::try_from(replay_proposal.proposer).expect("small fixture proposer")]
        .validator
        .clone();
        let target =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0xDB),
            ));
        let target_origin = context.roster[1].validator.clone();
        let (_leader_wire_directory, _leader_wire_ingress, ownerships) =
            preowned_leader_wire_ownerships(
                &context,
                &[
                    (replay.clone(), replay_origin),
                    (target.clone(), target_origin),
                ],
                runtime.ingress.lifecycle_ordinals.clone(),
            );
        let [mut replay_ownership, target_ownership]: [FairV2IngressOwnershipEvidence; 2] =
            ownerships
                .try_into()
                .expect("fixture creates one old-logical replay and one target");
        let replay_logical_ordinal = replay_ownership
            .runtime_lifecycle_ordinal()
            .expect("replay retains its old logical position");
        let target_logical_ordinal = target_ownership
            .runtime_lifecycle_ordinal()
            .expect("target retains its logical position");
        assert!(replay_logical_ordinal < target_logical_ordinal);
        let target_source_physical_ordinal = target_ownership
            .physical_admission_ordinal()
            .expect("target owns a checked physical occurrence");
        let target_physical_cut = target_ownership
            .runtime_physical_cut()
            .expect("target owns a checked physical cut");

        // Model a reconnect which retained the replay's immutable logical
        // identity but acquired a fresh physical position after the target's
        // checked-dequeue cut.
        let replay_source_physical_ordinal =
            u64::try_from(target_physical_cut).expect("small fixture cut fits u64");
        replay_ownership.first.physical_admission_ordinal = replay_source_physical_ordinal;
        replay_ownership.latest.physical_admission_ordinal = replay_source_physical_ordinal;
        replay_ownership.runtime_physical_cut = target_physical_cut.checked_add(1);
        assert!(replay_ownership.validate_exact());

        let start = Instant::now();
        runtime
            .arm_live_clocks(start)
            .expect("arm runtime before opening the shared signing fence");
        let deadline = start + runtime.round_timeout();
        let timeout_step = runtime
            .step(deadline)
            .expect("absolute deadline opens TimeoutVote signing");
        runtime
            .take_last_scheduler_ownership()
            .expect("timeout retains exact scheduler ownership");
        let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
            panic!("absolute deadline unexpectedly idled")
        };
        let timeout_ownership = runtime
            .take_effect_ownership(timeout_effects.len())
            .expect("TimeoutVote Sign retains its timeout root");
        let [timeout_ownership] = timeout_ownership.as_slice() else {
            panic!("TimeoutVote Sign has one exact owner")
        };
        let (sign_tag, signature_preimage) = match timeout_effects.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(vote),
                },
            ] => (*tag, vote.signature_preimage()),
            effects => panic!("unexpected timeout effects: {effects:?}"),
        };
        runtime
            .set_external_lifecycle_owners(vec![timeout_ownership.owner().clone()])
            .expect("publish pending TimeoutVote signer owner");

        runtime
            .enqueue_network_with_ingress_ownership(target.clone(), target_ownership)
            .expect("admit the target before the physical replay");
        runtime
            .set_ingress_physical_cut(
                target_physical_cut
                    .checked_add(1)
                    .expect("small target cut has a successor"),
            )
            .expect("later physical replay advances only the global high-watermark");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("target crosses into Busy-deferred ownership"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        let target_deferred_ordinal = runtime
            .driver()
            .all_deferred_admission_ordinals()
            .into_iter()
            .next()
            .expect("target owns one adapter-deferred ordinal");
        let target_deferred = &runtime.deferred_lifecycle_ownership[&target_deferred_ordinal];
        assert_eq!(
            target_deferred.source_physical_ordinal,
            Some(target_source_physical_ordinal)
        );
        assert_eq!(target_deferred.physical_cut, target_physical_cut);

        runtime
            .enqueue_network_with_ingress_ownership(replay.clone(), replay_ownership)
            .expect("admit the old-logical replay at its fresh physical position");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("replay reaches a distinct Busy-deferred lane"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(
            runtime.driver().all_deferred_admission_ordinals().len(),
            2,
            "different deferred classes retain independent bounded owners"
        );
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("pairwise physical selector remains exact"),
            BTreeSet::from([target_deferred_ordinal]),
            "the post-cut replay cannot reclaim its old logical priority"
        );

        let signature = Signature::new(keys[0].private_key(), &signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature_with_owner(sign_tag, signature, timeout_ownership)
            .expect("enqueue the exact owned TimeoutVote completion");
        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("retire pending signer after completion enqueue");
        let completion_step = runtime
            .step(deadline)
            .expect("the target-relative fence selector finds the exact completion");
        let completion_scheduling = runtime
            .take_last_scheduler_ownership()
            .expect("completion bypass retains scheduler evidence");
        assert_eq!(
            completion_scheduling.selected,
            RuntimeSelectedOwnerKind::FenceCompletion
        );
        assert_eq!(
            completion_scheduling.fence_predecessor_lifecycle_ordinal,
            Some(target_logical_ordinal)
        );
        assert_eq!(completion_scheduling.validate_exact(), Ok(()));
        let RuntimeStep::Advanced(completion_effects) = completion_step else {
            panic!("exact fence completion unexpectedly idled")
        };
        runtime
            .take_effect_ownership(completion_effects.len())
            .expect("consume completion effect ownership");

        let target_step = runtime
            .step(deadline)
            .expect("the pre-cut target owns service before the replay");
        let target_scheduling = runtime
            .take_last_scheduler_ownership()
            .expect("target service retains scheduler evidence");
        let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
            &target_scheduling.candidate
        else {
            panic!("expected exact deferred target ownership")
        };
        assert_eq!(
            candidate.lifecycle_ownership.physical_cut,
            target_physical_cut
        );
        assert_eq!(
            candidate.lifecycle_ownership.source_physical_ordinal,
            Some(target_source_physical_ordinal)
        );
        assert_eq!(
            candidate
                .ingress_ownership
                .as_ref()
                .expect("target retains authenticated provenance")
                .runtime_bytes
                .as_ref(),
            target.encode().as_slice(),
            "the selected deferred occurrence is the target, not the replay"
        );
        assert_eq!(target_scheduling.validate_exact(), Ok(()));
        let RuntimeStep::Advanced(target_effects) = target_step else {
            panic!("exact deferred target unexpectedly idled")
        };
        runtime
            .take_effect_ownership(target_effects.len())
            .expect("consume target effect ownership");
        let _ = runtime.take_leader_wire_runtime_terminals();
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
        let validation_step = runtime
            .step(before_timeout)
            .expect("dispatch validated-body completion");
        runtime
            .take_last_scheduler_ownership()
            .expect("validation retains exact scheduler ownership");
        let RuntimeStep::Advanced(validation_effects) = validation_step else {
            panic!("validation dispatch unexpectedly idle")
        };
        let prepare_effect_ownership = runtime
            .take_effect_ownership(validation_effects.len())
            .expect("Prepare signature request retains its lifecycle owner");
        let (prepare_sign_tag, prepare_signature_preimage) = match validation_effects.as_slice() {
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
        };
        assert_eq!(prepare_effect_ownership.len(), 1);
        runtime
            .set_external_lifecycle_owners(vec![prepare_effect_ownership[0].owner().clone()])
            .expect("publish pending Prepare signer owner");

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
            .enqueue_signature_with_owner(
                prepare_sign_tag,
                prepare_signature,
                &prepare_effect_ownership[0],
            )
            .expect("enqueue exact Prepare signature completion");
        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("retire pending Prepare signer owner after completion enqueue");
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
    fn real_adapter_fence_completion_breaks_pre_and_post_timeout_retransmit_debt() {
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

        // Service the first periodic episode before the signer becomes busy.
        // Every later tick in this view reconstructs this exact cached root,
        // including its immutable early lifecycle ordinal.
        let before_timeout = start + runtime.retransmit_interval();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("service pre-fence retransmission"),
            RuntimeStep::Advanced(_)
        ));

        let proposal = signed_runtime_proposal(&context, &keys, 0xE1);
        runtime
            .enqueue_network(proposal)
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
        let validation_step = runtime
            .step(before_timeout)
            .expect("dispatch validated-body completion");
        runtime
            .take_last_scheduler_ownership()
            .expect("validation macro-step retains exact scheduler ownership");
        let RuntimeStep::Advanced(validation_effects) = validation_step else {
            panic!("validation dispatch unexpectedly idled")
        };
        let prepare_effect_ownership = runtime
            .take_effect_ownership(validation_effects.len())
            .expect("Prepare signature request retains its lifecycle owner");
        let (prepare_sign_tag, prepare_signature_preimage) = match validation_effects.as_slice() {
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
        };
        assert_eq!(prepare_effect_ownership.len(), 1);
        runtime
            .set_external_lifecycle_owners(vec![prepare_effect_ownership[0].owner().clone()])
            .expect("publish the pending Prepare signer owner");

        // The second periodic episode is still before the absolute deadline.
        // Its cached root predates the proposal lifecycle, reaches the reducer
        // while Prepare signing is fenced, and becomes the oldest
        // Busy-deferred owner.
        let second_retransmission = before_timeout + runtime.retransmit_interval();
        assert!(second_retransmission < start + runtime.round_timeout());
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(second_retransmission)
                .expect("defer the pre-deadline second retransmission"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert!(
            !runtime.driver().deferred_work_is_serviceable(),
            "the exact Prepare signature still fences retransmission debt"
        );
        assert!(
            runtime.retransmit_owner.is_none(),
            "the cached retransmission root must not retain a second runtime alias"
        );

        let prepare_signature = Signature::new(keys[0].private_key(), &prepare_signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(prepare_sign_tag, prepare_signature.clone())
            .expect("enqueue an independently rooted signature callback");
        runtime
            .enqueue_signature_with_owner(
                prepare_sign_tag,
                prepare_signature,
                &prepare_effect_ownership[0],
            )
            .expect("enqueue exact Prepare signature completion");
        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("retire the pending Prepare signer owner after completion enqueue");
        assert_eq!(runtime.queued_commands(), 2);

        let prepare_broadcast = runtime
            .step(second_retransmission)
            .expect("owned Prepare completion opens the retransmission fence");
        let prepare_bypass = runtime
            .take_last_scheduler_ownership()
            .expect("fence completion retains exact scheduler ownership");
        assert_eq!(
            prepare_bypass.selected,
            RuntimeSelectedOwnerKind::FenceCompletion
        );
        assert!(prepare_bypass.fence_completion_bypass);
        assert!(
            prepare_bypass
                .fence_predecessor_lifecycle_ordinal
                .is_some_and(|predecessor| {
                    let RuntimeSelectedCandidateOwnership::Exact(candidate) =
                        &prepare_bypass.candidate
                    else {
                        return false;
                    };
                    predecessor < candidate.lifecycle_ordinal
                })
        );
        assert!(prepare_bypass.validate_exact().is_ok());
        let mut local_cut_mutation = prepare_bypass.clone();
        let mutated_local_cut = local_cut_mutation
            .fence_predecessor_ownership
            .as_ref()
            .expect("local retransmit fence carries its exact wrapper")
            .physical_cut
            .checked_add(1)
            .expect("small local cut has a successor");
        local_cut_mutation
            .fence_predecessor_ownership
            .as_mut()
            .expect("local retransmit fence carries its exact wrapper")
            .physical_cut = mutated_local_cut;
        local_cut_mutation.projection_hash = runtime_scheduler_projection_hash(&local_cut_mutation);
        assert_eq!(
            local_cut_mutation.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the adapter-private seal rejects a coherently rehashed local cut"
        );
        let mut local_rank_mutation = prepare_bypass.clone();
        let mutated_local_rank = {
            let wrapper = local_rank_mutation
                .fence_predecessor_ownership
                .as_mut()
                .expect("local retransmit fence carries its exact wrapper");
            let mutated = wrapper
                .owner
                .lifecycle_ordinal
                .checked_add(1)
                .expect("small local lifecycle rank has a successor");
            wrapper.owner.lifecycle_ordinal = mutated;
            wrapper.owner.causal_origin.root_lifecycle_ordinal = Some(mutated);
            wrapper.owner.causal_origin.projection_hash =
                runtime_candidate_causal_origin_projection_hash(&wrapper.owner.causal_origin);
            wrapper.owner.projection_hash = runtime_lifecycle_owner_projection_hash(&wrapper.owner);
            mutated
        };
        local_rank_mutation.fence_predecessor_lifecycle_ordinal = Some(mutated_local_rank);
        local_rank_mutation.projection_hash =
            runtime_scheduler_projection_hash(&local_rank_mutation);
        assert_eq!(
            local_rank_mutation.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the adapter-private seal rejects a coherently rehashed local logical rank"
        );
        let mut foreign_seal_mutation = prepare_bypass.clone();
        let foreign_wrapper = foreign_seal_mutation
            .fence_predecessor_ownership
            .as_mut()
            .expect("local retransmit fence carries its exact wrapper");
        foreign_wrapper.runtime_seal = DeferredRuntimeOwnershipSeal::for_test(
            foreign_wrapper.deferred_admission_ordinal,
            foreign_wrapper.owner.causal_origin().lifecycle_key.clone(),
            foreign_wrapper.owner.lifecycle_ordinal(),
            false,
            foreign_wrapper.source_physical_ordinal,
            foreign_wrapper.physical_cut,
        );
        foreign_seal_mutation.projection_hash =
            runtime_scheduler_projection_hash(&foreign_seal_mutation);
        assert_eq!(
            foreign_seal_mutation.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "a same-number foreign capability cannot replace the exact adapter seal"
        );
        let RuntimeStep::Advanced(prepare_broadcasts) = prepare_broadcast else {
            panic!("Prepare fence completion unexpectedly idled")
        };
        assert!(matches!(
            prepare_broadcasts.as_slice(),
            [AdapterEffect::Broadcast(message)]
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::Vote(vote)
                        if vote.phase == wire::GlobalPhase::Prepare
                            && vote.round == manifest.round
                            && vote.subject == manifest.subject
                )
        ));
        runtime
            .take_effect_ownership(prepare_broadcasts.len())
            .expect("test executor consumes Prepare broadcast ownership");
        assert!(
            runtime.retransmit_owner.is_none(),
            "the deferred retransmission remains the sole cached-root owner"
        );
        assert_eq!(
            runtime.queued_commands(),
            1,
            "the independently rooted callback cannot use the dependency bypass"
        );

        // Once the fence is open, the exact older retransmission debt runs and
        // rebroadcasts the newly published Prepare vote. Other finite deferred
        // work and the independently rooted callback then drain normally.
        let retransmit_retry = runtime
            .step_and_take_scheduler_ownership_for_test(second_retransmission)
            .expect("service older pre-deadline retransmission debt");
        assert!(matches!(
            retransmit_retry,
            RuntimeStep::Advanced(ref effects)
                if effects.iter().any(|effect| matches!(
                    effect,
                    AdapterEffect::Broadcast(message)
                        if matches!(
                            &message.payload,
                            wire::ConsensusMessageV2Payload::Vote(vote)
                                if vote.phase == wire::GlobalPhase::Prepare
                                    && vote.round == manifest.round
                        )
                ))
        ));
        assert_eq!(
            prepare_bypass.validate_exact(),
            Ok(()),
            "immutable fence evidence remains valid after its target is later claimed"
        );
        while runtime.driver().deferred_work_is_serviceable() {
            runtime
                .step_and_take_scheduler_ownership_for_test(second_retransmission)
                .expect("drain finite adapter debt after Prepare completion");
        }
        while runtime.queued_commands() != 0 {
            runtime
                .step_and_take_scheduler_ownership_for_test(second_retransmission)
                .expect("drain non-bypassing completion normally");
        }
        assert!(
            !runtime.fail_closed,
            "an independently rooted completion remains a recoverable ordinary FIFO occurrence"
        );

        // Absolute timeout remains one-shot after the pre-deadline dependency
        // cycle has drained. A drained cached retransmission root is not
        // replenished ahead of this still-unemitted timeout.
        let deadline = start + runtime.round_timeout();
        let timeout_macro_step = runtime
            .step(deadline)
            .expect("deliver the absolute timeout through the real adapter");
        runtime
            .take_last_scheduler_ownership()
            .expect("timeout macro-step retains exact scheduler ownership");
        let RuntimeStep::Advanced(timeout_effects) = timeout_macro_step else {
            panic!("absolute timeout unexpectedly idled")
        };
        let timeout_effect_ownership = runtime
            .take_effect_ownership(timeout_effects.len())
            .expect("timeout signature request retains its lifecycle owner");
        let (timeout_sign_tag, timeout_signature_preimage) = match timeout_effects.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(vote),
                },
            ] if vote.round == manifest.round => (*tag, vote.signature_preimage()),
            effects => panic!("unexpected timeout effects: {effects:?}"),
        };
        assert_eq!(timeout_effect_ownership.len(), 1);
        runtime
            .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
            .expect("publish the pending TimeoutVote signer owner");

        // The cached retransmission root becomes due again while TimeoutVote
        // signing is active. It is allowed one bounded turn and becomes
        // unserviceable Busy debt; it must not be resurrected over its later
        // exact completion on every subsequent call.
        let post_timeout_retransmission = deadline + runtime.retransmit_interval();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(post_timeout_retransmission)
                .expect("defer post-timeout retransmission behind signing"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert!(
            runtime.retransmit_owner.is_none(),
            "post-timeout deferred retransmission must not retain a runtime alias"
        );

        let timeout_signature = Signature::new(keys[0].private_key(), &timeout_signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature_with_owner(
                timeout_sign_tag,
                timeout_signature,
                &timeout_effect_ownership[0],
            )
            .expect("enqueue exact TimeoutVote signature completion");
        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("retire the pending TimeoutVote signer owner after completion enqueue");
        let first_timeout_vote = runtime
            .step(post_timeout_retransmission)
            .expect("owned TimeoutVote completion opens the retransmission fence");
        let timeout_bypass = runtime
            .take_last_scheduler_ownership()
            .expect("TimeoutVote completion retains exact scheduler ownership");
        assert_eq!(
            timeout_bypass.selected,
            RuntimeSelectedOwnerKind::FenceCompletion
        );
        assert!(timeout_bypass.fence_completion_bypass);
        assert!(timeout_bypass.fence_predecessor_lifecycle_ordinal.is_some());
        assert!(timeout_bypass.validate_exact().is_ok());
        let RuntimeStep::Advanced(first_timeout_vote_effects) = first_timeout_vote else {
            panic!("TimeoutVote fence completion unexpectedly idled")
        };
        assert!(first_timeout_vote_effects.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(message)
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                        if vote.round == manifest.round
                )
        )));
        runtime
            .take_effect_ownership(first_timeout_vote_effects.len())
            .expect("test executor consumes first TimeoutVote ownership");
        assert!(
            runtime.retransmit_owner.is_none(),
            "the deferred retransmission remains the sole post-timeout cached-root owner"
        );

        // Treat the first TimeoutVote broadcast as lost. The exact overdue
        // retransmission debt is still present and must rebroadcast it on the
        // next serialized turn rather than being permanently suppressed after
        // the absolute deadline.
        let timeout_vote_retry = runtime
            .step_and_take_scheduler_ownership_for_test(post_timeout_retransmission)
            .expect("rebroadcast a lost first TimeoutVote");
        assert!(matches!(
            timeout_vote_retry,
            RuntimeStep::Advanced(ref effects)
                if effects.iter().any(|effect| matches!(
                    effect,
                    AdapterEffect::Broadcast(message)
                        if matches!(
                            &message.payload,
                            wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                                if vote.round == manifest.round
                        )
                ))
        ));
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            runtime
                .driver()
                .all_deferred_admission_ordinals()
                .is_empty()
        );
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
        assert!(runtime.retransmit_owner.is_none());

        // A later periodic tick remains armed after the one-shot timeout and
        // continues broadcasting the published TimeoutVote.
        let later_post_timeout_tick = post_timeout_retransmission + runtime.retransmit_interval();
        let later_retry = runtime
            .step(later_post_timeout_tick)
            .expect("service a later post-timeout periodic tick");
        let later_retry_owner = runtime
            .take_last_scheduler_ownership()
            .expect("later periodic tick retains scheduler ownership");
        assert_eq!(
            later_retry_owner.selected,
            RuntimeSelectedOwnerKind::PeriodicTimer
        );
        assert!(later_retry_owner.validate_exact().is_ok());
        let RuntimeStep::Advanced(later_retry_effects) = later_retry else {
            panic!("later post-timeout periodic tick unexpectedly idled")
        };
        assert!(later_retry_effects.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(message)
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                        if vote.round == manifest.round
                )
        )));
        runtime
            .take_effect_ownership(later_retry_effects.len())
            .expect("test executor consumes later TimeoutVote retry ownership");
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            runtime
                .driver()
                .all_deferred_admission_ordinals()
                .is_empty()
        );
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
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
            FakeCommand::record(9)
                .exact_runtime_command_identity()
                .digest()
        );
        assert_eq!(candidate.kind, RuntimeCommandKind::Test);
        assert_eq!(candidate.class, SERVICE_CLASS_PROGRESS);
        assert_eq!(candidate.tag, owner_tag);
        assert_eq!(candidate.admission_ordinal, 2);
        assert_eq!(candidate.lifecycle_ordinal, 2);
        assert_eq!(candidate.causal_origin.root_lifecycle_ordinal, Some(2));
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
        candidate.identity.canonical_hash = iroha_crypto::Hash::new([0xFF]);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.identity = FakeCommand::record(42)
            .exact_runtime_command_identity()
            .digest();
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
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
        candidate.tag = tag(99);
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
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
        candidate.admission_ordinal = 0;
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.lifecycle_ordinal = candidate
            .lifecycle_ordinal
            .checked_add(1)
            .expect("small test lifecycle rank has a successor");
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        let replacement_origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            candidate.tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"coherently-rehashed-causal-root",
        );
        candidate.causal_origin =
            RuntimeLifecycleOwner::new(replacement_origin, candidate.lifecycle_ordinal)
                .expect("replacement causal root retains the same logical ordinal")
                .causal_origin()
                .clone();
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.admission_ordinal = candidate
            .lifecycle_ordinal
            .checked_sub(1)
            .expect("fresh FIFO lifecycle rank has a nonzero predecessor");
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
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
        mutated.queue_after.max_service_debt =
            evidence.queue_before.max_service_debt.saturating_add(2);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_before.service_cursor = SERVICE_CLASS_NONE;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
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
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);
    }

    #[test]
    fn scheduler_queue_seal_rejects_valid_same_wire_ingress_carrier_substitution() {
        let directory = TempDir::new().expect("temporary scheduler-ingress-seal directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before authenticated scheduler selection");
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0xA7),
            ));
        let original_source = PeerId::new(keys[0].public_key().clone());
        let replacement_source = PeerId::new(keys[1].public_key().clone());
        let replacement_ingress = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &message,
            fair_network_ownership(&message, replacement_source),
        )
        .expect("independent same-wire carrier has exact runtime ownership");
        assert!(replacement_ingress.validate_frozen_physical());

        runtime
            .enqueue_network_with_ingress_ownership(
                message.clone(),
                fair_network_ownership(&message, original_source),
            )
            .expect("original authenticated carrier enters the runtime FIFO");
        assert!(matches!(runtime.step(now), Ok(RuntimeStep::Advanced(_))));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("authenticated FIFO selection retains exact scheduler ownership")
            .clone();
        assert_eq!(evidence.validate_exact(), Ok(()));
        let RuntimeSelectedCandidateOwnership::Exact(original) = &evidence.candidate else {
            panic!("authenticated FIFO dispatch must retain one exact candidate")
        };
        let original_ingress = original
            .ingress_ownership
            .as_ref()
            .expect("authenticated candidate retains its full ingress carrier");
        assert_ne!(
            replacement_ingress.projection_hash, original_ingress.projection_hash,
            "independent sources have distinct complete ownership projections"
        );
        assert_eq!(
            runtime_ingress_causal_origin_projection_hash(&replacement_ingress),
            runtime_ingress_causal_origin_projection_hash(original_ingress),
            "equal aggregate certificates retain one route-neutral logical identity"
        );
        assert_eq!(
            replacement_ingress.earliest_physical_carrier(),
            original_ingress.earliest_physical_carrier(),
            "the independent test queues deliberately assign the same valid physical shape"
        );
        assert_eq!(
            replacement_ingress.earliest_lifecycle_ordinal(),
            original_ingress.earliest_lifecycle_ordinal(),
            "the replacement is rank-compatible before the private selection check"
        );

        let mut substituted = evidence;
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut substituted.candidate else {
            unreachable!();
        };
        candidate.ingress_ownership = Some(replacement_ingress);
        assert!(runtime_fifo_candidate_ingress_is_exact(candidate));
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        substituted.projection_hash = runtime_scheduler_projection_hash(&substituted);
        assert_eq!(
            substituted.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the queue-private seal rejects a valid same-wire full-carrier substitution after every public projection is recomputed"
        );
    }

    #[test]
    fn full_lane_retryable_backpressure_restores_and_services_exact_fifo_owner() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        assert!(driver.retry_once.insert(1));
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(3, 1, 1));
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("oldest retryable owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(2),
        )
        .expect("later completion owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("later progress owner fills the lane");
        assert_eq!(runtime.ingress.remaining_capacity(), 0);
        let original = runtime
            .ingress
            .commands
            .front()
            .expect("oldest physical owner is present")
            .clone();

        assert!(matches!(
            runtime.step(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("retry turn retains typed scheduler ownership")
            .clone();
        assert_eq!(
            evidence.selected,
            RuntimeSelectedOwnerKind::FifoRetryRetained
        );
        assert_eq!(evidence.queue_before.len, 3);
        assert_eq!(evidence.queue_after.len, 3);
        assert_eq!(evidence.validate_exact(), Ok(()));
        let restored = runtime
            .ingress
            .commands
            .front()
            .expect("retry restores the original physical owner");
        assert_eq!(restored.tag, original.tag);
        assert_eq!(restored.class, original.class);
        assert_eq!(restored.identity, original.identity);
        assert_eq!(restored.admission_ordinal, original.admission_ordinal);
        assert_eq!(restored.lifecycle_ordinal, original.lifecycle_ordinal);
        assert_eq!(restored.causal_origin, original.causal_origin);
        assert_eq!(runtime.driver.delivered, Vec::new());

        let mut weakened = evidence.clone();
        weakened.selected = RuntimeSelectedOwnerKind::Fifo;
        weakened.projection_hash = runtime_scheduler_projection_hash(&weakened);
        assert_eq!(
            weakened.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "an equal-length retry cannot be relabelled as completed FIFO service"
        );
        assert!(runtime.take_last_scheduler_ownership().is_some());
        assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 1)]);
        assert_eq!(runtime.ingress.len(), 2);
        assert_eq!(
            runtime
                .ingress
                .commands
                .front()
                .and_then(|queued| queued.command.record),
            Some(2),
            "later Completion work cannot overtake the retained lifecycle"
        );
    }

    #[test]
    fn retryable_backpressure_restores_the_exact_recovery_fifo_owner_once() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        assert!(driver.retry_once.insert(7));
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            driver,
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(4, 1, 1),
            Vec::new(),
        )
        .expect("construct unarmed recovery runtime");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("recovery owner fits");
        let original_owner = runtime
            .ingress
            .commands
            .front()
            .expect("recovery owner is present")
            .lifecycle_owner()
            .expect("recovery owner is exact");

        assert!(matches!(
            runtime.step_recovery(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("retrying recovery retains scheduler ownership");
        assert_eq!(
            evidence.selected,
            RuntimeSelectedOwnerKind::RecoveryFifoRetryRetained
        );
        assert_eq!(evidence.queue_before.len, evidence.queue_after.len);
        assert_eq!(evidence.validate_exact(), Ok(()));
        assert_eq!(
            runtime
                .ingress
                .commands
                .front()
                .expect("recovery retry remains physically admitted")
                .lifecycle_owner()
                .expect("restored recovery owner is exact"),
            original_owner
        );
        assert!(runtime.take_last_scheduler_ownership().is_some());
        assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));

        assert!(matches!(
            runtime.step_recovery_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 7)]);
        assert_eq!(runtime.queued_commands(), 0);
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
        assert_eq!(candidate.identity, expected.digest());
        assert_eq!(candidate.kind, RuntimeCommandKind::SignatureCompleted);
        assert_eq!(candidate.class, SERVICE_CLASS_COMPLETION);
        assert_eq!(candidate.tag, owner_tag);
        assert_eq!(candidate.admission_ordinal, 1);
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
        let mut nonempty_debt_on_empty_queue = idle
            .last_scheduler_ownership()
            .expect("idle branch retains its empty queue projection")
            .clone();
        nonempty_debt_on_empty_queue.queue_before.max_service_debt = 1;
        nonempty_debt_on_empty_queue.projection_hash =
            runtime_scheduler_projection_hash(&nonempty_debt_on_empty_queue);
        assert_eq!(
            nonempty_debt_on_empty_queue.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "a coherently rehashed empty queue cannot claim service debt"
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
                    && candidate.lifecycle_ownership.owner.lifecycle_ordinal() == 1
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
    fn runtime_rejects_driver_selection_outside_eligible_deferred_owner_set() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        driver.deferred_effects.push_back(vec![FakeEffect::other()]);
        let ineligible = DeferredServiceEvidence::completion_for_test(
            &driver.deferred_admission_ordinals,
            owner_tag,
            1,
            DeferredPriority::Completion,
        );
        assert_eq!(ineligible.admission_ordinal, 0);
        assert!(ineligible.claim_adapter_service_for_test());
        driver.deferred_evidence_overrides.push_back(ineligible);
        driver.deferred_active_ordinals.insert(1);

        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(6, 2, 1));
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"eligible-deferred-owner",
        );
        let owner = RuntimeLifecycleOwner::new(origin, 1)
            .expect("test target owns the global minimum lifecycle rank");
        let ownership = deferred_lifecycle_ownership_for_test(
            owner,
            1,
            RuntimeDispatchIngress::LocalOrCausal,
            None,
            runtime.ingress_physical_cut,
        )
        .expect("test target retains an exact runtime wrapper");
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(1, ownership)
                .is_none()
        );
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("the active target has one exact eligible owner"),
            BTreeSet::from([1])
        );

        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("deferred driver selected an ineligible admission owner")
        );
    }

    #[test]
    fn runtime_rejects_two_deferred_occurrences_for_one_logical_lifecycle() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        driver.deferred_effects.push_back(vec![FakeEffect::other()]);
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(6, 2, 1));
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"duplicate-deferred-logical-owner",
        );
        let owner = RuntimeLifecycleOwner::new(origin, 1)
            .expect("duplicate fixture owns one exact logical lifecycle");
        let physical_cut = runtime.ingress_physical_cut;
        let (first, second) = {
            let source = runtime.driver.deferred_admission_ordinal_source();
            let make = || {
                let runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
                    source,
                    owner.causal_origin().lifecycle_key.clone(),
                    owner.lifecycle_ordinal(),
                    false,
                    None,
                    physical_cut,
                );
                let ordinal = runtime_seal.admission_ordinal();
                let ownership = RuntimeDeferredLifecycleOwnership::new(
                    owner.clone(),
                    ordinal,
                    RuntimeDispatchIngress::LocalOrCausal,
                    None,
                    physical_cut,
                    runtime_seal,
                )
                .expect("each duplicate wrapper is independently well formed");
                (ordinal, ownership)
            };
            (make(), make())
        };
        for (ordinal, ownership) in [first, second] {
            runtime.driver.deferred_active_ordinals.insert(ordinal);
            assert!(
                runtime
                    .deferred_lifecycle_ownership
                    .insert(ordinal, ownership)
                    .is_none()
            );
        }

        assert!(matches!(
            runtime.eligible_deferred_admission_ordinals(),
            Err(EnqueueError::FailClosed)
        ));
        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert_eq!(runtime.driver.deferred_dispatches, 0);
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("deferred physical-cut lifecycle ownership was invalid")
        );
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
    fn checked_admission_reservation_rejection_preserves_and_reuses_the_owner() {
        let source = RuntimeLifecycleOrdinalSource::after_high_watermark(40);
        let rejected: Result<(), EnqueueError> =
            source.with_checked_reservation(1, |first, successor| {
                assert_eq!(first, 41);
                assert_eq!(successor, 42);
                Err(EnqueueError::FailClosed)
            });
        assert_eq!(rejected, Err(EnqueueError::FailClosed));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after rejected checked reservation"),
            Some(41),
            "a rejected checked admission cannot burn its prospective owner"
        );

        let admitted = source
            .with_checked_reservation(1, |first, successor| Ok((first, successor)))
            .expect("retry commits the same prospective owner");
        assert_eq!(admitted, (41, 42));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after committed retry"),
            Some(42)
        );
    }

    #[test]
    fn checked_ingress_rejection_preserves_dormant_owner_until_exact_retry() {
        let owner_tag = tag(0);
        let lifecycle_key = Hash::new(b"checked rejection dormant owner");
        let source = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            source.clone(),
        );
        let dormant = RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 9);
        ingress
            .install_dormant_local_fifo_reservations(vec![dormant.clone()])
            .expect("install one exact restart-dormant owner");
        let mirror_before = ingress.next_admission_ordinal;

        let rejected: Result<(), EnqueueError> =
            ingress.with_checked_admission_ordinal_range(1, |checked_ingress, first, successor| {
                assert_eq!((first, successor), (2, 3));
                assert!(
                    checked_ingress
                        .dormant_local_fifo_reservations
                        .contains(&dormant)
                );
                Err(EnqueueError::FailClosed)
            });
        assert_eq!(rejected, Err(EnqueueError::FailClosed));
        assert_eq!(ingress.next_admission_ordinal, mirror_before);
        assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
        assert!(ingress.commands.is_empty());
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after rejected dormant replacement"),
            Some(2)
        );

        ingress
            .enqueue(restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                lifecycle_key,
                1,
                9,
            ))
            .expect("exact retry reuses and commits the rejected prospective ordinal");
        assert!(ingress.dormant_local_fifo_reservations.is_empty());
        assert_eq!(ingress.commands.len(), 1);
        assert_eq!(ingress.commands[0].admission_ordinal, Some(2));
        assert_eq!(ingress.commands[0].lifecycle_ordinal, Some(1));
        assert_eq!(ingress.next_admission_ordinal, Some(3));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after exact dormant retry"),
            Some(3)
        );
    }

    #[test]
    fn checked_admission_reservation_exhaustion_never_enters_commit() {
        let source = RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 1);
        let commit_called = std::cell::Cell::new(false);
        for _ in 0..2 {
            let result: Result<(), EnqueueError> = source.with_checked_reservation(1, |_, _| {
                commit_called.set(true);
                Ok(())
            });
            assert_eq!(result, Err(EnqueueError::FailClosed));
            assert_eq!(
                source
                    .next_ordinal_for_test()
                    .expect("inspect exhausted checked source"),
                Some(u128::MAX),
                "exhaustion and retry must preserve the last prospective value"
            );
        }
        assert!(!commit_called.get());
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
        runtime.ingress.lifecycle_ordinals =
            RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 2);
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
        let next_before_rejection = runtime.ingress.next_admission_ordinal;
        let source_before_rejection = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source before exhausted FIFO admission");
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
        assert_eq!(runtime.ingress.commands.len(), 1);
        assert_eq!(
            runtime.ingress.next_admission_ordinal,
            next_before_rejection
        );
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect source after exhausted FIFO admission"),
            source_before_rejection,
            "failed FIFO admission cannot advance either ordinal representation"
        );
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
    fn corrupt_cached_identity_and_rebound_origin_are_rejected_before_service() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
        let mut corrupt = TaggedCommand::new(
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
            admitted_at,
        );
        corrupt.identity.canonical_hash = iroha_crypto::Hash::new(b"corrupt cached identity");
        assert_eq!(ingress.enqueue(corrupt), Err(EnqueueError::FailClosed));
        assert!(ingress.commands.is_empty());

        let root = FakeCommand::record(2);
        let mut origin =
            RuntimeCandidateCausalOrigin::mint(owner_tag, CommandClass::Normal, &root, None);
        assert!(origin.bind_lifecycle_ordinal(7));
        assert!(matches!(
            TaggedCommand::with_causal_origin(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(3),
                admitted_at,
                origin,
                8,
            ),
            Err(EnqueueError::FailClosed)
        ));
    }

    #[test]
    fn lifecycle_owner_constructor_rejects_a_conflicting_prebound_ordinal() {
        let owner_tag = tag(0);
        let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"prebound-owner",
        );
        assert!(origin.bind_lifecycle_ordinal(7));
        assert!(matches!(
            RuntimeLifecycleOwner::new(origin.clone(), 8),
            Err(EnqueueError::FailClosed)
        ));
        let exact = RuntimeLifecycleOwner::new(origin, 7)
            .expect("the already-bound exact ordinal remains admissible");
        assert!(exact.validate_exact());
        assert_eq!(exact.lifecycle_ordinal(), 7);
    }

    #[test]
    fn runtime_physical_cut_is_monotone_and_regression_fails_closed() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        assert_eq!(runtime.ingress_physical_cut, 1);
        runtime
            .set_ingress_physical_cut(4)
            .expect("receiver high-watermark advances");
        runtime
            .set_ingress_physical_cut(4)
            .expect("publishing the same high-watermark is idempotent");
        assert_eq!(runtime.ingress_physical_cut, 4);
        assert!(runtime.set_ingress_physical_cut(3).is_err());
        assert!(runtime.fail_closed);
        assert_eq!(runtime.ingress_physical_cut, 4);
    }

    #[test]
    fn deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences() {
        let directory = TempDir::new().expect("temporary physical-cut runtime directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
        let message = signed_runtime_proposal(&context, &keys, 0x5A);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            unreachable!("signed runtime proposal fixture carries Proposal")
        };
        let semantic_origin = context.roster
            [usize::try_from(proposal.proposer).expect("small fixture proposer")]
        .validator
        .clone();
        let (_owner_directory, _owner_ingress, mut ownerships) = preowned_leader_wire_ownerships(
            &context,
            &[(message.clone(), semantic_origin)],
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let pre_cut_fair = ownerships
            .pop()
            .expect("one productive leader-wire ownership carrier");
        let predecessor_ordinal = pre_cut_fair
            .runtime_lifecycle_ordinal()
            .expect("leader-wire carrier has an immutable logical ordinal");
        let target_cut = pre_cut_fair
            .runtime_physical_cut()
            .expect("checked dequeue freezes the target predecessor cut");
        assert!(
            u128::from(
                pre_cut_fair
                    .physical_admission_ordinal()
                    .expect("leader-wire carrier has a physical occurrence")
            ) < target_cut
        );

        let target_owner = runtime
            .mint_fresh_lifecycle_owner(
                runtime.round_tag(),
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"already-admitted deferred continuation",
            )
            .expect("mint target lifecycle after the leader-wire predecessor");
        assert!(predecessor_ordinal < target_owner.lifecycle_ordinal());
        let target = deferred_lifecycle_ownership_for_test(
            target_owner.clone(),
            7,
            RuntimeDispatchIngress::LocalOrCausal,
            None,
            target_cut,
        )
        .expect("freeze the target physical cut exactly once");
        assert!(matches!(
            deferred_lifecycle_ownership_for_test(
                target_owner.clone(),
                7,
                RuntimeDispatchIngress::LocalOrCausal,
                Some(u64::try_from(target_cut).expect("small target cut")),
                target_cut,
            ),
            Err(EnqueueError::FailClosed)
        ));
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(7, target.clone())
                .is_none()
        );
        let foreign_source = DeferredAdmissionOrdinalSource::new(7);
        let mut foreign_target = target.clone();
        foreign_target.runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
            &foreign_source,
            foreign_target.owner.causal_origin().lifecycle_key.clone(),
            foreign_target.owner.lifecycle_ordinal(),
            false,
            None,
            foreign_target.physical_cut,
        );
        assert!(
            foreign_target.validate_exact(),
            "the foreign capability can be internally self-consistent"
        );
        assert!(
            !foreign_target.validate_active_against_ingress(
                None,
                runtime.driver.deferred_admission_ordinal_source(),
            ),
            "a same-number capability minted by another source cannot own this runtime"
        );

        let make_command = |runtime: &SerializedV2Runtime<SumeragiV2Adapter>,
                            fair: FairV2IngressOwnershipEvidence| {
            let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, fair)
                .expect("project exact leader-wire ownership into runtime");
            let authenticated = runtime
                .driver
                .authenticate(message.clone())
                .expect("authenticate the exact leader-wire proposal");
            TaggedCommand::with_ingress_ownership(
                runtime.round_tag(),
                CommandClass::Normal,
                AdapterCommand::Authenticated(authenticated),
                Instant::now(),
                ownership,
            )
        };

        let pre_cut_command = make_command(&runtime, pre_cut_fair.clone());
        runtime
            .ingress
            .enqueue(pre_cut_command)
            .expect("enqueue the real pre-cut predecessor");
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("pre-cut minimum is exact"),
            Some(predecessor_ordinal),
            "a physical predecessor with an older logical identity still blocks"
        );

        runtime.ingress.commands.clear();
        let mut post_cut_fair = pre_cut_fair;
        let post_cut_ordinal =
            u64::try_from(target_cut).expect("small receiver-local physical cut");
        post_cut_fair.first.physical_admission_ordinal = post_cut_ordinal;
        post_cut_fair.latest.physical_admission_ordinal = post_cut_ordinal;
        post_cut_fair.runtime_physical_cut = target_cut.checked_add(1);
        assert!(
            post_cut_fair.validate_exact(),
            "the replay retains its exact logical identity at a fresh physical occurrence"
        );
        let post_cut_command = make_command(&runtime, post_cut_fair);
        runtime
            .ingress
            .enqueue(post_cut_command)
            .expect("enqueue the exact post-cut replay");
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("post-cut minimum is exact"),
            Some(target_owner.lifecycle_ordinal()),
            "a post-cut replay cannot resurrect its obsolete logical queue position"
        );

        let replay_owner = runtime
            .ingress
            .commands
            .front()
            .expect("post-cut replay remains physically queued")
            .lifecycle_owner()
            .expect("post-cut replay retains its old logical owner");
        let replay_ingress = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.clone())
            .expect("post-cut replay retains its exact ingress carrier");
        runtime.ingress.commands.clear();
        let causal_completion = TaggedCommand::with_causal_origin(
            runtime.round_tag(),
            CommandClass::Completion,
            AdapterCommand::ApplicationCompleted(proposal.subject),
            Instant::now(),
            replay_owner.causal_origin().clone(),
            replay_owner.lifecycle_ordinal(),
        )
        .expect("construct a local completion inheriting the replay root");
        runtime
            .ingress
            .enqueue(causal_completion)
            .expect("enqueue the post-cut causal completion");
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("post-cut causal FIFO minimum is exact"),
            Some(target_owner.lifecycle_ordinal()),
            "dropping the current envelope cannot drop the causal root's physical position"
        );
        runtime.ingress.commands.clear();
        runtime.pending_effect_ownership = Some(vec![RuntimeEffectOwnership::inherited(
            replay_owner.clone(),
        )]);
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("post-cut effect minimum is exact"),
            Some(target_owner.lifecycle_ordinal()),
            "post-cut effect and external work cannot reclaim the root's old logical rank"
        );
        runtime.pending_effect_ownership = None;
        let replay = deferred_lifecycle_ownership_for_test(
            replay_owner,
            8,
            RuntimeDispatchIngress::DirectAuthenticated,
            Some(post_cut_ordinal),
            target_cut
                .checked_add(1)
                .expect("small target cut has a successor"),
        )
        .expect("post-cut replay can cross into a distinct Busy-deferred owner");
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(8, replay)
                .is_none()
        );
        assert!(
            runtime
                .deferred_ingress_ownership
                .insert(8, replay_ingress)
                .is_none()
        );
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("deferred post-cut minimum is exact"),
            Some(target_owner.lifecycle_ordinal()),
            "crossing Busy cannot turn the post-cut replay into a predecessor"
        );
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("pairwise deferred cut relation is exact"),
            BTreeSet::from([7]),
            "the earlier target remains the sole runner-eligible continuation"
        );

        // Pairwise target-relative precedence can form a cycle even though
        // every source/cut pair is individually exact: B logically precedes
        // A, C logically precedes B, and A physically precedes C.  The global
        // selector must first exclude C as post-A-cut, then choose B by
        // logical rank.  Retiring each selected owner yields B, A, C without
        // a lasso or an empty eligible set.
        runtime.deferred_ingress_ownership.clear();
        runtime.deferred_lifecycle_ownership.clear();
        let (a, b, c) = {
            let source = runtime.driver.deferred_admission_ordinal_source();
            let make_owner = |semantic_identity: &[u8],
                              source_physical_ordinal: Option<u64>,
                              physical_cut: u128,
                              lifecycle_ordinal: u128| {
                let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
                    runtime.round_tag(),
                    CommandClass::Progress,
                    RuntimeFreshRootKind::StartupRecovery,
                    semantic_identity,
                );
                if let Some(source_physical_ordinal) = source_physical_ordinal {
                    origin.root_ingress_identity = Some(Hash::new(semantic_identity));
                    origin.root_ingress_physical_ownership =
                        Some(RuntimeIngressPhysicalOwnership {
                            source_ordinal: source_physical_ordinal,
                            physical_cut,
                        });
                    origin.lifecycle_key = runtime_candidate_causal_origin_lifecycle_key(&origin);
                }
                let owner = RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
                    .expect("cycle fixture owns an exact logical lifecycle");
                let runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
                    source,
                    owner.causal_origin().lifecycle_key.clone(),
                    owner.lifecycle_ordinal(),
                    false,
                    source_physical_ordinal,
                    physical_cut,
                );
                let admission_ordinal = runtime_seal.admission_ordinal();
                let ownership = RuntimeDeferredLifecycleOwnership::new(
                    owner,
                    admission_ordinal,
                    RuntimeDispatchIngress::LocalOrCausal,
                    source_physical_ordinal,
                    physical_cut,
                    runtime_seal,
                )
                .expect("cycle fixture retains an exact source-bound runtime seal");
                assert!(ownership.validate_active_against_ingress(None, source));
                (admission_ordinal, ownership)
            };
            (
                make_owner(b"cycle-a", None, 5, 3),
                make_owner(b"cycle-b", Some(4), 9, 2),
                make_owner(b"cycle-c", Some(8), 12, 1),
            )
        };
        for (ordinal, ownership) in [a.clone(), b.clone(), c.clone()] {
            assert!(
                runtime
                    .deferred_lifecycle_ownership
                    .insert(ordinal, ownership)
                    .is_none()
            );
        }
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("two-stage selector breaks the physical/logical cycle"),
            BTreeSet::from([b.0])
        );
        assert!(runtime.deferred_lifecycle_ownership.remove(&b.0).is_some());
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("A becomes eligible after B retires"),
            BTreeSet::from([a.0])
        );
        assert!(runtime.deferred_lifecycle_ownership.remove(&a.0).is_some());
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("C becomes eligible only after its physical predecessor retires"),
            BTreeSet::from([c.0])
        );
    }

    #[test]
    fn global_lifecycle_minimum_blocks_later_fifo_until_its_completion_arrives() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        let older = runtime
            .mint_fresh_lifecycle_owner(
                owner_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"older external exact request",
            )
            .expect("mint the older externally retained lifecycle");
        runtime
            .configure_external_lifecycle_owner_capacity(4)
            .expect("install the independent asynchronous bound");
        runtime
            .set_external_lifecycle_owners(vec![older.clone()])
            .expect("publish the older external owner");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(9),
        )
        .expect("enqueue later unrelated work");

        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Idle)));
        let idle = runtime
            .take_last_scheduler_ownership()
            .expect("blocked scheduling still publishes exact Idle evidence");
        assert_eq!(idle.selected, RuntimeSelectedOwnerKind::Idle);
        assert!(!idle.fifo_ready);
        assert_eq!(runtime.queued_commands(), 1);

        let due = start + Duration::from_secs(10);
        assert!(matches!(runtime.step(due), Ok(RuntimeStep::Idle)));
        runtime
            .take_last_scheduler_ownership()
            .expect("blocked due clocks publish exact Idle evidence");
        assert!(runtime.timeout_owner.is_some());
        assert!(
            runtime.retransmit_owner.is_none(),
            "an absolute timeout suppresses replenishing the periodic owner until the timeout drains"
        );
        assert!(runtime.driver.timeouts.is_empty());
        assert!(runtime.driver.retransmits.is_empty());

        let older_effect = RuntimeEffectOwnership::fresh(
            older.clone(),
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
        );
        runtime
            .enqueue_with_lifecycle_owner(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                &older_effect,
            )
            .expect("enqueue the exact older completion");
        assert!(matches!(
            runtime.step(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("completion selection publishes exact ownership");
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = selected.candidate else {
            panic!("older completion must be the exact FIFO candidate");
        };
        assert_eq!(candidate.fifo_position, 1);
        assert_eq!(candidate.lifecycle_ordinal, older.lifecycle_ordinal());
        runtime
            .take_effect_ownership(1)
            .expect("test executor consumes the completion effect owner");
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 1)]);
        assert_eq!(runtime.queued_commands(), 1);

        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("the asynchronous owner retires after its exact completion handoff");
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the older queued FIFO command now drains");
        assert_eq!(
            runtime.driver.delivered,
            vec![(owner_tag, 1), (owner_tag, 9)]
        );
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the frozen timeout drains after all older lifecycles");
        assert_eq!(runtime.driver.timeouts, vec![owner_tag]);
        assert!(runtime.timeout_owner.is_none());
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the later frozen retransmission drains next");
        assert_eq!(runtime.driver.retransmits, vec![owner_tag]);
        assert!(runtime.retransmit_owner.is_none());
    }

    #[test]
    fn external_owner_bound_uses_effect_capacity_not_small_ingress_capacity() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        let pending_bound = 1_024usize;
        runtime
            .configure_external_lifecycle_owner_capacity(pending_bound)
            .expect("configure the executor's independent pending-work bound");
        let exact_capacity = pending_bound + MAX_EFFECTS_PER_STEP;
        let owners = (0..exact_capacity)
            .map(|ordinal| {
                let ordinal = u128::try_from(ordinal).expect("small test owner ordinal");
                let semantic = ordinal.to_le_bytes();
                RuntimeLifecycleOwner::new(
                    RuntimeCandidateCausalOrigin::mint_fresh_root(
                        owner_tag,
                        CommandClass::Progress,
                        RuntimeFreshRootKind::HistoricalLockedRetransmit,
                        &semantic,
                    ),
                    ordinal,
                )
                .expect("synthetic external owner binds its first ordinal")
            })
            .collect::<Vec<_>>();
        runtime
            .set_external_lifecycle_owners(owners)
            .expect("1024 pending owners plus one retained batch fit despite ingress capacity 8");
        assert_eq!(runtime.external_lifecycle_owners.len(), exact_capacity);
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn restart_and_periodic_historical_retries_reuse_one_lifecycle_owner() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let historical = FakeEffect::historical(0xA5);
        let (mut runtime, startup) = SerializedV2Runtime::with_driver(
            FakeDriver::new(owner_tag),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            vec![historical],
        )
        .expect("construct deterministic restart ownership");
        assert_eq!(startup, vec![historical]);
        let startup_owner = runtime
            .take_effect_ownership(1)
            .expect("consume startup ownership")
            .pop()
            .expect("one startup owner");
        assert_eq!(
            startup_owner.causality(),
            RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::StartupRecovery)
        );
        runtime
            .arm_live_clocks(start)
            .expect("startup dispatch completes before clocks arm");
        runtime.driver.timer_effects.push_back(vec![historical]);
        runtime.driver.timer_effects.push_back(vec![historical]);

        let mut retry_owners = Vec::new();
        for elapsed in [2, 4] {
            let RuntimeStep::Advanced(effects) = runtime
                .step(start + Duration::from_secs(elapsed))
                .expect("periodic historical retry dispatches")
            else {
                panic!("periodic historical retry must advance");
            };
            assert_eq!(effects, vec![historical]);
            runtime
                .take_last_scheduler_ownership()
                .expect("periodic retry publishes scheduler ownership");
            retry_owners.push(
                runtime
                    .take_effect_ownership(1)
                    .expect("consume retry ownership")
                    .pop()
                    .expect("one retry owner"),
            );
        }
        assert!(retry_owners.iter().all(|ownership| {
            ownership.causality()
                == RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::HistoricalLockedRetransmit)
                && ownership.owner() == startup_owner.owner()
        }));
        let cache_after_owned_retries = runtime.dormant_fresh_lifecycle_owners.len();
        assert_ne!(cache_after_owned_retries, 0);
        for elapsed in [6, 8] {
            let RuntimeStep::Advanced(effects) = runtime
                .step(start + Duration::from_secs(elapsed))
                .expect("drained historical lifecycle still services its periodic clock")
            else {
                panic!("the periodic clock must advance even after exact work drains")
            };
            assert!(
                effects.is_empty(),
                "a drained exact historical request cannot recreate physical work"
            );
            runtime
                .take_last_scheduler_ownership()
                .expect("proofless periodic stutter retains scheduler ownership");
            assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
            assert_eq!(runtime.queued_commands(), 0);
            assert_eq!(
                runtime.dormant_fresh_lifecycle_owners.len(),
                cache_after_owned_retries,
                "fresh periodic episodes replace one bounded cache slot rather than growing it"
            );
        }
        assert_eq!(runtime.driver.retransmits, vec![owner_tag; 4]);

        let next_tag = tag(1);
        runtime
            .observe_effects_with_test_ownership(
                start + Duration::from_secs(9),
                &[FakeEffect::enter_view(next_tag)],
            )
            .expect("test EnterView retains positional producer ownership");
        assert!(
            runtime.dormant_fresh_lifecycle_owners.is_empty(),
            "certified view transition purges every prior-view dormant alias"
        );
    }

    #[test]
    fn dormant_fresh_owner_cache_is_derived_bounded_and_purged_by_view() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let queue = RuntimeQueueConfig::new(8, 2, 2);
        let exact_capacity = queue.capacity + MAX_EFFECTS_PER_STEP;
        let mut runtime = runtime(FakeDriver::new(owner_tag), start, queue);
        let mut last_ordinal = None;
        for identity in 0..exact_capacity {
            let identity = u128::try_from(identity)
                .expect("small dormant-cache fixture")
                .to_le_bytes();
            let owner = runtime
                .mint_fresh_lifecycle_owner(
                    owner_tag,
                    CommandClass::Progress,
                    RuntimeFreshRootKind::HistoricalLockedRetransmit,
                    &identity,
                )
                .expect("derived dormant-cache capacity admits every configured owner");
            last_ordinal = Some(owner.lifecycle_ordinal());
        }
        assert_eq!(runtime.dormant_fresh_lifecycle_owners.len(), exact_capacity);
        assert_eq!(
            runtime.mint_fresh_lifecycle_owner(
                owner_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"one owner beyond the derived bound",
            ),
            Err(EnqueueError::Full)
        );

        let next_tag = tag(1);
        runtime
            .observe_effects_with_test_ownership(start, &[FakeEffect::enter_view(next_tag)])
            .expect("test EnterView retains positional producer ownership");
        assert!(runtime.dormant_fresh_lifecycle_owners.is_empty());
        let successor = runtime
            .mint_fresh_lifecycle_owner(
                next_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"successor-view exact request",
            )
            .expect("view reclamation reopens the same derived cache geometry");
        assert!(
            successor.lifecycle_ordinal() > last_ordinal.expect("cache was filled"),
            "cache reclamation cannot reuse an old admission ordinal"
        );
    }

    #[test]
    fn causal_successors_inherit_root_and_lifecycle_ordinal() {
        let admitted_at = Instant::now();
        let root_tag = tag(0);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 2, 2));
        ingress
            .enqueue(TaggedCommand::new(
                root_tag,
                CommandClass::Normal,
                FakeCommand::record(1),
                admitted_at,
            ))
            .expect("root candidate is admitted");
        let (root, root_owner) = ingress
            .pop_next_with_ownership()
            .expect("root selection is exact")
            .expect("root candidate is ready");
        assert_eq!(root.lifecycle_ordinal, Some(root_owner.lifecycle_ordinal));

        let successor_tag = EventTag::new(
            root_tag.height(),
            root_tag.view() + 1,
            Generation::new(root_tag.generation().get() + 1),
        );
        for value in [2, 3, 4] {
            ingress
                .enqueue(
                    TaggedCommand::with_causal_origin(
                        successor_tag,
                        CommandClass::Completion,
                        FakeCommand::record(value),
                        admitted_at,
                        root_owner.causal_origin.clone(),
                        root_owner.lifecycle_ordinal,
                    )
                    .expect("causal owner is internally consistent"),
                )
                .expect("causal child is admitted with a unique physical owner");
        }

        let physical_ordinals = ingress
            .commands
            .iter()
            .map(|candidate| {
                assert_eq!(
                    candidate.causal_origin, root_owner.causal_origin,
                    "evidence/view rewriting cannot replace the first-admission root"
                );
                assert_eq!(
                    candidate.lifecycle_ordinal,
                    Some(root_owner.lifecycle_ordinal),
                    "every child inherits one logical lifecycle ordinal"
                );
                candidate
                    .admission_ordinal
                    .expect("every physical child has its own FIFO ordinal")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(physical_ordinals.len(), 3);

        let unrelated = TaggedCommand::new(
            successor_tag,
            CommandClass::Completion,
            FakeCommand::record(2),
            admitted_at,
        );
        assert!(
            !unrelated
                .causal_origin
                .same_lifecycle(&root_owner.causal_origin),
            "a physically similar command with a different causal root cannot coalesce"
        );
    }

    #[test]
    fn preassigned_batch_lifecycles_require_shared_mint_and_exact_root() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let unminted_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut unminted_ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            unminted_source.clone(),
        );
        let unminted_command = FakeCommand::record(1);
        let mut unminted_origin = RuntimeCandidateCausalOrigin::mint(
            owner_tag,
            CommandClass::Completion,
            &unminted_command,
            None,
        );
        assert!(unminted_origin.bind_lifecycle_ordinal(1));
        let unminted = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            unminted_command,
            admitted_at,
            unminted_origin,
            1,
        )
        .expect("construct internally exact but unminted lifecycle");
        assert_eq!(
            unminted_ingress.enqueue_completion_batch(vec![unminted]),
            Err(EnqueueError::FailClosed)
        );
        assert!(unminted_ingress.commands.is_empty());
        assert_eq!(
            unminted_source
                .next_ordinal_for_test()
                .expect("unminted batch rejection preserves the source"),
            Some(1)
        );

        let collision_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut collision_ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            collision_source.clone(),
        );
        collision_ingress
            .enqueue(TaggedCommand::new(
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(2),
                admitted_at,
            ))
            .expect("mint one exact lifecycle root");
        let (_, root_owner) = collision_ingress
            .pop_next_with_ownership()
            .expect("select the minted root exactly")
            .expect("root is ready");
        let sibling = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(3),
            admitted_at,
            root_owner.causal_origin.clone(),
            root_owner.lifecycle_ordinal,
        )
        .expect("construct one legitimate causal sibling");
        let conflicting_command = FakeCommand::record(4);
        let mut conflicting_origin = RuntimeCandidateCausalOrigin::mint(
            owner_tag,
            CommandClass::Completion,
            &conflicting_command,
            None,
        );
        assert!(conflicting_origin.bind_lifecycle_ordinal(root_owner.lifecycle_ordinal));
        let conflicting = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            conflicting_command,
            admitted_at,
            conflicting_origin,
            root_owner.lifecycle_ordinal,
        )
        .expect("construct a distinct root at the colliding ordinal");
        let next_before_collision = collision_source
            .next_ordinal_for_test()
            .expect("inspect source before batch collision");
        assert_eq!(
            collision_ingress.enqueue_completion_batch(vec![sibling, conflicting]),
            Err(EnqueueError::FailClosed)
        );
        assert!(
            collision_ingress.commands.is_empty(),
            "batch collision must reject atomically"
        );
        assert_eq!(
            collision_source
                .next_ordinal_for_test()
                .expect("batch collision preserves the source"),
            next_before_collision,
            "collision validation must run before reserving physical positions"
        );
    }

    #[test]
    fn restart_dormant_local_fifo_reservation_survives_full_class_churn() {
        let started_at = Instant::now();
        let owner_tag = tag(0);
        let lifecycle_key = Hash::new(b"restart dormant Local FIFO lifecycle");
        let mut driver = FakeDriver::new(owner_tag);
        driver.dormant_local_fifo_reservations =
            vec![RuntimeDormantLocalFifoReservation::completion(
                lifecycle_key,
                1,
                8,
            )];
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
        let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            driver,
            started_at,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            Vec::new(),
            lifecycle_ordinals,
        )
        .expect("restart installs exact latent FIFO ownership")
        .0;
        runtime
            .arm_live_clocks(started_at)
            .expect("arm the restarted runtime without advancing its latent owner");
        assert_eq!(
            runtime.remaining_completion_capacity(),
            4,
            "the dormant Local stage consumes one physical completion slot"
        );
        let later_serve = runtime
            .ingress
            .lifecycle_ordinals
            .reserve_one()
            .expect("mint a later exact Serve ticket");
        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(started_at, later_serve)
                .expect("latent FIFO owner participates in the active minimum"),
            "the restart-dormant owner must remain ahead of later Serve work"
        );

        for value in [1, 2] {
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .expect("ordinary churn fills only the remaining normal prefix");
        }
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(3),
            ),
            Err(EnqueueError::ReservedCapacity),
            "normal churn cannot acquire the dormant target's slot"
        );
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(4),
        )
        .expect("progress fills its existing prefix");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(5),
        )
        .expect("a trusted completion fills the last unreserved position");
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        assert!(
            matches!(runtime.step(started_at), Ok(RuntimeStep::Idle)),
            "later Completion, Progress, and Normal commands must idle behind the latent minimum"
        );
        let idle_ownership = runtime
            .take_last_scheduler_ownership()
            .expect("the blocked turn retains exact idle ownership");
        assert_eq!(idle_ownership.selected, RuntimeSelectedOwnerKind::Idle);
        assert!(
            runtime.driver.delivered.is_empty(),
            "no younger physical command may dispatch before exact replacement"
        );

        runtime.driver.admission_preflight_override =
            Some(RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key: lifecycle_key,
                admission_ordinal: 1,
                producer_stage: 8,
            });
        let next_before_replay = runtime.ingress.next_admission_ordinal;
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .expect("exact retry atomically replaces its latent slot at full capacity");
        assert!(runtime.ingress.dormant_local_fifo_reservations.is_empty());
        assert_eq!(runtime.queued_commands(), 5);
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        assert_eq!(
            runtime.minimum_active_lifecycle_ordinal(),
            Ok(Some(1)),
            "the restored FIFO owner retains the pre-restart lifecycle age"
        );

        let next_after_replay = runtime.ingress.next_admission_ordinal;
        assert_ne!(
            next_after_replay, next_before_replay,
            "the first physical replay receives one fresh FIFO position"
        );
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .expect("duplicate exact retry coalesces with the one physical owner");
        assert_eq!(runtime.queued_commands(), 5);
        assert_eq!(
            runtime.ingress.next_admission_ordinal, next_after_replay,
            "coalescing cannot mint another physical admission ordinal"
        );

        let RuntimeStep::Advanced(effects) = runtime
            .step(started_at)
            .expect("the exact replacement becomes the global ready owner")
        else {
            panic!("the exact replacement must dispatch before younger queued work");
        };
        assert!(effects.is_empty());
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("the replacement dispatch retains exact FIFO ownership");
        assert_eq!(selected.selected, RuntimeSelectedOwnerKind::Fifo);
        assert_eq!(
            runtime.driver.delivered,
            vec![(owner_tag, 9)],
            "the restored target dispatches before every younger physical command"
        );
        assert_eq!(runtime.queued_commands(), 4);

        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(9),
            ),
            Err(EnqueueError::FailClosed),
            "ReuseDormant after latent-slot removal cannot recreate the drained stage"
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.queued_commands(),
            4,
            "rejected resurrection cannot install another physical owner"
        );
    }

    #[test]
    fn restart_dormant_completion_batch_atomically_replaces_latent_slots() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let first_key = Hash::new(b"first dormant validation lifecycle");
        let second_key = Hash::new(b"second dormant validation lifecycle");
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            RuntimeLifecycleOrdinalSource::after_high_watermark(2),
        );
        ingress
            .install_dormant_local_fifo_reservations(vec![
                RuntimeDormantLocalFifoReservation::completion(first_key, 1, 9),
                RuntimeDormantLocalFifoReservation::completion(second_key, 2, 9),
            ])
            .expect("restart installs two exact completion reservations");
        for value in [1, 2] {
            ingress
                .enqueue(TaggedCommand::new(
                    owner_tag,
                    CommandClass::Completion,
                    FakeCommand::record(value),
                    admitted_at,
                ))
                .expect("ordinary completions fill the unreserved positions");
        }
        assert_eq!(ingress.remaining_capacity(), 0);
        let batch = vec![
            restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(3),
                first_key,
                1,
                9,
            ),
            restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(4),
                second_key,
                2,
                9,
            ),
        ];
        ingress
            .enqueue_completion_batch(batch.clone())
            .expect("one atomic batch replaces both latent reservations");
        assert!(ingress.dormant_local_fifo_reservations.is_empty());
        assert_eq!(ingress.len(), 4);
        let next_after_first_batch = ingress.next_admission_ordinal;

        ingress
            .enqueue_completion_batch(batch)
            .expect("repeated exact batch coalesces with physical owners");
        assert_eq!(ingress.len(), 4);
        assert_eq!(
            ingress.next_admission_ordinal, next_after_first_batch,
            "duplicate batch cannot allocate another physical range"
        );
    }

    #[test]
    fn dormant_local_fifo_metadata_rejects_wrong_stage_ordinal_and_capacity() {
        let owner_tag = tag(0);
        let lifecycle_key = Hash::new(b"immutable dormant completion lifecycle");
        let new_ingress = || {
            let mut ingress = BoundedIngress::with_lifecycle_ordinals(
                RuntimeQueueConfig::new(4, 1, 1),
                RuntimeLifecycleOrdinalSource::after_high_watermark(2),
            );
            ingress
                .install_dormant_local_fifo_reservations(vec![
                    RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8),
                ])
                .expect("install exact dormant metadata");
            ingress
        };

        let mut wrong_stage = new_ingress();
        assert_eq!(
            wrong_stage.enqueue(restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                lifecycle_key,
                1,
                9,
            )),
            Err(EnqueueError::FailClosed),
            "a retry cannot change its persisted reducer stage"
        );
        assert_eq!(wrong_stage.remaining_capacity(), 3);

        let mut wrong_ordinal = new_ingress();
        assert_eq!(
            wrong_ordinal.enqueue(restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                lifecycle_key,
                2,
                8,
            )),
            Err(EnqueueError::FailClosed),
            "a retry cannot change its immutable lifecycle ordinal"
        );
        assert_eq!(wrong_ordinal.remaining_capacity(), 3);

        let mut over_capacity = BoundedIngress::<FakeCommand>::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            RuntimeLifecycleOrdinalSource::after_high_watermark(5),
        );
        let forged = (1_u128..=5)
            .map(|ordinal| {
                RuntimeDormantLocalFifoReservation::completion(
                    Hash::new(ordinal.to_le_bytes()),
                    ordinal,
                    8,
                )
            })
            .collect();
        assert_eq!(
            over_capacity.install_dormant_local_fifo_reservations(forged),
            Err(EnqueueError::FailClosed),
            "an over-capacity snapshot must fail before live admission"
        );
        assert!(over_capacity.dormant_local_fifo_reservations.is_empty());

        for producer_stage in 0_u8..=u8::MAX {
            if RuntimeDormantLocalFifoReservation::is_local_fifo_stage(producer_stage) {
                continue;
            }
            let mut malformed = BoundedIngress::<FakeCommand>::with_lifecycle_ordinals(
                RuntimeQueueConfig::new(4, 1, 1),
                RuntimeLifecycleOrdinalSource::after_high_watermark(1),
            );
            assert_eq!(
                malformed.install_dormant_local_fifo_reservations(vec![
                    RuntimeDormantLocalFifoReservation::completion(
                        lifecycle_key,
                        1,
                        producer_stage,
                    ),
                ]),
                Err(EnqueueError::FailClosed),
                "nonlocal or unknown stage {producer_stage} cannot forge reserved FIFO capacity"
            );
            assert!(malformed.dormant_local_fifo_reservations.is_empty());
        }
    }

    #[test]
    fn restored_exact_stage_coalesces_at_full_capacity_without_aliasing_successors() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let lifecycle_key = Hash::new(b"persisted producer lifecycle");
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            RuntimeLifecycleOrdinalSource::after_high_watermark(1),
        );
        let restored_with_ordinal = |value, producer_stage, tag, class, lifecycle_ordinal| {
            let command = FakeCommand::record(value);
            let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
                tag,
                class,
                &command,
                None,
                lifecycle_key,
                lifecycle_ordinal,
            )
            .expect("validated dormant metadata reconstructs one exact owner");
            let mut tagged = TaggedCommand::with_causal_origin(
                tag,
                class,
                command,
                admitted_at,
                owner.causal_origin().clone(),
                owner.lifecycle_ordinal(),
            )
            .expect("restored command binds its persisted ordinal");
            tagged.restored_producer_stage = Some(producer_stage);
            tagged
        };
        let restored_with = |value, producer_stage, tag, class| {
            restored_with_ordinal(value, producer_stage, tag, class, 1)
        };
        let restored = |value, producer_stage| {
            restored_with(value, producer_stage, owner_tag, CommandClass::Completion)
        };
        ingress
            .install_dormant_local_fifo_reservations(vec![
                RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8),
                RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 9),
            ])
            .expect("restart installs both latent Local FIFO reservations");

        ingress
            .enqueue(restored(1, 8))
            .expect("first restored stage owns one physical position");
        ingress
            .enqueue(restored(2, 9))
            .expect("a distinct causal successor stage shares the lifecycle");
        for value in [3, 4] {
            ingress
                .enqueue(TaggedCommand::new(
                    owner_tag,
                    CommandClass::Completion,
                    FakeCommand::record(value),
                    admitted_at,
                ))
                .expect("fill the remaining physical capacity");
        }
        assert_eq!(ingress.remaining_capacity(), 0);
        let next_before_duplicate = ingress.next_admission_ordinal;

        ingress
            .enqueue(restored(1, 8))
            .expect("the exact restored retry coalesces at full capacity");
        assert_eq!(ingress.len(), 4);
        assert_eq!(
            ingress.next_admission_ordinal, next_before_duplicate,
            "coalescing cannot mint another physical admission ordinal"
        );
        assert_eq!(
            ingress.enqueue(restored_with_ordinal(
                1,
                8,
                owner_tag,
                CommandClass::Completion,
                2,
            )),
            Err(EnqueueError::FailClosed),
            "one restored lifecycle key cannot change its immutable ordinal at the same stage"
        );
        assert_eq!(
            ingress.enqueue(restored_with_ordinal(
                2,
                9,
                owner_tag,
                CommandClass::Completion,
                2,
            )),
            Err(EnqueueError::FailClosed),
            "a restored successor stage cannot change its lifecycle ordinal"
        );
        assert_eq!(
            ingress.enqueue(restored(9, 8)),
            Err(EnqueueError::FailClosed),
            "one persisted lifecycle stage cannot carry conflicting command identity"
        );
        assert_eq!(
            ingress.enqueue(restored_with(1, 8, owner_tag, CommandClass::Progress,)),
            Err(EnqueueError::FailClosed),
            "one persisted lifecycle stage cannot change its service class"
        );
        assert_eq!(
            ingress.enqueue(restored_with(
                1,
                8,
                EventTag::new(
                    owner_tag.height(),
                    owner_tag.view(),
                    Generation::new(owner_tag.generation().get() + 1),
                ),
                CommandClass::Completion,
            )),
            Err(EnqueueError::FailClosed),
            "one queued restart stage cannot change its exact reducer tag"
        );
        let mut changed_origin = restored(1, 8);
        changed_origin.causal_origin.root_ingress_identity =
            Some(Hash::new(b"foreign restored ingress origin"));
        changed_origin.causal_origin.projection_hash =
            runtime_candidate_causal_origin_projection_hash(&changed_origin.causal_origin);
        assert!(changed_origin.validate_admission_identity());
        assert_eq!(
            ingress.enqueue(changed_origin),
            Err(EnqueueError::FailClosed),
            "one persisted lifecycle stage cannot change causal-origin metadata"
        );
        assert_eq!(ingress.len(), 4);
    }

    #[test]
    fn restored_producer_preflight_cannot_change_completion_service_class() {
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        driver.admission_preflight_override =
            Some(RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key: Hash::new(b"persisted completion lifecycle"),
                admission_ordinal: 1,
                producer_stage: 5,
            });
        let started_at = Instant::now();
        let mut runtime = runtime(driver, started_at, RuntimeQueueConfig::new(4, 1, 1));

        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Progress,
                FakeCommand::record(1),
            ),
            Err(EnqueueError::FailClosed)
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.ingress.len(),
            0,
            "a caller-class mutation cannot acquire a priority position"
        );
    }

    #[test]
    fn causal_lifecycle_key_ignores_only_process_generation() {
        let first_tag = EventTag::new(9, 4, Generation::new(1));
        let replay_tag = EventTag::new(9, 4, Generation::new(7));
        let different_view = EventTag::new(9, 5, Generation::new(7));
        let command = FakeCommand::record(0xA5);

        let first =
            RuntimeCandidateCausalOrigin::mint(first_tag, CommandClass::Progress, &command, None);
        let replay =
            RuntimeCandidateCausalOrigin::mint(replay_tag, CommandClass::Progress, &command, None);
        let other_view = RuntimeCandidateCausalOrigin::mint(
            different_view,
            CommandClass::Progress,
            &command,
            None,
        );

        assert!(first.same_lifecycle(&replay));
        assert_eq!(first.lifecycle_key, replay.lifecycle_key);
        assert_ne!(
            first.projection_hash, replay.projection_hash,
            "the full diagnostic carrier still records process generation"
        );
        assert!(!first.same_lifecycle(&other_view));
        assert_ne!(first.lifecycle_key, other_view.lifecycle_key);
    }

    #[test]
    fn aggregate_certificate_causal_roots_ignore_signer_carrier_replacement() {
        let (context, keys) = authenticated_runtime_context();
        let owner_tag = tag(0);
        let source_a = PeerId::new(keys[0].public_key().clone());
        let source_b = PeerId::new(keys[1].public_key().clone());
        let tagged_origin = |message: wire::ConsensusMessageV2, source: PeerId| {
            let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
                &message,
                fair_runtime_ownership(&message, source.clone(), source),
            )
            .expect("fair ingress yields exact runtime ownership");
            let authenticated = AuthenticatedConsensusMessage::for_test(message);
            assert_eq!(
                authenticated.exact_runtime_command_identity(),
                AdapterCommand::Authenticated(authenticated.clone())
                    .exact_runtime_command_identity(),
                "the authenticated token and adapter wrapper share one exact identity"
            );
            TaggedCommand::with_ingress_ownership(
                owner_tag,
                CommandClass::Progress,
                authenticated,
                Instant::now(),
                ownership,
            )
            .causal_origin
        };

        let qc_a = signed_runtime_quorum_certificate(&context, &keys, 0xD1);
        let mut qc_b = qc_a.clone();
        qc_b.signers.rotate_left(1);
        qc_b.aggregate_signature = vec![0xB2; qc_b.aggregate_signature.len()];
        let qc_origin_a = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(qc_a)),
            source_a.clone(),
        );
        let qc_origin_b = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(qc_b)),
            source_b.clone(),
        );
        assert!(qc_origin_a.same_lifecycle(&qc_origin_b));

        let tc_a = signed_runtime_timeout_certificate(&context, &keys);
        let mut tc_b = tc_a.clone();
        tc_b.groups[0].signers.rotate_left(1);
        tc_b.groups[0].aggregate_signature = vec![0xC3; tc_b.groups[0].aggregate_signature.len()];
        let tc_message_a = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(tc_a.clone()),
        );
        let tc_message_b = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(tc_b),
        );
        let exact_tc_a = AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
            tc_message_a.clone(),
        ))
        .exact_runtime_command_identity()
        .digest();
        let exact_tc_b = AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
            tc_message_b.clone(),
        ))
        .exact_runtime_command_identity()
        .digest();
        assert_ne!(
            exact_tc_a, exact_tc_b,
            "deep command identity still distinguishes replaceable certificate carriers"
        );
        let tc_origin_a = tagged_origin(tc_message_a, source_a);
        let tc_origin_b = tagged_origin(tc_message_b, source_b.clone());
        assert!(tc_origin_a.same_lifecycle(&tc_origin_b));

        let mut other_round = tc_a;
        other_round.round.view = other_round.round.view.saturating_add(1);
        let other_round_origin = tagged_origin(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                other_round,
            )),
            source_b,
        );
        assert!(
            !tc_origin_a.same_lifecycle(&other_round_origin),
            "transition-relevant certified round cannot collide with carrier normalization"
        );
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
    fn unpublished_body_completion_reservation_fences_conflicting_proposals() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"reserved-body-context",
            ))),
            height: 8,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"reserved-body-block")),
            payload_hash: Hash::new(b"reserved-body-payload"),
        };
        let canonical = wire::PayloadManifest {
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
            chunk_hashes: vec![Hash::new(b"reserved canonical chunk")],
            chunk_root: Hash::new(b"reserved canonical root"),
        };
        let conflicting = wire::PayloadManifest {
            chunk_hashes: vec![Hash::new(b"reserved conflicting chunk")],
            chunk_root: Hash::new(b"reserved conflicting root"),
            ..canonical.clone()
        };
        let canonical_proposal = authenticated_proposal_for_test(canonical.clone());
        let conflicting_proposal = authenticated_proposal_for_test(conflicting);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(1, 0, 0));

        let reservation = ingress
            .reserve_canonical_body_available(tag(0), canonical)
            .expect("the unpublished completion atomically claims capacity and an ordinal");
        assert_eq!(ingress.len(), 0, "reservation is not reducer-visible");
        assert_eq!(ingress.remaining_capacity(), 0);
        assert_eq!(reservation.admission_ordinal, Some(1));
        assert!(
            ingress.conflicts_with_pending_body_available(&conflicting_proposal),
            "the unpublished canonical manifest must already fence a conflicting proposal"
        );
        assert!(
            !ingress.conflicts_with_pending_body_available(&canonical_proposal),
            "an exact proposal does not conflict with its reserved completion"
        );

        let mut mismatched = reservation.clone();
        mismatched.tag = tag(1);
        assert_eq!(
            ingress.commit_canonical_body_available(mismatched),
            Err(EnqueueError::FailClosed),
            "a stale or mismatched token must not silently lose the completion"
        );
        assert_eq!(ingress.len(), 0);
        assert_eq!(
            ingress.reserved_body_available.as_ref(),
            Some(&reservation),
            "a rejected token preserves the exact unpublished owner"
        );

        ingress
            .commit_canonical_body_available(reservation)
            .expect("the exact reservation token publishes its completion");
        let completion = ingress
            .commands
            .front()
            .expect("commit publishes the already-owned completion slot");
        assert_eq!(completion.admission_ordinal, Some(1));
        assert_eq!(completion.lifecycle_ordinal, Some(1));
        assert!(ingress.conflicts_with_pending_body_available(&conflicting_proposal));
    }

    #[test]
    fn aborted_body_completion_retry_reclaims_the_entire_token_without_reminting() {
        let directory = TempDir::new().expect("temporary body retry directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(3, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xB1);
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0xB3))
            .expect("ordinary ingress occupies its sole unreserved slot");
        runtime
            .enqueue_network(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(
                    signed_runtime_timeout_certificate(&context, &keys),
                ),
            ))
            .expect("certified progress occupies the progress slot");
        assert_eq!(runtime.remaining_completion_capacity(), 1);
        let reservation = runtime
            .reserve_body_available(owner_tag, manifest.clone())
            .expect("reserve one unpublished exact completion");
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        let source_after_reserve = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after body reservation");

        let mut mismatched_abort = reservation.clone();
        mismatched_abort.tag = tag(1);
        runtime.abort_body_available(mismatched_abort);
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&reservation),
            "a mismatched abort has no authority to clear the exact token",
        );
        runtime.abort_body_available(reservation.clone());
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&reservation),
            "abort retains the exact token instead of orphaning its ordinal",
        );
        let retry = runtime
            .reserve_body_available(owner_tag, manifest.clone())
            .expect("exact retry reclaims the unpublished token");
        assert_eq!(retry, reservation);
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect source after exact retry"),
            source_after_reserve,
            "exact retry cannot mint a second physical ordinal",
        );

        let competing_ordinal = runtime
            .ingress
            .lifecycle_ordinals
            .reserve_one()
            .expect("advance the shared source through another actor owner");
        let source_before_materialization = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect actual shared source before materialization");
        assert_eq!(Some(competing_ordinal), source_after_reserve);
        runtime
            .commit_body_available(retry)
            .expect("materialize the exact retained reservation");
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("materialization preserves the shared source"),
            source_before_materialization,
            "materialization observes but never advances the current source",
        );
    }

    #[test]
    fn conflicting_body_completion_retry_latches_without_replacing_the_exact_token() {
        let directory = TempDir::new().expect("temporary body conflict directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xB4);
        let reservation = runtime
            .reserve_body_available(owner_tag, manifest.clone())
            .expect("reserve one unpublished exact completion");
        let source_after_reserve = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after body reservation");
        let conflicting = wire::PayloadManifest {
            chunk_root: Hash::new(b"conflicting retained body root"),
            chunk_hashes: vec![Hash::new(b"conflicting retained body chunk")],
            ..manifest
        };

        assert_eq!(
            runtime.reserve_body_available(owner_tag, conflicting),
            Err(EnqueueError::DuplicateCompletionOwnership),
            "same logical slot with different evidence cannot replace the retained token",
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&reservation),
        );
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect source after rejected conflict"),
            source_after_reserve,
            "conflicting evidence cannot burn a fresh physical ordinal",
        );
    }

    #[test]
    fn dormant_body_reservation_aliases_full_capacity_across_abort_retry_and_commit() {
        let directory = TempDir::new().expect("temporary dormant body retry directory");
        let (_runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let owner_tag = tag(0);
        let manifest = runtime_manifest(&context, 0xB2);
        let lifecycle_key = Hash::new(b"dormant body completion lifecycle");
        let body_command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
            owner_tag,
            CommandClass::Completion,
            &body_command,
            None,
            lifecycle_key,
            1,
        )
        .expect("restore exact dormant body owner");
        let dormant = RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 8);
        let source = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(1, 0, 0),
            source.clone(),
        );
        ingress
            .install_dormant_local_fifo_reservations(vec![dormant])
            .expect("install the full-capacity dormant completion owner");
        assert_eq!(ingress.remaining_capacity(), 0);

        let reservation = ingress
            .reserve_canonical_body_available_internal(
                owner_tag,
                manifest.clone(),
                Some(&owner),
                Some(8),
            )
            .expect("unpublished token aliases the dormant capacity owner");
        assert_eq!(reservation.lifecycle_ordinal, Some(1));
        assert_eq!(reservation.admission_ordinal, Some(2));
        assert_eq!(reservation.dormant_replacement, Some(dormant));
        assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
        assert_eq!(ingress.remaining_capacity(), 0);
        let source_after_reserve = source
            .next_ordinal_for_test()
            .expect("inspect source after dormant reservation");

        ingress.abort_canonical_body_available(reservation.clone());
        let retry = ingress
            .reserve_canonical_body_available_internal(owner_tag, manifest, Some(&owner), Some(8))
            .expect("exact dormant retry reclaims the whole token");
        assert_eq!(retry, reservation);
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after dormant retry"),
            source_after_reserve,
        );
        assert_eq!(
            ingress.reserve_canonical_body_available_internal(
                owner_tag,
                retry.manifest().clone(),
                Some(&owner),
                Some(9),
            ),
            Err(EnqueueError::FailClosed),
            "retry cannot replace the exact dormant stage",
        );
        assert_eq!(ingress.reserved_body_available.as_ref(), Some(&reservation));

        let source_before_failed_commit = source
            .next_ordinal_for_test()
            .expect("inspect source before rejected dormant commit");
        let mut mismatched_commit = retry.clone();
        mismatched_commit.tag = tag(1);
        assert_eq!(
            ingress.commit_canonical_body_available(mismatched_commit),
            Err(EnqueueError::FailClosed),
        );
        assert_eq!(ingress.reserved_body_available.as_ref(), Some(&reservation));
        assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("rejected dormant commit preserves the source"),
            source_before_failed_commit,
        );

        ingress
            .commit_canonical_body_available(retry)
            .expect("materialization atomically replaces token and dormant backing");
        assert!(ingress.reserved_body_available.is_none());
        assert!(ingress.dormant_local_fifo_reservations.is_empty());
        assert_eq!(ingress.len(), 1);
        assert_eq!(ingress.commands[0].admission_ordinal, Some(2));
        assert_eq!(ingress.commands[0].lifecycle_ordinal, Some(1));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("materialization preserves the source"),
            source_after_reserve,
        );
    }

    #[test]
    fn mismatched_body_completion_commit_fails_closed_without_losing_reservation() {
        let directory = TempDir::new().expect("temporary body reservation directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0xA4);
        let reservation = runtime
            .reserve_body_available(owner_tag, manifest)
            .expect("reserve the exact unpublished completion");
        let exact = reservation.clone();
        let mut mismatched = reservation;
        mismatched.tag = tag(1);

        assert_eq!(
            runtime.commit_body_available(mismatched),
            Err(EnqueueError::FailClosed)
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&exact),
            "the invalid token cannot consume the exact reserved owner"
        );
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
        let admitted_origin = ingress.commands[0].causal_origin.clone();
        let admitted_lifecycle_ordinal = ingress.commands[0].lifecycle_ordinal;
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(1), CommandClass::Progress, authenticated())
                .expect("equal authenticated retransmission is coalesced"),
            tag(0),
            "a coalesced retransmission returns the original queue owner's tag"
        );
        assert_eq!(ingress.len(), 1);
        assert_eq!(ingress.commands[0].causal_origin, admitted_origin);
        assert_eq!(
            ingress.commands[0].lifecycle_ordinal, admitted_lifecycle_ordinal,
            "an exact transport retry retains the first lifecycle owner"
        );

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
        assert!(
            !ingress.commands[0]
                .causal_origin
                .same_lifecycle(&admitted_origin),
            "a later interval is not spliced into the drained causal root"
        );
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
    fn later_same_semantic_fair_retry_retains_runtime_lifecycle_root() {
        let directory = TempDir::new().expect("temporary lifecycle-retry runtime directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
        let message = signed_runtime_proposal(&context, &keys, 0xD1);
        let semantic_origin = PeerId::new(keys[0].public_key().clone());
        let authenticated_via = PeerId::new(keys[1].public_key().clone());
        let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
        let retained_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint first fair lifecycle");
        let retry_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint later fair retry lifecycle");
        let retained = fair_runtime_ownership_at_lifecycle(
            fair_runtime_ownership(&message, semantic_origin.clone(), authenticated_via.clone()),
            retained_ordinal,
        );
        let retry = fair_runtime_ownership_at_lifecycle(
            fair_runtime_ownership(&message, semantic_origin, authenticated_via),
            retry_ordinal,
        );

        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), retained)
            .expect("first fair lifecycle enters runtime");
        let physical_ordinal = runtime.ingress.commands[0]
            .admission_ordinal
            .expect("runtime admission owns one physical position");
        let next_before_retry = lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect shared source before coalescing retry");
        runtime
            .enqueue_network_with_ingress_ownership(message, retry)
            .expect("later same-semantic retry coalesces");

        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect shared source after coalescing retry"),
            next_before_retry,
            "runtime coalescence cannot mint a second physical FIFO position"
        );
        let queued = &runtime.ingress.commands[0];
        assert_eq!(queued.admission_ordinal, Some(physical_ordinal));
        assert_eq!(queued.lifecycle_ordinal, Some(retained_ordinal));
        assert_eq!(
            queued.causal_origin.root_lifecycle_ordinal,
            Some(retained_ordinal)
        );
        let ownership = queued
            .ingress_ownership
            .as_ref()
            .expect("coalesced command retains exact fair ownership");
        assert_eq!(
            ownership.earliest_lifecycle_ordinal(),
            Ok(Some(retained_ordinal))
        );
        let carrier = ownership
            .direct
            .first()
            .expect("same semantic retry remains one bounded carrier");
        assert_eq!(carrier.admission_count, 2);
        assert_eq!(carrier.first.lifecycle_ordinal, Some(retained_ordinal));
        assert_eq!(carrier.latest.lifecycle_ordinal, Some(retained_ordinal));
        assert!(ownership.validate_exact());
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it() {
        let directory = TempDir::new().expect("temporary fair-to-runtime predecessor directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
        let message = signed_runtime_proposal(&context, &keys, 0xD6);
        let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
        let fair_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint ordinary fair-ingress predecessor lifecycle");
        let ownership = fair_runtime_ownership_at_lifecycle(
            fair_runtime_ownership(
                &message,
                PeerId::new(keys[0].public_key().clone()),
                PeerId::new(keys[1].public_key().clone()),
            ),
            fair_ordinal,
        );
        runtime
            .enqueue_network_with_ingress_ownership(message, ownership)
            .expect("transfer ordinary fair predecessor into serialized runtime");
        let serve_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint exact Serve target behind the transferred predecessor");
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for exact predecessor comparison");
        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(now, serve_ordinal)
                .expect("transferred Fair owner participates in runtime minimum"),
            "the exact Serve target cannot prepare past the transferred predecessor"
        );

        let (_, consumed) = runtime
            .ingress
            .pop_next_with_ownership()
            .expect("runtime predecessor selection remains exact")
            .expect("ordinary Fair predecessor is ready");
        assert_eq!(consumed.lifecycle_ordinal, fair_ordinal);
        assert!(
            !runtime
                .older_lifecycle_predates_exact_serve(now, serve_ordinal)
                .expect("recompute minimum after consuming the predecessor"),
            "Serve becomes eligible only after the transferred lifecycle drains"
        );
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn older_frozen_aggregate_carrier_rebases_queued_runtime_minimum() {
        let directory = TempDir::new().expect("temporary aggregate-rebase runtime directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0xD2),
            ));
        let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
        let older_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint frozen older aggregate lifecycle");
        let newer_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint later independently admissible aggregate lifecycle");
        let newer = fair_runtime_ownership_at_lifecycle(
            fair_network_ownership(&message, PeerId::new(keys[2].public_key().clone())),
            newer_ordinal,
        );
        let older = fair_runtime_ownership_at_lifecycle(
            fair_network_ownership(&message, PeerId::new(keys[1].public_key().clone())),
            older_ordinal,
        );

        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), newer)
            .expect("newer admissible aggregate enters runtime first");
        assert_eq!(
            runtime.ingress.commands[0].lifecycle_ordinal,
            Some(newer_ordinal)
        );
        let physical_ordinal = runtime.ingress.commands[0].admission_ordinal;
        let next_before_older = lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect shared source before older carrier transfer");
        let mut unfrozen_older = older.clone();
        unfrozen_older.runtime_physical_cut = None;
        assert!(unfrozen_older.validate_exact());
        let unfrozen_projection =
            RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, unfrozen_older)
                .expect("pre-dequeue aggregate identity remains exact");
        assert!(!unfrozen_projection.validate_frozen_physical());
        let retained_projection = runtime.ingress.commands[0]
            .ingress_ownership
            .as_ref()
            .expect("newer aggregate retains checked ingress ownership");
        let mut mixed_preview = retained_projection.clone();
        mixed_preview
            .merge_downstream(unfrozen_projection)
            .expect("capacity probe can preview a frozen/unfrozen aggregate merge");
        assert!(mixed_preview.validate_exact());
        assert!(
            !mixed_preview.validate_frozen_physical(),
            "only checked dequeue may promote the preview to mutable runtime ownership"
        );
        runtime
            .enqueue_network_with_ingress_ownership(message, older)
            .expect("older frozen aggregate carrier joins the queued envelope");

        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect shared source after aggregate reconciliation"),
            next_before_older,
            "carrier reconciliation cannot mint another physical command"
        );
        let queued = &runtime.ingress.commands[0];
        assert_eq!(queued.admission_ordinal, physical_ordinal);
        assert_eq!(queued.lifecycle_ordinal, Some(older_ordinal));
        assert_eq!(
            queued.causal_origin.root_lifecycle_ordinal,
            Some(older_ordinal)
        );
        let ownership = queued
            .ingress_ownership
            .as_ref()
            .expect("aggregate command retains both fair carriers");
        assert_eq!(ownership.direct.len(), 2);
        assert_eq!(
            ownership.earliest_lifecycle_ordinal(),
            Ok(Some(older_ordinal))
        );
        assert!(ownership.validate_exact());

        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before exact Serve comparison");
        let serve_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint exact Serve barrier after both aggregate carriers");
        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(now, serve_ordinal)
                .expect("compare reconciled aggregate minimum"),
            "the later-transferred frozen carrier must become the active minimum"
        );
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals() {
        let unminted_directory = TempDir::new().expect("temporary unminted-fair runtime directory");
        let (mut unminted_runtime, context, keys) =
            authenticated_network_runtime(&unminted_directory, RuntimeQueueConfig::new(8, 2, 2));
        let source = unminted_runtime.ingress.lifecycle_ordinals.clone();
        let unminted_ordinal = source
            .next_ordinal_for_test()
            .expect("inspect unminted source position")
            .expect("fresh source has a first ordinal");
        let first_message = signed_runtime_proposal(&context, &keys, 0xD3);
        let first_ownership = fair_runtime_ownership_at_lifecycle(
            fair_runtime_ownership(
                &first_message,
                PeerId::new(keys[0].public_key().clone()),
                PeerId::new(keys[1].public_key().clone()),
            ),
            unminted_ordinal,
        );
        assert!(matches!(
            unminted_runtime.enqueue_network_with_ingress_ownership(first_message, first_ownership),
            Err(NetworkIngressError::FailClosed)
        ));
        assert!(unminted_runtime.fail_closed);
        assert_eq!(unminted_runtime.queued_commands(), 0);
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("unminted rejection preserves the source"),
            Some(unminted_ordinal)
        );

        let collision_directory =
            TempDir::new().expect("temporary fair-collision runtime directory");
        let (mut collision_runtime, context, keys) =
            authenticated_network_runtime(&collision_directory, RuntimeQueueConfig::new(8, 2, 2));
        let source = collision_runtime.ingress.lifecycle_ordinals.clone();
        let shared_ordinal = source.reserve_one().expect("mint one exact fair lifecycle");
        let admitted_message = signed_runtime_proposal(&context, &keys, 0xD4);
        let conflicting_message = signed_runtime_proposal(&context, &keys, 0xD5);
        let admitted_ownership = fair_runtime_ownership_at_lifecycle(
            fair_runtime_ownership(
                &admitted_message,
                PeerId::new(keys[0].public_key().clone()),
                PeerId::new(keys[1].public_key().clone()),
            ),
            shared_ordinal,
        );
        let conflicting_ownership = fair_runtime_ownership_at_lifecycle(
            fair_runtime_ownership(
                &conflicting_message,
                PeerId::new(keys[0].public_key().clone()),
                PeerId::new(keys[1].public_key().clone()),
            ),
            shared_ordinal,
        );
        collision_runtime
            .enqueue_network_with_ingress_ownership(admitted_message, admitted_ownership)
            .expect("first exact fair lifecycle enters runtime");
        let next_before_collision = source
            .next_ordinal_for_test()
            .expect("inspect source before unrelated collision");
        assert!(matches!(
            collision_runtime.enqueue_network_with_ingress_ownership(
                conflicting_message,
                conflicting_ownership,
            ),
            Err(NetworkIngressError::FailClosed)
        ));
        assert!(collision_runtime.fail_closed);
        assert_eq!(collision_runtime.queued_commands(), 1);
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("collision rejection preserves the physical source"),
            next_before_collision,
            "unrelated ordinal collision must fail before a FIFO position is minted"
        );
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
        let mut commands = runtime.ingress.commands.iter();
        let first = commands.next().expect("first semantic root is retained");
        let second = commands.next().expect("second semantic root is retained");
        assert!(
            !first.causal_origin.same_lifecycle(&second.causal_origin),
            "identical wire bytes from unrelated semantic origins cannot coalesce"
        );
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
    fn busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation() {
        let directory = TempDir::new().expect("temporary Busy-deferred rebase directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 2, 2),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before Busy-deferred aggregate ingress");
        let owner_tag = runtime.round_tag();
        let timeout = runtime
            .driver
            .timeout_elapsed(owner_tag)
            .expect("install a signer fence before aggregate dispatch");
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

        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0x79),
            ));
        let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
        let mutation_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint the oldest identity-mutation carrier");
        let older_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint the older delayed aggregate carrier");
        let newer_ordinal = lifecycle_ordinals
            .reserve_one()
            .expect("mint the newer aggregate carrier admitted first");
        let newer = fair_runtime_ownership_at_lifecycle(
            fair_network_ownership(&message, PeerId::new(keys[2].public_key().clone())),
            newer_ordinal,
        );
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), newer)
            .expect("newer aggregate carrier enters runtime before the frozen predecessor");
        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("Busy dispatch retains the exact queued owner");
        assert!(selected.validate_exact().is_ok());
        let deferred_ordinals = runtime
            .deferred_ingress_ownership
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let [deferred_ordinal] = deferred_ordinals.as_slice() else {
            panic!("aggregate dispatch must retain exactly one Busy-deferred owner")
        };
        let deferred_ordinal = *deferred_ordinal;
        assert_eq!(
            runtime.deferred_ingress_ownership[&deferred_ordinal].earliest_lifecycle_ordinal(),
            Ok(Some(newer_ordinal))
        );
        assert_eq!(
            runtime.deferred_lifecycle_ownership[&deferred_ordinal].lifecycle_ordinal(),
            newer_ordinal
        );
        let frozen_physical_cut =
            runtime.deferred_lifecycle_ownership[&deferred_ordinal].physical_cut;
        let frozen_source_physical_ordinal =
            runtime.deferred_lifecycle_ownership[&deferred_ordinal].source_physical_ordinal;
        let frozen_runtime_seal = runtime.deferred_lifecycle_ownership[&deferred_ordinal]
            .runtime_seal
            .clone();
        assert_ne!(frozen_physical_cut, 0);
        assert!(frozen_source_physical_ordinal.is_some());

        let older = fair_runtime_ownership_at_lifecycle(
            fair_network_ownership(&message, PeerId::new(keys[1].public_key().clone())),
            older_ordinal,
        );
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(message.clone(), older)
                .expect("older frozen carrier joins the exact Busy-deferred aggregate"),
            owner_tag
        );
        let merged = runtime
            .deferred_ingress_ownership
            .get(&deferred_ordinal)
            .expect("Busy-deferred aggregate retains the merged carrier set");
        assert_eq!(merged.direct.len(), 2);
        assert_eq!(merged.earliest_lifecycle_ordinal(), Ok(Some(older_ordinal)));
        assert!(merged.validate_exact());
        let rebased_owner = runtime
            .deferred_lifecycle_ownership
            .get(&deferred_ordinal)
            .expect("Busy-deferred aggregate retains its rebased lifecycle owner");
        assert_eq!(rebased_owner.lifecycle_ordinal(), older_ordinal);
        assert_eq!(
            rebased_owner.physical_cut, frozen_physical_cut,
            "logical owner replacement cannot refresh the continuation's physical cut"
        );
        assert_eq!(
            rebased_owner.source_physical_ordinal, frozen_source_physical_ordinal,
            "logical owner replacement cannot replace the source occurrence"
        );
        assert_eq!(
            rebased_owner.runtime_seal, frozen_runtime_seal,
            "logical owner replacement cannot replace the admitted occurrence capability"
        );
        assert_eq!(
            rebased_owner.causal_origin().root_lifecycle_ordinal,
            Some(older_ordinal)
        );
        assert_eq!(
            rebased_owner.causal_origin().root_ingress_identity,
            Some(runtime_ingress_causal_origin_projection_hash(merged))
        );
        assert!(rebased_owner.validate_exact());

        let healthy_owner = rebased_owner.clone();
        let mutation = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &message,
            fair_runtime_ownership_at_lifecycle(
                fair_network_ownership(&message, PeerId::new(keys[0].public_key().clone())),
                mutation_ordinal,
            ),
        )
        .expect("oldest aggregate carrier has exact runtime ownership");
        assert_eq!(
            mutation.earliest_lifecycle_ordinal(),
            Ok(Some(mutation_ordinal))
        );
        let mut identity_mutated_lifecycle_owner = healthy_owner.owner.clone();
        identity_mutated_lifecycle_owner
            .causal_origin
            .root_ingress_identity = Some(Hash::new(b"mutated Busy-deferred ingress identity"));
        identity_mutated_lifecycle_owner.causal_origin.lifecycle_key =
            runtime_candidate_causal_origin_lifecycle_key(
                &identity_mutated_lifecycle_owner.causal_origin,
            );
        identity_mutated_lifecycle_owner
            .causal_origin
            .projection_hash = runtime_candidate_causal_origin_projection_hash(
            &identity_mutated_lifecycle_owner.causal_origin,
        );
        identity_mutated_lifecycle_owner.projection_hash =
            runtime_lifecycle_owner_projection_hash(&identity_mutated_lifecycle_owner);
        assert!(
            matches!(
                RuntimeDeferredLifecycleOwnership::new(
                    identity_mutated_lifecycle_owner,
                    healthy_owner.deferred_admission_ordinal,
                    healthy_owner.current_ingress,
                    healthy_owner.source_physical_ordinal,
                    healthy_owner.physical_cut,
                    healthy_owner.runtime_seal.clone(),
                ),
                Err(EnqueueError::FailClosed)
            ),
            "the adapter-private seal rejects a coherently rehashed causal identity substitution"
        );
        runtime
            .reconcile_deferred_ingress_ownership(Some((deferred_ordinal, mutation)))
            .expect("the same earlier carrier rebases after restoring the exact identity");
        let final_ingress = &runtime.deferred_ingress_ownership[&deferred_ordinal];
        assert_eq!(final_ingress.direct.len(), 3);
        assert_eq!(
            final_ingress.earliest_lifecycle_ordinal(),
            Ok(Some(mutation_ordinal))
        );
        let final_owner = &runtime.deferred_lifecycle_ownership[&deferred_ordinal];
        assert_eq!(final_owner.lifecycle_ordinal(), mutation_ordinal);
        assert_eq!(final_owner.physical_cut, frozen_physical_cut);
        assert_eq!(
            final_owner.source_physical_ordinal,
            frozen_source_physical_ordinal
        );
        assert_eq!(
            final_owner.runtime_seal, frozen_runtime_seal,
            "repeated aggregate rebasing retains the first admitted occurrence capability"
        );
        assert_eq!(
            final_owner.causal_origin().root_lifecycle_ordinal,
            Some(mutation_ordinal)
        );
        assert_eq!(
            final_owner.causal_origin().root_ingress_identity,
            Some(runtime_ingress_causal_origin_projection_hash(final_ingress))
        );
        assert!(final_owner.validate_exact());
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn distinct_pre_runtime_leader_wire_qc_waits_behind_busy_deferred_owner() {
        let directory = TempDir::new().expect("temporary pre-runtime leader-wire directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 2, 2),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before Busy-deferred aggregate ingress");
        let owner_tag = runtime.round_tag();
        let timeout = runtime
            .driver
            .timeout_elapsed(owner_tag)
            .expect("install a signer fence before aggregate dispatch");
        assert!(matches!(
            timeout.effects(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));

        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0x7A),
            ));
        let first_source = context.roster[2].validator.clone();
        let second_source = context.roster[1].validator.clone();
        let (_leader_wire_directory, leader_wire_ingress, ownerships) =
            preowned_leader_wire_ownerships(
                &context,
                &[(message.clone(), first_source)],
                runtime.ingress.lifecycle_ordinals.clone(),
            );
        let [first_ownership]: [FairV2IngressOwnershipEvidence; 1] = ownerships
            .try_into()
            .expect("fixture creates one exact runtime-owned carrier");
        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), first_ownership)
            .expect("first leader-wire carrier enters the runtime");
        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        runtime
            .take_last_scheduler_ownership()
            .expect("Busy dispatch retains the first exact carrier");
        assert_eq!(runtime.deferred_ingress_ownership.len(), 1);

        assert!(matches!(
            leader_wire_ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(message.clone()),
                Some(second_source),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ));
        let selected = leader_wire_ingress.try_recv_if(|inbound| {
            let BlockMessage::V2(candidate) = inbound.message() else {
                return true;
            };
            let ownership = inbound
                .ingress_ownership()
                .expect("productive fair ingress attaches exact ownership");
            runtime.can_admit_network_message_with_ingress_ownership(candidate, ownership)
        });
        assert!(
            selected.is_none(),
            "a distinct productive leader-wire token must remain physically queued behind the Busy owner"
        );
        assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
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
            "direct carrier projections must retain their distinct authenticated-source identities"
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
    fn exact_authenticated_tc_from_distinct_sources_retains_one_busy_owner() {
        let directory = TempDir::new().expect("temporary multi-source TC directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(4, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before authenticated ingress");
        let owner_tag = runtime.round_tag();
        let timeout_effects = runtime
            .driver
            .timeout_elapsed(owner_tag)
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
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                signed_runtime_timeout_certificate(&context, &keys),
            ));

        for source in &keys[..2] {
            assert_eq!(
                runtime
                    .enqueue_network_with_ingress_ownership(
                        message.clone(),
                        fair_network_ownership(&message, PeerId::new(source.public_key().clone()),),
                    )
                    .expect("each authenticated TC carrier coalesces"),
                owner_tag
            );
        }
        assert_eq!(runtime.queued_commands(), 1);
        let queued = runtime
            .ingress
            .commands
            .front()
            .and_then(|command| command.ingress_ownership.as_ref())
            .expect("the queued TC retains both fair-ingress carriers");
        assert_eq!(queued.direct.len(), 2);
        assert!(queued.validate_exact());

        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let fifo_owner = runtime
            .take_last_scheduler_ownership()
            .expect("Busy TC dispatch retains its exact FIFO owner");
        assert!(fifo_owner.validate_exact().is_ok());
        assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
        let deferred = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy TC owns one deferred ordinal");
        assert_eq!(deferred.direct.len(), 2);
        assert!(deferred.validate_exact());

        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, PeerId::new(keys[2].public_key().clone()),),
                )
                .expect("a later authenticated carrier merges into the Busy TC"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .deferred_ingress_ownership
                .values()
                .next()
                .expect("the Busy TC retains its merged carrier set")
                .direct
                .len(),
            3
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
        assert!(matches!(runtime.step(now), Ok(RuntimeStep::Advanced(_))));
        let deferred_owner = runtime
            .take_last_scheduler_ownership()
            .expect("deferred TC service hands off its exact owner");
        assert!(deferred_owner.validate_exact().is_ok());
        let RuntimeSelectedCandidateOwnership::ExactDeferred(deferred) = &deferred_owner.candidate
        else {
            panic!("expected exact deferred TC scheduler ownership")
        };
        assert!(
            deferred
                .ingress_ownership
                .as_ref()
                .is_some_and(|ownership| ownership.direct.len() == 3)
        );
        assert!(runtime.deferred_ingress_ownership.is_empty());
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

        assert!(matches!(
            super::super::InboundBlockMessage::try_from_transport_with_reply_route(
                super::super::message::BlockMessage::V2(message.clone()),
                source.clone(),
                source.clone(),
                conflicting_route.clone(),
            ),
            Err(NetworkReplyRouteError::EqualOrdinalDifferentTenure)
        ));
        let first_ownership = fair_network_ownership_with_route(
            &message,
            source.clone(),
            source.clone(),
            first_route,
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

        let mut conflicting_ownership = retained_before.direct[0].clone();
        conflicting_ownership.attempts[0].route = conflicting_route.clone();
        conflicting_ownership.latest.attempts_after[0].route = conflicting_route;
        assert!(
            !conflicting_ownership.validate_exact(),
            "the runtime must reject a carrier whose cursor projection substitutes a forged tenure"
        );
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
        let queued_before = runtime.queued_commands();
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
        assert_eq!(
            runtime.queued_commands(),
            queued_before,
            "carrier saturation must not create a duplicate runtime command"
        );
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
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: ingress_manifest.round,
                subject: ingress_manifest.subject,
                receipt: durable.clone(),
            },
        );
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
        let active_before_store = runtime.driver.all_deferred_admission_ordinals();
        runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                owner_tag,
                &deferred_store,
                DeferredBodyPipelineStageForTest::BodyStored,
            )
            .expect("stage a Busy-deferred durable-store completion");
        let store_ordinals = runtime
            .driver
            .all_deferred_admission_ordinals()
            .difference(&active_before_store)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(store_ordinals.len(), 1);
        let store_owner = bind_local_deferred_lifecycle_for_test(
            &mut runtime,
            store_ordinals[0],
            b"body-store-pipeline-retirement-owner",
        );
        runtime
            .enqueue_body_stored(
                owner_tag,
                deferred_store.round,
                deferred_store.subject,
                durable,
            )
            .expect("a retransmit coalesces with the Busy-deferred store owner");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal()
                .expect("inspect the exact Busy-deferred store owner"),
            Some(store_owner.lifecycle_ordinal())
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    deferred_store.round,
                    deferred_store.subject,
                )
                .expect("retire the coalesced Busy-deferred store owner"),
            RetiredBodyPipelineCompletions {
                body_available: 0,
                body_stored: 1,
                validation: 0,
                local_proposal: 0,
            }
        );
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
        assert!(runtime.deferred_ingress_ownership.is_empty());
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal()
                .expect("retirement cannot retain a phantom store owner"),
            None
        );

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
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                deferred_validation.round,
                deferred_validation.subject,
            )
            .expect("retire the coalesced Busy-deferred validation owner");

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
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                deferred_proposal.round,
                deferred_proposal.subject,
            )
            .expect("retire the coalesced Busy-deferred proposal owner");
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
        let active_before_body = runtime.driver.all_deferred_admission_ordinals();
        assert!(
            runtime
                .driver
                .body_available(source_tag, manifest.clone())
                .expect("stage exact completion behind the signer fence")
                .into_effects()
                .is_empty()
        );
        let body_ordinals = runtime
            .driver
            .all_deferred_admission_ordinals()
            .difference(&active_before_body)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(body_ordinals.len(), 1);
        let body_ordinal = body_ordinals[0];
        let body_owner = bind_local_deferred_lifecycle_for_test(
            &mut runtime,
            body_ordinal,
            b"body-available-retirement-owner",
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
        stage_completion_for_queue_test(
            &mut runtime,
            source_tag,
            AdapterCommand::BodyAvailable {
                manifest: manifest.clone(),
            },
        );
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
        assert_eq!(
            runtime
                .deferred_lifecycle_ownership
                .get(&body_ordinal)
                .map(RuntimeDeferredLifecycleOwnership::owner),
            Some(&body_owner),
            "coalescing cannot retire the wrapper of the retained Busy owner"
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
                .deferred_lifecycle_ownership
                .contains_key(&body_ordinal)
        );
        assert!(
            runtime
                .retire_body_available(same_view_rebound, &manifest)
                .expect("the unique destination owner remains retireable")
        );
        assert!(
            !runtime
                .deferred_lifecycle_ownership
                .contains_key(&body_ordinal),
            "retirement cannot leave the drained Busy owner at the global minimum"
        );
        assert!(runtime.deferred_ingress_ownership.is_empty());

        // Exercise the opposite coalescing direction: a Busy source loses to
        // an already-installed FIFO destination. The adapter occurrence and
        // its sealed runtime wrapper must retire in the same transition.
        let retirement_directory =
            TempDir::new().expect("temporary Busy-source coalescing directory");
        let (mut retirement_runtime, retirement_context, _keys) =
            authenticated_network_runtime(&retirement_directory, RuntimeQueueConfig::new(8, 1, 1));
        let retirement_source = retirement_runtime.round_tag();
        let retirement_manifest = runtime_manifest(&retirement_context, 0x8F);
        retirement_runtime
            .driver
            .defer_body_pipeline_stage_for_test(
                retirement_source,
                &retirement_manifest,
                DeferredBodyPipelineStageForTest::BodyAvailable,
            )
            .expect("stage the exact Busy source completion");
        let retirement_ordinals = retirement_runtime
            .driver
            .all_deferred_admission_ordinals()
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(retirement_ordinals.len(), 1);
        let retirement_ordinal = retirement_ordinals[0];
        bind_local_deferred_lifecycle_for_test(
            &mut retirement_runtime,
            retirement_ordinal,
            b"body-available-rebind-retirement-owner",
        );
        let retirement_rebound = EventTag::new(
            retirement_source.height(),
            retirement_source.view() + 1,
            Generation::new(retirement_source.generation().get() + 1),
        );
        observe_enter_view_for_test(
            &mut retirement_runtime,
            retirement_source,
            retirement_rebound,
            &retirement_manifest,
        );
        stage_completion_for_queue_test(
            &mut retirement_runtime,
            retirement_rebound,
            AdapterCommand::BodyAvailable {
                manifest: retirement_manifest.clone(),
            },
        );
        assert!(
            retirement_runtime
                .rebind_body_available(retirement_source, retirement_rebound, &retirement_manifest,)
                .expect("the existing FIFO destination coalesces the Busy source")
        );
        assert!(
            !retirement_runtime
                .deferred_lifecycle_ownership
                .contains_key(&retirement_ordinal),
            "Busy-source coalescing cannot leave its runtime wrapper alive"
        );
        assert!(
            !retirement_runtime
                .driver
                .all_deferred_admission_ordinals()
                .contains(&retirement_ordinal)
        );
        assert_eq!(retirement_runtime.queued_commands(), 1);
        assert!(
            retirement_runtime
                .retire_body_available(retirement_rebound, &retirement_manifest)
                .expect("the retained FIFO destination remains uniquely retireable")
        );
        assert_eq!(retirement_runtime.queued_commands(), 0);
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
        stage_completion_for_queue_test(
            &mut stored_runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: manifest.round,
                subject: manifest.subject,
                receipt: exact_receipt,
            },
        );
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
        stage_completion_for_queue_test(
            &mut validation_runtime,
            owner_tag,
            AdapterCommand::ValidationSucceeded {
                round: manifest.round,
                subject: manifest.subject,
                receipt: ValidatedBodyReceipt::for_test(durable),
            },
        );
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
            .map(|manifest| (owner_tag, manifest.round, manifest.subject))
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
        stage_completion_for_queue_test(
            &mut conflict_runtime,
            conflict_tag,
            AdapterCommand::ValidationSucceeded {
                round: conflicting.round,
                subject: conflicting.subject,
                receipt: ValidatedBodyReceipt::for_test(durable),
            },
        );
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
        stage_completion_for_queue_test(
            &mut validation_runtime,
            owner_tag,
            AdapterCommand::ValidationSucceeded {
                round: manifest.round,
                subject: manifest.subject,
                receipt: exact_validated,
            },
        );
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
        stage_completion_for_queue_test(
            &mut proposal_runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable,
                validated_receipt: validated,
            },
        );

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
    fn applied_body_pipeline_phases_suppress_retries_before_ordinal_allocation() {
        const PHASE_INVENTORY: [&str; 4] = [
            "body_available",
            "body_stored",
            "validation_succeeded",
            "signature_completed",
        ];

        let directory = TempDir::new().expect("temporary production phase-inventory directory");
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
        let mut suppressed_phases = Vec::new();

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
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("an applied BodyAvailable retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("body_available");

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
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("an applied BodyStored retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("body_stored");

        let validated = ValidatedBodyReceipt::for_test(durable);
        runtime
            .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated.clone())
            .expect("enqueue validation completion");
        let (signature_tag, signature_preimage) = match runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch validation completion")
        {
            RuntimeStep::Advanced(effects) => match effects.as_slice() {
                [
                    AdapterEffect::Sign {
                        tag,
                        request: SignRequest::Vote(vote),
                    },
                ] => (*tag, vote.signature_preimage()),
                effects => panic!("unexpected validation effects: {effects:?}"),
            },
            RuntimeStep::Idle => panic!("validation completion unexpectedly idle"),
        };
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_validation_succeeded(tag, manifest.round, manifest.subject, validated.clone())
            .expect("an applied validation retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("validation_succeeded");

        let signature = Signature::new(keys[0].private_key(), &signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(signature_tag, signature.clone())
            .expect("enqueue exact signature completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch exact signature completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
        ));
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_signature(signature_tag, signature)
            .expect("an applied signature retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("signature_completed");

        assert_eq!(
            runtime
                .retire_body_pipeline_completions(tag, manifest.round, manifest.subject)
                .expect("no applied callback remains physically owned"),
            RetiredBodyPipelineCompletions::default()
        );
        assert_eq!(suppressed_phases, PHASE_INVENTORY);
    }

    #[test]
    fn applied_validation_failure_suppresses_retry_and_rejects_opposite_outcome() {
        const PHASE_INVENTORY: [&str; 1] = ["validation_failed"];

        let directory = TempDir::new().expect("temporary failed-validation phase directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for failed-validation dispatch");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0x9B))
            .expect("enqueue authenticated proposal");
        let (tag, manifest) = match runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch proposal")
        {
            RuntimeStep::Advanced(effects) => match effects.as_slice() {
                [
                    AdapterEffect::FetchBody {
                        tag,
                        manifest: Some(manifest),
                        ..
                    },
                ] => (*tag, manifest.clone()),
                effects => panic!("unexpected proposal effects: {effects:?}"),
            },
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
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

        runtime
            .enqueue_validation_failed(tag, manifest.round, manifest.subject)
            .expect("enqueue deterministic validation failure");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch deterministic validation failure"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_validation_failed(tag, manifest.round, manifest.subject)
            .expect("an applied failed-validation retry is a monotone stutter");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        assert_eq!(["validation_failed"], PHASE_INVENTORY);

        assert_eq!(
            runtime.enqueue_validation_succeeded(
                tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            ),
            Err(EnqueueError::FailClosed),
            "opposite deterministic outcomes for one durable body conflict"
        );
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        assert!(runtime.fail_closed);
    }

    #[test]
    fn applied_local_proposal_handoff_suppresses_retry_before_ordinal_allocation() {
        const PHASE_INVENTORY: [&str; 1] = ["local_proposal_ready"];

        let directory = TempDir::new().expect("temporary local-proposal phase directory");
        let (fixture_context, _) = authenticated_runtime_context();
        let leader = fixture_context.leader(0);
        let (mut runtime, context, _keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(leader),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime for local proposal dispatch");
        let tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x9C);
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        runtime
            .enqueue_local_proposal(tag, manifest.clone(), durable.clone(), validated.clone())
            .expect("enqueue exact local proposal completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("persist the exact proposal intent"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::Sign { .. }])
        ));

        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_local_proposal(tag, manifest, durable, validated)
            .expect("the durable proposal intent suppresses its exact callback retry");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        assert_eq!(["local_proposal_ready"], PHASE_INVENTORY);
    }

    #[test]
    fn drained_internal_ignore_uses_exact_durable_tombstone_before_readmission() {
        const PHASE_INVENTORY: [&str; 2] = ["terminal_ignore", "restart_tombstone"];

        let directory = TempDir::new().expect("temporary runtime tombstone directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x9D);
        let ordinal_before_first = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("the first ownerless completion reaches its terminal reducer discard");
        assert_eq!(runtime.queued_commands(), 1);
        assert_ne!(runtime.ingress.next_admission_ordinal, ordinal_before_first);
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(Instant::now())
                .expect("drain the first ownerless completion"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));

        let next_ordinal = runtime.ingress.next_admission_ordinal;
        for _ in 0..3 {
            runtime
                .enqueue_body_available(tag, manifest.clone())
                .expect("the exact terminal lifecycle coalesces in-process");
        }
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        let mut suppressed_phases = vec!["terminal_ignore"];
        drop(runtime);

        let (mut restarted, restarted_context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        assert_eq!(restarted_context.id(), context.id());
        let restarted_tag = restarted.round_tag();
        let next_ordinal = restarted.ingress.next_admission_ordinal;
        for _ in 0..3 {
            restarted
                .enqueue_body_available(restarted_tag, manifest.clone())
                .expect("the exact terminal lifecycle coalesces after restart");
        }
        assert_eq!(restarted.queued_commands(), 0);
        assert_eq!(restarted.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("restart_tombstone");
        assert_eq!(suppressed_phases, PHASE_INVENTORY);
    }

    #[test]
    fn stale_internal_callback_is_marker_free_and_malformed_callback_spends_no_ordinal() {
        let stale_directory = TempDir::new().expect("temporary stale internal-callback directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
        let current = runtime.round_tag();
        let stale = EventTag::new(
            current.height(),
            current.view(),
            Generation::new(current.generation().get().saturating_sub(1)),
        );
        let manifest = runtime_manifest(&context, 0x9E);
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_body_available(stale, manifest.clone())
            .expect("valid stale internal callback is discarded before admission");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        drop(runtime);

        let (mut restarted, restarted_context, _keys) =
            authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
        assert_eq!(restarted_context.id(), context.id());
        let next_ordinal = restarted.ingress.next_admission_ordinal;
        restarted
            .enqueue_body_available(restarted.round_tag(), manifest)
            .expect("stale discard did not create a current-incarnation tombstone");
        assert_eq!(restarted.queued_commands(), 1);
        assert_ne!(restarted.ingress.next_admission_ordinal, next_ordinal);

        let malformed_directory =
            TempDir::new().expect("temporary malformed internal-callback directory");
        let (mut malformed_runtime, malformed_context, _keys) =
            authenticated_network_runtime(&malformed_directory, RuntimeQueueConfig::new(8, 1, 1));
        let mut malformed_manifest = runtime_manifest(&malformed_context, 0x9F);
        let mut foreign_context = malformed_context.clone();
        foreign_context.chain_id = "foreign-runtime-preflight".into();
        malformed_manifest.round.context_id = foreign_context.id();
        let next_ordinal = malformed_runtime.ingress.next_admission_ordinal;
        assert_eq!(
            malformed_runtime
                .enqueue_body_available(malformed_runtime.round_tag(), malformed_manifest),
            Err(EnqueueError::FailClosed)
        );
        assert_eq!(malformed_runtime.queued_commands(), 0);
        assert_eq!(
            malformed_runtime.ingress.next_admission_ordinal,
            next_ordinal
        );
        assert!(malformed_runtime.fail_closed);
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
        let body_available_only = RetiredBodyPipelineCompletions {
            body_available: 1,
            body_stored: 0,
            validation: 0,
            local_proposal: 0,
        };

        let dormant_manifest = runtime_manifest(&context, 0xA0);
        let dormant_lifecycle_key = Hash::new(b"bulk-retired dormant body lifecycle");
        let dormant_lifecycle_ordinal = runtime
            .ingress
            .lifecycle_ordinals
            .reserve_one()
            .expect("mint the restart-restored body lifecycle");
        let dormant_command = AdapterCommand::BodyAvailable {
            manifest: dormant_manifest.clone(),
        };
        let dormant_owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
            owner_tag,
            CommandClass::Completion,
            &dormant_command,
            None,
            dormant_lifecycle_key,
            dormant_lifecycle_ordinal,
        )
        .expect("restore the exact dormant body owner");
        let dormant = RuntimeDormantLocalFifoReservation::completion(
            dormant_lifecycle_key,
            dormant_lifecycle_ordinal,
            8,
        );
        let capacity_before_dormant = runtime.remaining_completion_capacity();
        runtime
            .ingress
            .install_dormant_local_fifo_reservations(vec![dormant])
            .expect("install one dormant body-pipeline slot");
        let dormant_reservation = runtime
            .ingress
            .reserve_canonical_body_available_internal(
                owner_tag,
                dormant_manifest.clone(),
                Some(&dormant_owner),
                Some(8),
            )
            .expect("reserve an unpublished token backed by the dormant slot");
        assert_eq!(
            runtime.remaining_completion_capacity(),
            capacity_before_dormant - 1,
            "the token aliases rather than duplicates its dormant capacity charge",
        );
        assert_eq!(
            runtime.ingress.body_pipeline_completion_counts(
                owner_tag,
                dormant_manifest.round,
                dormant_manifest.subject,
            ),
            body_available_only,
            "the unpublished reservation is exactly one BodyAvailable owner",
        );
        let dormant_mismatch = runtime_manifest(&context, 0xAF);
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    dormant_mismatch.round,
                    dormant_mismatch.subject,
                )
                .expect("mismatched bulk retirement is an atomic no-op"),
            RetiredBodyPipelineCompletions::default(),
        );
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&dormant_reservation),
        );
        assert!(
            runtime
                .ingress
                .dormant_local_fifo_reservations
                .contains(&dormant)
        );
        assert_eq!(
            runtime.remaining_completion_capacity(),
            capacity_before_dormant - 1,
            "mismatched bulk retirement preserves the aliased capacity charge",
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    dormant_manifest.round,
                    dormant_manifest.subject,
                )
                .expect("retire the unpublished dormant-backed body token"),
            body_available_only,
        );
        assert!(runtime.ingress.reserved_body_available.is_none());
        assert!(
            !runtime
                .ingress
                .dormant_local_fifo_reservations
                .contains(&dormant)
        );
        assert_eq!(
            runtime.remaining_completion_capacity(),
            capacity_before_dormant,
            "bulk retirement releases the token and its one aliased capacity owner",
        );
        assert_eq!(
            runtime
                .retire_body_pipeline_completions(
                    owner_tag,
                    dormant_manifest.round,
                    dormant_manifest.subject,
                )
                .expect("a repeated exact retirement cannot recreate the drained stage"),
            RetiredBodyPipelineCompletions::default(),
        );
        assert_eq!(
            runtime.remaining_completion_capacity(),
            capacity_before_dormant,
            "repeated retirement cannot reacquire or release capacity",
        );

        let ingress_manifest = runtime_manifest(&context, 0xA1);
        let (durable, validated) = receipts(&ingress_manifest);
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: ingress_manifest.round,
                subject: ingress_manifest.subject,
                receipt: durable.clone(),
            },
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::ValidationSucceeded {
                round: ingress_manifest.round,
                subject: ingress_manifest.subject,
                receipt: validated.clone(),
            },
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: ingress_manifest.clone(),
                durable_receipt: durable,
                validated_receipt: validated,
            },
        );
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
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: duplicate_body_stored.round,
                subject: duplicate_body_stored.subject,
                receipt: durable,
            },
        );
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

        let duplicate_directory =
            TempDir::new().expect("temporary duplicate dormant-body retirement directory");
        let (mut duplicate_runtime, duplicate_context, _keys) =
            authenticated_network_runtime(&duplicate_directory, RuntimeQueueConfig::new(4, 1, 1));
        let duplicate_tag = duplicate_runtime.round_tag();
        let duplicate_manifest = runtime_manifest(&duplicate_context, 0xD1);
        let duplicate_lifecycle_key = Hash::new(b"duplicate bulk-retired dormant body lifecycle");
        let duplicate_lifecycle_ordinal = duplicate_runtime
            .ingress
            .lifecycle_ordinals
            .reserve_one()
            .expect("mint the duplicate fixture's dormant lifecycle");
        let duplicate_command = AdapterCommand::BodyAvailable {
            manifest: duplicate_manifest.clone(),
        };
        let duplicate_owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
            duplicate_tag,
            CommandClass::Completion,
            &duplicate_command,
            None,
            duplicate_lifecycle_key,
            duplicate_lifecycle_ordinal,
        )
        .expect("restore the duplicate fixture's dormant body owner");
        let duplicate_dormant = RuntimeDormantLocalFifoReservation::completion(
            duplicate_lifecycle_key,
            duplicate_lifecycle_ordinal,
            8,
        );
        duplicate_runtime
            .ingress
            .install_dormant_local_fifo_reservations(vec![duplicate_dormant])
            .expect("install duplicate fixture dormant ownership");
        let duplicate_reservation = duplicate_runtime
            .ingress
            .reserve_canonical_body_available_internal(
                duplicate_tag,
                duplicate_manifest.clone(),
                Some(&duplicate_owner),
                Some(8),
            )
            .expect("reserve duplicate fixture unpublished ownership");
        stage_completion_for_queue_test(&mut duplicate_runtime, duplicate_tag, duplicate_command);
        let duplicate_capacity_before_rejection = duplicate_runtime.remaining_completion_capacity();
        assert_eq!(
            duplicate_runtime.ingress.body_pipeline_completion_counts(
                duplicate_tag,
                duplicate_manifest.round,
                duplicate_manifest.subject,
            ),
            RetiredBodyPipelineCompletions {
                body_available: 2,
                body_stored: 0,
                validation: 0,
                local_proposal: 0,
            },
        );
        let duplicate_mismatch = runtime_manifest(&duplicate_context, 0xDF);
        assert_eq!(
            duplicate_runtime
                .retire_body_pipeline_completions(
                    duplicate_tag,
                    duplicate_mismatch.round,
                    duplicate_mismatch.subject,
                )
                .expect("mismatched duplicate retirement is an atomic no-op"),
            RetiredBodyPipelineCompletions::default(),
        );
        assert_eq!(
            duplicate_runtime
                .retire_body_pipeline_completions(
                    duplicate_tag,
                    duplicate_manifest.round,
                    duplicate_manifest.subject,
                )
                .expect_err("duplicate unpublished and queued owners must fail closed"),
            "Sumeragi v2 body pipeline has duplicate exact serialized completion stages",
        );
        assert!(duplicate_runtime.fail_closed);
        assert_eq!(
            duplicate_runtime.ingress.reserved_body_available.as_ref(),
            Some(&duplicate_reservation),
            "duplicate preflight cannot consume the unpublished token",
        );
        assert!(
            duplicate_runtime
                .ingress
                .dormant_local_fifo_reservations
                .contains(&duplicate_dormant)
        );
        assert_eq!(duplicate_runtime.queued_commands(), 1);
        assert_eq!(
            duplicate_runtime.remaining_completion_capacity(),
            duplicate_capacity_before_rejection,
            "duplicate preflight must preserve the complete capacity charge",
        );
    }

    #[test]
    fn pre_dequeue_probe_validates_unfrozen_leader_wire_identity() {
        let directory = TempDir::new().expect("temporary pre-dequeue probe directory");
        let (runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(3, 1, 1));
        let fixture = leader_wire_proposal_fixture(
            &directory,
            &context,
            &keys,
            0xC0,
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let projected = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &fixture.message,
            fixture.ownership.clone(),
        )
        .expect("checked dequeue publishes exact runtime ownership");
        assert!(projected.validate_frozen_physical());
        assert!(
            projected
                .leader_wire_runtime_receipt()
                .is_ok_and(|receipt| receipt.is_some())
        );
    }

    #[test]
    fn decision_retirement_releases_queued_leader_wire_runtime_owner() {
        let directory = TempDir::new().expect("temporary leader-wire Decision directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let fixture = leader_wire_proposal_fixture(
            &directory,
            &context,
            &keys,
            0xC1,
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &fixture.message.payload else {
            unreachable!("leader-wire fixture carries Proposal")
        };
        runtime
            .enqueue_network_with_ingress_ownership(
                fixture.message.clone(),
                fixture.ownership.clone(),
            )
            .expect("enqueue proposal with durable leader-wire runtime ownership");
        let ordinal = fixture.receipt.owner().admission_ordinal();
        assert_eq!(
            runtime.leader_wire_runtime_receipts.get(&ordinal),
            Some(&fixture.receipt)
        );

        let commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"leader-wire Decision state root"),
            Hash::new(b"leader-wire Decision event root"),
            Hash::new(b"leader-wire Decision reject root"),
            Hash::new(b"leader-wire Decision fee root"),
        );
        assert_eq!(
            runtime
                .retire_proposal_work_after_decision(proposal.round, proposal.subject, commitment,)
                .expect("Decision retires queued proposal ownership"),
            DecisionProposalRetirement::default()
        );
        assert_eq!(runtime.queued_commands(), 0);
        assert!(!runtime.leader_wire_runtime_receipts.contains_key(&ordinal));
        let terminals = runtime.take_leader_wire_runtime_terminals();
        let [LeaderWireRuntimeTerminal::Volatile(receipt)] = terminals.as_slice() else {
            panic!("Decision retirement must emit one volatile leader-wire terminal")
        };
        assert_volatile_leader_wire_release(&fixture, receipt);
        assert!(runtime.take_leader_wire_runtime_terminals().is_empty());

        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime after consuming Decision terminal");
        assert!(matches!(runtime.step(now), Ok(RuntimeStep::Idle)));
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn lock_retirement_releases_busy_deferred_leader_wire_runtime_owner() {
        let directory = TempDir::new().expect("temporary leader-wire lock directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let fixture = leader_wire_proposal_fixture(
            &directory,
            &context,
            &keys,
            0xC2,
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let (proposal, _deferred_ordinal) =
            bind_authenticated_deferred_proposal_for_test(&mut runtime, &fixture);
        let ordinal = fixture.receipt.owner().admission_ordinal();
        assert_eq!(
            runtime.leader_wire_runtime_receipts.get(&ordinal),
            Some(&fixture.receipt)
        );

        let locked_subject = runtime_manifest(&context, 0xC3).subject;
        assert_ne!(locked_subject, proposal.subject);
        assert_eq!(
            runtime
                .retire_unsafe_proposals_for_lock(proposal.round, locked_subject)
                .expect("lock retires unsafe Busy-deferred proposal"),
            1
        );
        assert!(
            runtime
                .driver
                .authenticated_deferred_admission_ordinals()
                .is_empty()
        );
        assert!(runtime.deferred_ingress_ownership.is_empty());
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
        assert!(!runtime.leader_wire_runtime_receipts.contains_key(&ordinal));
        let terminals = runtime.take_leader_wire_runtime_terminals();
        let [LeaderWireRuntimeTerminal::Volatile(receipt)] = terminals.as_slice() else {
            panic!("lock retirement must emit one volatile leader-wire terminal")
        };
        assert_volatile_leader_wire_release(&fixture, receipt);
        assert!(runtime.take_leader_wire_runtime_terminals().is_empty());

        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime after consuming lock terminal");
        assert!(matches!(runtime.step(now), Ok(RuntimeStep::Idle)));
        assert!(!runtime.fail_closed);

        // A BodyAvailable continuation can own an older causal lifecycle than
        // a proposal which crossed into Busy while the shared reducer fence
        // was closed. Once the fence opens, servicing that completion removes
        // the conflicting Busy proposal inside the adapter dispatch. The
        // runtime must terminalize the removed proposal's durable leader-wire
        // receipt after classifying the selected completion owner.
        let dispatch_directory =
            TempDir::new().expect("temporary dispatch-side leader-wire retirement directory");
        let (mut dispatch_runtime, dispatch_context, dispatch_keys) =
            authenticated_network_runtime(&dispatch_directory, RuntimeQueueConfig::new(8, 1, 1));
        let dispatch_tag = dispatch_runtime.round_tag();
        let body_parent = dispatch_runtime
            .mint_fresh_lifecycle_owner(
                dispatch_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::StartupRecovery,
                b"older-body-available-continuation",
            )
            .expect("reserve the older body continuation lifecycle");
        let body_ownership =
            RuntimeEffectOwnership::fresh(body_parent, RuntimeFreshRootKind::StartupRecovery);
        let dispatch_fixture = leader_wire_proposal_fixture(
            &dispatch_directory,
            &dispatch_context,
            &dispatch_keys,
            0xCB,
            dispatch_runtime.ingress.lifecycle_ordinals.clone(),
        );
        let (busy_proposal, busy_ordinal) =
            bind_authenticated_deferred_proposal_for_test(&mut dispatch_runtime, &dispatch_fixture);
        assert!(
            body_ownership.owner().lifecycle_ordinal()
                < dispatch_fixture.receipt.owner().admission_ordinal(),
            "the reconstructed body retains the frozen predecessor lifecycle"
        );
        let canonical_body = b"canonical body superseding Busy proposal".to_vec();
        let canonical_manifest = wire::PayloadManifest::derive(
            &dispatch_context,
            busy_proposal.round,
            busy_proposal.subject,
            u64::try_from(canonical_body.len()).expect("small canonical body length fits u64"),
            &[canonical_body],
        )
        .expect("derive a structurally valid conflicting canonical manifest");
        assert_ne!(canonical_manifest, busy_proposal.manifest);
        let reservation = dispatch_runtime
            .reserve_body_available_with_owner(dispatch_tag, canonical_manifest, &body_ownership)
            .expect("reserve the older causal BodyAvailable owner");
        dispatch_runtime
            .commit_body_available(reservation)
            .expect("publish the exact BodyAvailable completion");
        assert_eq!(dispatch_runtime.queued_commands(), 1);
        assert!(
            dispatch_runtime
                .eligible_deferred_admission_ordinals()
                .expect("compare the two exact lifecycle owners")
                .is_empty(),
            "the later Busy proposal cannot overtake the older body continuation"
        );
        assert!(
            dispatch_runtime
                .deferred_lifecycle_ownership
                .contains_key(&busy_ordinal)
        );

        let dispatch_now = Instant::now();
        dispatch_runtime
            .arm_live_clocks(dispatch_now)
            .expect("arm runtime for dispatch-side retirement");
        let body_step = dispatch_runtime
            .step(dispatch_now)
            .expect("the older BodyAvailable owner receives the FIFO turn");
        let body_scheduling = dispatch_runtime
            .take_last_scheduler_ownership()
            .expect("BodyAvailable dispatch retains exact scheduler ownership");
        assert_eq!(body_scheduling.selected, RuntimeSelectedOwnerKind::Fifo);
        let RuntimeStep::Advanced(body_effects) = body_step else {
            panic!("BodyAvailable dispatch unexpectedly idled")
        };
        dispatch_runtime
            .take_effect_ownership(body_effects.len())
            .expect("consume BodyAvailable effect ownership");
        assert_eq!(dispatch_runtime.queued_commands(), 0);
        assert!(
            dispatch_runtime
                .driver
                .authenticated_deferred_admission_ordinals()
                .is_empty()
        );
        assert!(dispatch_runtime.deferred_ingress_ownership.is_empty());
        assert!(dispatch_runtime.deferred_lifecycle_ownership.is_empty());
        let dispatch_receipt_ordinal = dispatch_fixture.receipt.owner().admission_ordinal();
        assert!(
            !dispatch_runtime
                .leader_wire_runtime_receipts
                .contains_key(&dispatch_receipt_ordinal)
        );
        let dispatch_terminals = dispatch_runtime.take_leader_wire_runtime_terminals();
        let [LeaderWireRuntimeTerminal::Volatile(receipt)] = dispatch_terminals.as_slice() else {
            panic!("BodyAvailable cleanup must retire the orphaned Busy proposal receipt")
        };
        assert_volatile_leader_wire_release(&dispatch_fixture, receipt);
        assert!(!dispatch_runtime.fail_closed);

        // Materializing the same older completion can prune a conflicting
        // proposal which is still in FIFO rather than Busy. Its durable
        // receipt is allowed to remain Runtime only while the exact finite
        // BodyAvailable predecessor is physically queued; servicing that
        // predecessor must publish the volatile terminal in the same turn.
        let queued_directory =
            TempDir::new().expect("temporary queued leader-wire retirement directory");
        let (mut queued_runtime, queued_context, queued_keys) =
            authenticated_network_runtime(&queued_directory, RuntimeQueueConfig::new(8, 1, 1));
        let queued_tag = queued_runtime.round_tag();
        let queued_body_parent = queued_runtime
            .mint_fresh_lifecycle_owner(
                queued_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::StartupRecovery,
                b"older-queued-body-available-continuation",
            )
            .expect("reserve the older queued body lifecycle");
        let queued_body_ownership = RuntimeEffectOwnership::fresh(
            queued_body_parent,
            RuntimeFreshRootKind::StartupRecovery,
        );
        let queued_fixture = leader_wire_proposal_fixture(
            &queued_directory,
            &queued_context,
            &queued_keys,
            0xCC,
            queued_runtime.ingress.lifecycle_ordinals.clone(),
        );
        let wire::ConsensusMessageV2Payload::Proposal(queued_proposal) =
            &queued_fixture.message.payload
        else {
            unreachable!("queued leader-wire fixture carries Proposal")
        };
        queued_runtime
            .enqueue_network_with_ingress_ownership(
                queued_fixture.message.clone(),
                queued_fixture.ownership.clone(),
            )
            .expect("enqueue the conflicting leader-wire proposal");
        let queued_receipt_ordinal = queued_fixture.receipt.owner().admission_ordinal();
        assert!(
            queued_body_ownership.owner().lifecycle_ordinal() < queued_receipt_ordinal,
            "the body completion retains the older causal lifecycle"
        );
        let queued_canonical_body = b"canonical body superseding queued proposal".to_vec();
        let queued_canonical_manifest = wire::PayloadManifest::derive(
            &queued_context,
            queued_proposal.round,
            queued_proposal.subject,
            u64::try_from(queued_canonical_body.len())
                .expect("small queued canonical body length fits u64"),
            &[queued_canonical_body],
        )
        .expect("derive a conflicting canonical manifest for the queued proposal");
        assert_ne!(queued_canonical_manifest, queued_proposal.manifest);
        let queued_reservation = queued_runtime
            .reserve_body_available_with_owner(
                queued_tag,
                queued_canonical_manifest,
                &queued_body_ownership,
            )
            .expect("reserve the queued-prune BodyAvailable owner");
        queued_runtime
            .commit_body_available(queued_reservation)
            .expect("atomically replace the conflicting FIFO proposal");
        assert_eq!(queued_runtime.queued_commands(), 1);
        assert!(
            queued_runtime
                .ingress
                .commands
                .iter()
                .all(|queued| matches!(&queued.command, AdapterCommand::BodyAvailable { .. }))
        );
        assert_eq!(
            queued_runtime
                .leader_wire_runtime_receipts
                .get(&queued_receipt_ordinal),
            Some(&queued_fixture.receipt),
            "the finite queued completion temporarily owns retirement of the pruned receipt"
        );
        assert!(queued_runtime.pending_leader_wire_terminals.is_empty());

        let queued_now = Instant::now();
        queued_runtime
            .arm_live_clocks(queued_now)
            .expect("arm runtime for queued-prune retirement");
        let queued_body_step = queued_runtime
            .step(queued_now)
            .expect("service the exact queued BodyAvailable predecessor");
        let queued_scheduling = queued_runtime
            .take_last_scheduler_ownership()
            .expect("queued BodyAvailable dispatch retains scheduler ownership");
        assert_eq!(queued_scheduling.selected, RuntimeSelectedOwnerKind::Fifo);
        let RuntimeStep::Advanced(queued_body_effects) = queued_body_step else {
            panic!("queued BodyAvailable dispatch unexpectedly idled")
        };
        queued_runtime
            .take_effect_ownership(queued_body_effects.len())
            .expect("consume queued BodyAvailable effect ownership");
        assert_eq!(queued_runtime.queued_commands(), 0);
        assert!(
            !queued_runtime
                .leader_wire_runtime_receipts
                .contains_key(&queued_receipt_ordinal)
        );
        let queued_terminals = queued_runtime.take_leader_wire_runtime_terminals();
        let [LeaderWireRuntimeTerminal::Volatile(receipt)] = queued_terminals.as_slice() else {
            panic!("queued proposal pruning must emit one volatile leader-wire terminal")
        };
        assert_volatile_leader_wire_release(&queued_fixture, receipt);
        assert!(!queued_runtime.fail_closed);
    }

    #[test]
    fn production_authenticated_preflight_is_never_semantic_only_coalesce() {
        let directory = TempDir::new().expect("temporary authenticated-preflight directory");
        let (runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let message = signed_runtime_proposal(&context, &keys, 0xC4);
        let authenticated = runtime
            .driver
            .authenticate(message)
            .expect("authenticate the production Proposal command");
        let command = AdapterCommand::Authenticated(authenticated);

        assert_eq!(
            runtime
                .driver
                .preflight_runtime_command_admission(runtime.round_tag(), &command),
            RuntimeCommandAdmissionPreflight::Admit
        );
    }

    #[test]
    fn semantic_only_authenticated_coalesce_fails_before_receipt_registration() {
        let directory = TempDir::new().expect("temporary coalesce-defense directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let existing = signed_runtime_proposal(&context, &keys, 0xC5);
        runtime
            .enqueue_network(existing)
            .expect("retain an existing authenticated semantic owner");
        let queued_before = runtime.queued_commands();

        let candidate = leader_wire_proposal_fixture(
            &directory,
            &context,
            &keys,
            0xC6,
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let candidate_ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &candidate.message,
            candidate.ownership.clone(),
        )
        .expect("project the fresh leader-wire runtime receipt");
        assert!(
            candidate_ownership
                .leader_wire_runtime_receipt()
                .expect("inspect exact candidate receipt")
                .is_some()
        );
        assert!(runtime.leader_wire_runtime_receipts.is_empty());

        assert!(matches!(
            runtime.reject_authenticated_preflight_coalescence(
                RuntimeCommandAdmissionPreflight::Coalesce,
            ),
            Err(NetworkIngressError::FailClosed)
        ));
        assert_eq!(
            runtime.queued_commands(),
            queued_before,
            "defensive rejection must not delete the existing semantic owner"
        );
        assert!(
            runtime.leader_wire_runtime_receipts.is_empty(),
            "semantic-only coalescence cannot register an ownerless runtime receipt"
        );
        assert!(runtime.pending_leader_wire_terminals.is_empty());
        assert!(runtime.fail_closed);
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
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: decision_manifest.clone(),
                durable_receipt: decision_durable.clone(),
                validated_receipt: decision_validated,
            },
        );
        let other_local_manifest = runtime_manifest(&context, 0xD2);
        let (other_durable, other_validated) = receipts(&other_local_manifest);
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: other_local_manifest.clone(),
                durable_receipt: other_durable,
                validated_receipt: other_validated,
            },
        );
        runtime
            .enqueue_body_available(owner_tag, decision_manifest.clone())
            .expect("enqueue body-recovery completion");
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::BodyStored {
                round: decision_manifest.round,
                subject: decision_manifest.subject,
                receipt: decision_durable,
            },
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::ApplicationCompleted(decision_manifest.subject),
        );

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
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: duplicate_manifest.clone(),
                durable_receipt: duplicate_durable,
                validated_receipt: duplicate_validated,
            },
        );
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
        stage_completion_for_queue_test(
            &mut runtime,
            stale_tag,
            AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable,
                validated_receipt: validated,
            },
        );

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
        const PHASE_INVENTORY: [&str; 2] =
            ["decided_local_proposal_ready", "application_completed"];

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
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable.clone(),
                validated_receipt: validated.clone(),
            },
        );
        runtime
            .enqueue_local_proposal(
                owner_tag,
                manifest.clone(),
                durable.clone(),
                validated.clone(),
            )
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

        let mut suppressed_phases = Vec::new();
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        runtime
            .enqueue_local_proposal(owner_tag, manifest.clone(), durable, validated)
            .expect("the decided validated body suppresses a drained local completion retry");
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("decided_local_proposal_ready");

        runtime
            .enqueue_application_completed(owner_tag, manifest.subject)
            .expect("enqueue exact Apply acknowledgement");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(now)
                .expect("dispatch exact Apply acknowledgement"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        let next_ordinal = runtime.ingress.next_admission_ordinal;
        for _ in 0..3 {
            runtime
                .enqueue_application_completed(owner_tag, manifest.subject)
                .expect("an applied-height acknowledgement retry is a monotone stutter");
        }
        assert_eq!(runtime.queued_commands(), 0);
        assert_eq!(runtime.ingress.next_admission_ordinal, next_ordinal);
        suppressed_phases.push("application_completed");
        assert_eq!(suppressed_phases, PHASE_INVENTORY);
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
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable,
                validated_receipt: validated,
            },
        );

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
    fn unbound_direct_prepare_and_commit_votes_are_recoverable_after_validation() {
        for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
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
            let signed_vote = signed_runtime_vote(
                &keys,
                manifest.round,
                phase,
                manifest.subject,
                validated.execution_commitment(),
            );

            let far_future_round = wire::ConsensusRound {
                view: u64::MAX,
                ..manifest.round
            };
            let signed_far_future = signed_runtime_vote(
                &keys,
                far_future_round,
                phase,
                manifest.subject,
                validated.execution_commitment(),
            );
            assert!(
                runtime.can_admit_network_message(&signed_far_future),
                "a structurally valid far-future {phase:?} vote must drain without certified local view authority"
            );
            assert!(matches!(
                runtime.enqueue_network(signed_far_future),
                Err(NetworkIngressError::Authentication(
                    AdapterError::MissingExecutionCommitment
                ))
            ));
            assert_eq!(runtime.queued_commands(), 0);
            assert!(
                !runtime.fail_closed,
                "rejecting a far-future unbound {phase:?} vote must not poison the runtime"
            );

            let mut malformed_future = signed_vote.clone();
            let wire::ConsensusMessageV2Payload::Vote(malformed_vote) =
                &mut malformed_future.payload
            else {
                unreachable!("fixture is a direct vote");
            };
            malformed_vote.round.view = u64::MAX;
            malformed_vote.proposal_round.view = u64::MAX;
            malformed_vote.signature.clear();
            assert!(
                runtime.can_admit_network_message(&malformed_future),
                "a structurally invalid far-future {phase:?} vote must drain for normal rejection"
            );
            assert!(matches!(
                runtime.enqueue_network(malformed_future),
                Err(NetworkIngressError::Authentication(_))
            ));
            assert_eq!(runtime.queued_commands(), 0);

            assert!(
                !runtime.can_admit_network_message(&signed_vote),
                "an early {phase:?} vote must remain fair-ingress owned until its proposal is validated"
            );
            // The mutating seam still rejects a caller that bypasses the
            // non-mutating fair-ingress gate.
            assert!(matches!(
                runtime.enqueue_network(signed_vote.clone()),
                Err(NetworkIngressError::Authentication(
                    AdapterError::MissingExecutionCommitment
                ))
            ));
            assert_eq!(runtime.queued_commands(), 0);
            assert!(
                !runtime.fail_closed,
                "recoverable {phase:?} authentication rejection must not poison the runtime"
            );

            let proposer = context.leader(manifest.round.view);
            let mut proposal = wire::Proposal {
                round: manifest.round,
                proposer,
                subject: manifest.subject,
                manifest: manifest.clone(),
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
            runtime
                .enqueue_network(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Proposal(proposal),
                ))
                .expect("matching proposal establishes a pending body pipeline");
            assert_eq!(runtime.queued_commands(), 1);
            assert!(
                !runtime.can_admit_network_message(&signed_vote),
                "the {phase:?} vote remains a recoverable fair-ingress prerequisite while validation is pending"
            );
            runtime
                .arm_live_clocks(Instant::now())
                .expect("arm fixture clocks before dispatch");
            runtime
                .step_and_take_scheduler_ownership_for_test(Instant::now())
                .expect("dispatch matching proposal");
            assert_eq!(runtime.queued_commands(), 0);
            assert!(
                !runtime.can_admit_network_message(&signed_vote),
                "the registered manifest keeps the {phase:?} vote deferred while validation is pending"
            );
            assert!(!runtime.fail_closed);

            let reducer_round = reducer::Round::new(manifest.round.height, manifest.round.view);
            let reducer_subject =
                reducer::Subject::new(Hash::new(manifest.subject.encode()).into());
            let reducer_tag_before_binding = runtime.driver.reducer.current_tag();
            let reducer_body_before_binding = runtime
                .driver
                .reducer
                .body_state(reducer_round, reducer_subject);
            runtime
                .bind_validated_body(&manifest, &validated)
                .expect("live validation establishes canonical commitment authority");
            assert_eq!(
                runtime.driver.reducer.current_tag(),
                reducer_tag_before_binding,
                "wire-authority binding cannot retag the reducer"
            );
            assert_eq!(
                runtime
                    .driver
                    .reducer
                    .body_state(reducer_round, reducer_subject),
                reducer_body_before_binding,
                "wire-authority binding cannot revive a reducer consumer"
            );
            assert!(
                runtime.can_admit_network_message(&signed_vote),
                "the retained fair-ingress {phase:?} vote becomes drainable after validation"
            );

            let conflicting_commitment = wire::ExecutionCommitment::without_topups(
                Hash::new(b"conflicting early vote parent state"),
                Hash::new(b"conflicting early vote post state"),
                Hash::new(b"conflicting early vote ordinary writes"),
                Hash::new(b"conflicting early vote executed block"),
            );
            assert_ne!(
                conflicting_commitment,
                validated.execution_commitment(),
                "the conflict fixture must differ from canonical validation"
            );
            let conflicting_vote = signed_runtime_vote(
                &keys,
                manifest.round,
                phase,
                manifest.subject,
                conflicting_commitment,
            );
            assert!(
                runtime.can_admit_network_message(&conflicting_vote),
                "a conflicting bound {phase:?} vote must drain for authenticated rejection"
            );
            assert!(matches!(
                runtime.enqueue_network(conflicting_vote),
                Err(NetworkIngressError::Authentication(
                    AdapterError::ConflictingExecutionCommitment
                ))
            ));
            assert_eq!(runtime.queued_commands(), 0);
            assert!(
                !runtime.fail_closed,
                "conflicting {phase:?} vote rejection must not poison the runtime"
            );

            runtime
                .enqueue_network(signed_vote)
                .expect("the same signed canonical vote becomes admissible after validation");
            assert_eq!(runtime.queued_commands(), 1);
            assert!(!runtime.fail_closed);

            let stale_directory = TempDir::new().expect("temporary stale-vote directory");
            let (mut stale_runtime, stale_context, stale_keys) =
                authenticated_network_runtime(&stale_directory, RuntimeQueueConfig::new(8, 1, 1));
            let stale_manifest = runtime_manifest(&stale_context, 0xD9);
            let stale_durable = DurableBodyReceipt::for_test(
                stale_context.id(),
                stale_manifest.round,
                stale_manifest.subject,
                HashOf::new(&stale_manifest),
            );
            let stale_validated = ValidatedBodyReceipt::for_test(stale_durable);
            let stale_message = signed_runtime_vote(
                &stale_keys,
                stale_manifest.round,
                phase,
                stale_manifest.subject,
                stale_validated.execution_commitment(),
            );
            assert!(
                !stale_runtime.can_admit_network_message(&stale_message),
                "an unbound {phase:?} vote is retained while its view remains active"
            );
            let initial = stale_runtime.round_tag();
            let next = EventTag::new(
                initial.height(),
                initial.view() + 1,
                Generation::new(initial.generation().get() + 1),
            );
            observe_enter_view_for_test(&mut stale_runtime, initial, next, &stale_manifest);
            assert!(
                stale_runtime.can_admit_network_message(&stale_message),
                "view change releases an unmatched stale {phase:?} vote for bounded rejection"
            );
        }
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
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(vec![4]),
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(vec![5]),
        );
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

        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(vec![3]),
        );
        stage_completion_for_queue_test(
            &mut runtime,
            owner_tag,
            AdapterCommand::SignatureCompleted(vec![4]),
        );
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
        assert_eq!(
            runtime
                .driver
                .deferred_quorum_certificate_owner_tag(&exact_certificate),
            Some(owner_tag),
            "the exact canonical QC retains its Busy-deferred owner"
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
    fn exact_authenticated_timeout_certificate_from_distinct_sources_coalesces_in_one_runtime_slot()
    {
        let directory = TempDir::new().expect("temporary multi-source TC directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(4, 1, 1),
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
        assert!(matches!(
            timeout_effects.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));

        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                signed_runtime_timeout_certificate(&context, &keys),
            ));
        let first_source = PeerId::new(keys[1].public_key().clone());
        let second_source = PeerId::new(keys[2].public_key().clone());
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, first_source),
                )
                .expect("the first authenticated TC carrier owns the runtime command"),
            round_tag
        );
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, second_source),
                )
                .expect("the same TC from another source coalesces"),
            round_tag
        );
        assert_eq!(
            runtime.queued_commands(),
            1,
            "one exact aggregate TC must retain every bounded source carrier"
        );
        let retained = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the coalesced TC retains exact ingress ownership");
        assert!(retained.validate_exact());
        assert_eq!(retained.direct.len(), 2);

        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("the Busy TC dispatch retains its exact runtime owner");
        assert!(selected.validate_exact().is_ok());
        let deferred = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy TC retains the coalesced source carriers");
        assert!(deferred.validate_exact());
        assert_eq!(deferred.direct.len(), 2);
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn admitted_progress_runs_after_its_frozen_prefix_before_later_normal_churn() {
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

        let initial_queue = runtime.queue_snapshot(start);
        assert_eq!(initial_queue.normal.depth, 3);
        assert_eq!(initial_queue.progress.depth, 1);

        for (expected, replacement) in [(0, 3), (1, 4), (2, 5)] {
            runtime
                .step_and_take_scheduler_ownership_for_test(start)
                .expect("one frozen normal predecessor drains");
            assert_eq!(runtime.driver.delivered.last(), Some(&(initial, expected)));
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(replacement),
            )
            .expect("later normal churn may refill only the vacated normal slot");
        }
        runtime
            .step_and_take_scheduler_ownership_for_test(start)
            .expect("the admitted progress owner runs after its finite frozen prefix");
        assert_eq!(
            runtime.driver.delivered,
            vec![(initial, 0), (initial, 1), (initial, 2), (initial, 200)]
        );
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

        // Drain a periodic episode and the one-shot timeout before admitting
        // a new target. Every later runner entry is again exactly one whole
        // retransmit interval late. The drained timer's dormant semantic key
        // must not resurrect its old physical ordinal on each entry.
        let mut post_timeout = self::runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        post_timeout
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2))
            .expect("drain the first periodic episode");
        assert_eq!(post_timeout.driver.retransmits, vec![initial]);
        post_timeout
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("emit the one-shot absolute timeout");
        assert_eq!(post_timeout.driver.timeouts, vec![initial]);
        enqueue_fake(
            &mut post_timeout,
            initial,
            CommandClass::Normal,
            FakeCommand::record(9),
        )
        .expect("admit work after the old periodic owner drained");

        post_timeout
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(12))
            .expect("the admitted target precedes the fresh periodic episode");
        assert_eq!(post_timeout.driver.delivered, vec![(initial, 9)]);
        assert_eq!(
            post_timeout.driver.retransmits,
            vec![initial],
            "a drained timer cannot reacquire its old position ahead of the target"
        );
        post_timeout
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(14))
            .expect("the freshly positioned periodic episode follows the target");
        assert_eq!(post_timeout.driver.retransmits, vec![initial, initial]);
    }

    #[test]
    fn frozen_lifecycle_order_precedes_timeout_priority() {
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
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2))
            .expect("the older admitted FIFO lifecycle dispatches first");
        assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
        assert!(runtime.driver.retransmits.is_empty());
        assert!(runtime.driver.timeouts.is_empty());

        runtime
            .step(start + Duration::from_secs(10))
            .expect("the earlier frozen periodic lifecycle dispatches next");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("periodic retransmit publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::PeriodicTimer
        );
        assert_eq!(runtime.driver.retransmits, vec![initial]);
        assert!(runtime.driver.timeouts.is_empty());

        runtime
            .step(start + Duration::from_secs(12))
            .expect("the later absolute-timeout lifecycle dispatches last");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("absolute timeout publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Timeout
        );
        assert_eq!(runtime.driver.timeouts, vec![initial]);
        assert_eq!(
            runtime.driver.retransmits,
            vec![initial],
            "the absolute deadline cannot replenish the drained periodic owner"
        );
    }

    #[test]
    fn due_timeout_becomes_older_than_replenished_exact_serve_tickets() {
        let start = Instant::now();
        let initial = tag(0);
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            Vec::new(),
            lifecycle_ordinals.clone(),
        )
        .expect("construct runtime with the shared Serve source")
        .0;
        runtime
            .arm_live_clocks(start)
            .expect("arm shared-source runtime");

        let first_barrier = lifecycle_ordinals
            .reserve_one()
            .expect("reserve first exact Serve occurrence");
        assert!(
            !runtime
                .older_lifecycle_predates_exact_serve(
                    start + Duration::from_secs(10),
                    first_barrier,
                )
                .expect("first barrier freezes the due timeout"),
            "a clock first frozen behind this ticket cannot overtake it"
        );

        let second_barrier = lifecycle_ordinals
            .reserve_one()
            .expect("reserve a distinct retransmission occurrence");
        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(
                    start + Duration::from_secs(10),
                    second_barrier,
                )
                .expect("replenished barrier validates against the same source"),
            "the frozen timeout must predate every later exact ticket"
        );
        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("one bounded predecessor episode dispatches the timeout");
        assert_eq!(runtime.driver.timeouts, vec![initial]);
    }

    #[test]
    fn restored_serve_high_watermark_precedes_startup_runtime_owner() {
        let start = Instant::now();
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(41);
        let (mut runtime, startup) = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            FakeDriver::new(tag(0)),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            vec![FakeEffect::other()],
            lifecycle_ordinals.clone(),
        )
        .expect("construct restarted runtime after durable Serve waiter");
        let ownership = runtime
            .take_effect_ownership(startup.len())
            .expect("startup owner retains exact lifecycle sidecar");
        assert_eq!(ownership.len(), 1);
        assert_eq!(ownership[0].owner().lifecycle_ordinal(), 42);
        assert_eq!(
            lifecycle_ordinals
                .reserve_one()
                .expect("later exact Serve ticket follows startup recovery"),
            43
        );
    }

    #[test]
    fn full_runtime_churn_cannot_cross_an_exact_serve_ordinal() {
        let start = Instant::now();
        let initial = tag(0);
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            Vec::new(),
            lifecycle_ordinals.clone(),
        )
        .expect("construct runtime with shared admission order")
        .0;
        runtime
            .arm_live_clocks(start)
            .expect("arm shared-source runtime");
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("admit the frozen predecessor");
        let barrier = lifecycle_ordinals
            .reserve_one()
            .expect("reserve exact Serve position");
        for value in 2..=3 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .expect("fill only the later normal prefix");
        }

        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(start, barrier)
                .expect("compare the full runtime prefix")
        );
        runtime
            .step_and_take_scheduler_ownership_for_test(start)
            .expect("one bounded predecessor transition runs");
        assert_eq!(runtime.driver.delivered, vec![(initial, 1)]);
        assert_eq!(runtime.queued_commands(), 2);
        assert!(
            !runtime
                .older_lifecycle_predates_exact_serve(start, barrier)
                .expect("later churn remains behind the exact ticket")
        );
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
    fn stale_completion_retains_tag_and_precedes_a_later_due_retransmit() {
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
            .expect("the older admitted completion owns the first turn");
        assert_eq!(runtime.driver.delivered, vec![(stale, 9)]);
        assert!(runtime.driver.retransmits.is_empty());
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("the completion publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Fifo
        );
        runtime
            .take_effect_ownership(1)
            .expect("consume the completion effect owner before the next turn");

        // The retransmit lifecycle was frozen when it first became due, so it
        // owns the next turn after the older completion drains.
        runtime
            .step(start + Duration::from_secs(4))
            .expect("the frozen retransmit owns the next turn");
        assert_eq!(runtime.driver.retransmits, vec![current]);
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("the retransmit publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::PeriodicTimer
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
        // The TC-like progress command was admitted before the next runner
        // freeze, so it precedes the newly positioned old-view retransmit
        // episode. EnterView then resets both clocks and retires that stale
        // periodic owner before it can dispatch.
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(runtime.round_tag(), next);
        assert!(runtime.driver.retransmits.is_empty());
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9)),
            Ok(RuntimeStep::Idle)
        ));
        assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
        assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(22));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10)),
            Ok(RuntimeStep::Idle)
        ));
        let _ = runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(11));
        assert_eq!(runtime.driver.retransmits, vec![next]);
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
