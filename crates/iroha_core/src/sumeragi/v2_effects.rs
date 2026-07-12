//! Fail-closed execution boundary for Sumeragi v2 reducer effects.
//!
//! [`SerializedV2Runtime`] is the only owner of consensus state. This module
//! does not select leaders, count votes, form certificates, change views, or
//! decide blocks. It turns each [`AdapterEffect`] into explicit work at the
//! networking, signing, exact-body, deterministic-validation, application,
//! status, and evidence boundaries, then returns completions to the runtime
//! with the exact [`EventTag`] which created that work.
//!
//! The caller must explicitly select the exact-body signature policy: the
//! configured genesis authority at height one or the context's rotating leader
//! thereafter. The executor forwards that policy to the body store and still
//! routes full semantic block validation through the deterministic validator;
//! it never invents a second block-authorization rule.
//!
//! Exact-body fsync, canonical decoding, and deterministic validation execute
//! as tagged asynchronous tasks. Only [`V2BodyStore`] can mint their completion
//! receipts, so networking code cannot acknowledge durability or validity.
//!
//! # Worker integration contract
//!
//! 1. Open the adapter/runtime, then call [`V2EffectExecutor::open`]. Move the
//!    returned [`V2BodyStore`] to the storage/validation service thread. If
//!    recovery reported an interrupted canonical Kura tip, call
//!    [`V2EffectExecutor::verify_pending_kura_apply_replay`] before dispatching
//!    startup effects or opening ingress.
//! 2. Route control envelopes through [`V2EffectExecutor::enqueue_network`]
//!    and payload traffic through the authenticated chunk/certified-response
//!    methods in this module.
//! 3. Repeatedly call [`V2EffectExecutor::step`] and execute every task handed
//!    to [`V2EffectServices`].
//! 4. Execute [`BodyStoreTask`] and [`BodyValidationTask`] only through
//!    [`V2BodyStore::execute_store_task`] and
//!    [`V2BodyStore::execute_validation_task`], then return their minted
//!    completions to the matching executor completion methods. The production
//!    validation callback is
//!    `ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block`; legacy
//!    validation rejects valid empty heartbeat blocks and is not interchangeable.
//! 5. Return application durability with
//!    [`V2EffectExecutor::complete_application`].
//! 6. After [`V2EffectExecutor::ready_to_finish`], consume the executor with
//!    [`V2EffectExecutor::into_finalized_parts`]. This returns the runtime,
//!    typed Kura receipt, and exact finality artifact together so rollover
//!    cannot accidentally discard either durability proof.
//!
//! # Finalized rollover cleanup
//!
//! Kura receipt/finality validation is the fail-closed commit boundary. After
//! that boundary, obsolete WAL, body, chunk, and worker resources are retired
//! on a best-effort basis. [`PostFinalityCleanupOutcome`] retains every typed
//! cleanup warning in execution order; callers must report the warnings but
//! must not reinterpret an already durable decision as unfinalized.
//! Retained files remain replay-safe and never turn a durable decision back
//! into an unfinalized height.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::Path,
    sync::Arc,
    time::Instant,
};

use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    block::{BlockHeader, CertifiedMergeLedgerReference, consensus_v2 as wire},
    merge::MergeLedgerEntry,
    peer::PeerId,
};

use super::{
    v2::{AdapterEffect, SignRequest},
    v2_body_store::{
        BlockSignaturePolicy, BodyStoreCompletion, BodyValidationCompletion, DurableBodyReceipt,
        V2BodyStore, ValidatedBodyReceipt,
    },
    v2_core::EventTag,
    v2_recovery::PendingKuraApply,
    v2_runtime::{EnqueueError, NetworkIngressError, RuntimeStep, SerializedV2Runtime},
    v2_transport::{
        AuthenticatedCertifiedBodyRequest, AuthenticatedPayloadChunk,
        OutstandingCertifiedBodyRequests, V2TransportError, authenticate_certified_body_request,
        authenticate_payload_chunk,
    },
};
use crate::kura::KuraV2CommitReceipt;

/// Stable identifier for one asynchronous effect invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EffectWorkId(u64);

impl EffectWorkId {
    /// Numeric identifier useful for operational correlation.
    pub(crate) const fn get(self) -> u64 {
        self.0
    }

    /// Construct a stable identifier for cross-module unit fixtures.
    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

/// Explicit bounds for outstanding effect work and reconstructed bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EffectQueueConfig {
    max_pending_work: usize,
    max_ready_bodies: usize,
    max_ready_body_bytes: u64,
    max_certified_requests: usize,
}

impl EffectQueueConfig {
    /// Construct a queue allocation. All limits must be non-zero.
    pub(crate) const fn new(
        max_pending_work: usize,
        max_ready_bodies: usize,
        max_ready_body_bytes: u64,
        max_certified_requests: usize,
    ) -> Self {
        Self {
            max_pending_work,
            max_ready_bodies,
            max_ready_body_bytes,
            max_certified_requests,
        }
    }

    fn validate(self) -> Result<Self, EffectExecutorError> {
        if self.max_pending_work == 0
            || self.max_ready_bodies == 0
            || self.max_ready_body_bytes == 0
            || self.max_certified_requests == 0
        {
            return Err(EffectExecutorError::InvalidQueueConfig);
        }
        Ok(self)
    }
}

impl Default for EffectQueueConfig {
    fn default() -> Self {
        Self::new(1_024, 64, 256 * 1024 * 1024, 256)
    }
}

/// Asynchronous request to sign one already-durable consensus intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConsensusSignTask {
    id: EffectWorkId,
    tag: EventTag,
    request: SignRequest,
}

impl ConsensusSignTask {
    /// Construct an exact signing task for service-boundary unit tests.
    #[cfg(test)]
    pub(crate) const fn for_test(id: u64, tag: EventTag, request: SignRequest) -> Self {
        Self {
            id: EffectWorkId(id),
            tag,
            request,
        }
    }

    /// Work identifier which must accompany the signature completion.
    pub(crate) const fn id(&self) -> EffectWorkId {
        self.id
    }

    /// Original reducer incarnation tag.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }

    /// Canonical unsigned message.
    pub(crate) const fn request(&self) -> &SignRequest {
        &self.request
    }
}

/// Body reconstruction or certified-fetch request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BodyFetchTask {
    id: EffectWorkId,
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest: Option<wire::PayloadManifest>,
    sources: Vec<PeerId>,
    certified_request: Option<wire::CertifiedBodyRequest>,
}

impl BodyFetchTask {
    /// Work identifier used by chunk and reconstruction callbacks.
    pub(crate) const fn id(&self) -> EffectWorkId {
        self.id
    }

    /// Manifest known before reconstruction, if any.
    pub(crate) const fn manifest(&self) -> Option<&wire::PayloadManifest> {
        self.manifest.as_ref()
    }

    /// Certified signers selected as fetch sources.
    pub(crate) fn sources(&self) -> &[PeerId] {
        &self.sources
    }

    /// Exact signed request for a certified fetch.
    pub(crate) const fn certified_request(&self) -> Option<&wire::CertifiedBodyRequest> {
        self.certified_request.as_ref()
    }
}

/// Tagged exact-body persistence work executed outside the reducer owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BodyStoreTask {
    id: EffectWorkId,
    tag: EventTag,
    manifest: wire::PayloadManifest,
    canonical_wire: Arc<[u8]>,
}

impl BodyStoreTask {
    /// Stable work identifier reused by every retry.
    pub(crate) const fn id(&self) -> EffectWorkId {
        self.id
    }

    /// Original reducer event tag.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }

    /// Exact canonical manifest stored beside the body.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }

    /// Exact canonical `SignedBlockWire` bytes.
    pub(crate) fn canonical_wire(&self) -> &[u8] {
        &self.canonical_wire
    }
}

/// Tagged deterministic-validation work for one exact durable body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BodyValidationTask {
    id: EffectWorkId,
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    durable_receipt: DurableBodyReceipt,
}

impl BodyValidationTask {
    /// Construct exact deterministic-validation work for body-store boundary tests.
    #[cfg(test)]
    pub(crate) const fn for_test(
        id: u64,
        tag: EventTag,
        durable_receipt: DurableBodyReceipt,
    ) -> Self {
        Self {
            id: EffectWorkId(id),
            tag,
            round: durable_receipt.round(),
            subject: durable_receipt.subject(),
            durable_receipt,
        }
    }

    /// Stable work identifier reused by every retry.
    pub(crate) const fn id(&self) -> EffectWorkId {
        self.id
    }

    /// Original reducer event tag.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }

    /// Exact proposal round.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        self.round
    }

    /// Exact proposal subject.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    /// Non-forgeable durable receipt whose body must be reloaded.
    pub(crate) const fn durable_receipt(&self) -> &DurableBodyReceipt {
        &self.durable_receipt
    }
}

/// Application request for an exact durable, validated decided block.
#[derive(Clone, Debug)]
pub(crate) struct ApplyTask {
    id: EffectWorkId,
    tag: EventTag,
    subject: wire::BlockSubject,
    certificate: wire::QuorumCertificate,
    validated_receipt: ValidatedBodyReceipt,
}

impl ApplyTask {
    /// Construct an exact application task for crash-boundary unit tests.
    #[cfg(test)]
    pub(crate) const fn for_test(
        id: u64,
        tag: EventTag,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Self {
        Self {
            id: EffectWorkId(id),
            tag,
            subject,
            certificate,
            validated_receipt,
        }
    }

    /// Work identifier which must accompany the durable Kura completion.
    pub(crate) const fn id(&self) -> EffectWorkId {
        self.id
    }

    /// Original reducer incarnation tag.
    #[cfg(test)]
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }

    /// Exact decided subject.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }

    /// CommitQC authorizing application.
    pub(crate) const fn certificate(&self) -> &wire::QuorumCertificate {
        &self.certificate
    }

    /// Non-forgeable receipt identifying the exact durable validated body.
    pub(crate) const fn validated_receipt(&self) -> &ValidatedBodyReceipt {
        &self.validated_receipt
    }
}

/// Typed completion proving Kura durably stored the exact block/finality pair.
#[derive(Debug)]
pub(crate) struct DurableApplyCompletion {
    work_id: EffectWorkId,
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
}

impl DurableApplyCompletion {
    /// Bind Kura's non-forgeable receipt and canonical artifact to an apply task.
    pub(crate) const fn new(
        work_id: EffectWorkId,
        receipt: KuraV2CommitReceipt,
        artifact: wire::finality::V2FinalityArtifact,
    ) -> Self {
        Self {
            work_id,
            receipt,
            artifact,
        }
    }
}

/// Operational status of the effect boundary, excluding consensus state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectExecutorStatus {
    /// Whether an internal boundary failure permanently stopped execution.
    pub fail_closed: bool,
    /// First fatal diagnostic, retained until process restart.
    pub fatal_reason: Option<String>,
    /// Outstanding signing operations.
    pub pending_signatures: usize,
    /// Outstanding body reconstruction/fetch operations.
    pub pending_fetches: usize,
    /// Outstanding exact-body persistence operations.
    pub pending_stores: usize,
    /// Outstanding deterministic-validation operations.
    pub pending_validations: usize,
    /// Validation or application operations waiting for an exact merge sidecar.
    pub deferred_merge_work: usize,
    /// Outstanding durable application operations.
    pub pending_applications: usize,
    /// Reconstructed bodies waiting for the reducer's StoreBody effect.
    pub ready_bodies: usize,
    /// Total exact bytes retained for reconstructed bodies.
    pub ready_body_bytes: u64,
    /// Total exact bytes retained by pending store tasks.
    pub pending_store_bytes: u64,
    /// Runtime completions queued for serialized reducer delivery.
    pub queued_runtime_completions: usize,
}

/// Production callbacks used to perform effects outside the reducer owner.
///
/// Queueing methods must either retain the complete task or return an error;
/// silently dropping an accepted task violates the adapter contract. The
/// executor verifies returned consensus and transport signatures before
/// allowing them to reach the reducer or network.
pub(crate) trait V2EffectServices {
    /// Adapter-specific failure type.
    type Error: fmt::Display;

    /// Queue one control-message signing task.
    fn enqueue_consensus_sign(&mut self, task: ConsensusSignTask) -> Result<(), Self::Error>;
    /// Cancel signing work made stale by a certified view transition.
    fn cancel_consensus_sign(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error>;
    /// Broadcast one canonical v2 consensus envelope to every voting validator.
    fn broadcast_consensus(&mut self, message: wire::ConsensusMessageV2)
    -> Result<(), Self::Error>;
    /// Sign a certified-body request with the requester's transport identity.
    fn sign_body_request(&mut self, preimage: &[u8]) -> Result<Vec<u8>, Self::Error>;
    /// Start or retransmit body reconstruction/fetch. Repeated tasks with the
    /// same work identifier are idempotent retransmission requests, not new
    /// work. Authenticated chunks are delivered separately through
    /// [`Self::accept_authenticated_chunk`].
    fn enqueue_body_fetch(&mut self, task: BodyFetchTask) -> Result<(), Self::Error>;
    /// Cancel reconstruction work made stale by a certified view transition.
    fn cancel_body_fetch(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error>;
    /// Hand one structurally, cryptographically, and outer-peer authenticated
    /// chunk to the persistent chunk/reconstruction adapter.
    fn accept_authenticated_chunk(
        &mut self,
        work_id: EffectWorkId,
        chunk: AuthenticatedPayloadChunk,
    ) -> Result<(), Self::Error>;
    /// Queue or retransmit exact-body persistence. Repeated task identifiers
    /// refer to the same immutable bytes.
    fn enqueue_body_store(&mut self, task: BodyStoreTask) -> Result<(), Self::Error>;
    /// Cancel body persistence made stale before a certified view transition.
    fn cancel_body_store(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error>;
    /// Queue or retransmit deterministic validation of one exact durable body.
    fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error>;
    /// Retain a bounded request for the exact certified merge sidecar which
    /// must be authenticated before validation or decided application retries.
    fn work_deferred_for_merge_sidecar(
        &mut self,
        work_id: EffectWorkId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reference: &CertifiedMergeLedgerReference,
    ) -> Result<(), Self::Error>;
    /// Queue exact decision application and Kura finality persistence. A
    /// repeated task identifier requests an idempotent retry of the same
    /// durable operation.
    fn enqueue_apply(&mut self, task: ApplyTask) -> Result<(), Self::Error>;
    /// Observe a reducer-authorized view installation for timer/status wiring.
    fn entered_view(
        &mut self,
        tag: EventTag,
        certificate: wire::TimeoutCertificate,
    ) -> Result<(), Self::Error>;
    /// Persist or publish equivocation evidence.
    fn report_equivocation(
        &mut self,
        evidence: wire::SumeragiV2Equivocation,
    ) -> Result<(), Self::Error>;
    /// Persist or publish certified-invalid-body evidence.
    fn report_invalid_certified_body(
        &mut self,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
    ) -> Result<(), Self::Error>;
    /// Observe a deterministic validation rejection for local diagnostics.
    fn validation_rejected(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reason: &str,
    );
    /// Publish bounded executor operational status.
    fn publish_effect_status(&mut self, status: &EffectExecutorStatus) -> Result<(), Self::Error>;
    /// Best-effort notification after the executor permanently fails closed.
    fn fail_closed(&mut self, reason: &str);
}

/// Result of accepting an asynchronous completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompletionDisposition {
    /// The exact tagged completion entered the serialized runtime FIFO.
    Enqueued,
    /// Validation remains pending until its exact certified merge sidecar is
    /// fetched, authenticated, and installed for a deterministic retry.
    Deferred,
    /// The work identifier was already completed or belongs to an old owner.
    Stale,
}

/// Local resource whose retirement failed after Kura-authorized finality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PostFinalityCleanupTarget {
    /// Reducer safety WAL for the finalized height.
    SafetyWal,
    /// Exact durable body files for the finalized height.
    DurableBodies,
    /// Reconstructed payload chunk files for the finalized height.
    PayloadChunks,
    /// Ordered I/O worker lifecycle or protocol state.
    CleanupWorker,
}

impl PostFinalityCleanupTarget {
    /// Stable operational label for structured diagnostics.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SafetyWal => "safety_wal",
            Self::DurableBodies => "durable_bodies",
            Self::PayloadChunks => "payload_chunks",
            Self::CleanupWorker => "cleanup_worker",
        }
    }
}

/// One explicit local-cleanup diagnostic after durable finality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PostFinalityCleanupWarning {
    target: PostFinalityCleanupTarget,
    reason: String,
}

impl PostFinalityCleanupWarning {
    /// Resource whose cleanup failed.
    pub(crate) const fn target(&self) -> PostFinalityCleanupTarget {
        self.target
    }

    /// Exact diagnostic returned by the local cleanup operation.
    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
}

/// Ordered partial-success report for cleanup after Kura-authorized finality.
///
/// An empty outcome means all local cleanup completed. Warnings never alter
/// the finalized block or successor context and are kept in deterministic
/// cleanup order so operators receive every available diagnostic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PostFinalityCleanupOutcome {
    warnings: Vec<PostFinalityCleanupWarning>,
}

impl PostFinalityCleanupOutcome {
    /// Record one cleanup warning without discarding earlier diagnostics.
    pub(crate) fn record(&mut self, target: PostFinalityCleanupTarget, reason: impl Into<String>) {
        self.warnings.push(PostFinalityCleanupWarning {
            target,
            reason: reason.into(),
        });
    }

    /// Append a later cleanup stage while preserving execution order.
    pub(crate) fn append(&mut self, mut later: Self) {
        self.warnings.append(&mut later.warnings);
    }

    /// Borrow all retained cleanup diagnostics in execution order.
    pub(crate) fn warnings(&self) -> &[PostFinalityCleanupWarning] {
        &self.warnings
    }
}

