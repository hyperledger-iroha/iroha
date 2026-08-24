//! Serialized runtime shell for the authoritative Sumeragi v2 adapter.
//!
//! This module owns scheduling and backpressure, not consensus state.
//! A class-aware arbiter serially delivers admitted commands to [`SumeragiV2Adapter`]
//! and returns every [`AdapterEffect`] unchanged. Only `EnterView` is inspected:
//! installing a certified view alone may restart round and retransmission clocks.
//! The round deadline grows to a finite ceiling while retransmission stays fixed,
//! giving post-GST service more room without making a long-idle height wait hours.
//! Every admitted owner freezes both its receiver-local physical predecessor
//! cut and its logical lifecycle ordinal. A replay admitted at or after that
//! cut cannot overtake the owner even when the replay retains an older logical
//! identity; logical minima govern only the finite pre-cut predecessor set.
//! Within the exact eligible set, a small deterministic arbiter and cyclic
//! class service prevent a saturated normal prefix from starving a locked
//! Commit vote or trusted local completion.
#[cfg(test)]
use super::v2::{AdapterEquivocationEvidence, DeferredPriority};
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
use super::{
    FairV2IngressLeaderWirePhase, FairV2IngressLeaderWireSlot, FairV2IngressLeaderWireToken,
    FairV2IngressOwnershipEvidence,
    serviced_candidate_store::{
        LeaderWireLifecycleRuntimeReceipt, ProducerContinuationHandoffToken,
        ProducerContinuationTerminalToken,
    },
    v2::{
        AdapterEffect, AdapterError, AuthenticatedConsensusMessage, BodyPipelineCompletionEvidence,
        DecisionLocalProposalDisposition, DeferredAdmissionOrdinalSource, DeferredEventKind,
        DeferredOccurrenceOwnershipEvidence, DeferredRuntimeOwnershipSeal, DeferredServiceEvidence,
        LifecycleDecisionApplyAdapterCompletionAuthorityV1, LiveProposalIntentWalSignHandoffV1,
        LiveWalFrameIdentity, PersistedWalFrameLocatorV1,
        PreparedLifecycleDecisionApplyAdapterCompletionV1, ProducerContinuationHandoffEvidence,
        ReadyDurableValidateAdapterPublicationKind, RecoveredWalControlSign,
        RecoveredWalDecisionFetch, RecoveredWalFrameIdentity, RecoveredWalVoteSign,
        RegisteredPrepareInvalidBodyReportCapability, RegisteredPrepareValidateSignCapability,
        SignRequest, SumeragiV2Adapter, VerifiedHeightContext, classify_decided_local_proposal,
        proposal_is_safe_for_lock,
    },
    v2_body_store::{DurableBodyReceipt, ValidatedBodyReceipt},
    v2_first_release_recovery::{
        LocalBodyPreIntentReplaySealV1, LocalValidateReplayEvidenceV1,
        RemoteProposalFetchReplayEvidenceV1,
    },
    v2_lifecycle_coordinator::{
        AuthenticatedRecoveredWalControlProjection,
        AuthenticatedRecoveredWalDecisionFetchProjection,
        AuthenticatedRecoveredWalValidateLedgerParent, AuthenticatedRecoveredWalVoteProjection,
        DurableCertifiedFetchPendingMintPermit, DurableLifecycleOutputPendingMintPermit,
        DurableStandaloneValidatePendingMintPermit, DurableValidateReplayEvidenceV1,
        LifecycleDecisionApplyLineageV1, PreparedReadyDurableValidateAdapterPreview,
        PreparedReadyDurableValidateExecution, ReadyDurableValidateAdapterPreviewError,
        RecoveredLifecycleNextWalVoteCandidateProjectionV1, RecoveredLifecycleNextWalVoteSealV1,
        RecoveredWalVoteReplayEvidenceV1, RuntimeLifecycleOrdinalAuthority,
        runtime_lifecycle_ordinal_authority_after_high_watermark,
    },
};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode as _, Encode as _};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const RETRANSMIT_DIVISOR: u32 = 5;
/// Maximum base intervals assigned to one certified view.
const MAX_ROUND_TIMEOUT_MULTIPLIER: u128 = 10;
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
    authority: RuntimeLifecycleOrdinalAuthority,
}
impl RuntimeLifecycleOrdinalSource {
    /// Construct a source strictly after a durable high-watermark.
    ///
    /// The runtime surface intentionally constructs only its restricted view;
    /// coordinator handles cannot be injected through this type. The first
    /// reservable ordinal is exactly one greater than `high_watermark`, unless
    /// the watermark exhausts the `u128` namespace.
    /// This keeps runtime callers within the runtime-restricted authority API.
    pub(crate) fn after_high_watermark(high_watermark: u128) -> Self {
        Self {
            authority: runtime_lifecycle_ordinal_authority_after_high_watermark(high_watermark),
        }
    }
    /// Wrap the runtime-restricted half of the paired production launch authority.
    pub(in crate::sumeragi) const fn from_authority(
        authority: RuntimeLifecycleOrdinalAuthority,
    ) -> Self {
        Self { authority }
    }
    /// Reserve one globally unique ordinal.
    pub(crate) fn reserve_one(&self) -> Result<u128, String> {
        self.reserve_range(1)?
            .0
            .ok_or_else(|| "Sumeragi v2 lifecycle ordinal source returned no owner".to_owned())
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
        self.authority
            .with_checked_reservation(count, commit)
            .map_err(|_| EnqueueError::FailClosed)?
    }
    /// Hold the source at one already-minted successor while a reservation is
    /// materialized without allocating another ordinal.
    fn with_checked_current<T>(
        &self,
        commit: impl FnOnce(u128) -> Result<T, EnqueueError>,
    ) -> Result<T, EnqueueError> {
        self.authority
            .with_checked_current(commit)
            .map_err(|_| EnqueueError::FailClosed)?
    }
    fn reserve_range(&self, count: usize) -> Result<(Option<u128>, Option<u128>), String> {
        self.authority.reserve_range(count)
    }
    /// Advance a live source past a high-watermark restored by another owner.
    pub(crate) fn advance_past(&self, high_watermark: u128) -> Result<(), String> {
        self.authority.advance_past(high_watermark)
    }
    /// Read the next unused ordinal without reserving it.
    ///
    /// Runtime ingress uses this to initialize its diagnostic mirror from the
    /// same actor-global source that owns all lifecycle reservations.
    pub(super) fn next_ordinal(&self) -> Result<Option<u128>, String> {
        self.authority.next_ordinal()
    }
    /// Inspect the next actor-global lifecycle ordinal in tests.
    #[cfg(test)]
    pub(crate) fn next_ordinal_for_test(&self) -> Result<Option<u128>, String> {
        self.next_ordinal()
    }
    fn recognizes_minted(&self, ordinal: u128) -> Result<bool, String> {
        self.authority.recognizes_minted(ordinal)
    }
}
/// Derive the deadline for one certified view from the immutable base timeout.
///
/// Later views add one base interval through
/// [`MAX_ROUND_TIMEOUT_MULTIPLIER`], then stay fixed. Saturating arithmetic
/// avoids wraparound; liveness assumes post-GST service fits below the cap.
pub(super) fn round_timeout_for_view(base_timeout: Duration, view: u64) -> Duration {
    let multiplier = (u128::from(view) + 1).min(MAX_ROUND_TIMEOUT_MULTIPLIER);
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
/// additionally use the progress reserve. Trusted asynchronous completions may
/// use every ordinary slot, while one final physical slot is reserved solely
/// for an authenticated TC, CommitQC, or CommitCertificateResponse. Retained
/// certificates share that one credit; each certificate after the first
/// consumes ordinary Progress capacity. This prevents completion or retrying
/// Prepare traffic from excluding the certificate which retires its fence.
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
                .and_then(|reserved| reserved.checked_add(1))
                .is_none_or(|reserved| reserved >= self.capacity)
        {
            return Err(RuntimeConfigError::InvalidQueueAllocation);
        }
        Ok(self)
    }
    const fn normal_limit(self) -> usize {
        self.capacity - self.progress_reserve - self.completion_reserve - 1
    }
    const fn progress_limit(self) -> usize {
        self.capacity - self.completion_reserve - 1
    }
    const fn ordinary_total_limit(self) -> usize {
        self.capacity - 1
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
                "Sumeragi v2 runtime queue must reserve non-zero normal, progress, completion, and certified-fence capacity",
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
    /// An interrupted canonical Kura tip must remain permanently unarmed.
    PendingKuraRecovery,
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
            Self::PendingKuraRecovery => formatter
                .write_str("interrupted Kura-tip recovery cannot arm live pacemaker clocks"),
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
}
impl<E: fmt::Display> fmt::Display for RuntimeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(error) => write!(formatter, "Sumeragi v2 runtime failed closed: {error}"),
            Self::FailClosed => formatter.write_str("Sumeragi v2 runtime is fail-closed"),
            Self::ClocksNotArmed => {
                formatter.write_str("Sumeragi v2 pacemaker clocks are not armed")
            }
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
            Self::FailClosed | Self::ClocksNotArmed => None,
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
    /// Whether this carrier is a later physical delivery of an already
    /// reserved productive lifecycle.
    fn is_physical_leader_wire_replay(&self) -> Result<bool, RuntimeIngressMergeError> {
        match (
            self.leader_wire_token()?,
            self.leader_wire_physical_carrier()?,
        ) {
            (Some(token), Some((physical_ordinal, _))) => {
                Ok(token.admission_ordinal() < physical_ordinal)
            }
            (None, None) => Ok(false),
            (Some(_), None) | (None, Some(_)) => Err(RuntimeIngressMergeError::Conflict),
        }
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
    /// Match this frozen receiver carrier to one exact authenticated envelope.
    pub(in crate::sumeragi) fn exactly_matches_authenticated(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> bool {
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
mod exact_runtime_command_identity_sealed {
    pub trait Sealed {}
}
/// Derive the exact identity of a command rather than accepting an asserted
/// identity from the scheduler's caller.
///
/// The trait is sealed in this module so another production command type
/// cannot assert eligibility for the certified credit without extending this
/// audited classifier beside the exact command representation.
pub(crate) trait ExactRuntimeCommandIdentity:
    exact_runtime_command_identity_sealed::Sealed
{
    /// Project every command field which can distinguish reducer behavior.
    fn exact_runtime_command_identity(&self) -> RuntimeCommandIdentity;
    /// Return whether this exact command is an authenticated certificate which
    /// may be charged to the runtime's final physical fence-escape slot.
    fn is_certified_fence_escape(&self) -> bool {
        false
    }
}
impl exact_runtime_command_identity_sealed::Sealed for AuthenticatedConsensusMessage {}
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
    fn is_certified_fence_escape(&self) -> bool {
        wire_payload_is_certified_fence_escape(self.payload())
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
                    append_runtime_identity_field(
                        &mut semantic,
                        &carrier.first.wire_key.origin.encode(),
                    );
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
        canonical_bytes.extend_from_slice(b"iroha:sumeragi:v2:fresh-runtime-root:v2");
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
/// Compare one complete adapter effect with a closed lifecycle digest.
///
/// This fixed oracle lets a dedicated lifecycle executor reauthenticate an
/// exact carrier-derived effect without releasing either the runtime's hash or
/// a generic effect-identity constructor.
pub(in crate::sumeragi) fn adapter_effect_matches_lifecycle_digest(
    effect: &AdapterEffect,
    digest: &[u8; 32],
) -> bool {
    runtime_effect_identity_hash(
        production_adapter_effect_kind(effect),
        &production_adapter_effect_semantic_identity(effect),
    )
    .as_ref()
        == digest
}
#[cfg(test)]
/// Hash one adapter effect through the production semantic-identity projection.
pub(in crate::sumeragi) fn adapter_effect_identity_for_test(
    effect: &AdapterEffect,
) -> iroha_crypto::Hash {
    runtime_effect_identity_hash(
        production_adapter_effect_kind(effect),
        &production_adapter_effect_semantic_identity(effect),
    )
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
    /// Return the frozen height-context identity.
    pub(crate) const fn context_id(self) -> wire::HeightContextId {
        self.context_id
    }
    /// Return the exact execution/certificate round.
    pub(crate) const fn round(self) -> wire::ConsensusRound {
        self.round
    }
    /// Return the exact proposal/body origin round.
    pub(crate) const fn proposal_round(self) -> wire::ConsensusRound {
        self.proposal_round
    }
    /// Return the optional exact block subject.
    pub(crate) const fn subject(self) -> Option<wire::BlockSubject> {
        self.subject
    }
    /// Return inherited Prepare/Commit authority, when present.
    pub(crate) const fn phase(self) -> Option<wire::GlobalPhase> {
        self.phase
    }
    /// Return the deterministic execution commitment paired with authority.
    pub(crate) const fn execution_commitment(self) -> Option<wire::ExecutionCommitment> {
        self.execution_commitment
    }
    fn binds_exact_body_manifest(self, manifest: &wire::PayloadManifest) -> bool {
        self.validate_exact()
            && self.context_id == manifest.round.context_id
            && self.round == manifest.round
            && self.proposal_round == manifest.round
            && self.subject == Some(manifest.subject)
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
    /// Compare two carriers for the same physical body-fetch lineage.
    ///
    /// The candidate statement deliberately includes quorum authority, while
    /// the physical fetch is keyed only by its immutable consensus coordinates.
    /// This relation admits exactly the monotonic authority lattice used by
    /// that physical task: ordinary, Prepare, then Commit. A weaker carrier
    /// arriving after a stronger one is stale rather than an authority
    /// downgrade; callers retain the stronger task state in that case.
    fn fetch_authority_relation_to(self, incoming: Self) -> Option<RuntimeFetchAuthorityRelation> {
        if !self.validate_exact()
            || !incoming.validate_exact()
            || self.context_id != incoming.context_id
            || self.round != incoming.round
            || self.proposal_round != incoming.proposal_round
            || self.subject != incoming.subject
        {
            return None;
        }
        match (
            self.phase,
            self.execution_commitment,
            incoming.phase,
            incoming.execution_commitment,
        ) {
            (None, None, None, None) => Some(RuntimeFetchAuthorityRelation::Same),
            (None, None, Some(wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit), Some(_)) => {
                Some(RuntimeFetchAuthorityRelation::Upgrade)
            }
            (
                Some(wire::GlobalPhase::Prepare),
                Some(incumbent),
                Some(wire::GlobalPhase::Prepare),
                Some(successor),
            ) if incumbent == successor => Some(RuntimeFetchAuthorityRelation::Same),
            (
                Some(wire::GlobalPhase::Prepare),
                Some(incumbent),
                Some(wire::GlobalPhase::Commit),
                Some(successor),
            ) if incumbent == successor => Some(RuntimeFetchAuthorityRelation::Upgrade),
            (Some(wire::GlobalPhase::Prepare), Some(_), None, None) => {
                Some(RuntimeFetchAuthorityRelation::Stale)
            }
            (
                Some(wire::GlobalPhase::Commit),
                Some(incumbent),
                Some(wire::GlobalPhase::Commit | wire::GlobalPhase::Prepare),
                Some(successor),
            ) if incumbent == successor => Some(if incoming.phase == self.phase {
                RuntimeFetchAuthorityRelation::Same
            } else {
                RuntimeFetchAuthorityRelation::Stale
            }),
            (Some(wire::GlobalPhase::Commit), Some(_), None, None) => {
                Some(RuntimeFetchAuthorityRelation::Stale)
            }
            _ => None,
        }
    }
    /// Compare two carriers for one physical StoreBody or ValidateBody stage.
    ///
    /// EventTag incarnation may advance independently, but the certified and
    /// proposal rounds remain exact consensus coordinates. Only the ordinary,
    /// Prepare, then Commit authority lattice may change beneath the physical
    /// task owner.
    pub(crate) fn body_stage_authority_relation_to(
        self,
        incoming: Self,
    ) -> Option<RuntimeFetchAuthorityRelation> {
        self.fetch_authority_relation_to(incoming)
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
/// Authority relation between two exact carriers for one physical FetchBody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeFetchAuthorityRelation {
    /// Both carriers have the same authority statement.
    Same,
    /// The incoming carrier monotonically strengthens the task authority.
    Upgrade,
    /// The incoming carrier is weaker and must not downgrade the task.
    Stale,
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
#[derive(Clone, Copy)]
struct RuntimeEffectCandidateBindingProjectionParts<'a> {
    owner_projection_hash: &'a iroha_crypto::Hash,
    parent_owner_projection_hash: Option<&'a iroha_crypto::Hash>,
    effect_kind: u8,
    effect_identity: &'a iroha_crypto::Hash,
    candidate_kind: u8,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    candidate_semantic_identity: Option<&'a iroha_crypto::Hash>,
    candidate_identity: Option<&'a iroha_crypto::Hash>,
    effect_position: u8,
    effect_count: u8,
    candidate_position: u8,
    candidate_count: u8,
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
    parts: RuntimeEffectCandidateBindingProjectionParts<'_>,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-effect-binding:v1");
    append_runtime_identity_field(&mut projection, owner.projection_hash.as_ref());
    projection.push(runtime_effect_causality_code(causality));
    projection.push(runtime_effect_fresh_root_code(causality));
    append_runtime_identity_field(&mut projection, parts.owner_projection_hash.as_ref());
    append_optional_runtime_hash(&mut projection, parts.parent_owner_projection_hash);
    projection.push(parts.effect_kind);
    append_runtime_identity_field(&mut projection, parts.effect_identity.as_ref());
    projection.push(parts.candidate_kind);
    match parts.candidate_statement {
        None => projection.push(0),
        Some(statement) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, &statement.semantic_identity());
        }
    }
    append_optional_runtime_hash(&mut projection, parts.candidate_semantic_identity);
    append_optional_runtime_hash(&mut projection, parts.candidate_identity);
    projection.push(parts.effect_position);
    projection.push(parts.effect_count);
    projection.push(parts.candidate_position);
    projection.push(parts.candidate_count);
    iroha_crypto::Hash::new(projection)
}
impl RuntimeEffectCandidateBinding {
    fn projection_parts(&self) -> RuntimeEffectCandidateBindingProjectionParts<'_> {
        RuntimeEffectCandidateBindingProjectionParts {
            owner_projection_hash: &self.owner_projection_hash,
            parent_owner_projection_hash: self.parent_owner_projection_hash.as_ref(),
            effect_kind: self.effect_kind,
            effect_identity: &self.effect_identity,
            candidate_kind: self.candidate_kind,
            candidate_statement: self.candidate_statement,
            candidate_semantic_identity: self.candidate_semantic_identity.as_ref(),
            candidate_identity: self.candidate_identity.as_ref(),
            effect_position: self.effect_position,
            effect_count: self.effect_count,
            candidate_position: self.candidate_position,
            candidate_count: self.candidate_count,
        }
    }
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
        let owner_projection_hash = owner.projection_hash;
        let projection_hash = runtime_effect_candidate_binding_projection_hash(
            owner,
            causality,
            RuntimeEffectCandidateBindingProjectionParts {
                owner_projection_hash: &owner_projection_hash,
                parent_owner_projection_hash: parent_owner_projection_hash.as_ref(),
                effect_kind,
                effect_identity: &effect_identity,
                candidate_kind,
                candidate_statement,
                candidate_semantic_identity: candidate_semantic_identity.as_ref(),
                candidate_identity: candidate_identity.as_ref(),
                effect_position,
                effect_count,
                candidate_position,
                candidate_count,
            },
        );
        let binding = Self {
            owner_projection_hash,
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
            projection_hash,
        };
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
                == runtime_effect_candidate_binding_projection_hash(
                    owner,
                    causality,
                    self.projection_parts(),
                )
    }
}
/// Sidecar metadata paired positionally with one reducer effect.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeEffectOwnership {
    owner: RuntimeLifecycleOwner,
    causality: RuntimeEffectCausality,
    binding: RuntimeEffectCandidateBinding,
    remote_proposal_fetch_replay: Option<RemoteProposalFetchReplayEvidenceV1>,
}
/// Lifecycle-owner selection consumed by the atomic effect-binding constructor.
///
/// This value cannot enter an executor or admission boundary: it deliberately
/// exposes no effect-ownership API and is consumed when a complete positional
/// batch is converted into exact [`RuntimeEffectOwnership`] values.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeEffectOwnerAssignment {
    owner: RuntimeLifecycleOwner,
    causality: RuntimeEffectCausality,
}
/// One-shot runtime-only permit for minting local pre-intent replay authority.
///
/// Only the serialized active-view producer cut can construct this value. Its
/// private non-Copy marker keeps an exact scheduling owner from being reused to
/// manufacture a second replay seal.
#[derive(Debug)]
pub(in crate::sumeragi) struct LocalBodyReplayMintPermit {
    _linearity: LocalBodyReplayMintLinearity,
}
#[derive(Debug)]
struct LocalBodyReplayMintLinearity;
impl Drop for LocalBodyReplayMintLinearity {
    fn drop(&mut self) {}
}
impl LocalBodyReplayMintPermit {
    fn new() -> Self {
        Self {
            _linearity: LocalBodyReplayMintLinearity,
        }
    }
}
/// Linear composite owning both one local Store capability and its replay seal.
///
/// Cloneable Store/Validate scheduling metadata may be projected only after an
/// exact-effect check; the non-decodable replay seal itself stays inside this
/// composite until the body store returns its exact durable receipt.
#[derive(Debug)]
#[must_use = "local proposal ownership must remain attached to its pre-intent replay seal"]
pub(crate) struct LocalProposalEffectOwnership {
    ownership: RuntimeEffectOwnership,
    replay: LocalBodyPreIntentReplaySealV1,
}
/// Inert exact key for one owned `LocalProposalReady` runtime command.
///
/// The key is not replay authority: it only lets the executor keep a
/// non-Clone authority beside the cloneable FIFO command and later prove that
/// the exact command, causal root, and ProposalIntent successor met again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LocalProposalReadyCommandIdentity {
    tag: EventTag,
    command_hash: iroha_crypto::Hash,
    causal_lifecycle_key: iroha_crypto::Hash,
    projection_hash: iroha_crypto::Hash,
}
fn local_proposal_ready_command_projection_hash(
    identity: &LocalProposalReadyCommandIdentity,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:local-proposal-ready-command:v1");
    append_runtime_identity_tag(&mut projection, identity.tag);
    append_runtime_identity_field(&mut projection, identity.command_hash.as_ref());
    append_runtime_identity_field(&mut projection, identity.causal_lifecycle_key.as_ref());
    iroha_crypto::Hash::new(projection)
}
impl LocalProposalReadyCommandIdentity {
    /// Derive the same inert key from a lifecycle-owned pending Validate binding.
    pub(in crate::sumeragi) fn from_exact_pending_handoff(
        tag: EventTag,
        manifest: &wire::PayloadManifest,
        durable_receipt: &DurableBodyReceipt,
        validated_receipt: &ValidatedBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        let effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        if validated_receipt.durable() != durable_receipt
            || durable_receipt.round() != manifest.round
            || durable_receipt.subject() != manifest.subject
            || durable_receipt.manifest_hash() != iroha_crypto::HashOf::new(manifest)
            || !pending.exactly_binds_adapter_effect(&effect)
        {
            return None;
        }
        let command = AdapterCommand::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            validated_receipt: validated_receipt.clone(),
        }
        .exact_runtime_command_identity();
        if !command.validate_exact() {
            return None;
        }
        let mut identity = Self {
            tag,
            command_hash: command.canonical_hash,
            causal_lifecycle_key: *pending.causal_lifecycle_key(),
            projection_hash: iroha_crypto::Hash::new([]),
        };
        identity.projection_hash = local_proposal_ready_command_projection_hash(&identity);
        identity.validate_exact().then_some(identity)
    }
    fn validate_exact(&self) -> bool {
        self.projection_hash == local_proposal_ready_command_projection_hash(self)
    }

    /// Compare the cloneable FIFO command with its retained Validate owner.
    pub(in crate::sumeragi) fn exactly_matches_handoff(
        &self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
        durable_receipt: &DurableBodyReceipt,
        validated_receipt: &ValidatedBodyReceipt,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        let command = AdapterCommand::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            validated_receipt: validated_receipt.clone(),
        }
        .exact_runtime_command_identity();
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        self.validate_exact()
            && self.tag == tag
            && command.validate_exact()
            && self.command_hash == command.canonical_hash
            && self.causal_lifecycle_key == validate_pending.causal_lifecycle_key
            && validate_pending.exactly_binds_adapter_effect(&validate_effect)
    }
    /// Match only the exact ProposalIntent successor of this queued handoff.
    pub(in crate::sumeragi) fn exactly_matches_proposal_intent(
        &self,
        validate_pending: &PendingRuntimeEffectBinding,
        manifest: &wire::PayloadManifest,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        let AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        } = effect
        else {
            return false;
        };
        let validate_effect = AdapterEffect::ValidateBody {
            tag: self.tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        self.validate_exact()
            && *tag == self.tag
            && proposal.signature.is_empty()
            && proposal.round == manifest.round
            && proposal.subject == manifest.subject
            && proposal.manifest == *manifest
            && validate_pending.exactly_binds_adapter_effect(&validate_effect)
            && validate_pending.causal_lifecycle_key == self.causal_lifecycle_key
            && ownership.exactly_binds_adapter_effect(effect)
            && ownership.owner().causal_origin().lifecycle_key == self.causal_lifecycle_key
    }
}
impl LocalProposalEffectOwnership {
    fn from_exact_assemble_body(
        ownership: RuntimeEffectOwnership,
        effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
    ) -> Option<Self> {
        let pending = ownership
            .exact_pending_adapter_effect_binding(effect)
            .ok()?;
        let replay = LocalBodyPreIntentReplaySealV1::from_exact_assemble_body(
            LocalBodyReplayMintPermit::new(),
            effect,
            pending,
            manifest,
        )?;
        Some(Self { ownership, replay })
    }
    /// Project scheduling metadata only for this composite's exact Store work.
    pub(in crate::sumeragi) fn exact_store_task_ownership(
        &self,
        effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
    ) -> Option<RuntimeEffectOwnership> {
        (self.ownership.exactly_binds_adapter_effect(effect)
            && self.replay.exactly_matches_store(effect, manifest))
        .then(|| self.ownership.clone())
    }
    /// Compare a retained Store task without exposing either authority part.
    pub(in crate::sumeragi) fn exactly_matches_store_task(
        &self,
        effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        self.ownership == *ownership
            && self.ownership.exactly_binds_adapter_effect(effect)
            && self.replay.exactly_matches_store(effect, manifest)
    }
    /// Preflight the fixed Store-to-Validate projection without releasing the seal.
    pub(in crate::sumeragi) fn exactly_projects_validate_task(
        &self,
        store_effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_ownership: &RuntimeEffectOwnership,
    ) -> bool {
        let Ok(validate_pending) =
            validate_ownership.exact_pending_adapter_effect_binding(validate_effect)
        else {
            return false;
        };
        self.replay.exactly_projects_validate(
            store_effect,
            manifest,
            receipt,
            validate_effect,
            &validate_pending,
        )
    }
    /// Consume the composite through the exact durable Store-to-Validate cut.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn project_exact_validate(
        self,
        store_effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
        receipt: &DurableBodyReceipt,
        validate_effect: &AdapterEffect,
        validate_ownership: &RuntimeEffectOwnership,
    ) -> Result<LocalValidateReplayEvidenceV1, Self> {
        let Ok(validate_pending) =
            validate_ownership.exact_pending_adapter_effect_binding(validate_effect)
        else {
            return Err(self);
        };
        let Self { ownership, replay } = self;
        match replay.bind_and_project_validate(
            store_effect,
            manifest,
            receipt,
            validate_effect,
            &validate_pending,
        ) {
            Ok(evidence) => Ok(evidence),
            Err(replay) => Err(Self { ownership, replay }),
        }
    }
    /// Construct the same closed composite for production-shaped executor tests.
    #[cfg(test)]
    pub(crate) fn for_test(
        ownership: RuntimeEffectOwnership,
        effect: &AdapterEffect,
        manifest: &wire::PayloadManifest,
    ) -> Option<Self> {
        Self::from_exact_assemble_body(ownership, effect, manifest)
    }
}
/// Move-only, ordinal-free authority for one exact adapter effect awaiting
/// lifecycle admission.
///
/// The old runtime owner is consulted only while this sealed value is minted.
/// Its integrity projection deliberately excludes the runtime lifecycle
/// ordinal, allowing the coordinator to become the sole logical ordinal
/// allocator in the production cutover. Fields and construction remain sealed
/// in this module, so sibling modules cannot assert a causal root, physical
/// identity, or inherited body statement.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PendingRuntimeEffectBinding {
    causal_lifecycle_key: iroha_crypto::Hash,
    effect_kind: u8,
    effect_identity: iroha_crypto::Hash,
    candidate_kind: u8,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    candidate_semantic_identity: Option<iroha_crypto::Hash>,
    projection_hash: iroha_crypto::Hash,
}
/// Move-only restart successor derived from one exact recovered WAL vote.
///
/// The consuming projection retains the complete verified WAL
/// sequence/persistence/frame-hash identity, the authenticated PrepareQC needed
/// by a recovered Commit vote, the exact unsigned `Sign` effect, the exact
/// Validate predecessor effect, and both ordinal-free causal bindings. It
/// cannot be cloned or minted from caller-supplied vote coordinates.
#[derive(Debug)]
#[must_use = "a recovered WAL vote successor must be joined to startup lifecycle recovery"]
pub(crate) struct RecoveredWalVoteSuccessor {
    wal_identity: RecoveredWalFrameIdentity,
    replay_evidence: RecoveredWalVoteReplayEvidenceV1,
    predecessor_effect: AdapterEffect,
    predecessor_pending: PendingRuntimeEffectBinding,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    _prepare_certificate: Option<wire::QuorumCertificate>,
}
/// Unforgeable one-shot permit for the recovered-WAL candidate projection seam.
///
/// Its field and constructor are private to this runtime module. Replay and
/// ledger evidence accept it only so separated evidence/effect parts cannot
/// invoke their candidate constructors.
pub(in crate::sumeragi) struct RecoveredWalCandidateProjectionPermit {
    _linearity: RecoveredWalCandidateProjectionLinearity,
}
/// Runtime-private one-shot permit for consuming one sealed follow-on WAL Vote.
///
/// The recovered seal owns the exact WAL identity, unsigned Sign effect, replay
/// evidence, and validated body receipt. Only this runtime module can mint the
/// permit which rejoins those constituents to their reconstructed pending
/// binding and canonical standalone lifecycle admission.
pub(in crate::sumeragi) struct RecoveredLifecycleNextWalVoteCandidateProjectionPermitV1 {
    _linearity: RecoveredLifecycleNextWalVoteCandidateProjectionLinearityV1,
}
/// Runtime-private one-shot permit for a recovered-frame pending owner.
///
/// The constructor stays in this module. The recovered control token consumes
/// the permit together with its non-decodable WAL identity, preventing decoded
/// locator bytes or a raw effect from reaching the mint.
pub(in crate::sumeragi) struct RecoveredWalControlPendingMintPermit {
    _linearity: RecoveredWalControlPendingMintLinearity,
}
/// Runtime-private one-shot permit for a recovered Decision Fetch owner.
///
/// Only the consuming WAL token projection can construct this permit, so
/// decoded replay bytes and caller-supplied Fetch effects cannot mint pending
/// ownership.
pub(in crate::sumeragi) struct RecoveredWalDecisionFetchPendingMintPermit {
    _linearity: RecoveredWalDecisionFetchPendingMintLinearity,
}
struct RecoveredWalDecisionFetchPendingMintLinearity;
impl Drop for RecoveredWalDecisionFetchPendingMintLinearity {
    fn drop(&mut self) {}
}
impl RecoveredWalDecisionFetchPendingMintPermit {
    fn new() -> Self {
        Self {
            _linearity: RecoveredWalDecisionFetchPendingMintLinearity,
        }
    }
}
struct RecoveredWalControlPendingMintLinearity;
impl Drop for RecoveredWalControlPendingMintLinearity {
    fn drop(&mut self) {}
}
impl RecoveredWalControlPendingMintPermit {
    fn new() -> Self {
        Self {
            _linearity: RecoveredWalControlPendingMintLinearity,
        }
    }
}
struct RecoveredWalCandidateProjectionLinearity;
impl Drop for RecoveredWalCandidateProjectionLinearity {
    fn drop(&mut self) {}
}
impl RecoveredWalCandidateProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: RecoveredWalCandidateProjectionLinearity,
        }
    }
}
struct RecoveredLifecycleNextWalVoteCandidateProjectionLinearityV1;
impl Drop for RecoveredLifecycleNextWalVoteCandidateProjectionLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextWalVoteCandidateProjectionPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleNextWalVoteCandidateProjectionLinearityV1,
        }
    }
}
/// Consume one adapter-authenticated follow-on WAL Vote into its complete
/// replay-authorized standalone Sign projection.
///
/// Failure returns the intact affine seal. No effect, pending owner, WAL
/// identity, body receipt, or candidate constituent crosses this boundary.
#[allow(clippy::result_large_err)]
pub(in crate::sumeragi) fn project_recovered_lifecycle_next_wal_vote_candidate(
    verified: &VerifiedHeightContext,
    seal: RecoveredLifecycleNextWalVoteSealV1,
) -> Result<RecoveredLifecycleNextWalVoteCandidateProjectionV1, RecoveredLifecycleNextWalVoteSealV1>
{
    seal.into_candidate_projection(
        RecoveredLifecycleNextWalVoteCandidateProjectionPermitV1::new(),
        RecoveredWalCandidateProjectionPermit::new(),
        verified,
    )
}
/// Consume one adapter-authenticated recovered control token into its exact
/// pending owner and replay-authorized lifecycle admission.
#[allow(clippy::result_large_err)]
pub(in crate::sumeragi) fn project_recovered_wal_control_sign(
    verified: &super::v2::VerifiedHeightContext,
    recovered: RecoveredWalControlSign,
) -> Result<AuthenticatedRecoveredWalControlProjection, RecoveredWalControlSign> {
    recovered.into_lifecycle_projection(
        RecoveredWalControlPendingMintPermit::new(),
        RecoveredWalCandidateProjectionPermit::new(),
        verified,
    )
}
/// Consume one authenticated Decision Fetch token into its closed lifecycle projection.
#[allow(clippy::result_large_err)]
pub(in crate::sumeragi) fn project_recovered_wal_decision_fetch(
    verified: &super::v2::VerifiedHeightContext,
    recovered: RecoveredWalDecisionFetch,
) -> Result<AuthenticatedRecoveredWalDecisionFetchProjection, RecoveredWalDecisionFetch> {
    recovered.into_lifecycle_projection(
        RecoveredWalDecisionFetchPendingMintPermit::new(),
        RecoveredWalCandidateProjectionPermit::new(),
        verified,
    )
}
/// Ownership-preserving failure from the consuming recovered-WAL projection.
#[must_use = "failed recovered WAL projection retains its move-only successor"]
pub(crate) enum RecoveredWalVoteProjectionFailure {
    /// The opaque WAL frame identity no longer matched its persisted locator.
    InvalidWalIdentity(RecoveredWalVoteSuccessor),
    /// The canonical replay evidence no longer matched the retained Sign vote.
    InvalidReplayEvidence(RecoveredWalVoteSuccessor),
    /// The selected persisted or durable Validate parent did not match.
    Parent(RecoveredWalVoteSuccessor),
    /// The recovered WAL evidence could not authorize its exact Sign child.
    Child(RecoveredWalVoteSuccessor),
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredWalVoteSuccessor {
    /// Revalidate the complete opaque WAL-frame identity retained by this successor.
    fn wal_identity_is_exact(&self) -> bool {
        self.wal_identity.is_exact()
    }
    /// Revalidate the inert canonical replay envelope against the retained Sign effect.
    pub(in crate::sumeragi) fn replay_evidence_is_exact(&self) -> bool {
        let AdapterEffect::Sign {
            tag,
            request: super::v2::SignRequest::Vote(vote),
        } = &self.effect
        else {
            return false;
        };
        self.replay_evidence
            .exactly_matches_recovered_vote(self.wal_identity, *tag, vote)
    }
    /// Consume this successor through an exact persisted ledger-parent seal.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_ledger_lifecycle_projection(
        self,
        verified: &VerifiedHeightContext,
        parent: &AuthenticatedRecoveredWalValidateLedgerParent,
    ) -> Result<AuthenticatedRecoveredWalVoteProjection, RecoveredWalVoteProjectionFailure> {
        if !self.wal_identity_is_exact() {
            return Err(RecoveredWalVoteProjectionFailure::InvalidWalIdentity(self));
        }
        if !self.replay_evidence_is_exact() {
            return Err(RecoveredWalVoteProjectionFailure::InvalidReplayEvidence(
                self,
            ));
        }
        let Some(parent_candidate) = parent.project_recovered_candidate(
            RecoveredWalCandidateProjectionPermit::new(),
            verified,
            &self.predecessor_effect,
            &self.predecessor_pending,
        ) else {
            return Err(RecoveredWalVoteProjectionFailure::Parent(self));
        };
        let Some(child_candidate) = self.replay_evidence.project_recovered_vote_candidate(
            RecoveredWalCandidateProjectionPermit::new(),
            verified,
            self.wal_identity,
            &self.effect,
            &self.pending,
        ) else {
            return Err(RecoveredWalVoteProjectionFailure::Child(self));
        };
        Ok(
            AuthenticatedRecoveredWalVoteProjection::from_runtime_projection(
                RecoveredWalCandidateProjectionPermit::new(),
                self,
                parent_candidate,
                child_candidate,
            ),
        )
    }
    /// Consume this successor through its retained durable Validate origin.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_durable_lifecycle_projection(
        self,
        verified: &VerifiedHeightContext,
        receipt: &DurableBodyReceipt,
        evidence: &DurableValidateReplayEvidenceV1,
    ) -> Result<AuthenticatedRecoveredWalVoteProjection, RecoveredWalVoteProjectionFailure> {
        if !self.wal_identity_is_exact() {
            return Err(RecoveredWalVoteProjectionFailure::InvalidWalIdentity(self));
        }
        if !self.replay_evidence_is_exact() {
            return Err(RecoveredWalVoteProjectionFailure::InvalidReplayEvidence(
                self,
            ));
        }
        let Some(parent_candidate) = evidence.project_recovered_validate_candidate(
            RecoveredWalCandidateProjectionPermit::new(),
            verified,
            &self.predecessor_effect,
            receipt,
            &self.predecessor_pending,
        ) else {
            return Err(RecoveredWalVoteProjectionFailure::Parent(self));
        };
        let Some(child_candidate) = self.replay_evidence.project_recovered_vote_candidate(
            RecoveredWalCandidateProjectionPermit::new(),
            verified,
            self.wal_identity,
            &self.effect,
            &self.pending,
        ) else {
            return Err(RecoveredWalVoteProjectionFailure::Child(self));
        };
        Ok(
            AuthenticatedRecoveredWalVoteProjection::from_runtime_projection(
                RecoveredWalCandidateProjectionPermit::new(),
                self,
                parent_candidate,
                child_candidate,
            ),
        )
    }
    /// Revalidate both retained effects against their opaque pending bindings.
    pub(in crate::sumeragi) fn concrete_pair_is_exact(&self) -> bool {
        self.predecessor_pending
            .exactly_binds_adapter_effect(&self.predecessor_effect)
            && self.pending.exactly_binds_adapter_effect(&self.effect)
    }
    /// Match one validated body to the retained Validate-to-Sign coordinates.
    pub(in crate::sumeragi) fn concrete_pair_matches_validation(
        &self,
        validated: &ValidatedBodyReceipt,
    ) -> bool {
        let (
            AdapterEffect::ValidateBody {
                tag: validate_tag,
                round: validate_round,
                subject: validate_subject,
            },
            AdapterEffect::Sign {
                tag: sign_tag,
                request: super::v2::SignRequest::Vote(vote),
            },
        ) = (&self.predecessor_effect, &self.effect)
        else {
            return false;
        };
        let tags_match = match vote.phase {
            wire::GlobalPhase::Prepare => validate_tag == sign_tag,
            wire::GlobalPhase::Commit => commit_tags_match(
                *validate_tag,
                *sign_tag,
                vote.round,
                CommitSuccessorTagRelation::RecoveredMonotone,
            ),
        };
        self.concrete_pair_is_exact()
            && tags_match
            && validated.durable().context_id() == validate_round.context_id
            && validated.durable().round() == *validate_round
            && validated.durable().subject() == *validate_subject
            && vote.proposal_round == *validate_round
            && vote.subject == *validate_subject
            && vote.execution_commitment == validated.execution_commitment()
    }
    /// Borrow only the child effect needed by closed registry installation.
    pub(in crate::sumeragi) const fn installed_child_effect(&self) -> &AdapterEffect {
        &self.effect
    }
    /// Derive the mandatory signed Broadcast binding without releasing the
    /// recovered vote's pending owner or WAL identity.
    pub(in crate::sumeragi) fn project_signed_broadcast_successor(
        &self,
        broadcast: &AdapterEffect,
    ) -> Option<PendingRuntimeEffectBinding> {
        self.pending
            .project_signed_broadcast_successor(&self.effect, broadcast)
    }
    /// Recheck one retained signed-Broadcast binding without releasing the vote owner.
    pub(in crate::sumeragi) fn signed_broadcast_successor_is_exact(
        &self,
        broadcast: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        self.project_signed_broadcast_successor(broadcast).as_ref() == Some(pending)
    }
}
fn pending_runtime_effect_binding_projection_hash(
    causal_lifecycle_key: &iroha_crypto::Hash,
    effect_kind: u8,
    effect_identity: &iroha_crypto::Hash,
    candidate_kind: u8,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    candidate_semantic_identity: Option<&iroha_crypto::Hash>,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:pending-effect-binding:v1");
    append_runtime_identity_field(&mut projection, causal_lifecycle_key.as_ref());
    projection.push(effect_kind);
    append_runtime_identity_field(&mut projection, effect_identity.as_ref());
    projection.push(candidate_kind);
    match candidate_statement {
        None => projection.push(0),
        Some(statement) => {
            projection.push(1);
            append_runtime_identity_field(&mut projection, &statement.semantic_identity());
        }
    }
    append_optional_runtime_hash(&mut projection, candidate_semantic_identity);
    iroha_crypto::Hash::new(projection)
}
/// Reconstruct the exact ordinal-free Validate-to-Sign successor named by a
/// ledger-authenticated parent and the adapter-authenticated current WAL vote.
///
/// No caller-supplied runtime owner or lifecycle ordinal participates. The
/// causal key comes only from the opaque LedgerV1 projection, while the
/// inherited body statement is rebuilt from the exact recovered vote and the
/// ledger's ordinary-versus-Prepare authority bit. Failure returns the WAL
/// authority intact and exposes no predecessor effect or pending binding.
#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::result_large_err)]
pub(crate) fn reconstruct_recovered_wal_vote_successor(
    parent: &AuthenticatedRecoveredWalValidateLedgerParent,
    recovered: RecoveredWalVoteSign,
) -> Result<RecoveredWalVoteSuccessor, RecoveredWalVoteSign> {
    if !parent.exactly_matches_recovered_vote(&recovered) {
        return Err(recovered);
    }
    let vote = recovered.vote();
    let predecessor = AdapterEffect::ValidateBody {
        tag: recovered.tag(),
        round: vote.proposal_round,
        subject: vote.subject,
    };
    let inherited = RuntimeCandidateSemanticStatement::new(
        vote.round,
        vote.proposal_round,
        Some(vote.subject),
        parent
            .inherited_prepare_authority()
            .then_some(wire::GlobalPhase::Prepare),
        parent
            .inherited_prepare_authority()
            .then_some(vote.execution_commitment),
    );
    let Some(candidate) =
        production_adapter_effect_candidate_binding(&predecessor, Some(&inherited))
            .ok()
            .flatten()
    else {
        return Err(recovered);
    };
    if candidate.statement != Some(inherited) {
        return Err(recovered);
    }
    let pending = PendingRuntimeEffectBinding::from_effect_candidate(
        parent.runtime_causal_lifecycle_key(),
        &predecessor,
        Some(&candidate),
    );
    if !pending.validate_exact(&predecessor) {
        return Err(recovered);
    }
    pending
        .project_recovered_wal_vote_successor(&predecessor, recovered)
        .map_err(|(_pending, recovered)| recovered)
}
#[derive(Clone, Copy)]
enum CommitSuccessorTagRelation {
    LiveExact,
    RecoveredMonotone,
}
fn commit_tags_match(
    predecessor: EventTag,
    successor: EventTag,
    vote_round: wire::ConsensusRound,
    relation: CommitSuccessorTagRelation,
) -> bool {
    match relation {
        CommitSuccessorTagRelation::LiveExact => {
            predecessor == successor
                && successor.height() == vote_round.height
                && successor.view() == vote_round.view
        }
        CommitSuccessorTagRelation::RecoveredMonotone => {
            predecessor.height() == vote_round.height
                && successor.height() == vote_round.height
                && predecessor.generation() == successor.generation()
                && predecessor.view() >= vote_round.view
                && predecessor.view() <= successor.view()
        }
    }
}
fn recovered_prepare_matches_commit_vote(
    prepare: &wire::QuorumCertificate,
    vote: &wire::Vote,
) -> bool {
    prepare.phase == wire::GlobalPhase::Prepare
        && vote.phase == wire::GlobalPhase::Commit
        && prepare.round == vote.round
        && prepare.proposal_round == vote.proposal_round
        && prepare.subject == vote.subject
        && prepare.execution_commitment == vote.execution_commitment
}
impl PendingRuntimeEffectBinding {
    fn from_effect_candidate(
        causal_lifecycle_key: iroha_crypto::Hash,
        effect: &AdapterEffect,
        candidate: Option<&RuntimeEffectCandidateSemantic>,
    ) -> Self {
        let effect_kind = production_adapter_effect_kind(effect);
        let effect_identity = runtime_effect_identity_hash(
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
        );
        let (candidate_kind, candidate_statement, candidate_semantic_identity) =
            candidate.map_or((RUNTIME_CANDIDATE_KIND_NONE, None, None), |candidate| {
                (
                    candidate.kind,
                    candidate.statement,
                    Some(runtime_effect_candidate_semantic_hash(
                        candidate.kind,
                        &candidate.semantic_identity,
                    )),
                )
            });
        let projection_hash = pending_runtime_effect_binding_projection_hash(
            &causal_lifecycle_key,
            effect_kind,
            &effect_identity,
            candidate_kind,
            candidate_statement,
            candidate_semantic_identity.as_ref(),
        );
        Self {
            causal_lifecycle_key,
            effect_kind,
            effect_identity,
            candidate_kind,
            candidate_statement,
            candidate_semantic_identity,
            projection_hash,
        }
    }