/// Effect-executor construction, contract, durability, or service failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EffectExecutorError {
    /// One or more explicit queue bounds were zero.
    InvalidQueueConfig,
    /// Work identifiers exhausted their integer representation.
    WorkIdExhausted,
    /// Outstanding work would exceed the configured fixed bound.
    PendingWorkCapacity { capacity: usize },
    /// Outstanding exact certified-body requests would exceed their bound.
    CertifiedRequestCapacity { capacity: usize },
    /// Reconstructed body retention would exceed its entry or byte bound.
    ReadyBodyCapacity,
    /// An effect contradicted prior effect-boundary state.
    Contract(String),
    /// The serialized runtime rejected a step or completion.
    Runtime(String),
    /// A body-store durability/integrity operation failed.
    BodyStore(String),
    /// An external production adapter failed to retain or execute work.
    Service(String),
    /// A returned consensus signature was malformed, used the wrong key, or
    /// did not authenticate the exact requested preimage.
    InvalidConsensusSignature(String),
    /// Kura's typed receipt/artifact differed from the exact Apply effect.
    InvalidApplyCompletion,
    /// WAL/body replay did not bind an interrupted canonical Kura tip to one
    /// exact durable Decision and validation marker.
    PendingApplyRecoveryMismatch(String),
    /// Height rollover was requested before the reducer and executor drained.
    NotReadyToFinish,
    /// Executor already stopped after an earlier fatal error.
    FailClosed(String),
}

impl fmt::Display for EffectExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQueueConfig => {
                f.write_str("Sumeragi v2 effect queue limits must be non-zero")
            }
            Self::WorkIdExhausted => f.write_str("Sumeragi v2 effect work identifiers exhausted"),
            Self::PendingWorkCapacity { capacity } => write!(
                f,
                "Sumeragi v2 pending effect-work capacity {capacity} is exhausted"
            ),
            Self::CertifiedRequestCapacity { capacity } => write!(
                f,
                "Sumeragi v2 certified-body request capacity {capacity} is exhausted"
            ),
            Self::ReadyBodyCapacity => {
                f.write_str("Sumeragi v2 reconstructed-body capacity is exhausted")
            }
            Self::Contract(reason) => write!(f, "Sumeragi v2 effect contract failed: {reason}"),
            Self::Runtime(reason) => write!(f, "Sumeragi v2 serialized runtime failed: {reason}"),
            Self::BodyStore(reason) => write!(f, "Sumeragi v2 exact-body store failed: {reason}"),
            Self::Service(reason) => write!(f, "Sumeragi v2 effect service failed: {reason}"),
            Self::InvalidConsensusSignature(reason) => {
                write!(f, "invalid Sumeragi v2 signer completion: {reason}")
            }
            Self::InvalidApplyCompletion => f.write_str(
                "Sumeragi v2 durable application receipt does not match the exact decision",
            ),
            Self::PendingApplyRecoveryMismatch(reason) => {
                write!(f, "Sumeragi v2 pending-apply recovery mismatch: {reason}")
            }
            Self::NotReadyToFinish => {
                f.write_str("Sumeragi v2 height is not ready for finalized rollover")
            }
            Self::FailClosed(reason) => {
                write!(f, "Sumeragi v2 effect executor is fail-closed: {reason}")
            }
        }
    }
}

impl std::error::Error for EffectExecutorError {}

/// Rejection of unauthenticated, unsolicited, stale, or mismatched transport
/// data. These errors do not by themselves close consensus execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EffectTransportError {
    /// Executor is already permanently closed.
    FailClosed(String),
    /// No matching fetch remains outstanding.
    UnknownWork(EffectWorkId),
    /// The callback kind does not match ordinary versus certified fetch state.
    WrongFetchKind,
    /// Chunk/request/response authentication failed.
    Authentication(V2TransportError),
    /// Reconstructed manifest or exact body differs from the requested subject.
    BodyMismatch,
    /// Reconstructed-body retention is currently full.
    Backpressure,
}

impl fmt::Display for EffectTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FailClosed(reason) => {
                write!(f, "Sumeragi v2 effect executor is fail-closed: {reason}")
            }
            Self::UnknownWork(id) => {
                write!(f, "unknown or stale Sumeragi v2 fetch work {}", id.get())
            }
            Self::WrongFetchKind => {
                f.write_str("Sumeragi v2 fetch completion used the wrong transport path")
            }
            Self::Authentication(error) => write!(f, "{error}"),
            Self::BodyMismatch => {
                f.write_str("Sumeragi v2 reconstructed body does not match its fetch")
            }
            Self::Backpressure => f.write_str("Sumeragi v2 reconstructed-body queue is full"),
        }
    }
}

impl std::error::Error for EffectTransportError {}

impl From<V2TransportError> for EffectTransportError {
    fn from(error: V2TransportError) -> Self {
        Self::Authentication(error)
    }
}

#[derive(Clone, Debug)]
struct PendingSignature {
    tag: EventTag,
    request: SignRequest,
}

#[derive(Clone, Debug)]
struct PendingFetch {
    task: BodyFetchTask,
    request_hash: Option<HashOf<wire::CertifiedBodyRequest>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StorePurpose {
    Reducer,
    LocalProposal,
}

#[derive(Clone, Debug)]
struct PendingStore {
    task: BodyStoreTask,
    purpose: StorePurpose,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValidationPurpose {
    Reducer,
    LocalProposal { manifest: wire::PayloadManifest },
}

#[derive(Clone, Debug)]
struct PendingValidation {
    task: BodyValidationTask,
    purpose: ValidationPurpose,
}

#[derive(Clone, Debug)]
struct PendingApply {
    task: ApplyTask,
}

#[derive(Clone, Debug)]
struct ReadyBody {
    manifest: wire::PayloadManifest,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct FinalityCompletion {
    receipt: KuraV2CommitReceipt,
    artifact: wire::finality::V2FinalityArtifact,
}

pub(crate) trait EffectRuntime {
    fn step_effects(&mut self, now: Instant) -> Result<RuntimeStep<AdapterEffect>, String>;
    fn enqueue_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError>;
    fn enqueue_body_stored(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    ) -> Result<(), EnqueueError>;
    fn enqueue_validation_succeeded(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError>;
    fn enqueue_validation_failed(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError>;
    fn enqueue_signature(&mut self, tag: EventTag, signature: Vec<u8>) -> Result<(), EnqueueError>;
    fn enqueue_application_completed(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError>;
    fn enqueue_local_proposal(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError>;
    fn verify_certificate(
        &self,
        context: &wire::HeightContext,
        certificate: &wire::QuorumCertificate,
    ) -> Result<(), String>;
    fn queued_commands(&self) -> usize;
}

impl EffectRuntime for SerializedV2Runtime {
    fn step_effects(&mut self, now: Instant) -> Result<RuntimeStep<AdapterEffect>, String> {
        self.step(now).map_err(|error| error.to_string())
    }

    fn enqueue_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_body_available(self, tag, manifest)
    }

    fn enqueue_body_stored(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_body_stored(self, tag, round, subject, receipt)
    }

    fn enqueue_validation_succeeded(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_validation_succeeded(self, tag, round, subject, receipt)
    }

    fn enqueue_validation_failed(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_validation_failed(self, tag, round, subject)
    }

    fn enqueue_signature(&mut self, tag: EventTag, signature: Vec<u8>) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_signature(self, tag, signature)
    }

    fn enqueue_application_completed(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_application_completed(self, tag, subject)
    }

    fn enqueue_local_proposal(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        SerializedV2Runtime::enqueue_local_proposal(
            self,
            tag,
            manifest,
            durable_receipt,
            validated_receipt,
        )
    }

    fn verify_certificate(
        &self,
        _context: &wire::HeightContext,
        certificate: &wire::QuorumCertificate,
    ) -> Result<(), String> {
        let message = wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone()),
        );
        self.driver()
            .authenticate(message)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn queued_commands(&self) -> usize {
        SerializedV2Runtime::queued_commands(self)
    }
}

/// One-owner executor which binds runtime effects to production adapters.
pub(crate) struct V2EffectExecutor<R = SerializedV2Runtime> {
    runtime: R,
    recovered_bodies: BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    context: wire::HeightContext,
    requester: PeerId,
    local_validator: Option<wire::ValidatorIndex>,
    config: EffectQueueConfig,
    next_work_id: u64,
    pending_signatures: BTreeMap<EffectWorkId, PendingSignature>,
    pending_fetches: BTreeMap<EffectWorkId, PendingFetch>,
    pending_stores: BTreeMap<EffectWorkId, PendingStore>,
    pending_validations: BTreeMap<EffectWorkId, PendingValidation>,
    deferred_merge_work: BTreeMap<EffectWorkId, HashOf<MergeLedgerEntry>>,
    pending_applications: BTreeMap<EffectWorkId, PendingApply>,
    certified_work: BTreeMap<HashOf<wire::CertifiedBodyRequest>, EffectWorkId>,
    outstanding_requests: OutstandingCertifiedBodyRequests,
    ready_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ReadyBody>,
    ready_body_bytes: u64,
    pending_store_bytes: u64,
    durable_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    validated_bodies: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    finality_completion: Option<FinalityCompletion>,
    fatal_reason: Option<String>,
}

impl V2EffectExecutor<SerializedV2Runtime> {
    /// Open the exact-body store under an explicit signature-authority policy
    /// and take ownership of the serialized runtime.
    pub(crate) fn open(
        runtime: SerializedV2Runtime,
        body_store_root: impl AsRef<Path>,
        context: wire::HeightContext,
        requester: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        signature_policy: BlockSignaturePolicy,
        config: EffectQueueConfig,
    ) -> Result<(Self, V2BodyStore), EffectExecutorError> {
        let body_store =
            V2BodyStore::open_with_policy(body_store_root, context.clone(), signature_policy)
                .map_err(|error| EffectExecutorError::BodyStore(error.to_string()))?;
        let recovered_bodies = body_store
            .recovery_catalog()
            .map_err(|error| EffectExecutorError::BodyStore(error.to_string()))?;
        let recovered_validations = body_store.validated_recovery_catalog();
        let mut executor = Self::with_runtime(
            runtime,
            recovered_bodies,
            context,
            requester,
            local_validator,
            config,
        )?;
        executor.validated_bodies = recovered_validations;
        Ok((executor, body_store))
    }

    /// Bind an interrupted Kura tip to the exact reducer Decision and durable
    /// validation marker reconstructed before network ingress opens.
    ///
    /// This must be called immediately after [`Self::open`] whenever recovery
    /// returns a [`PendingKuraApply`]. A missing Decision, a different block,
    /// or absent exact body/validation durability fails closed before the
    /// startup effects can be dispatched.
    pub(crate) fn verify_pending_kura_apply_replay(
        &self,
        expected: PendingKuraApply,
    ) -> Result<(), EffectExecutorError> {
        let decision = self.runtime.replayed_decision_key().map_err(|error| {
            EffectExecutorError::PendingApplyRecoveryMismatch(error.to_string())
        })?;
        verify_pending_kura_apply_parts(
            &self.context,
            decision,
            &self.recovered_bodies,
            &self.validated_bodies,
            expected,
        )
    }

    /// Authenticate and enqueue one reducer-directed v2 network message.
    pub(crate) fn enqueue_network(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<EventTag, NetworkIngressError> {
        if self.fatal_reason.is_some() {
            return Err(NetworkIngressError::FailClosed);
        }
        self.runtime.enqueue_network(message)
    }

    /// Borrow the immutable context governing this executor height.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
    }

    /// Return the exact reducer incarnation currently owning timers and work.
    pub(crate) const fn current_tag(&self) -> EventTag {
        self.runtime.round_tag()
    }

    /// Snapshot the reducer-owned leader/lock constraint for the local
    /// candidate assembler without exposing mutable consensus state.
    pub(crate) fn local_proposal_directive(
        &self,
    ) -> Result<super::v2::LocalProposalDirective, EffectExecutorError> {
        self.ensure_open()?;
        self.runtime
            .local_proposal_directive()
            .map_err(|error| EffectExecutorError::Runtime(error.to_string()))
    }

    /// Authenticate a certified-body request through the same production
    /// certificate verifier used for reducer ingress.
    ///
    /// Serving code receives an opaque authenticated token, never a merely
    /// structural request, so an attacker cannot use a forged QC to read or
    /// amplify retained consensus bodies.
    pub(crate) fn authenticate_certified_body_request(
        &self,
        request: wire::CertifiedBodyRequest,
        authenticated_requester: &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError> {
        authenticate_certified_body_request(
            &self.context,
            request,
            authenticated_requester,
            |context, certificate| self.runtime.verify_certificate(context, certificate),
        )
    }

    /// Whether application completion has drained through the reducer and the
    /// height is ready for the explicit rollover transaction.
    pub(crate) fn ready_to_finish(&self) -> bool {
        self.finality_completion.is_some()
            && self.runtime.queued_commands() == 0
            && self.runtime.driver().ready_to_finish()
    }

    /// Consume a completely drained height without separating its runtime from
    /// the typed Kura durability evidence required by rollover.
    pub(crate) fn into_finalized_parts(
        self,
    ) -> Result<
        (
            SerializedV2Runtime,
            KuraV2CommitReceipt,
            wire::finality::V2FinalityArtifact,
        ),
        EffectExecutorError,
    > {
        if !self.ready_to_finish() {
            return Err(EffectExecutorError::NotReadyToFinish);
        }
        let finality = self
            .finality_completion
            .expect("ready executor has durable finality");
        Ok((self.runtime, finality.receipt, finality.artifact))
    }
}

impl<R: EffectRuntime> V2EffectExecutor<R> {
    fn with_runtime(
        runtime: R,
        recovered_bodies: BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            (wire::PayloadManifest, DurableBodyReceipt),
        >,
        context: wire::HeightContext,
        requester: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        config: EffectQueueConfig,
    ) -> Result<Self, EffectExecutorError> {
        context
            .validate()
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        if let Some(index) = local_validator {
            if usize::try_from(index)
                .ok()
                .is_none_or(|index| index >= context.roster.len())
            {
                return Err(EffectExecutorError::Contract(
                    "local validator index is outside the frozen roster".to_owned(),
                ));
            }
        }
        let config = config.validate()?;
        let outstanding_requests =
            OutstandingCertifiedBodyRequests::new(config.max_certified_requests)
                .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        Ok(Self {
            runtime,
            recovered_bodies,
            context,
            requester,
            local_validator,
            config,
            next_work_id: 0,
            pending_signatures: BTreeMap::new(),
            pending_fetches: BTreeMap::new(),
            pending_stores: BTreeMap::new(),
            pending_validations: BTreeMap::new(),
            deferred_merge_work: BTreeMap::new(),
            pending_applications: BTreeMap::new(),
            certified_work: BTreeMap::new(),
            outstanding_requests,
            ready_bodies: BTreeMap::new(),
            ready_body_bytes: 0,
            pending_store_bytes: 0,
            durable_bodies: BTreeMap::new(),
            validated_bodies: BTreeMap::new(),
            finality_completion: None,
            fatal_reason: None,
        })
    }

    /// Consume startup or reducer effects in their exact emitted order.
    pub(crate) fn consume_effects<S: V2EffectServices>(
        &mut self,
        effects: Vec<AdapterEffect>,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let count = effects.len();
        for effect in effects {
            if let Err(error) = self.consume_one(effect, services) {
                return Err(self.close(error, services));
            }
        }
        if let Err(error) = self.publish_status(services) {
            return Err(self.close(error, services));
        }
        Ok(count)
    }

    /// Run at most one serialized runtime step and dispatch all of its effects.
    pub(crate) fn step<S: V2EffectServices>(
        &mut self,
        now: Instant,
        services: &mut S,
    ) -> Result<EffectExecutorStep, EffectExecutorError> {
        self.ensure_open()?;
        let step = match self.runtime.step_effects(now) {
            Ok(step) => step,
            Err(reason) => {
                return Err(self.close(EffectExecutorError::Runtime(reason), services));
            }
        };
        match step {
            RuntimeStep::Idle => {
                if let Err(error) = self.publish_status(services) {
                    return Err(self.close(error, services));
                }
                Ok(EffectExecutorStep::Idle)
            }
            RuntimeStep::Advanced(effects) => {
                let count = self.consume_effects(effects, services)?;
                Ok(EffectExecutorStep::Advanced { effects: count })
            }
        }
    }

    /// Begin the asynchronous durable-store → deterministic-validation chain
    /// for a locally built proposal.
    pub(crate) fn admit_local_proposal<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        canonical_wire: Vec<u8>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        manifest
            .validate(&self.context)
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        if u64::try_from(canonical_wire.len()).ok() != Some(manifest.payload_size_bytes)
            || Hash::new(&canonical_wire) != manifest.subject.payload_hash
        {
            return Err(EffectExecutorError::Contract(
                "local proposal bytes do not match the canonical manifest".to_owned(),
            ));
        }
        if let Err(error) = self.begin_store(
            tag,
            manifest,
            Arc::from(canonical_wire),
            StorePurpose::LocalProposal,
            services,
        ) {
            return Err(self.close(error, services));
        }
        self.publish_status(services)
            .map_err(|error| self.close(error, services))
    }

    /// Verify and enqueue a signature completion under the exact originating tag.
    pub(crate) fn complete_consensus_signature<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        signature: Vec<u8>,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        let Some(pending) = self.pending_signatures.get(&work_id) else {
            return Ok(CompletionDisposition::Stale);
        };
        if let Err(reason) = verify_signer_completion(
            &self.context,
            self.local_validator,
            &pending.request,
            &signature,
        ) {
            return Err(self.close(
                EffectExecutorError::InvalidConsensusSignature(reason),
                services,
            ));
        }
        let tag = pending.tag;
        if let Err(error) = self.runtime.enqueue_signature(tag, signature) {
            return Err(self.close(runtime_enqueue_error(error), services));
        }
        self.pending_signatures.remove(&work_id);
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Enqueued)
    }

    /// Accept a body-store-minted durable completion under its original tag.
    pub(crate) fn complete_body_store<S: V2EffectServices>(
        &mut self,
        completion: BodyStoreCompletion,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        let manifest = completion.manifest().clone();
        let receipt = completion.receipt().clone();
        if !store_completion_matches(&self.context, &manifest, &receipt) {
            return Err(self.close(
                EffectExecutorError::BodyStore(
                    "body-store completion does not match its exact durable receipt".to_owned(),
                ),
                services,
            ));
        }
        let key = (manifest.round, manifest.subject);
        let Some(pending) = self.pending_stores.get(&completion.work_id()).cloned() else {
            self.recovered_bodies
                .insert(key, (manifest, receipt.clone()));
            self.durable_bodies.insert(key, receipt);
            return Ok(CompletionDisposition::Stale);
        };
        if completion.tag() != pending.task.tag()
            || pending.task.manifest() != &manifest
            || pending.task.id() != completion.work_id()
        {
            return Err(self.close(
                EffectExecutorError::BodyStore(
                    "body-store completion differs from its pending tagged task".to_owned(),
                ),
                services,
            ));
        }
        self.pending_stores.remove(&completion.work_id());
        let stored_bytes = u64::try_from(pending.task.canonical_wire.len()).map_err(|_| {
            self.close(
                EffectExecutorError::Contract(
                    "pending-store byte count is not representable".to_owned(),
                ),
                services,
            )
        })?;
        self.pending_store_bytes = self
            .pending_store_bytes
            .checked_sub(stored_bytes)
            .ok_or_else(|| {
                self.close(
                    EffectExecutorError::Contract(
                        "pending-store byte accounting underflow".to_owned(),
                    ),
                    services,
                )
            })?;
        self.recovered_bodies
            .insert(key, (manifest.clone(), receipt.clone()));
        self.durable_bodies.insert(key, receipt.clone());
        let result = match pending.purpose {
            StorePurpose::Reducer => self
                .runtime
                .enqueue_body_stored(
                    pending.task.tag(),
                    manifest.round,
                    manifest.subject,
                    receipt,
                )
                .map_err(runtime_enqueue_error),
            StorePurpose::LocalProposal => self.begin_validation(
                pending.task.tag(),
                manifest.round,
                manifest.subject,
                receipt,
                ValidationPurpose::LocalProposal { manifest },
                services,
            ),
        };
        if let Err(error) = result {
            return Err(self.close(error, services));
        }
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Enqueued)
    }

    /// Accept a body-store-minted deterministic-validation completion.
    pub(crate) fn complete_body_validation<S: V2EffectServices>(
        &mut self,
        completion: BodyValidationCompletion,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        let Some(pending) = self.pending_validations.get(&completion.work_id()).cloned() else {
            if let Some(validated) = completion.validated_receipt() {
                let durable = validated.durable();
                self.durable_bodies
                    .insert((durable.round(), durable.subject()), durable.clone());
                self.validated_bodies
                    .insert((durable.round(), durable.subject()), validated.clone());
            }
            return Ok(CompletionDisposition::Stale);
        };
        if completion.tag() != pending.task.tag() {
            return Err(self.close(
                EffectExecutorError::BodyStore(
                    "validation completion carries the wrong reducer tag".to_owned(),
                ),
                services,
            ));
        }
        let round = pending.task.round();
        let subject = pending.task.subject();
        if let Some(reference) = completion.missing_merge_sidecar() {
            if !merge_sidecar_reference_matches_validation(&pending.task, reference) {
                return Err(self.close(
                    EffectExecutorError::BodyStore(
                        "deferred merge sidecar reference is not bound to the pending carrier"
                            .to_owned(),
                    ),
                    services,
                ));
            }
            if let Some(existing_hash) = self.deferred_merge_work.get(&completion.work_id()) {
                if *existing_hash != reference.entry_hash {
                    return Err(self.close(
                        EffectExecutorError::BodyStore(
                            "validation task deferred for two different merge sidecars".to_owned(),
                        ),
                        services,
                    ));
                }
                return Ok(CompletionDisposition::Deferred);
            }
            if let Err(error) = services.work_deferred_for_merge_sidecar(
                completion.work_id(),
                round,
                subject,
                reference,
            ) {
                return Err(self.close(service_error(error), services));
            }
            self.deferred_merge_work
                .insert(completion.work_id(), reference.entry_hash);
            self.publish_status(services)
                .map_err(|error| self.close(error, services))?;
            return Ok(CompletionDisposition::Deferred);
        }
        let key = (round, subject);
        let result = if let Some(validated) = completion.validated_receipt().cloned() {
            if validated.durable() != pending.task.durable_receipt() {
                return Err(self.close(
                    EffectExecutorError::BodyStore(
                        "validation completion covers a different durable body".to_owned(),
                    ),
                    services,
                ));
            }
            self.validated_bodies.insert(key, validated.clone());
            match pending.purpose {
                ValidationPurpose::Reducer => self
                    .runtime
                    .enqueue_validation_succeeded(pending.task.tag(), round, subject, validated)
                    .map_err(runtime_enqueue_error),
                ValidationPurpose::LocalProposal { manifest } => self
                    .runtime
                    .enqueue_local_proposal(
                        pending.task.tag(),
                        manifest,
                        pending.task.durable_receipt().clone(),
                        validated,
                    )
                    .map_err(runtime_enqueue_error),
            }
        } else {
            let reason = completion
                .rejection_reason()
                .ok_or_else(|| {
                    EffectExecutorError::BodyStore(
                        "validation completion has neither receipt nor rejection".to_owned(),
                    )
                })?
                .to_owned();
            services.validation_rejected(round, subject, &reason);
            match pending.purpose {
                ValidationPurpose::Reducer => self
                    .runtime
                    .enqueue_validation_failed(pending.task.tag(), round, subject)
                    .map_err(runtime_enqueue_error),
                ValidationPurpose::LocalProposal { .. } => Ok(()),
            }
        };
        self.deferred_merge_work.remove(&completion.work_id());
        self.pending_validations.remove(&completion.work_id());
        if let Err(error) = result {
            return Err(self.close(error, services));
        }
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Enqueued)
    }