    /// Reconstruct the unique pending owner of a frame-bound Certified Fetch.
    ///
    /// The caller cannot mint the permit from decoded ledger bytes. It is
    /// issued only by the replay-family/body-frame join, after that join has
    /// reconstructed every effect field retained by the original binding.
    pub(in crate::sumeragi) fn from_durable_certified_fetch(
        _permit: DurableCertifiedFetchPendingMintPermit,
        causal_lifecycle_key: iroha_crypto::Hash,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        if !matches!(
            effect,
            AdapterEffect::FetchBody {
                certificate: Some(_),
                ..
            }
        ) {
            return None;
        }
        let candidate = production_adapter_effect_candidate_binding(effect, None).ok()??;
        let pending = Self::from_effect_candidate(causal_lifecycle_key, effect, Some(&candidate));
        pending.validate_exact(effect).then_some(pending)
    }
}
include!("v2_runtime_durable_recovery_pending.rs");
impl PendingRuntimeEffectBinding {
    /// Mint the unique pending owner of one exact payload-free live-WAL continuation.
    ///
    /// The causal key is derived from the non-decodable post-fsync frame seal
    /// and complete effect identity. `Apply` is deliberately excluded because
    /// its pending owner must instead project from the retained Validate
    /// predecessor after the durable body receipt joins the WAL source.
    pub(super) fn from_exact_live_wal_append(
        wal_identity: &LiveWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        if !wal_identity.is_exact()
            || !matches!(
                effect,
                AdapterEffect::Sign {
                    request: SignRequest::Proposal(_)
                        | SignRequest::Vote(_)
                        | SignRequest::TimeoutVote(_),
                    ..
                } | AdapterEffect::EnterView { .. }
            )
        {
            return None;
        }
        Self::from_exact_wal_locator(wal_identity.persisted_locator(), effect)
    }
    /// Mint the unique pending owner of one recovered Proposal/Timeout control Sign.
    ///
    /// The one-shot permit is private to the consuming recovered-token join.
    /// This uses the identical locator/semantic causal-root formula as live
    /// append and deliberately excludes phase votes, Decision, and EnterView.
    pub(in crate::sumeragi) fn from_exact_recovered_wal_frame(
        _permit: RecoveredWalControlPendingMintPermit,
        wal_identity: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        if !wal_identity.is_exact()
            || !matches!(
                effect,
                AdapterEffect::Sign {
                    request: SignRequest::Proposal(_) | SignRequest::TimeoutVote(_),
                    ..
                }
            )
        {
            return None;
        }
        Self::from_exact_wal_locator(wal_identity.persisted_locator(), effect)
    }
    /// Mint the unique pending owner of one adapter-sealed recovered phase Vote.
    ///
    /// The permit is minted only by the consuming runtime projection. A raw
    /// locator or decoded replay envelope therefore cannot use this seam to
    /// manufacture an independently executable Sign owner.
    pub(in crate::sumeragi) fn from_exact_recovered_next_wal_vote(
        _permit: &RecoveredLifecycleNextWalVoteCandidateProjectionPermitV1,
        wal_identity: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        if !wal_identity.is_exact()
            || !matches!(
                effect,
                AdapterEffect::Sign {
                    request: SignRequest::Vote(_),
                    ..
                }
            )
        {
            return None;
        }
        Self::from_exact_wal_locator(wal_identity.persisted_locator(), effect)
    }
    /// Mint the unique pending owner of one exact Decision-owned Fetch.
    pub(in crate::sumeragi) fn from_exact_recovered_wal_decision_fetch(
        _permit: RecoveredWalDecisionFetchPendingMintPermit,
        wal_identity: RecoveredWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        if !wal_identity.is_exact()
            || !matches!(
                effect,
                AdapterEffect::FetchBody {
                    manifest: None,
                    certificate: Some(_),
                    ..
                }
            )
        {
            return None;
        }
        Self::from_exact_wal_locator(wal_identity.persisted_locator(), effect)
    }
    fn from_exact_wal_locator(
        locator: PersistedWalFrameLocatorV1,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        if !locator.is_exact() {
            return None;
        }
        let semantic_identity = production_adapter_effect_semantic_identity(effect);
        let candidate = production_adapter_effect_candidate_binding(effect, None).ok()?;
        let mut causal_preimage = Vec::new();
        causal_preimage.extend_from_slice(b"iroha:sumeragi:v2:live-wal-pending-root:v1");
        append_runtime_identity_field(&mut causal_preimage, &locator.encode());
        append_runtime_identity_field(&mut causal_preimage, &semantic_identity);
        let causal_lifecycle_key = iroha_crypto::Hash::new(causal_preimage);
        let pending = Self::from_effect_candidate(causal_lifecycle_key, effect, candidate.as_ref());
        pending.validate_exact(effect).then_some(pending)
    }
    /// Borrow the immutable runtime causal-origin lifecycle key.
    pub(crate) const fn causal_lifecycle_key(&self) -> &iroha_crypto::Hash {
        &self.causal_lifecycle_key
    }
    /// Borrow the exact physical identity already bound to the complete effect.
    pub(crate) const fn exact_effect_identity(&self) -> &iroha_crypto::Hash {
        &self.effect_identity
    }
    /// Return the route-neutral candidate statement retained by the runtime.
    pub(crate) const fn candidate_statement(&self) -> Option<RuntimeCandidateSemanticStatement> {
        self.candidate_statement
    }
    fn validate_exact(&self, effect: &AdapterEffect) -> bool {
        let exact_candidate = match (
            self.candidate_kind,
            self.candidate_statement,
            self.candidate_semantic_identity.as_ref(),
        ) {
            (RUNTIME_CANDIDATE_KIND_NONE, None, None) => true,
            (kind, Some(statement), Some(identity)) => {
                kind != RUNTIME_CANDIDATE_KIND_NONE
                    && statement.validate_exact()
                    && *identity
                        == runtime_effect_candidate_semantic_hash(
                            kind,
                            &statement.semantic_identity(),
                        )
            }
            _ => false,
        };
        let effect_kind = production_adapter_effect_kind(effect);
        let expected_candidate_kind = production_adapter_effect_candidate_statement(effect)
            .map_or(RUNTIME_CANDIDATE_KIND_NONE, |(kind, _)| kind);
        self.effect_kind == effect_kind
            && self.candidate_kind == expected_candidate_kind
            && self.effect_identity
                == runtime_effect_identity_hash(
                    effect_kind,
                    &production_adapter_effect_semantic_identity(effect),
                )
            && exact_candidate
            && self.projection_hash
                == pending_runtime_effect_binding_projection_hash(
                    &self.causal_lifecycle_key,
                    self.effect_kind,
                    &self.effect_identity,
                    self.candidate_kind,
                    self.candidate_statement,
                    self.candidate_semantic_identity.as_ref(),
                )
    }
    /// Return whether this sealed pending binding still names the supplied
    /// complete concrete effect.
    pub(crate) fn exactly_binds_adapter_effect(&self, effect: &AdapterEffect) -> bool {
        self.validate_exact(effect)
    }
    /// Project the mandatory signed Broadcast successor of one exact Sign.
    ///
    /// The signed wire payload must be byte-for-byte the predecessor request
    /// with only its signature field filled. Broadcast owns no independent
    /// candidate statement, but it retains the immutable causal lifecycle key
    /// so its replay row cannot be substituted by an unrelated signed message.
    pub(in crate::sumeragi) fn project_signed_broadcast_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        let AdapterEffect::Sign { request, .. } = predecessor else {
            return None;
        };
        let AdapterEffect::Broadcast(message) = successor else {
            return None;
        };
        if !self.validate_exact(predecessor) || message.validate_version().is_err() {
            return None;
        }
        let signed_request_matches = match (request, &message.payload) {
            (
                super::v2::SignRequest::Proposal(unsigned),
                wire::ConsensusMessageV2Payload::Proposal(signed),
            ) => {
                let mut projected = signed.clone();
                let signature_is_present = !projected.signature.is_empty();
                projected.signature.clear();
                signature_is_present && &projected == unsigned
            }
            (
                super::v2::SignRequest::Vote(unsigned),
                wire::ConsensusMessageV2Payload::Vote(signed),
            ) => {
                let mut projected = signed.clone();
                let signature_is_present = !projected.signature.is_empty();
                projected.signature.clear();
                signature_is_present && &projected == unsigned
            }
            (
                super::v2::SignRequest::TimeoutVote(unsigned),
                wire::ConsensusMessageV2Payload::TimeoutVote(signed),
            ) => {
                let mut projected = signed.clone();
                let signature_is_present = !projected.signature.is_empty();
                projected.signature.clear();
                signature_is_present && &projected == unsigned
            }
            _ => false,
        };
        if !signed_request_matches {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, None);
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project the exact `StoreBody` successor of one certified Fetch without
    /// consulting or minting a legacy runtime lifecycle ordinal.
    ///
    /// The returned binding retains the Fetch's immutable causal key and full
    /// authenticated candidate statement while replacing only the concrete
    /// effect kind and identity. This is the closed handoff needed by the
    /// lifecycle coordinator's future direct `BodyAvailable` executor; no
    /// other predecessor/successor pair is accepted here. This pure projection
    /// is not independently executable authority: the registry must retain it
    /// inside a borrow-free, move-only parent-to-child transaction before the
    /// production cutover.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn project_certified_fetch_store_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        let (
            AdapterEffect::FetchBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
                certificate: Some(_),
                ..
            },
            AdapterEffect::StoreBody {
                tag: successor_tag,
                round: successor_round,
                subject: successor_subject,
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if predecessor_tag != successor_tag
            || predecessor_round != successor_round
            || predecessor_subject != successor_subject
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        let candidate =
            production_adapter_effect_candidate_binding(successor, Some(&inherited)).ok()??;
        if candidate.statement != Some(inherited) {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project the exact ordinary Proposal-Fetch successor into Store.
    ///
    /// The Proposal signature and receiver ingress remain in the opaque replay
    /// envelope; this method moves only the already-sealed runtime causal
    /// binding. Certified Fetches are deliberately excluded so the two replay
    /// origins cannot be interchanged by matching body coordinates.
    pub(in crate::sumeragi) fn project_proposal_fetch_store_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        let (
            AdapterEffect::FetchBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
                manifest: Some(_),
                certified_sources,
                certificate: None,
            },
            AdapterEffect::StoreBody {
                tag: successor_tag,
                round: successor_round,
                subject: successor_subject,
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if !certified_sources.is_empty()
            || predecessor_tag != successor_tag
            || predecessor_round != successor_round
            || predecessor_subject != successor_subject
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        let candidate =
            production_adapter_effect_candidate_binding(successor, Some(&inherited)).ok()??;
        if candidate.statement != Some(inherited) {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project the exact `ValidateBody` successor of one durable `StoreBody`.
    ///
    /// The returned binding retains the Store's immutable causal key and full
    /// inherited candidate statement while replacing only the concrete effect
    /// kind and identity. No lifecycle ordinal or independently executable
    /// authority is minted here: the usable value must remain sealed inside
    /// the future move-only registry transaction which advances Store to
    /// Validate.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn project_store_validate_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        let (
            AdapterEffect::StoreBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::ValidateBody {
                tag: successor_tag,
                round: successor_round,
                subject: successor_subject,
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if predecessor_tag != successor_tag
            || predecessor_round != successor_round
            || predecessor_subject != successor_subject
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        let candidate =
            production_adapter_effect_candidate_binding(successor, Some(&inherited)).ok()??;
        if candidate.statement != Some(inherited) {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project the exact `Apply` successor of one successfully completed
    /// `ValidateBody` without consulting or minting a legacy lifecycle ordinal.
    ///
    /// The causal lifecycle key remains immutable. Candidate authority may
    /// change only through the already reviewed body-authority lattice:
    /// ordinary validation acquires Commit authority, Prepare authority is
    /// promoted by the matching CommitQC, and an existing exact Commit
    /// statement is retained. The returned binding is deterministic data, not
    /// independently executable authority; the future move-only Validate
    /// transaction must keep it sealed until the parent settles and the Apply
    /// child is admitted atomically.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn project_validate_apply_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        let (
            AdapterEffect::ValidateBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::Apply {
                tag: successor_tag,
                subject: successor_subject,
                certificate,
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if predecessor_tag != successor_tag
            || predecessor_subject != successor_subject
            || certificate.proposal_round != *predecessor_round
            || certificate.subject != *predecessor_subject
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        let candidate =
            production_adapter_effect_candidate_binding(successor, Some(&inherited)).ok()??;
        let successor_statement = candidate.statement?;
        inherited.commit_refinement_to(successor_statement)?;
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project the exact Prepare-vote `Sign` successor of one successfully
    /// completed ordinary `ValidateBody`.
    ///
    /// The Validate predecessor must still carry no quorum authority. The
    /// unsigned vote acquires Prepare authority for the exact inherited round,
    /// proposal round, subject, tag, and newly validated execution commitment.
    /// A Prepare- or Commit-authorized predecessor is rejected because those
    /// reducer states have different closed continuations. The returned value
    /// remains inert deterministic data; only the live sealed Validate
    /// transaction may pair it with the adapter-produced signing request.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn project_validate_sign_prepare_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        let (
            AdapterEffect::ValidateBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::Sign {
                tag: successor_tag,
                request: super::v2::SignRequest::Vote(vote),
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if predecessor_tag != successor_tag
            || successor_tag.height() != vote.round.height
            || successor_tag.view() != vote.round.view
            || vote.phase != wire::GlobalPhase::Prepare
            || vote.proposal_round != *predecessor_round
            || vote.subject != *predecessor_subject
            || !vote.signature.is_empty()
            || vote.execution_commitment.validate().is_err()
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        let candidate =
            production_adapter_effect_candidate_binding(successor, Some(&inherited)).ok()??;
        let successor_statement = candidate.statement?;
        if inherited.phase.is_some()
            || inherited.execution_commitment.is_some()
            || successor_statement.phase != Some(wire::GlobalPhase::Prepare)
            || successor_statement.execution_commitment.is_none()
            || inherited.context_id != successor_statement.context_id
            || inherited.round != successor_statement.round
            || inherited.proposal_round != successor_statement.proposal_round
            || inherited.subject != successor_statement.subject
        {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project the exact Commit-vote `Sign` successor of one successfully
    /// completed Prepare-authorized `ValidateBody`.
    ///
    /// The inherited candidate statement is the route-neutral proof that the
    /// body-stage owner already carried a registered Prepare certificate. The
    /// unsigned Commit vote must preserve every coordinate and the execution
    /// commitment while monotonically promoting that Prepare authority.
    ///
    /// An ordinary Validate which observed a concurrent PrepareQC remains
    /// rejected here. That distinct case is accepted only by
    /// [`Self::project_validate_sign_commit_successor_with_registered_prepare`]
    /// with the adapter-minted opaque carrier capability.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn project_validate_sign_commit_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        self.project_inherited_validate_commit_successor(
            predecessor,
            successor,
            CommitSuccessorTagRelation::LiveExact,
        )
    }
    fn project_inherited_validate_commit_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
        tag_relation: CommitSuccessorTagRelation,
    ) -> Option<Self> {
        let (
            AdapterEffect::ValidateBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::Sign {
                tag: successor_tag,
                request: super::v2::SignRequest::Vote(vote),
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if !commit_tags_match(*predecessor_tag, *successor_tag, vote.round, tag_relation)
            || vote.phase != wire::GlobalPhase::Commit
            || vote.proposal_round != *predecessor_round
            || vote.subject != *predecessor_subject
            || !vote.signature.is_empty()
            || vote.execution_commitment.validate().is_err()
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        let candidate =
            production_adapter_effect_candidate_binding(successor, Some(&inherited)).ok()??;
        let successor_statement = candidate.statement?;
        if inherited.commit_refinement_to(successor_statement)
            != Some(RuntimeCandidateAuthorityRefinement::PromotePrepare)
        {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project a Commit-vote successor for an ordinary Validate only while an
    /// opaque adapter capability proves the exact concurrently registered
    /// Prepare certificate.
    ///
    /// No certificate crosses this boundary. The capability first binds the
    /// predecessor and successor coordinates, then this runtime projection
    /// reconstructs only the registered Prepare statement needed for the
    /// monotone Prepare-to-Commit refinement.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn project_validate_sign_commit_successor_with_registered_prepare(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
        registered: &RegisteredPrepareValidateSignCapability,
    ) -> Option<Self> {
        if !registered.authorizes_ordinary_validate_commit(predecessor, successor) {
            return None;
        }
        self.project_ordinary_validate_commit_after_registered_prepare(
            predecessor,
            successor,
            CommitSuccessorTagRelation::LiveExact,
        )
    }
    /// Consume one adapter-authenticated WAL vote into its exact Validate
    /// successor binding.
    ///
    /// Prepare replay uses the ordinary Validate-to-Prepare projection. Commit
    /// replay first accepts the existing inherited-Prepare projection, then
    /// admits the one restart-only refinement where an ordinary Validate was
    /// already in flight when the adapter durably registered the exact
    /// PrepareQC carried by `LockAndCommit`. The move-only WAL authority is
    /// consumed together with the predecessor binding on success and both are
    /// returned intact on failure, so neither authority can mint more than one
    /// usable successor.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(crate) fn project_recovered_wal_vote_successor(
        self,
        predecessor: &AdapterEffect,
        recovered: RecoveredWalVoteSign,
    ) -> Result<RecoveredWalVoteSuccessor, (Self, RecoveredWalVoteSign)> {
        if !recovered.replay_evidence_is_exact() {
            return Err((self, recovered));
        }
        let successor = AdapterEffect::Sign {
            tag: recovered.tag(),
            request: super::v2::SignRequest::Vote(recovered.vote().clone()),
        };
        let pending = match recovered.vote().phase {
            wire::GlobalPhase::Prepare => {
                if recovered.prepare_certificate().is_some() {
                    None
                } else {
                    self.project_validate_sign_prepare_successor(predecessor, &successor)
                }
            }
            wire::GlobalPhase::Commit => recovered.prepare_certificate().and_then(|prepare| {
                self.project_recovered_inherited_validate_commit_successor(
                    predecessor,
                    &successor,
                    prepare,
                )
                .or_else(|| {
                    self.project_recovered_ordinary_validate_commit_successor(
                        predecessor,
                        &successor,
                        prepare,
                    )
                })
            }),
        };
        let Some(pending) = pending else {
            return Err((self, recovered));
        };
        let wal_identity = recovered.wal_identity();
        let replay_evidence = recovered.replay_evidence().clone();
        let prepare_certificate = recovered.prepare_certificate().cloned();
        drop(recovered);
        Ok(RecoveredWalVoteSuccessor {
            wal_identity,
            replay_evidence,
            predecessor_effect: predecessor.clone(),
            predecessor_pending: self,
            effect: successor,
            pending,
            _prepare_certificate: prepare_certificate,
        })
    }
    fn project_recovered_ordinary_validate_commit_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
        prepare: &wire::QuorumCertificate,
    ) -> Option<Self> {
        let AdapterEffect::Sign {
            request: super::v2::SignRequest::Vote(vote),
            ..
        } = successor
        else {
            return None;
        };
        if !recovered_prepare_matches_commit_vote(prepare, vote) {
            return None;
        }
        self.project_ordinary_validate_commit_after_registered_prepare(
            predecessor,
            successor,
            CommitSuccessorTagRelation::RecoveredMonotone,
        )
    }
    fn project_recovered_inherited_validate_commit_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
        prepare: &wire::QuorumCertificate,
    ) -> Option<Self> {
        let AdapterEffect::Sign {
            request: super::v2::SignRequest::Vote(vote),
            ..
        } = successor
        else {
            return None;
        };
        if !recovered_prepare_matches_commit_vote(prepare, vote) {
            return None;
        }
        self.project_inherited_validate_commit_successor(
            predecessor,
            successor,
            CommitSuccessorTagRelation::RecoveredMonotone,
        )
    }
    fn project_ordinary_validate_commit_after_registered_prepare(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
        tag_relation: CommitSuccessorTagRelation,
    ) -> Option<Self> {
        let (
            AdapterEffect::ValidateBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::Sign {
                tag: successor_tag,
                request: super::v2::SignRequest::Vote(vote),
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if !commit_tags_match(*predecessor_tag, *successor_tag, vote.round, tag_relation)
            || vote.phase != wire::GlobalPhase::Commit
            || vote.proposal_round != *predecessor_round
            || vote.subject != *predecessor_subject
            || !vote.signature.is_empty()
            || vote.execution_commitment.validate().is_err()
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        if inherited.phase.is_some()
            || inherited.execution_commitment.is_some()
            || inherited.context_id != vote.round.context_id
            || inherited.round != vote.round
            || inherited.proposal_round != vote.proposal_round
            || inherited.subject != Some(vote.subject)
        {
            return None;
        }
        let registered_prepare = RuntimeCandidateSemanticStatement::new(
            vote.round,
            vote.proposal_round,
            Some(vote.subject),
            Some(wire::GlobalPhase::Prepare),
            Some(vote.execution_commitment),
        );
        let candidate =
            production_adapter_effect_candidate_binding(successor, Some(&registered_prepare))
                .ok()??;
        let successor_statement = candidate.statement?;
        if registered_prepare.commit_refinement_to(successor_statement)
            != Some(RuntimeCandidateAuthorityRefinement::PromotePrepare)
        {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project the exact invalid-certified-body report emitted by one failed
    /// Prepare-authorized `ValidateBody` completion.
    ///
    /// Reports are not scheduler candidates, so the successor deliberately
    /// carries no candidate statement. Its complete certificate bytes remain
    /// bound by the concrete effect identity, while the certificate's phase,
    /// round, proposal round, subject, and commitment must exactly match the
    /// registered Prepare-carrier semantics inherited by the predecessor.
    ///
    /// An ordinary Validate which observed Prepare only after installation is
    /// handled by the separate move-only registered-Prepare projection below;
    /// this inherited path never accepts that refinement implicitly.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn project_validate_report_invalid_certified_body_successor(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        let (
            AdapterEffect::ValidateBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::ReportInvalidCertifiedBody {
                subject: successor_subject,
                certificate,
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if predecessor_tag.height() != certificate.round.height
            || predecessor_tag.view() != certificate.round.view
            || certificate.phase != wire::GlobalPhase::Prepare
            || certificate.round != *predecessor_round
            || certificate.proposal_round != *predecessor_round
            || certificate.subject != *predecessor_subject
            || successor_subject != predecessor_subject
            || certificate.execution_commitment.validate().is_err()
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        let registered_carrier = RuntimeCandidateSemanticStatement::new(
            certificate.round,
            certificate.proposal_round,
            Some(certificate.subject),
            Some(certificate.phase),
            Some(certificate.execution_commitment),
        );
        if inherited != registered_carrier
            || inherited.phase != Some(wire::GlobalPhase::Prepare)
            || production_adapter_effect_candidate_binding(successor, Some(&inherited))
                .ok()?
                .is_some()
        {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, None);
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
    /// Project the exact invalid-body report observed after an ordinary
    /// Validate owner was installed but before its matching PrepareQC arrived.
    ///
    /// The registered-Prepare capability is move-only and can be minted only
    /// by the fixed adapter rejection preview after checking its post-step
    /// registry. It supplies no statement, certificate, effect, or pending
    /// parts; this projection still derives every child field internally.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn project_validate_report_invalid_certified_body_with_registered_prepare(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
        registered: &RegisteredPrepareInvalidBodyReportCapability,
    ) -> Option<Self> {
        let (
            AdapterEffect::ValidateBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::ReportInvalidCertifiedBody {
                subject: successor_subject,
                certificate,
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if !registered.exactly_matches_report(successor)
            || predecessor_tag.height() != certificate.round.height
            || predecessor_tag.view() != certificate.round.view
            || certificate.phase != wire::GlobalPhase::Prepare
            || certificate.round != *predecessor_round
            || certificate.proposal_round != *predecessor_round
            || certificate.subject != *predecessor_subject
            || successor_subject != predecessor_subject
            || certificate.execution_commitment.validate().is_err()
            || !self.validate_exact(predecessor)
        {
            return None;
        }
        let inherited = self.candidate_statement?;
        if inherited.phase.is_some()
            || inherited.execution_commitment.is_some()
            || inherited.context_id != certificate.round.context_id
            || inherited.round != certificate.round
            || inherited.proposal_round != certificate.proposal_round
            || inherited.subject != Some(certificate.subject)
        {
            return None;
        }
        let registered_carrier = RuntimeCandidateSemanticStatement::new(
            certificate.round,
            certificate.proposal_round,
            Some(certificate.subject),
            Some(wire::GlobalPhase::Prepare),
            Some(certificate.execution_commitment),
        );
        if production_adapter_effect_candidate_binding(successor, Some(&registered_carrier))
            .ok()?
            .is_some()
        {
            return None;
        }
        let successor_binding =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, None);
        successor_binding
            .validate_exact(successor)
            .then_some(successor_binding)
    }
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
include!("v2_runtime_effect_ownership_core_impl.rs");
/// Canonical exact identity bytes for every field of one production effect.
///
/// These bytes are internal evidence only. They are never serialized as a wire
/// field and do not introduce a runtime configuration surface.
pub(crate) fn production_adapter_effect_semantic_identity(effect: &AdapterEffect) -> Vec<u8> {
    let mut identity = Vec::new();
    identity.extend_from_slice(b"iroha:sumeragi:v2:adapter-effect-semantic:v2");
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
            protected_lock,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &certificate.encode());
            append_optional_runtime_identity_bytes(
                &mut identity,
                protected_lock.as_ref().map(norito::codec::Encode::encode),
            );
        }
        AdapterEffect::ReportEquivocation { evidence } => {
            identity.push(match evidence.kind() {
                super::v2_core::EquivocationKind::Vote => 1,
                super::v2_core::EquivocationKind::Timeout => 2,
                super::v2_core::EquivocationKind::Proposal => 3,
            });
            let (first, second) = evidence.signed_artifact_pair();
            append_runtime_identity_field(&mut identity, &first);
            append_runtime_identity_field(&mut identity, &second);
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
    assignments: Vec<RuntimeEffectOwnerAssignment>,
) -> Result<Vec<RuntimeEffectOwnership>, String> {
    if effects.is_empty()
        || effects.len() != assignments.len()
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
        .zip(assignments)
        .enumerate()
        .map(|(index, (effect, assignment))| {
            let effect_position = u8::try_from(index + 1)
                .map_err(|_| "Sumeragi v2 effect position is not representable".to_owned())?;
            let effect_semantic_identity = production_adapter_effect_semantic_identity(effect);
            let candidate = production_adapter_effect_candidate_binding(effect, None)?;
            if candidate.is_some() {
                candidate_position = candidate_position
                    .checked_add(1)
                    .ok_or_else(|| "Sumeragi v2 candidate position overflowed".to_owned())?;
            }
            RuntimeEffectOwnership::new_bound(
                assignment.owner,
                assignment.causality,
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
include!("v2_runtime_effect_ownership_rebind_impl.rs");
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
    let binding = ownership.binding();
    if !binding.validate_exact(ownership.owner(), ownership.causality()) {
        return Err("Sumeragi v2 effect carried an invalid candidate binding".to_owned());
    }
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
/// Exact physical FIFO occurrence granted one retryable fence-predecessor
/// attempt. Causal siblings may share a logical lifecycle owner, so the
/// admission ordinal and immutable command identity are both required.
#[derive(Clone, Debug)]
struct RuntimeQueueOccurrenceOwner {
    source_identity: Arc<()>,
    admission_ordinal: u128,
    identity: RuntimeCommandIdentityDigest,
    projection_hash: iroha_crypto::Hash,
}
impl PartialEq for RuntimeQueueOccurrenceOwner {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && self.admission_ordinal == other.admission_ordinal
            && self.identity == other.identity
            && self.projection_hash == other.projection_hash
    }
}
impl Eq for RuntimeQueueOccurrenceOwner {}
fn runtime_queue_occurrence_owner_projection_hash(
    owner: &RuntimeQueueOccurrenceOwner,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-queue-occurrence-owner:v1");
    append_runtime_identity_field(
        &mut projection,
        &(Arc::as_ptr(&owner.source_identity) as usize).to_le_bytes(),
    );
    append_runtime_identity_field(&mut projection, &owner.admission_ordinal.to_le_bytes());
    append_runtime_identity_field(&mut projection, owner.identity.projection_hash.as_ref());
    iroha_crypto::Hash::new(projection)
}
impl RuntimeQueueOccurrenceOwner {
    fn from_queued<C: ExactRuntimeCommandIdentity>(
        source_identity: &Arc<()>,
        queued: &TaggedCommand<C>,
    ) -> Option<Self> {
        if !queued.validate_admission_identity() {
            return None;
        }
        let mut owner = Self {
            source_identity: Arc::clone(source_identity),
            admission_ordinal: queued.admission_ordinal?,
            identity: queued.identity,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        owner.projection_hash = runtime_queue_occurrence_owner_projection_hash(&owner);
        owner.validate_exact().then_some(owner)
    }
    fn from_candidate(candidate: &RuntimeFifoCandidateOwnership) -> Option<Self> {
        let mut owner = Self {
            source_identity: Arc::clone(&candidate.selection_seal.source_identity),
            admission_ordinal: candidate.admission_ordinal,
            identity: candidate.identity,
            projection_hash: iroha_crypto::Hash::new([]),
        };
        owner.projection_hash = runtime_queue_occurrence_owner_projection_hash(&owner);
        owner.validate_exact().then_some(owner)
    }
    fn validate_exact(&self) -> bool {
        self.admission_ordinal != 0
            && self.identity.validate_exact()
            && self.projection_hash == runtime_queue_occurrence_owner_projection_hash(self)
    }
    fn matches_queued<C: ExactRuntimeCommandIdentity>(
        &self,
        source_identity: &Arc<()>,
        queued: &TaggedCommand<C>,
    ) -> bool {
        Arc::ptr_eq(&self.source_identity, source_identity)
            && queued.cached_queue_occurrence_owner(source_identity) == Some(self)
    }
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
    occurrence_scan_complete: bool,
    occurrence_owners: Vec<RuntimeQueueOccurrenceOwner>,
    occurrence_index: BTreeMap<u128, usize>,
    minimum_lifecycle_ordinal: Option<u128>,
    completion_minimum_lifecycle_ordinal: Option<u128>,
    progress_minimum_lifecycle_ordinal: Option<u128>,
    normal_minimum_lifecycle_ordinal: Option<u128>,
    completion_count: u64,
    progress_count: u64,
    normal_count: u64,
    projection_hash: iroha_crypto::Hash,
}
impl PartialEq for RuntimeQueueOwnershipSnapshot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && self.projection == other.projection
            && self.occurrence_scan_complete == other.occurrence_scan_complete
            && self.occurrence_owners == other.occurrence_owners
            && self.occurrence_index == other.occurrence_index
            && self.minimum_lifecycle_ordinal == other.minimum_lifecycle_ordinal
            && self.completion_minimum_lifecycle_ordinal
                == other.completion_minimum_lifecycle_ordinal
            && self.progress_minimum_lifecycle_ordinal == other.progress_minimum_lifecycle_ordinal
            && self.normal_minimum_lifecycle_ordinal == other.normal_minimum_lifecycle_ordinal
            && self.completion_count == other.completion_count
            && self.progress_count == other.progress_count
            && self.normal_count == other.normal_count
            && self.projection_hash == other.projection_hash
    }
}
impl Eq for RuntimeQueueOwnershipSnapshot {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeQueueSelectionKind {
    Ordinary,
    FenceCompletion,
    FencePredecessor,
    PacemakerProgress,
    PacemakerCertifiedProgress,
}
impl RuntimeQueueSelectionKind {
    const fn code(self) -> u8 {
        match self {
            Self::Ordinary => 1,
            Self::FenceCompletion => 2,
            Self::PacemakerProgress => 3,
            Self::PacemakerCertifiedProgress => 4,
            Self::FencePredecessor => 5,
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
    queue_before_snapshot_hash: iroha_crypto::Hash,
    oldest_lifecycle_ordinal: u128,
    completion_minimum_lifecycle_ordinal: Option<u128>,
    progress_minimum_lifecycle_ordinal: Option<u128>,
    normal_minimum_lifecycle_ordinal: Option<u128>,
    completion_count: u64,
    progress_count: u64,
    normal_count: u64,
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
            && self.queue_before_snapshot_hash == other.queue_before_snapshot_hash
            && self.oldest_lifecycle_ordinal == other.oldest_lifecycle_ordinal
            && self.completion_minimum_lifecycle_ordinal
                == other.completion_minimum_lifecycle_ordinal
            && self.progress_minimum_lifecycle_ordinal == other.progress_minimum_lifecycle_ordinal
            && self.normal_minimum_lifecycle_ordinal == other.normal_minimum_lifecycle_ordinal
            && self.completion_count == other.completion_count
            && self.progress_count == other.progress_count
            && self.normal_count == other.normal_count
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
fn append_runtime_optional_ordinal(projection: &mut Vec<u8>, ordinal: Option<u128>) {
    match ordinal {
        None => projection.push(0),
        Some(ordinal) => {
            projection.push(1);
            append_runtime_identity_field(projection, &ordinal.to_le_bytes());
        }
    }
}
fn append_runtime_optional_u64(projection: &mut Vec<u8>, value: Option<u64>) {
    match value {
        None => projection.push(0),
        Some(value) => {
            projection.push(1);
            append_runtime_identity_u64(projection, value);
        }
    }
}
fn runtime_queue_ownership_snapshot_projection_hash(
    snapshot: &RuntimeQueueOwnershipSnapshot,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-queue-snapshot:v3");
    append_runtime_identity_field(
        &mut projection,
        &(Arc::as_ptr(&snapshot.source_identity) as usize).to_le_bytes(),
    );
    append_runtime_queue_projection(&mut projection, snapshot.projection);
    projection.push(u8::from(snapshot.occurrence_scan_complete));
    append_runtime_identity_u64(
        &mut projection,
        u64::try_from(snapshot.occurrence_owners.len())
            .expect("bounded runtime occurrence count is representable as u64"),
    );
    for owner in &snapshot.occurrence_owners {
        append_runtime_identity_field(&mut projection, owner.projection_hash.as_ref());
    }
    append_runtime_optional_ordinal(&mut projection, snapshot.minimum_lifecycle_ordinal);
    append_runtime_optional_ordinal(
        &mut projection,
        snapshot.completion_minimum_lifecycle_ordinal,
    );
    append_runtime_optional_ordinal(&mut projection, snapshot.progress_minimum_lifecycle_ordinal);
    append_runtime_optional_ordinal(&mut projection, snapshot.normal_minimum_lifecycle_ordinal);
    append_runtime_identity_u64(&mut projection, snapshot.completion_count);
    append_runtime_identity_u64(&mut projection, snapshot.progress_count);
    append_runtime_identity_u64(&mut projection, snapshot.normal_count);
    iroha_crypto::Hash::new(projection)
}
impl RuntimeQueueOwnershipSnapshot {
    fn validate_identity(&self) -> bool {
        let total_count = self
            .completion_count
            .checked_add(self.progress_count)
            .and_then(|count| count.checked_add(self.normal_count));
        let class_minima_are_exact = [
            (
                self.completion_minimum_lifecycle_ordinal,
                self.completion_count,
            ),
            (self.progress_minimum_lifecycle_ordinal, self.progress_count),
            (self.normal_minimum_lifecycle_ordinal, self.normal_count),
        ]
        .into_iter()
        .all(|(minimum, count)| match (minimum, count) {
            (None, 0) => true,
            (Some(ordinal), count) => ordinal != 0 && count != 0,
            _ => false,
        });
        let occurrences_are_exact = self.occurrence_scan_complete
            && u64::try_from(self.occurrence_owners.len()) == Ok(self.projection.len)
            && self.occurrence_index.len() == self.occurrence_owners.len()
            && self.occurrence_owners.iter().all(|owner| {
                owner.admission_ordinal != 0
                    && Arc::ptr_eq(&owner.source_identity, &self.source_identity)
                    && self
                        .occurrence_index
                        .get(&owner.admission_ordinal)
                        .and_then(|index| self.occurrence_owners.get(*index))
                        == Some(owner)
            });
        self.projection_hash == runtime_queue_ownership_snapshot_projection_hash(self)
            && self.projection.len <= self.projection.capacity
            && CommandClass::from_service_code(self.projection.service_cursor).is_some()
            && (self.projection.len != 0 || self.projection.max_service_debt == 0)
            && class_minima_are_exact
            && occurrences_are_exact
            && match (self.minimum_lifecycle_ordinal, total_count) {
                (None, Some(0)) => self.projection.len == 0,
                (Some(ordinal), Some(count)) => ordinal != 0 && count == self.projection.len,
                _ => false,
            }
    }
    fn class_readiness(&self) -> (bool, bool, bool) {
        (
            self.completion_count != 0,
            self.progress_count != 0,
            self.normal_count != 0,
        )
    }
}
fn runtime_queue_occurrence_set_matches_snapshot(
    owners: &[RuntimeQueueOccurrenceOwner],
    snapshot: &RuntimeQueueOwnershipSnapshot,
) -> bool {
    if !u64::try_from(owners.len()).is_ok_and(|len| len <= snapshot.projection.len) {
        return false;
    }
    let mut seen = BTreeSet::new();
    owners.iter().all(|owner| {
        Arc::ptr_eq(&owner.source_identity, &snapshot.source_identity)
            && seen.insert(owner.admission_ordinal)
            && snapshot
                .occurrence_index
                .get(&owner.admission_ordinal)
                .and_then(|index| snapshot.occurrence_owners.get(*index))
                == Some(owner)
    })
}
fn runtime_queue_selection_seal_projection_hash(
    seal: &RuntimeQueueSelectionSeal,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-queue-selection:v3");
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
    append_runtime_identity_field(&mut projection, seal.queue_before_snapshot_hash.as_ref());
    append_runtime_identity_field(
        &mut projection,
        &seal.oldest_lifecycle_ordinal.to_le_bytes(),
    );
    append_runtime_optional_ordinal(&mut projection, seal.completion_minimum_lifecycle_ordinal);
    append_runtime_optional_ordinal(&mut projection, seal.progress_minimum_lifecycle_ordinal);
    append_runtime_optional_ordinal(&mut projection, seal.normal_minimum_lifecycle_ordinal);
    append_runtime_identity_u64(&mut projection, seal.completion_count);
    append_runtime_identity_u64(&mut projection, seal.progress_count);
    append_runtime_identity_u64(&mut projection, seal.normal_count);
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
        let total_count = self
            .completion_count
            .checked_add(self.progress_count)
            .and_then(|count| count.checked_add(self.normal_count));
        let class_minima_are_exact = [
            (
                self.completion_minimum_lifecycle_ordinal,
                self.completion_count,
            ),
            (self.progress_minimum_lifecycle_ordinal, self.progress_count),
            (self.normal_minimum_lifecycle_ordinal, self.normal_count),
        ]
        .into_iter()
        .all(|(minimum, count)| match (minimum, count) {
            (None, 0) => true,
            (Some(ordinal), count) => ordinal != 0 && count != 0,
            _ => false,
        });
        let selected_by_ordinary_cursor = select_bounded_service_class(
            self.queue_before.service_cursor,
            self.completion_count != 0,
            self.progress_count != 0,
            self.normal_count != 0,
        );
        let selected_class_minimum = match CommandClass::from_service_code(self.selected_class) {
            Some(CommandClass::Completion) => self.completion_minimum_lifecycle_ordinal,
            Some(CommandClass::Progress) => self.progress_minimum_lifecycle_ordinal,
            Some(CommandClass::Normal) => self.normal_minimum_lifecycle_ordinal,
            None => None,
        };
        self.projection_hash == runtime_queue_selection_seal_projection_hash(self)
            && self.queue_before.len != 0
            && self.queue_before.len <= self.queue_before.capacity
            && CommandClass::from_service_code(self.queue_before.service_cursor).is_some()
            && self.oldest_lifecycle_ordinal != 0
            && class_minima_are_exact
            && total_count == Some(self.queue_before.len)
            && self.selected_class != SERVICE_CLASS_NONE
            && self.selected_position < self.queue_before.len
            && self.selected_admission_ordinal != 0
            && self.selected_lifecycle_ordinal != 0
            && self.selected_lifecycle_ordinal <= self.selected_admission_ordinal
            && self.selected_eligible_skips <= self.queue_before.max_service_debt
            && self.selected_identity.validate_exact()
            && match self.kind {
                RuntimeQueueSelectionKind::Ordinary => {
                    selected_class_minimum == Some(self.selected_lifecycle_ordinal)
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
                RuntimeQueueSelectionKind::FencePredecessor => {
                    self.cursor_after_removal == self.queue_before.service_cursor
                        && self.max_debt_after_upper_bound == self.queue_before.max_service_debt
                }
                RuntimeQueueSelectionKind::PacemakerProgress => {
                    matches!(
                        CommandClass::from_service_code(self.selected_class),
                        Some(CommandClass::Completion | CommandClass::Progress)
                    ) && self.cursor_after_removal == self.queue_before.service_cursor
                        && self.max_debt_after_upper_bound == self.queue_before.max_service_debt
                }
                RuntimeQueueSelectionKind::PacemakerCertifiedProgress => {
                    self.selected_class == SERVICE_CLASS_PROGRESS
                        && self.selected_identity.kind == RuntimeCommandKind::Authenticated
                        && self.selected_ingress_ownership_hash.is_some()
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
            && Arc::ptr_eq(&self.source_identity, &before.source_identity)
            && Arc::ptr_eq(&self.source_identity, &after.source_identity)
            && self.kind == expected_kind
            && self.queue_before == before.projection
            && self.queue_before_snapshot_hash == before.projection_hash
            && self.oldest_lifecycle_ordinal == before.minimum_lifecycle_ordinal.unwrap_or(0)
            && self.completion_minimum_lifecycle_ordinal
                == before.completion_minimum_lifecycle_ordinal
            && self.progress_minimum_lifecycle_ordinal == before.progress_minimum_lifecycle_ordinal
            && self.normal_minimum_lifecycle_ordinal == before.normal_minimum_lifecycle_ordinal
            && self.completion_count == before.completion_count
            && self.progress_count == before.progress_count
            && self.normal_count == before.normal_count
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
            && RuntimeQueueOccurrenceOwner::from_candidate(candidate).is_some_and(|selected| {
                usize::try_from(self.selected_position)
                    .ok()
                    .and_then(|position| before.occurrence_owners.get(position))
                    == Some(&selected)
                    && if retry_retained {
                        after.projection.len == before.projection.len
                            && after.occurrence_owners == before.occurrence_owners
                    } else {
                        after.projection.len.checked_add(1) == Some(before.projection.len)
                            && before
                                .occurrence_owners
                                .iter()
                                .enumerate()
                                .filter(|(position, _)| {
                                    u64::try_from(*position).ok() != Some(self.selected_position)
                                })
                                .map(|(_, owner)| owner)
                                .eq(after.occurrence_owners.iter())
                    }
            })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeSchedulerArbitrationInputs {
    clocks_armed: bool,
    timeout_due: bool,
    periodic_timer_due: bool,
    fifo_ready: bool,
    completion_ready: bool,
    progress_ready: bool,
    normal_ready: bool,
    fence_completion_bypass: bool,
    fence_dependency_minimum_lifecycle_ordinal: Option<u128>,
    fence_dependency_minimum_admission_ordinal: Option<u128>,
    fence_dependency_minimum_fifo_position: Option<u64>,
    fence_dependency_required_root_class: Option<u8>,
    fence_predecessor_lifecycle_ordinal: Option<u128>,
    fence_predecessor_ownership: Option<RuntimeDeferredLifecycleOwnership>,
    fence_predecessor_ingress_ownership: Option<RuntimeIngressOwnershipEvidence>,
    fence_predecessor_occurrence_ownership: Option<DeferredOccurrenceOwnershipEvidence>,
    fence_retry_blocked_fifo_before: Vec<RuntimeQueueOccurrenceOwner>,
    fence_retry_marker_required: bool,
}
/// Exact source selected for one live scheduler turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeSelectedOwnerKind {
    /// One older adapter-owned Busy-deferred occurrence.
    Deferred,
    /// The exact causally owned signature completion which opens an active
    /// reducer fence for target-relative unserviceable adapter debt.
    FenceCompletion,
    /// The oldest exact unblocked pre-cut FIFO owner which must retire before
    /// a fenced deferred occurrence can reach its signature completion.
    FencePredecessor,
    /// A fence predecessor encountered retryable adapter pressure and kept
    /// its immutable physical queue owner for a later bounded turn.
    FencePredecessorRetryRetained,
    /// One authenticated Progress root, or its trusted Completion successor,
    /// selected while ordinary work is unable to release the pacemaker.
    PacemakerProgress,
    /// A pacemaker Progress occurrence encountered bounded adapter pressure
    /// and retained its exact queue position for retry.
    PacemakerProgressRetryRetained,
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
    /// A FIFO selection owns this exact admitted command.
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
    /// Whether live reducer clocks were armed for this scheduling turn.
    pub(crate) clocks_armed: bool,
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
    /// Exact target-relative unblocked minimum selected by a fence dependency
    /// turn. Its lifecycle, physical admission, and FIFO position form one
    /// exact rank and are present for completion and predecessor branches only.
    fence_dependency_minimum_lifecycle_ordinal: Option<u128>,
    fence_dependency_minimum_admission_ordinal: Option<u128>,
    fence_dependency_minimum_fifo_position: Option<u64>,
    /// Optional causal-root restriction imposed by a typed pacemaker turn.
    /// Ordinary dependency service leaves this unrestricted.
    fence_dependency_required_root_class: Option<u8>,
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
    /// Exact retry-excluded physical FIFO occurrences before this turn.
    fence_retry_blocked_fifo_before: Vec<RuntimeQueueOccurrenceOwner>,
    /// Exact retry-excluded physical FIFO occurrences after this turn.
    fence_retry_blocked_fifo_after: Vec<RuntimeQueueOccurrenceOwner>,
    /// Whether this retry occurred while the same fence remained
    /// unserviceable and therefore had to add the selected exact occurrence.
    fence_retry_marker_required: bool,
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
            Self::FifoRetryRetained => 6,
            Self::FenceCompletion => 7,
            Self::PacemakerProgress => 8,
            Self::PacemakerProgressRetryRetained => 9,
            Self::FencePredecessor => 10,
            Self::FencePredecessorRetryRetained => 11,
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
fn append_runtime_queue_occurrence_owners(
    projection: &mut Vec<u8>,
    owners: &[RuntimeQueueOccurrenceOwner],
) {
    append_runtime_identity_u64(
        projection,
        u64::try_from(owners.len()).expect("bounded runtime occurrence set length fits u64"),
    );
    for owner in owners {
        append_runtime_identity_field(projection, owner.projection_hash.as_ref());
    }
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
    append_runtime_optional_ordinal(
        &mut projection,
        evidence.fence_dependency_minimum_lifecycle_ordinal,
    );
    append_runtime_optional_ordinal(
        &mut projection,
        evidence.fence_dependency_minimum_admission_ordinal,
    );
    append_runtime_optional_u64(
        &mut projection,
        evidence.fence_dependency_minimum_fifo_position,
    );
    match evidence.fence_dependency_required_root_class {
        None => projection.push(0),
        Some(class) => {
            projection.push(1);
            projection.push(class);
        }
    }
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
    append_runtime_queue_occurrence_owners(
        &mut projection,
        &evidence.fence_retry_blocked_fifo_before,
    );
    append_runtime_queue_occurrence_owners(
        &mut projection,
        &evidence.fence_retry_blocked_fifo_after,
    );
    projection.push(u8::from(evidence.fence_retry_marker_required));
    projection.push(u8::from(evidence.clocks_armed));
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
            || self.queue_before_snapshot.class_readiness()
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
        let fence_dependency_target_expected = matches!(
            self.selected,
            RuntimeSelectedOwnerKind::FenceCompletion
                | RuntimeSelectedOwnerKind::FencePredecessor
                | RuntimeSelectedOwnerKind::FencePredecessorRetryRetained
        );
        let fence_dependency_rank_is_exact = match (
            self.fence_dependency_minimum_lifecycle_ordinal,
            self.fence_dependency_minimum_admission_ordinal,
            self.fence_dependency_minimum_fifo_position,
        ) {
            (Some(lifecycle), Some(admission), Some(position)) => {
                fence_dependency_target_expected
                    && lifecycle != 0
                    && admission != 0
                    && lifecycle <= admission
                    && position < self.queue_before.len
            }
            (None, None, None) => !fence_dependency_target_expected,
            _ => false,
        };
        let fence_dependency_root_restriction_is_exact = self
            .fence_dependency_required_root_class
            .is_none_or(|class| {
                fence_dependency_target_expected && class == SERVICE_CLASS_PROGRESS
            });
        let fence_retry_sets_are_exact = runtime_queue_occurrence_set_matches_snapshot(
            &self.fence_retry_blocked_fifo_before,
            &self.queue_before_snapshot,
        ) && runtime_queue_occurrence_set_matches_snapshot(
            &self.fence_retry_blocked_fifo_after,
            &self.queue_after_snapshot,
        );
        let fence_retry_before_by_admission = self
            .fence_retry_blocked_fifo_before
            .iter()
            .map(|owner| (owner.admission_ordinal, owner))
            .collect::<BTreeMap<_, _>>();
        let fence_retry_after_by_admission = self
            .fence_retry_blocked_fifo_after
            .iter()
            .map(|owner| (owner.admission_ordinal, owner))
            .collect::<BTreeMap<_, _>>();
        let fence_retry_clear_is_required =
            self.selected == RuntimeSelectedOwnerKind::FenceCompletion;
        let fence_retry_transition_is_exact = if fence_retry_clear_is_required {
            !self.fence_retry_marker_required && self.fence_retry_blocked_fifo_after.is_empty()
        } else if matches!(
            self.selected,
            RuntimeSelectedOwnerKind::FencePredecessorRetryRetained
                | RuntimeSelectedOwnerKind::PacemakerProgressRetryRetained
        ) {
            let selected_retry_owner = match &self.candidate {
                RuntimeSelectedCandidateOwnership::Exact(candidate) => {
                    RuntimeQueueOccurrenceOwner::from_candidate(candidate)
                }
                RuntimeSelectedCandidateOwnership::NotApplicable
                | RuntimeSelectedCandidateOwnership::ExactDeferred(_) => None,
            };
            let added_selected_owner = selected_retry_owner.is_some_and(|selected| {
                !fence_retry_before_by_admission.contains_key(&selected.admission_ordinal)
                    && fence_retry_after_by_admission.get(&selected.admission_ordinal)
                        == Some(&&selected)
                    && self.fence_retry_blocked_fifo_after.len()
                        == self.fence_retry_blocked_fifo_before.len().saturating_add(1)
                    && fence_retry_before_by_admission
                        .iter()
                        .all(|(ordinal, owner)| {
                            fence_retry_after_by_admission.get(ordinal) == Some(owner)
                        })
            });
            if self.fence_retry_marker_required {
                added_selected_owner
            } else {
                self.selected == RuntimeSelectedOwnerKind::PacemakerProgressRetryRetained
                    && self.fence_retry_blocked_fifo_before == self.fence_retry_blocked_fifo_after
            }
        } else {
            !self.fence_retry_marker_required
                && self.fence_retry_blocked_fifo_before == self.fence_retry_blocked_fifo_after
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
            || (!self.clocks_armed && (self.timeout_due || self.periodic_timer_due))
            || (self.fence_completion_bypass
                != matches!(self.selected, RuntimeSelectedOwnerKind::FenceCompletion))
            || (fence_dependency_target_expected != self.fence_predecessor_ownership.is_some())
            || !fence_dependency_rank_is_exact
            || !fence_dependency_root_restriction_is_exact
            || !fence_retry_sets_are_exact
            || !fence_retry_transition_is_exact
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
            let target_cut = self
                .fence_predecessor_ownership
                .as_ref()
                .map(|ownership| ownership.physical_cut);
            let candidate_precedes_target_cut = match (
                candidate.causal_origin.root_ingress_physical_ownership,
                target_cut,
            ) {
                (None, Some(_)) => true,
                (Some(physical), Some(cut)) => u128::from(physical.source_ordinal) < cut,
                _ => false,
            };
            let exact = self.clocks_armed
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
                && self.fence_dependency_minimum_lifecycle_ordinal
                    == Some(candidate.lifecycle_ordinal)
                && self.fence_dependency_minimum_admission_ordinal
                    == Some(candidate.admission_ordinal)
                && self.fence_dependency_minimum_fifo_position == Some(candidate.fifo_position)
                && self.fence_dependency_required_root_class.is_none()
                && candidate_precedes_target_cut
                // A callback minted as an independent SignatureCompleted root
                // did not inherit the Sign effect and cannot bypass lifecycle
                // order even if its bytes happen to clear the reducer fence.
                && candidate.causal_origin.root_identity.kind
                    != RuntimeCommandKind::SignatureCompleted
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
        if let (
            RuntimeSelectedOwnerKind::FencePredecessor
            | RuntimeSelectedOwnerKind::FencePredecessorRetryRetained,
            RuntimeSelectedCandidateOwnership::Exact(candidate),
        ) = (&self.selected, &self.candidate)
        {
            let retry_retained =
                self.selected == RuntimeSelectedOwnerKind::FencePredecessorRetryRetained;
            let target_cut = self
                .fence_predecessor_ownership
                .as_ref()
                .map(|ownership| ownership.physical_cut);
            let candidate_precedes_target_cut = match (
                candidate.causal_origin.root_ingress_physical_ownership,
                target_cut,
            ) {
                (None, Some(_)) => true,
                (Some(physical), Some(cut)) => u128::from(physical.source_ordinal) < cut,
                _ => false,
            };
            let exact = self.clocks_armed
                && !self.fence_completion_bypass
                && !self.timeout_due
                && !self.periodic_timer_due
                && !self.fifo_ready
                && !self.completion_ready
                && !self.progress_ready
                && !self.normal_ready
                && candidate.identity.validate_exact()
                && candidate.kind == candidate.identity.kind
                && candidate.admission_ordinal != 0
                && candidate.lifecycle_ordinal != 0
                && candidate.lifecycle_ordinal <= candidate.admission_ordinal
                && runtime_fifo_candidate_ingress_is_exact(candidate)
                && candidate.projection_hash == runtime_fifo_candidate_projection_hash(candidate)
                && candidate.causal_origin.validate_exact()
                && candidate.causal_origin.root_lifecycle_ordinal
                    == Some(candidate.lifecycle_ordinal)
                && self.fence_dependency_minimum_lifecycle_ordinal
                    == Some(candidate.lifecycle_ordinal)
                && self.fence_dependency_minimum_admission_ordinal
                    == Some(candidate.admission_ordinal)
                && self.fence_dependency_minimum_fifo_position == Some(candidate.fifo_position)
                && self
                    .fence_dependency_required_root_class
                    .is_none_or(|class| candidate.causal_origin.root_class == class)
                && candidate_precedes_target_cut
                && candidate.class != SERVICE_CLASS_NONE
                && candidate.fifo_position < self.queue_before.len
                && candidate.eligible_skips_before <= self.queue_before.max_service_debt
                && candidate.eligible_skips_after == 0
                && self.queue_before.service_cursor == self.queue_after.service_cursor
                && self.queue_after.max_service_debt <= self.queue_before.max_service_debt
                && self.fifo_owed_before == self.fifo_owed_after
                && if retry_retained {
                    self.queue_after.len == self.queue_before.len
                } else {
                    self.queue_after.len.checked_add(1) == Some(self.queue_before.len)
                }
                && candidate.selection_seal.matches_scheduler_occurrence(
                    candidate,
                    &self.queue_before_snapshot,
                    &self.queue_after_snapshot,
                    RuntimeQueueSelectionKind::FencePredecessor,
                    retry_retained,
                );
            return exact
                .then_some(())
                .ok_or(RuntimeSchedulerEvidenceError::InvalidProjection);
        }
        if let (
            RuntimeSelectedOwnerKind::PacemakerProgress
            | RuntimeSelectedOwnerKind::PacemakerProgressRetryRetained,
            RuntimeSelectedCandidateOwnership::Exact(candidate),
        ) = (&self.selected, &self.candidate)
        {
            let retry_retained =
                self.selected == RuntimeSelectedOwnerKind::PacemakerProgressRetryRetained;
            let selection_kind = candidate.selection_seal.kind;
            let class_and_ingress_are_exact = match selection_kind {
                RuntimeQueueSelectionKind::PacemakerCertifiedProgress => {
                    candidate.class == SERVICE_CLASS_PROGRESS
                        && candidate.kind == RuntimeCommandKind::Authenticated
                        && candidate.ingress_ownership.is_some()
                }
                RuntimeQueueSelectionKind::PacemakerProgress => matches!(
                    CommandClass::from_service_code(candidate.class),
                    Some(CommandClass::Completion | CommandClass::Progress)
                ),
                RuntimeQueueSelectionKind::Ordinary
                | RuntimeQueueSelectionKind::FenceCompletion
                | RuntimeQueueSelectionKind::FencePredecessor => false,
            };
            let exact = self.clocks_armed
                && !self.fence_completion_bypass
                && !self.timeout_due
                && !self.periodic_timer_due
                && !self.fifo_ready
                && !self.completion_ready
                && !self.progress_ready
                && !self.normal_ready
                && candidate.identity.validate_exact()
                && candidate.kind == candidate.identity.kind
                && candidate.admission_ordinal != 0
                && candidate.lifecycle_ordinal != 0
                && candidate.lifecycle_ordinal <= candidate.admission_ordinal
                && runtime_fifo_candidate_ingress_is_exact(candidate)
                && candidate.projection_hash == runtime_fifo_candidate_projection_hash(candidate)
                && candidate.causal_origin.validate_exact()
                && candidate.causal_origin.root_class == SERVICE_CLASS_PROGRESS
                && candidate.causal_origin.root_lifecycle_ordinal
                    == Some(candidate.lifecycle_ordinal)
                && class_and_ingress_are_exact
                && candidate.fifo_position < self.queue_before.len
                && candidate.eligible_skips_before <= self.queue_before.max_service_debt
                && candidate.eligible_skips_after == 0
                && self.queue_before.service_cursor == self.queue_after.service_cursor
                && self.queue_after.max_service_debt <= self.queue_before.max_service_debt
                && self.fifo_owed_before == self.fifo_owed_after
                && if retry_retained {
                    self.queue_after.len == self.queue_before.len
                } else {
                    self.queue_after.len.checked_add(1) == Some(self.queue_before.len)
                }
                && candidate.selection_seal.matches_scheduler_occurrence(
                    candidate,
                    &self.queue_before_snapshot,
                    &self.queue_after_snapshot,
                    selection_kind,
                    retry_retained,
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
        if let (
            RuntimeSelectedOwnerKind::Fifo | RuntimeSelectedOwnerKind::FifoRetryRetained,
            RuntimeSelectedCandidateOwnership::Exact(candidate),
        ) = (&self.selected, &self.candidate)
        {
            let retry_retained = self.selected == RuntimeSelectedOwnerKind::FifoRetryRetained;
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
                && candidate.projection_hash == runtime_fifo_candidate_projection_hash(candidate)
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
                && self.clocks_armed
                && candidate.selection_seal.matches_scheduler_occurrence(
                    candidate,
                    &self.queue_before_snapshot,
                    &self.queue_after_snapshot,
                    RuntimeQueueSelectionKind::Ordinary,
                    retry_retained,
                );
            return exact
                .then_some(())
                .ok_or(RuntimeSchedulerEvidenceError::InvalidProjection);
        }
        match (&self.selected, &self.candidate) {
            (
                RuntimeSelectedOwnerKind::Timeout,
                RuntimeSelectedCandidateOwnership::NotApplicable,
            ) if self.clocks_armed
                && self.queue_before == self.queue_after
                && scheduled == ScheduledWork::Timeout =>
            {
                Ok(())
            }
            (
                RuntimeSelectedOwnerKind::PeriodicTimer,
                RuntimeSelectedCandidateOwnership::NotApplicable,
            ) if self.clocks_armed
                && self.queue_before == self.queue_after
                && scheduled == ScheduledWork::PeriodicTimer =>
            {
                Ok(())
            }
            (RuntimeSelectedOwnerKind::Idle, RuntimeSelectedCandidateOwnership::NotApplicable)
                if self.clocks_armed
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
    /// Queue-private immutable physical occurrence capability, minted once
    /// after the admission ordinal is assigned and then cloned into snapshots.
    queue_occurrence_owner: Option<RuntimeQueueOccurrenceOwner>,
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
            queue_occurrence_owner: None,
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
            queue_occurrence_owner: None,
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
            queue_occurrence_owner: None,
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
    fn mint_queue_occurrence_owner(
        &mut self,
        source_identity: &Arc<()>,
    ) -> Result<(), EnqueueError> {
        if self.queue_occurrence_owner.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        let owner = RuntimeQueueOccurrenceOwner::from_queued(source_identity, self)
            .ok_or(EnqueueError::FailClosed)?;
        self.queue_occurrence_owner = Some(owner);
        Ok(())
    }
    fn cached_queue_occurrence_owner(
        &self,
        source_identity: &Arc<()>,
    ) -> Option<&RuntimeQueueOccurrenceOwner> {
        self.queue_occurrence_owner.as_ref().filter(|owner| {
            Arc::ptr_eq(&owner.source_identity, source_identity)
                && self.admission_ordinal == Some(owner.admission_ordinal)
                && self.identity == owner.identity
        })
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
            && (!self.command.is_certified_fence_escape()
                || (self.class == CommandClass::Progress
                    && self.identity.kind == RuntimeCommandKind::Authenticated
                    && self.ingress_ownership.is_some()))
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
            || total > self.config.ordinary_total_limit()
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
        command: TaggedCommand<C>,
    ) -> Result<(), EnqueueError> {
        self.enqueue_classified_command_with_capacity(command)
    }
    fn enqueue_classified_command_with_capacity(
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
        let certified_fence_escape = command.command.is_certified_fence_escape();
        self.check_capacity_change_inner(
            command.class,
            usize::from(dormant_replacement.is_some()),
            1,
            certified_fence_escape,
        )?;
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
                command.mint_queue_occurrence_owner(&ingress.selection_source_identity)?;
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
        if !matches!(
            producer_stage,
            RuntimeDormantLocalFifoReservation::TIMEOUT_ELAPSED_STAGE
                | RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE
        ) {
            // Other transport-conditional stages cannot own a
            // restart-dormant continuation at all.
            return Err(EnqueueError::FailClosed);
        }
        // These restored producer classes have no latent FIFO charge. Timeout
        // is reconstructed from its durable clock owner. BodyAvailable's
        // pre-store bytes are reacquired through a new exact FetchBody effect,
        // which transfers the persisted logical lifecycle into one fresh
        // Completion position rather than aliasing dormant capacity.
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
    fn certified_fence_escape_credit(&self) -> usize {
        usize::from(
            self.commands
                .iter()
                .any(|queued| queued.command.is_certified_fence_escape()),
        )
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
        self.check_capacity_change_inner(class, dormant_replacements, additions, false)
    }
    fn check_certified_fence_escape_capacity(&self) -> Result<(), EnqueueError> {
        self.check_capacity_change_inner(CommandClass::Progress, 0, 1, true)
    }
    fn check_capacity_change_inner(
        &self,
        class: CommandClass,
        dormant_replacements: usize,
        additions: usize,
        certified_fence_escape: bool,
    ) -> Result<(), EnqueueError> {
        if certified_fence_escape
            && (class != CommandClass::Progress || additions != 1 || dormant_replacements != 0)
        {
            return Err(EnqueueError::FailClosed);
        }
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
        let (normal_before, progress_before, retained_certified) = self.commands.iter().try_fold(
            (0usize, 0usize, false),
            |(normal, progress, certified), queued| {
                let normal = normal
                    .checked_add(usize::from(queued.class == CommandClass::Normal))
                    .ok_or(EnqueueError::FailClosed)?;
                let progress = progress
                    .checked_add(usize::from(queued.class == CommandClass::Progress))
                    .ok_or(EnqueueError::FailClosed)?;
                Ok::<_, EnqueueError>((
                    normal,
                    progress,
                    certified || queued.command.is_certified_fence_escape(),
                ))
            },
        )?;
        let certified_credit = usize::from(retained_certified || certified_fence_escape);
        let ordinary_occupied_after = occupied_after
            .checked_sub(certified_credit)
            .ok_or(EnqueueError::FailClosed)?;
        if ordinary_occupied_after > self.config.ordinary_total_limit() {
            return Err(EnqueueError::Full);
        }
        let normal_after = normal_before
            .checked_add(usize::from(class == CommandClass::Normal) * additions)
            .ok_or(EnqueueError::FailClosed)?;
        let progress_after = progress_before
            .checked_add(usize::from(class == CommandClass::Progress) * additions)
            .ok_or(EnqueueError::FailClosed)?;
        let noncompletion_after = normal_after
            .checked_add(progress_after)
            .ok_or(EnqueueError::FailClosed)?;
        let ordinary_noncompletion_after = noncompletion_after
            .checked_sub(certified_credit)
            .ok_or(EnqueueError::FailClosed)?;
        if normal_after > self.config.normal_limit()
            || ordinary_noncompletion_after > self.config.progress_limit()
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
    fn class_lifecycle_stats(&self, class: CommandClass) -> (Option<u128>, u64) {
        let mut minimum = None;
        let mut count = 0u64;
        for queued in self.commands.iter().filter(|queued| queued.class == class) {
            count = count
                .checked_add(1)
                .expect("bounded runtime class count is representable as u64");
            if let Some(ordinal) = queued.lifecycle_ordinal {
                minimum = Some(minimum.map_or(ordinal, |current: u128| current.min(ordinal)));
            }
        }
        (minimum, count)
    }
    fn ownership_snapshot(&self) -> RuntimeQueueOwnershipSnapshot {
        let occurrence_owners = self
            .commands
            .iter()
            .map(|queued| {
                queued
                    .cached_queue_occurrence_owner(&self.selection_source_identity)
                    .cloned()
            })
            .collect::<Option<Vec<_>>>();
        let occurrence_scan_complete = occurrence_owners.is_some();
        let occurrence_owners = occurrence_owners.unwrap_or_default();
        let occurrence_index = occurrence_owners
            .iter()
            .enumerate()
            .map(|(index, owner)| (owner.admission_ordinal, index))
            .collect::<BTreeMap<_, _>>();
        let occurrence_scan_complete =
            occurrence_scan_complete && occurrence_index.len() == occurrence_owners.len();
        let minimum_lifecycle_ordinal = self
            .commands
            .iter()
            .map(|queued| queued.lifecycle_ordinal)
            .min()
            .unwrap_or(None);
        let (completion_minimum_lifecycle_ordinal, completion_count) =
            self.class_lifecycle_stats(CommandClass::Completion);
        let (progress_minimum_lifecycle_ordinal, progress_count) =
            self.class_lifecycle_stats(CommandClass::Progress);
        let (normal_minimum_lifecycle_ordinal, normal_count) =
            self.class_lifecycle_stats(CommandClass::Normal);
        let mut snapshot = RuntimeQueueOwnershipSnapshot {
            source_identity: Arc::clone(&self.selection_source_identity),
            projection: self.ownership_projection(),
            occurrence_scan_complete,
            occurrence_owners,
            occurrence_index,
            minimum_lifecycle_ordinal,
            completion_minimum_lifecycle_ordinal,
            progress_minimum_lifecycle_ordinal,
            normal_minimum_lifecycle_ordinal,
            completion_count,
            progress_count,
            normal_count,
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
            RuntimeQueueSelectionKind::FenceCompletion
            | RuntimeQueueSelectionKind::FencePredecessor
            | RuntimeQueueSelectionKind::PacemakerProgress
            | RuntimeQueueSelectionKind::PacemakerCertifiedProgress => {
                queue_before.projection.max_service_debt
            }
        };
        let mut seal = RuntimeQueueSelectionSeal {
            source_identity: Arc::clone(&self.selection_source_identity),
            scheduler_handoff_claimed: Arc::new(AtomicBool::new(false)),
            kind,
            queue_before: queue_before.projection,
            queue_before_snapshot_hash: queue_before.projection_hash,
            oldest_lifecycle_ordinal,
            completion_minimum_lifecycle_ordinal: queue_before.completion_minimum_lifecycle_ordinal,
            progress_minimum_lifecycle_ordinal: queue_before.progress_minimum_lifecycle_ordinal,
            normal_minimum_lifecycle_ordinal: queue_before.normal_minimum_lifecycle_ordinal,
            completion_count: queue_before.completion_count,
            progress_count: queue_before.progress_count,
            normal_count: queue_before.normal_count,
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
        for reservation in &self.dormant_local_fifo_reservations {
            if reservation.admission_ordinal == 0
                || !self
                    .lifecycle_ordinals
                    .recognizes_minted(reservation.admission_ordinal)
                    .map_err(|_| EnqueueError::FailClosed)?
            {
                return Err(EnqueueError::FailClosed);
            }
        }
        // Dormant replay reservations are passive capacity claims, not
        // runnable FIFO occurrences. Treating one as the dependency minimum
        // would let an unmaterialized restart stage permanently suppress the
        // exact completion which opens the reducer fence.
        Ok(command_minimum)
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
        excluded_occurrences: &[RuntimeQueueOccurrenceOwner],
    ) -> Result<Option<u128>, EnqueueError> {
        if physical_cut == 0 {
            return Err(EnqueueError::FailClosed);
        }
        let mut excluded_by_admission = BTreeMap::new();
        for owner in excluded_occurrences {
            if !owner.validate_exact()
                || !Arc::ptr_eq(&owner.source_identity, &self.selection_source_identity)
                || excluded_by_admission
                    .insert(owner.admission_ordinal, owner)
                    .is_some()
            {
                return Err(EnqueueError::FailClosed);
            }
        }
        let command_minimum = self.commands.iter().try_fold(
            None,
            |minimum, queued| -> Result<Option<u128>, EnqueueError> {
                let ordinal = queued.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
                if !queued.validate_cached_admission_identity()
                    || queued
                        .cached_queue_occurrence_owner(&self.selection_source_identity)
                        .is_none()
                    || !queued.causal_origin.validate_exact()
                    || queued.causal_origin.root_lifecycle_ordinal != Some(ordinal)
                {
                    return Err(EnqueueError::FailClosed);
                }
                if let Some(excluded) = queued
                    .admission_ordinal
                    .and_then(|ordinal| excluded_by_admission.get(&ordinal))
                {
                    if !excluded.matches_queued(&self.selection_source_identity, queued) {
                        return Err(EnqueueError::FailClosed);
                    }
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
        for reservation in &self.dormant_local_fifo_reservations {
            if reservation.admission_ordinal == 0
                || !self
                    .lifecycle_ordinals
                    .recognizes_minted(reservation.admission_ordinal)
                    .map_err(|_| EnqueueError::FailClosed)?
            {
                return Err(EnqueueError::FailClosed);
            }
        }
        // Dormant replay reservations are validated above but remain passive
        // until materialized as an actual queue occurrence.
        Ok(command_minimum)
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
    fn contains_queue_occurrence_owner(
        &self,
        owner: &RuntimeQueueOccurrenceOwner,
    ) -> Result<bool, EnqueueError> {
        if !owner.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        for queued in &self.commands {
            if queued.admission_ordinal == Some(owner.admission_ordinal) {
                if !owner.matches_queued(&self.selection_source_identity, queued) {
                    return Err(EnqueueError::FailClosed);
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn class_readiness(&self) -> (bool, bool, bool) {
        let class_ready = |class| self.commands.iter().any(|queued| queued.class == class);
        (
            class_ready(CommandClass::Completion),
            class_ready(CommandClass::Progress),
            class_ready(CommandClass::Normal),
        )
    }
    fn minimum_lifecycle_for_class(&self, class: CommandClass) -> Option<u128> {
        self.commands
            .iter()
            .filter(|queued| queued.class == class)
            .filter_map(|queued| queued.lifecycle_ordinal)
            .min()
    }
    /// Remove the exact target-relative FIFO minimum among the matching fence
    /// completion and every runnable predecessor at the same lifecycle rank.
    ///
    /// Causal siblings may share a lifecycle ordinal. Selection therefore
    /// compares their immutable physical admission ordinal and FIFO position
    /// before deciding whether the next owner is the completion or an ordinary
    /// predecessor. Class cursor and service debt remain unchanged.
    fn pop_fence_dependency_with_ownership(
        &mut self,
        lifecycle_ordinal: u128,
        physical_cut: u128,
        mut matches_fence_completion: impl FnMut(&TaggedCommand<C>) -> bool,
        mut is_unblocked_predecessor: impl FnMut(&TaggedCommand<C>) -> bool,
    ) -> Result<Option<(TaggedCommand<C>, RuntimeFifoCandidateOwnership, bool)>, EnqueueError> {
        if lifecycle_ordinal == 0 || physical_cut == 0 {
            return Err(EnqueueError::FailClosed);
        }
        // Validate the complete retained set before an exceptional dependency
        // selection may bypass the ordinary class cursor.
        let _ = self.oldest_lifecycle_ordinal()?;
        let queue_before = self.ownership_snapshot();
        let mut selected_index = None;
        let mut selected_key = None;
        let mut selected_is_completion = false;
        for (index, queued) in self.commands.iter().enumerate() {
            if !queued.validate_cached_admission_identity()
                || queued
                    .cached_queue_occurrence_owner(&self.selection_source_identity)
                    .is_none()
            {
                return Err(EnqueueError::FailClosed);
            }
            let queued_lifecycle = queued.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
            if queued_lifecycle != lifecycle_ordinal {
                continue;
            }
            let before_physical_cut = match (
                queued.causal_origin.root_ingress_physical_ownership,
                queued.ingress_ownership.as_ref(),
            ) {
                (Some(root), Some(ownership)) => {
                    if ownership.contains_physical_carrier(root) != Ok(true) {
                        return Err(EnqueueError::FailClosed);
                    }
                    u128::from(root.source_ordinal) < physical_cut
                }
                (Some(root), None) => u128::from(root.source_ordinal) < physical_cut,
                (None, None) => true,
                (None, Some(_)) => return Err(EnqueueError::FailClosed),
            };
            if !before_physical_cut {
                continue;
            }
            let is_completion = queued.class == CommandClass::Completion
                && queued.identity.kind == RuntimeCommandKind::SignatureCompleted
                && queued.ingress_ownership.is_none()
                && queued.causal_origin.root_identity.kind
                    != RuntimeCommandKind::SignatureCompleted
                && matches_fence_completion(queued);
            let is_predecessor = !is_completion && is_unblocked_predecessor(queued);
            if !is_completion && !is_predecessor {
                continue;
            }
            let admission_ordinal = queued.admission_ordinal.ok_or(EnqueueError::FailClosed)?;
            let key = (admission_ordinal, index);
            if selected_key.is_none_or(|current| key < current) {
                selected_key = Some(key);
                selected_index = Some(index);
                selected_is_completion = is_completion;
            }
        }
        let Some(index) = selected_index else {
            return Ok(None);
        };
        let selected = self
            .commands
            .get(index)
            .expect("selected fence dependency remains present");
        let admission_ordinal = selected.admission_ordinal.ok_or(EnqueueError::FailClosed)?;
        let selected_lifecycle_ordinal =
            selected.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
        let identity = selected.identity;
        let ingress_exact = match identity.kind {
            RuntimeCommandKind::Authenticated => selected.ingress_ownership.is_some(),
            _ => selected.ingress_ownership.is_none(),
        };
        if !selected.identity_deep_validated
            || !identity.validate_exact()
            || !ingress_exact
            || !selected.causal_origin.validate_exact()
            || selected.causal_origin.root_lifecycle_ordinal != Some(selected_lifecycle_ordinal)
            || (selected_is_completion
                && (identity.kind != RuntimeCommandKind::SignatureCompleted
                    || selected.class != CommandClass::Completion
                    || selected.causal_origin.root_identity.kind
                        == RuntimeCommandKind::SignatureCompleted))
        {
            return Err(EnqueueError::FailClosed);
        }
        let fifo_position =
            u64::try_from(index).expect("bounded runtime FIFO position is representable as u64");
        let selection_seal = self.mint_selection_seal(
            if selected_is_completion {
                RuntimeQueueSelectionKind::FenceCompletion
            } else {
                RuntimeQueueSelectionKind::FencePredecessor
            },
            &queue_before,
            selected.class.service_code(),
            fifo_position,
            admission_ordinal,
            selected_lifecycle_ordinal,
            selected.eligible_skips,
            identity,
            selected.tag,
            selected.causal_origin.projection_hash,
            selected
                .ingress_ownership
                .as_ref()
                .map(|ownership| ownership.projection_hash),
            queue_before.projection.service_cursor,
        )?;
        let mut candidate = RuntimeFifoCandidateOwnership {
            kind: identity.kind,
            identity,
            class: selected.class.service_code(),
            tag: selected.tag,
            admission_ordinal,
            lifecycle_ordinal: selected_lifecycle_ordinal,
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
        let command = self
            .commands
            .remove(index)
            .expect("selected fence dependency remains present");
        debug_assert_eq!(
            queue_before.projection.len,
            self.ownership_projection().len + 1
        );
        Ok(Some((command, candidate, selected_is_completion)))
    }
    /// Remove one exact authenticated certified fence escape first, otherwise
    /// the oldest Progress root or one of its trusted Completion descendants,
    /// without advancing ordinary class debt.
    ///
    /// This is the narrow control escape used while an older ordinary
    /// producer or effect batch is backpressured. Eligibility comes only from
    /// the deeply validated frozen causal root; raw command bytes and caller
    /// assertions cannot promote Normal work into this path.
    fn pop_pacemaker_progress_with_ownership(
        &mut self,
        mut is_runnable: impl FnMut(&TaggedCommand<C>) -> bool,
        mut is_certified_fence_escape: impl FnMut(&C) -> bool,
    ) -> Result<Option<(TaggedCommand<C>, RuntimeFifoCandidateOwnership)>, EnqueueError> {
        let _ = self.oldest_lifecycle_ordinal()?;
        let queue_before = self.ownership_snapshot();
        let selected = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, queued)| {
                let eligible = matches!(
                    queued.class,
                    CommandClass::Completion | CommandClass::Progress
                ) && queued.causal_origin.root_class == SERVICE_CLASS_PROGRESS
                    && is_runnable(queued);
                eligible.then(|| {
                    let certified = queued.class == CommandClass::Progress
                        && queued.identity.kind == RuntimeCommandKind::Authenticated
                        && queued.ingress_ownership.is_some()
                        && is_certified_fence_escape(&queued.command);
                    (index, queued, certified)
                })
            })
            .min_by_key(|(index, queued, certified)| {
                (
                    !*certified,
                    queued.lifecycle_ordinal.unwrap_or(u128::MAX),
                    queued.admission_ordinal.unwrap_or(u128::MAX),
                    *index,
                )
            });
        let Some((index, _, certified_fence_escape)) = selected else {
            return Ok(None);
        };
        let selected = self
            .commands
            .get(index)
            .expect("selected pacemaker progress remains present");
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
            || selected.causal_origin.root_class != SERVICE_CLASS_PROGRESS
            || selected.causal_origin.root_lifecycle_ordinal != Some(lifecycle_ordinal)
        {
            return Err(EnqueueError::FailClosed);
        }
        let fifo_position =
            u64::try_from(index).expect("bounded runtime FIFO position is representable as u64");
        let selection_kind = if certified_fence_escape {
            RuntimeQueueSelectionKind::PacemakerCertifiedProgress
        } else {
            RuntimeQueueSelectionKind::PacemakerProgress
        };
        let selection_seal = self.mint_selection_seal(
            selection_kind,
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
        let command = self
            .commands
            .remove(index)
            .expect("selected pacemaker progress remains present");
        Ok(Some((command, candidate)))
    }
    /// Return exact physical occurrences of queued commands which the driver
    /// proves cannot cross its active reducer fence. Causal siblings share a
    /// logical lifecycle, so excluding only the root owner could accidentally
    /// hide the matching completion which releases the fence.
    fn fence_blocked_occurrence_owners(
        &self,
        mut is_blocked: impl FnMut(&TaggedCommand<C>) -> bool,
    ) -> Result<Vec<RuntimeQueueOccurrenceOwner>, EnqueueError> {
        // Validate the complete queue before any owner can be excluded from a
        // lifecycle comparison, including entries which do not match.
        let _ = self.oldest_lifecycle_ordinal()?;
        self.commands
            .iter()
            .filter(|queued| is_blocked(queued))
            .map(|queued| {
                queued
                    .cached_queue_occurrence_owner(&self.selection_source_identity)
                    .cloned()
                    .ok_or(EnqueueError::FailClosed)
            })
            .collect()
    }
    /// Return the lifecycle owner of the exact command ordinary class
    /// rotation would select and the adapter's exact fence state for it.
    ///
    /// This does not skip the owner or mutate service debt. The runtime may
    /// report it not ready while an exact earlier signer or deferred wrapper
    /// owns the fence; its causal completion releases that fence.
    fn ordinary_candidate_owner_and_fence_state(
        &self,
        mut fence_state: impl FnMut(&TaggedCommand<C>) -> (bool, bool),
    ) -> Result<Option<(RuntimeLifecycleOwner, bool, bool)>, EnqueueError> {
        if self.oldest_lifecycle_ordinal()?.is_none() {
            return Ok(None);
        }
        let (completion_ready, progress_ready, normal_ready) = self.class_readiness();
        let selection = select_bounded_service_class(
            self.next_class.service_code(),
            completion_ready,
            progress_ready,
            normal_ready,
        );
        if selection.selected == SERVICE_CLASS_NONE {
            return Ok(None);
        }
        let class =
            CommandClass::from_service_code(selection.selected).ok_or(EnqueueError::FailClosed)?;
        let oldest_class_lifecycle_ordinal = self
            .minimum_lifecycle_for_class(class)
            .ok_or(EnqueueError::FailClosed)?;
        let selected = self
            .commands
            .iter()
            .find(|queued| {
                queued.class == class
                    && queued.lifecycle_ordinal == Some(oldest_class_lifecycle_ordinal)
            })
            .ok_or(EnqueueError::FailClosed)?;
        if !selected.validate_admission_identity() {
            return Err(EnqueueError::FailClosed);
        }
        let (blocked, deferred_alias) = fence_state(selected);
        Ok(Some((selected.lifecycle_owner()?, blocked, deferred_alias)))
    }
    fn pop_next_with_ownership(
        &mut self,
    ) -> Result<Option<(TaggedCommand<C>, RuntimeFifoCandidateOwnership)>, EnqueueError>
    where
        C: ExactRuntimeCommandIdentity,
    {
        let queue_before = self.ownership_snapshot();
        let cursor_before = self.next_class.service_code();
        if self.oldest_lifecycle_ordinal()?.is_none() {
            return Ok(None);
        }
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
        let oldest_class_lifecycle_ordinal = self
            .minimum_lifecycle_for_class(class)
            .ok_or(EnqueueError::FailClosed)?;
        let Some(index) = self.commands.iter().position(|queued| {
            queued.class == class
                && queued.lifecycle_ordinal == Some(oldest_class_lifecycle_ordinal)
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
            let skipped_minimum = self.minimum_lifecycle_for_class(skipped_class);
            if self
                .commands
                .iter()
                .find(|queued| {
                    queued.class == skipped_class && queued.lifecycle_ordinal == skipped_minimum
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
            let skipped_minimum = self.minimum_lifecycle_for_class(skipped_class);
            if let Some(oldest) = self.commands.iter_mut().find(|queued| {
                queued.class == skipped_class && queued.lifecycle_ordinal == skipped_minimum
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
        let candidate_occurrence = RuntimeQueueOccurrenceOwner::from_candidate(candidate)
            .ok_or(EnqueueError::FailClosed)?;
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
            || command.cached_queue_occurrence_owner(&self.selection_source_identity)
                != Some(&candidate_occurrence)
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
        let ordinary_occupied = self
            .occupied_with_dormant_reservations()
            .unwrap_or(usize::MAX)
            .saturating_sub(self.certified_fence_escape_credit());
        self.config
            .ordinary_total_limit()
            .saturating_sub(ordinary_occupied)
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
            CommandClass::Progress => self
                .config
                .progress_limit()
                .saturating_add(self.certified_fence_escape_credit()),
            CommandClass::Completion => self
                .config
                .ordinary_total_limit()
                .saturating_add(self.certified_fence_escape_credit()),
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
    /// Configured class boundary used for diagnostics. Cross-class occupancy
    /// can reduce immediate headroom; Progress and Completion include the one
    /// retained certified credit while it exists.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RestoredProducerRetirement {
    causal_lifecycle_key: iroha_crypto::Hash,
    admission_ordinal: u128,
    producer_stage: u8,
}
impl RestoredProducerRetirement {
    fn from_body_owner(
        causal_origin: &RuntimeCandidateCausalOrigin,
        lifecycle_ordinal: Option<u128>,
        producer_stage: Option<u8>,
    ) -> Result<Option<Self>, EnqueueError> {
        let Some(producer_stage) = producer_stage else {
            return Ok(None);
        };
        let admission_ordinal = lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
        let causal_lifecycle_key = causal_origin
            .restored_producer_lifecycle_key
            .ok_or(EnqueueError::FailClosed)?;
        if producer_stage != RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE
            || admission_ordinal == 0
            || !causal_origin.validate_exact()
            || causal_origin.root_lifecycle_ordinal != Some(admission_ordinal)
            || causal_origin.lifecycle_key != causal_lifecycle_key
        {
            return Err(EnqueueError::FailClosed);
        }
        Ok(Some(Self {
            causal_lifecycle_key,
            admission_ordinal,
            producer_stage,
        }))
    }
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
    fn coalesced_with_lifecycle_owner(
        tag: EventTag,
        manifest: wire::PayloadManifest,
        owner: RuntimeLifecycleOwner,
        candidate_semantic_statement: Option<RuntimeCandidateSemanticStatement>,
    ) -> Result<Self, EnqueueError> {
        if !owner.validate_exact()
            || candidate_semantic_statement.is_some_and(|statement| !statement.validate_exact())
        {
            return Err(EnqueueError::FailClosed);
        }
        let mut reservation = Self::coalesced(tag, manifest);
        reservation.lifecycle_ordinal = Some(owner.lifecycle_ordinal());
        reservation.causal_origin = Some(owner.causal_origin().clone());
        reservation.candidate_semantic_statement = candidate_semantic_statement;
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
    /// Retag this exact unpublished token while preserving every physical and
    /// logical ownership field.
    pub(crate) fn rebind_consumer_if_exact(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> bool {
        if self.tag != previous || self.manifest != *manifest {
            return false;
        }
        self.tag = rebound;
        true
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
            AdapterCommand::Authenticated(_)
            | AdapterCommand::LocalProposalReady { .. }
            | AdapterCommand::BodyAvailable { .. }
            | AdapterCommand::BodyStored { .. }
            | AdapterCommand::SignatureCompleted(_)
            | AdapterCommand::ApplicationCompleted(_) => false,
        }
    }
    fn merge(self, other: Self) -> Self {
        Self {
            body_available: self.body_available.saturating_add(other.body_available),
            body_stored: self.body_stored.saturating_add(other.body_stored),
            local_proposal: self.local_proposal.saturating_add(other.local_proposal),
        }
    }
    fn validate_unique(self) -> Result<Self, String> {
        if self.body_available > 1 || self.body_stored > 1 || self.local_proposal > 1 {
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
    fn body_pipeline_completion_evidence(&self) -> Option<BodyPipelineCompletionEvidence> {
        match self {
            Self::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            } => Some(BodyPipelineCompletionEvidence::LocalProposalReady {
                manifest: manifest.clone(),
                durable_receipt: durable_receipt.clone(),
                validated_receipt: validated_receipt.clone(),
            }),
            Self::BodyAvailable { .. } => None,
            Self::BodyStored {
                round,
                subject,
                receipt,
            } => Some(BodyPipelineCompletionEvidence::BodyStored {
                round: *round,
                subject: *subject,
                receipt: receipt.clone(),
            }),
            Self::Authenticated(_)
            | Self::SignatureCompleted(_)
            | Self::ApplicationCompleted(_) => None,
        }
    }
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
impl exact_runtime_command_identity_sealed::Sealed for AdapterCommand {}
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
    fn is_certified_fence_escape(&self) -> bool {
        matches!(self, Self::Authenticated(message) if message.is_certified_fence_escape())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RuntimeBodyCompletionStorageTarget {
    Queued(u128),
    Reserved(u128),
    Deferred(u128),
}
#[derive(Clone, Debug)]
struct RuntimeBodyCompletionOwnershipPlan {
    tag: EventTag,
    candidate: BodyPipelineCompletionEvidence,
    retained_owner: RuntimeLifecycleOwner,
    retained_statement: Option<RuntimeCandidateSemanticStatement>,
    target: RuntimeBodyCompletionStorageTarget,
    replacement_statement: Option<RuntimeCandidateSemanticStatement>,
}
impl RuntimeBodyCompletionOwnershipPlan {
    fn effective_statement(&self) -> Option<RuntimeCandidateSemanticStatement> {
        self.replacement_statement.or(self.retained_statement)
    }
    /// Rebind the reviewed retry under the immutable terminal owner without
    /// committing an authority refinement.
    ///
    /// The caller uses this sidecar for the complete positional refinement
    /// gate. Only after that gate succeeds may the serialized runtime commit
    /// `replacement_statement` to the queued or Busy terminal.
    fn adopt_effect_ownership(
        &self,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<RuntimeEffectOwnership, String> {
        if !self.retained_owner.validate_exact()
            || !incoming.validate_exact()
            || !incoming.exactly_binds_adapter_effect(effect)
        {
            return Err(
                "Sumeragi v2 body terminal retry omitted exact effect ownership".to_owned(),
            );
        }
        let incoming_binding = incoming.binding();
        let retained_statement = self.effective_statement().ok_or_else(|| {
            "Sumeragi v2 body terminal retry omitted its retained authority statement".to_owned()
        })?;
        let candidate =
            production_adapter_effect_candidate_binding(effect, Some(&retained_statement))?
                .ok_or_else(|| {
                    "Sumeragi v2 body terminal retry rebound to a non-candidate effect".to_owned()
                })?;
        if candidate.statement != Some(retained_statement)
            || candidate.kind != incoming_binding.candidate_kind
        {
            return Err(
                "Sumeragi v2 body terminal retry changed its retained candidate statement"
                    .to_owned(),
            );
        }
        RuntimeEffectOwnership::new_bound(
            self.retained_owner.clone(),
            incoming.causality(),
            production_adapter_effect_kind(effect),
            &production_adapter_effect_semantic_identity(effect),
            Some(&candidate),
            incoming_binding.effect_position,
            incoming_binding.effect_count,
            incoming_binding.candidate_position,
            incoming_binding.candidate_count,
        )
        .map_err(|_| {
            "Sumeragi v2 body terminal retry could not retain its incumbent owner".to_owned()
        })
    }
}
#[derive(Clone, Debug, Default)]
struct RuntimePreparedBodyCompletionRefinements {
    ingress: Vec<(
        RuntimeBodyCompletionStorageTarget,
        RuntimeCandidateSemanticStatement,
    )>,
    deferred: BTreeMap<u128, RuntimeDeferredLifecycleOwnership>,
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
    ) -> Result<
        Vec<(
            RuntimeLifecycleOwner,
            Option<RuntimeCandidateSemanticStatement>,
            RuntimeBodyCompletionStorageTarget,
        )>,
        EnqueueError,
    > {
        let mut owners = self
            .commands
            .iter()
            .filter(|queued| {
                queued.tag == tag
                    && queued.command.body_pipeline_completion_ownership(candidate) == Some(true)
            })
            .map(|queued| {
                Ok((
                    queued.lifecycle_owner()?,
                    queued.candidate_semantic_statement,
                    RuntimeBodyCompletionStorageTarget::Queued(
                        queued.admission_ordinal.ok_or(EnqueueError::FailClosed)?,
                    ),
                ))
            })
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
            owners.push((
                reservation
                    .lifecycle_owner()
                    .ok_or(EnqueueError::FailClosed)?,
                reservation.candidate_semantic_statement,
                RuntimeBodyCompletionStorageTarget::Reserved(
                    reservation
                        .admission_ordinal
                        .ok_or(EnqueueError::FailClosed)?,
                ),
            ));
        }
        Ok(owners)
    }
    fn exact_body_pipeline_completion_refinement_matches(
        &self,
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
        target: RuntimeBodyCompletionStorageTarget,
        owner: &RuntimeLifecycleOwner,
        incumbent: RuntimeCandidateSemanticStatement,
    ) -> Result<bool, EnqueueError> {
        let retained = self.exact_body_pipeline_completion_owners(tag, candidate)?;
        Ok(matches!(
            retained.as_slice(),
            [(retained_owner, Some(retained_statement), retained_target)]
                if retained_owner == owner
                    && *retained_statement == incumbent
                    && *retained_target == target
        ))
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
        if wire_payload_is_certified_fence_escape(&message.payload) {
            return self.check_certified_fence_escape_capacity();
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
        if wire_payload_is_certified_fence_escape(&message.payload) {
            return self.check_certified_fence_escape_capacity();
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
        if !ingress_ownership.exactly_matches_authenticated(&authenticated) {
            return Err(EnqueueError::FailClosed);
        }
        let certified_fence_escape =
            wire_payload_is_certified_fence_escape(authenticated.payload());
        if certified_fence_escape && class != CommandClass::Progress {
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
        if tagged.command.is_certified_fence_escape() != certified_fence_escape {
            return Err(EnqueueError::FailClosed);
        }
        self.enqueue_classified_command_with_capacity(tagged)?;
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
        let mut admitted = super::fair_v2_ingress_admit_for_test(
            super::InboundBlockMessage::from_authenticated_peer(
                super::message::BlockMessage::V2(message.clone()),
                super::authenticated_peer_for_test(),
            ),
        );
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
        let physical_limit = self
            .config
            .ordinary_total_limit()
            .checked_add(self.certified_fence_escape_credit())
            .ok_or(EnqueueError::FailClosed)?;
        if occupied_after_commit > physical_limit {
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
                // Capacity above was computed after retiring proposals which
                // conflict with this canonical body, so publish that
                // retirement atomically with the replacement token. Leaving
                // those commands live until materialization would transiently
                // own more physical slots than the calculation admitted and
                // could exclude the certified fence escape.
                ingress.discard_proposals_conflicting_with(reservation.manifest());
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
            command.mint_queue_occurrence_owner(&self.selection_source_identity)?;
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
            && reservation.rebind_consumer_if_exact(previous, rebound, manifest)
        {
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
    fn restored_body_available_retirement(
        &self,
        tag: EventTag,
        matches_manifest: impl Fn(&wire::PayloadManifest) -> bool,
    ) -> Result<Option<RestoredProducerRetirement>, EnqueueError> {
        let mut owners = Vec::new();
        if let Some(reservation) = self
            .reserved_body_available
            .as_ref()
            .filter(|reservation| reservation.tag == tag && matches_manifest(&reservation.manifest))
        {
            owners.push(RestoredProducerRetirement::from_body_owner(
                reservation
                    .causal_origin
                    .as_ref()
                    .ok_or(EnqueueError::FailClosed)?,
                reservation.lifecycle_ordinal,
                reservation.restored_producer_stage,
            )?);
        }
        for queued in self.commands.iter().filter(|queued| queued.tag == tag) {
            let AdapterCommand::BodyAvailable { manifest } = &queued.command else {
                continue;
            };
            if !matches_manifest(manifest) {
                continue;
            }
            owners.push(RestoredProducerRetirement::from_body_owner(
                &queued.causal_origin,
                queued.lifecycle_ordinal,
                queued.restored_producer_stage,
            )?);
        }
        match owners.as_slice() {
            [] => Ok(None),
            [owner] => Ok(*owner),
            _ => Err(EnqueueError::DuplicateCompletionOwnership),
        }
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
    remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>,
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
    const BODY_AVAILABLE_STAGE: u8 = 7;
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
    #[cfg_attr(not(test), allow(dead_code))]
    fn completed(effects: Vec<E>) -> Self {
        Self {
            effects,
            deferred_ingress: None,
            deferred_ordinal: None,
            retry_unadmitted: false,
            producer_handoff: None,
            remote_proposal_replay: None,
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
    /// Exact equality identity of one active signer incarnation.
    type SignatureFenceIdentity: Clone + Eq;
    /// Current authoritative reducer tag.
    fn current_tag(&self) -> EventTag;
    /// Whether replay-authenticated safety-WAL state closes local production
    /// for the exact current reducer round. Synthetic drivers have no durable
    /// WAL and retain the open default.
    fn durable_current_round_local_proposal_is_closed(&self) -> bool {
        false
    }
    /// Frozen current-height TimeoutVote slot universe. Synthetic drivers have
    /// no network roster and therefore retain the empty default.
    fn timeout_vote_owner_universe(&self) -> BTreeSet<FairV2IngressLeaderWireSlot> {
        BTreeSet::new()
    }
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
    /// Return whether this deeply authenticated Progress root carries a TC or
    /// CommitQC which may supersede an outstanding local signature fence.
    /// The default is deliberately closed; only a driver which owns the
    /// authenticated command token may opt an exact command into this escape.
    fn certified_progress_bypasses_signature_fence(&self, _command: &Self::Command) -> bool {
        false
    }
    /// Return whether a pending replay or persistence acknowledgement forbids
    /// every pacemaker queue dispatch. The matching asynchronous completion
    /// owns the only legal next reducer transition.
    fn pacemaker_escape_is_parked(&self) -> bool {
        false
    }
    /// Return whether the reducer is specifically waiting for a signature and
    /// has no earlier persistence/replay fence. Only this state authorizes the
    /// exact fence-dependency selector.
    fn signature_fence_is_active(&self) -> bool {
        false
    }
    /// Return the exact active signer owner. Runtime retry exclusions are
    /// scoped to this identity so a successor signer cannot inherit debt from
    /// the fence it replaced.
    fn signature_fence_identity(
        &self,
    ) -> Result<Option<Self::SignatureFenceIdentity>, Self::Error> {
        Ok(None)
    }
    /// Prove that a current-tag monotone terminal consumes an exact async
    /// effect owner rather than dropping an unrelated fresh lifecycle.
    fn owned_terminal_completion_matches_effect(
        &self,
        _tag: EventTag,
        _command: &Self::Command,
        _ownership: &RuntimeEffectOwnership,
    ) -> bool {
        false
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
    /// Consume one authenticated Proposal dispatch origin into the exact
    /// ordinary Fetch ownership emitted by the same reducer macro-step.
    fn bind_remote_proposal_fetch_replay(
        &self,
        _origin: AuthenticatedRemoteProposalDispatchOrigin,
        _effects: &[Self::Effect],
        _ownership: &mut [RuntimeEffectOwnership],
    ) -> Result<Option<AuthenticatedRemoteProposalDispatchOrigin>, ()> {
        Err(())
    }
    /// Return whether this exact Proposal remains the sole current-view Set-B
    /// candidate waiting for periodic fallback to emit its ordinary Fetch.
    fn remote_proposal_fetch_replay_is_dormant(
        &self,
        _origin: &AuthenticatedRemoteProposalDispatchOrigin,
    ) -> bool {
        false
    }
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
    /// The runtime uses this only when target-relative adapter debt is present
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
    /// The runtime uses this proof only to ignore that exact physical queue
    /// occurrence while locating the causal completion or the next runnable
    /// same-rank predecessor. External tasks, producer reservations, timers,
    /// and commands which can terminate before the reducer remain ordered
    /// blockers.
    fn command_is_blocked_by_deferred_fence(
        &self,
        _tag: EventTag,
        _command: &Self::Command,
    ) -> bool {
        false
    }
    /// Return whether this already-authenticated queued command aliases one
    /// exact adapter-owned Busy occurrence and can retire before reaching the
    /// reducer's active signature fence.
    ///
    /// The default is closed. A production driver may opt in only after the
    /// adapter has retained the matching authenticated deferred owner; the
    /// normal dispatch path must still validate and retire that alias.
    fn command_matches_deferred_authenticated_owner(&self, _command: &Self::Command) -> bool {
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
    /// Closed refinement kind for exact effect-to-candidate projection.
    fn effect_refinement_kind(_effect: &Self::Effect) -> u8 {
        RUNTIME_EFFECT_KIND_OPAQUE_TEST
    }
    /// Exact semantic bytes for the complete concrete effect.
    fn effect_semantic_identity(_effect: &Self::Effect) -> Vec<u8> {
        vec![RUNTIME_EFFECT_KIND_OPAQUE_TEST]
    }
    /// Route-neutral candidate kind and semantic bytes, or `None` for a
    /// synchronous/transport/diagnostic effect.
    fn effect_candidate_semantic_identity(_effect: &Self::Effect) -> Option<(u8, Vec<u8>)> {
        None
    }
    /// Bind a candidate to an optional typed causal statement. Synthetic
    /// drivers retain their opaque bytes; the production adapter overrides
    /// this seam to preserve the exact body-pipeline statement.
    fn effect_candidate_semantic_binding(
        &self,
        effect: &Self::Effect,
        _inherited: Option<&RuntimeCandidateSemanticStatement>,
    ) -> Result<Option<RuntimeEffectCandidateSemantic>, String> {
        Ok(
            Self::effect_candidate_semantic_identity(effect).map(|(kind, semantic_identity)| {
                RuntimeEffectCandidateSemantic {
                    kind,
                    semantic_identity,
                    statement: None,
                }
            }),
        )
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
    type SignatureFenceIdentity = (EventTag, super::v2_core::SignableMessage);
    fn current_tag(&self) -> EventTag {
        SumeragiV2Adapter::current_tag(self)
    }
    fn durable_current_round_local_proposal_is_closed(&self) -> bool {
        SumeragiV2Adapter::durable_current_round_local_proposal_is_closed(self)
    }
    fn timeout_vote_owner_universe(&self) -> BTreeSet<FairV2IngressLeaderWireSlot> {
        self.wire_context()
            .roster
            .iter()
            .map(|entry| FairV2IngressLeaderWireSlot {
                semantic_origin: entry.validator.clone(),
                phase: FairV2IngressLeaderWirePhase::TimeoutVote,
                chunk_index: None,
            })
            .collect()
    }
    fn preflight_command_admission(
        &self,
        tag: EventTag,
        command: &Self::Command,
    ) -> RuntimeCommandAdmissionPreflight {
        self.preflight_runtime_command_admission(tag, command)
    }
    fn certified_progress_bypasses_signature_fence(&self, command: &Self::Command) -> bool {
        self.signature_fence_is_active()
            && matches!(
                command,
                AdapterCommand::Authenticated(authenticated)
                    if wire_payload_is_certified_fence_escape(authenticated.payload())
            )
    }
    fn pacemaker_escape_is_parked(&self) -> bool {
        SumeragiV2Adapter::pacemaker_escape_is_parked(self)
    }
    fn signature_fence_is_active(&self) -> bool {
        SumeragiV2Adapter::signature_fence_is_active(self)
    }
    fn signature_fence_identity(
        &self,
    ) -> Result<Option<Self::SignatureFenceIdentity>, Self::Error> {
        Ok(SumeragiV2Adapter::signature_fence_identity(self))
    }
    fn owned_terminal_completion_matches_effect(
        &self,
        tag: EventTag,
        command: &Self::Command,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        match command {
            AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            } => {
                let predecessor = AdapterEffect::ValidateBody {
                    tag,
                    round: manifest.round,
                    subject: manifest.subject,
                };
                let successor = BodyPipelineCompletionEvidence::LocalProposalReady {
                    manifest: manifest.clone(),
                    durable_receipt: durable_receipt.clone(),
                    validated_receipt: validated_receipt.clone(),
                };
                return ownership.exactly_authorizes_body_pipeline_successor(
                    &predecessor,
                    tag,
                    &successor,
                );
            }
            AdapterCommand::Authenticated(_)
            | AdapterCommand::BodyAvailable { .. }
            | AdapterCommand::BodyStored { .. }
            | AdapterCommand::SignatureCompleted(_)
            | AdapterCommand::ApplicationCompleted(_) => return false,
        }
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
        let remote_proposal_replay = match (&tagged.command, &tagged.ingress_ownership) {
            (AdapterCommand::Authenticated(message), Some(ingress))
                if matches!(
                    message.payload(),
                    wire::ConsensusMessageV2Payload::Proposal(_)
                ) =>
            {
                Some(
                    AuthenticatedRemoteProposalDispatchOrigin::new(
                        message.clone(),
                        ingress.clone(),
                    )
                    .ok_or(AdapterError::RuntimeIngressOwnershipViolation)?,
                )
            }
            (AdapterCommand::Authenticated(_), Some(_)) => None,
            (AdapterCommand::Authenticated(_), None) | (_, Some(_)) => {
                return Err(AdapterError::RuntimeIngressOwnershipViolation);
            }
            (_, None) => None,
        };
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
                    if !ownership.exactly_matches_authenticated(&message) {
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
            remote_proposal_replay,
        })
    }
    fn bind_remote_proposal_fetch_replay(
        &self,
        origin: AuthenticatedRemoteProposalDispatchOrigin,
        effects: &[Self::Effect],
        ownership: &mut [RuntimeEffectOwnership],
    ) -> Result<Option<AuthenticatedRemoteProposalDispatchOrigin>, ()> {
        if effects.len() != ownership.len() {
            return Err(());
        }
        let mut fetches = effects.iter().enumerate().filter(|(_, effect)| {
            matches!(
                effect,
                AdapterEffect::FetchBody {
                    certificate: None,
                    ..
                }
            )
        });
        let Some((index, effect)) = fetches.next() else {
            // Set B deliberately waits one retransmission boundary before
            // acquiring an ordinary Proposal body. Retain only that exact
            // current candidate; every ignored, superseded, or certified
            // Proposal terminates without latent ordinary-fetch authority.
            return Ok(self
                .remote_proposal_fetch_replay_is_dormant(&origin)
                .then_some(origin));
        };
        if fetches.next().is_some() || ownership[index].remote_proposal_fetch_replay.is_some() {
            return Err(());
        }
        let pending = ownership[index]
            .exact_pending_adapter_effect_binding(effect)
            .map_err(|_| ())?;
        let replay = origin.bind_exact_fetch(effect, pending).ok_or(())?;
        ownership[index].remote_proposal_fetch_replay = Some(replay);
        Ok(None)
    }
    fn remote_proposal_fetch_replay_is_dormant(
        &self,
        origin: &AuthenticatedRemoteProposalDispatchOrigin,
    ) -> bool {
        origin
            .exact_proposal()
            .is_some_and(|proposal| self.retains_dormant_remote_proposal_fetch(proposal))
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
                remote_proposal_replay: None,
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
                remote_proposal_replay: None,
            }
        })
    }
    fn completion_unblocks_deferred_fence(&self, tag: EventTag, command: &Self::Command) -> bool {
        SumeragiV2Adapter::completion_unblocks_deferred_fence(self, tag, command)
    }
    fn command_is_blocked_by_deferred_fence(&self, tag: EventTag, command: &Self::Command) -> bool {
        SumeragiV2Adapter::command_is_blocked_by_deferred_fence(self, tag, command)
    }
    fn command_matches_deferred_authenticated_owner(&self, command: &Self::Command) -> bool {
        match command {
            AdapterCommand::Authenticated(authenticated) => self
                .deferred_authenticated_message_owner(authenticated.wire_envelope())
                .is_some(),
            AdapterCommand::LocalProposalReady { .. }
            | AdapterCommand::BodyAvailable { .. }
            | AdapterCommand::BodyStored { .. }
            | AdapterCommand::SignatureCompleted(_)
            | AdapterCommand::ApplicationCompleted(_) => false,
        }
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
        &self,
        effect: &Self::Effect,
        inherited: Option<&RuntimeCandidateSemanticStatement>,
    ) -> Result<Option<RuntimeEffectCandidateSemantic>, String> {
        let effective_inherited = match effect {
            AdapterEffect::StoreBody { round, subject, .. }
            | AdapterEffect::ValidateBody { round, subject, .. } => {
                match self
                    .replayed_decision_key()
                    .map_err(|error| error.to_string())?
                {
                    Some((decision_round, proposal_round, decision_subject, commitment))
                        if *round == proposal_round && *subject == decision_subject =>
                    {
                        let decision_statement = RuntimeCandidateSemanticStatement::new(
                            decision_round,
                            proposal_round,
                            Some(decision_subject),
                            Some(wire::GlobalPhase::Commit),
                            Some(commitment),
                        );
                        if inherited.is_some_and(|parent| {
                            parent.commit_refinement_to(decision_statement).is_none()
                        }) {
                            return Err(
                                "Sumeragi v2 durable Decision body recovery conflicted with its causal authority"
                                    .to_owned(),
                            );
                        }
                        Some(decision_statement)
                    }
                    Some(_) | None => inherited.copied(),
                }
            }
            AdapterEffect::Sign { .. }
            | AdapterEffect::Broadcast(_)
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::Apply { .. }
            | AdapterEffect::EnterView { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => inherited.copied(),
        };
        production_adapter_effect_candidate_binding(effect, effective_inherited.as_ref())
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
                protected_lock,
                ..
            } => {
                identity.push(6);
                append_runtime_identity_field(&mut identity, &certificate.round.encode());
                // Fresh-root identity retains the complete authenticated lock
                // statement while excluding interchangeable aggregate carrier
                // bytes. The exact effect identity above retains both.
                append_optional_runtime_qc_statement(&mut identity, protected_lock.as_ref());
            }
            AdapterEffect::ReportEquivocation { evidence } => {
                identity.push(7);
                identity.push(match evidence.kind() {
                    super::v2_core::EquivocationKind::Vote => 1,
                    super::v2_core::EquivocationKind::Timeout => 2,
                    super::v2_core::EquivocationKind::Proposal => 3,
                });
                append_runtime_identity_u64(&mut identity, u64::from(evidence.offender_index()));
                let (first, second) = evidence.canonical_unsigned_statement_pair();
                append_runtime_identity_field(&mut identity, &first);
                append_runtime_identity_field(&mut identity, &second);
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
    owner: RuntimeLifecycleOwner,
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
/// Frozen clock episodes whose immutable physical cuts reject one ingress
/// occurrence.
///
/// Timeout and periodic retransmission have different scheduler authority:
/// the absolute timeout preempts FIFO debt, while a retransmission does not.
/// Keeping the two bits separate prevents restart recovery from treating a
/// periodic cut as timeout authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RuntimeClockReservationBlockers {
    timeout: bool,
    retransmit: bool,
}
/// How one authenticated TimeoutVote participates in the finite producer
/// episode opened by a durable local timeout.
///
/// Restored or already-scheduled owners below the timeout owner are genuine
/// rank descent. Their physical publication may straddle the runner's last
/// receiver snapshot. A first scheduler owner minted after the timeout is
/// finite replenishment: it may increase the collected-vote count, but
/// admission itself is never reported as scheduler progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeTimeoutVoteEpisodeDisposition {
    PreCutDescent,
    RestoredDescent,
    FreshReplenishment,
}
/// Immutable owner retained for one roster source in a timeout-recovery
/// episode.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeTimeoutVoteEpisodeOwner {
    token: FairV2IngressLeaderWireToken,
    carrier_physical_ordinal: u64,
    disposition: RuntimeTimeoutVoteEpisodeDisposition,
}
impl RuntimeTimeoutVoteEpisodeOwner {
    /// Whether two concrete carriers retain the same immutable lifecycle
    /// owner.
    ///
    /// A transport retry may publish the retained token on a later physical
    /// carrier and therefore move from pre-cut to restored descent. Neither
    /// the carrier ordinal nor that derived disposition is owner identity;
    /// the incumbent record remains authoritative for both fields.
    fn same_lifecycle_owner_as(&self, other: &Self) -> bool {
        self.token == other.token
    }
    fn validate_against(&self, timeout_ordinal: u128, physical_cut: u128) -> bool {
        let admission_ordinal = u128::from(self.token.admission_ordinal());
        let carrier_physical_ordinal = u128::from(self.carrier_physical_ordinal);
        self.carrier_physical_ordinal != 0
            && self.token.admission_ordinal() != 0
            && self.token.scheduler_ordinal() != 0
            && self.token.scheduler_ordinal() != timeout_ordinal
            && physical_cut != 0
            && match self.disposition {
                RuntimeTimeoutVoteEpisodeDisposition::PreCutDescent => {
                    self.token.scheduler_ordinal() < timeout_ordinal
                        && self.token.admission_ordinal() == self.carrier_physical_ordinal
                }
                RuntimeTimeoutVoteEpisodeDisposition::RestoredDescent => {
                    self.token.scheduler_ordinal() < timeout_ordinal
                        && self.token.admission_ordinal() < self.carrier_physical_ordinal
                        && admission_ordinal < physical_cut
                        && carrier_physical_ordinal >= physical_cut
                }
                RuntimeTimeoutVoteEpisodeDisposition::FreshReplenishment => {
                    self.token.scheduler_ordinal() > timeout_ordinal
                        && self.token.admission_ordinal() == self.carrier_physical_ordinal
                        && carrier_physical_ordinal >= physical_cut
                }
            }
    }
}
/// Exact source/slot candidate checked before authentication mutates runtime
/// ownership.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeTimeoutVoteEpisodeCandidate {
    slot: FairV2IngressLeaderWireSlot,
    owner: RuntimeTimeoutVoteEpisodeOwner,
}
/// Exact 0→0, 0→1, or 1→1 admission projection for one timeout-vote producer.
#[derive(Clone, Debug, PartialEq, Eq)]
enum RuntimeTimeoutVoteEpisodeAdmissionPlan {
    NonCandidate,
    FirstAdmission {
        candidate: RuntimeTimeoutVoteEpisodeCandidate,
        prospective: BTreeMap<FairV2IngressLeaderWireSlot, RuntimeTimeoutVoteEpisodeOwner>,
    },
    CoalescedRetry {
        candidate: RuntimeTimeoutVoteEpisodeCandidate,
        prospective: BTreeMap<FairV2IngressLeaderWireSlot, RuntimeTimeoutVoteEpisodeOwner>,
    },
}
impl RuntimeTimeoutVoteEpisodeAdmissionPlan {
    const fn is_candidate(&self) -> bool {
        !matches!(self, Self::NonCandidate)
    }
    fn count_transition(&self) -> (u8, u8) {
        match self {
            Self::NonCandidate => (0, 0),
            Self::FirstAdmission {
                candidate,
                prospective,
            } => {
                debug_assert_eq!(prospective.get(&candidate.slot), Some(&candidate.owner));
                (0, 1)
            }
            Self::CoalescedRetry {
                candidate,
                prospective,
            } => {
                debug_assert!(
                    prospective.get(&candidate.slot).is_some_and(
                        |incumbent| incumbent.same_lifecycle_owner_as(&candidate.owner)
                    )
                );
                (1, 1)
            }
        }
    }
    fn prospective(
        self,
    ) -> Option<BTreeMap<FairV2IngressLeaderWireSlot, RuntimeTimeoutVoteEpisodeOwner>> {
        match self {
            Self::NonCandidate => None,
            Self::FirstAdmission { prospective, .. } | Self::CoalescedRetry { prospective, .. } => {
                Some(prospective)
            }
        }
    }
}
impl RuntimeClockReservationBlockers {
    const fn any(self) -> bool {
        self.timeout || self.retransmit
    }
    const fn timeout_only(self) -> bool {
        self.timeout && !self.retransmit
    }
}
/// Finite current-view producer prefix opened by one durable timeout.
///
/// The inclusive lifecycle cut is the exact `BeginTimeout` owner. Every
/// restart-dormant leader-wire token is restored into the shared ordinal
/// source before this owner is minted, while every fresh producer after the
/// cut receives a larger ordinal. The physical cut independently prevents a
/// pre-crash token from claiming a carrier which was never actually replayed
/// after the timeout boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeTimeoutRecoveryEpisode {
    tag: EventTag,
    timeout_owner: RuntimeLifecycleOwner,
    physical_cut: u128,
    pre_frozen_retransmit: Option<(RuntimeLifecycleOwner, u128)>,
    timeout_vote_owner_universe: BTreeSet<FairV2IngressLeaderWireSlot>,
    admitted_timeout_vote_owners:
        BTreeMap<FairV2IngressLeaderWireSlot, RuntimeTimeoutVoteEpisodeOwner>,
}
impl RuntimeTimeoutRecoveryEpisode {
    fn validate_exact(&self) -> bool {
        self.timeout_owner.validate_exact()
            && self.timeout_owner.causal_origin().root_tag == self.tag
            && self.timeout_owner.causal_origin().root_class == SERVICE_CLASS_PROGRESS
            && self.physical_cut != 0
            && self
                .pre_frozen_retransmit
                .as_ref()
                .is_none_or(|(owner, cut)| {
                    owner.validate_exact()
                        && owner.causal_origin().root_tag == self.tag
                        && owner.causal_origin().root_class == SERVICE_CLASS_PROGRESS
                        && owner.lifecycle_ordinal() < self.timeout_owner.lifecycle_ordinal()
                        && *cut != 0
                        && *cut <= self.physical_cut
                })
            && self.timeout_vote_owner_universe.iter().all(|slot| {
                slot.phase == FairV2IngressLeaderWirePhase::TimeoutVote
                    && slot.chunk_index.is_none()
            })
            && self.admitted_timeout_vote_owners.len() <= self.timeout_vote_owner_universe.len()
            && self
                .admitted_timeout_vote_owners
                .iter()
                .all(|(slot, owner)| {
                    self.timeout_vote_owner_universe.contains(slot)
                        && slot == &owner.token.slot
                        && slot.phase == FairV2IngressLeaderWirePhase::TimeoutVote
                        && slot.chunk_index.is_none()
                        && owner.validate_against(
                            self.timeout_owner.lifecycle_ordinal(),
                            self.physical_cut,
                        )
                })
    }
}
/// One-owner, class-aware scheduling shell for Sumeragi v2.
pub(crate) struct SerializedV2Runtime<D: RuntimeDriver = SumeragiV2Adapter> {
    driver: D,
    ingress: BoundedIngress<D::Command>,
    deferred_ingress_ownership: BTreeMap<u128, RuntimeIngressOwnershipEvidence>,
    deferred_lifecycle_ownership: BTreeMap<u128, RuntimeDeferredLifecycleOwnership>,
    /// Authenticated Proposal tokens waiting in the same bounded deferred owners.
    deferred_remote_proposal_replay: BTreeMap<u128, AuthenticatedRemoteProposalDispatchOrigin>,
    /// FIFO owners which received one exact dependency-predecessor turn but
    /// hit retryable adapter capacity while the same signing fence remained.
    /// They are temporarily excluded from that fence's dependency minimum so
    /// retry cannot become a new cycle ahead of the completion which frees
    /// the capacity. The set is bounded by the physical FIFO capacity.
    fence_retry_blocked_fifo_owners: Vec<RuntimeQueueOccurrenceOwner>,
    /// Exact signer incarnation which minted
    /// `fence_retry_blocked_fifo_owners`. A duplicate TC/CommitQC preserves
    /// this identity; a consumed or replaced signer retires the whole set
    /// before another scheduler owner can observe it.
    fence_retry_signature_fence_identity: Option<D::SignatureFenceIdentity>,
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
    /// Restart-recovery authority retained after the timeout owner transfers.
    /// This is process-local scheduling evidence, never wire or configuration
    /// state.
    timeout_recovery_episode: Option<RuntimeTimeoutRecoveryEpisode>,
    retransmit_owner: Option<RuntimeLifecycleOwner>,
    /// Receiver-local ingress high-watermark frozen atomically with the
    /// current periodic owner. Retries of the same clock episode retain this
    /// cut; a later physical replay cannot revive an older logical position
    /// ahead of the already-admitted periodic work.
    retransmit_owner_physical_cut: Option<u128>,
    dormant_fresh_lifecycle_owners:
        BTreeMap<(RuntimeFreshRootKind, iroha_crypto::Hash), RuntimeLifecycleOwner>,
    /// At most one direct Proposal origin awaiting exact effect binding.
    pending_remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>,
    /// Exact current-view Set-B Proposal origin waiting for periodic fallback.
    dormant_remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>,
    pending_effect_ownership: Option<Vec<RuntimeEffectOwnership>>,
    #[cfg(test)]
    recovered_validated_body_bindings: BTreeSet<(wire::ConsensusRound, wire::BlockSubject)>,
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
    #[cfg_attr(not(test), allow(dead_code))]
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
            deferred_remote_proposal_replay: BTreeMap::new(),
            fence_retry_blocked_fifo_owners: Vec::new(),
            fence_retry_signature_fence_identity: None,
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
            timeout_recovery_episode: None,
            retransmit_owner: None,
            retransmit_owner_physical_cut: None,
            dormant_fresh_lifecycle_owners: BTreeMap::new(),
            pending_remote_proposal_replay: None,
            dormant_remote_proposal_replay: None,
            pending_effect_ownership: None,
            #[cfg(test)]
            recovered_validated_body_bindings: BTreeSet::new(),
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
    fn reconcile_dormant_remote_proposal_replay(&mut self) {
        let retain = self
            .dormant_remote_proposal_replay
            .as_ref()
            .is_some_and(|origin| self.driver.remote_proposal_fetch_replay_is_dormant(origin));
        if !retain {
            self.dormant_remote_proposal_replay = None;
        }
    }
    fn retain_dormant_remote_proposal_replay(
        &mut self,
        origin: AuthenticatedRemoteProposalDispatchOrigin,
    ) -> Result<(), EnqueueError> {
        if !self.driver.remote_proposal_fetch_replay_is_dormant(&origin) {
            return Err(EnqueueError::FailClosed);
        }
        let retained = match self.dormant_remote_proposal_replay.take() {
            None => origin,
            Some(incumbent)
                if self
                    .driver
                    .remote_proposal_fetch_replay_is_dormant(&incumbent)
                    && incumbent.same_authenticated_proposal(&origin) =>
            {
                incumbent
            }
            Some(incumbent) => {
                self.dormant_remote_proposal_replay = Some(incumbent);
                return Err(EnqueueError::FailClosed);
            }
        };
        self.dormant_remote_proposal_replay = Some(retained);
        Ok(())
    }
    fn bind_or_retain_remote_proposal_replay(
        &mut self,
        origin: AuthenticatedRemoteProposalDispatchOrigin,
        effects: &[D::Effect],
        ownership: &mut [RuntimeEffectOwnership],
    ) -> Result<(), EnqueueError> {
        let dormant = self
            .driver
            .bind_remote_proposal_fetch_replay(origin, effects, ownership)
            .map_err(|()| EnqueueError::FailClosed)?;
        if let Some(dormant) = dormant {
            self.retain_dormant_remote_proposal_replay(dormant)?;
        }
        Ok(())
    }
    fn retain_effect_ownership(
        &mut self,
        source: RuntimeEffectSource,
        parent: Option<&RuntimeLifecycleOwner>,
        parent_statement: Option<&RuntimeCandidateSemanticStatement>,
        effects: &[D::Effect],
    ) -> Result<(), EnqueueError> {
        let dormant_retransmit = if source == RuntimeEffectSource::Retransmit {
            self.dormant_remote_proposal_replay.take()
        } else {
            self.reconcile_dormant_remote_proposal_replay();
            None
        };
        if self.pending_remote_proposal_replay.is_some() && dormant_retransmit.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        if effects.is_empty() {
            let origin = self
                .pending_remote_proposal_replay
                .take()
                .or(dormant_retransmit);
            if let Some(origin) = origin {
                self.bind_or_retain_remote_proposal_replay(origin, effects, &mut [])?;
            }
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
                self.driver
                    .effect_candidate_semantic_binding(
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
            let owner = match causality {
                RuntimeEffectCausality::Inherit => {
                    parent.cloned().ok_or(EnqueueError::FailClosed)?
                }
                RuntimeEffectCausality::Fresh(kind) => {
                    let tag = D::effect_root_tag(effect).unwrap_or(self.round_tag);
                    self.mint_fresh_lifecycle_owner(
                        tag,
                        CommandClass::Progress,
                        kind,
                        &D::fresh_effect_semantic_identity(effect, kind),
                    )?
                }
            };
            if candidate.is_some() {
                candidate_position = candidate_position
                    .checked_add(1)
                    .ok_or(EnqueueError::FailClosed)?;
            }
            let effect_position = u8::try_from(index + 1).map_err(|_| EnqueueError::FailClosed)?;
            let evidence = RuntimeEffectOwnership::new_bound(
                owner,
                causality,
                D::effect_refinement_kind(effect),
                &D::effect_semantic_identity(effect),
                candidate.as_ref(),
                effect_position,
                effect_count,
                candidate.as_ref().map_or(0, |_| candidate_position),
                candidate_count,
            )?;
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
        if let Some(origin) = self.pending_remote_proposal_replay.take() {
            self.bind_or_retain_remote_proposal_replay(origin, effects, &mut ownership)?;
        }
        if let Some(origin) = dormant_retransmit {
            self.bind_or_retain_remote_proposal_replay(origin, effects, &mut ownership)?;
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
            || ownership.iter().any(|evidence| !evidence.validate_exact())
        {
            self.latch_fail_closed("effect lifecycle ownership did not match its batch");
            return Err("Sumeragi v2 effect lifecycle ownership was invalid".to_owned());
        }
        Ok(ownership)
    }
    /// Bind an externally constructed historical-lock retransmit batch to the
    /// same exact fresh lifecycle ownership used by the production timer.
    #[cfg(test)]
    pub(crate) fn retain_retransmit_effect_ownership_for_test(
        &mut self,
        effects: &[D::Effect],
    ) -> Result<(), EnqueueError> {
        self.retain_effect_ownership(RuntimeEffectSource::Retransmit, None, None, effects)
    }
    /// Bind one externally settled lifecycle effect to the next runtime
    /// ordinal for ordering tests whose lifecycle coordinator is out of scope.
    #[cfg(test)]
    pub(crate) fn retain_external_lifecycle_effect_ownership_for_test(
        &mut self,
        effects: &[D::Effect],
    ) -> Result<(), String>
    where
        D: RuntimeDriver<Effect = AdapterEffect>,
    {
        if effects.len() != 1 || self.pending_effect_ownership.is_some() {
            return Err("external lifecycle test effect must be one unowned successor".to_owned());
        }
        let lifecycle_ordinal = self
            .ingress
            .lifecycle_ordinals
            .reserve_one()
            .map_err(|error| error.to_string())?;
        let ownership = bind_adapter_effect_batch_ownership(
            effects,
            vec![RuntimeEffectOwnerAssignment::fresh_for_test(
                self.round_tag,
                lifecycle_ordinal,
            )],
        )?;
        self.pending_effect_ownership = Some(ownership);
        Ok(())
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
        let recovery_episode_is_valid =
            self.timeout_recovery_episode
                .as_ref()
                .is_none_or(|episode| {
                    episode.validate_exact()
                        && episode.tag == self.round_tag
                        && episode.physical_cut <= self.ingress_physical_cut
                        && episode.timeout_vote_owner_universe
                            == self.driver.timeout_vote_owner_universe()
                });
        if timeout_is_paired && retransmit_is_paired && cuts_are_valid && recovery_episode_is_valid
        {
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
    /// Frozen clock episodes which one physically later occurrence would
    /// overtake by resurrecting an older logical position.
    ///
    /// The two clock classes remain distinct because only timeout has
    /// absolute priority over FIFO debt. A periodic reservation can never be
    /// used as authority for a restart-recovery ingress exception.
    fn clock_owner_reservation_blockers_occurrence(
        &self,
        lifecycle_ordinal: u128,
        source_physical_ordinal: u64,
    ) -> Result<RuntimeClockReservationBlockers, EnqueueError> {
        if lifecycle_ordinal == 0 || source_physical_ordinal == 0 {
            return Err(EnqueueError::FailClosed);
        }
        self.validate_clock_owner_physical_cuts()?;
        let occurrence_is_blocked = |owner: &RuntimeLifecycleOwner, physical_cut: u128| {
            u128::from(source_physical_ordinal) >= physical_cut
                && lifecycle_ordinal <= owner.lifecycle_ordinal()
        };
        Ok(RuntimeClockReservationBlockers {
            timeout: self
                .timeout_owner
                .as_ref()
                .zip(self.timeout_owner_physical_cut)
                .is_some_and(|(owner, physical_cut)| occurrence_is_blocked(owner, physical_cut)),
            retransmit: self
                .retransmit_owner
                .as_ref()
                .zip(self.retransmit_owner_physical_cut)
                .is_some_and(|(owner, physical_cut)| occurrence_is_blocked(owner, physical_cut)),
        })
    }
    /// Whether one physically later occurrence would resurrect a logical
    /// position at or ahead of any already-frozen clock owner.
    ///
    /// Ordinary occurrences stay in their existing fair-ingress or executor
    /// owner and retry after the clock episode transfers. They must not enter
    /// the FIFO, where persistent `fifo_owed` debt could otherwise let a
    /// post-cut identity overtake a periodic reservation.
    fn clock_owner_reservation_blocks_occurrence(
        &self,
        lifecycle_ordinal: u128,
        source_physical_ordinal: u64,
    ) -> Result<bool, EnqueueError> {
        self.clock_owner_reservation_blockers_occurrence(lifecycle_ordinal, source_physical_ordinal)
            .map(RuntimeClockReservationBlockers::any)
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
    /// Return the exact timeout root whose durable dispatch opened the
    /// current view's finite restart-recovery episode.
    ///
    /// The root remains in the bounded fresh-owner cache after dispatch so a
    /// restored leader-wire token can prove that it was admitted before the
    /// timeout.  `timeout_emitted` alone is insufficient authority: a corrupt
    /// or missing cached root must fail closed instead of turning every old
    /// physical replay into retained-debt bypass.
    fn emitted_timeout_recovery_owner(
        &self,
    ) -> Result<Option<RuntimeLifecycleOwner>, EnqueueError> {
        if !self.timeout_emitted {
            return Ok(None);
        }
        if self.timeout_owner.is_some() || self.timeout_owner_physical_cut.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        let Some(episode) = self.timeout_recovery_episode.as_ref() else {
            return Err(EnqueueError::FailClosed);
        };
        if !episode.validate_exact()
            || episode.tag != self.round_tag
            || episode.physical_cut > self.ingress_physical_cut
            || episode.timeout_vote_owner_universe != self.driver.timeout_vote_owner_universe()
        {
            return Err(EnqueueError::FailClosed);
        }
        Ok(Some(episode.timeout_owner.clone()))
    }
    /// Whether the current clock owners are outside the frozen timeout
    /// recovery prefix.
    ///
    /// The timeout itself and a retransmit already frozen before it must run
    /// first. A fresh post-timeout retransmit has an ordinal above the
    /// inclusive episode cut and cannot acquire precedence over a restored
    /// vote whose immutable owner is below that cut.
    fn timeout_recovery_episode_allows_clock_blockers(
        &self,
        blockers: RuntimeClockReservationBlockers,
    ) -> Result<bool, EnqueueError> {
        let Some(episode) = self.timeout_recovery_episode.as_ref() else {
            return Ok(false);
        };
        if !self.timeout_emitted
            || !episode.validate_exact()
            || episode.tag != self.round_tag
            || episode.pre_frozen_retransmit.is_some()
            || blockers.timeout
        {
            return Ok(false);
        }
        if !blockers.retransmit {
            return Ok(true);
        }
        match (
            self.retransmit_owner.as_ref(),
            self.retransmit_owner_physical_cut,
        ) {
            (Some(owner), Some(cut)) => Ok(owner.validate_exact()
                && owner.lifecycle_ordinal() > episode.timeout_owner.lifecycle_ordinal()
                && cut != 0
                && cut <= self.ingress_physical_cut),
            (None, None) => Err(EnqueueError::FailClosed),
            (Some(_), None) | (None, Some(_)) => Err(EnqueueError::FailClosed),
        }
    }
    /// Supersede the one best-effort retransmit which was frozen before the
    /// absolute timeout cut.
    ///
    /// This runs inside the successful timeout macro-step. The captured owner
    /// has never dispatched and therefore has no effects or causal children;
    /// clearing that exact pair cannot lose protocol state. A later periodic
    /// tick is a fresh producer above the timeout cut and remains enabled.
    fn supersede_pre_timeout_retransmit(
        &mut self,
        timeout_owner: &RuntimeLifecycleOwner,
    ) -> Result<(), EnqueueError> {
        let Some(episode) = self.timeout_recovery_episode.as_ref() else {
            return Err(EnqueueError::FailClosed);
        };
        if !episode.validate_exact()
            || &episode.timeout_owner != timeout_owner
            || episode.tag != self.round_tag
        {
            return Err(EnqueueError::FailClosed);
        }
        let captured = episode.pre_frozen_retransmit.clone();
        match (
            captured.as_ref(),
            self.retransmit_owner.as_ref(),
            self.retransmit_owner_physical_cut,
        ) {
            (Some((captured_owner, captured_cut)), Some(owner), Some(cut))
                if captured_owner == owner && *captured_cut == cut =>
            {
                self.retransmit_owner = None;
                self.retransmit_owner_physical_cut = None;
                self.timeout_recovery_episode
                    .as_mut()
                    .expect("validated timeout episode remains installed")
                    .pre_frozen_retransmit = None;
                Ok(())
            }
            (None, None, None) => Ok(()),
            _ => Err(EnqueueError::FailClosed),
        }
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
    /// Replace the bounded set of exact owners currently held by retained
    /// executor effects or asynchronous Sign/Store/Validate/Apply tasks.
    ///
    /// The executor derives this set from its existing bounded maps before
    /// each runtime step. Supplying a forged carrier or exceeding the existing
    /// pending-work plus the ordinary and typed-control retained-batch bound
    /// fails closed.
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
    /// Return the number of external owners published to the runtime.
    #[cfg(test)]
    pub(crate) fn external_lifecycle_owner_count(&self) -> usize {
        self.external_lifecycle_owners.len()
    }
    /// Bind external lifecycle capacity to the effect executor's existing
    /// pending-work limit plus one ordinary and one typed-control retained
    /// reducer-effect batch.
    ///
    /// Runtime ingress and asynchronous effect work have independent bounded
    /// configurations.  Keeping this relation explicit avoids rejecting a
    /// legitimate executor with a small ingress FIFO and a larger task bound.
    pub(crate) fn configure_external_lifecycle_owner_capacity(
        &mut self,
        max_pending_work: usize,
    ) -> Result<(), String> {
        let retained_capacity = MAX_EFFECTS_PER_STEP
            .checked_mul(2)
            .ok_or_else(|| "external lifecycle-owner capacity overflowed".to_owned())?;
        let capacity = max_pending_work
            .checked_add(retained_capacity)
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
    ) -> Result<LocalProposalEffectOwnership, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        if tag != self.round_tag {
            self.latch_fail_closed("local proposal changed the authoritative reducer tag");
            return Err("Sumeragi v2 local proposal tag was not authoritative".to_owned());
        }
        if self.driver.durable_current_round_local_proposal_is_closed() {
            return Err(
                "Sumeragi v2 current round is already closed by durable local safety authority"
                    .to_owned(),
            );
        }
        if let Some(reservation) = self.active_view_producer.as_ref() {
            if reservation.tag != tag || !reservation.owner.validate_exact() {
                self.latch_fail_closed(
                    "local proposal changed its active-view producer reservation",
                );
                return Err(
                    "Sumeragi v2 local proposal changed its active-view producer".to_owned(),
                );
            }
            let owner = reservation.owner.clone();
            self.retain_fresh_lifecycle_alias(
                tag,
                CommandClass::Normal,
                RuntimeFreshRootKind::LocalProposalAdmission,
                &manifest.encode(),
                &owner,
            )
            .map_err(|error| error.to_string())?;
            let effect = AdapterEffect::StoreBody {
                tag,
                round: manifest.round,
                subject: manifest.subject,
            };
            let ownership = bind_adapter_effect_batch_ownership(
                std::slice::from_ref(&effect),
                vec![RuntimeEffectOwnerAssignment::inherit(owner)],
            )?
            .pop()
            .ok_or_else(|| "local proposal StoreBody binding was empty".to_owned())?;
            return LocalProposalEffectOwnership::from_exact_assemble_body(
                ownership, &effect, manifest,
            )
            .ok_or_else(|| {
                "local proposal StoreBody replay seal did not match its active-view owner"
                    .to_owned()
            });
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
        let ownership = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnerAssignment::fresh_root(
                owner,
                RuntimeFreshRootKind::LocalProposalAdmission,
            )],
        )?
        .pop()
        .ok_or_else(|| "local proposal StoreBody binding was empty".to_owned())?;
        LocalProposalEffectOwnership::from_exact_assemble_body(ownership, &effect, manifest)
            .ok_or_else(|| {
                "local proposal StoreBody replay seal did not match its fresh owner".to_owned()
            })
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
        if self.driver.durable_current_round_local_proposal_is_closed() {
            return Ok(false);
        }
        let Some(reservation) = self.active_view_producer.as_ref() else {
            return Ok(!self.clocks_armed);
        };
        if reservation.tag != tag || !reservation.owner.validate_exact() {
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
            if reservation.tag == tag && reservation.owner.validate_exact() {
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
        self.active_view_producer = Some(ActiveViewProducerReservation { tag, owner });
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
            || !reservation.owner.validate_exact()
            || !ownership.validate_exact()
        {
            self.latch_fail_closed("Proposal fanout changed its active-view producer");
            return Err("Sumeragi v2 Proposal fanout changed producer ownership".to_owned());
        }
        if &reservation.owner != ownership.owner() {
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
        command: &D::Command,
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
            RuntimeCommandAdmissionPreflight::Coalesce
                if self
                    .driver
                    .owned_terminal_completion_matches_effect(tag, command, ownership) =>
            {
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
        if self.owned_preflight_is_coalesced(tag, &command, preflight, ownership)? {
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
        let all_active = self.driver.all_deferred_admission_ordinals();
        if !active.is_subset(&all_active) {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        let mut retained = self.deferred_ingress_ownership.clone();
        let mut lifecycle_ownership = self.deferred_lifecycle_ownership.clone();
        lifecycle_ownership.retain(|ordinal, _| all_active.contains(ordinal));
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
            || self
                .deferred_remote_proposal_replay
                .iter()
                .any(|(ordinal, origin)| {
                    active.contains(ordinal)
                        && (self.deferred_ingress_ownership.get(ordinal) != Some(&origin.ingress)
                            || !origin
                                .ingress
                                .exactly_matches_authenticated(&origin.authenticated)
                            || !retained.get(ordinal).is_some_and(|ingress| {
                                ingress.exactly_matches_authenticated(&origin.authenticated)
                            }))
                })
        {
            return Err(RuntimeIngressMergeError::Conflict);
        }
        // The replay origin and runtime ingress map are one physical Proposal
        // carrier, so commit their validated refinements together.
        for (ordinal, origin) in &mut self.deferred_remote_proposal_replay {
            if active.contains(ordinal) {
                origin.ingress = retained[ordinal].clone();
            }
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
        self.deferred_remote_proposal_replay
            .retain(|ordinal, _| authenticated.contains(ordinal));
        if !deferred_lifecycle_ordinals_are_unique(&lifecycle)
            || ingress.iter().any(|(ordinal, ownership)| {
                !ownership.validate_frozen_physical() || !lifecycle.contains_key(ordinal)
            })
            || self
                .deferred_remote_proposal_replay
                .iter()
                .any(|(ordinal, origin)| {
                    ingress.get(ordinal) != Some(&origin.ingress)
                        || !origin
                            .ingress
                            .exactly_matches_authenticated(&origin.authenticated)
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
            remote_proposal_replay,
        } = dispatch;
        if self.pending_remote_proposal_replay.is_some() {
            self.latch_fail_closed("a Proposal replay origin overtook exact effect binding");
            return Err(RuntimeError::FailClosed);
        }
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
        self.deferred_remote_proposal_replay
            .retain(|ordinal, _| active.contains(ordinal));
        match (retry_unadmitted, deferred_ordinal, remote_proposal_replay) {
            (true, None, Some(origin)) => {
                // The exact authenticated command is restored by the caller;
                // its immutable token and ingress carrier mint a fresh sealed
                // projection on the later dispatch attempt.
                drop(origin);
            }
            (true, None, None) => {}
            (false, Some(ordinal), Some(origin)) => {
                let retained_ingress = self
                    .deferred_ingress_ownership
                    .get(&ordinal)
                    .cloned()
                    .ok_or_else(|| {
                        self.latch_fail_closed(
                            "deferred Proposal replay origin lost its ingress carrier",
                        );
                        RuntimeError::FailClosed
                    })?;
                let origin = if let Some(incumbent) =
                    self.deferred_remote_proposal_replay.remove(&ordinal)
                {
                    origin.merge_retained(incumbent, retained_ingress)
                } else {
                    origin.rebind_retained_ingress(retained_ingress)
                }
                .ok_or_else(|| {
                    self.latch_fail_closed(
                        "deferred Proposal replay origin changed its authenticated carrier",
                    );
                    RuntimeError::FailClosed
                })?;
                self.deferred_remote_proposal_replay.insert(ordinal, origin);
            }
            (false, None, Some(origin)) => {
                self.pending_remote_proposal_replay = Some(origin);
            }
            (false, Some(_), None) | (false, None, None) => {}
            (true, Some(_), _) => {
                self.latch_fail_closed("retryable Proposal dispatch retained deferred ownership");
                return Err(RuntimeError::FailClosed);
            }
        }
        if self
            .deferred_remote_proposal_replay
            .iter()
            .any(|(ordinal, origin)| {
                !active.contains(ordinal)
                    || self.deferred_ingress_ownership.get(ordinal) != Some(&origin.ingress)
                    || !origin
                        .ingress
                        .exactly_matches_authenticated(&origin.authenticated)
            })
        {
            self.latch_fail_closed("deferred Proposal replay map lost exact bounded ownership");
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
            if self.timeout_owner_physical_cut.is_some() || self.timeout_recovery_episode.is_some()
            {
                return Err(EnqueueError::FailClosed);
            }
            let owner = self.mint_fresh_lifecycle_owner(
                self.round_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::Timeout,
                b"begin-timeout",
            )?;
            let (pre_frozen_retransmit, superseded_newer_retransmit) = match (
                self.retransmit_owner.clone(),
                self.retransmit_owner_physical_cut,
            ) {
                (Some(retransmit), Some(cut))
                    if retransmit.lifecycle_ordinal() < owner.lifecycle_ordinal() =>
                {
                    (Some((retransmit, cut)), None)
                }
                (Some(retransmit), Some(cut))
                    if retransmit.lifecycle_ordinal() > owner.lifecycle_ordinal() =>
                {
                    (None, Some((retransmit, cut)))
                }
                // Actor-global lifecycle ordinals are unique. Distinct fresh
                // timeout and retransmit roots at the same ordinal therefore
                // prove corrupt ownership rather than a supersession order.
                (Some(_), Some(_)) => return Err(EnqueueError::FailClosed),
                (None, None) => (None, None),
                (Some(_), None) | (None, Some(_)) => return Err(EnqueueError::FailClosed),
            };
            let episode = RuntimeTimeoutRecoveryEpisode {
                tag: self.round_tag,
                timeout_owner: owner.clone(),
                physical_cut: self.ingress_physical_cut,
                pre_frozen_retransmit,
                timeout_vote_owner_universe: self.driver.timeout_vote_owner_universe(),
                admitted_timeout_vote_owners: BTreeMap::new(),
            };
            if !episode.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            let superseded_newer_retransmit_cache_key =
                if let Some((retransmit, cut)) = superseded_newer_retransmit.as_ref() {
                    let cache_key = (
                        RuntimeFreshRootKind::Retransmit,
                        retransmit.causal_origin().lifecycle_key,
                    );
                    if self.retransmit_owner.as_ref() != Some(retransmit)
                        || self.retransmit_owner_physical_cut != Some(*cut)
                        || self.dormant_fresh_lifecycle_owners.get(&cache_key) != Some(retransmit)
                    {
                        return Err(EnqueueError::FailClosed);
                    }
                    Some(cache_key)
                } else {
                    None
                };
            // A restart can restore the durable Timeout at its original
            // ordinal after a live periodic tick has already minted a newer
            // Retransmit. The absolute deadline supersedes only that exact
            // undelivered occurrence. Every validation above precedes this
            // mutation, and the remainder of the commit is infallible.
            if let Some(cache_key) = superseded_newer_retransmit_cache_key {
                self.dormant_fresh_lifecycle_owners.remove(&cache_key);
                self.retransmit_owner = None;
                self.retransmit_owner_physical_cut = None;
            }
            self.timeout_owner_physical_cut = Some(self.ingress_physical_cut);
            self.timeout_owner = Some(owner);
            self.timeout_recovery_episode = Some(episode);
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
            if reservation.tag != self.round_tag || !reservation.owner.validate_exact() {
                return Err(EnqueueError::FailClosed);
            }
            observe(&reservation.owner)?;
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
        self.minimum_active_lifecycle_ordinal_for_deferred_excluding_occurrences(
            target,
            excluded,
            &[],
        )
    }
    fn minimum_active_lifecycle_ordinal_for_deferred_excluding_occurrences(
        &self,
        target: &RuntimeDeferredLifecycleOwnership,
        excluded: &[RuntimeLifecycleOwner],
        excluded_occurrences: &[RuntimeQueueOccurrenceOwner],
    ) -> Result<Option<u128>, EnqueueError> {
        if !target.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        let mut minimum = self
            .ingress
            .oldest_active_lifecycle_ordinal_before_physical_cut_excluding(
                target.physical_cut,
                excluded_occurrences,
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
        // Producer reservations, executor-retained tasks, pending effect
        // handoffs, and reserved completion slots are passive capabilities.
        // They are validated by `freeze_due_clock_owners` and by their exact
        // transfer boundaries, but they are not runnable predecessors. A slow
        // Store/Validate/Sign task must not prevent an unrelated reducer
        // continuation from making progress.
        Ok(minimum)
    }
    /// Adapter-deferred occurrences which are not physically behind another
    /// active continuation or a frozen clock.
    ///
    /// This first-stage relation deliberately ignores logical FIFO rank. A
    /// signing-fenced target can have an older runnable FIFO predecessor; the
    /// dependency dispatcher must retain that target long enough to service
    /// the predecessor instead of making the target ineligible because of the
    /// very dependency which can unblock it.
    fn physically_eligible_deferred_admission_ordinals(
        &self,
    ) -> Result<BTreeSet<u128>, EnqueueError> {
        // Pairwise target-relative precedence is not transitive when several
        // frozen physical intervals overlap.  First remove every occurrence
        // whose source is physically behind any active target or clock.
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
        Ok(physically_eligible)
    }
    /// Adapter-deferred occurrences which may own the next runner turn under
    /// every active continuation's immutable physical cut and logical rank.
    fn eligible_deferred_admission_ordinals(&self) -> Result<BTreeSet<u128>, EnqueueError> {
        let physically_eligible = self.physically_eligible_deferred_admission_ordinals()?;
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
                .is_some_and(|reservation| owner_matches(&reservation.owner))
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

    fn current_signature_fence_identity(
        &self,
    ) -> Result<Option<D::SignatureFenceIdentity>, EnqueueError> {
        let identity = self
            .driver
            .signature_fence_identity()
            .map_err(|_| EnqueueError::FailClosed)?;
        if self.driver.signature_fence_is_active() != identity.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        Ok(identity)
    }
    fn clear_fence_retry_blocked_fifo_owners(&mut self) {
        self.fence_retry_blocked_fifo_owners.clear();
        self.fence_retry_signature_fence_identity = None;
    }
    fn reconcile_fence_retry_blocked_fifo_owners(&mut self) -> Result<(), EnqueueError> {
        if self.fence_retry_blocked_fifo_owners.is_empty() {
            return if self.fence_retry_signature_fence_identity.is_none() {
                Ok(())
            } else {
                Err(EnqueueError::FailClosed)
            };
        }
        if self.driver.all_deferred_admission_ordinals().is_empty()
            || self.driver.deferred_work_is_serviceable()
            || !self.driver.signature_fence_is_active()
        {
            self.clear_fence_retry_blocked_fifo_owners();
            return Ok(());
        }
        let Some(current_fence_identity) = self.current_signature_fence_identity()? else {
            return Err(EnqueueError::FailClosed);
        };
        let Some(marker_fence_identity) = self.fence_retry_signature_fence_identity.as_ref() else {
            return Err(EnqueueError::FailClosed);
        };
        if marker_fence_identity != &current_fence_identity {
            self.clear_fence_retry_blocked_fifo_owners();
            return Ok(());
        }
        if self.fence_retry_blocked_fifo_owners.len() > self.ingress.config.capacity {
            return Err(EnqueueError::FailClosed);
        }
        let queue_snapshot = self.ingress.ownership_snapshot();
        if !queue_snapshot.validate_identity() {
            return Err(EnqueueError::FailClosed);
        }
        let mut retained = Vec::with_capacity(self.fence_retry_blocked_fifo_owners.len());
        let mut seen = BTreeSet::new();
        for owner in &self.fence_retry_blocked_fifo_owners {
            if !seen.insert(owner.admission_ordinal) {
                return Err(EnqueueError::FailClosed);
            }
            match queue_snapshot
                .occurrence_index
                .get(&owner.admission_ordinal)
                .and_then(|index| queue_snapshot.occurrence_owners.get(*index))
            {
                Some(retained_owner) if retained_owner == owner => retained.push(owner.clone()),
                Some(_) => return Err(EnqueueError::FailClosed),
                None => {}
            }
        }
        // A certified transition or pipeline terminal may legitimately retire
        // a queued occurrence outside scheduler selection. Its obsolete retry
        // exclusion must disappear with it; a same-ordinal identity mismatch
        // still fails closed in `contains_queue_occurrence_owner` above.
        self.fence_retry_blocked_fifo_owners = retained;
        if self.fence_retry_blocked_fifo_owners.is_empty() {
            self.fence_retry_signature_fence_identity = None;
        }
        Ok(())
    }
    fn retain_fence_retry_blocked_fifo_owner(
        &mut self,
        owner: RuntimeQueueOccurrenceOwner,
    ) -> Result<(), EnqueueError> {
        if self.driver.all_deferred_admission_ordinals().is_empty()
            || self.driver.deferred_work_is_serviceable()
            || !self.driver.signature_fence_is_active()
            || !self.ingress.contains_queue_occurrence_owner(&owner)?
        {
            return Err(EnqueueError::FailClosed);
        }
        let Some(current_fence_identity) = self.current_signature_fence_identity()? else {
            return Err(EnqueueError::FailClosed);
        };
        match self.fence_retry_signature_fence_identity.as_ref() {
            Some(marker_fence_identity) if marker_fence_identity != &current_fence_identity => {
                return Err(EnqueueError::FailClosed);
            }
            Some(_) => {}
            None if self.fence_retry_blocked_fifo_owners.is_empty() => {
                self.fence_retry_signature_fence_identity = Some(current_fence_identity);
            }
            None => return Err(EnqueueError::FailClosed),
        }
        if self.fence_retry_blocked_fifo_owners.contains(&owner) {
            return Ok(());
        }
        if self.fence_retry_blocked_fifo_owners.len() >= self.ingress.config.capacity {
            return Err(EnqueueError::FailClosed);
        }
        self.fence_retry_blocked_fifo_owners.push(owner);
        Ok(())
    }
    fn periodic_timer_owns_runnable_turn(&self) -> Result<bool, EnqueueError> {
        let (Some(owner), Some(physical_cut)) = (
            self.retransmit_owner.as_ref(),
            self.retransmit_owner_physical_cut,
        ) else {
            return Ok(false);
        };
        let owner_ordinal = owner.lifecycle_ordinal();
        let first_prompt = self.retransmit_started_at == self.round_started_at;
        for queued in &self.ingress.commands {
            let queued_owner = queued.lifecycle_owner()?;
            if !queued_owner.is_post_physical_cut(physical_cut)
                && queued_owner.lifecycle_ordinal() < owner_ordinal
                && (!first_prompt
                    || (queued.class != CommandClass::Normal
                        && !self.external_lifecycle_owners.contains(&queued_owner)))
            {
                return Ok(false);
            }
        }
        if self.driver.deferred_work_is_serviceable()
            && self
                .eligible_deferred_admission_ordinals()?
                .iter()
                .any(|ordinal| {
                    let candidate = &self.deferred_lifecycle_ownership[ordinal];
                    !candidate.owner().is_post_physical_cut(physical_cut)
                        && candidate.owner().lifecycle_ordinal() < owner_ordinal
                })
        {
            return Ok(false);
        }
        Ok(!self.driver.signature_fence_is_active()
            || !self.external_lifecycle_owners.iter().any(|candidate| {
                !candidate.is_post_physical_cut(physical_cut)
                    && candidate.lifecycle_ordinal() < owner_ordinal
            }))
    }
    fn scheduler_arbitration_inputs(
        &self,
        now: Instant,
    ) -> Result<RuntimeSchedulerArbitrationInputs, EnqueueError> {
        self.validate_clock_owner_physical_cuts()?;
        // Validate every retained capability before making a scheduling
        // decision, but do not confuse passive ownership with runnable work.
        // The old global-minimum comparison made a stalled proposal build or
        // asynchronous I/O task a permanent barrier for the pacemaker and for
        // already-authenticated certificates.
        let _ = self.minimum_active_lifecycle_ordinal()?;
        let fifo_minimum = self.ingress.oldest_lifecycle_ordinal()?;
        let mut fifo_ready = fifo_minimum.is_some();
        let (mut completion_ready, mut progress_ready, mut normal_ready) = if fifo_ready {
            self.ingress.class_readiness()
        } else {
            (false, false, false)
        };
        let timers_enabled = self.clocks_armed;
        let queue_source_identity = &self.ingress.selection_source_identity;
        let ordinary_candidate =
            if timers_enabled && fifo_ready && !self.driver.deferred_work_is_serviceable() {
                self.ingress
                    .ordinary_candidate_owner_and_fence_state(|queued| {
                        (
                            self.driver
                                .command_is_blocked_by_deferred_fence(queued.tag, &queued.command)
                                || self.fence_retry_blocked_fifo_owners.iter().any(|owner| {
                                    owner.matches_queued(queue_source_identity, queued)
                                }),
                            self.driver
                                .command_matches_deferred_authenticated_owner(&queued.command),
                        )
                    })?
            } else {
                None
            };
        let deferred_owner_blocks_fifo =
            ordinary_candidate
                .as_ref()
                .is_some_and(|(candidate, blocked, deferred_alias)| {
                    self.deferred_lifecycle_ownership.values().any(|target| {
                        let post_cut = candidate.is_post_physical_cut(target.physical_cut);
                        (*blocked && !post_cut) || (*deferred_alias && post_cut)
                    })
                });
        let older_signer_blocks_fifo =
            ordinary_candidate
                .as_ref()
                .is_some_and(|(candidate, _, _)| {
                    self.driver.signature_fence_is_active()
                        && self.external_lifecycle_owners.iter().any(|owner| {
                            owner.lifecycle_ordinal() < candidate.lifecycle_ordinal()
                                || owner
                                    .causal_origin()
                                    .root_ingress_physical_ownership
                                    .is_some_and(|root| {
                                        candidate.is_post_physical_cut(root.physical_cut)
                                    })
                        })
                });
        if ordinary_candidate.is_some() && (deferred_owner_blocks_fifo || older_signer_blocks_fifo)
        {
            // Keep exact aliases behind their canonical Busy occurrence;
            // distinct post-cut input may acquire its own ordered Busy lane.
            // Earlier signers and reducer-blocked pre-cut input likewise keep
            // their positions until the matching completion runs.
            fifo_ready = false;
            completion_ready = false;
            progress_ready = false;
            normal_ready = false;
        }
        let raw_timeout_due = timers_enabled
            && !self.timeout_emitted
            && now.saturating_duration_since(self.round_started_at)
                >= round_timeout_for_view(self.base_round_timeout, self.round_tag.view());
        // The view deadline is absolute. Earlier local work may still finish
        // and is checked against the reducer's durable view/lock guards, but
        // it cannot suppress the one timeout occurrence for this view.
        let timeout_due = raw_timeout_due
            && self.timeout_owner.is_some()
            && self.timeout_owner_physical_cut.is_some();
        let raw_periodic_timer_due = timers_enabled
            && now.saturating_duration_since(self.retransmit_started_at)
                >= self.retransmit_interval;
        let periodic_timer_due = raw_periodic_timer_due
            && !timeout_due
            && self.retransmit_owner.is_some()
            && self.retransmit_owner_physical_cut.is_some()
            && self.periodic_timer_owns_runnable_turn()?;
        Ok(RuntimeSchedulerArbitrationInputs {
            clocks_armed: timers_enabled,
            timeout_due,
            periodic_timer_due,
            fifo_ready,
            completion_ready,
            progress_ready,
            normal_ready,
            fence_completion_bypass: false,
            fence_dependency_minimum_lifecycle_ordinal: None,
            fence_dependency_minimum_admission_ordinal: None,
            fence_dependency_minimum_fifo_position: None,
            fence_dependency_required_root_class: None,
            fence_predecessor_lifecycle_ordinal: None,
            fence_predecessor_ownership: None,
            fence_predecessor_ingress_ownership: None,
            fence_predecessor_occurrence_ownership: None,
            fence_retry_blocked_fifo_before: self.fence_retry_blocked_fifo_owners.clone(),
            fence_retry_marker_required: false,
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
            clocks_armed: arbitration.clocks_armed,
            timeout_due: arbitration.timeout_due,
            periodic_timer_due: arbitration.periodic_timer_due,
            fifo_ready: arbitration.fifo_ready,
            completion_ready: arbitration.completion_ready,
            progress_ready: arbitration.progress_ready,
            normal_ready: arbitration.normal_ready,
            fence_completion_bypass: arbitration.fence_completion_bypass,
            fence_dependency_minimum_lifecycle_ordinal: arbitration
                .fence_dependency_minimum_lifecycle_ordinal,
            fence_dependency_minimum_admission_ordinal: arbitration
                .fence_dependency_minimum_admission_ordinal,
            fence_dependency_minimum_fifo_position: arbitration
                .fence_dependency_minimum_fifo_position,
            fence_dependency_required_root_class: arbitration.fence_dependency_required_root_class,
            fence_predecessor_lifecycle_ordinal: arbitration.fence_predecessor_lifecycle_ordinal,
            fence_predecessor_ownership: arbitration.fence_predecessor_ownership,
            fence_predecessor_ingress_ownership: arbitration.fence_predecessor_ingress_ownership,
            fence_predecessor_occurrence_ownership: arbitration
                .fence_predecessor_occurrence_ownership,
            fence_retry_blocked_fifo_before: arbitration.fence_retry_blocked_fifo_before,
            fence_retry_blocked_fifo_after: self.fence_retry_blocked_fifo_owners.clone(),
            fence_retry_marker_required: arbitration.fence_retry_marker_required,
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
    /// set. Passive proposal and asynchronous-task capabilities are validated
    /// but never treated as runnable predecessors. The absolute timeout
    /// preempts every dependency and deferred branch once due; this is the
    /// pacemaker escape which prevents a stalled local producer, signer, or I/O
    /// task from pinning a view forever. Retransmission runs at most once per
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
        if self.reconcile_fence_retry_blocked_fifo_owners().is_err() {
            self.latch_fail_closed("fence-predecessor retry ownership was invalid");
            return Err(RuntimeError::FailClosed);
        }
        if self.freeze_due_clock_owners(now).is_err() {
            self.latch_fail_closed("clock lifecycle ownership could not be frozen");
            return Err(RuntimeError::FailClosed);
        }
        // A due view timeout is an absolute pacemaker boundary, not another
        // lifecycle-ordered work item. In particular, do not give an older
        // Busy-deferred occurrence or its completion dependency another turn
        // before emitting the one-shot timeout.
        let timeout_preempts = self
            .scheduler_arbitration_inputs(now)
            .map_err(|_| {
                self.latch_fail_closed("timeout-preemption ownership was invalid");
                RuntimeError::FailClosed
            })?
            .timeout_due;
        // An older timer or ingress occurrence can already belong to the
        // adapter's Busy-deferred set while a later Sign effect owns the only
        // completion which can open that reducer fence. Immutable lifecycle
        // order alone would then select the older occurrence forever, observe
        // Busy again, and starve its dependency. Give only the exact causally
        // owned fence completion one bounded turn; every frozen timer and
        // scheduler debt remains intact for the immediately following call.
        if !timeout_preempts && let Some(step) = self.dispatch_one_fence_dependency(now, None)? {
            return Ok(step);
        }
        // Work which already crossed runtime ingress and acquired the
        // adapter's Busy-deferred ownership competes by its frozen physical
        // cut and then by logical rank inside that retained predecessor set.
        // Once its WAL/signing fence opens, give exactly one eligible
        // transition a serialized turn. Each returned effect batch still
        // represents only one reducer macro-step.
        if !timeout_preempts && let Some(step) = self.dispatch_one_adapter_deferred(now, None)? {
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
                if self.supersede_pre_timeout_retransmit(&owner).is_err() {
                    self.latch_fail_closed(
                        "timeout recovery changed its captured retransmit before transfer",
                    );
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
                if let Some(episode) = self.timeout_recovery_episode.as_mut()
                    && episode.pre_frozen_retransmit.as_ref().is_some_and(
                        |(captured_owner, captured_cut)| {
                            captured_owner == &owner && *captured_cut == physical_cut
                        },
                    )
                {
                    episode.pre_frozen_retransmit = None;
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
        self.finish_dispatched_step(
            now,
            effects,
            effect_source,
            effect_parent,
            effect_parent_statement,
            producer_handoff,
            retained_deferred_ingress,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn finish_dispatched_step(
        &mut self,
        now: Instant,
        effects: Vec<D::Effect>,
        effect_source: RuntimeEffectSource,
        effect_parent: RuntimeLifecycleOwner,
        effect_parent_statement: Option<RuntimeCandidateSemanticStatement>,
        producer_handoff: Option<ProducerContinuationHandoffToken>,
        retained_deferred_ingress: bool,
    ) -> Result<RuntimeStep<D::Effect>, RuntimeError<D::Error>> {
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
    /// Try one typed pacemaker/control turn without admitting ordinary work.
    ///
    /// A due absolute timeout is emitted first. Otherwise only a deeply
    /// validated Progress root (TimeoutVote, TC, PrepareQC, CommitQC), one of
    /// its trusted Completion descendants, an exact dependency which opens an
    /// older signing fence, or an already-admitted deferred continuation of
    /// that same root may run. `None` proves that this call consumed no
    /// scheduler owner and is safe to use inside a retained I/O or exact-Serve
    /// episode.
    pub(crate) fn try_step_pacemaker_escape(
        &mut self,
        now: Instant,
    ) -> Result<Option<RuntimeStep<D::Effect>>, RuntimeError<D::Error>> {
        if self.fail_closed {
            return Err(RuntimeError::FailClosed);
        }
        if self.last_scheduler_ownership.is_some()
            || self.pending_effect_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            self.latch_fail_closed("pacemaker escape overtook retained runtime ownership");
            return Err(RuntimeError::FailClosed);
        }
        if !self.clocks_armed {
            return Err(RuntimeError::ClocksNotArmed);
        }
        if self.reconcile_fence_retry_blocked_fifo_owners().is_err() {
            self.latch_fail_closed("pacemaker fence-retry ownership was invalid");
            return Err(RuntimeError::FailClosed);
        }
        if self.driver.pacemaker_escape_is_parked() {
            return Ok(None);
        }
        if self.freeze_due_clock_owners(now).is_err() {
            self.latch_fail_closed("pacemaker escape could not freeze clock ownership");
            return Err(RuntimeError::FailClosed);
        }
        let timeout_due = self
            .scheduler_arbitration_inputs(now)
            .map_err(|_| {
                self.latch_fail_closed("pacemaker escape ownership was invalid");
                RuntimeError::FailClosed
            })?
            .timeout_due;
        if timeout_due {
            return self.step(now).map(Some);
        }
        if let Some(step) = self.dispatch_one_fence_dependency(now, Some(SERVICE_CLASS_PROGRESS))? {
            return Ok(Some(step));
        }
        if let Some(step) = self.dispatch_one_adapter_deferred(now, Some(SERVICE_CLASS_PROGRESS))? {
            return Ok(Some(step));
        }
        self.dispatch_one_pacemaker_progress(now)
    }
    fn dispatch_one_pacemaker_progress(
        &mut self,
        now: Instant,
    ) -> Result<Option<RuntimeStep<D::Effect>>, RuntimeError<D::Error>> {
        if self.driver.pacemaker_escape_is_parked() {
            return Ok(None);
        }
        let selected_round_tag = self.round_tag;
        let schedule = self.schedule;
        let queue_before = self.ingress.ownership_snapshot();
        let fence_retry_blocked_fifo_before = self.fence_retry_blocked_fifo_owners.clone();
        let retry_blocked_admissions = self
            .fence_retry_blocked_fifo_owners
            .iter()
            .map(|owner| owner.admission_ordinal)
            .collect::<BTreeSet<_>>();
        let active_unserviceable_fence = self.driver.signature_fence_is_active()
            && !self.driver.deferred_work_is_serviceable()
            && !self.driver.all_deferred_admission_ordinals().is_empty();
        let driver = &self.driver;
        let selected = self
            .ingress
            .pop_pacemaker_progress_with_ownership(
                |queued| {
                    if queued
                        .admission_ordinal
                        .is_some_and(|ordinal| retry_blocked_admissions.contains(&ordinal))
                    {
                        return false;
                    }
                    if !active_unserviceable_fence {
                        return true;
                    }
                    let certified = queued.class == CommandClass::Progress
                        && driver.certified_progress_bypasses_signature_fence(&queued.command);
                    let deferred_alias = queued.class == CommandClass::Progress
                        && driver.command_matches_deferred_authenticated_owner(&queued.command);
                    certified
                        || deferred_alias
                        || (queued.identity.kind != RuntimeCommandKind::SignatureCompleted
                            && !driver
                                .command_is_blocked_by_deferred_fence(queued.tag, &queued.command))
                },
                |command| driver.certified_progress_bypasses_signature_fence(command),
            )
            .map_err(|_| {
                self.latch_fail_closed("pacemaker Progress selection lost exact ownership");
                RuntimeError::FailClosed
            })?;
        let Some((command, candidate)) = selected else {
            return Ok(None);
        };
        let certified_fence_escape = self
            .driver
            .certified_progress_bypasses_signature_fence(&command.command);
        if certified_fence_escape
            != matches!(
                candidate.selection_seal.kind,
                RuntimeQueueSelectionKind::PacemakerCertifiedProgress
            )
        {
            self.latch_fail_closed(
                "pacemaker certified-progress selection changed after queue ownership transfer",
            );
            return Err(RuntimeError::FailClosed);
        }
        let owner = match command.lifecycle_owner() {
            Ok(owner)
                if owner.lifecycle_ordinal() == candidate.lifecycle_ordinal
                    && owner.causal_origin() == &candidate.causal_origin
                    && owner.causal_origin().root_class == SERVICE_CLASS_PROGRESS =>
            {
                owner
            }
            Ok(_) | Err(_) => {
                self.latch_fail_closed("pacemaker Progress changed its causal root");
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
        if certified_fence_escape && (retry_unadmitted || retained_deferred_ingress) {
            self.latch_fail_closed(
                "certified pacemaker escape became retryable or adapter-deferred",
            );
            return Err(RuntimeError::FailClosed);
        }
        // Keep retry exclusions through this evidence boundary. A duplicate
        // but valid certificate leaves the exact signer unchanged and must
        // not re-enable a retry spin. Before the next scheduling turn,
        // reconciliation compares the stored signer identity and retires the
        // exclusions only if certified progress really consumed or replaced
        // that fence.
        let mut arbitration = self.scheduler_arbitration_inputs(now).map_err(|_| {
            self.latch_fail_closed("pacemaker Progress scheduler ownership was invalid");
            RuntimeError::FailClosed
        })?;
        arbitration.timeout_due = false;
        arbitration.periodic_timer_due = false;
        arbitration.fifo_ready = false;
        arbitration.completion_ready = false;
        arbitration.progress_ready = false;
        arbitration.normal_ready = false;
        arbitration.fence_retry_blocked_fifo_before = fence_retry_blocked_fifo_before;
        if retry_unadmitted {
            if self
                .ingress
                .restore_selected_command(retry_command, &candidate)
                .is_err()
            {
                self.latch_fail_closed("pacemaker Progress retry lost its exact queue owner");
                return Err(RuntimeError::FailClosed);
            }
            if self.driver.signature_fence_is_active()
                && !self.driver.deferred_work_is_serviceable()
                && !self.driver.all_deferred_admission_ordinals().is_empty()
            {
                arbitration.fence_retry_marker_required = true;
                let Some(retry_owner) = RuntimeQueueOccurrenceOwner::from_candidate(&candidate)
                else {
                    self.latch_fail_closed(
                        "pacemaker Progress retry lost its exact occurrence identity",
                    );
                    return Err(RuntimeError::FailClosed);
                };
                if self
                    .retain_fence_retry_blocked_fifo_owner(retry_owner)
                    .is_err()
                {
                    self.latch_fail_closed(
                        "pacemaker Progress retry could not retain its bounded bypass owner",
                    );
                    return Err(RuntimeError::FailClosed);
                }
            }
            let queue_after = self.ingress.ownership_snapshot();
            self.retain_scheduler_ownership(
                RuntimeSelectedOwnerKind::PacemakerProgressRetryRetained,
                selected_round_tag,
                RuntimeSelectedCandidateOwnership::Exact(candidate),
                queue_before,
                queue_after,
                arbitration,
                schedule,
                schedule,
            )?;
            return Ok(Some(RuntimeStep::Advanced(Vec::new())));
        }
        let queue_after = self.ingress.ownership_snapshot();
        self.retain_scheduler_ownership(
            RuntimeSelectedOwnerKind::PacemakerProgress,
            selected_round_tag,
            RuntimeSelectedCandidateOwnership::Exact(candidate),
            queue_before,
            queue_after,
            arbitration,
            schedule,
            schedule,
        )?;
        self.finish_dispatched_step(
            now,
            effects,
            RuntimeEffectSource::Fifo,
            owner,
            parent_statement,
            producer_handoff,
            retained_deferred_ingress,
        )
        .map(Some)
    }
    /// Dispatch the exact runnable FIFO dependency required by currently
    /// unserviceable adapter debt.
    ///
    /// This is not Completion-class priority in general. The production
    /// driver must prove that a callback matches its active signing fence and
    /// at least one exact deferred lifecycle remains blocked. The selected
    /// occurrence is the exact target-relative queue minimum after excluding
    /// only physical occurrences proved blocked by the same fence. Runnable
    /// FIFO owners and due clocks cannot be bypassed; passive external tasks,
    /// producers, and reservations are validated but do not become scheduling
    /// barriers. The ownership carrier records the exceptional edge and its
    /// exact lifecycle/admission/position rank explicitly.
    fn dispatch_one_fence_dependency(
        &mut self,
        now: Instant,
        required_predecessor_root_class: Option<u8>,
    ) -> Result<Option<RuntimeStep<D::Effect>>, RuntimeError<D::Error>> {
        if self.driver.deferred_work_is_serviceable() || !self.driver.signature_fence_is_active() {
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
        let eligible_deferred = self
            .physically_eligible_deferred_admission_ordinals()
            .map_err(|_| {
                self.latch_fail_closed(
                    "fence-completion deferred physical-cut ownership was invalid",
                );
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
            .fence_blocked_occurrence_owners(|queued| {
                self.driver
                    .command_is_blocked_by_deferred_fence(queued.tag, &queued.command)
            })
            .map_err(|_| {
                self.latch_fail_closed("fence-blocked FIFO ownership was invalid");
                RuntimeError::FailClosed
            })?;
        let blocked_dependency_owners = blocked_deferred_owners;
        let mut blocked_by_admission = BTreeMap::new();
        for owner in blocked_fifo_owners
            .into_iter()
            .chain(self.fence_retry_blocked_fifo_owners.iter().cloned())
        {
            match blocked_by_admission.insert(owner.admission_ordinal, owner.clone()) {
                Some(previous) if previous != owner => {
                    self.latch_fail_closed("fence-blocked FIFO occurrence identity changed");
                    return Err(RuntimeError::FailClosed);
                }
                Some(_) | None => {}
            }
        }
        let blocked_fifo_occurrences = blocked_by_admission.values().cloned().collect::<Vec<_>>();
        let blocked_admissions = blocked_by_admission
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let Some(oldest_unblocked_lifecycle) = self
            .minimum_active_lifecycle_ordinal_for_deferred_excluding_occurrences(
                &target,
                &blocked_dependency_owners,
                &blocked_fifo_occurrences,
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
            self.ingress.pop_fence_dependency_with_ownership(
                oldest_unblocked_lifecycle,
                target.physical_cut,
                |queued| driver.completion_unblocks_deferred_fence(queued.tag, &queued.command),
                |queued| {
                    required_predecessor_root_class
                        .is_none_or(|class| queued.causal_origin.root_class == class)
                        && queued
                            .admission_ordinal
                            .is_some_and(|ordinal| !blocked_admissions.contains(&ordinal))
                },
            )
        };
        let Some((command, candidate, is_completion)) = (match selected_result {
            Ok(selected) => selected,
            Err(_) => {
                self.latch_fail_closed("fence dependency lost exact FIFO ownership");
                return Err(RuntimeError::FailClosed);
            }
        }) else {
            // The target-relative minimum can instead belong to a due clock
            // owner. Leave that owner to ordinary timer arbitration.
            return Ok(None);
        };
        arbitration.fence_completion_bypass = is_completion;
        arbitration.fence_dependency_minimum_lifecycle_ordinal = Some(candidate.lifecycle_ordinal);
        arbitration.fence_dependency_minimum_admission_ordinal = Some(candidate.admission_ordinal);
        arbitration.fence_dependency_minimum_fifo_position = Some(candidate.fifo_position);
        arbitration.fence_dependency_required_root_class = (!is_completion)
            .then_some(required_predecessor_root_class)
            .flatten();
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
            self.latch_fail_closed("selected fence dependency changed its lifecycle owner");
            return Err(RuntimeError::FailClosed);
        }
        let parent_statement = command.candidate_semantic_statement;
        if !is_completion {
            let current_ingress = if command.ingress_ownership.is_some() {
                RuntimeDispatchIngress::DirectAuthenticated
            } else {
                RuntimeDispatchIngress::LocalOrCausal
            };
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
                        "retryable fence predecessor lost its exact queue owner",
                    );
                    return Err(RuntimeError::FailClosed);
                }
                let Some(retry_owner) = RuntimeQueueOccurrenceOwner::from_candidate(&candidate)
                else {
                    self.latch_fail_closed(
                        "retryable fence predecessor lost its exact occurrence identity",
                    );
                    return Err(RuntimeError::FailClosed);
                };
                if self
                    .retain_fence_retry_blocked_fifo_owner(retry_owner)
                    .is_err()
                {
                    self.latch_fail_closed(
                        "retryable fence predecessor could not retain its bounded bypass owner",
                    );
                    return Err(RuntimeError::FailClosed);
                }
                arbitration.fence_retry_marker_required = true;
                let queue_after = self.ingress.ownership_snapshot();
                self.retain_scheduler_ownership(
                    RuntimeSelectedOwnerKind::FencePredecessorRetryRetained,
                    round_tag,
                    RuntimeSelectedCandidateOwnership::Exact(candidate),
                    queue_before,
                    queue_after,
                    arbitration,
                    schedule,
                    schedule,
                )?;
                return Ok(Some(RuntimeStep::Advanced(Vec::new())));
            }
            let queue_after = self.ingress.ownership_snapshot();
            self.retain_scheduler_ownership(
                RuntimeSelectedOwnerKind::FencePredecessor,
                round_tag,
                RuntimeSelectedCandidateOwnership::Exact(candidate),
                queue_before,
                queue_after,
                arbitration,
                schedule,
                schedule,
            )?;
            return self
                .finish_dispatched_step(
                    now,
                    effects,
                    RuntimeEffectSource::Fifo,
                    owner,
                    parent_statement,
                    producer_handoff,
                    retained_deferred_ingress,
                )
                .map(Some);
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
        // Matcher-true consumed the exact signing fence. `Signed` may install
        // the next queued signable without an observable fence-free interval,
        // so retire every predecessor retry exclusion at this boundary.
        self.clear_fence_retry_blocked_fifo_owners();
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
    /// Dispatch one target-relative eligible adapter-owned transition without
    /// concatenating it with a timer or runtime-ingress command.
    ///
    /// Returning `None` means either no adapter debt exists or its reducer
    /// persistence/signature fence still needs an ordinary completion command.
    /// Returning `Some` always represents exactly one reducer macro-step.
    fn dispatch_one_adapter_deferred(
        &mut self,
        now: Instant,
        required_root_class: Option<u8>,
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
        let mut eligible = self.eligible_deferred_admission_ordinals().map_err(|_| {
            self.latch_fail_closed("deferred physical-cut lifecycle ownership was invalid");
            RuntimeError::FailClosed
        })?;
        if let Some(required_root_class) = required_root_class {
            eligible.retain(|ordinal| {
                self.deferred_lifecycle_ownership
                    .get(ordinal)
                    .is_some_and(|ownership| {
                        ownership.owner().causal_origin().root_class == required_root_class
                    })
            });
        }
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
        let remote_proposal_replay = self
            .deferred_remote_proposal_replay
            .remove(&evidence.admission_ordinal);
        if evidence.is_authenticated_ingress() != ingress_ownership.is_some()
            || self.reconcile_deferred_ingress_ownership(None).is_err()
        {
            self.latch_fail_closed("deferred service lost authenticated ingress ownership");
            return Err(RuntimeError::FailClosed);
        }
        let remote_proposal_replay = match (
            evidence.event_kind,
            remote_proposal_replay,
            ingress_ownership.as_ref(),
        ) {
            (DeferredEventKind::ProposalReceived, Some(origin), Some(ingress)) => origin
                .rebind_retained_ingress(ingress.clone())
                .ok_or_else(|| {
                    self.latch_fail_closed(
                        "deferred Proposal replay origin changed its selected ingress carrier",
                    );
                    RuntimeError::FailClosed
                })?
                .into(),
            (DeferredEventKind::ProposalReceived, _, _) | (_, Some(_), _) => {
                self.latch_fail_closed(
                    "deferred Proposal replay origin did not match its selected event",
                );
                return Err(RuntimeError::FailClosed);
            }
            (_, None, _) => None,
        };
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
        if self.pending_remote_proposal_replay.is_some() {
            self.latch_fail_closed("deferred Proposal replay origin overtook effect binding");
            return Err(RuntimeError::FailClosed);
        }
        self.pending_remote_proposal_replay = remote_proposal_replay;
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
    /// Last exact scheduling ownership carrier produced by `step`.
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
    /// worker.
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
    /// their own bounded queues. When no certified root is queued it excludes
    /// the one physical slot reserved for an authenticated TC or CommitQC.
    /// Once such a root arrives, that exact root is charged to the reserved
    /// slot and every ordinary Completion position remains available
    /// regardless of enqueue order.
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
        if wire_payload_is_certified_fence_escape(payload) {
            return self.ingress.check_certified_fence_escape_capacity().is_ok();
        }
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
    /// Whether the one-shot live-height clock boundary has been crossed.
    pub(in crate::sumeragi) const fn lifecycle_live_clocks_are_armed(&self) -> bool {
        self.clocks_armed
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
    /// Return whether one exact current-view Set-B Proposal replay origin is
    /// waiting for periodic fallback to publish its ordinary Fetch.
    pub(crate) const fn has_dormant_remote_proposal_replay(&self) -> bool {
        self.dormant_remote_proposal_replay.is_some()
    }
    /// Mutably borrow the driver for tests which must hold an exact
    /// persistence crash cut across runtime construction.
    #[cfg(test)]
    pub(crate) fn driver_mut_for_test(&mut self) -> &mut D {
        &mut self.driver
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
                self.timeout_recovery_episode = None;
                self.retransmit_owner = None;
                self.retransmit_owner_physical_cut = None;
                self.dormant_fresh_lifecycle_owners
                    .retain(|_, owner| owner.causal_origin().root_tag == tag);
                self.active_view_producer = Some(ActiveViewProducerReservation {
                    tag,
                    owner: ownership.owner().clone(),
                });
                self.schedule = ScheduleState::default();
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn observe_effects_with_test_ownership(
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
        let effect_count = u8::try_from(effects.len()).map_err(|_| EnqueueError::FailClosed)?;
        let candidates = effects
            .iter()
            .map(|effect| {
                self.driver
                    .effect_candidate_semantic_binding(effect, None)
                    .map_err(|_| EnqueueError::FailClosed)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let candidate_count = u8::try_from(
            candidates
                .iter()
                .filter(|candidate| candidate.is_some())
                .count(),
        )
        .map_err(|_| EnqueueError::FailClosed)?;
        let mut candidate_position = 0u8;
        let ownership = effects
            .iter()
            .zip(candidates.iter())
            .enumerate()
            .map(|(index, (effect, candidate))| {
                if candidate.is_some() {
                    candidate_position = candidate_position
                        .checked_add(1)
                        .ok_or(EnqueueError::FailClosed)?;
                }
                RuntimeEffectOwnership::new_bound(
                    owner.clone(),
                    RuntimeEffectCausality::Inherit,
                    D::effect_refinement_kind(effect),
                    &D::effect_semantic_identity(effect),
                    candidate.as_ref(),
                    u8::try_from(index + 1).map_err(|_| EnqueueError::FailClosed)?,
                    effect_count,
                    candidate.as_ref().map_or(0, |_| candidate_position),
                    candidate_count,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.pending_effect_ownership = Some(ownership);
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
include!("v2_runtime_ready_validate_publication.rs");

impl SerializedV2Runtime<SumeragiV2Adapter> {
    /// Freeze the serialized shell around one ordinary Fetch-to-Store preview.
    pub(in crate::sumeragi) fn prepare_certified_fetch_store(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<super::v2::CertifiedFetchStoreAdapterPreparationV1<'_>, AdapterError> {
        if self.fail_closed
            || self.ingress.len() != 0
            || self.pending_effect_ownership.is_some()
            || self.last_scheduler_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            return Err(AdapterError::DirectCertifiedBodyAvailableContractViolation);
        }
        self.driver.prepare_certified_fetch_store(tag, manifest)
    }
    /// Freeze the serialized shell around one ordinary Store-to-Validate preview.
    pub(in crate::sumeragi) fn prepare_durable_store_validate(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<super::v2::DurableStoreValidateAdapterPreparationV1<'_>, AdapterError> {
        if self.fail_closed
            || self.ingress.len() != 0
            || self.pending_effect_ownership.is_some()
            || self.last_scheduler_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            return Err(AdapterError::DirectBodyStoredContractViolation);
        }
        self.driver
            .prepare_durable_store_validate(tag, round, subject, receipt)
    }
    /// Freeze the serialized shell around one registry-owned Ready Validate outcome.
    pub(in crate::sumeragi) fn prepare_ready_durable_validate_adapter_preview<'registry>(
        &mut self,
        execution: PreparedReadyDurableValidateExecution<'registry>,
        local_publication: Option<(LocalProposalReadyCommandIdentity, u128)>,
    ) -> Result<
        PreparedReadyDurableValidateAdapterPreview<'registry, '_>,
        ReadyDurableValidateAdapterPreviewError<'registry>,
    > {
        if self.fail_closed
            || !self.ready_validate_runtime_gate_is_open(local_publication.is_some())
        {
            return Err(ReadyDurableValidateAdapterPreviewError::runtime_gate(
                execution,
                AdapterError::ReadyDurableValidatePublicationContractViolation,
            ));
        }
        execution.prepare_adapter_preview(&mut self.driver)
    }
    /// Freeze the serialized shell around one lifecycle-owned signature.
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(
        &mut self,
        authority: super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1,
    ) -> Result<super::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'_>, AdapterError>
    {
        // Queued ingress is inert; only active mutation debts exclude Completion.
        if self.fail_closed {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        if self.pending_effect_ownership.is_some()
            || self.last_scheduler_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionRuntimeDebt);
        }
        self.driver
            .prepare_recovered_lifecycle_sign_completion(authority)
    }
    /// Freeze the serialized shell around one recovered Decision Store preview.
    pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_store(
        &mut self,
        authority: super::v2_lifecycle_coordinator::RecoveredDecisionFetchStoreAdapterAuthorityV1,
    ) -> Result<super::v2::PreparedRecoveredDecisionFetchStoreAdapterV1<'_>, AdapterError> {
        if self.fail_closed
            || self.ingress.len() != 0
            || self.pending_effect_ownership.is_some()
            || self.last_scheduler_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            return Err(AdapterError::RecoveredDecisionFetchStoreMismatch);
        }
        self.driver
            .prepare_recovered_decision_fetch_store(authority)
    }
    /// Return whether a typed lifecycle Decision Apply may freeze reducer mutation.
    pub(in crate::sumeragi) fn lifecycle_decision_apply_dispatch_available(&self) -> bool {
        !self.fail_closed
            && self.pending_effect_ownership.is_none()
            && self.last_scheduler_ownership.is_none()
            && self.pending_leader_wire_terminals.is_empty()
    }
    /// Freeze the serialized shell around one registry-owned Apply completion.
    pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(
        &mut self,
        authority: LifecycleDecisionApplyAdapterCompletionAuthorityV1,
    ) -> Result<PreparedLifecycleDecisionApplyAdapterCompletionV1<'_>, AdapterError> {
        let recovered_requires_empty_ingress =
            authority.lineage() == LifecycleDecisionApplyLineageV1::Recovered;
        if self.fail_closed
            || (recovered_requires_empty_ingress && self.ingress.len() != 0)
            || self.pending_effect_ownership.is_some()
            || self.last_scheduler_ownership.is_some()
            || !self.pending_leader_wire_terminals.is_empty()
        {
            return Err(AdapterError::LifecycleDecisionApplyCompletionMismatch);
        }
        self.driver
            .prepare_lifecycle_decision_apply_completion(authority)
    }
    fn timeout_vote_recovery_candidate_from_fair(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> Result<Option<RuntimeTimeoutVoteEpisodeCandidate>, EnqueueError> {
        let Some(token) = ingress_ownership.leader_wire_token() else {
            return Ok(None);
        };
        let Some(physical_ordinal) = ingress_ownership.physical_admission_ordinal() else {
            return Ok(None);
        };
        self.timeout_vote_recovery_candidate(payload, token, physical_ordinal)
    }
    fn timeout_vote_recovery_candidate_from_runtime(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
        ingress_ownership: &RuntimeIngressOwnershipEvidence,
    ) -> Result<Option<RuntimeTimeoutVoteEpisodeCandidate>, EnqueueError> {
        let token = ingress_ownership
            .leader_wire_token()
            .map_err(|_| EnqueueError::FailClosed)?;
        let physical = ingress_ownership
            .leader_wire_physical_carrier()
            .map_err(|_| EnqueueError::FailClosed)?;
        match (token, physical) {
            (Some(token), Some((physical_ordinal, _))) => {
                self.timeout_vote_recovery_candidate(payload, token, physical_ordinal)
            }
            (None, None) => Ok(None),
            (Some(_), None) | (None, Some(_)) => Err(EnqueueError::FailClosed),
        }
    }
    /// Classify one exact current-view TimeoutVote in the finite episode opened
    /// by the already-dispatched local timeout.
    ///
    /// An owner scheduled below the timeout owner is a descent step whether
    /// its original physical publication precedes or straddles the runner's
    /// receiver snapshot, or it is a post-restart carrier. A first owner
    /// scheduled above the timeout is count-increasing replenishment and must
    /// own its original physical carrier. The fair leader-wire gate supplies
    /// one immutable TimeoutVote slot per authenticated roster source; this
    /// runtime projection retains that same owner through authentication and
    /// reducer service.
    fn timeout_vote_recovery_candidate(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
        token: &FairV2IngressLeaderWireToken,
        physical_ordinal: u64,
    ) -> Result<Option<RuntimeTimeoutVoteEpisodeCandidate>, EnqueueError> {
        let wire::ConsensusMessageV2Payload::TimeoutVote(vote) = payload else {
            return Ok(None);
        };
        let context = self.driver.wire_context();
        if !wire_payload_matches_current_strict_timeout_recovery_round(
            payload,
            context,
            self.round_tag,
        ) {
            return Ok(None);
        }
        if token.admission_ordinal() > physical_ordinal {
            return Err(EnqueueError::FailClosed);
        }
        let Some(timeout_owner) = self.emitted_timeout_recovery_owner()? else {
            return Ok(None);
        };
        let episode = self
            .timeout_recovery_episode
            .as_ref()
            .ok_or(EnqueueError::FailClosed)?;
        if episode.pre_frozen_retransmit.is_some() {
            return Ok(None);
        }
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let signer_index = usize::try_from(vote.signer).map_err(|_| EnqueueError::FailClosed)?;
        let signer = context
            .roster
            .get(signer_index)
            .ok_or(EnqueueError::FailClosed)?;
        if !token.validate_exact(
            context.id(),
            context.height,
            &roster,
            context.da_layout.max_chunk_count,
        ) || token.identity.phase != FairV2IngressLeaderWirePhase::TimeoutVote
            || token.identity.context_id != vote.round.context_id
            || token.identity.height != vote.round.height
            || token.identity.view != vote.round.view
            || token.identity.subject_hash != iroha_crypto::Hash::new([])
            || token.identity.semantic_origin != signer.validator
            || token.slot.semantic_origin != signer.validator
        {
            return Err(EnqueueError::FailClosed);
        }
        let timeout_ordinal = timeout_owner.lifecycle_ordinal();
        let disposition = if token.scheduler_ordinal() < timeout_ordinal
            && token.admission_ordinal() == physical_ordinal
        {
            RuntimeTimeoutVoteEpisodeDisposition::PreCutDescent
        } else if token.scheduler_ordinal() < timeout_ordinal
            && token.admission_ordinal() < physical_ordinal
        {
            RuntimeTimeoutVoteEpisodeDisposition::RestoredDescent
        } else if token.scheduler_ordinal() > timeout_ordinal
            && token.admission_ordinal() == physical_ordinal
        {
            RuntimeTimeoutVoteEpisodeDisposition::FreshReplenishment
        } else if token.scheduler_ordinal() == timeout_ordinal {
            return Err(EnqueueError::FailClosed);
        } else {
            return Ok(None);
        };
        let owner = RuntimeTimeoutVoteEpisodeOwner {
            token: token.clone(),
            carrier_physical_ordinal: physical_ordinal,
            disposition,
        };
        if !owner.validate_against(timeout_ordinal, episode.physical_cut) {
            return Err(EnqueueError::FailClosed);
        }
        Ok(Some(RuntimeTimeoutVoteEpisodeCandidate {
            slot: token.slot.clone(),
            owner,
        }))
    }
    /// Project the exact 0→0, 0→1, or 1→1 owner-count transition before
    /// authentication can produce reducer refinement evidence.
    fn timeout_vote_episode_admission_plan(
        &self,
        candidate: Option<RuntimeTimeoutVoteEpisodeCandidate>,
    ) -> Result<RuntimeTimeoutVoteEpisodeAdmissionPlan, EnqueueError> {
        let Some(candidate) = candidate else {
            return Ok(RuntimeTimeoutVoteEpisodeAdmissionPlan::NonCandidate);
        };
        let Some(_) = self.emitted_timeout_recovery_owner()? else {
            return Err(EnqueueError::FailClosed);
        };
        let episode = self
            .timeout_recovery_episode
            .as_ref()
            .ok_or(EnqueueError::FailClosed)?;
        let roster = self
            .driver
            .wire_context()
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let current_universe = roster
            .iter()
            .cloned()
            .map(|semantic_origin| FairV2IngressLeaderWireSlot {
                semantic_origin,
                phase: FairV2IngressLeaderWirePhase::TimeoutVote,
                chunk_index: None,
            })
            .collect::<BTreeSet<_>>();
        if episode.timeout_vote_owner_universe != current_universe
            || candidate.slot.phase != FairV2IngressLeaderWirePhase::TimeoutVote
            || candidate.slot.chunk_index.is_some()
            || !roster.contains(&candidate.slot.semantic_origin)
            || candidate.slot != candidate.owner.token.slot
            || !candidate.owner.validate_against(
                episode.timeout_owner.lifecycle_ordinal(),
                episode.physical_cut,
            )
        {
            return Err(EnqueueError::FailClosed);
        }
        let mut prospective = episode.admitted_timeout_vote_owners.clone();
        let disposition = match prospective.get(&candidate.slot) {
            Some(incumbent) if !incumbent.same_lifecycle_owner_as(&candidate.owner) => {
                return Err(EnqueueError::FailClosed);
            }
            Some(_) => RuntimeTimeoutVoteEpisodeAdmissionPlan::CoalescedRetry {
                candidate: candidate.clone(),
                prospective: prospective.clone(),
            },
            None => {
                prospective.insert(candidate.slot.clone(), candidate.owner.clone());
                RuntimeTimeoutVoteEpisodeAdmissionPlan::FirstAdmission {
                    candidate: candidate.clone(),
                    prospective: prospective.clone(),
                }
            }
        };
        if prospective.len() > roster.len()
            || prospective.iter().any(|(slot, owner)| {
                slot.phase != FairV2IngressLeaderWirePhase::TimeoutVote
                    || slot.chunk_index.is_some()
                    || !roster.contains(&slot.semantic_origin)
                    || slot != &owner.token.slot
                    || !owner.validate_against(
                        episode.timeout_owner.lifecycle_ordinal(),
                        episode.physical_cut,
                    )
            })
        {
            return Err(EnqueueError::FailClosed);
        }
        Ok(disposition)
    }
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

    /// Snapshot an already-decided interrupted tip without arming successor clocks.
    pub(crate) fn pending_kura_activation_status_snapshot(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        if self.clocks_armed {
            return Err(AdapterError::PendingKuraActivationNotReady);
        }
        self.driver.pending_kura_activation_status()
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
    fn resolve_body_pipeline_completion_owner(
        &mut self,
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<Option<RuntimeBodyCompletionOwnershipPlan>, EnqueueError> {
        if !ownership.validate_exact() {
            self.latch_fail_closed("body completion retry omitted exact predecessor ownership");
            return Err(EnqueueError::FailClosed);
        }
        if !self.body_pipeline_completion_is_owned(tag, candidate)? {
            return Ok(None);
        }
        let ingress_retained = match self
            .ingress
            .exact_body_pipeline_completion_owners(tag, candidate)
        {
            Ok(retained) => retained,
            Err(error) => {
                self.latch_fail_closed(
                    "exact ingress body completion lost its runtime lifecycle owner",
                );
                return Err(error);
            }
        };
        let deferred_ordinals = self
            .driver
            .deferred_body_pipeline_completion_exact_owner_ordinals(tag, candidate);
        let retained = match (ingress_retained.as_slice(), deferred_ordinals.as_slice()) {
            ([(owner, statement, target)], []) => (owner.clone(), *statement, *target),
            ([], [ordinal]) => {
                let Some(deferred) = self.deferred_lifecycle_ownership.get(ordinal) else {
                    self.latch_fail_closed(
                        "exact deferred body completion lost its runtime lifecycle owner",
                    );
                    return Err(EnqueueError::FailClosed);
                };
                if deferred.deferred_admission_ordinal != *ordinal
                    || !deferred.validate_active_against_ingress(
                        self.deferred_ingress_ownership.get(ordinal),
                        self.driver.deferred_admission_ordinal_source(),
                    )
                {
                    self.latch_fail_closed(
                        "exact deferred body completion had invalid runtime lifecycle ownership",
                    );
                    return Err(EnqueueError::FailClosed);
                }
                (
                    deferred.owner.clone(),
                    deferred.candidate_semantic_statement,
                    RuntimeBodyCompletionStorageTarget::Deferred(*ordinal),
                )
            }
            _ => {
                self.latch_fail_closed(
                    "coalesced body completion changed its exact lifecycle owner",
                );
                return Err(EnqueueError::FailClosed);
            }
        };
        let (retained_owner, retained_statement, target) = retained;
        // An asynchronous predecessor can publish its terminal completion
        // before a retransmitted effect or a later Prepare/Commit carrier
        // reaches the executor. Live Fetch/Store/Validate work already keeps
        // one physical owner across exact retries; preserve that rule after
        // the result has crossed into queued or Busy-deferred completion
        // storage. The complete completion evidence, exact predecessor kind
        // and body coordinates, and typed authority lattice must all agree,
        // so an unrelated lifecycle or conflicting certificate still fails
        // closed below.
        let incoming_statement = ownership.candidate_semantic_statement();
        if !ownership.binds_body_pipeline_completion_predecessor(candidate) {
            self.latch_fail_closed("body completion retry changed its exact predecessor stage");
            return Err(EnqueueError::FailClosed);
        }
        let Some(relation) = retained_statement
            .zip(incoming_statement)
            .and_then(|(incumbent, incoming)| incumbent.fetch_authority_relation_to(incoming))
        else {
            self.latch_fail_closed("body completion retry changed its exact authority statement");
            return Err(EnqueueError::FailClosed);
        };
        let replacement_statement = match relation {
            RuntimeFetchAuthorityRelation::Upgrade => incoming_statement,
            RuntimeFetchAuthorityRelation::Same => None,
            RuntimeFetchAuthorityRelation::Stale => {
                if retained_owner != *ownership.owner() {
                    self.latch_fail_closed(
                        "coalesced body completion changed its exact lifecycle owner",
                    );
                    return Err(EnqueueError::FailClosed);
                }
                None
            }
        };
        Ok(Some(RuntimeBodyCompletionOwnershipPlan {
            tag,
            candidate: candidate.clone(),
            retained_owner,
            retained_statement,
            target,
            replacement_statement,
        }))
    }
    fn prepare_body_pipeline_completion_refinements(
        &mut self,
        plans: &[RuntimeBodyCompletionOwnershipPlan],
    ) -> Result<RuntimePreparedBodyCompletionRefinements, EnqueueError> {
        let mut prepared = RuntimePreparedBodyCompletionRefinements::default();
        let mut targets = BTreeSet::new();
        for plan in plans {
            let Some(replacement) = plan.replacement_statement else {
                continue;
            };
            let Some(incumbent) = plan.retained_statement else {
                self.latch_fail_closed(
                    "authority upgrade omitted its incumbent body completion statement",
                );
                return Err(EnqueueError::FailClosed);
            };
            if !replacement.validate_exact() || !targets.insert(plan.target) {
                self.latch_fail_closed(
                    "authority upgrade had invalid or duplicate body completion targets",
                );
                return Err(EnqueueError::FailClosed);
            }
            match plan.target {
                RuntimeBodyCompletionStorageTarget::Queued(_)
                | RuntimeBodyCompletionStorageTarget::Reserved(_) => {
                    let matches = match self
                        .ingress
                        .exact_body_pipeline_completion_refinement_matches(
                            plan.tag,
                            &plan.candidate,
                            plan.target,
                            &plan.retained_owner,
                            incumbent,
                        ) {
                        Ok(matches) => matches,
                        Err(error) => {
                            self.latch_fail_closed(
                                "authority upgrade could not validate its ingress completion owner",
                            );
                            return Err(error);
                        }
                    };
                    if !matches {
                        self.latch_fail_closed(
                            "authority upgrade changed its ingress body completion owner",
                        );
                        return Err(EnqueueError::FailClosed);
                    }
                    prepared.ingress.push((plan.target, replacement));
                }
                RuntimeBodyCompletionStorageTarget::Deferred(ordinal) => {
                    if self
                        .driver
                        .deferred_body_pipeline_completion_exact_owner_ordinals(
                            plan.tag,
                            &plan.candidate,
                        )
                        != vec![ordinal]
                    {
                        self.latch_fail_closed(
                            "authority upgrade changed its deferred body completion target",
                        );
                        return Err(EnqueueError::FailClosed);
                    }
                    let Some(existing) = self.deferred_lifecycle_ownership.get(&ordinal) else {
                        self.latch_fail_closed(
                            "authority upgrade lost its deferred body completion owner",
                        );
                        return Err(EnqueueError::FailClosed);
                    };
                    if existing.owner != plan.retained_owner
                        || existing.candidate_semantic_statement != Some(incumbent)
                    {
                        self.latch_fail_closed(
                            "authority upgrade changed its deferred body completion owner",
                        );
                        return Err(EnqueueError::FailClosed);
                    }
                    let upgraded = match existing
                        .clone()
                        .with_candidate_semantic_statement(Some(replacement))
                    {
                        Ok(upgraded) => upgraded,
                        Err(error) => {
                            self.latch_fail_closed(
                                "authority upgrade invalidated its deferred body completion owner",
                            );
                            return Err(error);
                        }
                    };
                    if prepared.deferred.insert(ordinal, upgraded).is_some() {
                        self.latch_fail_closed(
                            "authority upgrade duplicated its deferred body completion owner",
                        );
                        return Err(EnqueueError::FailClosed);
                    }
                }
            }
        }
        Ok(prepared)
    }
    fn commit_prepared_body_pipeline_completion_refinements(
        &mut self,
        prepared: RuntimePreparedBodyCompletionRefinements,
    ) -> Result<(), EnqueueError> {
        let ingress_targets_exist = prepared.ingress.iter().all(|(target, _)| match target {
            RuntimeBodyCompletionStorageTarget::Queued(admission_ordinal) => self
                .ingress
                .commands
                .iter()
                .any(|queued| queued.admission_ordinal == Some(*admission_ordinal)),
            RuntimeBodyCompletionStorageTarget::Reserved(admission_ordinal) => self
                .ingress
                .reserved_body_available
                .as_ref()
                .is_some_and(|reservation| {
                    reservation.admission_ordinal == Some(*admission_ordinal)
                }),
            RuntimeBodyCompletionStorageTarget::Deferred(_) => false,
        });
        if !ingress_targets_exist
            || !prepared
                .deferred
                .keys()
                .all(|ordinal| self.deferred_lifecycle_ownership.contains_key(ordinal))
        {
            self.latch_fail_closed(
                "prevalidated authority upgrade lost its body completion target",
            );
            return Err(EnqueueError::FailClosed);
        }
        // Every target was validated above and the serialized runtime admits
        // only vacant batch keys before this assignment-only commit tail.
        for (target, replacement) in prepared.ingress {
            match target {
                RuntimeBodyCompletionStorageTarget::Queued(admission_ordinal) => {
                    self.ingress
                        .commands
                        .iter_mut()
                        .find(|queued| queued.admission_ordinal == Some(admission_ordinal))
                        .expect("prevalidated queued body completion target remains serialized")
                        .candidate_semantic_statement = Some(replacement);
                }
                RuntimeBodyCompletionStorageTarget::Reserved(admission_ordinal) => {
                    let reservation = self
                        .ingress
                        .reserved_body_available
                        .as_mut()
                        .expect("prevalidated body reservation remains serialized");
                    debug_assert_eq!(reservation.admission_ordinal, Some(admission_ordinal));
                    reservation.candidate_semantic_statement = Some(replacement);
                }
                RuntimeBodyCompletionStorageTarget::Deferred(_) => {
                    unreachable!("deferred completion targets use the deferred refinement commit")
                }
            }
        }
        for (ordinal, replacement) in prepared.deferred {
            *self
                .deferred_lifecycle_ownership
                .get_mut(&ordinal)
                .expect("prevalidated deferred body completion target remains serialized") =
                replacement;
        }
        Ok(())
    }
    fn body_pipeline_completion_predecessor(
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> Option<AdapterEffect> {
        match candidate {
            BodyPipelineCompletionEvidence::BodyStored { round, subject, .. } => {
                Some(AdapterEffect::StoreBody {
                    tag,
                    round: *round,
                    subject: *subject,
                })
            }
            BodyPipelineCompletionEvidence::LocalProposalReady {
                manifest: wire::PayloadManifest { round, subject, .. },
                ..
            } => Some(AdapterEffect::ValidateBody {
                tag,
                round: *round,
                subject: *subject,
            }),
            BodyPipelineCompletionEvidence::BodyAvailable { .. } => None,
        }
    }
    /// Resolve the sole serialized terminal for an exact Store/Validate
    /// candidate without committing an authority refinement.
    fn body_pipeline_candidate_terminal_ownership_plan(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<Option<RuntimeBodyCompletionOwnershipPlan>, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 body terminal query entered a closed runtime".to_owned());
        }
        if !matches!(
            effect,
            AdapterEffect::StoreBody { .. } | AdapterEffect::ValidateBody { .. }
        ) {
            return Ok(None);
        }
        if !ownership.exactly_binds_adapter_effect(effect) {
            self.latch_fail_closed("body terminal query omitted its exact predecessor capability");
            return Err("Sumeragi v2 body terminal query failed closed".to_owned());
        }
        let mut candidates = self
            .ingress
            .commands
            .iter()
            .filter_map(|queued| {
                let evidence = queued.command.body_pipeline_completion_evidence()?;
                ownership
                    .exactly_authorizes_body_pipeline_successor(effect, queued.tag, &evidence)
                    .then_some((queued.tag, evidence))
            })
            .collect::<Vec<_>>();
        for (_, tag, evidence) in self.driver.deferred_body_pipeline_terminal_candidates() {
            if !ownership.exactly_authorizes_body_pipeline_successor(effect, tag, &evidence) {
                continue;
            }
            candidates.push((tag, evidence));
        }
        let [(tag, candidate)] = candidates.as_slice() else {
            return if candidates.is_empty() {
                Ok(None)
            } else {
                self.latch_fail_closed(
                    "one body candidate retained multiple terminal completion owners",
                );
                Err("Sumeragi v2 body candidate has duplicate terminal owners".to_owned())
            };
        };
        let plan = self
            .resolve_body_pipeline_completion_owner(*tag, candidate, ownership)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                self.latch_fail_closed("body terminal query lost its serialized completion owner");
                "Sumeragi v2 body terminal query lost ownership".to_owned()
            })?;
        Ok(Some(plan))
    }
    /// Plan one terminal Store/Validate retry under its immutable incumbent
    /// owner. This is a read-only success path: the returned binding can be
    /// passed through the complete effect/candidate refinement gate before a
    /// stronger compatible authority statement is committed.
    pub(crate) fn plan_body_pipeline_candidate_terminal(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<Option<RuntimeEffectOwnership>, String> {
        self.body_pipeline_candidate_terminal_ownership_plan(effect, ownership)?
            .map(|plan| plan.adopt_effect_ownership(effect, ownership))
            .transpose()
    }
    /// Commit previously checked terminal authority refinements atomically.
    ///
    /// The serialized runtime cannot advance between plan and commit. The
    /// second exact lookups nevertheless revalidate every retained owner and
    /// target. All refinements are prepared before the assignment-only commit
    /// tail, so a malformed later effect cannot partially refine an earlier
    /// terminal in the same adapter macro-step.
    pub(crate) fn commit_body_pipeline_candidate_terminals(
        &mut self,
        terminals: &[(&AdapterEffect, &RuntimeEffectOwnership)],
    ) -> Result<(), String> {
        let mut plans = Vec::with_capacity(terminals.len());
        for (effect, ownership) in terminals {
            let plan = self
                .body_pipeline_candidate_terminal_ownership_plan(effect, ownership)?
                .ok_or_else(|| {
                    self.latch_fail_closed(
                        "body terminal refinement lost its serialized completion owner",
                    );
                    "Sumeragi v2 body terminal refinement lost ownership".to_owned()
                })?;
            plans.push(plan);
        }
        let prepared = self
            .prepare_body_pipeline_completion_refinements(&plans)
            .map_err(|error| error.to_string())?;
        self.commit_prepared_body_pipeline_completion_refinements(prepared)
            .map_err(|error| error.to_string())
    }
    /// Commit one source-audited terminal refinement through the atomic batch API.
    #[allow(dead_code)]
    pub(crate) fn commit_body_pipeline_candidate_terminal(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), String> {
        self.commit_body_pipeline_candidate_terminals(&[(effect, ownership)])
    }
    /// Return whether the runtime already owns the terminal completion for an
    /// exact Store/Validate candidate, committing an authority upgrade only
    /// after producing the incumbent-owner plan. Executor production uses the
    /// explicit plan/commit pair so its total positional refinement gate sits
    /// between these two operations.
    #[cfg(test)]
    pub(crate) fn body_pipeline_candidate_has_terminal(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<bool, String> {
        let Some(adopted) = self.plan_body_pipeline_candidate_terminal(effect, ownership)? else {
            return Ok(false);
        };
        self.commit_body_pipeline_candidate_terminal(effect, &adopted)?;
        Ok(true)
    }
    fn body_pipeline_completion_is_owned_by(
        &mut self,
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<
        Option<(
            RuntimeLifecycleOwner,
            Option<RuntimeCandidateSemanticStatement>,
        )>,
        EnqueueError,
    > {
        let Some(plan) = self.resolve_body_pipeline_completion_owner(tag, candidate, ownership)?
        else {
            return Ok(None);
        };
        let result = (plan.retained_owner.clone(), plan.effective_statement());
        let prepared =
            self.prepare_body_pipeline_completion_refinements(std::slice::from_ref(&plan))?;
        self.commit_prepared_body_pipeline_completion_refinements(prepared)?;
        Ok(Some(result))
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
        let Some(predecessor) = Self::body_pipeline_completion_predecessor(tag, &evidence) else {
            self.latch_fail_closed("owned body completion omitted its exact predecessor stage");
            return Err(EnqueueError::FailClosed);
        };
        if !ownership.exactly_authorizes_body_pipeline_successor(&predecessor, tag, &evidence) {
            self.latch_fail_closed("owned body completion changed its exact predecessor stage");
            return Err(EnqueueError::FailClosed);
        }
        if self
            .body_pipeline_completion_is_owned_by(tag, &evidence, ownership)?
            .is_some()
        {
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
    /// Return whether the exact body owner is the process carrier for a
    /// persistent adapter producer reservation.
    ///
    /// Runtime ingress represents a restart-restored stage-7 parent directly;
    /// a Busy-deferred owner represents it through the adapter reservation
    /// attached to that exact admission ordinal. The two representations may
    /// never coexist for one serialized owner.
    fn body_available_has_persistent_producer(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        let ingress = match self
            .ingress
            .restored_body_available_retirement(tag, |queued| queued == manifest)
        {
            Ok(retirement) => retirement.is_some(),
            Err(error) => {
                self.latch_fail_closed(
                    "body completion carried corrupt restored producer metadata",
                );
                return Err(error.to_string());
            }
        };
        let deferred = match self
            .driver
            .deferred_body_available_has_persistent_producer(tag, manifest)
        {
            Ok(persistent) => persistent,
            Err(error) => {
                let error = error.to_string();
                self.latch_fail_closed(
                    "body completion lost its exact deferred producer reservation",
                );
                return Err(format!(
                    "Sumeragi v2 deferred body producer validation failed: {error}"
                ));
            }
        };
        if ingress && deferred {
            self.latch_fail_closed("one body completion retained two persistent producer carriers");
            return Err(
                "Sumeragi v2 body completion has duplicate persistent producer ownership"
                    .to_owned(),
            );
        }
        Ok(ingress || deferred)
    }
    fn retire_restored_body_producer(
        &mut self,
        retirement: Option<RestoredProducerRetirement>,
    ) -> Result<(), String> {
        let Some(retirement) = retirement else {
            return Ok(());
        };
        match self.driver.retire_restored_producer_continuation(
            retirement.causal_lifecycle_key,
            retirement.admission_ordinal,
            retirement.producer_stage,
        ) {
            Ok(true) => Ok(()),
            Ok(false) => {
                self.latch_fail_closed(
                    "restored body completion retirement lost its durable producer owner",
                );
                Err(
                    "Sumeragi v2 restored body completion has no exact durable producer owner"
                        .to_owned(),
                )
            }
            Err(error) => {
                let error = error.to_string();
                self.latch_fail_closed(
                    "restored body completion retirement could not persist producer release",
                );
                Err(format!(
                    "Sumeragi v2 restored body producer retirement failed: {error}"
                ))
            }
        }
    }
    /// Take exclusive ownership of an opened adapter and preserve its recovery
    /// effects for immediate asynchronous dispatch.
    #[cfg_attr(not(test), allow(dead_code))]
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
    /// Return the complete durable Prepare/Commit authority for the retained body.
    pub(crate) fn replayed_body_authority_certificate(
        &self,
    ) -> Result<Option<wire::QuorumCertificate>, AdapterError> {
        self.driver.replayed_body_authority_certificate()
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
            .recover_validated_body(manifest, validated_receipt)?;
        #[cfg(test)]
        self.recovered_validated_body_bindings
            .insert((manifest.round, manifest.subject));
        Ok(())
    }

    /// Whether startup routed one exact durable validation marker into the driver.
    #[cfg(test)]
    pub(crate) fn recovered_validated_body_was_bound_for_test(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
    ) -> bool {
        self.recovered_validated_body_bindings.contains(&key)
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
        if !ingress_ownership.validate_exact() {
            self.latch_fail_closed(
                "network ingress changed its authenticated fair-queue ownership",
            );
            return Err(NetworkIngressError::FailClosed);
        }
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
        let timeout_vote_recovery_candidate = match self
            .timeout_vote_recovery_candidate_from_runtime(
                authenticated.payload(),
                &ingress_ownership,
            ) {
            Ok(candidate) => candidate,
            Err(_) => {
                self.latch_fail_closed(
                    "TimeoutVote recovery lost its exact finite episode authority",
                );
                return Err(NetworkIngressError::FailClosed);
            }
        };
        let timeout_vote_admission_plan =
            match self.timeout_vote_episode_admission_plan(timeout_vote_recovery_candidate) {
                Ok(plan) => plan,
                Err(_) => {
                    self.latch_fail_closed(
                        "TimeoutVote recovery attempted to replace a frozen source owner",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
            };
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
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
                unreachable!("handled above")
            }
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        let strict_timeout_recovery_replay = if wire_payload_is_direct_certificate_recovery_shape(
            authenticated.payload(),
        )
            && wire_payload_matches_current_strict_timeout_recovery_round(
                authenticated.payload(),
                self.driver.wire_context(),
                self.round_tag,
            ) {
            match ingress_ownership.is_physical_leader_wire_replay() {
                Ok(is_replay) => is_replay,
                Err(_) => {
                    self.latch_fail_closed(
                        "timeout-recovery leader-wire ingress changed its physical replay ownership",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
            }
        } else {
            false
        };
        let clock_occurrence = match (
            ingress_ownership.earliest_lifecycle_ordinal(),
            ingress_ownership.earliest_physical_carrier(),
        ) {
            (Ok(Some(lifecycle_ordinal)), Ok(Some(physical))) => {
                Some((lifecycle_ordinal, physical.source_ordinal))
            }
            (Ok(None), Ok(Some(_))) => None,
            (Ok(_), Ok(None)) | (Err(_), _) | (_, Err(_)) => {
                self.latch_fail_closed(
                    "network replay observed invalid clock reservation ownership",
                );
                return Err(NetworkIngressError::FailClosed);
            }
        };
        // Ordinary post-cut replays and fresh attempts remain on their
        // fair-ingress carrier. A restored TC or CommitQC may enter the
        // certified prefix while the absolute timeout still owns the cut;
        // scheduler arbitration nevertheless runs that timeout first. A
        // TimeoutVote instead belongs to a separate finite episode: pre-cut
        // owners descend, while at most one first post-cut owner per frozen
        // roster slot replenishes the vote count. Neither class gains
        // certified capacity or signature-fence authority.
        if let Some((lifecycle_ordinal, source_physical_ordinal)) = clock_occurrence {
            match self.clock_owner_reservation_blockers_occurrence(
                lifecycle_ordinal,
                source_physical_ordinal,
            ) {
                Ok(blockers) => {
                    let certified_timeout_escape = blockers.timeout_only()
                        && strict_timeout_recovery_replay
                        && !matches!(
                            authenticated.payload(),
                            wire::ConsensusMessageV2Payload::TimeoutVote(_)
                        );
                    let timeout_vote_episode_escape = if timeout_vote_admission_plan.is_candidate()
                    {
                        match self.timeout_recovery_episode_allows_clock_blockers(blockers) {
                            Ok(allows) => allows,
                            Err(_) => {
                                self.latch_fail_closed(
                                    "TimeoutVote recovery observed invalid clock-episode ownership",
                                );
                                return Err(NetworkIngressError::FailClosed);
                            }
                        }
                    } else {
                        false
                    };
                    if blockers.any() && !certified_timeout_escape && !timeout_vote_episode_escape {
                        // Preserve the exact fair-ingress occurrence outside
                        // the FIFO until its frozen predecessor transfers.
                        // Returning ordinary backpressure keeps retries
                        // coalesced on the same transport carrier and allocates
                        // no new position.
                        return Err(NetworkIngressError::Backpressure(EnqueueError::Full));
                    }
                }
                Err(_) => {
                    self.latch_fail_closed(
                        "network replay observed invalid clock reservation ownership",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
            }
        }
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
                if let Some(prospective) = timeout_vote_admission_plan.prospective() {
                    let Some(episode) = self.timeout_recovery_episode.as_mut() else {
                        self.latch_fail_closed(
                            "TimeoutVote recovery admission lost its finite episode",
                        );
                        return Err(NetworkIngressError::FailClosed);
                    };
                    episode.admitted_timeout_vote_owners = prospective;
                    if !episode.validate_exact() {
                        self.latch_fail_closed(
                            "TimeoutVote recovery exceeded its frozen roster universe",
                        );
                        return Err(NetworkIngressError::FailClosed);
                    }
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
        let mut admitted = super::fair_v2_ingress_admit_for_test(
            super::InboundBlockMessage::from_authenticated_peer(
                super::message::BlockMessage::V2(message.clone()),
                super::authenticated_peer_for_test(),
            ),
        );
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
        // generic runtime identity permits the absent receipt and physical cut
        // only for this read-only probe, while mutating admission still
        // requires the dequeue-frozen pair.
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
        if self.fail_closed {
            return Some(false);
        }
        if let Some((round, _)) = self
            .driver
            .wire_ingress_missing_execution_commitment(&runtime_message.payload)
            && round.height == self.round_tag.height()
            && round.view == self.round_tag.view()
        {
            return Some(false);
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
        let Some(source_physical_ordinal) = ownership.physical_admission_ordinal() else {
            return Some(true);
        };
        match self.clock_owner_reservation_blockers_occurrence(
            token.scheduler_ordinal(),
            source_physical_ordinal,
        ) {
            Ok(blockers)
                if blockers.timeout_only()
                    && wire_payload_is_direct_certificate_recovery_shape(
                        &runtime_message.payload,
                    )
                    && !matches!(
                        &runtime_message.payload,
                        wire::ConsensusMessageV2Payload::TimeoutVote(_)
                    )
                    && wire_payload_matches_current_strict_timeout_recovery_round(
                        &runtime_message.payload,
                        self.driver.wire_context(),
                        self.round_tag,
                    )
                    && token.admission_ordinal() < source_physical_ordinal =>
            {
                // A restored direct certificate keeps its immutable pre-crash
                // token while its new physical carrier is strictly later.
                // Admit that exact replay across only the absolute timeout
                // cut; timeout still runs first.
            }
            Ok(blockers)
                if matches!(
                    &runtime_message.payload,
                    wire::ConsensusMessageV2Payload::TimeoutVote(_)
                ) && matches!(
                    self.timeout_vote_recovery_candidate_from_fair(
                        &runtime_message.payload,
                        ownership,
                    )
                    .and_then(|candidate| self.timeout_vote_episode_admission_plan(candidate))
                    .map(|plan| plan.is_candidate()),
                    Ok(true)
                ) && matches!(
                    self.timeout_recovery_episode_allows_clock_blockers(blockers),
                    Ok(true)
                ) => {}
            Ok(blockers) if blockers.any() => return Some(false),
            Ok(_) => {}
            // Drain malformed process-local state so the mutating seam can
            // expose the invariant failure instead of pinning a fair lane.
            Err(_) => return Some(true),
        }
        let may_use_progress = self
            .driver
            .wire_ingress_may_use_progress(&runtime_message.payload);
        let capacity = if wire_payload_is_certified_fence_escape(&runtime_message.payload) {
            self.ingress.check_certified_fence_escape_capacity()
        } else {
            match self.ingress.check_capacity(default_class) {
                Ok(()) => Ok(()),
                Err(_) if may_use_progress => self.ingress.check_capacity(CommandClass::Progress),
                Err(error) => Err(error),
            }
        };
        Some(capacity.is_ok())
    }
    /// Whether one pre-runtime fair-ingress head belongs to the finite
    /// TimeoutVote episode needed to close a timeout cycle.
    ///
    /// This predicate is intentionally narrower than ordinary network
    /// admission. It never authenticates or dequeues the message and never
    /// consumes certified-fence capacity. It accepts a frozen owner below the
    /// timeout cut (descent) or the first post-cut owner of one authenticated
    /// roster source (finite replenishment). The mutating seam still performs
    /// full authentication after the checked dequeue.
    pub(crate) fn can_admit_timeout_vote_recovery_episode(
        &self,
        message: &wire::ConsensusMessageV2,
        ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        if !matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::TimeoutVote(_)
        ) || !wire_payload_matches_current_strict_timeout_recovery_round(
            &message.payload,
            self.driver.wire_context(),
            self.round_tag,
        ) {
            return false;
        }
        let Some(token) = ownership.leader_wire_token() else {
            return false;
        };
        let outer = super::message::BlockMessage::V2(message.clone());
        let Some(source_physical_ordinal) = ownership.physical_admission_ordinal() else {
            return false;
        };
        if ownership.leader_wire_runtime_receipt().is_some()
            || !ownership.validate_exact()
            || !ownership.matches_message(&outer)
            || ownership.runtime_physical_cut().is_some()
            || ownership.runtime_lifecycle_ordinal() != Some(token.scheduler_ordinal())
            || !self
                .ingress
                .lifecycle_ordinals
                .recognizes_minted(token.scheduler_ordinal())
                .unwrap_or(false)
        {
            return false;
        }
        if !matches!(
            self.timeout_vote_recovery_candidate_from_fair(&message.payload, ownership)
                .and_then(|candidate| self.timeout_vote_episode_admission_plan(candidate)),
            Ok(plan) if plan.count_transition() != (0, 0)
        ) {
            return false;
        }
        let Ok(blockers) = self.clock_owner_reservation_blockers_occurrence(
            token.scheduler_ordinal(),
            source_physical_ordinal,
        ) else {
            return false;
        };
        if !matches!(
            self.timeout_recovery_episode_allows_clock_blockers(blockers),
            Ok(true)
        ) {
            return false;
        }
        self.can_admit_pre_runtime_leader_wire(message, message, CommandClass::Progress, ownership)
            == Some(true)
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
        if let Some(admissible) = self.can_admit_pre_runtime_leader_wire(
            outer_message,
            &runtime_message,
            default_class,
            ingress_ownership,
        ) {
            return admissible;
        }
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
        if let (Ok(Some(lifecycle_ordinal)), Ok(Some(physical))) = (
            ownership.earliest_lifecycle_ordinal(),
            ownership.earliest_physical_carrier(),
        ) {
            match self.clock_owner_reservation_blocks_occurrence(
                lifecycle_ordinal,
                physical.source_ordinal,
            ) {
                Ok(true) => return false,
                Ok(false) => {}
                // Let the mutating seam consume malformed state and latch
                // fail-closed instead of pinning the fair-ingress head.
                Err(_) => return true,
            }
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
        let mut admitted = super::fair_v2_ingress_admit_for_test(
            super::InboundBlockMessage::from_authenticated_peer(
                super::message::BlockMessage::V2(message.clone()),
                super::authenticated_peer_for_test(),
            ),
        );
        let ownership = admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact ownership");
        self.can_admit_network_message_with_ingress_ownership(message, &ownership)
    }
    /// Enqueue successful canonical reconstruction with the exact fetch tag.
    ///
    /// Authenticated proposals already waiting in the FIFO are discarded only
    /// when they advertise a different manifest for this exact round and
    /// subject. Every retained command keeps its original relative order, and
    /// the completion is appended normally.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn enqueue_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        let reservation = self.reserve_body_available(tag, manifest)?;
        self.commit_body_available(reservation)
    }
    /// Install an ordinary volatile body owner without consulting adapter
    /// restart metadata, for crash-cut coalescence tests only.
    #[cfg(test)]
    pub(crate) fn enqueue_volatile_body_available_for_test(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        self.ingress.enqueue_canonical_body_available(tag, manifest)
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
        if already_owned {
            if self.ingress.reserved_body_available.is_none() {
                return Ok(BodyAvailableReservation::coalesced(tag, manifest));
            }
            let result = self
                .ingress
                .reserve_canonical_body_available_internal(tag, manifest, None, None, None);
            if matches!(
                result,
                Err(EnqueueError::FailClosed | EnqueueError::DuplicateCompletionOwnership)
            ) {
                self.latch_fail_closed("body-available reservation ownership validation failed");
            }
            return result;
        }
        let command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let preflight =
            self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
        let restored_owner = match preflight {
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
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
            None,
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
        let command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        if self.ingress.reserved_body_available.is_some() {
            // An unpublished reservation is already the sole physical
            // Completion owner.  In particular, `EnterView` may have moved
            // that token to a later consumer tag while retaining its original
            // lifecycle owner.  Reclaim the exact token before consulting the
            // reducer's current admission projection: the view transition may
            // legitimately change that projection, but it cannot remint or
            // replace the already charged slot.
            if !self.body_pipeline_completion_is_owned(tag, &evidence)? {
                self.latch_fail_closed(
                    "owned body-available retry differed from its unpublished exact owner",
                );
                return Err(EnqueueError::DuplicateCompletionOwnership);
            }
            let existing = self
                .ingress
                .reserved_body_available
                .as_ref()
                .expect("unpublished body owner remains serialized")
                .clone();
            let exact_retry = (|| -> Result<bool, EnqueueError> {
                let physical_admission_ordinal =
                    existing.admission_ordinal.ok_or(EnqueueError::FailClosed)?;
                let lifecycle_ordinal =
                    existing.lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
                let retained_owner = existing.lifecycle_owner().ok_or(EnqueueError::FailClosed)?;
                let restored = existing.restored_producer_stage.is_some();
                Ok(existing.tag == tag
                    && existing.manifest == manifest
                    && existing.owns_new_slot
                    && existing.candidate_semantic_statement
                        == ownership.candidate_semantic_statement()
                    && existing
                        .restored_producer_stage
                        .is_none_or(RuntimeDormantLocalFifoReservation::is_known_stage)
                    && (!restored || ownership.binds_exact_fetch_body_manifest(&manifest))
                    && (&retained_owner == ownership.owner() || restored)
                    && self
                        .ingress
                        .lifecycle_ordinals
                        .recognizes_minted(physical_admission_ordinal)
                        .map_err(|_| EnqueueError::FailClosed)?
                    && self
                        .ingress
                        .lifecycle_ordinals
                        .recognizes_minted(lifecycle_ordinal)
                        .map_err(|_| EnqueueError::FailClosed)?
                    && existing
                        .dormant_replacement
                        .as_ref()
                        .is_none_or(|replacement| {
                            self.ingress
                                .dormant_local_fifo_reservations
                                .contains(replacement)
                        }))
            })();
            match exact_retry {
                Ok(true) => return Ok(existing),
                Ok(false) => self.latch_fail_closed(
                    "owned body-available retry changed its unpublished exact owner",
                ),
                Err(error) => {
                    self.latch_fail_closed(
                        "owned body-available retry lost its unpublished exact owner",
                    );
                    return Err(error);
                }
            }
            return Err(EnqueueError::DuplicateCompletionOwnership);
        }
        if let Some((retained_owner, retained_statement)) =
            self.body_pipeline_completion_is_owned_by(tag, &evidence, ownership)?
        {
            return BodyAvailableReservation::coalesced_with_lifecycle_owner(
                tag,
                manifest,
                retained_owner,
                retained_statement,
            );
        }
        let preflight =
            self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
        if self.owned_preflight_is_coalesced(tag, &command, preflight, ownership)? {
            return BodyAvailableReservation::coalesced_with_owner(tag, manifest, ownership);
        }
        let restored_owner = match preflight {
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
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
                self.latch_fail_closed(
                    "unpublished body-available owner disagreed with adapter preflight",
                );
                return Err(EnqueueError::FailClosed);
            }
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        let candidate_statement = ownership.candidate_semantic_statement();
        // A crash can replace the volatile physical FetchBody carrier with a
        // later certified or differently routed fetch. Its lifecycle owner is
        // therefore allowed to differ from the persisted stage-7 successor.
        // The exact Fetch kind and frozen body coordinates authorize that
        // bridge; the completion itself continues under the restored owner.
        if restored_owner.is_some() && !ownership.binds_exact_fetch_body_manifest(&manifest) {
            self.latch_fail_closed(
                "restored body-available retry changed its frozen candidate coordinates",
            );
            return Err(EnqueueError::FailClosed);
        }
        let owner = restored_owner
            .as_ref()
            .map_or_else(|| ownership.owner(), |(owner, _)| owner);
        let result = self.ingress.reserve_canonical_body_available_internal(
            tag,
            manifest,
            Some(owner),
            candidate_statement,
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
    /// mutated. When one exact destination already exists, the sole persistent
    /// producer carrier survives and the ordinary volatile twin retires; a
    /// persistent source is then retagged to `rebound`. Conflicting evidence,
    /// duplicate ownership, or two independent persistent roots fail closed
    /// before mutation. Success leaves exactly one full-evidence owner at
    /// `rebound`.
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
            let source_persistent =
                self.body_available_has_persistent_producer(previous, manifest)?;
            let destination_persistent =
                self.body_available_has_persistent_producer(rebound, manifest)?;
            if source_persistent && destination_persistent {
                self.latch_fail_closed(
                    "body completion coalescence found two independent persistent producers",
                );
                return Err(
                    "Sumeragi v2 body completion has two persistent producer roots".to_owned(),
                );
            }
            if source_persistent {
                // Keep the sole crash-recoverable producer. Retiring the
                // ordinary destination first is crash-safe: until the final
                // in-memory retag, the durable source remains recoverable at
                // its previous certified coordinates.
                if !self.retire_body_available(rebound, manifest)? {
                    self.latch_fail_closed(
                        "body completion coalescence lost its volatile destination owner",
                    );
                    return Err(
                        "Sumeragi v2 body completion destination disappeared during coalescence"
                            .to_owned(),
                    );
                }
                let ingress = self
                    .ingress
                    .rebind_canonical_body_available(previous, rebound, manifest);
                let deferred = self
                    .driver
                    .rebind_deferred_body_available(previous, rebound, manifest);
                ingress.saturating_add(deferred)
            } else {
                // The destination is at least as crash-recoverable as the
                // source. Persist any deferred or restored source release
                // before removing its volatile queue occurrence.
                let deferred = match self
                    .driver
                    .retire_deferred_body_available(previous, manifest)
                {
                    Ok(retired) => retired,
                    Err(error) => {
                        let error = error.to_string();
                        self.latch_fail_closed(
                            "body completion rebind could not persist deferred producer release",
                        );
                        return Err(format!(
                            "Sumeragi v2 deferred body producer retirement failed: {error}"
                        ));
                    }
                };
                let restored = match self
                    .ingress
                    .restored_body_available_retirement(previous, |queued| queued == manifest)
                {
                    Ok(restored) => restored,
                    Err(error) => {
                        self.latch_fail_closed(
                            "body completion rebind found corrupt restored producer metadata",
                        );
                        return Err(error.to_string());
                    }
                };
                self.retire_restored_body_producer(restored)?;
                let ingress = self
                    .ingress
                    .retire_canonical_body_available(previous, manifest);
                ingress.saturating_add(deferred)
            }
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
    /// Transfer the sole unpublished exact-body completion owned by a
    /// protected fetch to its certified successor incarnation.
    ///
    /// A certified response supplies the canonical manifest only after the
    /// fetch task has been created, so the task can legitimately carry no
    /// manifest of its own. Match the runtime-owned token by its immutable
    /// consensus coordinates, then delegate the exact evidence and duplicate
    /// checks to [`Self::rebind_body_available`]. The delegated transfer edits
    /// only the tag and therefore preserves the physical admission ordinal,
    /// lifecycle owner, and any restart-dormant backing.
    pub(crate) fn rebind_unpublished_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        if !rebound.strictly_advances(previous) {
            return Err(
                "Sumeragi v2 unpublished body completion rebind did not advance its incarnation"
                    .to_owned(),
            );
        }
        if rebound != self.round_tag {
            return Err(
                "Sumeragi v2 unpublished body completion target is not the installed runtime incarnation"
                    .to_owned(),
            );
        }
        let Some(manifest) = self
            .ingress
            .reserved_body_available
            .as_ref()
            .filter(|reservation| {
                reservation.tag == previous
                    && reservation.manifest.round == round
                    && reservation.manifest.subject == subject
            })
            .map(|reservation| reservation.manifest.clone())
        else {
            return Ok(false);
        };
        if self.body_available_is_uniquely_owned(rebound, &manifest)? {
            self.latch_fail_closed(
                "unpublished body completion rebind found a second destination owner",
            );
            return Err(
                "Sumeragi v2 unpublished body completion already has a distinct destination owner"
                    .to_owned(),
            );
        }
        self.rebind_body_available(previous, rebound, &manifest)
    }
    /// Retire the sole unpublished exact-body completion for a fetch which no
    /// longer belongs to the installed safety frontier.
    ///
    /// Match by the fetch's immutable consensus coordinates, recover the full
    /// manifest from the runtime-owned token, and use the exact retirement
    /// path so duplicate owners fail closed before capacity or dormant restart
    /// backing changes.
    pub(crate) fn retire_unpublished_body_available(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let Some(manifest) = self
            .ingress
            .reserved_body_available
            .as_ref()
            .filter(|reservation| {
                reservation.tag == tag
                    && reservation.manifest.round == round
                    && reservation.manifest.subject == subject
            })
            .map(|reservation| reservation.manifest.clone())
        else {
            return Ok(false);
        };
        self.retire_body_available(tag, &manifest)
    }
    /// Retire the restart-dormant stage-7 parent of an exact body fetch which
    /// became terminal before reserving any `BodyAvailable` token.
    ///
    /// The effect binding proves the fetch coordinates. Because restart gives
    /// the reconstructed fetch a fresh physical lifecycle, the adapter—not
    /// that volatile owner—resolves those coordinates against its sole
    /// persisted dormant stage-7 producer. Ordinary same-process fetches have
    /// no such record and are an explicit no-op.
    pub(crate) fn retire_restored_body_fetch_parent(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let AdapterEffect::FetchBody {
            round,
            subject,
            manifest,
            ..
        } = effect
        else {
            self.latch_fail_closed(
                "restored body-fetch retirement changed its exact effect binding",
            );
            return Err(
                "Sumeragi v2 restored body-fetch retirement has invalid ownership".to_owned(),
            );
        };
        if !ownership.exactly_binds_adapter_effect(effect) {
            self.latch_fail_closed(
                "restored body-fetch retirement changed its exact effect binding",
            );
            return Err(
                "Sumeragi v2 restored body-fetch retirement has invalid ownership".to_owned(),
            );
        }
        match self
            .driver
            .retire_restored_body_fetch_parent(*round, *subject, manifest.as_ref())
        {
            Ok(retired) => Ok(retired),
            Err(error) => {
                let error = error.to_string();
                self.latch_fail_closed(
                    "restored body-fetch retirement could not persist its producer release",
                );
                Err(format!(
                    "Sumeragi v2 restored body-fetch producer retirement failed: {error}"
                ))
            }
        }
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
        let restored = match self
            .ingress
            .restored_body_available_retirement(tag, |queued| queued == manifest)
        {
            Ok(restored) => restored,
            Err(error) => {
                self.latch_fail_closed(
                    "body completion retirement found corrupt restored producer metadata",
                );
                return Err(error.to_string());
            }
        };
        let deferred = match self.driver.retire_deferred_body_available(tag, manifest) {
            Ok(retired) => retired,
            Err(error) => {
                let error = error.to_string();
                self.latch_fail_closed(
                    "body completion retirement could not persist deferred producer release",
                );
                return Err(format!(
                    "Sumeragi v2 deferred body producer retirement failed: {error}"
                ));
            }
        };
        self.retire_restored_body_producer(restored)?;
        let ingress = self.ingress.retire_canonical_body_available(tag, manifest);
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
        let restored = match self
            .ingress
            .restored_body_available_retirement(tag, |manifest| {
                manifest.round == round && manifest.subject == subject
            }) {
            Ok(restored) => restored,
            Err(error) => {
                self.latch_fail_closed(
                    "body pipeline retirement found corrupt restored producer metadata",
                );
                return Err(error.to_string());
            }
        };
        let deferred = match self
            .driver
            .retire_deferred_body_pipeline_completions(tag, round, subject)
        {
            Ok(retired) => retired,
            Err(error) => {
                let error = error.to_string();
                self.latch_fail_closed(
                    "body pipeline retirement could not persist deferred producer release",
                );
                return Err(format!(
                    "Sumeragi v2 deferred body producer retirement failed: {error}"
                ));
            }
        };
        self.retire_restored_body_producer(restored)?;
        let ingress = self
            .ingress
            .retire_body_pipeline_completions(tag, round, subject);
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
        if let Err(error) = self.driver.retire_deferred_proposal_work_after_decision(
            decision_tag,
            decision_round,
            decision_subject,
            decision_commitment,
        ) {
            let error = error.to_string();
            self.latch_fail_closed(
                "decided proposal retirement could not persist deferred producer release",
            );
            return Err(format!(
                "Sumeragi v2 deferred proposal producer retirement failed: {error}"
            ));
        }
        self.ingress.retire_proposal_work_after_decision(
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
        let deferred = match self
            .driver
            .retire_deferred_unsafe_proposals_for_lock(locked_round, locked_subject)
        {
            Ok(retired) => retired,
            Err(error) => {
                let error = error.to_string();
                self.latch_fail_closed(
                    "unsafe proposal retirement could not persist deferred producer release",
                );
                return Err(format!(
                    "Sumeragi v2 deferred proposal producer retirement failed: {error}"
                ));
            }
        };
        let ingress = self
            .ingress
            .retire_unsafe_proposals_for_lock(locked_round, locked_subject);
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
include!("v2_runtime/network_ingress_classification.rs");
#[cfg(test)]
mod tests {
    include!("tests/v2_runtime_pending_binding_cases.rs");
    include!("tests/v2_runtime_main_00.rs");
    include!("tests/v2_runtime_main_01.rs");
    include!("tests/v2_runtime_main_02.rs");
    include!("tests/v2_runtime_main_03.rs");
    include!("tests/v2_runtime_main_04.rs");
    include!("tests/v2_runtime_main_05.rs");
    include!("tests/v2_runtime_main_06.rs");
    include!("tests/v2_runtime_unsealed_01b_lifecycle_bounds.rs");
    include!("tests/v2_runtime_unsealed_02_owner_retirement_and_fairness.rs");
}