    /// Retry every retained validation or Apply task waiting for one exact
    /// certified merge entry after authentication and durable installation.
    ///
    /// The pending task and work identifier are reused verbatim. A service
    /// failure leaves the executor fail-closed rather than losing accepted
    /// durable work intent.
    pub(crate) fn retry_deferred_merge_sidecar<S: V2EffectServices>(
        &mut self,
        entry_hash: HashOf<MergeLedgerEntry>,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let work_ids = self
            .deferred_merge_work
            .iter()
            .filter_map(|(work_id, deferred_hash)| {
                (*deferred_hash == entry_hash).then_some(*work_id)
            })
            .collect::<Vec<_>>();
        for work_id in &work_ids {
            if let Some(pending) = self.pending_validations.get(work_id) {
                if let Err(error) = services.enqueue_body_validation(pending.task.clone()) {
                    return Err(self.close(service_error(error), services));
                }
            } else if let Some(pending) = self.pending_applications.get(work_id) {
                if let Err(error) = services.enqueue_apply(pending.task.clone()) {
                    return Err(self.close(service_error(error), services));
                }
            } else {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "deferred merge sidecar has no pending validation or application task"
                            .to_owned(),
                    ),
                    services,
                ));
            }
            self.deferred_merge_work.remove(work_id);
        }
        if !work_ids.is_empty() {
            self.publish_status(services)
                .map_err(|error| self.close(error, services))?;
        }
        Ok(work_ids.len())
    }

    /// Terminally reject every retained task which references one uniquely
    /// invalid certified merge entry. A decided Apply waiter fails closed.
    ///
    /// Transport failures and unavailable holders must not call this method;
    /// those conditions remain recoverable and keep the exact task pending.
    pub(crate) fn reject_deferred_merge_sidecar<S: V2EffectServices>(
        &mut self,
        entry_hash: HashOf<MergeLedgerEntry>,
        reason: impl Into<String>,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let work_ids = self
            .deferred_merge_work
            .iter()
            .filter_map(|(work_id, deferred_hash)| {
                (*deferred_hash == entry_hash).then_some(*work_id)
            })
            .collect::<Vec<_>>();
        let reason = reason.into();
        if let Some(pending) = work_ids
            .iter()
            .find_map(|work_id| self.pending_applications.get(work_id))
        {
            let certificate = pending.task.certificate().clone();
            let subject = pending.task.subject();
            if let Err(error) = services.report_invalid_certified_body(subject, certificate) {
                return Err(self.close(service_error(error), services));
            }
            return Err(self.close(
                EffectExecutorError::BodyStore(format!(
                    "decided body references an invalid certified merge sidecar: {reason}"
                )),
                services,
            ));
        }
        for work_id in &work_ids {
            let Some(pending) = self.pending_validations.get(work_id) else {
                return Err(self.close(
                    EffectExecutorError::Contract(
                        "deferred merge sidecar has no pending validation task".to_owned(),
                    ),
                    services,
                ));
            };
            let tag = pending.task.tag();
            self.complete_body_validation(
                BodyValidationCompletion::Rejected {
                    work_id: *work_id,
                    tag,
                    reason: reason.clone(),
                },
                services,
            )?;
        }
        Ok(work_ids.len())
    }

    /// Retain a decided Apply task when its previously validated merge sidecar
    /// must be recovered again (for example after a safe losing-view prune).
    pub(crate) fn defer_application_for_merge_sidecar<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        reference: &CertifiedMergeLedgerReference,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        let Some(pending) = self.pending_applications.get(&work_id) else {
            return Ok(CompletionDisposition::Stale);
        };
        let round = pending.task.certificate().round;
        let subject = pending.task.subject();
        if !merge_sidecar_reference_matches_carrier(round, subject, reference) {
            return Err(self.close(
                EffectExecutorError::BodyStore(
                    "deferred Apply merge sidecar is not bound to the decided carrier".to_owned(),
                ),
                services,
            ));
        }
        if let Some(existing_hash) = self.deferred_merge_work.get(&work_id) {
            if *existing_hash != reference.entry_hash {
                return Err(self.close(
                    EffectExecutorError::BodyStore(
                        "Apply task deferred for two different merge sidecars".to_owned(),
                    ),
                    services,
                ));
            }
            return Ok(CompletionDisposition::Deferred);
        }
        if let Err(error) =
            services.work_deferred_for_merge_sidecar(work_id, round, subject, reference)
        {
            return Err(self.close(service_error(error), services));
        }
        self.deferred_merge_work
            .insert(work_id, reference.entry_hash);
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Deferred)
    }

    /// Reject one registration-time deferral without affecting another body
    /// that merely claimed the same entry hash with different metadata.
    pub(crate) fn reject_deferred_merge_sidecar_work<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        reason: impl Into<String>,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        if !self.deferred_merge_work.contains_key(&work_id) {
            return Ok(CompletionDisposition::Stale);
        }
        if self.pending_applications.contains_key(&work_id) {
            return Err(self.close(
                EffectExecutorError::BodyStore(
                    "decided Apply task could not register its certified merge sidecar".to_owned(),
                ),
                services,
            ));
        }
        let Some(pending) = self.pending_validations.get(&work_id) else {
            return Err(self.close(
                EffectExecutorError::Contract(
                    "deferred merge sidecar has no pending validation task".to_owned(),
                ),
                services,
            ));
        };
        self.complete_body_validation(
            BodyValidationCompletion::Rejected {
                work_id,
                tag: pending.task.tag(),
                reason: reason.into(),
            },
            services,
        )
    }

    /// Fail closed when the asynchronous body-store/validation worker cannot
    /// complete a still-pending exact task.
    #[cfg(test)]
    pub(crate) fn body_service_failed<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        reason: impl fmt::Display,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        if !self.pending_stores.contains_key(&work_id)
            && !self.pending_validations.contains_key(&work_id)
        {
            return Ok(CompletionDisposition::Stale);
        }
        Err(self.close(EffectExecutorError::BodyStore(reason.to_string()), services))
    }

    /// Permanently close the height after an accepted asynchronous service
    /// task fails without a protocol-valid completion.
    ///
    /// Once an effect adapter has acknowledged queueing work, silently losing
    /// it would turn durable reducer intent into an unbounded stall. Production
    /// workers therefore surface signing, application, storage-thread, and
    /// network-service failures through this single fail-closed boundary.
    pub(crate) fn external_service_failed<S: V2EffectServices>(
        &mut self,
        reason: impl fmt::Display,
        services: &mut S,
    ) -> EffectExecutorError {
        self.close(EffectExecutorError::Service(reason.to_string()), services)
    }

    /// Authenticate a chunk before handing it to the reconstruction adapter.
    pub(crate) fn accept_payload_chunk<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        chunk: wire::PayloadChunk,
        authenticated_sender: &PeerId,
        services: &mut S,
    ) -> Result<(), EffectTransportError> {
        if let Some(reason) = &self.fatal_reason {
            return Err(EffectTransportError::FailClosed(reason.clone()));
        }
        let pending = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?;
        let manifest = pending
            .task
            .manifest
            .as_ref()
            .ok_or(EffectTransportError::WrongFetchKind)?;
        let authenticated =
            authenticate_payload_chunk(&self.context, manifest, chunk, authenticated_sender)?;
        if let Err(error) = services.accept_authenticated_chunk(work_id, authenticated) {
            let reason = EffectExecutorError::Service(error.to_string()).to_string();
            self.fatal_reason = Some(reason.clone());
            services.fail_closed(&reason);
            return Err(EffectTransportError::FailClosed(reason));
        }
        Ok(())
    }

    /// Complete ordinary authenticated-chunk reconstruction with exact bytes.
    pub(crate) fn complete_body_reconstruction<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        manifest: wire::PayloadManifest,
        body: Vec<u8>,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectTransportError> {
        let pending = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?;
        if pending.request_hash.is_some() {
            return Err(EffectTransportError::WrongFetchKind);
        }
        self.finish_fetch(work_id, manifest, body, services)
    }

    /// Authenticate a certified response against the exact outstanding signed
    /// request, then enqueue body availability with the original fetch tag.
    pub(crate) fn accept_certified_body_response<S: V2EffectServices>(
        &mut self,
        response: wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectTransportError> {
        if let Some(reason) = &self.fatal_reason {
            return Err(EffectTransportError::FailClosed(reason.clone()));
        }
        let work_id = self
            .certified_work
            .get(&response.request_hash)
            .copied()
            .ok_or(EffectTransportError::Authentication(
                V2TransportError::UnsolicitedResponse(response.request_hash),
            ))?;
        self.check_ready_capacity(response.body.len())?;
        let authenticated = self.outstanding_requests.authenticate_response(
            &self.context,
            response,
            authenticated_responder,
        )?;
        let response = authenticated.into_inner();
        self.certified_work.remove(&response.request_hash);
        self.finish_fetch(work_id, response.manifest, response.body, services)
    }

    /// Accept a durable application completion only when its typed Kura receipt
    /// and canonical finality artifact exactly match the Apply effect.
    pub(crate) fn complete_application<S: V2EffectServices>(
        &mut self,
        completion: DurableApplyCompletion,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectExecutorError> {
        self.ensure_open()?;
        let Some(pending) = self.pending_applications.get(&completion.work_id) else {
            return Ok(CompletionDisposition::Stale);
        };
        let task = &pending.task;
        let valid_artifact = completion.artifact.validate().is_ok()
            && completion.artifact.height_context == self.context
            && completion.artifact.subject == task.subject
            && completion.artifact.commit_qc == task.certificate;
        let valid_receipt = completion.receipt.height() == self.context.height
            && completion.receipt.context_id() == self.context.id()
            && completion.receipt.block_hash() == task.subject.block_hash
            && completion.receipt.subject() == task.subject
            && completion.receipt.certificate() == task.certificate.as_ref()
            && completion.receipt.artifact_hash() == HashOf::new(&completion.artifact);
        if !valid_artifact || !valid_receipt || self.finality_completion.is_some() {
            return Err(self.close(EffectExecutorError::InvalidApplyCompletion, services));
        }
        let tag = task.tag;
        let subject = task.subject;
        if let Err(error) = self.runtime.enqueue_application_completed(tag, subject) {
            return Err(self.close(runtime_enqueue_error(error), services));
        }
        self.pending_applications.remove(&completion.work_id);
        self.deferred_merge_work.remove(&completion.work_id);
        self.finality_completion = Some(FinalityCompletion {
            receipt: completion.receipt,
            artifact: completion.artifact,
        });
        self.publish_status(services)
            .map_err(|error| self.close(error, services))?;
        Ok(CompletionDisposition::Enqueued)
    }

    /// Current bounded operational status.
    pub(crate) fn status(&self) -> EffectExecutorStatus {
        EffectExecutorStatus {
            fail_closed: self.fatal_reason.is_some(),
            fatal_reason: self.fatal_reason.clone(),
            pending_signatures: self.pending_signatures.len(),
            pending_fetches: self.pending_fetches.len(),
            pending_stores: self.pending_stores.len(),
            pending_validations: self.pending_validations.len(),
            deferred_merge_work: self.deferred_merge_work.len(),
            pending_applications: self.pending_applications.len(),
            ready_bodies: self.ready_bodies.len(),
            ready_body_bytes: self.ready_body_bytes,
            pending_store_bytes: self.pending_store_bytes,
            queued_runtime_completions: self.runtime.queued_commands(),
        }
    }

    /// Exact carrier block hashes still owned by retained missing-sidecar work.
    pub(crate) fn deferred_merge_sidecar_blocks(&self) -> BTreeSet<HashOf<BlockHeader>> {
        self.deferred_merge_work
            .keys()
            .filter_map(|work_id| {
                self.pending_validations
                    .get(work_id)
                    .map(|pending| pending.task.subject().block_hash)
                    .or_else(|| {
                        self.pending_applications
                            .get(work_id)
                            .map(|pending| pending.task.subject().block_hash)
                    })
            })
            .collect()
    }

    /// Return whether the executor still owns this exact deferred validation.
    pub(crate) fn retains_deferred_merge_sidecar(
        &self,
        work_id: EffectWorkId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        entry_hash: HashOf<MergeLedgerEntry>,
    ) -> bool {
        self.deferred_merge_work.get(&work_id) == Some(&entry_hash)
            && (self
                .pending_validations
                .get(&work_id)
                .is_some_and(|pending| {
                    pending.task.round() == round && pending.task.subject() == subject
                })
                || self
                    .pending_applications
                    .get(&work_id)
                    .is_some_and(|pending| {
                        pending.task.certificate().round == round
                            && pending.task.subject() == subject
                    }))
    }

    /// Return whether this retained missing-sidecar dependency belongs to the
    /// uniquely decided Apply task rather than speculative validation work.
    pub(crate) fn deferred_merge_sidecar_is_decided(&self, work_id: EffectWorkId) -> bool {
        self.deferred_merge_work.contains_key(&work_id)
            && self.pending_applications.contains_key(&work_id)
    }

    /// Borrow the durable finality values returned by Kura after application.
    #[cfg(test)]
    pub(crate) fn durable_finality(
        &self,
    ) -> Option<(&KuraV2CommitReceipt, &wire::finality::V2FinalityArtifact)> {
        self.finality_completion
            .as_ref()
            .map(|completion| (&completion.receipt, &completion.artifact))
    }

    fn consume_one<S: V2EffectServices>(
        &mut self,
        effect: AdapterEffect,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        match effect {
            AdapterEffect::Sign { tag, request } => {
                if let SignRequest::Vote(vote) = &request {
                    let validated = self
                        .validated_bodies
                        .get(&(vote.round, vote.subject))
                        .ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "vote signing requires a recovered fsynced validation marker"
                                    .to_owned(),
                            )
                        })?;
                    if validated.durable().context_id() != self.context.id()
                        || validated.durable().round() != vote.round
                        || validated.durable().subject() != vote.subject
                        || validated.execution_commitment() != vote.execution_commitment
                    {
                        return Err(EffectExecutorError::Contract(
                            "vote execution commitment differs from the durable validation marker"
                                .to_owned(),
                        ));
                    }
                }
                self.ensure_pending_slot()?;
                let id = self.allocate_work_id()?;
                self.pending_signatures.insert(
                    id,
                    PendingSignature {
                        tag,
                        request: request.clone(),
                    },
                );
                services
                    .enqueue_consensus_sign(ConsensusSignTask { id, tag, request })
                    .map_err(service_error)
            }
            AdapterEffect::Broadcast(message) => {
                message
                    .validate_version()
                    .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
                services.broadcast_consensus(message).map_err(service_error)
            }
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                manifest,
                certified_sources,
                certificate,
            } => self.begin_fetch(
                tag,
                round,
                subject,
                manifest,
                certified_sources,
                certificate,
                services,
            ),
            AdapterEffect::StoreBody {
                tag,
                round,
                subject,
            } => self.store_body(tag, round, subject, services),
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } => self.validate_body(tag, round, subject, services),
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            } => self.begin_apply(tag, subject, certificate, services),
            AdapterEffect::EnterView { tag, certificate } => {
                self.install_view(tag, certificate, services)
            }
            AdapterEffect::ReportEquivocation { evidence } => services
                .report_equivocation(evidence)
                .map_err(service_error),
            AdapterEffect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => services
                .report_invalid_certified_body(subject, certificate)
                .map_err(service_error),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn begin_fetch<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        manifest: Option<wire::PayloadManifest>,
        sources: Vec<PeerId>,
        certificate: Option<wire::QuorumCertificate>,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        if round.context_id != self.context.id()
            || round.height != self.context.height
            || manifest
                .as_ref()
                .is_some_and(|manifest| manifest.round != round || manifest.subject != subject)
        {
            return Err(EffectExecutorError::Contract(
                "FetchBody is not bound to the frozen context/subject".to_owned(),
            ));
        }
        if let Some(certificate) = &certificate {
            let expected_sources = certificate
                .signers
                .iter()
                .map(|signer| {
                    usize::try_from(*signer)
                        .ok()
                        .and_then(|index| self.context.roster.get(index))
                        .map(|entry| entry.validator.clone())
                        .ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "certified FetchBody signer is outside the frozen roster"
                                    .to_owned(),
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if sources != expected_sources
                || certificate.round != round
                || certificate.subject != subject
            {
                return Err(EffectExecutorError::Contract(
                    "certified FetchBody sources are not the exact canonical QC signer sequence"
                        .to_owned(),
                ));
            }
        } else if manifest.is_none() {
            return Err(EffectExecutorError::Contract(
                "uncertified FetchBody is missing its proposal manifest".to_owned(),
            ));
        }

        if let Some(existing) = self
            .pending_fetches
            .values()
            .find(|pending| pending.task.round == round && pending.task.subject == subject)
        {
            let exact = existing.task.tag == tag
                && existing.task.manifest == manifest
                && existing.task.sources == sources
                && existing
                    .task
                    .certified_request
                    .as_ref()
                    .map(|request| &request.certificate)
                    == certificate.as_ref();
            if !exact {
                return Err(EffectExecutorError::Contract(
                    "conflicting retransmission for one body-fetch round/subject".to_owned(),
                ));
            }
            if self.deferred_merge_work.contains_key(&existing.task.id()) {
                return Ok(());
            }
            return services
                .enqueue_body_fetch(existing.task.clone())
                .map_err(service_error);
        }

        if let Some((recovered_manifest, receipt)) =
            self.recovered_bodies.get(&(round, subject)).cloned()
        {
            if receipt.context_id() != self.context.id()
                || receipt.round() != round
                || receipt.subject() != subject
                || recovered_manifest.round != round
                || recovered_manifest.subject != subject
                || receipt.manifest_hash() != HashOf::new(&recovered_manifest)
                || manifest
                    .as_ref()
                    .is_some_and(|manifest| manifest != &recovered_manifest)
            {
                return Err(EffectExecutorError::BodyStore(
                    "reopened durable body receipt does not match FetchBody".to_owned(),
                ));
            }
            self.durable_bodies.insert((round, subject), receipt);
            return self
                .runtime
                .enqueue_body_available(tag, recovered_manifest)
                .map_err(runtime_enqueue_error);
        }

        self.ensure_pending_slot()?;
        if certificate.is_some()
            && self.outstanding_requests.len() >= self.config.max_certified_requests
        {
            return Err(EffectExecutorError::CertifiedRequestCapacity {
                capacity: self.config.max_certified_requests,
            });
        }
        let id = self.allocate_work_id()?;
        let certified_request = if let Some(certificate) = certificate {
            let mut request = wire::CertifiedBodyRequest {
                round,
                subject,
                certificate,
                requester: self.requester.clone(),
                signature: Vec::new(),
            };
            request.signature = services
                .sign_body_request(&request.signature_preimage())
                .map_err(service_error)?;
            let authenticated = authenticate_certified_body_request(
                &self.context,
                request.clone(),
                &self.requester,
                |context, certificate| self.runtime.verify_certificate(context, certificate),
            )
            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
            let request_hash = authenticated.request_hash();
            self.outstanding_requests
                .register(authenticated)
                .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
            self.certified_work.insert(request_hash, id);
            Some(request)
        } else {
            None
        };
        let request_hash = certified_request.as_ref().map(HashOf::new);
        let task = BodyFetchTask {
            id,
            tag,
            round,
            subject,
            manifest,
            sources,
            certified_request,
        };
        self.pending_fetches.insert(
            id,
            PendingFetch {
                task: task.clone(),
                request_hash,
            },
        );
        services.enqueue_body_fetch(task).map_err(service_error)
    }

    fn store_body<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let key = (round, subject);
        if let Some(receipt) = self.durable_bodies.get(&key).cloned() {
            return self
                .runtime
                .enqueue_body_stored(tag, round, subject, receipt)
                .map_err(runtime_enqueue_error);
        }
        if let Some((manifest, canonical_wire)) = self
            .pending_stores
            .values()
            .find(|pending| {
                pending.task.manifest.round == round && pending.task.manifest.subject == subject
            })
            .map(|pending| {
                (
                    pending.task.manifest.clone(),
                    Arc::clone(&pending.task.canonical_wire),
                )
            })
        {
            return self.begin_store(
                tag,
                manifest,
                canonical_wire,
                StorePurpose::Reducer,
                services,
            );
        }
        let ready = self.ready_bodies.remove(&key).ok_or_else(|| {
            EffectExecutorError::Contract(
                "StoreBody has no matching reconstructed exact body".to_owned(),
            )
        })?;
        self.ready_body_bytes = self
            .ready_body_bytes
            .checked_sub(u64::try_from(ready.bytes.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                EffectExecutorError::Contract("ready-body byte accounting underflow".to_owned())
            })?;
        self.begin_store(
            tag,
            ready.manifest,
            Arc::from(ready.bytes),
            StorePurpose::Reducer,
            services,
        )
    }

    fn validate_body<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let key = (round, subject);
        if let Some(receipt) = self.validated_bodies.get(&key).cloned() {
            return self
                .runtime
                .enqueue_validation_succeeded(tag, round, subject, receipt)
                .map_err(runtime_enqueue_error);
        }
        let receipt = self.durable_bodies.get(&key).cloned().ok_or_else(|| {
            EffectExecutorError::Contract(
                "ValidateBody has no matching durable body receipt".to_owned(),
            )
        })?;
        self.begin_validation(
            tag,
            round,
            subject,
            receipt,
            ValidationPurpose::Reducer,
            services,
        )
    }

    fn begin_store<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        canonical_wire: Arc<[u8]>,
        purpose: StorePurpose,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let key = (manifest.round, manifest.subject);
        if let Some(receipt) = self.durable_bodies.get(&key).cloned() {
            return match purpose {
                StorePurpose::Reducer => self
                    .runtime
                    .enqueue_body_stored(tag, manifest.round, manifest.subject, receipt)
                    .map_err(runtime_enqueue_error),
                StorePurpose::LocalProposal => self.begin_validation(
                    tag,
                    manifest.round,
                    manifest.subject,
                    receipt,
                    ValidationPurpose::LocalProposal { manifest },
                    services,
                ),
            };
        }
        if let Some(existing) = self.pending_stores.values().find(|pending| {
            pending.task.manifest.round == manifest.round
                && pending.task.manifest.subject == manifest.subject
        }) {
            let exact = existing.task.tag == tag
                && existing.task.manifest == manifest
                && existing.task.canonical_wire.as_ref() == canonical_wire.as_ref()
                && existing.purpose == purpose;
            if !exact {
                return Err(EffectExecutorError::Contract(
                    "conflicting body-store retry for one round/subject".to_owned(),
                ));
            }
            return services
                .enqueue_body_store(existing.task.clone())
                .map_err(service_error);
        }
        let body_len = u64::try_from(canonical_wire.len()).map_err(|_| {
            EffectExecutorError::Contract("body-store task length is not representable".to_owned())
        })?;
        if self
            .ready_body_bytes
            .checked_add(self.pending_store_bytes)
            .and_then(|retained| retained.checked_add(body_len))
            .is_none_or(|retained| retained > self.config.max_ready_body_bytes)
        {
            return Err(EffectExecutorError::ReadyBodyCapacity);
        }
        self.ensure_pending_slot()?;
        let id = self.allocate_work_id()?;
        let task = BodyStoreTask {
            id,
            tag,
            manifest,
            canonical_wire,
        };
        self.pending_stores.insert(
            id,
            PendingStore {
                task: task.clone(),
                purpose,
            },
        );
        self.pending_store_bytes = self
            .pending_store_bytes
            .checked_add(body_len)
            .ok_or(EffectExecutorError::ReadyBodyCapacity)?;
        services.enqueue_body_store(task).map_err(service_error)
    }

    #[allow(clippy::too_many_arguments)]
    fn begin_validation<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        durable_receipt: DurableBodyReceipt,
        purpose: ValidationPurpose,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let key = (round, subject);
        if durable_receipt.context_id() != self.context.id()
            || durable_receipt.round() != round
            || durable_receipt.subject() != subject
        {
            return Err(EffectExecutorError::BodyStore(
                "validation task receipt differs from its round/subject".to_owned(),
            ));
        }
        if let Some(validated) = self.validated_bodies.get(&key).cloned() {
            return match purpose {
                ValidationPurpose::Reducer => self
                    .runtime
                    .enqueue_validation_succeeded(tag, round, subject, validated)
                    .map_err(runtime_enqueue_error),
                ValidationPurpose::LocalProposal { manifest } => self
                    .runtime
                    .enqueue_local_proposal(tag, manifest, durable_receipt, validated)
                    .map_err(runtime_enqueue_error),
            };
        }
        if let Some(existing) = self
            .pending_validations
            .values()
            .find(|pending| pending.task.round == round && pending.task.subject == subject)
        {
            let exact = existing.task.tag == tag
                && existing.task.durable_receipt == durable_receipt
                && existing.purpose == purpose;
            if !exact {
                return Err(EffectExecutorError::Contract(
                    "conflicting validation retry for one durable body".to_owned(),
                ));
            }
            if self.deferred_merge_work.contains_key(&existing.task.id()) {
                return Ok(());
            }
            return services
                .enqueue_body_validation(existing.task.clone())
                .map_err(service_error);
        }
        self.ensure_pending_slot()?;
        let id = self.allocate_work_id()?;
        let task = BodyValidationTask {
            id,
            tag,
            round,
            subject,
            durable_receipt,
        };
        self.pending_validations.insert(
            id,
            PendingValidation {
                task: task.clone(),
                purpose,
            },
        );
        services
            .enqueue_body_validation(task)
            .map_err(service_error)
    }

    fn begin_apply<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        if tag.height() != self.context.height
            || certificate.phase != wire::GlobalPhase::Commit
            || certificate.subject != subject
            || certificate.round.context_id != self.context.id()
            || certificate.round.height != self.context.height
        {
            return Err(EffectExecutorError::Contract(
                "Apply is not authorized by the frozen height's exact CommitQC".to_owned(),
            ));
        }
        if let Some(existing) = self.pending_applications.values().next() {
            let exact = existing.task.tag == tag
                && existing.task.subject == subject
                && existing.task.certificate == certificate;
            if !exact {
                return Err(EffectExecutorError::Contract(
                    "conflicting Apply retransmission for one height".to_owned(),
                ));
            }
            if self.deferred_merge_work.contains_key(&existing.task.id()) {
                return Ok(());
            }
            return services
                .enqueue_apply(existing.task.clone())
                .map_err(service_error);
        }
        let key = (certificate.round, subject);
        let validated_receipt = self.validated_bodies.get(&key).cloned().ok_or_else(|| {
            EffectExecutorError::Contract(
                "Apply body lacks an exact deterministic-validation receipt".to_owned(),
            )
        })?;
        if self.durable_bodies.get(&key) != Some(validated_receipt.durable()) {
            return Err(EffectExecutorError::Contract(
                "Apply validation receipt differs from local durable body".to_owned(),
            ));
        }
        if certificate.execution_commitment != validated_receipt.execution_commitment() {
            return Err(EffectExecutorError::Contract(
                "Apply CommitQC execution commitment differs from the durable validation marker"
                    .to_owned(),
            ));
        }
        self.ensure_pending_slot()?;
        let id = self.allocate_work_id()?;
        let task = ApplyTask {
            id,
            tag,
            subject,
            certificate,
            validated_receipt,
        };
        self.pending_applications
            .insert(id, PendingApply { task: task.clone() });
        services.enqueue_apply(task).map_err(service_error)
    }

    fn finish_fetch<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        manifest: wire::PayloadManifest,
        body: Vec<u8>,
        services: &mut S,
    ) -> Result<CompletionDisposition, EffectTransportError> {
        if let Some(reason) = &self.fatal_reason {
            return Err(EffectTransportError::FailClosed(reason.clone()));
        }
        let pending = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?;
        manifest
            .validate(&self.context)
            .map_err(|_| EffectTransportError::BodyMismatch)?;
        let task = &pending.task;
        if manifest.round != task.round
            || manifest.subject != task.subject
            || pending
                .task
                .manifest
                .as_ref()
                .is_some_and(|expected| expected != &manifest)
            || u64::try_from(body.len()).ok() != Some(manifest.payload_size_bytes)
            || Hash::new(&body) != task.subject.payload_hash
        {
            return Err(EffectTransportError::BodyMismatch);
        }
        self.check_ready_capacity(body.len())?;
        let key = (task.round, task.subject);
        if self.ready_bodies.contains_key(&key) || self.durable_bodies.contains_key(&key) {
            return Err(EffectTransportError::BodyMismatch);
        }
        let tag = task.tag;
        let body_len = u64::try_from(body.len()).map_err(|_| EffectTransportError::Backpressure)?;
        self.ready_bodies.insert(
            key,
            ReadyBody {
                manifest: manifest.clone(),
                bytes: body,
            },
        );
        self.ready_body_bytes = self
            .ready_body_bytes
            .checked_add(body_len)
            .ok_or(EffectTransportError::Backpressure)?;
        if let Err(error) = self.runtime.enqueue_body_available(tag, manifest) {
            self.fatal_reason = Some(runtime_enqueue_error(error).to_string());
            services.fail_closed(
                self.fatal_reason
                    .as_deref()
                    .expect("fatal reason was just installed"),
            );
            return Err(EffectTransportError::FailClosed(
                self.fatal_reason.clone().expect("fatal reason exists"),
            ));
        }
        self.pending_fetches.remove(&work_id);
        Ok(CompletionDisposition::Enqueued)
    }

    fn check_ready_capacity(&self, body_len: usize) -> Result<(), EffectTransportError> {
        let body_len = u64::try_from(body_len).map_err(|_| EffectTransportError::Backpressure)?;
        if self.ready_bodies.len() >= self.config.max_ready_bodies
            || self
                .ready_body_bytes
                .checked_add(self.pending_store_bytes)
                .and_then(|retained| retained.checked_add(body_len))
                .is_none_or(|total| total > self.config.max_ready_body_bytes)
        {
            return Err(EffectTransportError::Backpressure);
        }
        Ok(())
    }

    fn install_view<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        certificate: wire::TimeoutCertificate,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        if tag.height() != self.context.height
            || certificate.round.context_id != self.context.id()
            || certificate.round.height != self.context.height
            || certificate.round.view.checked_add(1) != Some(tag.view())
        {
            return Err(EffectExecutorError::Contract(
                "EnterView tag does not immediately follow its persisted timeout certificate"
                    .to_owned(),
            ));
        }

        let protected_validation = certificate
            .highest_prepare_qc()
            .map(|highest| (highest.round, highest.subject));

        let stale = self
            .pending_signatures
            .iter()
            .filter_map(|(id, pending)| (pending.tag.view() < tag.view()).then_some(*id))
            .collect::<Vec<_>>();
        for id in stale {
            services.cancel_consensus_sign(id).map_err(service_error)?;
            self.pending_signatures.remove(&id);
        }

        // A durable exact body can be revalidated if a late certificate later
        // proves it relevant. Until then, only the subject protected by this
        // TC's selected high PrepareQC may retain asynchronous validation and
        // merge-sidecar reservations across the certified view transition.
        let stale = self
            .pending_validations
            .iter()
            .filter_map(|(id, pending)| {
                (pending.task.round().view < tag.view()
                    && Some((pending.task.round(), pending.task.subject())) != protected_validation)
                    .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in stale {
            self.deferred_merge_work.remove(&id);
            self.pending_validations.remove(&id);
        }

        let stale = self
            .pending_fetches
            .iter()
            .filter_map(|(id, pending)| (pending.task.round.view < tag.view()).then_some(*id))
            .collect::<Vec<_>>();
        for id in stale {
            services.cancel_body_fetch(id).map_err(service_error)?;
            if let Some(pending) = self.pending_fetches.remove(&id)
                && let Some(hash) = pending.request_hash
            {
                self.certified_work.remove(&hash);
                if !self.outstanding_requests.cancel(hash) {
                    return Err(EffectExecutorError::Contract(
                        "stale certified fetch was absent from the exact request tracker"
                            .to_owned(),
                    ));
                }
            }
        }

        let stale = self
            .pending_stores
            .iter()
            .filter_map(|(id, pending)| {
                let key = (pending.task.manifest.round, pending.task.manifest.subject);
                (pending.task.manifest.round.view < tag.view()
                    && !self.durable_bodies.contains_key(&key)
                    && !self.validated_bodies.contains_key(&key))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in stale {
            services.cancel_body_store(id).map_err(service_error)?;
            if let Some(pending) = self.pending_stores.remove(&id) {
                let bytes = u64::try_from(pending.task.canonical_wire.len()).map_err(|_| {
                    EffectExecutorError::Contract(
                        "pending-store byte count is not representable".to_owned(),
                    )
                })?;
                self.pending_store_bytes =
                    self.pending_store_bytes.checked_sub(bytes).ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "pending-store byte accounting underflow".to_owned(),
                        )
                    })?;
            }
        }

        let stale_ready = self
            .ready_bodies
            .keys()
            .filter(|(round, _)| round.view < tag.view())
            .copied()
            .collect::<Vec<_>>();
        for key in stale_ready {
            if let Some(body) = self.ready_bodies.remove(&key) {
                let bytes = u64::try_from(body.bytes.len()).map_err(|_| {
                    EffectExecutorError::Contract(
                        "ready-body byte count is not representable".to_owned(),
                    )
                })?;
                self.ready_body_bytes =
                    self.ready_body_bytes.checked_sub(bytes).ok_or_else(|| {
                        EffectExecutorError::Contract(
                            "ready-body byte accounting underflow".to_owned(),
                        )
                    })?;
            }
        }

        services
            .entered_view(tag, certificate)
            .map_err(service_error)
    }

    fn ensure_pending_slot(&self) -> Result<(), EffectExecutorError> {
        if self.pending_work() >= self.config.max_pending_work {
            Err(EffectExecutorError::PendingWorkCapacity {
                capacity: self.config.max_pending_work,
            })
        } else {
            Ok(())
        }
    }

    fn allocate_work_id(&mut self) -> Result<EffectWorkId, EffectExecutorError> {
        let id = EffectWorkId(self.next_work_id);
        self.next_work_id = self
            .next_work_id
            .checked_add(1)
            .ok_or(EffectExecutorError::WorkIdExhausted)?;
        Ok(id)
    }

    fn pending_work(&self) -> usize {
        self.pending_signatures
            .len()
            .checked_add(self.pending_fetches.len())
            .and_then(|total| total.checked_add(self.pending_stores.len()))
            .and_then(|total| total.checked_add(self.pending_validations.len()))
            .and_then(|total| total.checked_add(self.pending_applications.len()))
            .unwrap_or(usize::MAX)
    }

    fn ensure_open(&self) -> Result<(), EffectExecutorError> {
        match &self.fatal_reason {
            Some(reason) => Err(EffectExecutorError::FailClosed(reason.clone())),
            None => Ok(()),
        }
    }

    fn publish_status<S: V2EffectServices>(
        &self,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        services
            .publish_effect_status(&self.status())
            .map_err(service_error)
    }

    fn close<S: V2EffectServices>(
        &mut self,
        error: EffectExecutorError,
        services: &mut S,
    ) -> EffectExecutorError {
        let reason = self
            .fatal_reason
            .get_or_insert_with(|| error.to_string())
            .clone();
        services.fail_closed(&reason);
        error
    }
}

/// Outcome of one runtime/executor scheduling step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectExecutorStep {
    /// No timer or runtime command was ready.
    Idle,
    /// One runtime transition advanced and all emitted effects were consumed.
    Advanced {
        /// Number of effects bound to external services or completions.
        effects: usize,
    },
}

fn verify_signer_completion(
    context: &wire::HeightContext,
    local_validator: Option<wire::ValidatorIndex>,
    request: &SignRequest,
    signature: &[u8],
) -> Result<(), String> {
    let (signer, preimage) = match request {
        SignRequest::Proposal(proposal) => (proposal.proposer, proposal.signature_preimage()),
        SignRequest::Vote(vote) => (vote.signer, vote.signature_preimage()),
        SignRequest::TimeoutVote(vote) => (vote.signer, vote.signature_preimage()),
    };
    if local_validator != Some(signer) {
        return Err("signing request does not belong to the configured local validator".to_owned());
    }
    let index = usize::try_from(signer)
        .ok()
        .filter(|index| *index < context.roster.len())
        .ok_or_else(|| "signing request index is outside the frozen roster".to_owned())?;
    let signature = Signature::try_from_bytes(signature).map_err(|error| error.to_string())?;
    signature
        .verify(context.roster[index].validator.public_key(), &preimage)
        .map_err(|error| error.to_string())
}

fn runtime_enqueue_error(error: EnqueueError) -> EffectExecutorError {
    EffectExecutorError::Runtime(error.to_string())
}

fn service_error(error: impl fmt::Display) -> EffectExecutorError {
    EffectExecutorError::Service(error.to_string())
}

fn store_completion_matches(
    context: &wire::HeightContext,
    manifest: &wire::PayloadManifest,
    receipt: &DurableBodyReceipt,
) -> bool {
    manifest.validate(context).is_ok()
        && receipt.context_id() == context.id()
        && receipt.round() == manifest.round
        && receipt.subject() == manifest.subject
        && receipt.manifest_hash() == HashOf::new(manifest)
}

fn merge_sidecar_reference_matches_validation(
    task: &BodyValidationTask,
    reference: &CertifiedMergeLedgerReference,
) -> bool {
    merge_sidecar_reference_matches_carrier(task.round(), task.subject(), reference)
}

fn merge_sidecar_reference_matches_carrier(
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    reference: &CertifiedMergeLedgerReference,
) -> bool {
    let certificate = &reference.merge_qc;
    reference.version == 1
        && reference.encoded_len != 0
        && reference.epoch_id == certificate.epoch_id
        && certificate.carrier_height == round.height
        // A locked body may be re-proposed in a later wire round. Its compact
        // reference must retain the immutable original carrier view.
        && certificate.view <= round.view
        && subject.parent_block_hash == Some(certificate.carrier_parent_hash)
}

fn verify_pending_kura_apply_parts(
    context: &wire::HeightContext,
    decision: Option<(
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    )>,
    recovered_bodies: &BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
    validated_bodies: &BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    expected: PendingKuraApply,
) -> Result<(), EffectExecutorError> {
    let mismatch =
        |reason: &'static str| EffectExecutorError::PendingApplyRecoveryMismatch(reason.to_owned());
    if expected.context_id() != context.id() || expected.height() != context.height {
        return Err(mismatch(
            "recovered Kura tip belongs to a different frozen height context",
        ));
    }
    let (round, subject, execution_commitment) = decision.ok_or_else(|| {
        mismatch("canonical Kura tip has no complete durable Decision WAL record")
    })?;
    if round.context_id != context.id()
        || round.height != context.height
        || subject.block_hash != expected.block_hash()
    {
        return Err(mismatch(
            "replayed Decision does not identify the canonical pending Kura tip",
        ));
    }
    let key = (round, subject);
    let (manifest, durable) = recovered_bodies.get(&key).ok_or_else(|| {
        mismatch("replayed Decision has no matching checksummed durable body frame")
    })?;
    if manifest.round != round
        || manifest.subject != subject
        || !store_completion_matches(context, manifest, durable)
    {
        return Err(mismatch(
            "recovered body frame differs from the replayed Decision key",
        ));
    }
    let validated = validated_bodies
        .get(&key)
        .ok_or_else(|| mismatch("replayed Decision has no matching durable validation marker"))?;
    if validated.durable() != durable {
        return Err(mismatch(
            "durable validation marker differs from the recovered exact body frame",
        ));
    }
    if validated.execution_commitment() != execution_commitment {
        return Err(mismatch(
            "durable Decision commitment differs from the recovered validation marker",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, num::NonZeroU64};

    use crate::sumeragi::v2_core::Generation;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        block::{BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire},
        merge::MergeQuorumCertificate,
        peer::PeerId,
    };
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn post_finality_cleanup_accumulates_typed_warnings_in_order() {
        let mut outcome = PostFinalityCleanupOutcome::default();
        outcome.record(PostFinalityCleanupTarget::SafetyWal, "WAL directory sync");

        let mut storage_cleanup = PostFinalityCleanupOutcome::default();
        storage_cleanup.record(
            PostFinalityCleanupTarget::DurableBodies,
            "body worker disconnected",
        );
        storage_cleanup.record(
            PostFinalityCleanupTarget::PayloadChunks,
            "chunk root retained",
        );
        outcome.append(storage_cleanup);

        assert_eq!(outcome.warnings().len(), 3);
        assert_eq!(
            outcome
                .warnings()
                .iter()
                .map(PostFinalityCleanupWarning::target)
                .collect::<Vec<_>>(),
            vec![
                PostFinalityCleanupTarget::SafetyWal,
                PostFinalityCleanupTarget::DurableBodies,
                PostFinalityCleanupTarget::PayloadChunks,
            ]
        );
        assert_eq!(outcome.warnings()[0].reason(), "WAL directory sync");
        assert_eq!(
            PostFinalityCleanupTarget::CleanupWorker.as_str(),
            "cleanup_worker"
        );
    }

    #[derive(Debug)]
    enum RuntimeCompletion {
        BodyAvailable(EventTag, wire::PayloadManifest),
        BodyStored(
            EventTag,
            wire::ConsensusRound,
            wire::BlockSubject,
            DurableBodyReceipt,
        ),
        ValidationSucceeded(
            EventTag,
            wire::ConsensusRound,
            wire::BlockSubject,
            ValidatedBodyReceipt,
        ),
        ValidationFailed(EventTag, wire::ConsensusRound, wire::BlockSubject),
        Signature(EventTag, Vec<u8>),
        Application(EventTag, wire::BlockSubject),
        LocalProposal(
            EventTag,
            wire::PayloadManifest,
            DurableBodyReceipt,
            ValidatedBodyReceipt,
        ),
    }

    #[derive(Default)]
    struct FakeRuntime {
        steps: VecDeque<Result<RuntimeStep<AdapterEffect>, String>>,
        completions: Vec<RuntimeCompletion>,
        fail_enqueue: bool,
    }

    impl FakeRuntime {
        fn push(&mut self, completion: RuntimeCompletion) -> Result<(), EnqueueError> {
            if self.fail_enqueue {
                return Err(EnqueueError::Full);
            }
            self.completions.push(completion);
            Ok(())
        }
    }

    impl EffectRuntime for FakeRuntime {
        fn step_effects(&mut self, _now: Instant) -> Result<RuntimeStep<AdapterEffect>, String> {
            self.steps.pop_front().unwrap_or(Ok(RuntimeStep::Idle))
        }

        fn enqueue_body_available(
            &mut self,
            tag: EventTag,
            manifest: wire::PayloadManifest,
        ) -> Result<(), EnqueueError> {
            self.push(RuntimeCompletion::BodyAvailable(tag, manifest))
        }

        fn enqueue_body_stored(
            &mut self,
            tag: EventTag,
            round: wire::ConsensusRound,
            subject: wire::BlockSubject,
            receipt: DurableBodyReceipt,
        ) -> Result<(), EnqueueError> {
            self.push(RuntimeCompletion::BodyStored(tag, round, subject, receipt))
        }

        fn enqueue_validation_succeeded(
            &mut self,
            tag: EventTag,
            round: wire::ConsensusRound,
            subject: wire::BlockSubject,
            receipt: ValidatedBodyReceipt,
        ) -> Result<(), EnqueueError> {
            self.push(RuntimeCompletion::ValidationSucceeded(
                tag, round, subject, receipt,
            ))
        }

        fn enqueue_validation_failed(
            &mut self,
            tag: EventTag,
            round: wire::ConsensusRound,
            subject: wire::BlockSubject,
        ) -> Result<(), EnqueueError> {
            self.push(RuntimeCompletion::ValidationFailed(tag, round, subject))
        }

        fn enqueue_signature(
            &mut self,
            tag: EventTag,
            signature: Vec<u8>,
        ) -> Result<(), EnqueueError> {
            self.push(RuntimeCompletion::Signature(tag, signature))
        }

        fn enqueue_application_completed(
            &mut self,
            tag: EventTag,
            subject: wire::BlockSubject,
        ) -> Result<(), EnqueueError> {
            self.push(RuntimeCompletion::Application(tag, subject))
        }

        fn enqueue_local_proposal(
            &mut self,
            tag: EventTag,
            manifest: wire::PayloadManifest,
            durable_receipt: DurableBodyReceipt,
            validated_receipt: ValidatedBodyReceipt,
        ) -> Result<(), EnqueueError> {
            self.push(RuntimeCompletion::LocalProposal(
                tag,
                manifest,
                durable_receipt,
                validated_receipt,
            ))
        }

        fn verify_certificate(
            &self,
            context: &wire::HeightContext,
            certificate: &wire::QuorumCertificate,
        ) -> Result<(), String> {
            certificate
                .validate(context)
                .map_err(|error| error.to_string())
        }

        fn queued_commands(&self) -> usize {
            self.completions.len()
        }
    }

    #[derive(Default)]
    struct FakeServices {
        _body_directory: Option<TempDir>,
        body_store: Option<V2BodyStore>,
        requester_key: Option<KeyPair>,
        sign_tasks: Vec<ConsensusSignTask>,
        cancelled_signatures: Vec<EffectWorkId>,
        broadcasts: Vec<wire::ConsensusMessageV2>,
        fetch_tasks: Vec<BodyFetchTask>,
        cancelled_fetches: Vec<EffectWorkId>,
        chunks: Vec<EffectWorkId>,
        store_tasks: Vec<BodyStoreTask>,
        cancelled_stores: Vec<EffectWorkId>,
        validation_tasks: Vec<BodyValidationTask>,
        deferred_merge_sidecars: Vec<(
            EffectWorkId,
            wire::ConsensusRound,
            wire::BlockSubject,
            CertifiedMergeLedgerReference,
        )>,
        apply_tasks: Vec<ApplyTask>,
        entered_views: Vec<EventTag>,
        equivocations: Vec<wire::SumeragiV2Equivocation>,
        invalid_bodies: Vec<wire::BlockSubject>,
        rejected_validations: Vec<String>,
        statuses: Vec<EffectExecutorStatus>,
        closed: Vec<String>,
        fail_on: Option<&'static str>,
        validation_error: Option<String>,
    }

    impl FakeServices {
        fn check(&mut self, operation: &'static str) -> Result<(), String> {
            if self.fail_on == Some(operation) {
                self.fail_on = None;
                Err(format!("{operation} failed"))
            } else {
                Ok(())
            }
        }

        fn execute_store(&mut self, work_id: EffectWorkId) -> BodyStoreCompletion {
            let task = self
                .store_tasks
                .iter()
                .rev()
                .find(|task| task.id() == work_id)
                .expect("store task")
                .clone();
            self.body_store
                .as_mut()
                .expect("body store service")
                .execute_store_task(&task)
                .expect("execute durable store task")
        }

        fn execute_validation(&mut self, work_id: EffectWorkId) -> BodyValidationCompletion {
            let task = self
                .validation_tasks
                .iter()
                .rev()
                .find(|task| task.id() == work_id)
                .expect("validation task");
            let rejection = self.validation_error.clone();
            let execution_commitment = fixture_execution_commitment();
            self.body_store
                .as_mut()
                .expect("body store service")
                .execute_validation_task(task, move |_| match rejection {
                    Some(reason) => Err(reason),
                    None => Ok(execution_commitment),
                })
                .expect("execute deterministic validation task")
        }
    }

    impl V2EffectServices for FakeServices {
        type Error = String;

        fn enqueue_consensus_sign(&mut self, task: ConsensusSignTask) -> Result<(), Self::Error> {
            self.check("sign")?;
            self.sign_tasks.push(task);
            Ok(())
        }

        fn cancel_consensus_sign(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
            self.check("cancel-sign")?;
            self.cancelled_signatures.push(work_id);
            Ok(())
        }

        fn broadcast_consensus(
            &mut self,
            message: wire::ConsensusMessageV2,
        ) -> Result<(), Self::Error> {
            self.check("broadcast")?;
            self.broadcasts.push(message);
            Ok(())
        }

        fn sign_body_request(&mut self, preimage: &[u8]) -> Result<Vec<u8>, Self::Error> {
            self.check("body-sign")?;
            let key = self
                .requester_key
                .as_ref()
                .ok_or_else(|| "missing requester key".to_owned())?;
            Ok(Signature::new(key.private_key(), preimage)
                .payload()
                .to_vec())
        }

        fn enqueue_body_fetch(&mut self, task: BodyFetchTask) -> Result<(), Self::Error> {
            self.check("fetch")?;
            self.fetch_tasks.push(task);
            Ok(())
        }

        fn cancel_body_fetch(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
            self.check("cancel-fetch")?;
            self.cancelled_fetches.push(work_id);
            Ok(())
        }

        fn accept_authenticated_chunk(
            &mut self,
            work_id: EffectWorkId,
            _chunk: AuthenticatedPayloadChunk,
        ) -> Result<(), Self::Error> {
            self.check("chunk")?;
            self.chunks.push(work_id);
            Ok(())
        }

        fn enqueue_body_store(&mut self, task: BodyStoreTask) -> Result<(), Self::Error> {
            self.check("store")?;
            self.store_tasks.push(task);
            Ok(())
        }

        fn cancel_body_store(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
            self.check("cancel-store")?;
            self.cancelled_stores.push(work_id);
            Ok(())
        }

        fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error> {
            self.check("validation")?;
            self.validation_tasks.push(task);
            Ok(())
        }

        fn work_deferred_for_merge_sidecar(
            &mut self,
            work_id: EffectWorkId,
            round: wire::ConsensusRound,
            subject: wire::BlockSubject,
            reference: &CertifiedMergeLedgerReference,
        ) -> Result<(), Self::Error> {
            self.check("merge-sidecar")?;
            self.deferred_merge_sidecars
                .push((work_id, round, subject, reference.clone()));
            Ok(())
        }

        fn enqueue_apply(&mut self, task: ApplyTask) -> Result<(), Self::Error> {
            self.check("apply")?;
            self.apply_tasks.push(task);
            Ok(())
        }

        fn entered_view(
            &mut self,
            tag: EventTag,
            _certificate: wire::TimeoutCertificate,
        ) -> Result<(), Self::Error> {
            self.check("view")?;
            self.entered_views.push(tag);
            Ok(())
        }

        fn report_equivocation(
            &mut self,
            evidence: wire::SumeragiV2Equivocation,
        ) -> Result<(), Self::Error> {
            self.check("equivocation")?;
            self.equivocations.push(evidence);
            Ok(())
        }

        fn report_invalid_certified_body(
            &mut self,
            subject: wire::BlockSubject,
            _certificate: wire::QuorumCertificate,
        ) -> Result<(), Self::Error> {
            self.check("invalid-body")?;
            self.invalid_bodies.push(subject);
            Ok(())
        }

        fn validation_rejected(
            &mut self,
            _round: wire::ConsensusRound,
            _subject: wire::BlockSubject,
            reason: &str,
        ) {
            self.rejected_validations.push(reason.to_owned());
        }

        fn publish_effect_status(
            &mut self,
            status: &EffectExecutorStatus,
        ) -> Result<(), Self::Error> {
            self.check("status")?;
            self.statuses.push(status.clone());
            Ok(())
        }

        fn fail_closed(&mut self, reason: &str) {
            self.closed.push(reason.to_owned());
        }
    }

    struct Fixture {
        context: wire::HeightContext,
        validator_keys: Vec<KeyPair>,
        requester_key: KeyPair,
        block: SignedBlock,
        body: Vec<u8>,
        manifest: wire::PayloadManifest,
    }

    impl Fixture {
        fn new() -> Self {
            let mut validator_keys = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                        .expect("deterministic validator key")
                })
                .collect::<Vec<_>>();
            validator_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let roster = validator_keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let context = wire::HeightContext {
                chain_id: "v2-effect-executor-test".into(),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                roster: roster.clone(),
                quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
                nexus_amx_context_hash: Hash::new(b"nexus amx context"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::Plain,
                    chunk_size_bytes: 1_048_576,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 1_048_576,
                    max_chunk_count: 1,
                },
                leader_seed: [0x33; 32],
            };
            let round = round(&context, 0);
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("height"),
                None,
                None,
                None,
                1_000,
                0,
            );
            let signature =
                SignatureOf::try_from_hash(validator_keys[0].private_key(), header.hash())
                    .expect("block signature");
            let block =
                SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
            let body = block.encode_wire().expect("canonical body");
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: block.hash(),
                payload_hash: Hash::new(&body),
            };
            let manifest = wire::PayloadManifest::derive(
                &context,
                round,
                subject,
                u64::try_from(body.len()).expect("body length"),
                std::slice::from_ref(&body),
            )
            .expect("manifest");
            let requester_key =
                KeyPair::try_from_seed(vec![99; 32], Algorithm::Ed25519).expect("requester key");
            Self {
                context,
                validator_keys,
                requester_key,
                block,
                body,
                manifest,
            }
        }

        fn services(&self) -> FakeServices {
            let directory = TempDir::new().expect("body-store directory");
            let body_store = V2BodyStore::open_with_policy(
                directory.path(),
                self.context.clone(),
                BlockSignaturePolicy::GenesisAuthority(self.validator_keys[0].public_key().clone()),
            )
            .expect("body-store service");
            FakeServices {
                _body_directory: Some(directory),
                body_store: Some(body_store),
                requester_key: Some(self.requester_key.clone()),
                ..FakeServices::default()
            }
        }

        fn executor(&self, config: EffectQueueConfig) -> V2EffectExecutor<FakeRuntime> {
            V2EffectExecutor::with_runtime(
                FakeRuntime::default(),
                BTreeMap::new(),
                self.context.clone(),
                PeerId::new(self.requester_key.public_key().clone()),
                Some(0),
                config,
            )
            .expect("effect executor")
        }

        fn qc(&self, phase: wire::GlobalPhase) -> wire::QuorumCertificate {
            wire::QuorumCertificate {
                round: self.manifest.round,
                phase,
                subject: self.manifest.subject,
                execution_commitment: fixture_execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            }
        }
    }

    fn round(context: &wire::HeightContext, view: u64) -> wire::ConsensusRound {
        wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        }
    }

    fn fixture_execution_commitment() -> wire::ExecutionCommitment {
        wire::ExecutionCommitment::without_topups(
            Hash::new(b"effects fixture parent state"),
            Hash::new(b"effects fixture post state"),
            Hash::new(b"effects fixture ordinary writes"),
        )
    }

    fn tag(view: u64) -> EventTag {
        EventTag::new(1, view, Generation::new(7))
    }

    fn vote(fixture: &Fixture) -> wire::Vote {
        wire::Vote {
            round: fixture.manifest.round,
            phase: wire::GlobalPhase::Prepare,
            subject: fixture.manifest.subject,
            execution_commitment: fixture_execution_commitment(),
            signer: 0,
            signature: Vec::new(),
        }
    }

    fn timeout_certificate(fixture: &Fixture) -> wire::TimeoutCertificate {
        wire::TimeoutCertificate {
            round: fixture.manifest.round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            }],
        }
    }

    fn manifest_at_view(fixture: &Fixture, view: u64) -> wire::PayloadManifest {
        wire::PayloadManifest::derive(
            &fixture.context,
            round(&fixture.context, view),
            fixture.manifest.subject,
            u64::try_from(fixture.body.len()).expect("body length"),
            std::slice::from_ref(&fixture.body),
        )
        .expect("view manifest")
    }

    fn timeout_at_view(fixture: &Fixture, view: u64) -> wire::TimeoutCertificate {
        wire::TimeoutCertificate {
            round: round(&fixture.context, view),
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            }],
        }
    }

    fn pending_merge_validation(
        fixture: &Fixture,
    ) -> (
        PendingValidation,
        CertifiedMergeLedgerReference,
        HashOf<MergeLedgerEntry>,
    ) {
        let parent_hash = HashOf::from_untyped_unchecked(Hash::new(b"merge carrier parent"));
        let round = round(&fixture.context, 3);
        let subject = wire::BlockSubject {
            parent_block_hash: Some(parent_hash),
            ..fixture.manifest.subject
        };
        let durable_receipt = DurableBodyReceipt::for_test(
            fixture.context.id(),
            round,
            subject,
            HashOf::new(&fixture.manifest),
        );
        let task = BodyValidationTask {
            id: EffectWorkId(77),
            tag: tag(3),
            round,
            subject,
            durable_receipt,
        };
        let entry_hash = HashOf::from_untyped_unchecked(Hash::new(b"certified merge entry"));
        let reference = CertifiedMergeLedgerReference {
            version: 1,
            entry_hash,
            encoded_len: 512,
            epoch_id: 9,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                2,
                9,
                round.height,
                parent_hash,
                Hash::new(b"chain id"),
                1,
                HashOf::new(&Vec::<PeerId>::new()),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"merge certificate message"),
            ),
        };
        (
            PendingValidation {
                task,
                purpose: ValidationPurpose::Reducer,
            },
            reference,
            entry_hash,
        )
    }

    fn complete_local_proposal_chain(
        executor: &mut V2EffectExecutor<FakeRuntime>,
        services: &mut FakeServices,
    ) {
        let store_id = services.store_tasks.last().expect("local store task").id();
        let store_completion = services.execute_store(store_id);
        executor
            .complete_body_store(store_completion, services)
            .expect("local durable store completion");
        let validation_id = services
            .validation_tasks
            .last()
            .expect("local validation task")
            .id();
        let validation_completion = services.execute_validation(validation_id);
        executor
            .complete_body_validation(validation_completion, services)
            .expect("local validation completion");
    }

    fn persist_fsynced_validation_marker(
        executor: &mut V2EffectExecutor<FakeRuntime>,
        services: &mut FakeServices,
        fixture: &Fixture,
        manifest: wire::PayloadManifest,
    ) {
        executor
            .admit_local_proposal(
                tag(manifest.round.view),
                manifest,
                fixture.body.clone(),
                services,
            )
            .expect("admit exact body before vote signing");
        complete_local_proposal_chain(executor, services);

        // The helper's purpose is only to cross the real body/marker fsync
        // boundary. Keep each caller's assertions focused on the subsequent
        // signature operation.
        executor.runtime.completions.clear();
        services.store_tasks.clear();
        services.validation_tasks.clear();
        services.statuses.clear();
    }

    #[test]
    fn queue_configuration_and_pending_capacity_fail_closed() {
        let fixture = Fixture::new();
        assert!(matches!(
            V2EffectExecutor::with_runtime(
                FakeRuntime::default(),
                BTreeMap::new(),
                fixture.context.clone(),
                PeerId::new(fixture.requester_key.public_key().clone()),
                Some(0),
                EffectQueueConfig::new(0, 1, 1, 1),
            ),
            Err(EffectExecutorError::InvalidQueueConfig)
        ));

        let mut executor = fixture.executor(EffectQueueConfig::new(1, 1, 1_048_576, 1));
        let mut services = fixture.services();
        persist_fsynced_validation_marker(
            &mut executor,
            &mut services,
            &fixture,
            fixture.manifest.clone(),
        );
        let effects = vec![
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            },
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            },
        ];
        assert!(matches!(
            executor.consume_effects(effects, &mut services),
            Err(EffectExecutorError::PendingWorkCapacity { capacity: 1 })
        ));
        assert!(executor.status().fail_closed);
        assert_eq!(services.closed.len(), 1);
    }

    #[test]
    fn local_proposal_async_chain_orders_and_reuses_bounded_work() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        for _ in 0..8 {
            executor
                .admit_local_proposal(
                    tag(0),
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("retry local store");
        }
        assert_eq!(executor.pending_stores.len(), 1);
        assert!(executor.pending_validations.is_empty());
        assert_eq!(services.store_tasks.len(), 8);
        let store_id = services.store_tasks[0].id();
        assert!(
            services
                .store_tasks
                .iter()
                .all(|task| task.id() == store_id)
        );
        assert!(
            !executor
                .runtime
                .completions
                .iter()
                .any(|completion| matches!(completion, RuntimeCompletion::LocalProposal(..)))
        );

        let store_completion = services.execute_store(store_id);
        let duplicate_store = store_completion.clone();
        executor
            .complete_body_store(store_completion, &mut services)
            .expect("durable completion starts validation");
        assert!(executor.pending_stores.is_empty());
        assert_eq!(executor.pending_validations.len(), 1);
        assert_eq!(services.validation_tasks.len(), 1);
        assert_eq!(
            executor
                .complete_body_store(duplicate_store, &mut services)
                .expect("duplicate durable completion"),
            CompletionDisposition::Stale
        );

        for _ in 0..8 {
            executor
                .admit_local_proposal(
                    tag(0),
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("retry local validation");
        }
        assert_eq!(executor.pending_validations.len(), 1);
        let validation_id = services.validation_tasks[0].id();
        assert!(
            services
                .validation_tasks
                .iter()
                .all(|task| task.id() == validation_id)
        );
        let validation_completion = services.execute_validation(validation_id);
        let duplicate_validation = validation_completion.clone();
        executor
            .complete_body_validation(validation_completion, &mut services)
            .expect("validated completion starts proposal");
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::LocalProposal(completion_tag, manifest, durable, validated))
                if *completion_tag == tag(0)
                    && manifest == &fixture.manifest
                    && validated.durable() == durable
        ));
        assert_eq!(
            executor
                .complete_body_validation(duplicate_validation, &mut services)
                .expect("duplicate validation completion"),
            CompletionDisposition::Stale
        );
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn missing_merge_sidecar_retains_exact_validation_until_retry() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
        let work_id = pending.task.id();
        let tag = pending.task.tag();
        let durable = pending.task.durable_receipt().clone();
        let round = pending.task.round();
        let subject = pending.task.subject();
        let task = pending.task.clone();
        executor.pending_validations.insert(work_id, pending);

        let completion = BodyValidationCompletion::DeferredMergeSidecar {
            work_id,
            tag,
            reference: reference.clone(),
        };
        assert_eq!(
            executor
                .complete_body_validation(completion.clone(), &mut services)
                .expect("defer validation for exact merge sidecar"),
            CompletionDisposition::Deferred
        );
        assert_eq!(executor.pending_validations.len(), 1);
        assert_eq!(executor.status().deferred_merge_work, 1);
        assert_eq!(
            services.deferred_merge_sidecars,
            vec![(work_id, round, subject, reference.clone())]
        );
        assert!(executor.runtime.completions.is_empty());
        assert!(services.rejected_validations.is_empty());

        assert_eq!(
            executor
                .complete_body_validation(completion, &mut services)
                .expect("duplicate deferral is idempotent"),
            CompletionDisposition::Deferred
        );
        assert_eq!(services.deferred_merge_sidecars.len(), 1);

        let unrelated_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"unrelated certified merge entry"));
        assert_eq!(
            executor
                .retry_deferred_merge_sidecar(unrelated_hash, &mut services)
                .expect("unrelated sidecar completion is ignored"),
            0
        );
        assert_eq!(executor.status().deferred_merge_work, 1);
        assert_eq!(
            executor
                .retry_deferred_merge_sidecar(entry_hash, &mut services)
                .expect("retry exact deferred validation"),
            1
        );
        assert_eq!(executor.status().deferred_merge_work, 0);
        assert_eq!(services.validation_tasks.last(), Some(&task));
        assert_eq!(executor.pending_validations.len(), 1);

        assert_eq!(
            executor
                .complete_body_validation(
                    BodyValidationCompletion::Validated {
                        work_id,
                        tag,
                        receipt: ValidatedBodyReceipt::for_test(durable),
                    },
                    &mut services,
                )
                .expect("complete exact retried validation"),
            CompletionDisposition::Enqueued
        );
        assert!(executor.pending_validations.is_empty());
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::ValidationSucceeded(
                completion_tag,
                completion_round,
                completion_subject,
                _
            )) if *completion_tag == tag
                && *completion_round == round
                && *completion_subject == subject
        ));
    }

    #[test]
    fn uniquely_invalid_merge_sidecar_terminally_rejects_exact_deferred_work() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
        let work_id = pending.task.id();
        let tag = pending.task.tag();
        let round = pending.task.round();
        let subject = pending.task.subject();
        executor.pending_validations.insert(work_id, pending);
        executor
            .complete_body_validation(
                BodyValidationCompletion::DeferredMergeSidecar {
                    work_id,
                    tag,
                    reference,
                },
                &mut services,
            )
            .expect("defer validation");

        let unrelated_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"unrelated certified merge entry"));
        assert_eq!(
            executor
                .reject_deferred_merge_sidecar(
                    unrelated_hash,
                    "invalid unrelated entry",
                    &mut services
                )
                .expect("ignore unrelated rejection"),
            0
        );
        assert_eq!(executor.pending_validations.len(), 1);
        assert_eq!(
            executor
                .reject_deferred_merge_sidecar(entry_hash, "invalid certified entry", &mut services)
                .expect("reject exact deferred entry"),
            1
        );
        assert!(executor.pending_validations.is_empty());
        assert_eq!(executor.status().deferred_merge_work, 0);
        assert_eq!(
            services.rejected_validations,
            vec!["invalid certified entry".to_owned()]
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::ValidationFailed(
                completion_tag,
                completion_round,
                completion_subject
            )) if *completion_tag == tag
                && *completion_round == round
                && *completion_subject == subject
        ));
    }

    #[test]
    fn conflicting_reference_registration_rejects_only_its_exact_work_id() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (first, first_reference, entry_hash) = pending_merge_validation(&fixture);
        let first_id = first.task.id();
        let mut second = first.clone();
        second.task.id = EffectWorkId(78);
        second.task.subject.block_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"conflicting second carrier"));
        second.task.durable_receipt = DurableBodyReceipt::for_test(
            fixture.context.id(),
            second.task.round,
            second.task.subject,
            HashOf::new(&fixture.manifest),
        );
        let second_id = second.task.id();
        let mut second_reference = first_reference.clone();
        second_reference.encoded_len += 1;
        executor.pending_validations.insert(first_id, first);
        executor
            .pending_validations
            .insert(second_id, second.clone());

        for (work_id, tag, reference) in [
            (first_id, tag(3), first_reference),
            (second_id, tag(3), second_reference),
        ] {
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id,
                        tag,
                        reference,
                    },
                    &mut services,
                )
                .expect("retain independently keyed deferral");
        }
        assert_eq!(executor.status().deferred_merge_work, 2);

        assert_eq!(
            executor
                .reject_deferred_merge_sidecar_work(
                    second_id,
                    "conflicting compact reference metadata",
                    &mut services,
                )
                .expect("reject only conflicting registration"),
            CompletionDisposition::Enqueued
        );
        assert!(!executor.pending_validations.contains_key(&second_id));
        assert!(executor.pending_validations.contains_key(&first_id));
        assert_eq!(
            executor.deferred_merge_work.get(&first_id),
            Some(&entry_hash)
        );
        assert_eq!(executor.status().deferred_merge_work, 1);
    }

    #[test]
    fn decided_apply_retries_after_exact_merge_sidecar_recovery() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending_validation, reference, entry_hash) = pending_merge_validation(&fixture);
        let work_id = EffectWorkId(91);
        let mut certificate = fixture.qc(wire::GlobalPhase::Commit);
        certificate.round = pending_validation.task.round();
        certificate.subject = pending_validation.task.subject();
        let task = ApplyTask {
            id: work_id,
            tag: pending_validation.task.tag(),
            subject: pending_validation.task.subject(),
            certificate,
            validated_receipt: ValidatedBodyReceipt::for_test(
                pending_validation.task.durable_receipt().clone(),
            ),
        };
        executor
            .pending_applications
            .insert(work_id, PendingApply { task: task.clone() });

        assert_eq!(
            executor
                .defer_application_for_merge_sidecar(work_id, &reference, &mut services)
                .expect("defer decided apply"),
            CompletionDisposition::Deferred
        );
        assert_eq!(executor.status().deferred_merge_work, 1);
        assert_eq!(services.apply_tasks.len(), 0);
        assert_eq!(
            executor
                .retry_deferred_merge_sidecar(entry_hash, &mut services)
                .expect("retry decided apply after sidecar persistence"),
            1
        );
        assert_eq!(
            services.apply_tasks.last().map(ApplyTask::id),
            Some(work_id)
        );
        assert_eq!(executor.status().deferred_merge_work, 0);
        assert!(executor.pending_applications.contains_key(&work_id));
    }

    #[test]
    fn deferred_merge_sidecar_must_match_carrier_height_parent_and_round_ceiling() {
        for mismatch in 0..3 {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let (pending, mut reference, _) = pending_merge_validation(&fixture);
            let work_id = pending.task.id();
            let tag = pending.task.tag();
            match mismatch {
                0 => {
                    reference.merge_qc.carrier_height =
                        reference.merge_qc.carrier_height.saturating_add(1);
                }
                1 => {
                    reference.merge_qc.carrier_parent_hash = HashOf::from_untyped_unchecked(
                        Hash::new(b"different merge carrier parent"),
                    );
                }
                2 => {
                    reference.merge_qc.view = pending.task.round().view.saturating_add(1);
                }
                _ => unreachable!(),
            }
            executor.pending_validations.insert(work_id, pending);
            assert!(matches!(
                executor.complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id,
                        tag,
                        reference,
                    },
                    &mut services,
                ),
                Err(EffectExecutorError::BodyStore(_))
            ));
            assert!(executor.status().fail_closed);
            assert_eq!(executor.pending_validations.len(), 1);
            assert_eq!(executor.status().deferred_merge_work, 0);
            assert!(services.deferred_merge_sidecars.is_empty());
            assert!(executor.runtime.completions.is_empty());
        }
    }

    #[test]
    fn certified_view_prunes_unprotected_merge_sidecar_work_but_keeps_high_qc_subject() {
        for protected in [false, true] {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
            let work_id = pending.task.id();
            let subject = pending.task.subject();
            let round = pending.task.round();
            let completion_tag = pending.task.tag();
            executor.pending_validations.insert(work_id, pending);
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id,
                        tag: completion_tag,
                        reference,
                    },
                    &mut services,
                )
                .expect("defer exact prior-view work");

            let mut timeout = timeout_at_view(&fixture, round.view);
            if protected {
                let mut highest = fixture.qc(wire::GlobalPhase::Prepare);
                highest.round = round;
                highest.subject = subject;
                timeout.groups[0].highest_prepare_qc = Some(highest);
            }
            executor
                .install_view(tag(round.view + 1), timeout, &mut services)
                .expect("install certified next view");

            assert_eq!(
                executor.retains_deferred_merge_sidecar(work_id, round, subject, entry_hash),
                protected
            );
            assert_eq!(
                executor.pending_validations.contains_key(&work_id),
                protected
            );
            assert_eq!(
                executor.status().deferred_merge_work,
                if protected { 1 } else { 0 }
            );
        }
    }

    #[test]
    fn certified_view_protects_only_the_exact_high_qc_round_for_one_subject() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (first, reference, entry_hash) = pending_merge_validation(&fixture);
        let first_id = first.task.id();
        let subject = first.task.subject();
        let first_round = first.task.round();
        let second_round = round(&fixture.context, first_round.view + 1);
        let second_id = EffectWorkId(79);
        let mut second = first.clone();
        second.task.id = second_id;
        second.task.tag = tag(second_round.view);
        second.task.round = second_round;
        second.task.durable_receipt = DurableBodyReceipt::for_test(
            fixture.context.id(),
            second_round,
            subject,
            HashOf::new(&fixture.manifest),
        );
        executor.pending_validations.insert(first_id, first);
        executor.pending_validations.insert(second_id, second);

        for (work_id, completion_tag) in [(first_id, tag(3)), (second_id, tag(4))] {
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id,
                        tag: completion_tag,
                        reference: reference.clone(),
                    },
                    &mut services,
                )
                .expect("defer same-subject validation round");
        }

        let mut timeout = timeout_at_view(&fixture, second_round.view);
        let mut highest = fixture.qc(wire::GlobalPhase::Prepare);
        highest.round = second_round;
        highest.subject = subject;
        timeout.groups[0].highest_prepare_qc = Some(highest);
        executor
            .install_view(tag(second_round.view + 1), timeout, &mut services)
            .expect("install certified view with exact high PrepareQC");

        assert!(!executor.retains_deferred_merge_sidecar(
            first_id,
            first_round,
            subject,
            entry_hash
        ));
        assert!(executor.retains_deferred_merge_sidecar(
            second_id,
            second_round,
            subject,
            entry_hash
        ));
        assert_eq!(executor.status().deferred_merge_work, 1);
    }

    #[test]
    fn view_change_cancels_non_durable_store_and_unprotected_validation() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("queue store");
        let store_id = services.store_tasks[0].id();
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, 1, Generation::new(7)),
                    certificate: timeout_at_view(&fixture, 0),
                }],
                &mut services,
            )
            .expect("install view");
        assert!(executor.pending_stores.is_empty());
        assert_eq!(services.cancelled_stores, vec![store_id]);

        let late_completion = services.execute_store(store_id);
        assert_eq!(
            executor
                .complete_body_store(late_completion, &mut services)
                .expect("late durable completion is retained"),
            CompletionDisposition::Stale
        );
        assert!(
            executor
                .durable_bodies
                .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
        );

        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("durable body starts validation");
        assert_eq!(executor.pending_validations.len(), 1);
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, 1, Generation::new(7)),
                    certificate: timeout_at_view(&fixture, 0),
                }],
                &mut services,
            )
            .expect("reinstall view for validation cancellation");
        assert!(
            executor.pending_validations.is_empty(),
            "a durable body remains reusable, but its stale validation survives only when the TC protects its exact high PrepareQC"
        );
        assert!(
            executor
                .durable_bodies
                .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
        );
        assert_eq!(services.cancelled_stores, vec![store_id]);
    }

    #[test]
    fn vote_signing_requires_the_exact_fsynced_execution_commitment() {
        let fixture = Fixture::new();
        let mut missing = fixture.executor(EffectQueueConfig::default());
        let mut missing_services = fixture.services();
        assert!(matches!(
            missing.consume_effects(
                vec![AdapterEffect::Sign {
                    tag: tag(0),
                    request: SignRequest::Vote(vote(&fixture)),
                }],
                &mut missing_services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("fsynced validation marker")
        ));
        assert!(missing.status().fail_closed);
        assert!(missing_services.sign_tasks.is_empty());

        let mut drift = fixture.executor(EffectQueueConfig::default());
        let mut drift_services = fixture.services();
        persist_fsynced_validation_marker(
            &mut drift,
            &mut drift_services,
            &fixture,
            fixture.manifest.clone(),
        );
        let mut drifted_vote = vote(&fixture);
        drifted_vote.execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"drifted effects fixture parent state"),
            Hash::new(b"drifted effects fixture post state"),
            Hash::new(b"drifted effects fixture ordinary writes"),
        );
        assert!(matches!(
            drift.consume_effects(
                vec![AdapterEffect::Sign {
                    tag: tag(0),
                    request: SignRequest::Vote(drifted_vote),
                }],
                &mut drift_services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("differs from the durable validation marker")
        ));
        assert!(drift.status().fail_closed);
        assert!(drift_services.sign_tasks.is_empty());
    }

    #[test]
    fn sign_effect_verifies_signature_and_preserves_original_tag() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        persist_fsynced_validation_marker(
            &mut executor,
            &mut services,
            &fixture,
            fixture.manifest.clone(),
        );
        let request = SignRequest::Vote(vote(&fixture));
        executor
            .consume_effects(
                vec![AdapterEffect::Sign {
                    tag: tag(0),
                    request: request.clone(),
                }],
                &mut services,
            )
            .expect("consume sign");
        let task = services.sign_tasks[0].clone();
        let preimage = match task.request() {
            SignRequest::Vote(vote) => vote.signature_preimage(),
            _ => panic!("vote task expected"),
        };
        let signature = Signature::new(fixture.validator_keys[0].private_key(), &preimage)
            .payload()
            .to_vec();
        assert_eq!(
            executor
                .complete_consensus_signature(task.id(), signature.clone(), &mut services)
                .expect("complete signature"),
            CompletionDisposition::Enqueued
        );
        assert!(matches!(
            &executor.runtime.completions[0],
            RuntimeCompletion::Signature(completion_tag, completion)
                if *completion_tag == tag(0) && completion == &signature
        ));
        assert_eq!(
            executor
                .complete_consensus_signature(task.id(), signature, &mut services)
                .expect("stale completion"),
            CompletionDisposition::Stale
        );
    }

    #[test]
    fn invalid_signer_completion_fails_closed_without_runtime_input() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        persist_fsynced_validation_marker(
            &mut executor,
            &mut services,
            &fixture,
            fixture.manifest.clone(),
        );
        executor
            .consume_effects(
                vec![AdapterEffect::Sign {
                    tag: tag(0),
                    request: SignRequest::Vote(vote(&fixture)),
                }],
                &mut services,
            )
            .expect("consume sign");
        let id = services.sign_tasks[0].id();
        let wrong = Signature::new(fixture.validator_keys[1].private_key(), b"wrong")
            .payload()
            .to_vec();
        assert!(matches!(
            executor.complete_consensus_signature(id, wrong, &mut services),
            Err(EffectExecutorError::InvalidConsensusSignature(_))
        ));
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.status().fail_closed);
    }

    #[test]
    fn broadcast_view_and_evidence_effects_reach_exact_hooks() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let message = wire::ConsensusMessageV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            payload: wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                signature: vec![1],
                ..vote(&fixture)
            }),
        };
        executor
            .consume_effects(
                vec![
                    AdapterEffect::Broadcast(message.clone()),
                    AdapterEffect::EnterView {
                        tag: tag(1),
                        certificate: timeout_certificate(&fixture),
                    },
                    AdapterEffect::ReportEquivocation {
                        evidence: wire::SumeragiV2Equivocation::PhaseVote {
                            first: vote(&fixture),
                            second: wire::Vote {
                                subject: wire::BlockSubject {
                                    block_hash: HashOf::from_untyped_unchecked(Hash::new(
                                        b"conflicting-v2-effect-vote",
                                    )),
                                    ..fixture.manifest.subject
                                },
                                ..vote(&fixture)
                            },
                        },
                    },
                    AdapterEffect::ReportInvalidCertifiedBody {
                        subject: fixture.manifest.subject,
                        certificate: fixture.qc(wire::GlobalPhase::Prepare),
                    },
                ],
                &mut services,
            )
            .expect("consume immediate effects");
        assert_eq!(services.broadcasts, vec![message]);
        assert_eq!(services.entered_views, vec![tag(1)]);
        assert_eq!(services.equivocations.len(), 1);
        assert_eq!(services.invalid_bodies, vec![fixture.manifest.subject]);
    }

    #[test]
    fn ordinary_fetch_authenticates_chunks_and_runs_store_validate_pipeline() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("begin fetch");
        let work_id = services.fetch_tasks[0].id();
        let mut chunk = wire::PayloadChunk {
            manifest_hash: HashOf::new(&fixture.manifest),
            index: 0,
            bytes: fixture.body.clone(),
            sender: 0,
            signature: Vec::new(),
        };
        chunk.signature = Signature::new(
            fixture.validator_keys[0].private_key(),
            &chunk
                .signature_preimage(&fixture.context, &fixture.manifest)
                .expect("chunk preimage"),
        )
        .payload()
        .to_vec();
        executor
            .accept_payload_chunk(
                work_id,
                chunk,
                &fixture.context.roster[0].validator,
                &mut services,
            )
            .expect("authenticated chunk");
        assert_eq!(services.chunks, vec![work_id]);
        executor
            .complete_body_reconstruction(
                work_id,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("body reconstruction");
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(0)
                    && manifest == &fixture.manifest
        ));

        for _ in 0..8 {
            executor
                .consume_effects(
                    vec![AdapterEffect::StoreBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    }],
                    &mut services,
                )
                .expect("retry store body");
        }
        assert_eq!(executor.pending_stores.len(), 1);
        assert!(
            services
                .store_tasks
                .iter()
                .all(|task| task.id() == services.store_tasks[0].id())
        );
        let store_id = services.store_tasks.last().expect("store task").id();
        let store_completion = services.execute_store(store_id);
        executor
            .complete_body_store(store_completion, &mut services)
            .expect("durable store completion");
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyStored(completion_tag, round, subject, receipt))
                if *completion_tag == tag(0)
                    && *round == fixture.manifest.round
                    && *subject == fixture.manifest.subject
                    && receipt.subject() == fixture.manifest.subject
        ));
        for _ in 0..8 {
            executor
                .consume_effects(
                    vec![AdapterEffect::ValidateBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    }],
                    &mut services,
                )
                .expect("retry validation");
        }
        assert_eq!(executor.pending_validations.len(), 1);
        assert!(
            services
                .validation_tasks
                .iter()
                .all(|task| task.id() == services.validation_tasks[0].id())
        );
        let validation_id = services
            .validation_tasks
            .last()
            .expect("validation task")
            .id();
        let validation_completion = services.execute_validation(validation_id);
        executor
            .complete_body_validation(validation_completion, &mut services)
            .expect("validation completion");
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::ValidationSucceeded(completion_tag, round, subject, receipt))
                if *completion_tag == tag(0)
                    && *round == fixture.manifest.round
                    && *subject == fixture.manifest.subject
                    && receipt.durable().subject() == fixture.manifest.subject
        ));
    }

    #[test]
    fn validation_rejection_enqueues_failure_without_success_receipt() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_ready_body_for_test(&fixture, &mut services)
            .expect("ready body");
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("store body");
        let store_id = services.store_tasks.last().expect("store task").id();
        let store_completion = services.execute_store(store_id);
        executor
            .complete_body_store(store_completion, &mut services)
            .expect("store completion");
        services.validation_error = Some("invalid transaction".to_owned());
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("queue validation");
        let validation_id = services
            .validation_tasks
            .last()
            .expect("validation task")
            .id();
        let validation_completion = services.execute_validation(validation_id);
        executor
            .complete_body_validation(validation_completion, &mut services)
            .expect("validation rejection is protocol input");
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::ValidationFailed(completion_tag, round, subject))
                if *completion_tag == tag(0)
                    && *round == fixture.manifest.round
                    && *subject == fixture.manifest.subject
        ));
        assert_eq!(services.rejected_validations, vec!["invalid transaction"]);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn certified_response_is_bound_to_exact_request_and_consumed_once() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: None,
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("certified fetch");
        let request = services.fetch_tasks[0]
            .certified_request()
            .expect("signed request")
            .clone();
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(&request),
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder: 0,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.validator_keys[0].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert_eq!(
            executor
                .accept_certified_body_response(
                    response.clone(),
                    &fixture.context.roster[0].validator,
                    &mut services,
                )
                .expect("authenticated certified response"),
            CompletionDisposition::Enqueued
        );
        assert!(matches!(
            executor.accept_certified_body_response(
                response,
                &fixture.context.roster[0].validator,
                &mut services,
            ),
            Err(EffectTransportError::Authentication(
                V2TransportError::UnsolicitedResponse(_)
            ))
        ));
    }

    #[test]
    fn apply_requires_validated_body_and_typed_exact_kura_completion() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("local proposal");
        complete_local_proposal_chain(&mut executor, &mut services);
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::LocalProposal(completion_tag, manifest, durable, validated))
                if *completion_tag == tag(0)
                    && manifest == &fixture.manifest
                    && durable.subject() == fixture.manifest.subject
                    && validated.durable() == durable
        ));
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor
            .consume_effects(
                vec![AdapterEffect::Apply {
                    tag: tag(0),
                    subject: fixture.manifest.subject,
                    certificate: commit.clone(),
                }],
                &mut services,
            )
            .expect("begin application");
        let task = &services.apply_tasks[0];
        assert_eq!(task.tag(), tag(0));
        assert_eq!(task.subject(), fixture.manifest.subject);
        assert_eq!(task.certificate(), &commit);
        assert_eq!(
            task.validated_receipt().durable().subject(),
            fixture.manifest.subject
        );
        let work_id = task.id();
        let artifact = wire::finality::V2FinalityArtifact::new(
            fixture.context.clone(),
            fixture.manifest.subject,
            commit,
            vec![vec![0x5C]; fixture.context.roster.len()],
        );
        let receipt = KuraV2CommitReceipt::for_test(&artifact);
        assert_eq!(
            executor
                .complete_application(
                    DurableApplyCompletion::new(work_id, receipt, artifact.clone()),
                    &mut services,
                )
                .expect("durable application"),
            CompletionDisposition::Enqueued
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::Application(completion_tag, subject))
                if *completion_tag == tag(0) && *subject == fixture.manifest.subject
        ));
        assert_eq!(
            executor.durable_finality().expect("durable finality").1,
            &artifact
        );
    }

    #[test]
    fn apply_accepts_decided_old_view_but_rejects_wrong_height_tag() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("local proposal");
        complete_local_proposal_chain(&mut executor, &mut services);

        let commit = fixture.qc(wire::GlobalPhase::Commit);
        assert!(matches!(
            executor.begin_apply(
                EventTag::new(2, 3, Generation::new(7)),
                fixture.manifest.subject,
                commit.clone(),
                &mut services,
            ),
            Err(EffectExecutorError::Contract(_))
        ));
        assert!(executor.pending_applications.is_empty());
        assert!(services.apply_tasks.is_empty());

        executor
            .begin_apply(
                tag(3),
                fixture.manifest.subject,
                commit.clone(),
                &mut services,
            )
            .expect("a delayed decided CommitQC remains actionable");
        assert_eq!(executor.pending_applications.len(), 1);
        assert_eq!(services.apply_tasks.len(), 1);
        assert_eq!(services.apply_tasks[0].tag(), tag(3));
        assert_eq!(services.apply_tasks[0].certificate(), &commit);
    }

    #[test]
    fn pending_kura_tip_requires_exact_decision_body_and_validation_replay() {
        let fixture = Fixture::new();
        let directory = TempDir::new().expect("body-store directory");
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("open body store");
        let durable = store
            .store(fixture.manifest.clone(), fixture.body.clone())
            .expect("persist exact body");
        let validated = store
            .validate(&durable, |_| {
                Ok::<_, &'static str>(
                    ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment(),
                )
            })
            .expect("persist validation marker");
        drop(store);
        let reopened = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("reopen exact body store");
        let recovered = reopened.recovery_catalog().expect("recovery catalog");
        let validations = reopened.validated_recovery_catalog();
        assert_eq!(
            validations
                .get(&(fixture.manifest.round, fixture.manifest.subject))
                .map(ValidatedBodyReceipt::durable),
            Some(&durable)
        );
        let expected = PendingKuraApply::for_test(
            fixture.context.id(),
            fixture.context.height,
            fixture.block.hash(),
        );
        let decision = Some((
            fixture.manifest.round,
            fixture.manifest.subject,
            validated.execution_commitment(),
        ));

        verify_pending_kura_apply_parts(
            &fixture.context,
            decision,
            &recovered,
            &validations,
            expected,
        )
        .expect("exact replay binding");
        assert!(matches!(
            verify_pending_kura_apply_parts(
                &fixture.context,
                None,
                &recovered,
                &validations,
                expected,
            ),
            Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
                if reason.contains("no complete durable Decision")
        ));

        let wrong_tip = PendingKuraApply::for_test(
            fixture.context.id(),
            fixture.context.height,
            HashOf::from_untyped_unchecked(Hash::new(b"different Kura tip")),
        );
        assert!(matches!(
            verify_pending_kura_apply_parts(
                &fixture.context,
                decision,
                &recovered,
                &validations,
                wrong_tip,
            ),
            Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
                if reason.contains("does not identify the canonical")
        ));

        assert!(matches!(
            verify_pending_kura_apply_parts(
                &fixture.context,
                decision,
                &recovered,
                &BTreeMap::new(),
                expected,
            ),
            Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
                if reason.contains("no matching durable validation marker")
        ));

        let mismatched_execution_commitment = fixture_execution_commitment();
        assert_ne!(
            mismatched_execution_commitment,
            validated.execution_commitment(),
            "the adversarial Decision fixture must change the consensus-bound execution result"
        );
        assert!(matches!(
            verify_pending_kura_apply_parts(
                &fixture.context,
                Some((
                    fixture.manifest.round,
                    fixture.manifest.subject,
                    mismatched_execution_commitment,
                )),
                &recovered,
                &validations,
                expected,
            ),
            Err(EffectExecutorError::PendingApplyRecoveryMismatch(reason))
                if reason.contains("Decision commitment differs")
        ));
        assert_eq!(validated.durable(), &durable);
    }

    #[test]
    fn mismatched_kura_completion_fails_closed_before_application_ack() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("local proposal");
        complete_local_proposal_chain(&mut executor, &mut services);
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor
            .consume_effects(
                vec![AdapterEffect::Apply {
                    tag: tag(0),
                    subject: fixture.manifest.subject,
                    certificate: commit.clone(),
                }],
                &mut services,
            )
            .expect("begin apply");
        let work_id = services.apply_tasks[0].id();
        let mut artifact = wire::finality::V2FinalityArtifact::new(
            fixture.context.clone(),
            fixture.manifest.subject,
            commit,
            vec![vec![0x5D]; fixture.context.roster.len()],
        );
        artifact.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong block"));
        let receipt = KuraV2CommitReceipt::for_test(&artifact);
        let completions_before = executor.runtime.completions.len();
        assert!(matches!(
            executor.complete_application(
                DurableApplyCompletion::new(work_id, receipt, artifact),
                &mut services,
            ),
            Err(EffectExecutorError::InvalidApplyCompletion)
        ));
        assert_eq!(executor.runtime.completions.len(), completions_before);
        assert!(executor.status().fail_closed);
    }

    #[test]
    fn service_runtime_body_store_and_status_failures_close_executor() {
        let fixture = Fixture::new();

        let mut service_executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        services.fail_on = Some("broadcast");
        let message = wire::ConsensusMessageV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            payload: wire::ConsensusMessageV2Payload::Vote(wire::Vote {
                signature: vec![1],
                ..vote(&fixture)
            }),
        };
        assert!(matches!(
            service_executor
                .consume_effects(vec![AdapterEffect::Broadcast(message)], &mut services),
            Err(EffectExecutorError::Service(_))
        ));
        assert!(service_executor.status().fail_closed);

        let mut runtime_executor = fixture.executor(EffectQueueConfig::default());
        let mut runtime_services = fixture.services();
        runtime_executor
            .runtime
            .steps
            .push_back(Err("driver failed".to_owned()));
        assert!(matches!(
            runtime_executor.step(Instant::now(), &mut runtime_services),
            Err(EffectExecutorError::Runtime(_))
        ));

        let mut body_executor = fixture.executor(EffectQueueConfig::default());
        let mut body_services = fixture.services();
        body_executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut body_services,
            )
            .expect("queue asynchronous body store");
        let store_id = body_services.store_tasks[0].id();
        assert!(matches!(
            body_executor.body_service_failed(store_id, "fsync failed", &mut body_services,),
            Err(EffectExecutorError::BodyStore(_))
        ));

        let mut status_executor = fixture.executor(EffectQueueConfig::default());
        let mut status_services = fixture.services();
        status_services.fail_on = Some("status");
        assert!(matches!(
            status_executor.consume_effects(Vec::new(), &mut status_services),
            Err(EffectExecutorError::Service(_))
        ));
        assert!(status_executor.status().fail_closed);
    }

    #[test]
    fn ready_body_backpressure_and_mismatches_are_nonfatal_transport_rejections() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(8, 1, 1, 4));
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("fetch");
        let work = services.fetch_tasks[0].id();
        assert!(matches!(
            executor.complete_body_reconstruction(
                work,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            ),
            Err(EffectTransportError::Backpressure)
        ));
        assert!(!executor.status().fail_closed);
        let mut wrong = fixture.body.clone();
        wrong[0] ^= 1;
        assert!(matches!(
            executor.complete_body_reconstruction(
                work,
                fixture.manifest.clone(),
                wrong,
                &mut services,
            ),
            Err(EffectTransportError::BodyMismatch)
        ));
    }

    #[test]
    fn fetch_retransmissions_reuse_one_work_slot_and_one_signed_request() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = prepare
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        let effect = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: None,
            certified_sources: sources,
            certificate: Some(prepare),
        };
        for _ in 0..8 {
            executor
                .consume_effects(vec![effect.clone()], &mut services)
                .expect("retransmit exact certified fetch");
        }
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);
        assert_eq!(services.fetch_tasks.len(), 8);
        let first_id = services.fetch_tasks[0].id();
        let first_request = services.fetch_tasks[0]
            .certified_request()
            .expect("certified request")
            .clone();
        assert!(services.fetch_tasks.iter().all(|task| {
            task.id() == first_id && task.certified_request() == Some(&first_request)
        }));
    }

    #[test]
    fn conflicting_fetch_retransmission_fails_closed() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let effect = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        executor
            .consume_effects(vec![effect], &mut services)
            .expect("first fetch");
        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: EventTag::new(1, 0, Generation::new(8)),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            ),
            Err(EffectExecutorError::Contract(_))
        ));
        assert_eq!(services.fetch_tasks.len(), 1);
        assert!(executor.status().fail_closed);
    }

    #[test]
    fn apply_retransmissions_reuse_one_work_slot() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("local proposal");
        complete_local_proposal_chain(&mut executor, &mut services);
        let effect = AdapterEffect::Apply {
            tag: tag(0),
            subject: fixture.manifest.subject,
            certificate: fixture.qc(wire::GlobalPhase::Commit),
        };
        for _ in 0..8 {
            executor
                .consume_effects(vec![effect.clone()], &mut services)
                .expect("retransmit exact apply");
        }
        assert_eq!(executor.pending_applications.len(), 1);
        assert_eq!(services.apply_tasks.len(), 8);
        let id = services.apply_tasks[0].id();
        assert!(services.apply_tasks.iter().all(|task| task.id() == id));
    }

    #[test]
    fn certified_view_churn_cancels_stale_fetches_and_releases_capacity() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        for view in 0..6 {
            let manifest = manifest_at_view(&fixture, view);
            let certificate = wire::QuorumCertificate {
                round: manifest.round,
                phase: wire::GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: fixture_execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            };
            let sources = certificate
                .signers
                .iter()
                .map(|index| fixture.context.roster[*index as usize].validator.clone())
                .collect();
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: EventTag::new(1, view, Generation::new(7)),
                        round: manifest.round,
                        subject: manifest.subject,
                        manifest: None,
                        certified_sources: sources,
                        certificate: Some(certificate),
                    }],
                    &mut services,
                )
                .expect("begin view fetch");
            assert_eq!(executor.pending_fetches.len(), 1);
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: EventTag::new(1, view + 1, Generation::new(7)),
                        certificate: timeout_at_view(&fixture, view),
                    }],
                    &mut services,
                )
                .expect("install next view");
            assert!(executor.pending_fetches.is_empty());
            assert!(executor.outstanding_requests.is_empty());
        }
        assert_eq!(services.cancelled_fetches.len(), 6);
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn certified_view_churn_cancels_stale_signing_and_releases_capacity() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
        let mut services = fixture.services();
        let mut stale_ids = Vec::new();
        for view in 0..6 {
            let manifest = manifest_at_view(&fixture, view);
            persist_fsynced_validation_marker(
                &mut executor,
                &mut services,
                &fixture,
                manifest.clone(),
            );
            executor
                .consume_effects(
                    vec![AdapterEffect::Sign {
                        tag: EventTag::new(1, view, Generation::new(7)),
                        request: SignRequest::Vote(wire::Vote {
                            round: manifest.round,
                            phase: wire::GlobalPhase::Prepare,
                            subject: manifest.subject,
                            execution_commitment: fixture_execution_commitment(),
                            signer: 0,
                            signature: Vec::new(),
                        }),
                    }],
                    &mut services,
                )
                .expect("begin view signing");
            stale_ids.push(services.sign_tasks.last().expect("sign task").id());
            assert_eq!(executor.pending_signatures.len(), 1);
            executor
                .consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: EventTag::new(1, view + 1, Generation::new(7)),
                        certificate: timeout_at_view(&fixture, view),
                    }],
                    &mut services,
                )
                .expect("install next view");
            assert!(executor.pending_signatures.is_empty());
        }
        assert_eq!(services.cancelled_signatures, stale_ids);
        let late_signature = Signature::new(
            fixture.validator_keys[0].private_key(),
            b"late completion is never admitted",
        )
        .payload()
        .to_vec();
        assert_eq!(
            executor
                .complete_consensus_signature(stale_ids[0], late_signature, &mut services)
                .expect("late signature is stale"),
            CompletionDisposition::Stale
        );
        assert!(!executor.status().fail_closed);
    }

    #[test]
    fn certified_sources_must_exactly_match_canonical_qc_signers() {
        let fixture = Fixture::new();
        let certificate = fixture.qc(wire::GlobalPhase::Prepare);
        let canonical = certificate
            .signers
            .iter()
            .map(|index| fixture.context.roster[*index as usize].validator.clone())
            .collect::<Vec<_>>();
        for bad_sources in [
            vec![
                canonical[0].clone(),
                canonical[0].clone(),
                canonical[2].clone(),
            ],
            vec![
                canonical[1].clone(),
                canonical[0].clone(),
                canonical[2].clone(),
            ],
        ] {
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            assert!(matches!(
                executor.consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: None,
                        certified_sources: bad_sources,
                        certificate: Some(certificate.clone()),
                    }],
                    &mut services,
                ),
                Err(EffectExecutorError::Contract(_))
            ));
            assert!(services.fetch_tasks.is_empty());
            assert!(executor.status().fail_closed);
        }
    }

    #[test]
    fn reopened_durable_receipt_satisfies_fetch_without_network() {
        let fixture = Fixture::new();
        let directory = TempDir::new().expect("recovery directory");
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("open body store");
        let task = BodyStoreTask {
            id: EffectWorkId(91),
            tag: tag(0),
            manifest: fixture.manifest.clone(),
            canonical_wire: Arc::from(fixture.body.clone()),
        };
        let durable = store
            .execute_store_task(&task)
            .expect("persist body before crash");
        let receipt = durable.receipt().clone();
        drop(store);
        let reopened = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("reopen body store");
        let catalog = reopened.recovery_catalog().expect("recovery catalog");
        let mut executor = V2EffectExecutor::with_runtime(
            FakeRuntime::default(),
            catalog,
            fixture.context.clone(),
            PeerId::new(fixture.requester_key.public_key().clone()),
            Some(0),
            EffectQueueConfig::default(),
        )
        .expect("recovered executor");
        let mut services = FakeServices {
            _body_directory: Some(directory),
            body_store: Some(reopened),
            requester_key: Some(fixture.requester_key.clone()),
            ..FakeServices::default()
        };
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("recover local durable body");
        assert!(services.fetch_tasks.is_empty());
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(0) && manifest == &fixture.manifest
        ));
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("acknowledge recovered durability");
        assert_eq!(
            executor
                .durable_bodies
                .get(&(fixture.manifest.round, fixture.manifest.subject)),
            Some(&receipt)
        );
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("queue recovered validation");
        let validation_id = services.validation_tasks[0].id();
        let completion = services.execute_validation(validation_id);
        executor
            .complete_body_validation(completion, &mut services)
            .expect("validate reopened exact body");
    }

    #[test]
    fn runtime_step_consumes_effect_batch_and_idle_publishes_status() {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        persist_fsynced_validation_marker(
            &mut executor,
            &mut services,
            &fixture,
            fixture.manifest.clone(),
        );
        executor
            .runtime
            .steps
            .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            }])));
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("advanced step"),
            EffectExecutorStep::Advanced { effects: 1 }
        );
        assert_eq!(
            executor
                .step(Instant::now(), &mut services)
                .expect("idle step"),
            EffectExecutorStep::Idle
        );
        assert_eq!(services.sign_tasks.len(), 1);
        assert_eq!(services.statuses.len(), 2);
    }

    impl V2EffectExecutor<FakeRuntime> {
        fn admit_ready_body_for_test(
            &mut self,
            fixture: &Fixture,
            services: &mut FakeServices,
        ) -> Result<(), EffectExecutorError> {
            let id = self.allocate_work_id()?;
            self.pending_fetches.insert(
                id,
                PendingFetch {
                    task: BodyFetchTask {
                        id,
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        sources: Vec::new(),
                        certified_request: None,
                    },
                    request_hash: None,
                },
            );
            self.finish_fetch(id, fixture.manifest.clone(), fixture.body.clone(), services)
                .map(|_| ())
                .map_err(|error| EffectExecutorError::Contract(error.to_string()))
        }
    }
}
